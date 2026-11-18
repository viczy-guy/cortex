//! macOS island behavior: turn the Tauri window into a non-activating
//! NSPanel so clicking Allow never steals focus from the user's editor or
//! terminal. This is the Phase 0 spike made permanent (see ADR 0001).

use std::ptr::NonNull;

use block2::RcBlock;
use tauri::{AppHandle, Emitter, Manager};
// `NSEvent` comes in with `tauri_panel!`.
use tauri_nspanel::objc2_app_kit::NSEventMask;
use tauri_nspanel::{
    tauri_panel, CollectionBehavior, PanelLevel, StyleMask, TrackingAreaOptions, WebviewWindowExt,
};

use crate::state::AppState;
use crate::{HOVER_EVENT, OUTSIDE_CLICK_EVENT};

tauri_panel! {
    // Never the key window: the user's app keeps keyboard focus while the
    // island still receives clicks.
    panel!(IslandPanel {
        config: {
            can_become_key_window: false,
            is_floating_panel: true
        }
        with: {
            // The web view only tracks the pointer in a key window, which the
            // island never is: hover has to come from the panel itself.
            tracking_area: {
                options: TrackingAreaOptions::new()
                    .active_always()
                    .mouse_entered_and_exited(),
                auto_resize: true
            }
        }
    })

    panel_event!(IslandPanelEvents {})
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

    // Tell the island when the pointer is over it (see `HOVER_EVENT`).
    let events = IslandPanelEvents::new();
    let entered = app.clone();
    events.on_mouse_entered(move |_| {
        let _ = entered.emit_to("island", HOVER_EVENT, true);
    });
    let exited = app.clone();
    events.on_mouse_exited(move |_| {
        let _ = exited.emit_to("island", HOVER_EVENT, false);
    });
    // The panel retains the handler.
    panel.set_event_handler(Some(events.as_ref()));

    // A click in any other app folds the island, even over a waiting card.
    // Mouse monitors need no accessibility permission (key monitors do), and
    // clicks on Cortex's own windows never reach a global monitor.
    let outside = app.clone();
    let on_click = RcBlock::new(move |_: NonNull<NSEvent>| {
        // Wake the web view only when there is a card to fold away.
        if outside.state::<AppState>().has_pending() {
            let _ = outside.emit_to("island", OUTSIDE_CLICK_EVENT, ());
        }
    });
    let mask =
        NSEventMask::LeftMouseDown | NSEventMask::RightMouseDown | NSEventMask::OtherMouseDown;
    // Kept for the life of the app: the monitor is never removed.
    std::mem::forget(NSEvent::addGlobalMonitorForEventsMatchingMask_handler(
        mask, &on_click,
    ));

    // The app never shows in the Dock or the Cmd-Tab switcher.
    let _ = app.set_activation_policy(tauri::ActivationPolicy::Accessory);
    Ok(())
}
