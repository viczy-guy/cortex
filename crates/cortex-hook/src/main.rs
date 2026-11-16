//! cortex-hook — the relay agents call on every hook event.
//!
//! Contract (see docs/AGENTS.md):
//! 1. Read one JSON event from stdin, inject `cortex_agent` (from `--agent`)
//!    and a fresh `cortex_request_id`.
//! 2. Forward it, newline-terminated, to the Cortex endpoint.
//! 3. For blocking events (`PermissionRequest`), wait for one decision line on
//!    the same connection and translate it to the agent's hook output.
//! 4. **Never block the agent**: if Cortex is absent, slow or confusing, exit
//!    with code 0 and no output, so the agent proceeds exactly as if no hook
//!    was installed.

use std::io::Read;
use std::time::Duration;

use serde_json::Value;

/// How long we wait for the user's decision before giving up (the agent's own
/// terminal then takes over). Claude Code's default hook timeout is 60 s.
const DECISION_TIMEOUT: Duration = Duration::from_secs(50);
/// Connect/write budget. If the app is running, this is milliseconds; if it is
/// not, connecting fails instantly anyway.
const IO_TIMEOUT: Duration = Duration::from_secs(2);
/// Stdin larger than this is hostile or broken.
const MAX_STDIN: usize = 1_000_000;

fn main() {
    // Nothing in here may fail loudly: any problem means "behave as if Cortex
    // did not exist".
    let _ = run();
}

fn run() -> Option<()> {
    let agent = parse_agent_arg(std::env::args().skip(1));

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
    let request_id = fresh_request_id();
    obj.insert("cortex_request_id".into(), Value::String(request_id));

    let blocking = obj
        .get("hook_event_name")
        .and_then(Value::as_str)
        .is_some_and(is_blocking_event);

    let mut line = serde_json::to_string(&event).ok()?;
    line.push('\n');

    let response = transport::send(&line, blocking)?;

    if blocking {
        let decision = parse_decision(&response?)?;
        emit_hook_output(&event, decision);
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

/// Must stay in sync with `cortex_core::EventKind::is_blocking`.
fn is_blocking_event(name: &str) -> bool {
    name == "PermissionRequest"
}

/// Unique enough to correlate a request with its decision: this process never
/// creates two, and collisions across processes are harmless (the app keys the
/// queue per connection).
fn fresh_request_id() -> String {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    format!("{:x}-{:x}", nanos, std::process::id())
}

/// `{"decision":"allow|deny|ask"}` → the decision, if well-formed.
fn parse_decision(line: &str) -> Option<&'static str> {
    let v: Value = serde_json::from_str(line.trim()).ok()?;
    match v.get("decision")?.as_str()? {
        "allow" => Some("allow"),
        "deny" => Some("deny"),
        "ask" => Some("ask"),
        _ => None,
    }
}

/// Translate the user's decision into the agent's hook output format.
///
/// `ask` (or anything unexpected upstream) prints nothing: the agent asks in
/// its own terminal, which is always a safe outcome.
///
/// NOTE(verify-against-docs): the exact PermissionRequest output schema is
/// re-checked against the published Claude Code hooks reference in Phase 3;
/// this function is the single place that encodes it.
fn emit_hook_output(event: &Value, decision: &str) {
    if decision == "ask" {
        return;
    }
    let behavior = if decision == "allow" { "allow" } else { "deny" };
    let _ = event;
    let output = serde_json::json!({
        "hookSpecificOutput": {
            "hookEventName": "PermissionRequest",
            "decision": { "behavior": behavior }
        }
    });
    println!("{output}");
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
    fn decisions_parse_strictly() {
        assert_eq!(parse_decision(r#"{"decision":"allow"}"#), Some("allow"));
        assert_eq!(parse_decision(r#"{"decision":"deny"}"#), Some("deny"));
        assert_eq!(parse_decision(r#"{"decision":"ask"}"#), Some("ask"));
        assert_eq!(parse_decision(r#"{"decision":"yes"}"#), None);
        assert_eq!(parse_decision("junk"), None);
    }

    #[test]
    fn request_ids_differ() {
        assert_ne!(fresh_request_id(), fresh_request_id());
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
        assert_eq!(parse_decision(&response), Some("allow"));

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
