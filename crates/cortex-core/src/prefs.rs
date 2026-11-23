//! Versioned, opt-in application preferences.
use crate::github::{is_valid_login, MAX_ACCOUNTS};
use crate::placement::is_position;
use serde_json::Value;
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Prefs {
    pub github: bool,
    pub github_hidden: Vec<String>,
    /// Signed update checks (ADR 0004); `None` until the user answers the first-run question.
    pub updates: Option<bool>,
    /// The island's mascot, one of [`MASCOTS`]; `None` is the default.
    pub mascot: Option<String>,
    /// Island notice sounds; `None` means on.
    pub sound: Option<bool>,
    /// Launch at login (#81). `None` until the first launch turns it on.
    pub launch_at_login: Option<bool>,
    pub island_position: Option<String>,
    pub island_screen: Option<String>,
    /// The version of the last launch: another one now means an update (#127).
    pub last_version: Option<String>,
}

/// The mascot gallery; "none" hides the mascot.
pub const MASCOTS: [&str; 9] = [
    "eyes", "lens", "owl", "cat", "ghost", "robot", "mustache", "penguin", "none",
];
pub const DEFAULT_MASCOT: &str = "eyes";

pub fn is_mascot(name: &str) -> bool {
    MASCOTS.contains(&name)
}
pub fn is_screen_name(name: &str) -> bool {
    name.len() <= 128 * 4 && name.chars().count() <= 128 && !name.chars().any(char::is_control)
}

/// "0.3.1", "1.0.0-beta.2": what an app version looks like, nothing longer.
pub fn is_version(text: &str) -> bool {
    (1..=32).contains(&text.len())
        && text
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '+'))
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
        sound: v.get("sound").and_then(Value::as_bool),
        launch_at_login: v.get("launch_at_login").and_then(Value::as_bool),
        island_position: v["island_position"]
            .as_str()
            .filter(|s| is_position(s))
            .map(str::to_owned),
        island_screen: v["island_screen"]
            .as_str()
            .filter(|s| is_screen_name(s))
            .map(str::to_owned),
        last_version: v["last_version"]
            .as_str()
            .filter(|s| is_version(s))
            .map(str::to_owned),
    })
}

pub fn prefs_to_json(prefs: &Prefs) -> String {
    serde_json::json!({"version":1,"github":prefs.github,"github_hidden":prefs.github_hidden,"updates":prefs.updates,"mascot":prefs.mascot,"sound":prefs.sound,"launch_at_login":prefs.launch_at_login,"island_position":prefs.island_position,"island_screen":prefs.island_screen,"last_version":prefs.last_version})
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
    fn launch_at_login_round_trips_and_absent_means_never_applied() {
        for launch_at_login in [None, Some(false), Some(true)] {
            let p = Prefs {
                launch_at_login,
                ..Prefs::default()
            };
            assert_eq!(prefs_from_json(&prefs_to_json(&p)), p);
        }
        let old = prefs_from_json(r#"{"version":1,"github":true}"#);
        assert!(old.github);
        assert_eq!(old.launch_at_login, None);
    }

    #[test]
    fn sound_round_trips_and_invalid_values_keep_the_other_preferences() {
        for sound in [None, Some(false), Some(true)] {
            let p = Prefs {
                github: true,
                sound,
                ..Prefs::default()
            };
            assert_eq!(prefs_from_json(&prefs_to_json(&p)), p);
        }
        for json in [
            r#"{"version":1,"github":true}"#,
            r#"{"version":1,"github":true,"sound":"false"}"#,
        ] {
            assert_eq!(
                checked_prefs_from_json(json),
                Some(Prefs {
                    github: true,
                    ..Prefs::default()
                })
            );
        }
    }

    #[test]
    fn island_placement_round_trips_and_invalid_fields_keep_other_preferences() {
        let prefs = Prefs {
            github: true,
            island_position: Some("bottom-right".into()),
            island_screen: Some("é".repeat(128)),
            ..Prefs::default()
        };
        assert_eq!(prefs_from_json(&prefs_to_json(&prefs)), prefs);
        for (position, screen) in [
            (serde_json::json!("center"), serde_json::json!("Display\n")),
            (serde_json::json!(false), serde_json::json!("é".repeat(129))),
            (serde_json::json!([]), serde_json::json!(42)),
        ] {
            let json = serde_json::json!({
                "version": 1, "github": true, "island_position": position, "island_screen": screen,
            });
            assert_eq!(
                prefs_from_json(&json.to_string()),
                Prefs {
                    github: true,
                    ..Prefs::default()
                }
            );
        }
    }

    #[test]
    fn the_last_version_round_trips_and_an_invalid_one_keeps_the_rest() {
        let p = Prefs {
            github: true,
            last_version: Some("0.3.1".into()),
            ..Prefs::default()
        };
        assert_eq!(prefs_from_json(&prefs_to_json(&p)), p);
        for bad in [
            serde_json::json!(""),
            serde_json::json!("0.3.1\n"),
            serde_json::json!("1".repeat(33)),
            serde_json::json!(31),
        ] {
            let json = serde_json::json!({"version": 1, "github": true, "last_version": bad});
            let prefs = prefs_from_json(&json.to_string());
            assert!(prefs.github);
            assert_eq!(prefs.last_version, None);
        }
        assert!(is_version("1.0.0-beta.2+build.7"));
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
