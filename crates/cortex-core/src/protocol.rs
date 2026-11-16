//! The tiny wire protocol between the app and `cortex-hook` for blocking
//! events: a single JSON line `{"decision": "..."}` sent back on the same
//! connection.

use std::collections::BTreeMap;

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

/// Answers to a question card: question text → chosen answer.
pub type Answers = BTreeMap<String, String>;

/// Response line for a blocking event.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DecisionResponse {
    pub decision: Decision,
    /// Present only when answering a question (`decision` is then `allow`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub answers: Option<Answers>,
}

impl DecisionResponse {
    pub fn new(decision: Decision) -> Self {
        Self {
            decision,
            answers: None,
        }
    }

    pub fn answered(answers: Answers) -> Self {
        Self {
            decision: Decision::Allow,
            answers: Some(answers),
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
    fn answers_travel_with_an_allow() {
        let mut answers = Answers::new();
        answers.insert("Which framework?".into(), "React".into());
        let line = DecisionResponse::answered(answers.clone()).to_line();
        assert_eq!(
            line,
            "{\"decision\":\"allow\",\"answers\":{\"Which framework?\":\"React\"}}\n"
        );
        assert_eq!(
            DecisionResponse::parse(&line).unwrap().answers,
            Some(answers)
        );
    }

    #[test]
    fn wire_format_is_stable() {
        assert_eq!(
            DecisionResponse::new(Decision::Allow).to_line(),
            "{\"decision\":\"allow\"}\n"
        );
    }
}
