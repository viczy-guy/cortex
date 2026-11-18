//! Versioned, opt-in application preferences.
use crate::github::{is_valid_login, MAX_ACCOUNTS};
use serde_json::Value;
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Prefs {
    pub github: bool,
    pub github_hidden: Vec<String>,
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
    Some(Prefs {
        github: v["github"].as_bool()?,
        github_hidden,
    })
}

pub fn prefs_to_json(prefs: &Prefs) -> String {
    serde_json::json!({"version":1,"github":prefs.github,"github_hidden":prefs.github_hidden})
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
    fn invalid_preferences_default_to_off() {
        for s in [
            "invalid",
            "{}",
            r#"{"version":2,"github":true}"#,
            r#"{"version":1,"github":"true"}"#,
        ] {
            assert_eq!(prefs_from_json(s), Prefs::default());
        }
    }
}
