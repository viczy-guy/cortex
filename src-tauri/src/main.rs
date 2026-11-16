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

/// Pin the island to the top-center of the primary monitor.
fn position_island(app: &AppHandle) {
    let Some(window) = app.get_webview_window("island") else {
        return;
    };
    let Ok(Some(monitor)) = window.primary_monitor() else {
        return;
    };
    let screen = monitor.size();
    let win = window.outer_size().unwrap_or(tauri::PhysicalSize {
        width: 460,
        height: 50,
    });
    let x = (screen.width.saturating_sub(win.width)) / 2;
    let _ = window.set_position(tauri::PhysicalPosition {
        x: monitor.position().x + x as i32,
        y: monitor.position().y,
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
