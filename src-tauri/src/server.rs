//! The hook endpoint: accepts newline-delimited JSON events from
//! `cortex-hook` (or any agent speaking the protocol in docs/AGENTS.md).
//!
//! One task per connection. Malformed input is dropped silently — this is the
//! untrusted boundary of the app.

use std::time::Duration;

use cortex_core::{Decision, DecisionResponse, HookEvent};
use tauri::{AppHandle, Emitter, Manager};
use tokio::io::{AsyncBufReadExt, AsyncRead, AsyncWrite, AsyncWriteExt, BufReader};

use crate::state::{AppState, Blocking};

pub const RULES_EVENT: &str = "cortex://rules";

pub const STATE_EVENT: &str = "cortex://state";

/// Server-side guard: a parked connection is answered `Ask` after this long
/// even if the janitor missed it. Slightly longer than the hook's own 50 s.
const DECISION_GUARD: Duration = Duration::from_secs(58);

/// Largest accepted line; everything beyond is hostile or broken.
const MAX_LINE: usize = cortex_core::event::MAX_EVENT_BYTES + 1024;

pub fn emit_state(app: &AppHandle) {
    let state = app.state::<AppState>();
    let snapshot = state.snapshot();
    let _ = app.emit(STATE_EVENT, snapshot);
}

pub fn emit_rules(app: &AppHandle) {
    let _ = app.emit(RULES_EVENT, app.state::<AppState>().rules());
}

/// Handle one agent connection end-to-end.
async fn handle_connection<S>(stream: S, app: AppHandle)
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    let (read_half, mut write_half) = tokio::io::split(stream);
    let mut lines = BufReader::with_capacity(64 * 1024, read_half).lines();

    loop {
        let line = match lines.next_line().await {
            Ok(Some(line)) => line,
            _ => return, // closed or broken: nothing to clean, responders have guards
        };
        if line.len() > MAX_LINE || line.trim().is_empty() {
            continue;
        }
        let Ok(event) = HookEvent::parse(&line) else {
            continue;
        };

        if event.is_blocking() {
            // The hook always injects a request id; synthesize one defensively.
            let request_id = event
                .request_id
                .clone()
                .unwrap_or_else(|| format!("anon-{:x}", crate::state::now_ms()));

            let state = app.state::<AppState>();
            let blocking = state.register_blocking(&event, request_id.clone());
            emit_state(&app);

            let ask = || DecisionResponse::new(Decision::Ask);
            let response = match blocking {
                Blocking::Immediate(response) => response,
                Blocking::Wait(mut rx) => {
                    let guard = tokio::time::sleep(DECISION_GUARD);
                    tokio::pin!(guard);
                    loop {
                        tokio::select! {
                            decided = &mut rx => break decided.unwrap_or_else(|_| ask()),
                            read = lines.next_line() => match read {
                                // Stray input while waiting is ignored.
                                Ok(Some(_)) => continue,
                                // Connection closed: the hook gave up or the
                                // user answered in the terminal. Drop the card.
                                _ => {
                                    if app.state::<AppState>().cancel(&request_id) {
                                        emit_state(&app);
                                    }
                                    return;
                                }
                            },
                            _ = &mut guard => {
                                app.state::<AppState>().cancel(&request_id);
                                break ask();
                            }
                        }
                    }
                }
            };

            let response = response.to_line();
            let _ = write_half.write_all(response.as_bytes()).await;
            let _ = write_half.flush().await;
            emit_state(&app);
        } else {
            let state = app.state::<AppState>();
            state.apply_event(&event);
            emit_state(&app);
        }
    }
}

/// Janitor: sweeps finished/stale sessions and expired approvals, but only
/// while there is something on the board — 0% CPU when the island is idle.
pub async fn janitor(app: AppHandle) {
    loop {
        let state = app.state::<AppState>();
        let (changed, empty) = state.sweep();
        if changed {
            emit_state(&app);
        }
        if empty {
            // Park until the next event arrives. No timers while idle.
            state.janitor.notified().await;
        } else {
            tokio::time::sleep(Duration::from_secs(2)).await;
        }
    }
}

// ---------------------------------------------------------------------------
// Unix: a socket file reachable only by this user.

#[cfg(unix)]
pub async fn serve(app: AppHandle) -> std::io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    use tokio::net::UnixListener;

    let path = endpoint_path();
    let dir = std::path::Path::new(&path)
        .parent()
        .ok_or_else(|| std::io::Error::other("endpoint has no parent directory"))?;
    std::fs::create_dir_all(dir)?;
    std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700))?;
    // Remove a stale socket from a previous run; errors are fine (first run).
    let _ = std::fs::remove_file(&path);

    let listener = UnixListener::bind(&path)?;
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))?;

    loop {
        match listener.accept().await {
            Ok((stream, _)) => {
                let app = app.clone();
                tokio::spawn(handle_connection(stream, app));
            }
            Err(_) => tokio::time::sleep(Duration::from_millis(100)).await,
        }
    }
}

// ---------------------------------------------------------------------------
// Windows: a per-user named pipe.

#[cfg(windows)]
pub async fn serve(app: AppHandle) -> std::io::Result<()> {
    use tokio::net::windows::named_pipe::ServerOptions;

    let path = endpoint_path();
    // Default pipe security on Windows grants access to the creating user;
    // remote clients are rejected by the pipe namespace itself.
    let mut server = ServerOptions::new()
        .first_pipe_instance(true)
        .create(&path)?;

    loop {
        server.connect().await?;
        let connected = server;
        server = ServerOptions::new().create(&path)?;
        let app = app.clone();
        tokio::spawn(handle_connection(connected, app));
    }
}

/// Must stay in sync with `cortex-hook`'s endpoint resolution.
pub fn endpoint_path() -> String {
    if let Ok(p) = std::env::var("CORTEX_SOCKET") {
        if !p.is_empty() {
            return p;
        }
    }
    default_endpoint()
}

#[cfg(target_os = "macos")]
fn default_endpoint() -> String {
    let home = std::env::var("HOME").unwrap_or_default();
    format!("{home}/Library/Application Support/Cortex/cortex.sock")
}

#[cfg(all(unix, not(target_os = "macos")))]
fn default_endpoint() -> String {
    match std::env::var("XDG_RUNTIME_DIR") {
        Ok(dir) if !dir.is_empty() => format!("{dir}/cortex.sock"),
        _ => {
            let home = std::env::var("HOME").unwrap_or_default();
            format!("{home}/.cortex/cortex.sock")
        }
    }
}

#[cfg(windows)]
fn default_endpoint() -> String {
    let user = std::env::var("USERNAME").unwrap_or_else(|_| "default".into());
    format!(r"\\.\pipe\cortex-{user}")
}
