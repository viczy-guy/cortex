//! Cortex — the Tauri shell.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod files;
mod github;
mod hooks;
mod prefs;
mod rules;
mod server;
mod state;
mod updates;

#[cfg(target_os = "macos")]
mod macos;

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

/// Open (or bring forward) the Settings window. Async on purpose: creating
/// a window from a sync command deadlocks on Windows.
#[tauri::command]
async fn open_settings(app: AppHandle) -> Result<(), String> {
    if let Some(window) = app.get_webview_window("settings") {
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
        tauri::WebviewUrl::App("settings.html".into()),
    )
    .title("Cortex Settings")
    .inner_size(560.0, 640.0)
    .min_inner_size(480.0, 480.0)
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

#[tauri::command]
fn quit(app: AppHandle) {
    app.exit(0);
}

/// Island width in logical points. Must match `WINDOW_WIDTH` in the UI and
/// the window width in `tauri.conf.json`.
const ISLAND_WIDTH: f64 = 460.0;
/// Bounds for what the UI may ask in `island_fit`.
const ISLAND_MIN_WIDTH: f64 = 40.0;
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

/// Size the island to what it draws, keeping it top-centered. The window is
/// never larger than its content, so no invisible area eats clicks.
#[tauri::command]
fn island_fit(app: AppHandle, width: f64, height: f64) {
    if width.is_finite() && height.is_finite() {
        place_island(
            &app,
            width.clamp(ISLAND_MIN_WIDTH, ISLAND_WIDTH),
            Some(height.clamp(ISLAND_MIN_HEIGHT, ISLAND_MAX_HEIGHT)),
        );
    }
}

/// Resize the island and pin it to the top-center of the primary monitor.
/// `height: None` keeps the current height.
fn place_island(app: &AppHandle, width: f64, height: Option<f64>) {
    let Some(window) = app.get_webview_window("island") else {
        return;
    };
    let Ok(Some(monitor)) = window.primary_monitor() else {
        return;
    };
    if let Some(height) = height {
        let _ = window.set_size(tauri::LogicalSize { width, height });
    }
    // Work in logical points of the *target* monitor. The window may have been
    // created on another display with a different scale factor, so its own
    // physical size must not enter the math.
    let scale = monitor.scale_factor();
    let origin_x = monitor.position().x as f64 / scale;
    let origin_y = monitor.position().y as f64 / scale;
    let screen_width = monitor.size().width as f64 / scale;
    let _ = window.set_position(tauri::LogicalPosition {
        x: origin_x + (screen_width - width) / 2.0,
        y: origin_y,
    });
}

fn main() {
    let builder = tauri::Builder::default();

    #[cfg(target_os = "macos")]
    let builder = builder.plugin(tauri_nspanel::init());

    let prefs = prefs::load();
    builder
        .plugin(tauri_plugin_updater::Builder::new().build())
        .manage(AppState::with_rules(rules::load()))
        .manage(updates::UpdateState::new(prefs.updates))
        .manage(github::GithubState::new(prefs))
        .invoke_handler(tauri::generate_handler![
            updates::updates_status,
            updates::updates_set_enabled,
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
        .setup(|app| {
            let handle = app.handle().clone();

            #[cfg(target_os = "macos")]
            if let Err(err) = macos::make_island_panel(&handle) {
                eprintln!("cortex: panel setup failed, plain window fallback: {err}");
            }
            #[cfg(target_os = "macos")]
            if let Err(err) = macos::add_menu_bar_item(&handle) {
                eprintln!("cortex: menu-bar item failed: {err}");
            }

            place_island(&handle, ISLAND_WIDTH, None);

            // Keep the relay at a stable path so installed hooks survive updates.
            if let Err(err) = hooks::ensure_hook_binary() {
                eprintln!("cortex: relay not installed to its stable path: {err}");
            }

            // The hook endpoint.
            let server_handle = handle.clone();
            tauri::async_runtime::spawn(async move {
                if let Err(err) = server::serve(server_handle).await {
                    eprintln!("cortex: hook endpoint failed: {err}");
                }
            });

            // Dev aid: open Settings at launch, for visual checks without a click.
            #[cfg(debug_assertions)]
            if std::env::var_os("CORTEX_DEV_OPEN_SETTINGS").is_some() {
                let settings_handle = handle.clone();
                tauri::async_runtime::spawn(async move {
                    if let Err(err) = open_settings(settings_handle).await {
                        eprintln!("cortex: settings window failed: {err}");
                    }
                });
            }

            tauri::async_runtime::spawn(github::poller(handle.clone()));
            tauri::async_runtime::spawn(updates::checker(handle.clone()));

            // The janitor (sleeps whenever the board is empty).
            tauri::async_runtime::spawn(server::janitor(handle));

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running Cortex");
}
