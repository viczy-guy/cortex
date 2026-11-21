//! Hook installation as pure data transformation on the agent's settings
//! JSON. No I/O here: reading, backing up and writing live in the app.
//!
//! Rules (see docs/SPEC.md — "Hook installer"):
//! - Never touch entries that are not ours.
//! - Installing twice is a no-op; a moved binary updates our entries in place.
//! - Anything we do not understand is an error, not a guess.

use serde_json::{json, Map, Value};

/// How Cortex recognizes its own hook entries in a settings file.
const MARKER: &str = "cortex-hook";

/// Seconds. Non-blocking events return in milliseconds; this is a safety net.
const TIMEOUT_FAST: u64 = 5;
/// Seconds. Events that may wait for the user (approvals, questions). Must
/// stay above the relay's own wait (`DECISION_TIMEOUT` in cortex-hook, 110 s).
const TIMEOUT_BLOCKING: u64 = 120;

struct HookSpec {
    event: &'static str,
    /// Tool events take a matcher; lifecycle events must not.
    matcher: Option<&'static str>,
    timeout: u64,
}

const SPECS: &[HookSpec] = &[
    HookSpec {
        event: "SessionStart",
        matcher: None,
        timeout: TIMEOUT_FAST,
    },
    HookSpec {
        event: "UserPromptSubmit",
        matcher: None,
        timeout: TIMEOUT_FAST,
    },
    HookSpec {
        event: "PreToolUse",
        matcher: Some("*"),
        timeout: TIMEOUT_BLOCKING,
    },
    HookSpec {
        event: "PermissionRequest",
        matcher: Some("*"),
        timeout: TIMEOUT_BLOCKING,
    },
    HookSpec {
        event: "PostToolUse",
        matcher: Some("*"),
        timeout: TIMEOUT_FAST,
    },
    HookSpec {
        event: "PostToolUseFailure",
        matcher: Some("*"),
        timeout: TIMEOUT_FAST,
    },
    HookSpec {
        event: "Notification",
        matcher: None,
        timeout: TIMEOUT_FAST,
    },
    HookSpec {
        event: "SubagentStart",
        matcher: None,
        timeout: TIMEOUT_FAST,
    },
    HookSpec {
        event: "SubagentStop",
        matcher: None,
        timeout: TIMEOUT_FAST,
    },
    HookSpec {
        event: "Stop",
        matcher: None,
        timeout: TIMEOUT_FAST,
    },
    HookSpec {
        event: "StopFailure",
        matcher: None,
        timeout: TIMEOUT_FAST,
    },
    HookSpec {
        event: "SessionEnd",
        matcher: None,
        timeout: TIMEOUT_FAST,
    },
];

#[derive(Debug, PartialEq, Eq)]
pub enum MergeError {
    /// The settings root is not a JSON object.
    RootNotObject,
    /// `hooks` exists but is not an object.
    HooksNotObject,
    /// `hooks.<event>` exists but is not an array.
    EventNotArray(String),
}

impl std::fmt::Display for MergeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::RootNotObject => f.write_str("settings file is not a JSON object"),
            Self::HooksNotObject => f.write_str("\"hooks\" is not an object"),
            Self::EventNotArray(e) => write!(f, "\"hooks.{e}\" is not an array"),
        }
    }
}

/// The shell command for a hook binary path (quoted: paths contain spaces).
pub fn command_for(hook_path: &str) -> String {
    format!("\"{hook_path}\"")
}

/// Return `settings` with Cortex hooks merged in.
pub fn install(settings: &Value, hook_command: &str) -> Result<Value, MergeError> {
    let mut root = settings
        .as_object()
        .cloned()
        .ok_or(MergeError::RootNotObject)?;

    let mut hooks = match root.get("hooks") {
        None | Some(Value::Null) => Map::new(),
        Some(Value::Object(map)) => map.clone(),
        Some(_) => return Err(MergeError::HooksNotObject),
    };

    for spec in SPECS {
        let mut groups = match hooks.get(spec.event) {
            None | Some(Value::Null) => Vec::new(),
            Some(Value::Array(items)) => items.clone(),
            Some(_) => return Err(MergeError::EventNotArray(spec.event.to_owned())),
        };

        if !update_existing(&mut groups, hook_command, spec.timeout) {
            groups.push(new_group(spec, hook_command));
        }
        hooks.insert(spec.event.to_owned(), Value::Array(groups));
    }

    root.insert("hooks".to_owned(), Value::Object(hooks));
    Ok(Value::Object(root))
}

/// Return `settings` with every Cortex hook entry removed. Groups and events
/// left empty by the removal are dropped; everything else is untouched.
pub fn uninstall(settings: &Value) -> Result<Value, MergeError> {
    let mut root = settings
        .as_object()
        .cloned()
        .ok_or(MergeError::RootNotObject)?;
    let Some(Value::Object(hooks)) = root.get("hooks") else {
        return Ok(Value::Object(root));
    };

    let mut cleaned = Map::new();
    for (event, value) in hooks {
        let Value::Array(groups) = value else {
            cleaned.insert(event.clone(), value.clone());
            continue;
        };
        let kept: Vec<Value> = groups
            .iter()
            .filter_map(|group| {
                let Some(Value::Array(entries)) = group.get("hooks") else {
                    return Some(group.clone());
                };
                let remaining: Vec<Value> =
                    entries.iter().filter(|e| !is_ours(e)).cloned().collect();
                if remaining.len() == entries.len() {
                    return Some(group.clone());
                }
                if remaining.is_empty() {
                    return None;
                }
                let mut group = group.clone();
                group["hooks"] = Value::Array(remaining);
                Some(group)
            })
            .collect();
        if !kept.is_empty() {
            cleaned.insert(event.clone(), Value::Array(kept));
        }
    }

    if cleaned.is_empty() {
        root.remove("hooks");
    } else {
        root.insert("hooks".to_owned(), Value::Object(cleaned));
    }
    Ok(Value::Object(root))
}

/// `true` when every event already points at `hook_command`.
pub fn is_installed(settings: &Value, hook_command: &str) -> bool {
    SPECS.iter().all(|spec| {
        settings
            .get("hooks")
            .and_then(|h| h.get(spec.event))
            .and_then(Value::as_array)
            .is_some_and(|groups| {
                groups.iter().any(|g| {
                    g.get("hooks")
                        .and_then(Value::as_array)
                        .is_some_and(|entries| {
                            entries.iter().any(|e| {
                                e.get("command").and_then(Value::as_str) == Some(hook_command)
                            })
                        })
                })
            })
    })
}

/// Installed and identical to what this version would install (timeouts,
/// events). `false` means a reinstall would change the file.
pub fn is_current(settings: &Value, hook_command: &str) -> bool {
    install(settings, hook_command).is_ok_and(|merged| &merged == settings)
}

/// A hook whose program is the relay, wherever it is installed. Matching the
/// program, not the whole text, leaves alone a command of the user's that
/// only mentions the name.
fn is_ours(entry: &Value) -> bool {
    let Some(command) = entry.get("command").and_then(Value::as_str) else {
        return false;
    };
    let program = match command.strip_prefix('"') {
        Some(quoted) => quoted.split('"').next(),
        None => command.split_whitespace().next(),
    };
    let name = program.and_then(|p| p.rsplit(['/', '\\']).next());
    name == Some(MARKER) || name.and_then(|n| n.strip_suffix(".exe")) == Some(MARKER)
}

/// Point an existing Cortex entry at the current command. Returns `false`
/// when there is none.
fn update_existing(groups: &mut [Value], hook_command: &str, timeout: u64) -> bool {
    let mut found = false;
    for group in groups.iter_mut() {
        let Some(Value::Array(entries)) = group.get_mut("hooks") else {
            continue;
        };
        for entry in entries.iter_mut().filter(|e| is_ours(e)) {
            entry["command"] = Value::String(hook_command.to_owned());
            entry["timeout"] = json!(timeout);
            found = true;
        }
    }
    found
}

fn new_group(spec: &HookSpec, hook_command: &str) -> Value {
    let handler = json!({
        "type": "command",
        "command": hook_command,
        "timeout": spec.timeout,
    });
    match spec.matcher {
        Some(matcher) => json!({ "matcher": matcher, "hooks": [handler] }),
        None => json!({ "hooks": [handler] }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const CMD: &str = "\"/home/u/.local/share/cortex/bin/cortex-hook\"";

    #[test]
    fn installs_into_empty_settings() {
        let out = install(&json!({}), CMD).unwrap();
        assert!(is_installed(&out, CMD));
        assert_eq!(out["hooks"]["PreToolUse"][0]["matcher"], "*");
        assert!(out["hooks"]["Stop"][0].get("matcher").is_none());
        assert_eq!(
            out["hooks"]["PermissionRequest"][0]["hooks"][0]["timeout"],
            120
        );
    }

    #[test]
    fn keeps_everything_that_is_not_ours() {
        let settings = json!({
            "model": "opus",
            "permissions": { "allow": ["Bash(ls:*)"] },
            "hooks": {
                "PreToolUse": [
                    { "matcher": "Bash", "hooks": [{ "type": "command", "command": "/my/lint.sh" }] }
                ],
                "CustomEvent": [{ "hooks": [{ "type": "command", "command": "/x.sh" }] }]
            }
        });
        let out = install(&settings, CMD).unwrap();

        assert_eq!(out["model"], "opus");
        assert_eq!(out["permissions"], settings["permissions"]);
        assert_eq!(
            out["hooks"]["CustomEvent"],
            settings["hooks"]["CustomEvent"]
        );
        // The user's own PreToolUse hook is still first and untouched.
        assert_eq!(
            out["hooks"]["PreToolUse"][0],
            settings["hooks"]["PreToolUse"][0]
        );
        assert_eq!(out["hooks"]["PreToolUse"].as_array().unwrap().len(), 2);
    }

    #[test]
    fn installing_twice_changes_nothing() {
        let once = install(&json!({}), CMD).unwrap();
        let twice = install(&once, CMD).unwrap();
        assert_eq!(once, twice);
    }

    #[test]
    fn an_older_timeout_is_installed_but_not_current() {
        let mut old = install(&json!({}), CMD).unwrap();
        assert!(is_current(&old, CMD));
        old["hooks"]["PermissionRequest"][0]["hooks"][0]["timeout"] = json!(60);
        assert!(is_installed(&old, CMD));
        assert!(!is_current(&old, CMD));
    }

    #[test]
    fn a_moved_binary_updates_entries_in_place() {
        let old = install(&json!({}), "\"/old/place/cortex-hook\"").unwrap();
        let new = install(&old, CMD).unwrap();
        assert!(is_installed(&new, CMD));
        assert_eq!(new["hooks"]["Stop"].as_array().unwrap().len(), 1);
    }

    #[test]
    fn uninstall_restores_the_original() {
        let original = json!({
            "model": "opus",
            "hooks": {
                "PreToolUse": [
                    { "matcher": "Bash", "hooks": [{ "type": "command", "command": "/my/lint.sh" }] }
                ]
            }
        });
        let installed = install(&original, CMD).unwrap();
        assert_eq!(uninstall(&installed).unwrap(), original);

        // And from a file that only ever had our hooks, nothing is left behind.
        let only_ours = install(&json!({ "model": "opus" }), CMD).unwrap();
        assert_eq!(uninstall(&only_ours).unwrap(), json!({ "model": "opus" }));
    }

    #[test]
    fn refuses_shapes_it_does_not_understand() {
        assert_eq!(
            install(&json!([]), CMD).unwrap_err(),
            MergeError::RootNotObject
        );
        assert_eq!(
            install(&json!({ "hooks": "nope" }), CMD).unwrap_err(),
            MergeError::HooksNotObject
        );
        assert_eq!(
            install(&json!({ "hooks": { "Stop": {} } }), CMD).unwrap_err(),
            MergeError::EventNotArray("Stop".into())
        );
    }

    #[test]
    fn user_key_order_is_preserved() {
        let settings: Value =
            serde_json::from_str(r#"{"zeta":1,"alpha":2,"model":"opus"}"#).unwrap();
        let out = install(&settings, CMD).unwrap();
        let keys: Vec<&String> = out.as_object().unwrap().keys().collect();
        assert_eq!(keys, ["zeta", "alpha", "model", "hooks"]);
    }

    #[test]
    fn only_a_command_that_runs_the_relay_is_ours() {
        let ours = |command: &str| is_ours(&json!({ "command": command }));
        assert!(ours(CMD));
        assert!(ours(
            r#""C:\Users\u\AppData\Local\Cortex\bin\cortex-hook.exe""#
        ));
        assert!(ours(
            r#""/Users/a b/Library/Application Support/Cortex/bin/cortex-hook" --agent x"#
        ));
        assert!(ours("/usr/bin/cortex-hook --agent x"));
        assert!(!ours("echo cortex-hook >> /tmp/log"));
        assert!(!ours(r#""/home/u/bin/my-cortex-hook-wrapper.sh""#));
        assert!(!ours("/opt/cortex-hook/notify.sh"));
        assert!(!is_ours(&json!({ "type": "command" })));
    }
}
