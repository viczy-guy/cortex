//! Cortex — the Tauri shell.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod server;
mod state;

#[cfg(target_os = "macos")]
mod macos;

use tauri::{AppHandle, Manager, State};

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
    }
}

/// Island width in logical points. Must match `WINDOW_WIDTH` in the UI and
/// the window width in `tauri.conf.json`.
const ISLAND_WIDTH: f64 = 460.0;

/// Pin the island to the top-center of the primary monitor.
fn position_island(app: &AppHandle) {
    let Some(window) = app.get_webview_window("island") else {
        return;
    };
    let Ok(Some(monitor)) = window.primary_monitor() else {
        return;
    };
    // Work in logical points of the *target* monitor. The window may have been
    // created on another display with a different scale factor, so its own
    // physical size must not enter the math.
    let scale = monitor.scale_factor();
    let origin_x = monitor.position().x as f64 / scale;
    let origin_y = monitor.position().y as f64 / scale;
    let screen_width = monitor.size().width as f64 / scale;
    let _ = window.set_position(tauri::LogicalPosition {
        x: origin_x + (screen_width - ISLAND_WIDTH) / 2.0,
        y: origin_y,
    });
}

fn main() {
    let mut builder = tauri::Builder::default();

    #[cfg(target_os = "macos")]
    {
        builder = builder.plugin(tauri_nspanel::init());
    }

    builder
        .manage(AppState::default())
        .invoke_handler(tauri::generate_handler![snapshot, respond])
        .setup(|app| {
            let handle = app.handle().clone();

            #[cfg(target_os = "macos")]
            if let Err(err) = macos::make_island_panel(&handle) {
                eprintln!("cortex: panel setup failed, plain window fallback: {err}");
            }

            position_island(&handle);

            // The hook endpoint.
            let server_handle = handle.clone();
            tauri::async_runtime::spawn(async move {
                if let Err(err) = server::serve(server_handle).await {
                    eprintln!("cortex: hook endpoint failed: {err}");
                }
            });

            // The janitor (sleeps whenever the board is empty).
            tauri::async_runtime::spawn(server::janitor(handle));

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running Cortex");
}
