//! A screen added or removed moves the island to its place again, with no
//! polling: macOS says so with a notification.

use tauri::AppHandle;

#[cfg(target_os = "macos")]
fn changed(app: &AppHandle) {
    use tauri::Emitter;

    crate::place_island(app);
    let _ = app.emit_to("settings", "cortex://island-screens", ());
}

#[cfg(target_os = "macos")]
pub fn watch(app: &AppHandle) {
    use std::ptr::NonNull;

    use block2::RcBlock;
    use tauri_nspanel::objc2_app_kit::NSApplicationDidChangeScreenParametersNotification;
    use tauri_nspanel::objc2_foundation::{NSNotification, NSNotificationCenter, NSOperationQueue};

    let app = app.clone();
    let observer = RcBlock::new(move |_: NonNull<NSNotification>| changed(&app));
    // AppKit posts this notification on the main thread. The center keeps the
    // observer for the lifetime of the application.
    unsafe {
        NSNotificationCenter::defaultCenter().addObserverForName_object_queue_usingBlock(
            Some(NSApplicationDidChangeScreenParametersNotification),
            None,
            Some(&NSOperationQueue::mainQueue()),
            &observer,
        );
    }
}

// shortcut: on Windows and Linux the island is placed again only when it
// next changes size (hover, a card, a color change); the system has already
// moved it onto a remaining screen. Watch their display messages if that
// turns out not to be enough.
#[cfg(not(target_os = "macos"))]
pub fn watch(_: &AppHandle) {}
