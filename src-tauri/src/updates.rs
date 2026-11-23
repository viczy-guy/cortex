//! Opt-in, signed self-update (ADR 0004). All of it runs here in Rust: the
//! web views get no updater permission, only these commands.
use crate::{prefs, state::AppState};
use serde::Serialize;
use std::{
    sync::{Mutex, MutexGuard},
    time::Duration,
};
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_updater::{Update, UpdaterExt};
use tokio::sync::Notify;

pub const UPDATES_EVENT: &str = "cortex://updates";
const CHECK_EVERY: Duration = Duration::from_secs(6 * 60 * 60);
/// A download that stalls must not hold the checker (and its buffer) forever.
const DOWNLOAD_TIMEOUT: Duration = Duration::from_secs(15 * 60);

#[derive(Clone, Serialize)]
pub struct UpdateStatus {
    /// `None` until the user answers the first-run question.
    pub enabled: Option<bool>,
    /// "unsupported" | "off" | "idle" | "checking" | "downloading" | "ready" | "error"
    pub state: &'static str,
    /// The new version, while it downloads and once it is ready.
    pub version: Option<String>,
    pub message: Option<String>,
}

struct Inner {
    status: UpdateStatus,
    // shortcut: the verified package stays in RAM until installed (the Linux
    // AppImage is the largest, ~80 MB); spill it to a temp file if that hurts.
    ready: Option<(Update, Vec<u8>)>,
    /// Bumped on every toggle, so a check started before it is discarded.
    revision: u64,
}

impl Inner {
    fn emit(&self, app: &AppHandle) {
        let _ = app.emit(UPDATES_EVENT, &self.status);
    }
}

pub struct UpdateState {
    inner: Mutex<Inner>,
    wake: Notify,
}

/// Only an installed app updates itself: not `tauri dev`, and not a `.deb` or
/// `.rpm`, which belong to the package manager.
fn supported() -> bool {
    use tauri::utils::{config::BundleType, platform::bundle_type};
    if cfg!(target_os = "macos") {
        // macOS binaries are not tagged with their bundle type: require an app bundle.
        std::env::current_exe().is_ok_and(|exe| {
            exe.parent()
                .is_some_and(|dir| dir.ends_with("Contents/MacOS"))
        })
    } else {
        // The bundler tags the binary; `tauri dev` leaves it untagged.
        matches!(
            bundle_type(),
            Some(BundleType::AppImage | BundleType::Msi | BundleType::Nsis)
        )
    }
}

impl UpdateState {
    pub fn new(enabled: Option<bool>) -> Self {
        let state = match (supported(), enabled) {
            (false, _) => "unsupported",
            (true, Some(true)) => "idle",
            (true, _) => "off",
        };
        Self {
            inner: Mutex::new(Inner {
                status: UpdateStatus {
                    enabled,
                    state,
                    version: None,
                    message: None,
                },
                ready: None,
                revision: 0,
            }),
            wake: Notify::new(),
        }
    }

    fn lock(&self) -> MutexGuard<'_, Inner> {
        self.inner.lock().expect("updates mutex poisoned")
    }
}

#[tauri::command]
pub fn updates_status(state: State<'_, UpdateState>) -> UpdateStatus {
    state.lock().status.clone()
}

/// The answer to the first-run question, or the switch in Settings.
#[tauri::command]
pub fn updates_set_enabled(
    app: AppHandle,
    enabled: bool,
    state: State<'_, UpdateState>,
) -> Result<(), String> {
    let mut inner = state.lock();
    prefs::update(|p| p.updates = Some(enabled))?;
    inner.revision += 1;
    inner.ready = None;
    let status = &mut inner.status;
    status.enabled = Some(enabled);
    if status.state != "unsupported" {
        status.state = if enabled { "idle" } else { "off" };
        status.version = None;
        status.message = None;
    }
    inner.emit(&app);
    state.wake.notify_one();
    Ok(())
}

/// Never over a card: restarting would drop the agent's waiting request.
fn no_cards(board: &AppState) -> Result<(), String> {
    if board.snapshot().approvals.is_empty() {
        Ok(())
    } else {
        Err("Answer the waiting cards first.".into())
    }
}

/// The user clicked **Update and restart**: show the update screen, which
/// then installs (#127). Async: a window made in a sync command deadlocks
/// on Windows.
#[tauri::command]
pub async fn updates_start(
    app: AppHandle,
    state: State<'_, UpdateState>,
    board: State<'_, AppState>,
) -> Result<(), String> {
    no_cards(&board)?;
    if state.lock().ready.is_none() {
        return Err("No update is ready.".into());
    }
    crate::open_splash(&app, Some("mode=updating"))
}

/// Install the downloaded update and restart; called by the update screen.
/// A failed install keeps the package, so the screen can try again and the
/// island keeps offering it.
#[tauri::command]
pub async fn updates_install(
    app: AppHandle,
    state: State<'_, UpdateState>,
    board: State<'_, AppState>,
) -> Result<(), String> {
    crate::keep_splash();
    no_cards(&board)?;
    let (update, bytes, revision) = {
        let mut inner = state.lock();
        let Some((update, bytes)) = inner.ready.take() else {
            return Err("No update is ready.".into());
        };
        (update, bytes, inner.revision)
    };
    // Saved first: on Windows the installer ends this process inside `install`.
    let from = app.package_info().version.to_string();
    if let Err(err) = prefs::update(|p| p.updated_from = Some(from)) {
        eprintln!("cortex: {err}");
    }
    // `install` checks the signature again before it touches the app. It
    // unpacks for a while: off the main thread, so the screen keeps moving.
    let (update, bytes, result) = tauri::async_runtime::spawn_blocking(move || {
        let result = update.install(&bytes);
        (update, bytes, result)
    })
    .await
    .map_err(|e| format!("Update failed: {e}"))?;
    if let Err(err) = result {
        let message = format!("Update failed: {err}");
        eprintln!("cortex: {message}");
        if let Err(err) = prefs::update(|p| p.updated_from = None) {
            eprintln!("cortex: {err}");
        }
        let mut inner = state.lock();
        if inner.revision == revision && inner.ready.is_none() {
            inner.ready = Some((update, bytes));
        }
        return Err(message);
    }
    crate::server::release_endpoint();
    app.restart()
}

/// Checks at launch and every `CHECK_EVERY` while on; a found update is
/// downloaded and verified right away, then waits for the user's click.
pub async fn checker(app: AppHandle) {
    let state = app.state::<UpdateState>();
    loop {
        let due = {
            let mut inner = state.lock();
            let due = inner.status.enabled == Some(true)
                && matches!(inner.status.state, "idle" | "error");
            if due {
                inner.status.state = "checking";
                inner.status.version = None;
                inner.status.message = None;
                inner.emit(&app);
            }
            due.then_some(inner.revision)
        };
        if let Some(revision) = due {
            let result = fetch(&app, revision).await;
            let mut inner = state.lock();
            if inner.revision == revision {
                match result {
                    Ok(Some((update, bytes))) => {
                        inner.status.state = "ready";
                        inner.status.version = Some(update.version.clone());
                        inner.ready = Some((update, bytes));
                    }
                    Ok(None) => inner.status.state = "idle",
                    Err(message) => {
                        eprintln!("cortex: {message}");
                        inner.status.state = "error";
                        inner.status.version = None;
                        inner.status.message = Some(message);
                    }
                }
                inner.emit(&app);
            }
        }
        let _ = tokio::time::timeout(CHECK_EVERY, state.wake.notified()).await;
    }
}

/// The manifest request, then the package. `download` rejects a package
/// whose signature does not match the public key compiled into the app.
async fn fetch(app: &AppHandle, revision: u64) -> Result<Option<(Update, Vec<u8>)>, String> {
    let updater = app.updater().map_err(|e| e.to_string())?;
    let Some(update) = updater
        .check()
        .await
        .map_err(|e| format!("Update check failed: {e}"))?
    else {
        return Ok(None);
    };
    {
        let state = app.state::<UpdateState>();
        let mut inner = state.lock();
        if inner.revision != revision {
            return Ok(None);
        }
        inner.status.state = "downloading";
        inner.status.version = Some(update.version.clone());
        inner.emit(app);
    }
    let bytes = tokio::time::timeout(DOWNLOAD_TIMEOUT, update.download(|_, _| {}, || {}))
        .await
        .map_err(|_| format!("Update {} took too long to download", update.version))?
        .map_err(|e| format!("Update {} rejected: {e}", update.version))?;
    Ok(Some((update, bytes)))
}
