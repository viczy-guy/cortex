//! macOS island behavior: turn the Tauri window into a non-activating
//! NSPanel so clicking Allow never steals focus from the user's editor or
//! terminal. This is the Phase 0 spike made permanent (see ADR 0001).

use std::ptr::NonNull;

use block2::RcBlock;
use tauri::{AppHandle, Emitter, Manager};
// `NSEvent`, `MainThreadMarker` and `NSObjectProtocol` come in with `tauri_panel!`.
use tauri_nspanel::objc2::sel;
use tauri_nspanel::objc2_app_kit::{NSEventMask, NSScreen};
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

/// The camera cutout of the screen at this place (logical points, y down
/// from the top of the main screen, as Tauri counts): its width and height.
pub fn camera_cutout(x: f64, y: f64, width: f64, height: f64) -> Option<(f64, f64)> {
    let screens = NSScreen::screens(MainThreadMarker::new()?);
    // AppKit counts y up from the bottom of the main screen.
    let main_height = screens.iter().next()?.frame().size.height;
    let cocoa_y = main_height - (y + height);
    let screen = screens.iter().find(|screen| {
        let frame = screen.frame();
        (frame.origin.x - x).abs() <= 1.0
            && (frame.origin.y - cocoa_y).abs() <= 1.0
            && (frame.size.width - width).abs() <= 1.0
    })?;
    // The safe-area and auxiliary-area selectors were introduced in macOS 12.
    if !screen.respondsToSelector(sel!(safeAreaInsets)) {
        return None;
    }
    let height = screen.safeAreaInsets().top;
    if height <= 0.0 {
        return None;
    }
    let width = screen.frame().size.width
        - screen.auxiliaryTopLeftArea().size.width
        - screen.auxiliaryTopRightArea().size.width;
    (width > 0.0).then_some((width, height))
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

/// A menu-bar item with Settings and Quit. Cortex is a background app, so it
/// is not in the Force Quit window; this menu is native and still works when
/// the island's web view does not respond (#76).
pub fn add_menu_bar_item(app: &AppHandle) -> tauri::Result<()> {
    use tauri::menu::MenuBuilder;
    use tauri::tray::TrayIconBuilder;

    let menu = MenuBuilder::new(app)
        .text("menu-bar-settings", "Settings…")
        .separator()
        .text("menu-bar-quit", "Quit Cortex")
        .build()?;
    TrayIconBuilder::with_id("cortex")
        .icon(menu_bar_icon())
        .icon_as_template(true)
        .tooltip("Cortex")
        .menu(&menu)
        .show_menu_on_left_click(true)
        // Menu events are app-wide: the ids say which menu they came from.
        .on_menu_event(|app, event| match event.id().as_ref() {
            "menu-bar-settings" => crate::spawn_open_settings(app.clone()),
            "menu-bar-quit" => {
                // The way out must not hang on a stuck teardown.
                std::thread::spawn(|| {
                    std::thread::sleep(std::time::Duration::from_secs(2));
                    std::process::exit(0);
                });
                app.exit(0);
            }
            _ => {}
        })
        .build(app)?;
    Ok(())
}

/// The app icon's eyes, drawn as a template image: AppKit colors it for a
/// light or dark menu bar. 36 px for an 18 pt item.
fn menu_bar_icon() -> tauri::image::Image<'static> {
    const SIZE: u32 = 36;
    // Each eye: a 7 × 18 px capsule, as in the mascot (`src/shared/mascots.ts`).
    const RADIUS: f32 = 3.5;
    const HALF_STRAIGHT: f32 = 5.5;
    const EYES_X: [f32; 2] = [11.5, 24.5];
    let center_y = SIZE as f32 / 2.0;
    let mut rgba = Vec::with_capacity((SIZE * SIZE * 4) as usize);
    for y in 0..SIZE {
        for x in 0..SIZE {
            let (px, py) = (x as f32 + 0.5, y as f32 + 0.5);
            // Distance to the nearer eye's edge, smoothed over a pixel.
            let dy = ((py - center_y).abs() - HALF_STRAIGHT).max(0.0);
            let edge = EYES_X
                .iter()
                .map(|&eye_x| ((px - eye_x).powi(2) + dy * dy).sqrt() - RADIUS)
                .fold(f32::INFINITY, f32::min);
            let alpha = (0.5 - edge).clamp(0.0, 1.0);
            rgba.extend_from_slice(&[0, 0, 0, (alpha * 255.0).round() as u8]);
        }
    }
    tauri::image::Image::new_owned(rgba, SIZE, SIZE)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_menu_bar_icon_is_two_eyes_with_clear_edges() {
        let icon = menu_bar_icon();
        let (w, h) = (icon.width() as usize, icon.height() as usize);
        let rgba = icon.rgba();
        assert_eq!(rgba.len(), w * h * 4);
        let alpha = |x: usize, y: usize| rgba[(y * w + x) * 4 + 3];
        assert_eq!(alpha(11, h / 2), 255, "the left eye");
        assert_eq!(alpha(24, h / 2), 255, "the right eye");
        assert_eq!(alpha(w / 2, h / 2), 0, "between the eyes");
        assert_eq!(alpha(0, 0), 0, "a corner");
        assert!(
            (0..w).all(|x| alpha(x, 0) == 0 && alpha(x, h - 1) == 0),
            "top and bottom are padding"
        );
    }
}
