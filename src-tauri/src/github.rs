//! Opt-in GitHub polling through the user's CLI credentials.
use cortex_core::{
    github::{parse_summary, QUERY},
    GithubSummary,
};
use serde::Serialize;
use std::{
    io::Read,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::{mpsc, Mutex},
    time::{Duration, Instant},
};
use tauri::{AppHandle, Emitter, Manager, State};
use tokio::sync::Notify;
pub const GITHUB_EVENT: &str = "cortex://github";
const POLL: Duration = Duration::from_secs(180);
const GH_TIMEOUT: Duration = Duration::from_secs(20);
const MAX_STDOUT: u64 = 2 * 1024 * 1024;
#[derive(Clone, Serialize)]
pub struct GithubStatus {
    pub enabled: bool,
    pub state: &'static str,
    /// Single-line error detail, capped at 200 characters.
    pub message: Option<String>,
    /// Last good data, retained after a later fetch error.
    pub summary: Option<GithubSummary>,
}
pub struct GithubState {
    status: Mutex<GithubStatus>,
    wake: Notify,
}
impl GithubState {
    pub fn new(enabled: bool) -> Self {
        Self {
            status: Mutex::new(GithubStatus {
                enabled,
                state: if enabled { "checking" } else { "off" },
                message: None,
                summary: None,
            }),
            wake: Notify::new(),
        }
    }
}
fn find_gh() -> Option<PathBuf> {
    #[cfg(debug_assertions)]
    if let Some(path) = std::env::var_os("CORTEX_GH") {
        let p = PathBuf::from(path);
        if p.is_file() {
            return Some(p);
        }
    }
    let name = if cfg!(windows) { "gh.exe" } else { "gh" };
    let mut paths: Vec<PathBuf> = std::env::var_os("PATH")
        .map(|p| std::env::split_paths(&p).map(|p| p.join(name)).collect())
        .unwrap_or_default();
    paths.extend(
        [
            "/opt/homebrew/bin/gh",
            "/usr/local/bin/gh",
            "/usr/bin/gh",
            "/home/linuxbrew/.linuxbrew/bin/gh",
            "/snap/bin/gh",
        ]
        .map(PathBuf::from),
    );
    #[cfg(windows)]
    for (var, tail) in [
        ("ProgramFiles", "GitHub CLI\\gh.exe"),
        ("LOCALAPPDATA", "Programs\\GitHub CLI\\gh.exe"),
    ] {
        if let Some(p) = std::env::var_os(var) {
            paths.push(PathBuf::from(p).join(tail));
        }
    }
    paths.into_iter().find(|p| p.is_file())
}
#[derive(Debug)]
enum GhError {
    SignedOut,
    Failed(String),
}
fn detail(s: &str) -> String {
    s.lines()
        .find(|l| !l.trim().is_empty())
        .unwrap_or("gh failed")
        .chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .take(200)
        .collect()
}
fn no_console(command: &mut Command) {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x0800_0000);
    }
    #[cfg(not(windows))]
    let _ = command;
}
fn read_pipe(pipe: impl Read, cap: u64) -> std::io::Result<Vec<u8>> {
    let mut bytes = Vec::new();
    pipe.take(cap + 1).read_to_end(&mut bytes)?;
    Ok(bytes)
}
fn run_gh(gh: &Path) -> Result<String, GhError> {
    let fail = |e: std::io::Error| GhError::Failed(detail(&e.to_string()));
    let mut command = Command::new(gh);
    command
        .args(["api", "graphql", "-f", &format!("query={QUERY}")])
        .env("GH_PROMPT_DISABLED", "1")
        .env("NO_COLOR", "1")
        .env("GH_NO_UPDATE_NOTIFIER", "1")
        .env("CLICOLOR", "0")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    no_console(&mut command);
    let mut child = command.spawn().map_err(fail)?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| GhError::Failed("missing gh stdout".into()))?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| GhError::Failed("missing gh stderr".into()))?;
    let (out_tx, out_rx) = mpsc::sync_channel(1);
    let (err_tx, err_rx) = mpsc::sync_channel(1);
    std::thread::spawn(move || {
        let _ = out_tx.send(read_pipe(stdout, MAX_STDOUT));
    });
    std::thread::spawn(move || {
        let _ = err_tx.send(read_pipe(stderr, 64 * 1024));
    });
    let start = Instant::now();
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break Ok(status),
            Ok(None) if start.elapsed() < GH_TIMEOUT => {
                std::thread::sleep(Duration::from_millis(50))
            }
            other => {
                let _ = child.kill();
                let _ = child.wait();
                break Err(match other {
                    Err(e) => fail(e),
                    _ => GhError::Failed("gh timed out".into()),
                });
            }
        }
    };
    let status = status?;
    // Credential helpers may inherit a pipe: never wait beyond the same deadline.
    let receive = |rx: mpsc::Receiver<std::io::Result<Vec<u8>>>| {
        rx.recv_timeout(GH_TIMEOUT.saturating_sub(start.elapsed()))
            .map_err(|_| GhError::Failed("gh output timed out".into()))?
            .map_err(fail)
    };
    let output = receive(out_rx)?;
    let errors = receive(err_rx)?;
    if output.len() as u64 > MAX_STDOUT {
        return Err(GhError::Failed("gh response too large".into()));
    }
    if !status.success() {
        let text = String::from_utf8_lossy(&errors);
        return Err(if text.contains("gh auth login") {
            GhError::SignedOut
        } else {
            GhError::Failed(detail(&text))
        });
    }
    String::from_utf8(output).map_err(|_| GhError::Failed("unexpected response from gh".into()))
}
#[tauri::command]
pub fn github_status(state: State<'_, GithubState>) -> GithubStatus {
    state.status.lock().expect("github mutex poisoned").clone()
}
#[tauri::command]
pub fn github_set_enabled(
    app: AppHandle,
    enabled: bool,
    state: State<'_, GithubState>,
) -> Result<(), String> {
    let mut status = state.status.lock().expect("github mutex poisoned");
    crate::prefs::save(&crate::prefs::Prefs { github: enabled })?;
    status.enabled = enabled;
    if !enabled {
        status.summary = None;
        status.state = "off";
        status.message = None;
    }
    let _ = app.emit(GITHUB_EVENT, &*status);
    state.wake.notify_one();
    Ok(())
}
#[tauri::command]
pub fn github_refresh(state: State<'_, GithubState>) {
    if state.status.lock().expect("github mutex poisoned").enabled {
        state.wake.notify_one();
    }
}
#[tauri::command]
pub fn open_url(url: String, state: State<'_, GithubState>) -> Result<(), String> {
    let status = state.status.lock().expect("github mutex poisoned");
    if !status.summary.as_ref().is_some_and(|s| s.has_url(&url)) {
        return Err("URL is not in the current GitHub summary".into());
    }
    #[cfg(target_os = "macos")]
    let mut command = Command::new("open");
    #[cfg(target_os = "linux")]
    let mut command = Command::new("xdg-open");
    #[cfg(windows)]
    let mut command = {
        let mut c = Command::new("rundll32");
        c.arg("url.dll,FileProtocolHandler");
        c
    };
    command
        .arg(url)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    no_console(&mut command);
    let mut child = command.spawn().map_err(|e| e.to_string())?;
    std::thread::spawn(move || {
        let _ = child.wait();
    });
    Ok(())
}
pub async fn poller(app: AppHandle) {
    let state = app.state::<GithubState>();
    loop {
        let enabled = {
            let mut s = state.status.lock().expect("github mutex poisoned");
            s.state = if s.enabled { "checking" } else { "off" };
            s.message = None;
            let _ = app.emit(GITHUB_EVENT, &*s);
            s.enabled
        };
        if !enabled {
            state.wake.notified().await;
            continue;
        }
        let result = tauri::async_runtime::spawn_blocking(|| find_gh().map(|gh| run_gh(&gh))).await;
        {
            let mut s = state.status.lock().expect("github mutex poisoned");
            // A result started before opt-out must never restore cleared data.
            if s.enabled {
                let (label, message) = match result {
                    Ok(Some(Ok(text))) => match parse_summary(&text, crate::state::now_ms()) {
                        Some(summary) => {
                            s.summary = Some(summary);
                            ("ok", None)
                        }
                        None => ("error", Some("unexpected response from gh".into())),
                    },
                    Ok(None) => ("no_gh", Some("GitHub CLI (gh) not found".into())),
                    Ok(Some(Err(GhError::SignedOut))) => ("signed_out", None),
                    Ok(Some(Err(GhError::Failed(message)))) => ("error", Some(message)),
                    Err(_) => ("error", Some("gh worker failed".into())),
                };
                s.state = label;
                s.message = message;
            }
            let _ = app.emit(GITHUB_EVENT, &*s);
        }
        if state.status.lock().expect("github mutex poisoned").enabled {
            tokio::select! { _=tokio::time::sleep(POLL) => {}, _=state.wake.notified() => {} }
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn error_details_are_bounded_single_lines() {
        assert_eq!(detail("\n\nfailed\nsecret"), "failed");
        assert_eq!(detail(&"é".repeat(300)).chars().count(), 200);
        assert!(!detail("a\tb").contains('\t'));
    }
    #[test]
    fn pipe_reads_are_bounded_even_if_input_is_larger() {
        assert_eq!(read_pipe(&b"abcdef"[..], 3).unwrap(), b"abcd");
    }
    #[test]
    fn initial_state_has_no_summary() {
        let s = GithubState::new(false);
        let s = s.status.lock().unwrap();
        assert!(!s.enabled);
        assert_eq!(s.state, "off");
        assert!(s.summary.is_none());
    }
}
