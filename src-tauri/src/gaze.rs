//! The lively mascot's eyes (#145). While the mascot is lively, a thread of its
//! own reads the cursor (ten times a second while it moves, less once it
//! rests) and tells the island only when the direction changes, plus once a
//! minute while the mouse keeps moving (so a busy mouse is never taken for a
//! nap). On macOS a locked screen pauses it all: any repaint of a locked
//! island keeps WebKit busy until unlock (#51).

use cortex_core::gaze::{gaze, Gaze, STEPS_X, STEPS_Y};
use cortex_core::prefs::Prefs;
use serde::Serialize;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, Manager};

pub const GAZE_EVENT: &str = "cortex://gaze";
static LIVELY: AtomicBool = AtomicBool::new(true);
/// Where the mascot sits in the island, in points from its top left corner;
/// the island says so when that changes (`None`: the window's center).
static ORIGIN: Mutex<Option<(f64, f64)>> = Mutex::new(None);
/// The island's window moved, resized or changed scale: it is read again.
static RESHAPED: AtomicBool = AtomicBool::new(true);
const TICK: Duration = Duration::from_millis(100);
/// A cursor still for `REST_AFTER` is read less often; a move then shows a
/// little later, while a resting mouse costs less.
const REST_AFTER: Duration = Duration::from_secs(3);
const RESTING_TICK: Duration = Duration::from_millis(400);
/// While paused (not lively, no mascot, screen locked, no cursor), how often to
/// look again.
const IDLE: Duration = Duration::from_secs(2);
/// How often to look at the lock. The direction is also worked out again
/// then, in case the island moved under a resting cursor.
const SLOW: Duration = Duration::from_secs(1);
/// The island's window is read again on its own events, and at least this
/// often in case one went missing.
const FRAME_EVERY: Duration = Duration::from_secs(10);
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
    RESHAPED.store(true, Ordering::Relaxed);
}

pub fn spawn(app: AppHandle) {
    if let Some(island) = app.get_webview_window("island") {
        island.on_window_event(|event| {
            if matches!(
                event,
                tauri::WindowEvent::Moved(_)
                    | tauri::WindowEvent::Resized(_)
                    | tauri::WindowEvent::ScaleFactorChanged { .. }
            ) {
                RESHAPED.store(true, Ordering::Relaxed);
            }
        });
    }
    // Its own thread: reading the window waits on the main thread, which an
    // async worker must not do.
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
    let mut last_slow: Option<Instant> = None;
    let mut last_motion = Instant::now();
    let mut locked = false;
    let mut island: Option<Frame> = None;
    let mut framed_at = Instant::now();
    loop {
        if !LIVELY.load(Ordering::Relaxed) {
            last = None;
            cursor_was = None;
            island = None;
            std::thread::sleep(IDLE);
            continue;
        }
        let slow = last_slow.is_none_or(|at| at.elapsed() >= SLOW);
        if slow {
            last_slow = Some(Instant::now());
        }
        // Once locked, the lock is looked at on every pass until it lifts.
        if slow || locked {
            locked = screen_locked();
            if locked {
                send(app, &mut last, &mut last_sent, Gaze::AHEAD, false);
                std::thread::sleep(IDLE);
                continue;
            }
        }
        if RESHAPED.swap(false, Ordering::Relaxed) || framed_at.elapsed() >= FRAME_EVERY {
            island = None;
        }
        std::thread::sleep(if last_motion.elapsed() >= REST_AFTER {
            RESTING_TICK
        } else {
            TICK
        });
        let Some(at) = cursor(app) else {
            send(app, &mut last, &mut last_sent, Gaze::AHEAD, true);
            std::thread::sleep(IDLE);
            continue;
        };
        let moved = cursor_was != Some(at);
        if moved {
            last_motion = Instant::now();
        }
        if !moved && !slow && last.is_some_and(|(_, awake)| awake) {
            continue;
        }
        if island.is_none() {
            island = island_frame(app);
            framed_at = Instant::now();
        }
        let Some(g) = island.map(|frame| look_from(frame, at)) else {
            // No window to look from yet: wait rather than ask the main
            // thread again on every pass.
            std::thread::sleep(IDLE);
            continue;
        };
        cursor_was = Some(at);
        if moved && last_sent.elapsed() >= STILL_MOVING {
            last = None;
        }
        // The screen may have locked since the last look: a new direction
        // would repaint a locked island (#51).
        if last != Some((g, true)) && screen_locked() {
            locked = true;
            send(app, &mut last, &mut last_sent, Gaze::AHEAD, false);
            continue;
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

/// The cursor in points, straight from Core Graphics: no wait on the main
/// thread, which `AppHandle::cursor_position` would wake on every read.
#[cfg(target_os = "macos")]
fn cursor(_app: &AppHandle) -> Option<(f64, f64)> {
    use std::ffi::c_void;
    #[repr(C)]
    struct CGPoint {
        x: f64,
        y: f64,
    }
    #[link(name = "CoreGraphics", kind = "framework")]
    extern "C" {
        fn CGEventCreate(source: *const c_void) -> *const c_void;
        fn CGEventGetLocation(event: *const c_void) -> CGPoint;
    }
    #[link(name = "CoreFoundation", kind = "framework")]
    extern "C" {
        fn CFRelease(cf: *const c_void);
    }
    // SAFETY: the event comes from a Create function, is checked for null,
    // read once and released once; Core Graphics events are usable from any
    // thread.
    let point = unsafe {
        let event = CGEventCreate(std::ptr::null());
        if event.is_null() {
            return None;
        }
        let point = CGEventGetLocation(event);
        CFRelease(event);
        point
    };
    (point.x.is_finite() && point.y.is_finite()).then_some((point.x, point.y))
}

/// The cursor in physical pixels. Wayland gives none outside the window and
/// says (0, 0).
#[cfg(not(target_os = "macos"))]
fn cursor(app: &AppHandle) -> Option<(f64, f64)> {
    let at = app.cursor_position().ok()?;
    let unknown = cfg!(target_os = "linux") && at.x == 0.0 && at.y == 0.0;
    (!unknown).then_some((at.x, at.y))
}

/// The island window: where it is, its size and its scale, read again only
/// when it changes (each read waits on the main thread).
#[derive(Clone, Copy)]
struct Frame {
    x: f64,
    y: f64,
    width: f64,
    height: f64,
    scale: f64,
}

fn island_frame(app: &AppHandle) -> Option<Frame> {
    let island = app.get_webview_window("island")?;
    let pos = island.outer_position().ok()?;
    let size = island.outer_size().ok()?;
    let scale = island.scale_factor().ok()?;
    (scale.is_finite() && scale > 0.0).then(|| Frame {
        x: f64::from(pos.x),
        y: f64::from(pos.y),
        width: f64::from(size.width),
        height: f64::from(size.height),
        scale,
    })
}

/// The cursor's direction from the mascot.
fn look_from(island: Frame, cursor: (f64, f64)) -> Gaze {
    let Frame {
        x,
        y,
        width,
        height,
        scale,
    } = island;
    let origin = *ORIGIN.lock().unwrap_or_else(|e| e.into_inner());
    let (ox, oy) = origin.unwrap_or((width / scale / 2.0, height / scale / 2.0));
    if cfg!(target_os = "macos") {
        // The cursor is in points: the window goes back to points too.
        gaze(cursor, (x / scale + ox, y / scale + oy), 1.0)
    } else {
        gaze(cursor, (x + ox * scale, y + oy * scale), scale)
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
