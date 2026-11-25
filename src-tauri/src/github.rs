//! Opt-in GitHub polling through the user's CLI credentials.
use crate::prefs::{self, Prefs};
use cortex_core::{
    atlantis::with_detail,
    github::{
        account_is_shown, active_account, device_code, has_account, is_valid_login, is_valid_token,
        overall_state, parse_accounts, parse_summary, poll_due, release_notes_url, GhAccount,
        CLI_URL, DEVICE_URL, MAX_ACCOUNTS, POLL_SECS, POLL_SLOW_SECS, QUERY,
    },
    notices::{
        known_from_json, known_to_json, notices, notifications_path, parse_failed_runs,
        parse_plans, plan_query, remember, split_poll_interval, FailedRun, Known, LastRun,
        PlanCache, MAX_KNOWN_BYTES,
    },
    GithubSummary,
};
use serde::Serialize;
use std::{
    collections::BTreeMap,
    io::{BufRead, BufReader, Read},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        mpsc, Arc, Mutex, MutexGuard,
    },
    time::{Duration, Instant},
};
use tauri::{AppHandle, Emitter, Manager, State};
use tokio::sync::Notify;

pub const GITHUB_EVENT: &str = "cortex://github";
const GH_TIMEOUT: Duration = Duration::from_secs(20);
const MAX_STDOUT: u64 = 2 * 1024 * 1024;
const MAX_DIAGNOSTICS: u64 = 64 * 1024;
const AUTH_STATUS: &[&str] = &["auth", "status", "--json", "hosts"];

#[derive(Clone, Serialize)]
pub struct AccountStatus {
    pub login: String,
    pub active: bool,
    pub shown: bool,
    pub state: &'static str,
    pub message: Option<String>,
    pub summary: Option<GithubSummary>,
    /// On the first answer after launch only: the keys of the notices that
    /// came while Cortex was closed (#157).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub new_since_last_run: Vec<String>,
}

#[derive(Clone, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Task {
    Idle,
    Installing,
    SigningIn { code: Option<String> },
    Failed { message: String },
}

#[derive(Clone, Serialize)]
pub struct GithubStatus {
    pub enabled: bool,
    pub state: &'static str,
    pub message: Option<String>,
    pub accounts: Vec<AccountStatus>,
    pub installer: Option<&'static str>,
    pub task: Task,
}

struct Inner {
    status: GithubStatus,
    prefs: Prefs,
    revision: u64,
    // Kept until the worker reaps the child and restores the active account.
    running: Option<Arc<AtomicBool>>,
}

impl Inner {
    fn emit(&self, app: &AppHandle) {
        let _ = app.emit(GITHUB_EVENT, &self.status);
    }

    fn cancel(&mut self) {
        if let Some(cancel) = &self.running {
            cancel.store(true, Ordering::Relaxed);
        }
        self.status.task = Task::Idle;
    }
}

pub struct GithubState {
    inner: Mutex<Inner>,
    wake: Notify,
    /// When the last good answer was asked for (wall clock, ms; 0 for none),
    /// so coming back from a sleep can wait for one asked after it (#157).
    answered_ms: AtomicU64,
}

impl GithubState {
    pub fn new(prefs: Prefs) -> Self {
        Self {
            inner: Mutex::new(Inner {
                status: GithubStatus {
                    enabled: prefs.github,
                    state: if prefs.github { "checking" } else { "off" },
                    message: None,
                    accounts: Vec::new(),
                    installer: None,
                    task: Task::Idle,
                },
                prefs,
                revision: 0,
                running: None,
            }),
            wake: Notify::new(),
            answered_ms: AtomicU64::new(0),
        }
    }

    fn lock(&self) -> MutexGuard<'_, Inner> {
        self.inner.lock().expect("github mutex poisoned")
    }

    pub fn enabled(&self) -> bool {
        self.lock().status.enabled
    }

    /// When the last good answer was asked for; `None` while GitHub is off.
    pub fn answered_ms(&self) -> Option<u64> {
        self.enabled()
            .then(|| self.answered_ms.load(Ordering::Relaxed))
    }

    /// Poll now rather than at the next turn.
    pub fn refresh(&self) {
        if self.enabled() {
            self.wake.notify_one();
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
fn installer() -> Option<(&'static str, PathBuf)> {
    #[cfg(target_os = "macos")]
    {
        ["/opt/homebrew/bin/brew", "/usr/local/bin/brew"]
            .into_iter()
            .map(PathBuf::from)
            .find(|p| p.is_file())
            .map(|p| ("brew", p))
    }
    #[cfg(windows)]
    {
        let path = std::env::var_os("PATH").unwrap_or_default();
        std::env::split_paths(&path)
            .map(|p| p.join("winget.exe"))
            .chain(
                std::env::var_os("LOCALAPPDATA")
                    .map(|p| PathBuf::from(p).join(r"Microsoft\WindowsApps\winget.exe")),
            )
            .find(|p| p.is_file())
            .map(|p| ("winget", p))
    }
    #[cfg(not(any(target_os = "macos", windows)))]
    None
}

#[derive(Debug)]
struct GhError(String);

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
fn child_command(path: &Path) -> Command {
    let mut command = Command::new(path);
    for name in [
        "GH_TOKEN",
        "GITHUB_TOKEN",
        "GH_ENTERPRISE_TOKEN",
        "GITHUB_ENTERPRISE_TOKEN",
        "GH_HOST",
    ] {
        command.env_remove(name);
    }
    command
        .env("GH_PROMPT_DISABLED", "1")
        .env("NO_COLOR", "1")
        .env("GH_NO_UPDATE_NOTIFIER", "1")
        .env("CLICOLOR", "0")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    no_console(&mut command);
    command
}

fn run_gh(gh: &Path, args: &[&str], token: Option<&str>) -> Result<String, GhError> {
    let fail = |e: std::io::Error| GhError(detail(&e.to_string()));
    let mut command = child_command(gh);
    command.args(args);
    if let Some(token) = token {
        command.env("GH_TOKEN", token);
    }
    let mut child = command.spawn().map_err(fail)?;
    drop(command);
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| GhError("missing gh stdout".into()))?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| GhError("missing gh stderr".into()))?;
    let (out_tx, out_rx) = mpsc::sync_channel(1);
    let (err_tx, err_rx) = mpsc::sync_channel(1);
    std::thread::spawn(move || {
        let _ = out_tx.send(read_pipe(stdout, MAX_STDOUT));
    });
    std::thread::spawn(move || {
        let _ = err_tx.send(read_pipe(stderr, MAX_DIAGNOSTICS));
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
                    _ => GhError("gh timed out".into()),
                });
            }
        }
    };
    let status = status?;
    // Credential helpers may inherit a pipe: never wait beyond the same deadline.
    let receive = |rx: mpsc::Receiver<std::io::Result<Vec<u8>>>| {
        rx.recv_timeout(GH_TIMEOUT.saturating_sub(start.elapsed()))
            .map_err(|_| GhError("gh output timed out".into()))?
            .map_err(fail)
    };
    let output = receive(out_rx)?;
    let errors = receive(err_rx)?;
    if output.len() as u64 > MAX_STDOUT {
        return Err(GhError("gh response too large".into()));
    }
    if !status.success() {
        // Credential-bearing commands must never surface child diagnostics.
        let message = if token.is_some() {
            "GitHub request failed".into()
        } else if args.starts_with(&["auth", "token"]) {
            "gh returned no usable token".into()
        } else {
            detail(&String::from_utf8_lossy(&errors))
        };
        return Err(GhError(message));
    }
    String::from_utf8(output).map_err(|_| GhError("unexpected response from gh".into()))
}
fn fetch_summary(gh: &Path, login: &str) -> Result<GithubSummary, GhError> {
    if !is_valid_login(login) {
        return Err(GhError("invalid GitHub login".into()));
    }
    let token = run_gh(
        gh,
        &["auth", "token", "--hostname", "github.com", "--user", login],
        None,
    )
    .map_err(|_| GhError("gh returned no usable token".into()))?;
    let token = token.trim();
    if !is_valid_token(token) {
        return Err(GhError("gh returned no usable token".into()));
    }
    let response = run_gh(
        gh,
        &[
            "api",
            "graphql",
            "--hostname",
            "github.com",
            "-f",
            &format!("query={QUERY}"),
        ],
        Some(token),
    )?;
    let mut summary = parse_summary(&response, crate::state::now_ms())
        .filter(|s| s.login.eq_ignore_ascii_case(login))
        .ok_or_else(|| GhError("unexpected response from gh".into()))?;
    add_notices(gh, token, &mut summary);
    Ok(summary)
}

/// What the notices remember between checks (#131).
struct Memory {
    plans: PlanCache,
    /// Each account's failed actions, kept between reads and when one fails.
    runs: BTreeMap<String, Vec<FailedRun>>,
    /// When each account may read its notifications again: GitHub's
    /// `X-Poll-Interval`, which "Refresh now" does not skip either.
    runs_due: BTreeMap<String, u64>,
}

static MEMORY: Mutex<Memory> = Mutex::new(Memory {
    plans: PlanCache::new(),
    runs: BTreeMap::new(),
    runs_due: BTreeMap::new(),
});

/// The failed actions and plans behind the island's notices (#131). A request
/// that fails costs only its own notices, never the summary.
fn add_notices(gh: &Path, token: &str, summary: &mut GithubSummary) {
    let now = crate::state::now_ms();
    let login = summary.login.to_ascii_lowercase();
    let path = notifications_path(now);
    let due = MEMORY
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .runs_due
        .get(&login)
        .is_none_or(|&at| now >= at);
    let runs = due.then(|| {
        run_gh(
            gh,
            &["api", "--include", "--hostname", "github.com", &path],
            Some(token),
        )
    });
    let mut memory = MEMORY.lock().unwrap_or_else(|e| e.into_inner());
    let read = match runs {
        None => None,
        Some(Ok(text)) => {
            let answer = split_poll_interval(&text);
            if answer.is_none() {
                eprintln!("cortex: failed actions not read: unexpected response from gh");
            }
            answer.map(|(secs, body)| (secs, parse_failed_runs(body)))
        }
        Some(Err(GhError(err))) => {
            eprintln!("cortex: failed actions not read: {err}");
            None
        }
    };
    if let Some((secs, runs)) = read {
        memory.runs.insert(login.clone(), runs);
        memory
            .runs_due
            .insert(login.clone(), now.saturating_add(secs * 1000));
    } else if due {
        // A failed read (a 429, say) waits at least as long as a good one.
        memory
            .runs_due
            .insert(login.clone(), now.saturating_add(POLL_SLOW_SECS * 1000));
    }
    summary.failed_runs = memory.runs.get(&login).cloned().unwrap_or_default();
    // Only pull requests with an Atlantis run past its plan, changed since
    // their comments were read (#161).
    let due = memory
        .plans
        .due(&login, summary.mine.iter().chain(&summary.review_requests));
    if !due.is_empty() {
        let mut args = vec![
            "api".to_owned(),
            "graphql".into(),
            "--hostname".into(),
            "github.com".into(),
            "-f".into(),
            format!("query={}", plan_query(due.len())),
        ];
        for (i, (url, _, _)) in due.iter().enumerate() {
            args.extend(["-f".to_owned(), format!("u{i}={url}")]);
        }
        let args: Vec<&str> = args.iter().map(String::as_str).collect();
        let read = run_gh(gh, &args, Some(token))
            .ok()
            .and_then(|text| parse_plans(&text, due.len()));
        if read.is_none() {
            eprintln!("cortex: pull request plans not read");
        }
        for (i, (url, updated_at, planned_at)) in due.iter().enumerate() {
            let outcome = read.as_ref().map(|details| details[i].clone());
            memory
                .plans
                .record(&login, url, updated_at, planned_at, outcome, now);
        }
    }
    // Merged ones too: an apply merges its pull request at once, and its
    // summary keeps the plan it applied (no new read for them).
    for pull in summary
        .mine
        .iter_mut()
        .chain(summary.review_requests.iter_mut())
        .chain(summary.merged.iter_mut())
    {
        if let Some(plan) = pull.plan.take() {
            let detail = memory.plans.detail(&login, &pull.url, &plan.planned_at);
            pull.plan = Some(with_detail(plan, detail));
        }
    }
    drop(memory);
    summary.notices = notices(summary);
}

fn fetch_account(gh: &Path, account: GhAccount, hidden: &[String]) -> AccountStatus {
    let shown = account_is_shown(hidden, &account.login);
    let (state, message, summary) = if !shown {
        ("hidden", None, None)
    } else if !account.valid {
        ("signed_out", Some("sign in again".into()), None)
    } else {
        match fetch_summary(gh, &account.login) {
            Ok(summary) => ("ok", None, Some(summary)),
            Err(GhError(message)) => ("error", Some(message), None),
        }
    };
    AccountStatus {
        login: account.login,
        active: account.active,
        shown,
        state,
        message,
        summary,
        new_since_last_run: Vec::new(),
    }
}

enum PollResult {
    Missing(Option<&'static str>),
    Accounts(Vec<AccountStatus>),
}

fn poll(hidden: &[String], current: impl Fn() -> bool) -> Result<PollResult, GhError> {
    let Some(gh) = find_gh() else {
        return Ok(PollResult::Missing(installer().map(|(name, _)| name)));
    };
    let text = run_gh(&gh, AUTH_STATUS, None)?;
    let accounts =
        parse_accounts(&text).ok_or_else(|| GhError("unexpected response from gh".into()))?;
    let mut results = Vec::with_capacity(accounts.len());
    for account in accounts {
        if !current() {
            break;
        }
        results.push(fetch_account(&gh, account, hidden));
    }
    Ok(PollResult::Accounts(results))
}

#[tauri::command]
pub fn github_status(state: State<'_, GithubState>) -> GithubStatus {
    state.lock().status.clone()
}

#[tauri::command]
pub fn github_set_enabled(
    app: AppHandle,
    enabled: bool,
    state: State<'_, GithubState>,
) -> Result<(), String> {
    let mut inner = state.lock();
    inner.prefs = prefs::update(|p| p.github = enabled)?;
    inner.revision += 1;
    inner.status.enabled = enabled;
    inner.status.state = if enabled { "checking" } else { "off" };
    inner.status.message = None;
    if !enabled {
        inner.cancel();
        inner.status.accounts.clear();
        inner.status.installer = None;
    }
    inner.emit(&app);
    state.wake.notify_one();
    Ok(())
}

#[tauri::command]
pub fn github_set_account_shown(
    app: AppHandle,
    login: String,
    shown: bool,
    state: State<'_, GithubState>,
) -> Result<(), String> {
    if !is_valid_login(&login) {
        return Err("invalid GitHub login".into());
    }
    let mut inner = state.lock();
    let mut prefs = inner.prefs.clone();
    if shown {
        prefs
            .github_hidden
            .retain(|name| !name.eq_ignore_ascii_case(&login));
    } else if account_is_shown(&prefs.github_hidden, &login) {
        if prefs.github_hidden.len() >= MAX_ACCOUNTS {
            return Err("too many hidden GitHub accounts".into());
        }
        prefs.github_hidden.push(login.clone());
    }
    inner.prefs = prefs::update(|p| p.github_hidden = prefs.github_hidden)?;
    inner.revision += 1;
    if let Some(account) = inner
        .status
        .accounts
        .iter_mut()
        .find(|a| a.login.eq_ignore_ascii_case(&login))
    {
        account.shown = shown;
        if !shown {
            account.state = "hidden";
            account.message = None;
            account.summary = None;
        } else if account.state == "hidden" {
            // The next poll supplies the current authentication and fetch state.
            account.state = "error";
        }
    }
    inner.emit(&app);
    state.wake.notify_one();
    Ok(())
}

#[tauri::command]
pub fn github_refresh(state: State<'_, GithubState>) {
    state.refresh();
}

#[tauri::command]
pub fn open_url(url: String, app: AppHandle, state: State<'_, GithubState>) -> Result<(), String> {
    if url != DEVICE_URL
        && url != CLI_URL
        && url != release_notes_url(&app.package_info().version.to_string())
        && !state
            .lock()
            .status
            .accounts
            .iter()
            .any(|a| a.summary.as_ref().is_some_and(|s| s.has_url(&url)))
    {
        return Err("URL is not in the current GitHub summary".into());
    }
    open_in_browser(&url)
}

fn open_in_browser(url: &str) -> Result<(), String> {
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
type TaskOutput = Option<(Instant, String)>;

fn task_output(
    pipe: impl Read,
    mut codes: Option<mpsc::SyncSender<String>>,
) -> std::io::Result<TaskOutput> {
    let mut pipe = BufReader::new(pipe);
    let sign_in = codes.is_some();
    let mut remaining = MAX_DIAGNOSTICS as usize;
    let mut diagnostic = None;
    let mut line = Vec::new();
    let mut record = |line: &[u8]| {
        let text = String::from_utf8_lossy(line);
        if let Some(code) = codes.as_ref().and_then(|_| device_code(&text)) {
            if let Some(tx) = codes.take() {
                let _ = tx.send(code);
            }
        }
        if !text.trim().is_empty() && (!sign_in || diagnostic.is_none()) {
            diagnostic = Some((Instant::now(), detail(&text)));
        }
    };
    loop {
        let bytes = pipe.fill_buf()?;
        if bytes.is_empty() {
            record(&line);
            break;
        }
        let end = bytes.iter().position(|&b| b == b'\n');
        let len = end.map_or(bytes.len(), |i| i + 1);
        let capacity = MAX_DIAGNOSTICS as usize - line.len();
        let count = len.min(capacity).min(remaining);
        line.extend_from_slice(&bytes[..count]);
        pipe.consume(len);
        if sign_in {
            remaining -= count;
        }
        if (end.is_some() || remaining == 0) && !line.is_empty() {
            record(&line);
            line.clear();
        }
    }
    Ok(diagnostic)
}

enum Operation {
    Install,
    SignIn,
}

fn run_task(
    app: &AppHandle,
    mut command: Command,
    operation: Operation,
    cancel: &AtomicBool,
) -> Result<bool, String> {
    let (timeout, timeout_message) = match operation {
        Operation::Install => (Duration::from_secs(15 * 60), "installation timed out"),
        Operation::SignIn => (Duration::from_secs(16 * 60), "sign-in timed out"),
    };
    let sign_in = matches!(operation, Operation::SignIn);
    if cancel.load(Ordering::Relaxed) {
        return Ok(false);
    }
    let mut child = command.spawn().map_err(|e| detail(&e.to_string()))?;
    let (code_tx, code_rx) = mpsc::sync_channel(1);
    let (out_tx, out_rx) = mpsc::sync_channel(1);
    let (err_tx, err_rx) = mpsc::sync_channel(1);
    if let Some(stdout) = child.stdout.take() {
        std::thread::spawn(move || {
            let _ = out_tx.send(task_output(stdout, None));
        });
    }
    if let Some(stderr) = child.stderr.take() {
        std::thread::spawn(move || {
            let _ = err_tx.send(task_output(stderr, sign_in.then_some(code_tx)));
        });
    }
    let state = app.state::<GithubState>();
    let start = Instant::now();
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if !cancel.load(Ordering::Relaxed) && start.elapsed() < timeout => {
                if let Ok(code) = code_rx.try_recv() {
                    let open = {
                        let mut inner = state.lock();
                        if cancel.load(Ordering::Relaxed) {
                            false
                        } else {
                            inner.status.task = Task::SigningIn { code: Some(code) };
                            inner.emit(app);
                            true
                        }
                    };
                    if open {
                        let _ = open_in_browser(DEVICE_URL);
                    }
                }
                std::thread::sleep(Duration::from_millis(100));
            }
            result => {
                let _ = child.kill();
                let status = child.wait();
                return match result {
                    Err(e) => Err(detail(&e.to_string())),
                    _ if status.is_ok_and(|status| status.success()) => Ok(true),
                    _ if cancel.load(Ordering::Relaxed) => Ok(false),
                    _ => Err(timeout_message.into()),
                };
            }
        }
    };
    if status.success() {
        return Ok(true);
    }
    if cancel.load(Ordering::Relaxed) {
        return Ok(false);
    }
    // An installer descendant can inherit a pipe after the command exits.
    let deadline = (start + timeout).min(Instant::now() + Duration::from_millis(500));
    let last = |rx: mpsc::Receiver<std::io::Result<TaskOutput>>| {
        rx.recv_timeout(deadline.saturating_duration_since(Instant::now()))
            .ok()?
            .ok()?
    };
    let stderr = last(err_rx);
    let output = if sign_in {
        stderr
    } else {
        stderr
            .into_iter()
            .chain(last(out_rx))
            .max_by_key(|(at, _)| *at)
    };
    Err(output.map_or_else(
        || {
            if sign_in {
                "GitHub sign-in failed".into()
            } else {
                "GitHub CLI installation failed".into()
            }
        },
        |(_, line)| line,
    ))
}

fn sign_in(
    app: &AppHandle,
    gh: &Path,
    previous: Option<String>,
    cancel: &AtomicBool,
) -> Result<(), String> {
    // Resolve the actual active account even before the initial poll, or when
    // the active account is outside the capped list shown in Settings.
    if cancel.load(Ordering::Relaxed) {
        return Ok(());
    }
    let text = run_gh(gh, AUTH_STATUS, None).map_err(|GhError(message)| message)?;
    let active = active_account(&text).ok_or("unexpected response from gh")?;
    let previous = match (active, previous) {
        (Some(active), Some(previous)) if active.eq_ignore_ascii_case(&previous) => Some(previous),
        (active, _) => active,
    };
    let mut command = child_command(gh);
    command.args([
        "auth",
        "login",
        "--web",
        "--hostname",
        "github.com",
        "--git-protocol",
        "https",
        "--skip-ssh-key",
    ]);
    let result = run_task(app, command, Operation::SignIn, cancel);
    let succeeded = matches!(result, Ok(true));
    if let Some(previous) = previous {
        let restore = || -> Result<(), GhError> {
            let auth = run_gh(gh, AUTH_STATUS, None)?;
            let exists = has_account(&auth, &previous)
                .ok_or_else(|| GhError("unexpected response from gh".into()))?;
            // A canceled or timed-out process can have already saved the new account.
            let active_changed = !succeeded
                && active_account(&auth)
                    .flatten()
                    .is_some_and(|active| !active.eq_ignore_ascii_case(&previous));
            if exists && (succeeded || active_changed) {
                run_gh(
                    gh,
                    &[
                        "auth",
                        "switch",
                        "--hostname",
                        "github.com",
                        "--user",
                        &previous,
                    ],
                    None,
                )?;
            }
            Ok(())
        };
        let restored = restore();
        if succeeded {
            restored.map_err(|_| "could not restore the previous active GitHub account")?;
        }
    }
    result.map(|_| ())
}

fn start_task(
    app: &AppHandle,
    state: &GithubState,
    operation: Operation,
    work: impl FnOnce(&AppHandle, Option<String>, &AtomicBool) -> Result<(), String> + Send + 'static,
) -> Result<(), String> {
    let mut inner = state.lock();
    if !inner.status.enabled {
        return Err("GitHub integration is off".into());
    }
    if inner.running.is_some() || !matches!(inner.status.task, Task::Idle | Task::Failed { .. }) {
        return Err("a GitHub task is already running".into());
    }
    if matches!(operation, Operation::Install) && inner.status.installer.is_none() {
        return Err("GitHub CLI installer is unavailable".into());
    }
    let previous = inner
        .status
        .accounts
        .iter()
        .find(|a| a.active && is_valid_login(&a.login))
        .map(|a| a.login.clone());
    let cancel = Arc::new(AtomicBool::new(false));
    let app_handle = app.clone();
    std::thread::Builder::new()
        .spawn({
            let cancel = Arc::clone(&cancel);
            move || {
                let state = app_handle.state::<GithubState>();
                // Wait for the initial task state to be published.
                drop(state.lock());
                let result = work(&app_handle, previous, &cancel);
                let mut inner = state.lock();
                inner.running = None;
                if !cancel.load(Ordering::Relaxed) {
                    inner.status.task = match result {
                        Ok(()) => Task::Idle,
                        Err(message) => Task::Failed { message },
                    };
                    inner.emit(&app_handle);
                }
                state.wake.notify_one();
            }
        })
        .map_err(|e| detail(&e.to_string()))?;
    inner.running = Some(cancel);
    inner.status.task = match operation {
        Operation::SignIn => Task::SigningIn { code: None },
        Operation::Install => Task::Installing,
    };
    inner.revision += 1;
    inner.emit(app);
    Ok(())
}

#[tauri::command]
pub fn github_sign_in(app: AppHandle, state: State<'_, GithubState>) -> Result<(), String> {
    let gh = find_gh().ok_or("GitHub CLI not found")?;
    start_task(
        &app,
        &state,
        Operation::SignIn,
        move |app, previous, cancel| sign_in(app, &gh, previous, cancel),
    )
}

#[tauri::command]
pub fn github_install(app: AppHandle, state: State<'_, GithubState>) -> Result<(), String> {
    if find_gh().is_some() {
        return Err("GitHub CLI is already installed".into());
    }
    let (name, path) = installer().ok_or("GitHub CLI installer is unavailable")?;
    start_task(&app, &state, Operation::Install, move |app, _, cancel| {
        let mut command = child_command(&path);
        if name == "brew" {
            command
                .args(["install", "gh"])
                .env("HOMEBREW_NO_ENV_HINTS", "1");
        } else {
            command.args([
                "install",
                "--id",
                "GitHub.cli",
                "--exact",
                "--source",
                "winget",
                "--silent",
                "--accept-package-agreements",
                "--accept-source-agreements",
                "--disable-interactivity",
            ]);
        }
        run_task(app, command, Operation::Install, cancel)?;
        Ok(())
    })
}

#[tauri::command]
pub fn github_cancel_task(app: AppHandle, state: State<'_, GithubState>) -> Result<(), String> {
    let mut inner = state.lock();
    inner.cancel();
    inner.emit(&app);
    Ok(())
}

/// Where the notice keys of the last check are kept between launches (#157).
const KNOWN_FILE: &str = "github-notices.json";

/// The notice keys the last run saw; `None` without a readable file.
fn load_known() -> Option<Known> {
    let path = crate::hooks::data_dir().ok()?.join(KNOWN_FILE);
    match crate::files::read_bounded(&path, MAX_KNOWN_BYTES) {
        Ok(Some(text)) => known_from_json(&text),
        _ => None,
    }
}

fn save_known(keys: &Known) -> Result<(), String> {
    crate::files::write_private(
        &crate::hooks::data_dir()?.join(KNOWN_FILE),
        known_to_json(keys).as_bytes(),
    )
    .map_err(|e| e.to_string())
}

pub async fn poller(app: AppHandle) {
    let state = app.state::<GithubState>();
    // What the last run saw: the first answers after launch name what came
    // while Cortex was closed (#157). Not once the user changed anything
    // (turning GitHub on, showing an account), which is no coming back.
    let launch_revision = state.lock().revision;
    let loaded = if state.enabled() { load_known() } else { None };
    let mut saved = loaded.clone().unwrap_or_default();
    let mut last_run = LastRun::new(loaded);
    loop {
        let request = {
            let inner = state.lock();
            // No "checking" here: turning on and launching already show it, and
            // every 15 s the dot and the account count would blink.
            if inner.status.enabled && inner.running.is_none() {
                Some((inner.revision, inner.prefs.github_hidden.clone()))
            } else {
                None
            }
        };
        let Some((revision, hidden)) = request else {
            state.wake.notified().await;
            continue;
        };
        let handle = app.clone();
        let asked_ms = crate::state::now_ms();
        let result = tauri::async_runtime::spawn_blocking(move || {
            poll(&hidden, || {
                handle.state::<GithubState>().lock().revision == revision
            })
        })
        .await
        .unwrap_or_else(|_| Err(GhError("gh worker failed".into())));
        let mut seen_now = None;
        let applied = {
            let mut inner = state.lock();
            if inner.revision == revision {
                let status = &mut inner.status;
                status.installer = None;
                match result {
                    Ok(PollResult::Accounts(mut accounts)) => {
                        status.message = None;
                        for account in accounts
                            .iter_mut()
                            .filter(|a| a.shown && a.summary.is_none())
                        {
                            account.summary = status
                                .accounts
                                .iter_mut()
                                .find(|old| old.login.eq_ignore_ascii_case(&account.login))
                                .and_then(|old| old.summary.take());
                        }
                        if revision != launch_revision {
                            last_run.forget();
                        }
                        for account in &mut accounts {
                            if let Some(summary) = &account.summary {
                                account.new_since_last_run = last_run.news(
                                    &account.login,
                                    account.state == "ok",
                                    &summary.notices,
                                );
                            }
                        }
                        if accounts.iter().any(|a| a.state == "ok") {
                            state.answered_ms.store(asked_ms, Ordering::Relaxed);
                        }
                        status.state = overall_state(accounts.iter().map(|a| a.state));
                        status.accounts = accounts;
                        if status.state == "error" {
                            status.message = Some("GitHub requests failed".into());
                        }
                        let mut known = saved.clone();
                        remember(
                            &mut known,
                            status
                                .accounts
                                .iter()
                                .filter(|a| a.shown)
                                .filter_map(|a| Some((a.login.as_str(), a.summary.as_ref()?))),
                        );
                        seen_now = Some(known);
                    }
                    Ok(PollResult::Missing(installer)) => {
                        status.state = "no_gh";
                        status.message = Some("GitHub CLI (gh) not found".into());
                        status.accounts.clear();
                        status.installer = installer;
                    }
                    Err(GhError(message)) => {
                        status.state = "error";
                        status.message = Some(message);
                    }
                }
                inner.emit(&app);
                true
            } else {
                false
            }
        };
        if !applied {
            state.wake.notified().await;
            continue;
        }
        if let Some(known) = seen_now.filter(|known| *known != saved) {
            let write = known.clone();
            match tauri::async_runtime::spawn_blocking(move || save_known(&write)).await {
                Ok(Ok(())) => saved = known,
                Ok(Err(err)) => {
                    eprintln!("cortex: GitHub notices not kept for the next launch: {err}")
                }
                Err(_) => eprintln!("cortex: GitHub notices not kept for the next launch"),
            }
        }
        // Woken every POLL_SECS to look at the lock: unlocking is seen within 15 s.
        let failing = state.lock().status.state == "error";
        let mut slept = 0;
        loop {
            tokio::select! {
                _ = tokio::time::sleep(Duration::from_secs(POLL_SECS)) => {}
                _ = state.wake.notified() => break,
            }
            slept += POLL_SECS;
            if poll_due(slept, crate::gaze::screen_locked(), failing) {
                break;
            }
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
    fn sign_in_reads_the_first_device_code_and_caps_stderr() {
        let (tx, rx) = mpsc::sync_channel(1);
        let stderr = b"\n! First copy your one-time code: ABCD-1234\nEFGH-5678\n";
        let output = task_output(&stderr[..], Some(tx)).unwrap().unwrap();
        assert_eq!(rx.try_recv().unwrap(), "ABCD-1234");
        assert_eq!(output.1, "! First copy your one-time code: ABCD-1234");

        let (tx, rx) = mpsc::sync_channel(1);
        let stderr = format!("{}\nABCD-1234", "x".repeat(MAX_DIAGNOSTICS as usize));
        task_output(stderr.as_bytes(), Some(tx)).unwrap();
        assert!(rx.try_recv().is_err());
    }

    #[test]
    fn installer_diagnostics_keep_the_last_nonempty_line() {
        let output = task_output(&b"progress\nfailed\n  \n"[..], None)
            .unwrap()
            .unwrap();
        assert_eq!(output.1, "failed");
    }

    #[test]
    fn initial_state_has_no_accounts() {
        let s = GithubState::new(Prefs::default());
        let s = s.inner.lock().unwrap();
        assert!(!s.status.enabled);
        assert_eq!(s.status.state, "off");
        assert!(s.status.accounts.is_empty());
    }
}
