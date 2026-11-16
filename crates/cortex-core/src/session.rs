//! The session board: one entry per (agent, session), driven by hook events.
//!
//! All methods take `now_ms` (Unix millis) instead of reading a clock, so every
//! transition is deterministic under test.

use std::collections::{HashMap, VecDeque};

use serde::Serialize;
use serde_json::Value;

use crate::diff::{self, DiffStat};
use crate::event::{AgentId, EventKind, HookEvent};

/// Sessions with no events for this long are garbage-collected.
pub const STALE_AFTER_MS: u64 = 30 * 60 * 1000;
/// How long a finished session stays visible.
pub const FINISHED_LINGER_MS: u64 = 5 * 1000;
/// Ticker history kept per session.
const TICKER_CAP: usize = 50;
/// Max distinct changed files tracked per session.
const CHANGES_CAP: usize = 200;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
pub struct SessionKey {
    pub agent: AgentId,
    pub session_id: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionState {
    Idle,
    Thinking,
    Working,
    WaitingApproval,
    WaitingInput,
    Finished,
    Error,
}

/// One line of the ticker (what the session is doing / did).
#[derive(Debug, Clone, Serialize)]
pub struct Step {
    pub at_ms: u64,
    pub label: String,
}

/// A changed file with cumulative +/− counts.
#[derive(Debug, Clone, Serialize)]
pub struct FileChange {
    pub path: String,
    pub added: usize,
    pub removed: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct Session {
    pub key: SessionKey,
    pub state: SessionState,
    pub prompt: Option<String>,
    pub current: Option<String>,
    pub ticker: VecDeque<Step>,
    pub changes: Vec<FileChange>,
    pub cwd: Option<String>,
    pub started_at_ms: u64,
    pub last_event_ms: u64,
}

impl Session {
    fn new(key: SessionKey, now_ms: u64) -> Self {
        Self {
            key,
            state: SessionState::Idle,
            prompt: None,
            current: None,
            ticker: VecDeque::new(),
            changes: Vec::new(),
            cwd: None,
            started_at_ms: now_ms,
            last_event_ms: now_ms,
        }
    }

    fn push_step(&mut self, now_ms: u64, label: impl Into<String>) {
        if self.ticker.len() == TICKER_CAP {
            self.ticker.pop_front();
        }
        self.ticker.push_back(Step {
            at_ms: now_ms,
            label: label.into(),
        });
    }

    fn record_change(&mut self, path: String, stat: DiffStat) {
        if let Some(c) = self.changes.iter_mut().find(|c| c.path == path) {
            c.added += stat.added;
            c.removed += stat.removed;
        } else if self.changes.len() < CHANGES_CAP {
            self.changes.push(FileChange {
                path,
                added: stat.added,
                removed: stat.removed,
            });
        }
    }
}

/// All live sessions.
#[derive(Debug, Default)]
pub struct SessionBoard {
    sessions: HashMap<SessionKey, Session>,
}

impl SessionBoard {
    pub fn new() -> Self {
        Self::default()
    }

    /// Apply one event. Returns `true` when visible state changed.
    pub fn apply(&mut self, ev: &HookEvent, now_ms: u64) -> bool {
        let key = SessionKey {
            agent: ev.agent.clone(),
            session_id: ev.session_id.clone(),
        };

        if ev.kind == EventKind::SessionEnd {
            return self.sessions.remove(&key).is_some();
        }

        let session = self
            .sessions
            .entry(key.clone())
            .or_insert_with(|| Session::new(key, now_ms));
        session.last_event_ms = now_ms;
        if let Some(cwd) = &ev.cwd {
            session.cwd = Some(cwd.clone());
        }

        match &ev.kind {
            EventKind::SessionStart => {
                session.state = SessionState::Idle;
            }
            EventKind::UserPromptSubmit => {
                session.state = SessionState::Thinking;
                session.prompt = ev.prompt.clone();
                if let Some(p) = &ev.prompt {
                    session.push_step(now_ms, truncate(p, 120));
                }
            }
            EventKind::PreToolUse => {
                session.state = SessionState::Working;
                let label = tool_label(ev);
                session.current = Some(label.clone());
                session.push_step(now_ms, label);
            }
            EventKind::PostToolUse | EventKind::PostToolUseFailure => {
                session.state = SessionState::Working;
                if let Some((path, stat)) = edit_stat(ev) {
                    session.record_change(path, stat);
                }
            }
            EventKind::PermissionRequest => {
                session.state = SessionState::WaitingApproval;
                session.push_step(now_ms, format!("needs approval: {}", tool_label(ev)));
            }
            EventKind::Notification => {
                session.state = SessionState::WaitingInput;
                if let Some(m) = &ev.message {
                    session.push_step(now_ms, truncate(m, 120));
                }
            }
            EventKind::Stop => {
                session.state = SessionState::Finished;
                session.current = None;
            }
            EventKind::StopFailure => {
                session.state = SessionState::Error;
            }
            EventKind::SubagentStart => session.push_step(now_ms, "subagent started"),
            EventKind::SubagentStop => session.push_step(now_ms, "subagent finished"),
            EventKind::SessionEnd => unreachable!("handled above"),
            EventKind::Other(name) => {
                // Unknown events only bump liveness; they never change state.
                let _ = name;
                return false;
            }
        }
        true
    }

    /// An approval for this session was answered, cancelled or expired: the
    /// session is no longer waiting on the user. Returns `true` when state
    /// changed.
    pub fn approval_closed(&mut self, key: &SessionKey) -> bool {
        match self.sessions.get_mut(key) {
            Some(s) if s.state == SessionState::WaitingApproval => {
                s.state = SessionState::Working;
                true
            }
            _ => false,
        }
    }

    /// Drop stale sessions and expired `Finished` ones. Returns `true` when
    /// something was removed.
    pub fn gc(&mut self, now_ms: u64) -> bool {
        let before = self.sessions.len();
        self.sessions.retain(|_, s| {
            let age = now_ms.saturating_sub(s.last_event_ms);
            match s.state {
                SessionState::Finished => age < FINISHED_LINGER_MS,
                _ => age < STALE_AFTER_MS,
            }
        });
        self.sessions.len() != before
    }

    /// Stable snapshot for the UI, most recently active first.
    pub fn snapshot(&self) -> Vec<&Session> {
        let mut all: Vec<&Session> = self.sessions.values().collect();
        all.sort_by_key(|a| std::cmp::Reverse(a.last_event_ms));
        all
    }

    pub fn len(&self) -> usize {
        self.sessions.len()
    }

    pub fn is_empty(&self) -> bool {
        self.sessions.is_empty()
    }
}

/// Human label for a tool event, safe to show in the ticker.
fn tool_label(ev: &HookEvent) -> String {
    let name = ev.tool_name.as_deref().unwrap_or("tool");
    match (name, &ev.tool_input) {
        ("Bash", Some(input)) => match input.get("command").and_then(Value::as_str) {
            Some(cmd) => format!("$ {}", truncate(cmd, 80)),
            None => "$ …".into(),
        },
        ("Read" | "Write" | "Edit", Some(input)) => {
            match input.get("file_path").and_then(Value::as_str) {
                Some(p) => format!("{name} {}", file_name(p)),
                None => name.to_owned(),
            }
        }
        _ => name.to_owned(),
    }
}

/// Extract (path, diff stat) from an Edit/Write PostToolUse.
fn edit_stat(ev: &HookEvent) -> Option<(String, DiffStat)> {
    let input = ev.tool_input.as_ref()?;
    let path = input.get("file_path")?.as_str()?.to_owned();
    match ev.tool_name.as_deref() {
        Some("Edit") => {
            let old = input
                .get("old_string")
                .and_then(Value::as_str)
                .unwrap_or("");
            let new = input
                .get("new_string")
                .and_then(Value::as_str)
                .unwrap_or("");
            Some((path, diff::stat(old, new)))
        }
        Some("Write") => {
            let new = input.get("content").and_then(Value::as_str).unwrap_or("");
            Some((path, diff::stat("", new)))
        }
        _ => None,
    }
}

fn file_name(path: &str) -> &str {
    path.rsplit(['/', '\\']).next().unwrap_or(path)
}

fn truncate(s: &str, max_chars: usize) -> String {
    let mut out: String = s.chars().take(max_chars).collect();
    if s.chars().count() > max_chars {
        out.push('…');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ev(json: &str) -> HookEvent {
        HookEvent::parse(json).unwrap()
    }

    #[test]
    fn lifecycle_start_prompt_tool_stop() {
        let mut board = SessionBoard::new();
        board.apply(
            &ev(r#"{"hook_event_name":"SessionStart","session_id":"s1"}"#),
            1,
        );
        board.apply(
            &ev(r#"{"hook_event_name":"UserPromptSubmit","session_id":"s1","prompt":"fix tests"}"#),
            2,
        );
        assert_eq!(board.snapshot()[0].state, SessionState::Thinking);

        board.apply(
            &ev(r#"{"hook_event_name":"PreToolUse","session_id":"s1","tool_name":"Bash","tool_input":{"command":"cargo test"}}"#),
            3,
        );
        let s = board.snapshot()[0];
        assert_eq!(s.state, SessionState::Working);
        assert_eq!(s.current.as_deref(), Some("$ cargo test"));

        board.apply(&ev(r#"{"hook_event_name":"Stop","session_id":"s1"}"#), 4);
        assert_eq!(board.snapshot()[0].state, SessionState::Finished);
    }

    #[test]
    fn session_end_removes_the_session() {
        let mut board = SessionBoard::new();
        board.apply(
            &ev(r#"{"hook_event_name":"SessionStart","session_id":"s1"}"#),
            1,
        );
        board.apply(
            &ev(r#"{"hook_event_name":"SessionEnd","session_id":"s1"}"#),
            2,
        );
        assert!(board.is_empty());
    }

    #[test]
    fn edits_accumulate_diff_stats_per_file() {
        let mut board = SessionBoard::new();
        let edit = r#"{"hook_event_name":"PostToolUse","session_id":"s1","tool_name":"Edit",
            "tool_input":{"file_path":"/p/a.rs","old_string":"a\nb","new_string":"a\nc\nd"}}"#;
        board.apply(&ev(edit), 1);
        board.apply(&ev(edit), 2);
        let s = board.snapshot()[0];
        assert_eq!(s.changes.len(), 1);
        assert_eq!(s.changes[0].added, 4); // 2 per edit
        assert_eq!(s.changes[0].removed, 2); // 1 per edit
    }

    #[test]
    fn agents_do_not_share_sessions() {
        let mut board = SessionBoard::new();
        board.apply(
            &ev(r#"{"hook_event_name":"SessionStart","session_id":"s1"}"#),
            1,
        );
        board.apply(
            &ev(r#"{"hook_event_name":"SessionStart","session_id":"s1","cortex_agent":"other"}"#),
            2,
        );
        assert_eq!(board.len(), 2);
    }

    #[test]
    fn gc_drops_stale_and_finished() {
        let mut board = SessionBoard::new();
        board.apply(
            &ev(r#"{"hook_event_name":"SessionStart","session_id":"old"}"#),
            0,
        );
        board.apply(
            &ev(r#"{"hook_event_name":"Stop","session_id":"done"}"#),
            1000,
        );
        assert_eq!(board.len(), 2);

        board.gc(1000 + FINISHED_LINGER_MS); // finished linger expired
        assert_eq!(board.len(), 1);
        board.gc(STALE_AFTER_MS); // stale expired
        assert!(board.is_empty());
    }

    #[test]
    fn unknown_events_do_not_change_state() {
        let mut board = SessionBoard::new();
        board.apply(
            &ev(r#"{"hook_event_name":"SessionStart","session_id":"s1"}"#),
            1,
        );
        let changed = board.apply(&ev(r#"{"hook_event_name":"Mystery","session_id":"s1"}"#), 2);
        assert!(!changed);
        assert_eq!(board.snapshot()[0].state, SessionState::Idle);
    }

    #[test]
    fn closing_an_approval_resumes_the_session() {
        let mut board = SessionBoard::new();
        board.apply(
            &ev(r#"{"hook_event_name":"PermissionRequest","session_id":"s1","tool_name":"Bash"}"#),
            1,
        );
        let key = board.snapshot()[0].key.clone();
        assert!(board.approval_closed(&key));
        assert_eq!(board.snapshot()[0].state, SessionState::Working);
        // Idempotent: nothing left to close.
        assert!(!board.approval_closed(&key));
    }

    #[test]
    fn permission_request_marks_waiting() {
        let mut board = SessionBoard::new();
        board.apply(
            &ev(r#"{"hook_event_name":"PermissionRequest","session_id":"s1","tool_name":"Bash","tool_input":{"command":"rm -rf /tmp/x"}}"#),
            1,
        );
        assert_eq!(board.snapshot()[0].state, SessionState::WaitingApproval);
    }
}
