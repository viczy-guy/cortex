//! Application state: the session board, the approval queue and the map of
//! connections waiting for a decision.

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use cortex_core::{
    Answers, ApprovalQueue, Decision, DecisionResponse, HookEvent, PendingApproval, Registered,
    SessionBoard,
};
use serde::Serialize;
use tokio::sync::{oneshot, Notify};

/// Serializable snapshot sent to the island on every change.
#[derive(Debug, Clone, Serialize)]
pub struct Snapshot {
    pub sessions: Vec<cortex_core::Session>,
    pub approvals: Vec<PendingApproval>,
}

/// What a connection does after registering a blocking event.
pub enum Blocking {
    /// Reply right away (Always rule, or nothing Cortex can show).
    Immediate(DecisionResponse),
    /// A card is up: wait for the user on this receiver.
    Wait(oneshot::Receiver<DecisionResponse>),
}

#[derive(Default)]
struct Domain {
    board: SessionBoard,
    approvals: ApprovalQueue,
}

#[derive(Default)]
pub struct AppState {
    domain: Mutex<Domain>,
    /// Connections parked on a pending request, keyed by request id.
    responders: Mutex<HashMap<String, oneshot::Sender<DecisionResponse>>>,
    /// Wakes the janitor task when there is something to clean later.
    pub janitor: Notify,
}

pub fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

impl AppState {
    /// Apply a non-blocking event. Returns `true` when visible state changed.
    pub fn apply_event(&self, event: &HookEvent) -> bool {
        let changed = {
            let mut domain = self.domain.lock().expect("state poisoned");
            domain.board.apply(event, now_ms())
        };
        self.janitor.notify_one();
        changed
    }

    /// Register a blocking event (approval or question).
    pub fn register_blocking(&self, event: &HookEvent, request_id: String) -> Blocking {
        let mut domain = self.domain.lock().expect("state poisoned");
        let now = now_ms();

        match domain.approvals.register(event, request_id.clone(), now) {
            Registered::AutoAllow => {
                domain.board.apply(event, now);
                let key = cortex_core::SessionKey {
                    agent: event.agent.clone(),
                    session_id: event.session_id.clone(),
                };
                domain.board.approval_closed(&key);
                Blocking::Immediate(DecisionResponse::new(Decision::Allow))
            }
            Registered::Skip => Blocking::Immediate(DecisionResponse::new(Decision::Ask)),
            Registered::Card(_) => {
                domain.board.apply(event, now);
                let (tx, rx) = oneshot::channel();
                drop(domain);
                self.responders
                    .lock()
                    .expect("responders poisoned")
                    .insert(request_id, tx);
                self.janitor.notify_one();
                Blocking::Wait(rx)
            }
        }
    }

    /// Resolve a pending approval from the UI. `always` records a rule.
    /// Returns `false` for unknown/stale ids (nothing is sent anywhere).
    pub fn resolve(&self, request_id: &str, ui_decision: &str) -> bool {
        let decision = match ui_decision {
            "allow" | "always" => Decision::Allow,
            "deny" => Decision::Deny,
            "ask" => Decision::Ask,
            _ => return false,
        };
        {
            let mut domain = self.domain.lock().expect("state poisoned");
            let taken = if ui_decision == "always" {
                domain.approvals.take_always(request_id)
            } else {
                domain.approvals.take(request_id)
            };
            let Some(approval) = taken else {
                return false;
            };
            // A question cannot be "allowed" without answers.
            if !approval.questions.is_empty() && decision == Decision::Allow {
                domain.board.approval_closed(&approval.session);
                drop(domain);
                self.send(request_id, DecisionResponse::new(Decision::Ask));
                return true;
            }
            domain.board.approval_closed(&approval.session);
        }
        self.send(request_id, DecisionResponse::new(decision));
        true
    }

    /// Answer a pending question from the UI. Every answer must belong to a
    /// question of that exact card; otherwise nothing happens.
    pub fn answer(&self, request_id: &str, answers: Answers) -> bool {
        {
            let mut domain = self.domain.lock().expect("state poisoned");
            let valid = domain.approvals.pending().iter().any(|p| {
                p.request_id == request_id
                    && !p.questions.is_empty()
                    && answers.len() == p.questions.len()
                    && p.questions.iter().all(|q| {
                        answers
                            .get(&q.question)
                            .is_some_and(|a| is_valid_answer(a, q))
                    })
            });
            if !valid {
                return false;
            }
            let Some(approval) = domain.approvals.take(request_id) else {
                return false;
            };
            domain.board.approval_closed(&approval.session);
        }
        self.send(request_id, DecisionResponse::answered(answers));
        true
    }

    /// The waiting connection died (hook timed out, or the user answered in
    /// the terminal): drop the card. Returns `true` when something was removed.
    pub fn cancel(&self, request_id: &str) -> bool {
        self.responders
            .lock()
            .expect("responders poisoned")
            .remove(request_id);
        let mut domain = self.domain.lock().expect("state poisoned");
        match domain.approvals.take(request_id) {
            Some(approval) => {
                domain.board.approval_closed(&approval.session);
                true
            }
            None => false,
        }
    }

    /// Janitor pass: GC sessions, expire stale approvals (answering `Ask`).
    /// Returns `(visible state changed, board is empty)`.
    pub fn sweep(&self) -> (bool, bool) {
        let (changed, expired, empty) = {
            let mut domain = self.domain.lock().expect("state poisoned");
            let now = now_ms();
            let gc_changed = domain.board.gc(now);
            let expired = domain.approvals.expire(now);
            for approval in &expired {
                domain.board.approval_closed(&approval.session);
            }
            let empty = domain.board.is_empty() && domain.approvals.pending().is_empty();
            (gc_changed || !expired.is_empty(), expired, empty)
        };
        for approval in expired {
            self.send(&approval.request_id, DecisionResponse::new(Decision::Ask));
        }
        (changed, empty)
    }

    pub fn snapshot(&self) -> Snapshot {
        let domain = self.domain.lock().expect("state poisoned");
        Snapshot {
            sessions: domain.board.snapshot().into_iter().cloned().collect(),
            approvals: domain.approvals.pending().into_iter().cloned().collect(),
        }
    }

    /// Hand the response to the parked connection, if it is still there.
    fn send(&self, request_id: &str, response: DecisionResponse) {
        let sender = self
            .responders
            .lock()
            .expect("responders poisoned")
            .remove(request_id);
        if let Some(tx) = sender {
            // The connection may have died; that is fine.
            let _ = tx.send(response);
        }
    }
}

/// An answer is one of the option labels — or, for multi-select, a
/// comma-separated list of them.
fn is_valid_answer(answer: &str, question: &cortex_core::approval::Question) -> bool {
    let is_label = |s: &str| question.options.iter().any(|o| o.label == s);
    if question.multi_select {
        !answer.is_empty() && answer.split(", ").all(is_label)
    } else {
        is_label(answer)
    }
}
