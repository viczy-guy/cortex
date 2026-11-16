//! Hook events: the single untrusted input of the whole system.
//!
//! Events arrive as one JSON object per line from the socket. Parsing is
//! deliberately lenient about unknown fields and strict about the few we rely
//! on: a malformed event is dropped by the caller, never panics, and unknown
//! event names are preserved as [`EventKind::Other`] so a newer agent cannot
//! crash an older Cortex.

use serde::Serialize;
use serde_json::Value;

/// Default agent for events that don't carry a valid `cortex_agent`.
pub const DEFAULT_AGENT: &str = "claude-code";

/// A validated agent identifier: `^[a-z0-9-]{1,24}$`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
pub struct AgentId(String);

impl AgentId {
    pub fn parse(raw: &str) -> Option<Self> {
        let ok = !raw.is_empty()
            && raw.len() <= 24
            && raw
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-');
        ok.then(|| Self(raw.to_owned()))
    }

    pub fn default_agent() -> Self {
        Self(DEFAULT_AGENT.to_owned())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Canonical event vocabulary (Claude Code hook event names).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub enum EventKind {
    SessionStart,
    UserPromptSubmit,
    PreToolUse,
    PostToolUse,
    PostToolUseFailure,
    Notification,
    PermissionRequest,
    Stop,
    StopFailure,
    SubagentStart,
    SubagentStop,
    SessionEnd,
    /// Unknown name, kept verbatim. Never an error.
    Other(String),
}

impl EventKind {
    fn from_name(name: &str) -> Self {
        match name {
            "SessionStart" => Self::SessionStart,
            "UserPromptSubmit" => Self::UserPromptSubmit,
            "PreToolUse" => Self::PreToolUse,
            "PostToolUse" => Self::PostToolUse,
            "PostToolUseFailure" => Self::PostToolUseFailure,
            "Notification" => Self::Notification,
            "PermissionRequest" => Self::PermissionRequest,
            "Stop" => Self::Stop,
            "StopFailure" => Self::StopFailure,
            "SubagentStart" => Self::SubagentStart,
            "SubagentStop" => Self::SubagentStop,
            "SessionEnd" => Self::SessionEnd,
            other => Self::Other(other.to_owned()),
        }
    }

    /// Events where the sender may wait for a user decision on the same
    /// connection. Must stay in sync with `cortex-hook`.
    pub fn is_blocking(&self) -> bool {
        matches!(self, Self::PermissionRequest)
    }
}

/// A parsed, validated hook event.
#[derive(Debug, Clone, Serialize)]
pub struct HookEvent {
    pub kind: EventKind,
    pub agent: AgentId,
    pub session_id: String,
    /// Correlates a blocking event with its decision.
    pub request_id: Option<String>,
    pub tool_name: Option<String>,
    pub tool_input: Option<Value>,
    pub prompt: Option<String>,
    pub message: Option<String>,
    /// Current working directory of the session, when the agent sends one.
    pub cwd: Option<String>,
}

/// Why an event line was rejected.
#[derive(Debug, PartialEq, Eq)]
pub enum ParseError {
    NotJson,
    NotAnObject,
    MissingEventName,
    MissingSessionId,
    TooLarge,
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            Self::NotJson => "line is not valid JSON",
            Self::NotAnObject => "event is not a JSON object",
            Self::MissingEventName => "missing hook_event_name",
            Self::MissingSessionId => "missing session_id",
            Self::TooLarge => "event exceeds size limit",
        };
        f.write_str(s)
    }
}

/// Hard cap on a single event line. Anything larger is hostile or broken.
pub const MAX_EVENT_BYTES: usize = 1_000_000;

impl HookEvent {
    /// Parse one event line. Lenient on extras, strict on essentials.
    pub fn parse(line: &str) -> Result<Self, ParseError> {
        if line.len() > MAX_EVENT_BYTES {
            return Err(ParseError::TooLarge);
        }
        let value: Value = serde_json::from_str(line).map_err(|_| ParseError::NotJson)?;
        let obj = value.as_object().ok_or(ParseError::NotAnObject)?;

        let name = obj
            .get("hook_event_name")
            .and_then(Value::as_str)
            .ok_or(ParseError::MissingEventName)?;
        let session_id = obj
            .get("session_id")
            .and_then(Value::as_str)
            .filter(|s| !s.is_empty())
            .ok_or(ParseError::MissingSessionId)?
            .to_owned();

        let agent = obj
            .get("cortex_agent")
            .and_then(Value::as_str)
            .and_then(AgentId::parse)
            .unwrap_or_else(AgentId::default_agent);

        let get_str = |key: &str| obj.get(key).and_then(Value::as_str).map(str::to_owned);

        Ok(Self {
            kind: EventKind::from_name(name),
            agent,
            session_id,
            request_id: get_str("cortex_request_id"),
            tool_name: get_str("tool_name"),
            tool_input: obj.get("tool_input").cloned(),
            prompt: get_str("prompt"),
            message: get_str("message"),
            cwd: get_str("cwd"),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_a_minimal_event() {
        let ev = HookEvent::parse(r#"{"hook_event_name":"Stop","session_id":"s1"}"#).unwrap();
        assert_eq!(ev.kind, EventKind::Stop);
        assert_eq!(ev.session_id, "s1");
        assert_eq!(ev.agent.as_str(), DEFAULT_AGENT);
    }

    #[test]
    fn keeps_unknown_event_names() {
        let ev = HookEvent::parse(r#"{"hook_event_name":"FutureThing","session_id":"s"}"#).unwrap();
        assert_eq!(ev.kind, EventKind::Other("FutureThing".into()));
    }

    #[test]
    fn rejects_garbage() {
        assert_eq!(
            HookEvent::parse("not json").unwrap_err(),
            ParseError::NotJson
        );
        assert_eq!(
            HookEvent::parse("[1,2]").unwrap_err(),
            ParseError::NotAnObject
        );
        assert_eq!(
            HookEvent::parse(r#"{"session_id":"s"}"#).unwrap_err(),
            ParseError::MissingEventName
        );
        assert_eq!(
            HookEvent::parse(r#"{"hook_event_name":"Stop"}"#).unwrap_err(),
            ParseError::MissingSessionId
        );
    }

    #[test]
    fn invalid_agent_name_falls_back_to_default() {
        for bad in ["UPPER", "has space", "wa\u{30c4}y", "", &"x".repeat(25)] {
            let line = format!(
                r#"{{"hook_event_name":"Stop","session_id":"s","cortex_agent":{}}}"#,
                serde_json::to_string(bad).unwrap()
            );
            let ev = HookEvent::parse(&line).unwrap();
            assert_eq!(
                ev.agent.as_str(),
                DEFAULT_AGENT,
                "agent {bad:?} should be rejected"
            );
        }
    }

    #[test]
    fn valid_agent_name_is_kept() {
        let ev = HookEvent::parse(
            r#"{"hook_event_name":"Stop","session_id":"s","cortex_agent":"my-tool-2"}"#,
        )
        .unwrap();
        assert_eq!(ev.agent.as_str(), "my-tool-2");
    }

    #[test]
    fn oversized_line_is_rejected() {
        let line = format!(
            r#"{{"hook_event_name":"Stop","session_id":"s","pad":"{}"}}"#,
            "a".repeat(MAX_EVENT_BYTES)
        );
        assert_eq!(HookEvent::parse(&line).unwrap_err(), ParseError::TooLarge);
    }

    #[test]
    fn only_permission_request_blocks() {
        assert!(EventKind::PermissionRequest.is_blocking());
        assert!(!EventKind::PreToolUse.is_blocking());
        assert!(!EventKind::Stop.is_blocking());
    }
}
