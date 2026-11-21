//! cortex-hook — the relay agents call on every hook event.
//!
//! Contract (see docs/AGENTS.md):
//! 1. Read one JSON event from stdin and inject `cortex_agent` (from `--agent`).
//! 2. Forward it, newline-terminated, to the Cortex endpoint.
//! 3. For blocking events (a `PermissionRequest`, or the `PreToolUse` of
//!    `AskUserQuestion`), wait for one response line on the same connection
//!    and translate it to the agent's hook output.
//! 4. **Never block the agent**: if Cortex is absent, slow or confusing, exit
//!    with code 0 and no output, so the agent proceeds exactly as if no hook
//!    was installed.

use std::io::Read;
use std::time::Duration;

use serde_json::Value;

/// How long we wait for the user's decision before giving up (the agent's own
/// terminal then takes over). Long enough to read a multi-question card;
/// "Answer in terminal" hands it back sooner. Must stay below the `timeout`
/// the installer writes for blocking hooks (120 s).
const DECISION_TIMEOUT: Duration = Duration::from_secs(110);
/// Connect/write budget. If the app is running, this is milliseconds; if it is
/// not, connecting fails instantly anyway.
const IO_TIMEOUT: Duration = Duration::from_secs(2);
/// Stdin larger than this is hostile or broken. The same cap applies to the
/// line sent: it is `cortex_core::event::MAX_EVENT_BYTES`, what the app accepts.
const MAX_STDIN: usize = 1_000_000;

fn main() {
    // Nothing in here may fail loudly: any problem means "behave as if Cortex
    // did not exist".
    let _ = run();
}

fn run() -> Option<()> {
    // One deadline for everything (stdin, connect, write, the decision), on
    // every platform: no read or connect below can outlive it. Exiting here
    // is the contract's "exit 0 with no output".
    std::thread::spawn(|| {
        std::thread::sleep(DECISION_TIMEOUT + IO_TIMEOUT);
        std::process::exit(0);
    });

    // `args()` panics on an argument that is not UTF-8; such a name is invalid anyway.
    let args = std::env::args_os().map(|a| a.into_string().unwrap_or_default());
    let agent = parse_agent_arg(args.skip(1));

    let mut raw = String::new();
    std::io::stdin()
        .take(MAX_STDIN as u64 + 1)
        .read_to_string(&mut raw)
        .ok()?;
    if raw.len() > MAX_STDIN {
        return None;
    }

    let mut event: Value = serde_json::from_str(&raw).ok()?;
    let obj = event.as_object_mut()?;

    if let Some(name) = agent {
        obj.insert("cortex_agent".into(), Value::String(name));
    }

    let blocking = blocking_kind(&event);
    let line = outgoing_line(&event)?;

    let response = transport::send(&line, blocking.is_some())?;

    if let Some(kind) = blocking {
        let response: Value = serde_json::from_str(response?.trim()).ok()?;
        if let Some(output) = hook_output(kind, &event, &response) {
            println!("{output}");
        }
    }
    Some(())
}

/// `--agent <name>` → validated agent name. Anything invalid is ignored.
fn parse_agent_arg(mut args: impl Iterator<Item = String>) -> Option<String> {
    while let Some(arg) = args.next() {
        if arg == "--agent" {
            let name = args.next()?;
            let valid = !name.is_empty()
                && name.len() <= 24
                && name
                    .bytes()
                    .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-');
            return valid.then_some(name);
        }
    }
    None
}

/// The two events where the agent waits for the user.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Blocking {
    /// `PermissionRequest`: allow or deny a tool call.
    Permission,
    /// `PreToolUse` of `AskUserQuestion`: answer a question.
    Question,
}

/// Must stay in sync with `cortex_core::HookEvent::is_blocking`.
fn blocking_kind(event: &Value) -> Option<Blocking> {
    let name = event.get("hook_event_name")?.as_str()?;
    let tool = event.get("tool_name").and_then(Value::as_str);
    match (name, tool) {
        ("PermissionRequest", _) => Some(Blocking::Permission),
        ("PreToolUse", Some("AskUserQuestion")) => Some(Blocking::Question),
        _ => None,
    }
}

/// The line for the app, or `None` when it would be dropped there for its
/// size: better to hand the request to the terminal now than to wait for a
/// card that will never appear.
fn outgoing_line(event: &Value) -> Option<String> {
    let mut line = serde_json::to_string(event).ok()?;
    if line.len() > MAX_STDIN {
        return None;
    }
    line.push('\n');
    Some(line)
}

/// Translate Cortex's response line into the agent's hook output, following
/// the Claude Code hooks reference (code.claude.com/docs/en/hooks). This is
/// the single place that encodes those schemas.
///
/// `None` means "print nothing": the agent asks in its own terminal, which is
/// always a safe outcome. That is what happens for `ask` and for anything
/// malformed.
fn hook_output(kind: Blocking, event: &Value, response: &Value) -> Option<Value> {
    let decision = response.get("decision")?.as_str()?;
    match (kind, decision) {
        (Blocking::Permission, "allow") => Some(serde_json::json!({
            "hookSpecificOutput": {
                "hookEventName": "PermissionRequest",
                "decision": { "behavior": "allow" }
            }
        })),
        (Blocking::Permission, "deny") => Some(serde_json::json!({
            "hookSpecificOutput": {
                "hookEventName": "PermissionRequest",
                "decision": { "behavior": "deny", "message": "Denied from Cortex." }
            }
        })),
        (Blocking::Question, "allow") => {
            // The tool only runs without prompting when the original
            // `questions` are echoed back together with the `answers`.
            let questions = event.get("tool_input")?.get("questions")?;
            let answers = response.get("answers")?.as_object()?;
            if answers.is_empty() || !answers.values().all(Value::is_string) {
                return None;
            }
            Some(serde_json::json!({
                "hookSpecificOutput": {
                    "hookEventName": "PreToolUse",
                    "permissionDecision": "allow",
                    "updatedInput": { "questions": questions, "answers": answers }
                }
            }))
        }
        _ => None,
    }
}

/// Platform transport. Returns `Some(response)` on success; the inner Option
/// is the decision line (only read for blocking sends).
mod transport {
    use super::{DECISION_TIMEOUT, IO_TIMEOUT};
    use std::io::{BufRead, BufReader};

    /// Send one line; optionally read one line back.
    pub fn send(line: &str, read_response: bool) -> Option<Option<String>> {
        let stream = connect()?;
        send_on(stream, line, read_response)
    }

    fn send_on<S: std::io::Read + std::io::Write + SetTimeouts>(
        mut stream: S,
        line: &str,
        read_response: bool,
    ) -> Option<Option<String>> {
        stream.set_write_deadline(IO_TIMEOUT);
        stream.write_all(line.as_bytes()).ok()?;
        stream.flush().ok()?;

        if !read_response {
            return Some(None);
        }
        stream.set_read_deadline(DECISION_TIMEOUT);
        let mut reader = BufReader::new(stream);
        let mut response = String::new();
        reader.read_line(&mut response).ok()?;
        if response.is_empty() {
            return None;
        }
        Some(Some(response))
    }

    pub trait SetTimeouts {
        fn set_read_deadline(&mut self, d: std::time::Duration);
        fn set_write_deadline(&mut self, d: std::time::Duration);
    }

    #[cfg(unix)]
    fn connect() -> Option<std::os::unix::net::UnixStream> {
        std::os::unix::net::UnixStream::connect(super::endpoint::path()).ok()
    }

    #[cfg(unix)]
    impl SetTimeouts for std::os::unix::net::UnixStream {
        fn set_read_deadline(&mut self, d: std::time::Duration) {
            let _ = self.set_read_timeout(Some(d));
        }
        fn set_write_deadline(&mut self, d: std::time::Duration) {
            let _ = self.set_write_timeout(Some(d));
        }
    }

    #[cfg(windows)]
    fn connect() -> Option<std::fs::File> {
        // Named pipes accept plain file opens on the client side. Opening fails
        // instantly when the pipe (the app) does not exist.
        std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(super::endpoint::path())
            .ok()
    }

    #[cfg(windows)]
    impl SetTimeouts for std::fs::File {
        // Pipe reads honor the server's lifetime; Cortex answers or closes.
        fn set_read_deadline(&mut self, _d: std::time::Duration) {}
        fn set_write_deadline(&mut self, _d: std::time::Duration) {}
    }
}

/// Endpoint resolution. `CORTEX_SOCKET` always wins (SSH tunnels, tests).
mod endpoint {
    pub fn path() -> String {
        if let Ok(p) = std::env::var("CORTEX_SOCKET") {
            if !p.is_empty() {
                return p;
            }
        }
        default_path()
    }

    #[cfg(target_os = "macos")]
    fn default_path() -> String {
        let home = std::env::var("HOME").unwrap_or_default();
        format!("{home}/Library/Application Support/Cortex/cortex.sock")
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    fn default_path() -> String {
        match std::env::var("XDG_RUNTIME_DIR") {
            Ok(dir) if !dir.is_empty() => format!("{dir}/cortex.sock"),
            _ => {
                let home = std::env::var("HOME").unwrap_or_default();
                format!("{home}/.cortex/cortex.sock")
            }
        }
    }

    #[cfg(windows)]
    fn default_path() -> String {
        let user = std::env::var("USERNAME").unwrap_or_else(|_| "default".into());
        format!(r"\\.\pipe\cortex-{user}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn agent_arg_is_validated() {
        let parse = |args: &[&str]| parse_agent_arg(args.iter().map(|s| s.to_string()));
        assert_eq!(parse(&["--agent", "my-tool"]), Some("my-tool".into()));
        assert_eq!(parse(&["--agent", "BAD"]), None);
        assert_eq!(parse(&["--agent"]), None);
        assert_eq!(parse(&[]), None);
        assert_eq!(
            parse(&["--other", "x", "--agent", "ok2"]),
            Some("ok2".into())
        );
    }

    #[test]
    fn only_approvals_and_questions_block() {
        let kind = |json: &str| blocking_kind(&serde_json::from_str(json).unwrap());
        assert_eq!(
            kind(r#"{"hook_event_name":"PermissionRequest"}"#),
            Some(Blocking::Permission)
        );
        assert_eq!(
            kind(r#"{"hook_event_name":"PreToolUse","tool_name":"AskUserQuestion"}"#),
            Some(Blocking::Question)
        );
        assert_eq!(
            kind(r#"{"hook_event_name":"PreToolUse","tool_name":"Bash"}"#),
            None
        );
        assert_eq!(kind(r#"{"hook_event_name":"Stop"}"#), None);
    }

    #[test]
    fn permission_output_follows_the_hooks_reference() {
        let event = serde_json::json!({});
        let out = |d: &str| {
            hook_output(
                Blocking::Permission,
                &event,
                &serde_json::json!({ "decision": d }),
            )
        };
        assert_eq!(
            out("allow").unwrap()["hookSpecificOutput"]["decision"]["behavior"],
            "allow"
        );
        assert_eq!(
            out("deny").unwrap()["hookSpecificOutput"]["decision"]["behavior"],
            "deny"
        );
        // `ask` and anything unexpected print nothing: the terminal decides.
        assert!(out("ask").is_none());
        assert!(out("yes").is_none());
    }

    #[test]
    fn question_output_echoes_questions_with_answers() {
        let event = serde_json::json!({
            "tool_input": { "questions": [{ "question": "Which framework?" }] }
        });
        let response = serde_json::json!({
            "decision": "allow",
            "answers": { "Which framework?": "React" }
        });
        let out = hook_output(Blocking::Question, &event, &response).unwrap();
        let specific = &out["hookSpecificOutput"];
        assert_eq!(specific["hookEventName"], "PreToolUse");
        assert_eq!(specific["permissionDecision"], "allow");
        assert_eq!(
            specific["updatedInput"]["questions"],
            event["tool_input"]["questions"]
        );
        assert_eq!(
            specific["updatedInput"]["answers"]["Which framework?"],
            "React"
        );

        // An allow without answers must not let the tool run unanswered.
        let bare = serde_json::json!({ "decision": "allow" });
        assert!(hook_output(Blocking::Question, &event, &bare).is_none());
        let ask = serde_json::json!({ "decision": "ask" });
        assert!(hook_output(Blocking::Question, &event, &ask).is_none());
    }

    #[test]
    fn a_line_the_app_would_drop_is_not_sent() {
        let padded = |len: usize| {
            let event = serde_json::json!({ "hook_event_name": "Stop", "pad": "" });
            let empty = serde_json::to_string(&event).unwrap().len();
            serde_json::json!({ "hook_event_name": "Stop", "pad": "x".repeat(len - empty) })
        };
        let line = outgoing_line(&padded(MAX_STDIN)).unwrap();
        assert_eq!(line.len(), MAX_STDIN + 1); // the newline is not part of the event
        assert!(outgoing_line(&padded(MAX_STDIN + 1)).is_none());
    }

    #[cfg(unix)]
    #[test]
    fn end_to_end_against_a_fake_server() {
        use std::io::{BufRead, BufReader, Write};
        use std::os::unix::net::UnixListener;

        let dir = std::env::temp_dir().join(format!("cortex-hook-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let sock = dir.join("test.sock");
        let listener = UnixListener::bind(&sock).unwrap();

        let server = std::thread::spawn(move || {
            let (stream, _) = listener.accept().unwrap();
            let mut reader = BufReader::new(stream.try_clone().unwrap());
            let mut line = String::new();
            reader.read_line(&mut line).unwrap();
            let mut stream = stream;
            stream.write_all(b"{\"decision\":\"allow\"}\n").unwrap();
            line
        });

        std::env::set_var("CORTEX_SOCKET", sock.to_str().unwrap());
        let event = "{\"hook_event_name\":\"PermissionRequest\",\"session_id\":\"s\"}\n";
        let response = transport::send(event, true).unwrap().unwrap();
        assert!(response.contains("\"allow\""));

        let received = server.join().unwrap();
        assert!(received.contains("PermissionRequest"));
        std::env::remove_var("CORTEX_SOCKET");
        let _ = std::fs::remove_dir_all(dir);
    }

    #[cfg(unix)]
    #[test]
    fn absent_app_fails_fast_and_quiet() {
        std::env::set_var("CORTEX_SOCKET", "/nonexistent/cortex.sock");
        let started = std::time::Instant::now();
        assert!(transport::send("{}\n", false).is_none());
        assert!(started.elapsed() < std::time::Duration::from_millis(50));
        std::env::remove_var("CORTEX_SOCKET");
    }
}
