//! Versioned, opt-in application preferences.
use serde_json::Value;
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Prefs {
    pub github: bool,
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
    Some(Prefs {
        github: v["github"].as_bool()?,
    })
}

pub fn prefs_to_json(prefs: &Prefs) -> String {
    serde_json::json!({"version":1,"github":prefs.github}).to_string()
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn preferences_round_trip_and_ignore_unknown_keys() {
        let p = Prefs { github: true };
        assert_eq!(prefs_from_json(&prefs_to_json(&p)), p);
        assert_eq!(
            prefs_from_json(r#"{"version":1,"github":true,"other":42}"#),
            p
        );
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
