//! The hook endpoint: accepts newline-delimited JSON events from
//! `cortex-hook` (or any agent speaking the protocol in docs/AGENTS.md).
//!
//! One task per connection. Malformed input is dropped silently — this is the
//! untrusted boundary of the app.

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use cortex_core::{Decision, DecisionResponse, HookEvent};
use tauri::{AppHandle, Emitter, Manager};
use tokio::io::{
    AsyncBufRead, AsyncBufReadExt, AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, BufReader,
};
use tokio::sync::Semaphore;

use crate::state::{AppState, Blocking};

pub const RULES_EVENT: &str = "cortex://rules";

pub const STATE_EVENT: &str = "cortex://state";

/// Server-side guard: a parked connection is answered `Ask` after this long
/// even if the janitor missed it. Longer than the relay's 110 s, shorter than
/// the 120 s hook timeout.
const DECISION_GUARD: Duration = Duration::from_secs(118);

/// Largest accepted line; everything beyond is hostile or broken.
const MAX_LINE: usize = cortex_core::event::MAX_EVENT_BYTES + 1024;

/// Connections served at once. A hook's connection lasts milliseconds, or
/// until its card is answered. Beyond this a new one is closed unread, and
/// its agent carries on as if Cortex were not running.
const MAX_CONNECTIONS: usize = 64;
static CONNECTIONS: Semaphore = Semaphore::const_new(MAX_CONNECTIONS);

/// The id of a pending request. The server makes it, never the client: two
/// connections can then never claim the same card.
fn next_request_id() -> String {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let n = NEXT.fetch_add(1, Ordering::Relaxed);
    format!("{:x}-{n:x}", crate::state::now_ms())
}

/// Lines of at most [`MAX_LINE`] bytes, never holding more than that of one.
struct Lines<R> {
    reader: R,
    /// The line being read: kept here so a cancelled read loses nothing.
    buf: Vec<u8>,
}

impl<R: AsyncBufRead + Unpin> Lines<R> {
    /// `None` when the peer closed, the read failed, or a line went over the
    /// limit; the connection is over in every case.
    async fn next_line(&mut self) -> Option<String> {
        let room = (MAX_LINE + 1).saturating_sub(self.buf.len()) as u64;
        let read = (&mut self.reader)
            .take(room)
            .read_until(b'\n', &mut self.buf)
            .await
            .ok()?;
        // Over the limit: MAX_LINE bytes and still no newline after them.
        let ended = self.buf.last() == Some(&b'\n');
        if (!ended && self.buf.len() > MAX_LINE) || (read == 0 && self.buf.is_empty()) {
            return None;
        }
        let mut line = String::from_utf8(std::mem::take(&mut self.buf)).ok()?;
        line.truncate(line.trim_end_matches(['\r', '\n']).len());
        Some(line)
    }
}

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
    let mut lines = Lines {
        reader: BufReader::with_capacity(64 * 1024, read_half),
        buf: Vec::new(),
    };

    loop {
        // Closed or broken: nothing to clean, responders have guards.
        let Some(line) = lines.next_line().await else {
            return;
        };
        if line.trim().is_empty() {
            continue;
        }
        let Ok(event) = HookEvent::parse(&line) else {
            continue;
        };

        if event.is_blocking() {
            let request_id = next_request_id();

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
                                Some(_) => continue,
                                // Connection closed: the hook gave up or the
                                // user answered in the terminal. Drop the card.
                                None => {
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

/// Janitor: sweeps finished/stale sessions and expired approvals. It sleeps
/// until the next thing is due, or until an event changes what is due: no
/// timer at all while the board is empty, and no polling while it is not.
pub async fn janitor(app: AppHandle) {
    loop {
        let state = app.state::<AppState>();
        let (changed, next_due_ms) = state.sweep();
        if changed {
            emit_state(&app);
        }
        match next_due_ms {
            None => state.janitor.notified().await,
            Some(due) => {
                let wait = Duration::from_millis(due.saturating_sub(crate::state::now_ms()));
                let _ = tokio::time::timeout(wait, state.janitor.notified()).await;
            }
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
                let Ok(permit) = CONNECTIONS.try_acquire() else {
                    continue;
                };
                let app = app.clone();
                tokio::spawn(async move {
                    handle_connection(stream, app).await;
                    drop(permit);
                });
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
    // Remote clients are rejected by the pipe namespace itself (tokio's default).
    let security = pipe_security::owner_only()?;
    let create = |first: bool| {
        // SAFETY: `security` is a valid SECURITY_ATTRIBUTES that outlives every call.
        unsafe {
            ServerOptions::new()
                .first_pipe_instance(first)
                .create_with_security_attributes_raw(&path, security.as_ptr())
        }
    };
    let mut server = create(true)?;

    loop {
        server.connect().await?;
        let connected = server;
        server = create(false)?;
        let Ok(permit) = CONNECTIONS.try_acquire() else {
            continue;
        };
        let app = app.clone();
        tokio::spawn(async move {
            handle_connection(connected, app).await;
            drop(permit);
        });
    }
}

/// Who may open the pipe. Windows' default would also let every local account
/// open it for reading, and hold its connections.
#[cfg(windows)]
mod pipe_security {
    use std::ffi::c_void;

    /// Full access for the pipe's owner, the user running Cortex, and no entry
    /// for anyone else.
    const OWNER_ONLY: &str = "D:P(A;;GA;;;OW)";
    const SDDL_REVISION_1: u32 = 1;

    /// Win32 `SECURITY_ATTRIBUTES`.
    #[repr(C)]
    pub struct SecurityAttributes {
        length: u32,
        descriptor: *mut c_void,
        inherit_handle: i32,
    }

    // The pointer is to memory the system allocated for this process and that
    // nothing frees or changes.
    unsafe impl Send for SecurityAttributes {}
    unsafe impl Sync for SecurityAttributes {}

    impl SecurityAttributes {
        pub fn as_ptr(&self) -> *mut c_void {
            std::ptr::from_ref(self).cast_mut().cast()
        }
    }

    #[link(name = "advapi32")]
    extern "system" {
        fn ConvertStringSecurityDescriptorToSecurityDescriptorW(
            string: *const u16,
            revision: u32,
            descriptor: *mut *mut c_void,
            size: *mut u32,
        ) -> i32;
    }

    pub fn owner_only() -> std::io::Result<SecurityAttributes> {
        let sddl: Vec<u16> = OWNER_ONLY.encode_utf16().chain([0]).collect();
        let mut descriptor = std::ptr::null_mut();
        // SAFETY: `sddl` is NUL-terminated and alive during the call, and
        // `descriptor` is a valid place for the result. The descriptor is
        // never freed: the pipe needs it for as long as the app runs.
        let converted = unsafe {
            ConvertStringSecurityDescriptorToSecurityDescriptorW(
                sddl.as_ptr(),
                SDDL_REVISION_1,
                &mut descriptor,
                std::ptr::null_mut(),
            )
        };
        if converted == 0 {
            return Err(std::io::Error::last_os_error());
        }
        Ok(SecurityAttributes {
            length: std::mem::size_of::<SecurityAttributes>() as u32,
            descriptor,
            inherit_handle: 0,
        })
    }
}

/// Another Cortex already serves the hook endpoint. Asked before anything
/// starts: on Unix a second copy would take the socket over and leave the
/// first one deaf, and on Windows it would itself be deaf (#73). Asked a few
/// times over two seconds, because an update's restart launches the new copy
/// while the old one is still on its way out.
pub fn already_served() -> bool {
    served(&endpoint_path(), 10, Duration::from_millis(200))
}

fn served(path: &str, tries: u32, pause: Duration) -> bool {
    for attempt in 0..tries {
        if attempt > 0 {
            std::thread::sleep(pause);
        }
        if !answers(path) {
            return false;
        }
    }
    true
}

/// Someone accepts connections at `path`. A socket file left by a crash
/// refuses them. The probe sends nothing; the server sees an empty connection.
#[cfg(unix)]
fn answers(path: &str) -> bool {
    std::os::unix::net::UnixStream::connect(path).is_ok()
}

/// The pipe exists only while a Cortex serves it; busy means served too.
#[cfg(windows)]
fn answers(path: &str) -> bool {
    const ERROR_PIPE_BUSY: i32 = 231;
    match std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(path)
    {
        Ok(_) => true,
        Err(e) => e.raw_os_error() == Some(ERROR_PIPE_BUSY),
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

#[cfg(test)]
mod tests {
    use super::*;
    use tauri::async_runtime::block_on;

    fn lines<R: AsyncBufRead + Unpin>(reader: R) -> Lines<R> {
        Lines {
            reader,
            buf: Vec::new(),
        }
    }

    #[cfg(unix)]
    #[test]
    fn only_a_listening_socket_counts_as_another_copy() {
        let dir = std::env::temp_dir().join(format!("cortex-probe-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("probe.sock");
        let path_str = path.to_str().unwrap();
        let _ = std::fs::remove_file(&path);
        let pause = Duration::from_millis(1);

        assert!(!served(path_str, 3, pause), "no file at all");
        let listener = std::os::unix::net::UnixListener::bind(&path).unwrap();
        assert!(served(path_str, 3, pause), "a copy is listening");
        drop(listener);
        assert!(path.exists());
        assert!(!served(path_str, 3, pause), "a file left behind by a crash");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn lines_come_without_their_ending_and_the_last_one_needs_none() {
        block_on(async {
            let mut lines = lines(&b"one\r\n\ntwo\nthree"[..]);
            for expected in ["one", "", "two", "three"] {
                assert_eq!(lines.next_line().await.as_deref(), Some(expected));
            }
            assert_eq!(lines.next_line().await, None);
        });
    }

    #[test]
    fn a_line_over_the_limit_ends_the_connection_without_being_held() {
        block_on(async {
            let mut input = vec![b'x'; MAX_LINE];
            input.extend_from_slice(b"\nnext\n");
            let mut ok = lines(&input[..]);
            assert_eq!(ok.next_line().await.map(|l| l.len()), Some(MAX_LINE));
            assert_eq!(ok.next_line().await.as_deref(), Some("next"));

            let endless = vec![b'x'; 3 * MAX_LINE];
            let mut over = lines(&endless[..]);
            assert_eq!(over.next_line().await, None);
            assert_eq!(over.buf.len(), MAX_LINE + 1);
            assert_eq!(over.next_line().await, None);
        });
    }

    #[test]
    fn a_cancelled_read_keeps_what_it_had() {
        block_on(async {
            let (mut tx, rx) = tokio::io::duplex(64);
            let mut lines = lines(BufReader::new(rx));
            tx.write_all(b"hal").await.unwrap();
            let waited = tokio::time::timeout(Duration::from_millis(50), lines.next_line()).await;
            assert!(waited.is_err(), "half a line is not a line yet");
            tx.write_all(b"f\n").await.unwrap();
            assert_eq!(lines.next_line().await.as_deref(), Some("half"));
        });
    }

    #[test]
    fn request_ids_never_repeat() {
        assert_ne!(next_request_id(), next_request_id());
    }
}
