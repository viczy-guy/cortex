//! The approval queue: pending permission requests and the rules created by
//! "Always allow".
//!
//! Safety property (see SECURITY.md): a decision resolves **only** the exact
//! request id it was made for. Late, duplicate or unknown ids resolve nothing.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::event::HookEvent;
use crate::protocol::Decision;
use crate::session::SessionKey;

/// A pending request expires after this long and resolves to `Ask`.
pub const PENDING_TTL_MS: u64 = 55 * 1000;

/// What the user sees on an approval card.
#[derive(Debug, Clone, Serialize)]
pub struct PendingApproval {
    pub request_id: String,
    pub session: SessionKey,
    pub tool_name: String,
    /// Human-readable detail: the command, the file path, …
    pub detail: String,
    pub created_ms: u64,
}

/// An "Always allow" rule, stored locally by Cortex.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AlwaysRule {
    pub tool_name: String,
    /// Exact detail for non-command tools; first word of the command for Bash.
    pub pattern: String,
}

#[derive(Debug, Default)]
pub struct ApprovalQueue {
    pending: HashMap<String, PendingApproval>,
    rules: Vec<AlwaysRule>,
}

impl ApprovalQueue {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_rules(rules: Vec<AlwaysRule>) -> Self {
        Self {
            pending: HashMap::new(),
            rules,
        }
    }

    /// Register a blocking event. Returns the card to display, or `None` when
    /// an Always rule already answers it (the caller replies `Allow` directly).
    pub fn register(
        &mut self,
        ev: &HookEvent,
        request_id: String,
        now_ms: u64,
    ) -> Option<PendingApproval> {
        let tool_name = ev.tool_name.clone().unwrap_or_else(|| "tool".into());
        let detail = detail_of(ev);

        if self.matches_rule(&tool_name, &detail) {
            return None;
        }

        let approval = PendingApproval {
            request_id: request_id.clone(),
            session: SessionKey {
                agent: ev.agent.clone(),
                session_id: ev.session_id.clone(),
            },
            tool_name,
            detail,
            created_ms: now_ms,
        };
        self.pending.insert(request_id, approval.clone());
        Some(approval)
    }

    /// Resolve a request by its exact id. Unknown or already-resolved ids
    /// return `None` — the caller must then answer nothing.
    pub fn resolve(&mut self, request_id: &str, decision: Decision) -> Option<Decision> {
        let approval = self.pending.remove(request_id)?;
        if decision == Decision::Allow {
            // No rule is created here; "Always" goes through `add_rule`.
        }
        let _ = approval;
        Some(decision)
    }

    /// "Always allow": resolve this request AND remember a rule for next time.
    pub fn resolve_always(&mut self, request_id: &str) -> Option<Decision> {
        let approval = self.pending.remove(request_id)?;
        self.rules
            .push(rule_for(&approval.tool_name, &approval.detail));
        Some(Decision::Allow)
    }

    /// Expire pending requests older than [`PENDING_TTL_MS`]. Returns the ids
    /// that expired (the caller answers `Ask` for each).
    pub fn expire(&mut self, now_ms: u64) -> Vec<String> {
        let expired: Vec<String> = self
            .pending
            .iter()
            .filter(|(_, p)| now_ms.saturating_sub(p.created_ms) >= PENDING_TTL_MS)
            .map(|(id, _)| id.clone())
            .collect();
        for id in &expired {
            self.pending.remove(id);
        }
        expired
    }

    pub fn pending(&self) -> Vec<&PendingApproval> {
        let mut all: Vec<&PendingApproval> = self.pending.values().collect();
        all.sort_by_key(|p| p.created_ms);
        all
    }

    pub fn rules(&self) -> &[AlwaysRule] {
        &self.rules
    }

    fn matches_rule(&self, tool_name: &str, detail: &str) -> bool {
        let candidate = rule_for(tool_name, detail);
        self.rules.contains(&candidate)
    }
}

/// Human-readable detail for the card.
fn detail_of(ev: &HookEvent) -> String {
    let input = ev.tool_input.as_ref();
    let get = |key: &str| input.and_then(|i| i.get(key)).and_then(Value::as_str);
    get("command")
        .or_else(|| get("file_path"))
        .or_else(|| get("url"))
        .map(str::to_owned)
        .unwrap_or_default()
}

/// The rule a detail generalizes to. Conservative on purpose: for commands only
/// the first word (the program) is kept; everything else must match exactly.
fn rule_for(tool_name: &str, detail: &str) -> AlwaysRule {
    let pattern = if tool_name == "Bash" {
        detail
            .split_whitespace()
            .next()
            .unwrap_or_default()
            .to_owned()
    } else {
        detail.to_owned()
    };
    AlwaysRule {
        tool_name: tool_name.to_owned(),
        pattern,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event::HookEvent;

    fn bash_event(cmd: &str) -> HookEvent {
        HookEvent::parse(&format!(
            r#"{{"hook_event_name":"PermissionRequest","session_id":"s1","tool_name":"Bash","tool_input":{{"command":{}}}}}"#,
            serde_json::to_string(cmd).unwrap()
        ))
        .unwrap()
    }

    #[test]
    fn decision_is_bound_to_the_exact_id() {
        let mut q = ApprovalQueue::new();
        q.register(&bash_event("ls"), "req-1".into(), 0).unwrap();

        assert_eq!(q.resolve("wrong-id", Decision::Allow), None);
        assert_eq!(q.resolve("req-1", Decision::Allow), Some(Decision::Allow));
        // A duplicate decision resolves nothing.
        assert_eq!(q.resolve("req-1", Decision::Allow), None);
    }

    #[test]
    fn always_creates_a_rule_that_auto_allows_next_time() {
        let mut q = ApprovalQueue::new();
        q.register(&bash_event("cargo test"), "r1".into(), 0)
            .unwrap();
        assert_eq!(q.resolve_always("r1"), Some(Decision::Allow));
        assert_eq!(
            q.rules(),
            &[AlwaysRule {
                tool_name: "Bash".into(),
                pattern: "cargo".into()
            }]
        );

        // Same program, different args: auto-allowed (register returns None).
        assert!(q
            .register(&bash_event("cargo build"), "r2".into(), 1)
            .is_none());
        // Different program: still asks.
        assert!(q
            .register(&bash_event("rm -rf x"), "r3".into(), 2)
            .is_some());
    }

    #[test]
    fn pending_requests_expire_to_ask() {
        let mut q = ApprovalQueue::new();
        q.register(&bash_event("ls"), "r1".into(), 0).unwrap();
        assert!(q.expire(PENDING_TTL_MS - 1).is_empty());
        assert_eq!(q.expire(PENDING_TTL_MS), vec!["r1".to_string()]);
        // Expired means gone: a late click resolves nothing.
        assert_eq!(q.resolve("r1", Decision::Allow), None);
    }
}
