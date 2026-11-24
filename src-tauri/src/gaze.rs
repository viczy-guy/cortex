//! The lively mascot's eyes (#145). While the mascot is lively, the cursor is
//! read ten times a second and the island hears only when the direction
//! changes: a resting mouse sends nothing. On macOS a locked screen stops the
//! mascot too, because any repaint of a locked island keeps WebKit busy until
//! unlock (#51).

use cortex_core::gaze::{gaze, Gaze};
use serde::Serialize;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager};

pub const GAZE_EVENT: &str = "cortex://gaze";
/// Set at launch from the preferences and by Settings.
pub static LIVELY: AtomicBool = AtomicBool::new(true);
const TICK: Duration = Duration::from_millis(100);
/// How often to look again while the mascot is not lively or the cursor
/// cannot be read (Wayland gives no position outside the window).
const IDLE: Duration = Duration::from_secs(2);
/// Ticks between two looks at the lock: once a second.
const LOCK_EVERY: u32 = 10;

#[derive(Serialize, Clone, Copy, PartialEq, Eq)]
struct GazeEvent {
    x: i8,
    y: i8,
    near: bool,
    /// `false` while the screen is locked: the island repaints nothing.
    awake: bool,
}

impl GazeEvent {
    fn new(g: Gaze, awake: bool) -> Self {
        Self {
            x: g.x,
            y: g.y,
            near: g.near,
            awake,
        }
    }
}

pub async fn watcher(app: AppHandle) {
    let mut last: Option<GazeEvent> = None;
    let mut cursor_was: Option<(f64, f64)> = None;
    let mut locked = false;
    let mut ticks: u32 = 0;
    loop {
        if !LIVELY.load(Ordering::Relaxed) {
            last = None;
            cursor_was = None;
            tokio::time::sleep(IDLE).await;
            continue;
        }
        tokio::time::sleep(TICK).await;
        ticks = ticks.wrapping_add(1);
        if ticks.is_multiple_of(LOCK_EVERY) {
            locked = screen_locked();
        }
        let event = if locked {
            GazeEvent::new(Gaze::AHEAD, false)
        } else {
            let Ok(cursor) = app.cursor_position() else {
                tokio::time::sleep(IDLE).await;
                continue;
            };
            let at = (cursor.x, cursor.y);
            if cursor_was == Some(at) && last.is_some_and(|e| e.awake) {
                continue;
            }
            cursor_was = Some(at);
            let Some(island) = app.get_webview_window("island") else {
                continue;
            };
            let (Ok(pos), Ok(size), Ok(scale)) = (
                island.outer_position(),
                island.outer_size(),
                island.scale_factor(),
            ) else {
                continue;
            };
            let center = (
                f64::from(pos.x) + f64::from(size.width) / 2.0,
                f64::from(pos.y) + f64::from(size.height) / 2.0,
            );
            GazeEvent::new(gaze(at, center, scale), true)
        };
        if last != Some(event) {
            last = Some(event);
            let _ = app.emit_to("island", GAZE_EVENT, event);
        }
    }
}

/// Whether the login session's screen is locked (the lock screen or the
/// screen saver with a password).
#[cfg(target_os = "macos")]
fn screen_locked() -> bool {
    use std::ffi::c_void;
    #[link(name = "CoreGraphics", kind = "framework")]
    extern "C" {
        fn CGSessionCopyCurrentDictionary() -> *const c_void;
    }
    #[link(name = "CoreFoundation", kind = "framework")]
    extern "C" {
        fn CFStringCreateWithBytes(
            alloc: *const c_void,
            bytes: *const u8,
            len: isize,
            encoding: u32,
            external: u8,
        ) -> *const c_void;
        fn CFDictionaryGetValue(dict: *const c_void, key: *const c_void) -> *const c_void;
        fn CFGetTypeID(cf: *const c_void) -> usize;
        fn CFBooleanGetTypeID() -> usize;
        fn CFBooleanGetValue(boolean: *const c_void) -> u8;
        fn CFRelease(cf: *const c_void);
    }
    const UTF8: u32 = 0x0800_0100;
    const KEY: &[u8] = b"CGSSessionScreenIsLocked";
    // SAFETY: every pointer is checked before use; the dictionary and the key
    // come from Create/Copy functions and are released once; the value is
    // borrowed from the dictionary, which outlives its use.
    unsafe {
        let session = CGSessionCopyCurrentDictionary();
        if session.is_null() {
            return false;
        }
        let key =
            CFStringCreateWithBytes(std::ptr::null(), KEY.as_ptr(), KEY.len() as isize, UTF8, 0);
        let mut locked = false;
        if !key.is_null() {
            let value = CFDictionaryGetValue(session, key);
            locked = !value.is_null()
                && CFGetTypeID(value) == CFBooleanGetTypeID()
                && CFBooleanGetValue(value) != 0;
            CFRelease(key);
        }
        CFRelease(session);
        locked
    }
}

/// Only WebKit on macOS pays for a locked repaint (#51).
#[cfg(not(target_os = "macos"))]
fn screen_locked() -> bool {
    false
}
