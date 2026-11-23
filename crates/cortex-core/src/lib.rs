//! Cortex domain logic.
//!
//! Pure Rust: no I/O, no Tauri, no platform code. Everything here is driven by
//! [`event::HookEvent`] values and plain timestamps, which keeps it fully
//! unit-testable.

pub mod approval;
pub mod autostart;
pub mod diff;
pub mod event;
pub mod github;
pub mod hooks;
pub mod placement;
pub mod prefs;
pub mod protocol;
pub mod session;

pub use approval::{
    rules_from_json, rules_to_json, AlwaysRule, ApprovalQueue, PendingApproval, Registered,
    MAX_RULES,
};
pub use event::{AgentId, EventKind, HookEvent};
pub use github::{GithubSummary, PullRequest};
pub use protocol::{Answers, Decision, DecisionResponse};
pub use session::{Session, SessionBoard, SessionKey, SessionState};
