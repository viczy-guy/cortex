//! Versioned, opt-in application preferences.
use crate::github::{is_valid_login, MAX_ACCOUNTS};
use serde_json::Value;
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Prefs {
    pub github: bool,
    pub github_hidden: Vec<String>,
    /// Signed update checks (ADR 0004); `None` until the user answers the first-run question.
    pub updates: Option<bool>,
    /// The island's mascot, one of [`MASCOTS`]; `None` is the default.
    pub mascot: Option<String>,
}

/// The mascot gallery; "none" hides the mascot.
pub const MASCOTS: [&str; 9] = [
    "eyes", "lens", "owl", "cat", "ghost", "robot", "mustache", "penguin", "none",
];
pub const DEFAULT_MASCOT: &str = "eyes";

pub fn is_mascot(name: &str) -> bool {
    MASCOTS.contains(&name)
}
pub fn prefs_from_json(json: &str) -> Prefs {
    checked_prefs_from_json(json).unwrap_or_default()
}
/// Distinguish invalid storage from a valid opt-out for the I/O boundary.
pub fn checked_prefs_from_json(json: &str) -> Option<Prefs> {
    let v: Value = serde_json::from_str(json).ok()?;
    if v["version"].as_u64() != Some(1) {
        return None;
    }
    let github_hidden = match v.get("github_hidden") {
        None => Vec::new(),
        Some(hidden) => {
            let hidden = hidden.as_array()?;
            if hidden.len() > MAX_ACCOUNTS {
                return None;
            }
            hidden
                .iter()
                .map(|login| {
                    login
                        .as_str()
                        .filter(|s| is_valid_login(s))
                        .map(str::to_owned)
                })
                .collect::<Option<Vec<_>>>()?
        }
    };
    let updates = match v.get("updates") {
        None | Some(Value::Null) => None,
        Some(on) => Some(on.as_bool()?),
    };
    // A look, not a permission: an unknown mascot falls back to the default
    // instead of costing the user every other setting.
    let mascot = v
        .get("mascot")
        .and_then(Value::as_str)
        .filter(|m| is_mascot(m))
        .map(str::to_owned);
    Some(Prefs {
        github: v["github"].as_bool()?,
        github_hidden,
        updates,
        mascot,
    })
}

pub fn prefs_to_json(prefs: &Prefs) -> String {
    serde_json::json!({"version":1,"github":prefs.github,"github_hidden":prefs.github_hidden,"updates":prefs.updates,"mascot":prefs.mascot})
        .to_string()
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn preferences_round_trip_and_ignore_unknown_keys() {
        let p = Prefs {
            github: true,
            ..Prefs::default()
        };
        assert_eq!(prefs_from_json(&prefs_to_json(&p)), p);
        assert_eq!(
            prefs_from_json(r#"{"version":1,"github":true,"other":42}"#),
            p
        );
    }
    #[test]
    fn hidden_accounts_round_trip_and_invalid_lists_are_rejected() {
        let p = Prefs {
            github: true,
            github_hidden: vec!["alice".into(), "Bob-2".into()],
            ..Prefs::default()
        };
        assert_eq!(prefs_from_json(&prefs_to_json(&p)), p);
        for hidden in [
            serde_json::json!(null),
            serde_json::json!("alice"),
            serde_json::json!([1]),
            serde_json::json!(["-x"]),
            serde_json::json!(vec!["alice"; MAX_ACCOUNTS + 1]),
        ] {
            let json = serde_json::json!({"version":1,"github":true,"github_hidden":hidden});
            assert_eq!(prefs_from_json(&json.to_string()), Prefs::default());
        }
    }

    #[test]
    fn the_update_answer_round_trips_and_absent_means_not_asked() {
        for updates in [None, Some(false), Some(true)] {
            let p = Prefs {
                updates,
                ..Prefs::default()
            };
            assert_eq!(prefs_from_json(&prefs_to_json(&p)), p);
        }
        assert_eq!(
            prefs_from_json(r#"{"version":1,"github":false}"#).updates,
            None
        );
    }

    #[test]
    fn the_mascot_round_trips_and_an_unknown_one_keeps_the_rest() {
        let p = Prefs {
            github: true,
            mascot: Some("owl".into()),
            ..Prefs::default()
        };
        assert_eq!(prefs_from_json(&prefs_to_json(&p)), p);
        let unknown = prefs_from_json(r#"{"version":1,"github":true,"mascot":"dragon"}"#);
        assert!(unknown.github);
        assert_eq!(unknown.mascot, None);
        assert!(MASCOTS.contains(&DEFAULT_MASCOT));
    }

    #[test]
    fn invalid_preferences_default_to_off() {
        for s in [
            "invalid",
            "{}",
            r#"{"version":2,"github":true}"#,
            r#"{"version":1,"github":"true"}"#,
            r#"{"version":1,"github":false,"updates":"yes"}"#,
        ] {
            assert_eq!(prefs_from_json(s), Prefs::default());
        }
    }
}
