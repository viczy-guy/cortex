//! Cortex — the Tauri shell.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod autostart;
mod away;
mod displays;
mod files;
mod gaze;
mod github;
mod hooks;
mod language;
mod prefs;
mod rules;
mod server;
mod state;
mod updates;

#[cfg(target_os = "macos")]
mod macos;

use std::sync::Mutex;

use cortex_core::autostart::OpeningScreen;
use cortex_core::placement::{is_position, island_origin, island_width, Rect};
use cortex_core::prefs::is_screen_name;
use tauri::{AppHandle, Emitter, Manager, State};

use state::{AppState, Snapshot};

/// Current state for a freshly loaded island.
#[tauri::command]
fn snapshot(state: State<'_, AppState>) -> Snapshot {
    state.snapshot()
}

/// The user decided on a pending approval card.
/// `decision`: "allow" | "deny" | "always" | "ask".
#[tauri::command]
fn respond(app: AppHandle, state: State<'_, AppState>, request_id: String, decision: String) {
    if state.resolve(&request_id, &decision) {
        server::emit_state(&app);
        if decision == "always" {
            server::emit_rules(&app);
        }
    }
}

/// The Always rules currently in force.
#[tauri::command]
fn rules_list(state: State<'_, AppState>) -> Vec<cortex_core::AlwaysRule> {
    state.rules()
}

/// Forget one Always rule.
#[tauri::command]
fn rule_remove(
    app: AppHandle,
    state: State<'_, AppState>,
    rule: cortex_core::AlwaysRule,
) -> Result<(), String> {
    let removed = state.remove_rule(&rule);
    server::emit_rules(&app);
    removed.map(|_| ()).map_err(not_saved)
}

/// The rules in memory changed but the file did not.
fn not_saved(error: String) -> String {
    format!("{error}. The removed rules come back when Cortex restarts.")
}

/// Forget every Always rule.
#[tauri::command]
fn rules_clear(app: AppHandle, state: State<'_, AppState>) -> Result<(), String> {
    let cleared = state.clear_rules();
    server::emit_rules(&app);
    cleared.map(|_| ()).map_err(not_saved)
}

/// The user answered a pending question card.
/// `answers`: question text → chosen option label(s).
#[tauri::command]
fn answer(
    app: AppHandle,
    state: State<'_, AppState>,
    request_id: String,
    answers: cortex_core::Answers,
) {
    if state.answer(&request_id, answers) {
        server::emit_state(&app);
    }
}

/// Whether the Claude Code hooks are installed.
#[tauri::command]
fn hooks_status() -> Result<hooks::Status, String> {
    hooks::status()
}

/// The exact change installing the hooks would make. Writes nothing.
#[tauri::command]
fn hooks_preview() -> Result<hooks::Preview, String> {
    hooks::preview()
}

/// Install the hooks the user just reviewed (`token` from `hooks_preview`).
#[tauri::command]
fn hooks_install(app: AppHandle, token: String) -> Result<hooks::Installed, String> {
    let result = hooks::install(&token)?;
    emit_hooks(&app);
    Ok(result)
}

/// The exact change removing the hooks would make. Writes nothing.
#[tauri::command]
fn hooks_uninstall_preview() -> Result<hooks::Preview, String> {
    hooks::preview_uninstall()
}

/// Remove the hooks the user just reviewed (`token` from `hooks_uninstall_preview`).
#[tauri::command]
fn hooks_uninstall(app: AppHandle, token: String) -> Result<hooks::Installed, String> {
    let result = hooks::uninstall(&token)?;
    emit_hooks(&app);
    Ok(result)
}

fn emit_hooks(app: &AppHandle) {
    if let Ok(status) = hooks::status() {
        let _ = app.emit(hooks::HOOKS_EVENT, &status);
    }
}

#[tauri::command]
async fn open_settings(app: AppHandle) -> Result<(), String> {
    show_settings(app, false).await
}

/// Another language (#154): what Rust draws follows; the webviews hear it
/// through the island preferences.
pub fn show_language(app: &AppHandle, lang: &str) {
    if let Some(window) = app.get_webview_window("settings") {
        let _ = window.set_title(language::settings_title(lang));
    }
    #[cfg(target_os = "macos")]
    macos::set_menu_bar_language(app, lang);
}

/// Open (or bring forward) the Settings window; with `start`, a new install's,
/// which shows what to connect first (#152). Async on purpose: creating a
/// window from a sync command deadlocks on Windows.
async fn show_settings(app: AppHandle, start: bool) -> Result<(), String> {
    if let Some(window) = app.get_webview_window("settings") {
        // Opened by hand during the first opening screen: Get started too.
        if start {
            let mut url = window.url().map_err(|e| e.to_string())?;
            url.set_query(Some("start"));
            // Get started is on the Overview, whatever page it was left on.
            url.set_fragment(None);
            window.navigate(url).map_err(|e| e.to_string())?;
        }
        window.unminimize().map_err(|e| e.to_string())?;
        window.show().map_err(|e| e.to_string())?;
        return window.set_focus().map_err(|e| e.to_string());
    }
    // A background (accessory) app cannot bring a window in front of the app
    // the user is working in. Cortex is a regular app — Dock icon, Cmd-Tab —
    // for as long as Settings is open, and goes back to the background after.
    #[cfg(target_os = "macos")]
    let _ = app.set_activation_policy(tauri::ActivationPolicy::Regular);

    let window = tauri::WebviewWindowBuilder::new(
        &app,
        "settings",
        tauri::WebviewUrl::App(
            if start {
                "settings.html?start"
            } else {
                "settings.html"
            }
            .into(),
        ),
    )
    .title(language::settings_title(language::current(&prefs::load())))
    // A 184-point sidebar beside a page about as wide as the old window (#151).
    .inner_size(720.0, 560.0)
    .min_inner_size(620.0, 440.0)
    .resizable(true)
    .center()
    .focused(true)
    .build()
    .map_err(|e| e.to_string())?;

    #[cfg(target_os = "macos")]
    {
        let handle = app.clone();
        window.on_window_event(move |event| {
            if matches!(event, tauri::WindowEvent::Destroyed) {
                let _ = handle.set_activation_policy(tauri::ActivationPolicy::Accessory);
            }
        });
    }

    window.set_focus().map_err(|e| e.to_string())
}

/// Started by a recovery a few minutes ago: no opening screen, and no
/// second recovery, so a WebKit that cannot draw at all never loops.
fn recovered_recently() -> bool {
    cortex_core::recovery::recovered_recently(std::env::args(), state::now_ms() / 1000)
}

/// One check at a time, however often the user opens the app.
#[cfg(target_os = "macos")]
static CHECKING_ISLAND: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// After `delay`, start over if WebKit no longer draws the island (#115).
/// Only refusals count: while the GPU process hangs, a snapshot just waits,
/// and WebKit refuses one only once it gave up on that process. Up to three
/// tries, 15 s apart; two refusals restart. A slow snapshot (a busy system)
/// or one not taken (a hidden island) never does. Waiting cards are dropped
/// by the restart; their agents fall back to asking in the terminal, as when
/// Cortex is not running.
#[cfg(target_os = "macos")]
fn check_island(app: AppHandle, delay: std::time::Duration) {
    use std::sync::atomic::Ordering;
    use std::time::Duration;
    if CHECKING_ISLAND.swap(true, Ordering::AcqRel) {
        return;
    }
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(delay).await;
        let mut refused = 0;
        for attempt in 0..3 {
            if attempt > 0 {
                tokio::time::sleep(Duration::from_secs(15)).await;
            }
            match tokio::time::timeout(Duration::from_secs(5), macos::island_draws(&app)).await {
                // Drawn, or not asked (hidden): nothing to fix.
                Ok(Ok(true)) | Ok(Err(_)) => break,
                Ok(Ok(false)) => refused += 1,
                Err(_) => {}
            }
            if refused == 2 {
                break;
            }
        }
        CHECKING_ISLAND.store(false, Ordering::Release);
        if refused < 2 {
            return;
        }
        if recovered_recently() {
            eprintln!("cortex: the island is still not drawn after a restart");
            return;
        }
        if app.state::<updates::UpdateState>().installing() {
            return;
        }
        eprintln!("cortex: WebKit stopped drawing the island; restarting");
        let handle = app.clone();
        let _ = app.run_on_main_thread(move || {
            let mut env = handle.env();
            let prefix = cortex_core::recovery::RECOVERED_ARG_PREFIX;
            env.args_os
                .retain(|arg| !arg.to_string_lossy().starts_with(prefix));
            env.args_os
                .push(cortex_core::recovery::recovered_arg(state::now_ms() / 1000).into());
            server::release_endpoint();
            handle.cleanup_before_exit();
            tauri::process::restart(&env);
        });
    });
}

/// Open Settings from Rust, outside a command.
fn spawn_open_settings(app: AppHandle, start: bool) {
    tauri::async_runtime::spawn(async move {
        if let Err(err) = show_settings(app, start).await {
            eprintln!("cortex: settings window failed: {err}");
        }
    });
}

#[tauri::command]
fn splash_done(app: AppHandle) {
    app.state::<updates::UpdateState>().disarm();
    close_splash(&app);
}

/// An always-on-top window in the middle of the screen must go even when
/// its page is stuck: close, and destroy if closing fails.
fn close_splash(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("splash") {
        if let Err(err) = window.close() {
            eprintln!("cortex: opening screen did not close: {err}");
            let _ = window.destroy();
        }
    }
    opening_closed(app);
}

/// This launch's opening screen is up: the island waits for it (#152).
static OPENING: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
/// A new install: Settings opens once the opening screen goes (#152).
static FIRST_LAUNCH: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
const OPENING_CLOSED_EVENT: &str = "cortex://opening-closed";

/// Asked by the island before its first drawing.
#[tauri::command]
fn opening_shown() -> bool {
    OPENING.load(std::sync::atomic::Ordering::Acquire)
}

/// The opening screen went, or never came: the island appears, and a new
/// install gets Settings with what to connect first.
fn opening_closed(app: &AppHandle) {
    if OPENING.swap(false, std::sync::atomic::Ordering::AcqRel) {
        let _ = app.emit_to("island", OPENING_CLOSED_EVENT, ());
    }
    if FIRST_LAUNCH.swap(false, std::sync::atomic::Ordering::AcqRel) {
        spawn_open_settings(app.clone(), true);
    }
}

/// Bumped by each screen shown, and by the update screen once its page
/// answers: the 10 s fallback closes only a screen nobody took over.
static SPLASH_TURN: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// The update screen's page is alive and waits on the user: keep it.
fn keep_splash() {
    SPLASH_TURN.fetch_add(1, std::sync::atomic::Ordering::AcqRel);
}

/// The opening screen, or with `query` ("mode=updated", "mode=updating") one
/// of the update screens (#127) in the same window; an open one is reused.
fn open_splash(app: &AppHandle, query: Option<&str>) -> Result<(), String> {
    let turn = SPLASH_TURN.fetch_add(1, std::sync::atomic::Ordering::AcqRel) + 1;
    if let Some(window) = app.get_webview_window("splash") {
        let mut url = window.url().map_err(|e| e.to_string())?;
        url.set_query(query);
        window.navigate(url).map_err(|e| e.to_string())?;
    } else {
        let page = match query {
            Some(query) => format!("splash.html?{query}"),
            None => "splash.html".into(),
        };
        tauri::WebviewWindowBuilder::new(app, "splash", tauri::WebviewUrl::App(page.into()))
            .title("Cortex")
            .inner_size(420.0, 280.0)
            .resizable(false)
            .decorations(false)
            .transparent(true)
            .always_on_top(true)
            .skip_taskbar(true)
            .center()
            .focused(false)
            .shadow(false)
            .accept_first_mouse(true)
            .build()
            .map_err(|e| e.to_string())?;
    }

    let handle = app.clone();
    tauri::async_runtime::spawn(async move {
        // Above the page's own timer (src/splash/main.ts): only a fallback, so
        // a page that never draws cannot leave a window that eats clicks.
        tokio::time::sleep(std::time::Duration::from_secs(10)).await;
        if SPLASH_TURN.load(std::sync::atomic::Ordering::Acquire) == turn {
            close_splash(&handle);
        }
    });
    Ok(())
}

#[tauri::command]
fn quit(app: AppHandle) {
    app.exit(0);
}

/// Island width in logical points. Must match `.island--open` in
/// `src/island/island.css` and the window width in `tauri.conf.json`.
const ISLAND_WIDTH: f64 = 460.0;
/// Bounds for what the UI may ask in `island_fit`.
const ISLAND_MIN_WIDTH: f64 = 40.0;
/// The window height in `tauri.conf.json`, until the UI asks for its own.
const ISLAND_START_HEIGHT: f64 = 50.0;
const ISLAND_MIN_HEIGHT: f64 = 6.0;
const ISLAND_MAX_HEIGHT: f64 = 600.0;

/// Pointer entered (`true`) or left (`false`) the island. Emitted on macOS,
/// where the web view gets no hover events in a window that is never key;
/// elsewhere the UI uses the DOM's own.
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
pub const HOVER_EVENT: &str = "cortex://hover";

/// The user clicked in another app while a card was waiting. Emitted on
/// macOS, where the island is never key and so never sees a blur; elsewhere
/// the UI uses the window's own blur.
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
pub const OUTSIDE_CLICK_EVENT: &str = "cortex://outside-click";

const ISLAND_CUTOUT_EVENT: &str = "cortex://island-cutout";

/// Only macOS knows of a camera cutout.
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
#[derive(Clone, Copy, PartialEq, serde::Serialize)]
struct IslandCutout {
    width: f64,
    height: f64,
}

struct IslandPlacement {
    position: String,
    screen: Option<String>,
    width: f64,
    height: f64,
    cutout: Option<IslandCutout>,
}

#[tauri::command]
fn island_cutout(app: AppHandle) -> Option<IslandCutout> {
    app.state::<Mutex<IslandPlacement>>()
        .lock()
        .expect("island placement lock")
        .cutout
}

/// Size the island to what it draws at the selected edge. The window is
/// never larger than its content, so no invisible area eats clicks.
#[tauri::command]
fn island_fit(app: AppHandle, width: f64, height: f64) {
    if width.is_finite() && height.is_finite() {
        let state = app.state::<Mutex<IslandPlacement>>();
        {
            let mut placement = state.lock().expect("island placement lock");
            placement.width = width.clamp(ISLAND_MIN_WIDTH, ISLAND_WIDTH);
            placement.height = height.clamp(ISLAND_MIN_HEIGHT, ISLAND_MAX_HEIGHT);
        }
        place_island(&app);
    }
}

#[derive(serde::Serialize)]
struct IslandScreen {
    name: String,
    primary: bool,
    width: u32,
    height: u32,
}

#[tauri::command]
fn island_screens(app: AppHandle) -> Result<Vec<IslandScreen>, String> {
    let primary = app.primary_monitor().map_err(|e| e.to_string())?;
    let monitors = app.available_monitors().map_err(|e| e.to_string())?;
    Ok(monitors
        .into_iter()
        .filter_map(|monitor| {
            let name = monitor.name().filter(|name| is_screen_name(name))?.clone();
            let primary = primary.as_ref().is_some_and(|primary| {
                primary.position() == monitor.position() && primary.name() == Some(&name)
            });
            let size = monitor.size().to_logical::<u32>(monitor.scale_factor());
            Some(IslandScreen {
                name,
                primary,
                width: size.width,
                height: size.height,
            })
        })
        .collect())
}

#[tauri::command]
fn island_set_position(app: AppHandle, position: String) -> Result<(), String> {
    if !is_position(&position) {
        return Err("Unknown island position.".into());
    }
    set_island_placement(&app, |p| p.island_position = Some(position))
}

#[tauri::command]
fn island_set_screen(app: AppHandle, screen: Option<String>) -> Result<(), String> {
    if let Some(name) = &screen {
        if !is_screen_name(name)
            || !app
                .available_monitors()
                .map_err(|e| e.to_string())?
                .iter()
                .any(|monitor| monitor.name() == Some(name))
        {
            return Err("Unknown screen.".into());
        }
    }
    set_island_placement(&app, |p| p.island_screen = screen)
}

fn set_island_placement(
    app: &AppHandle,
    change: impl FnOnce(&mut prefs::Prefs),
) -> Result<(), String> {
    let state = app.state::<Mutex<IslandPlacement>>();
    let island = {
        let mut placement = state.lock().expect("island placement lock");
        let island = prefs::IslandPrefs::from(&prefs::update(change)?);
        placement.position.clone_from(&island.position);
        placement.screen.clone_from(&island.screen);
        island
    };
    prefs::emit(app, island);
    place_island(app);
    Ok(())
}

/// Fit the island at its chosen edge, falling back to the primary monitor.
fn place_island(app: &AppHandle) {
    let Some(window) = app.get_webview_window("island") else {
        return;
    };
    let state = app.state::<Mutex<IslandPlacement>>();
    let (position, name, width, height) = {
        let placement = state.lock().expect("island placement lock");
        (
            placement.position.clone(),
            placement.screen.clone(),
            placement.width,
            placement.height,
        )
    };
    let monitor = name
        .and_then(|name| {
            app.available_monitors()
                .ok()?
                .into_iter()
                .find(|m| m.name() == Some(&name))
        })
        .or_else(|| app.primary_monitor().ok().flatten());
    let Some(monitor) = monitor else {
        return;
    };

    let scale = monitor.scale_factor();
    let rect = |origin: tauri::PhysicalPosition<i32>, size: tauri::PhysicalSize<u32>| Rect {
        x: origin.x as f64 / scale,
        y: origin.y as f64 / scale,
        width: size.width as f64 / scale,
        height: size.height as f64 / scale,
    };
    let screen = rect(*monitor.position(), *monitor.size());
    let work = rect(monitor.work_area().position, monitor.work_area().size);
    #[cfg(target_os = "macos")]
    let cutout = if position == "top" {
        macos::camera_cutout(screen.x, screen.y, screen.width, screen.height)
            .map(|(width, height)| IslandCutout { width, height })
    } else {
        None
    };
    #[cfg(not(target_os = "macos"))]
    let cutout = None;
    let cutout_changed = {
        let mut placement = state.lock().expect("island placement lock");
        std::mem::replace(&mut placement.cutout, cutout) != cutout
    };
    let width =
        island_width(width, cutout.map(|cutout| cutout.width), ISLAND_WIDTH).min(screen.width);
    let height = height.min(screen.height);
    let (x, y) = island_origin(&position, screen, work, width, height);
    let size = tauri::LogicalSize::new(width, height);
    let origin = tauri::LogicalPosition::new(x, y);
    // Windows uses physical desktop coordinates and the old monitor's DPI
    // when converting a logical size immediately before a move.
    #[cfg(target_os = "windows")]
    let (origin, size) = (
        origin.to_physical::<i32>(scale),
        size.to_physical::<u32>(scale),
    );
    let _ = window.set_size(size);
    let _ = window.set_position(origin);
    if cutout_changed {
        let _ = app.emit_to("island", ISLAND_CUTOUT_EVENT, cutout);
    }
}

fn main() {
    if server::already_served() {
        eprintln!("cortex: another copy is already running; this one quits");
        return;
    }

    let builder = tauri::Builder::default();

    #[cfg(target_os = "macos")]
    let builder = builder.plugin(tauri_nspanel::init());

    let prefs = prefs::load();
    let island = prefs::IslandPrefs::from(&prefs);
    gaze::set_lively(&prefs);
    let (last_version, launch_at_login) = (prefs.last_version.clone(), prefs.launch_at_login);
    builder
        .plugin(tauri_plugin_updater::Builder::new().build())
        .manage(AppState::with_rules(rules::load()))
        .manage(updates::UpdateState::new(prefs.updates))
        .manage(github::GithubState::new(prefs))
        .manage(Mutex::new(IslandPlacement {
            position: island.position,
            screen: island.screen,
            width: ISLAND_WIDTH,
            height: ISLAND_START_HEIGHT,
            cutout: None,
        }))
        .invoke_handler(tauri::generate_handler![
            updates::updates_status,
            updates::updates_set_enabled,
            updates::updates_start,
            updates::updates_install,
            github::github_status,
            github::github_set_enabled,
            github::github_refresh,
            github::github_set_account_shown,
            github::github_sign_in,
            github::github_install,
            github::github_cancel_task,
            github::open_url,
            snapshot,
            island_fit,
            island_cutout,
            prefs::island_prefs,
            autostart::autostart_status,
            autostart::autostart_set,
            prefs::island_set_mascot,
            island_screens,
            island_set_position,
            island_set_screen,
            prefs::island_set_sound,
            prefs::island_set_lively,
            prefs::language_set,
            gaze::island_gaze_origin,
            splash_done,
            opening_shown,
            away::away_now,
            open_settings,
            quit,
            rules_list,
            rule_remove,
            rules_clear,
            respond,
            answer,
            hooks_status,
            hooks_preview,
            hooks_install,
            hooks_uninstall_preview,
            hooks_uninstall
        ])
        .setup(move |app| {
            let handle = app.handle().clone();

            #[cfg(target_os = "macos")]
            if let Err(err) = macos::make_island_panel(&handle) {
                eprintln!("cortex: panel setup failed, plain window fallback: {err}");
            }
            #[cfg(target_os = "macos")]
            if let Err(err) = macos::add_menu_bar_item(&handle, &island.language) {
                eprintln!("cortex: menu-bar item failed: {err}");
            }

            displays::watch(&handle);
            place_island(&handle);
            // A window moved onto a screen of another scale is rescaled by the
            // system after the move: place it again with the new scale.
            if let Some(island) = handle.get_webview_window("island") {
                let replace = handle.clone();
                island.on_window_event(move |event| {
                    if matches!(event, tauri::WindowEvent::ScaleFactorChanged { .. }) {
                        place_island(&replace);
                    }
                });
            }

            autostart::on_launch();
            let version = handle.package_info().version.to_string();
            // Never over a file that could not be read: it would lose every
            // setting, and every launch would look like a new install.
            let prefs_valid = prefs::load_valid().is_some();
            let mut first_launch = prefs_valid
                && cortex_core::autostart::is_first_launch(
                    last_version.as_deref(),
                    launch_at_login,
                );
            if last_version.as_deref() != Some(version.as_str()) && prefs_valid {
                let saved = version.clone();
                if let Err(err) = prefs::update(|p| p.last_version = Some(saved)) {
                    eprintln!("cortex: {err}");
                    // Unsaved, every launch would be the first one.
                    first_launch = false;
                }
            }
            let screen = cortex_core::autostart::opening_screen(
                std::env::args().any(|a| a == cortex_core::autostart::AUTOSTART_ARG),
                recovered_recently(),
                cortex_core::autostart::was_updated(
                    last_version.as_deref(),
                    launch_at_login,
                    &version,
                ),
            );
            use std::sync::atomic::Ordering::Release;
            FIRST_LAUNCH.store(first_launch && screen == OpeningScreen::Welcome, Release);
            OPENING.store(screen != OpeningScreen::Nothing, Release);
            let opened = match screen {
                OpeningScreen::Welcome => open_splash(&handle, None),
                OpeningScreen::Updated => open_splash(&handle, Some("mode=updated")),
                OpeningScreen::Nothing => Ok(()),
            };
            if let Err(err) = opened {
                eprintln!("cortex: opening screen failed: {err}");
                opening_closed(&handle);
            }
            // A GPU process that hangs while the system is busy (just after an
            // update or a login) leaves the island empty for good (#115).
            #[cfg(target_os = "macos")]
            check_island(handle.clone(), std::time::Duration::from_secs(30));

            // Keep the relay at a stable path so installed hooks survive updates.
            if let Err(err) = hooks::ensure_hook_binary() {
                eprintln!("cortex: relay not installed to its stable path: {err}");
            }

            // The hook endpoint.
            let server_handle = handle.clone();
            tauri::async_runtime::spawn(async move {
                match server::serve(server_handle.clone()).await {
                    Err(err) if server::is_already_served(&err) => {
                        eprintln!("cortex: another copy is already running; this one quits");
                        server_handle.exit(0);
                    }
                    Err(err) => eprintln!("cortex: hook endpoint failed: {err}"),
                    Ok(()) => {}
                }
            });

            // Dev aid: open Settings at launch, for visual checks without a click.
            #[cfg(debug_assertions)]
            if std::env::var_os("CORTEX_DEV_OPEN_SETTINGS").is_some() {
                spawn_open_settings(handle.clone(), false);
            }

            tauri::async_runtime::spawn(github::poller(handle.clone()));
            tauri::async_runtime::spawn(updates::checker(handle.clone()));
            gaze::spawn(handle.clone());
            away::spawn(handle.clone());

            // The janitor (sleeps whenever the board is empty).
            tauri::async_runtime::spawn(server::janitor(handle));

            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error while building Cortex")
        .run(|app, event| {
            // Opening Cortex while it runs: show Settings, and make sure the
            // island is really drawn (#115).
            #[cfg(target_os = "macos")]
            if let tauri::RunEvent::Reopen { .. } = event {
                spawn_open_settings(app.clone(), false);
                check_island(app.clone(), std::time::Duration::ZERO);
            }
            #[cfg(not(target_os = "macos"))]
            let _ = (app, event);
        });
}
