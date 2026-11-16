//! Cortex domain logic.
//!
//! Pure Rust: no I/O, no Tauri, no platform code. Everything here is driven by
//! [`event::HookEvent`] values and plain timestamps, which keeps it fully
//! unit-testable.

pub mod approval;
pub mod diff;
pub mod event;
pub mod protocol;
pub mod session;

pub use approval::{AlwaysRule, ApprovalQueue, PendingApproval};
pub use event::{AgentId, EventKind, HookEvent};
pub use protocol::Decision;
pub use session::{Session, SessionBoard, SessionKey, SessionState};
