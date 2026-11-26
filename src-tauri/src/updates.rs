//! Opt-in, signed self-update (ADR 0004). All of it runs here in Rust: the
//! web views get no updater permission, only these commands.
use crate::{prefs, state::AppState};
use serde::Serialize;
use std::{
    io::Read,
    path::{Path, PathBuf},
    sync::{Mutex, MutexGuard},
    time::Duration,
};
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_updater::{Update, UpdaterExt};
use tokio::sync::Notify;

pub const UPDATES_EVENT: &str = "cortex://updates";
// shortcut: every minute while the maintainer tests releases (24/11/2026);
// back to 6 hours when the testing phase ends (#158).
const CHECK_EVERY: Duration = Duration::from_secs(60);
/// A package that failed to download or verify waits the old pace: a broken
/// release must not be downloaded every minute (82 MB for the AppImage).
const CHECK_AFTER_BAD_PACKAGE: Duration = Duration::from_secs(6 * 60 * 60);
/// A download that stalls must not hold the checker (and its buffer) forever.
const DOWNLOAD_TIMEOUT: Duration = Duration::from_secs(15 * 60);
/// The largest package Cortex takes: the Linux AppImage is ~82 MB (#72).
const MAX_PACKAGE_BYTES: u64 = 256 * 1024 * 1024;
/// The verified package waits for the click here, in the data folder, not
/// in memory: days of an 82 MB buffer otherwise (#72).
const PACKAGE_FILE: &str = "update-package";

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
    /// The update found and its verified package, saved to disk (#72).
    ready: Option<(Update, PathBuf)>,
    /// Bumped on every toggle, so a check started before it is discarded.
    revision: u64,
    /// The user clicked **Update and restart**: only then may the update
    /// screen install (the welcome screen shares its window and rights).
    armed: bool,
    /// An install runs: nothing checks, installs or restarts meanwhile.
    installing: bool,
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
                armed: false,
                installing: false,
            }),
            wake: Notify::new(),
        }
    }

    fn lock(&self) -> MutexGuard<'_, Inner> {
        self.inner.lock().expect("updates mutex poisoned")
    }

    /// The update screen closed: installing needs a new click.
    pub fn disarm(&self) {
        self.lock().armed = false;
    }

    /// A restart now could leave the app half replaced. Asked by the macOS
    /// island recovery (#115).
    #[cfg_attr(not(target_os = "macos"), allow(dead_code))]
    pub fn installing(&self) -> bool {
        self.lock().installing
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
    forget_package();
    inner.armed = false;
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
    {
        let mut inner = state.lock();
        // Its screen is up already.
        if inner.installing {
            return Ok(());
        }
        if inner.ready.is_none() {
            return Err("No update is ready.".into());
        }
        inner.armed = true;
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
    let (update, path, revision) = {
        let mut inner = state.lock();
        if !inner.armed || inner.installing {
            return Err("No update is ready.".into());
        }
        let Some((update, path)) = inner.ready.take() else {
            return Err("No update is ready.".into());
        };
        inner.installing = true;
        (update, path, inner.revision)
    };
    // `install` checks the signature again before it touches the app: a
    // package changed on disk since the download is refused. It unpacks for
    // a while: off the main thread, so the screen keeps moving.
    let (update, path, result) = tauri::async_runtime::spawn_blocking(move || {
        let result = read_package_at(&path, MAX_PACKAGE_BYTES)
            .map_err(|e| e.to_string())
            .and_then(|bytes| update.install(&bytes).map_err(|e| e.to_string()));
        (update, path, result)
    })
    .await
    .map_err(|e| format!("Update failed: {e}"))?;
    if let Err(err) = result {
        let message = format!("Update failed: {err}");
        eprintln!("cortex: {message}");
        // The updater hid every window just before starting the installer,
        // which did not start (a declined admin prompt).
        #[cfg(windows)]
        for window in app.webview_windows().values() {
            let _ = window.show();
        }
        let mut inner = state.lock();
        inner.installing = false;
        if inner.revision == revision && inner.ready.is_none() {
            inner.ready = Some((update, path));
        }
        return Err(message);
    }
    // Windows never gets here (its installer exits the app): the next
    // launch removes the package.
    let _ = std::fs::remove_file(&path);
    crate::server::release_endpoint();
    app.restart()
}

/// Checks at launch and every `CHECK_EVERY` while on; a found update is
/// downloaded and verified right away, then waits for the user's click.
pub async fn checker(app: AppHandle) {
    let state = app.state::<UpdateState>();
    // A package left by an earlier run is never installed: it is found again.
    forget_package();
    loop {
        let mut bad_package = false;
        let due = {
            let mut inner = state.lock();
            let due = inner.status.enabled == Some(true)
                && !inner.installing
                && matches!(inner.status.state, "idle" | "error");
            // From "up to date" the check is quiet: every minute, Settings
            // would blink "Checking…".
            let quiet = inner.status.state == "idle";
            if due {
                inner.status.state = "checking";
                inner.status.version = None;
                inner.status.message = None;
                if !quiet {
                    inner.emit(&app);
                }
            }
            due.then_some((inner.revision, quiet))
        };
        if let Some((revision, quiet)) = due {
            let result = fetch(&app, revision).await;
            let mut inner = state.lock();
            if inner.revision == revision {
                // `fetch` reports "downloading" once a newer version was found.
                bad_package = result.is_err() && inner.status.state == "downloading";
                let unchanged = quiet && matches!(result, Ok(None));
                match result {
                    Ok(Some((update, path))) => {
                        inner.status.state = "ready";
                        inner.status.version = Some(update.version.clone());
                        inner.ready = Some((update, path));
                    }
                    Ok(None) => inner.status.state = "idle",
                    Err(message) => {
                        eprintln!("cortex: {message}");
                        inner.status.state = "error";
                        inner.status.version = None;
                        inner.status.message = Some(message);
                    }
                }
                if !unchanged {
                    inner.emit(&app);
                }
            }
        }
        let wait = if bad_package {
            CHECK_AFTER_BAD_PACKAGE
        } else {
            CHECK_EVERY
        };
        let _ = tokio::time::timeout(wait, state.wake.notified()).await;
    }
}

/// The manifest request, then the package. `download` rejects a package
/// whose signature does not match the public key compiled into the app; the
/// verified package is saved to disk, and only its path is kept (#72).
async fn fetch(app: &AppHandle, revision: u64) -> Result<Option<(Update, PathBuf)>, String> {
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
    // A package past the limit, announced or received, stops the download:
    // dropping it ends the transfer.
    let too_large = Notify::new();
    let mut received = 0u64;
    let download = update.download(
        |chunk, announced| {
            received = received.saturating_add(chunk as u64);
            if too_large_package(received, announced) {
                too_large.notify_one();
            }
        },
        || {},
    );
    let bytes = tokio::select! {
        biased;
        () = too_large.notified() => return Err(format!("Update {} is too large", update.version)),
        result = tokio::time::timeout(DOWNLOAD_TIMEOUT, download) => result
            .map_err(|_| format!("Update {} took too long to download", update.version))?
            .map_err(|e| format!("Update {} rejected: {e}", update.version))?,
    };
    let path = package_path()?;
    let saved = path.clone();
    tauri::async_runtime::spawn_blocking(move || crate::files::write_private(&saved, &bytes))
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e| format!("Update {} not saved: {e}", update.version))?;
    Ok(Some((update, path)))
}

fn package_path() -> Result<PathBuf, String> {
    Ok(crate::hooks::data_dir()?.join(PACKAGE_FILE))
}

/// The saved package goes: after an install, at launch, and when the user
/// turns updates on or off. Only a package found by this run is installed.
fn forget_package() {
    if let Ok(path) = package_path() {
        let _ = std::fs::remove_file(path);
    }
}

fn too_large_package(received: u64, announced: Option<u64>) -> bool {
    received > MAX_PACKAGE_BYTES || announced.is_some_and(|n| n > MAX_PACKAGE_BYTES)
}

/// The saved package, whole, or an error: a file grown past the limit since
/// is never read into memory.
fn read_package_at(path: &Path, max_bytes: u64) -> std::io::Result<Vec<u8>> {
    let mut bytes = Vec::new();
    std::fs::File::open(path)?
        .take(max_bytes.saturating_add(1))
        .read_to_end(&mut bytes)?;
    if bytes.len() as u64 > max_bytes {
        return Err(std::io::Error::other("the saved update is too large"));
    }
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp(label: &str) -> PathBuf {
        std::env::temp_dir()
            .join(format!("cortex-update-{label}-{}", std::process::id()))
            .join(PACKAGE_FILE)
    }

    #[test]
    fn a_saved_package_is_private_and_read_back_whole() {
        let path = temp("save");
        let package = vec![7u8; 300_000];
        crate::files::write_private(&path, &package).unwrap();
        assert_eq!(read_package_at(&path, MAX_PACKAGE_BYTES).unwrap(), package);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(&path).unwrap().permissions().mode();
            assert_eq!(mode & 0o777, 0o600);
        }
        std::fs::remove_dir_all(path.parent().unwrap()).unwrap();
    }

    #[test]
    fn a_package_past_the_limit_is_never_read_or_downloaded_whole() {
        let path = temp("large");
        crate::files::write_private(&path, &[1u8; 11]).unwrap();
        assert!(read_package_at(&path, 10).is_err());
        assert_eq!(read_package_at(&path, 11).unwrap().len(), 11);
        assert!(read_package_at(&path.with_file_name("missing"), 10).is_err());
        std::fs::remove_dir_all(path.parent().unwrap()).unwrap();
        assert!(!too_large_package(MAX_PACKAGE_BYTES, None));
        assert!(too_large_package(MAX_PACKAGE_BYTES + 1, None));
        assert!(too_large_package(0, Some(MAX_PACKAGE_BYTES + 1)));
        assert!(!too_large_package(10, Some(82 * 1024 * 1024)));
    }
}
