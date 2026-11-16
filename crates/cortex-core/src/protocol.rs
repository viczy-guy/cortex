//! The tiny wire protocol between the app and `cortex-hook` for blocking
//! events: a single JSON line `{"decision": "..."}` sent back on the same
//! connection.

use serde::{Deserialize, Serialize};

/// A user decision on a blocking event.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Decision {
    /// Approve this request.
    Allow,
    /// Refuse this request.
    Deny,
    /// No opinion — the agent asks in its own terminal.
    Ask,
}

/// Response line for a blocking event.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecisionResponse {
    pub decision: Decision,
    /// Optional human-readable reason, shown by some agents.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

impl DecisionResponse {
    pub fn new(decision: Decision) -> Self {
        Self {
            decision,
            reason: None,
        }
    }

    pub fn to_line(&self) -> String {
        // Serialization of this struct cannot fail.
        let mut s = serde_json::to_string(self).expect("DecisionResponse is always serializable");
        s.push('\n');
        s
    }

    pub fn parse(line: &str) -> Option<Self> {
        serde_json::from_str(line.trim()).ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips() {
        for d in [Decision::Allow, Decision::Deny, Decision::Ask] {
            let line = DecisionResponse::new(d).to_line();
            assert!(line.ends_with('\n'));
            assert_eq!(DecisionResponse::parse(&line).unwrap().decision, d);
        }
    }

    #[test]
    fn wire_format_is_stable() {
        assert_eq!(
            DecisionResponse::new(Decision::Allow).to_line(),
            "{\"decision\":\"allow\"}\n"
        );
    }
}
