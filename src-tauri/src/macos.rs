//! macOS island behavior: turn the Tauri window into a non-activating
//! NSPanel so clicking Allow never steals focus from the user's editor or
//! terminal. This is the Phase 0 spike made permanent (see ADR 0001).

use tauri::{AppHandle, Manager};
use tauri_nspanel::{tauri_panel, CollectionBehavior, PanelLevel, StyleMask, WebviewWindowExt};

tauri_panel! {
    // Never the key window: the user's app keeps keyboard focus while the
    // island still receives clicks.
    panel!(IslandPanel {
        config: {
            can_become_key_window: false,
            is_floating_panel: true
        }
    })
}

pub fn make_island_panel(app: &AppHandle) -> Result<(), String> {
    let window = app
        .get_webview_window("island")
        .ok_or("island window not found")?;

    let panel = window
        .to_panel::<IslandPanel>()
        .map_err(|e| format!("to_panel: {e}"))?;

    // Clicks do not activate the app.
    panel
        .set_style_mask(
            StyleMask::empty()
                .borderless()
                .nonactivating_panel()
                .value(),
        )
        .map_err(|e| format!("set_style_mask: {e}"))?;
    // Above the menu bar, below system popups.
    panel.set_level(PanelLevel::Status.value());
    // Follows the user across Spaces and over fullscreen apps.
    panel.set_collection_behavior(
        CollectionBehavior::new()
            .can_join_all_spaces()
            .full_screen_auxiliary()
            .value(),
    );
    panel.set_hides_on_deactivate(false);

    // The app never shows in the Dock or the Cmd-Tab switcher.
    let _ = app.set_activation_policy(tauri::ActivationPolicy::Accessory);
    Ok(())
}
