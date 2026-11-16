//! Application state: the session board, the approval queue and the map of
//! connections waiting for a decision.

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use cortex_core::{ApprovalQueue, Decision, HookEvent, PendingApproval, SessionBoard};
use serde::Serialize;
use tokio::sync::{oneshot, Notify};

/// Serializable snapshot sent to the island on every change.
#[derive(Debug, Clone, Serialize)]
pub struct Snapshot {
    pub sessions: Vec<cortex_core::Session>,
    pub approvals: Vec<PendingApproval>,
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
    responders: Mutex<HashMap<String, oneshot::Sender<Decision>>>,
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

    /// Register a blocking event. `Some(decision)` means answer immediately
    /// (an Always rule matched); `None` means a card is up and the connection
    /// should await its oneshot.
    pub fn register_blocking(
        &self,
        event: &HookEvent,
        request_id: String,
    ) -> (Option<Decision>, Option<oneshot::Receiver<Decision>>) {
        let mut domain = self.domain.lock().expect("state poisoned");
        let now = now_ms();
        domain.board.apply(event, now);

        match domain.approvals.register(event, request_id.clone(), now) {
            None => (Some(Decision::Allow), None),
            Some(_) => {
                let (tx, rx) = oneshot::channel();
                drop(domain);
                self.responders
                    .lock()
                    .expect("responders poisoned")
                    .insert(request_id, tx);
                self.janitor.notify_one();
                (None, Some(rx))
            }
        }
    }

    /// Resolve a pending request from the UI. `always` records a rule.
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
            domain.board.approval_closed(&approval.session);
        }
        self.send_decision(request_id, decision);
        true
    }

    /// The waiting connection died (hook timed out, or the user answered in
    /// the terminal): drop the card. Returns `true` when something was removed.
    pub fn cancel(&self, request_id: &str) -> bool {
        self.forget_responder(request_id);
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
            self.send_decision(&approval.request_id, Decision::Ask);
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

    /// Hand the decision to the parked connection, if it is still there.
    fn send_decision(&self, request_id: &str, decision: Decision) {
        let sender = self
            .responders
            .lock()
            .expect("responders poisoned")
            .remove(request_id);
        if let Some(tx) = sender {
            // The connection may have died; that is fine.
            let _ = tx.send(decision);
        }
    }

    fn forget_responder(&self, request_id: &str) {
        self.responders
            .lock()
            .expect("responders poisoned")
            .remove(request_id);
    }
}
