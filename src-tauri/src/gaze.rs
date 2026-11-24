//! The lively mascot's eyes (#145). While the mascot is lively, a thread of its
//! own reads the cursor ten times a second and tells the island only when the
//! direction changes, plus once a minute while the mouse keeps moving (so a
//! busy mouse is never taken for a nap). A resting mouse sends nothing. On
//! macOS a locked screen pauses it all: any repaint of a locked island keeps
//! WebKit busy until unlock (#51).

use cortex_core::gaze::{gaze, Gaze, STEPS_X, STEPS_Y};
use cortex_core::prefs::Prefs;
use serde::Serialize;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, Manager, PhysicalPosition};

pub const GAZE_EVENT: &str = "cortex://gaze";
static LIVELY: AtomicBool = AtomicBool::new(true);
/// Where the mascot sits in the island, in points from its top left corner;
/// the island says so after it draws (`None`: the window's center).
static ORIGIN: Mutex<Option<(f64, f64)>> = Mutex::new(None);
const TICK: Duration = Duration::from_millis(100);
/// While paused (not lively, no mascot, screen locked, no cursor), how often to
/// look again.
const IDLE: Duration = Duration::from_secs(2);
/// Ticks between two looks at the lock and the screens: once a second. The
/// direction is also worked out again then, in case the island moved under a
/// resting cursor.
const SLOW_EVERY: u32 = 10;
/// A moving mouse is reported at least this often, even in the same direction.
const STILL_MOVING: Duration = Duration::from_secs(60);

/// Lively and with a mascot to move; set at launch and by Settings.
pub fn set_lively(prefs: &Prefs) {
    let on = prefs.lively.unwrap_or(true) && prefs.mascot.as_deref() != Some("none");
    LIVELY.store(on, Ordering::Relaxed);
}

#[derive(Serialize, Clone, Copy, PartialEq)]
struct GazeEvent {
    /// -1 (left) to 1 (right).
    x: f32,
    /// -1 (up) to 1 (down).
    y: f32,
    near: bool,
    /// `false` while the screen is locked: the island repaints nothing.
    awake: bool,
}

impl GazeEvent {
    fn new(g: Gaze, awake: bool) -> Self {
        Self {
            x: f32::from(g.x) / f32::from(STEPS_X),
            y: f32::from(g.y) / f32::from(STEPS_Y),
            near: g.near,
            awake,
        }
    }
}

/// The island's report of where its mascot is, in points.
#[tauri::command]
pub fn island_gaze_origin(x: f64, y: f64) {
    let fits = |v: f64| v.is_finite() && (0.0..=10_000.0).contains(&v);
    if fits(x) && fits(y) {
        *ORIGIN.lock().unwrap_or_else(|e| e.into_inner()) = Some((x, y));
    }
}

pub fn spawn(app: AppHandle) {
    // Its own thread: each read waits on the main thread, which an async
    // worker must not do.
    let started = std::thread::Builder::new()
        .name("cortex-gaze".into())
        .spawn(move || watch(&app));
    if let Err(err) = started {
        eprintln!("cortex: the lively mascot's eyes did not start: {err}");
    }
}

fn watch(app: &AppHandle) {
    let mut last: Option<(Gaze, bool)> = None;
    let mut last_sent = Instant::now();
    let mut cursor_was: Option<(f64, f64)> = None;
    let mut main_scale = 1.0;
    let mut ticks: u32 = 0;
    let mut locked = false;
    loop {
        if !LIVELY.load(Ordering::Relaxed) {
            last = None;
            cursor_was = None;
            std::thread::sleep(IDLE);
            continue;
        }
        let slow = ticks.is_multiple_of(SLOW_EVERY);
        ticks = ticks.wrapping_add(1);
        // Once locked, the lock is looked at on every pass until it lifts.
        if slow || locked {
            locked = screen_locked();
            if locked {
                send(app, &mut last, &mut last_sent, Gaze::AHEAD, false);
                std::thread::sleep(IDLE);
                continue;
            }
            if let Ok(Some(main)) = app.primary_monitor() {
                main_scale = main.scale_factor();
            }
        }
        std::thread::sleep(TICK);
        let Some(cursor) = app.cursor_position().ok().filter(readable) else {
            send(app, &mut last, &mut last_sent, Gaze::AHEAD, true);
            std::thread::sleep(IDLE);
            continue;
        };
        let at = (cursor.x, cursor.y);
        let moved = cursor_was != Some(at);
        if !moved && !slow && last.is_some_and(|(_, awake)| awake) {
            continue;
        }
        let Some(g) = look_from_island(app, at, main_scale) else {
            continue;
        };
        cursor_was = Some(at);
        if moved && last_sent.elapsed() >= STILL_MOVING {
            last = None;
        }
        send(app, &mut last, &mut last_sent, g, true);
    }
}

fn send(
    app: &AppHandle,
    last: &mut Option<(Gaze, bool)>,
    last_sent: &mut Instant,
    g: Gaze,
    awake: bool,
) {
    if *last == Some((g, awake)) {
        return;
    }
    *last = Some((g, awake));
    *last_sent = Instant::now();
    let _ = app.emit_to("island", GAZE_EVENT, GazeEvent::new(g, awake));
}

/// Wayland gives no cursor outside the window and says (0, 0).
fn readable(cursor: &PhysicalPosition<f64>) -> bool {
    !(cfg!(target_os = "linux") && cursor.x == 0.0 && cursor.y == 0.0)
}

/// The cursor's direction from the mascot.
fn look_from_island(app: &AppHandle, cursor: (f64, f64), main_scale: f64) -> Option<Gaze> {
    let island = app.get_webview_window("island")?;
    let pos = island.outer_position().ok()?;
    let size = island.outer_size().ok()?;
    let scale = island.scale_factor().ok()?;
    if !(scale.is_finite() && scale > 0.0) {
        return None;
    }
    let origin = *ORIGIN.lock().unwrap_or_else(|e| e.into_inner());
    let (ox, oy) = origin.unwrap_or((
        f64::from(size.width) / scale / 2.0,
        f64::from(size.height) / scale / 2.0,
    ));
    if cfg!(target_os = "macos") {
        // tao turns the cursor into pixels with the main screen's scale and
        // the window with its own: both back to points, or a second screen
        // with another scale would skew the direction.
        let main = if main_scale.is_finite() && main_scale > 0.0 {
            main_scale
        } else {
            1.0
        };
        let center = (f64::from(pos.x) / scale + ox, f64::from(pos.y) / scale + oy);
        Some(gaze((cursor.0 / main, cursor.1 / main), center, 1.0))
    } else {
        let center = (f64::from(pos.x) + ox * scale, f64::from(pos.y) + oy * scale);
        Some(gaze(cursor, center, scale))
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
    // borrowed from the dictionary, which outlives its use. Reading immutable
    // Core Foundation objects is safe from any thread.
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
