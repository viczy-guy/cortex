//! Is anybody looking (#157)? A thread of its own looks every 2 seconds: the
//! wall clock tells it when the computer slept, and the system says whether
//! the screen is locked and how long the mouse and the keyboard rested. The
//! island hears only when that changes, and asks once at launch.

use cortex_core::away::{Presence, System, TICK_SECS};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager};

use crate::github::GithubState;

pub const AWAY_EVENT: &str = "cortex://away";
static AWAY: AtomicBool = AtomicBool::new(false);

/// Whether nobody is looking now: the island asks at launch, in case the
/// change came before it listened.
#[tauri::command]
pub fn away_now() -> bool {
    AWAY.load(Ordering::Relaxed)
}

pub fn spawn(app: AppHandle) {
    let started = std::thread::Builder::new()
        .name("cortex-away".into())
        .spawn(move || watch(&app));
    if let Err(err) = started {
        eprintln!("cortex: the island will not know when nobody is looking: {err}");
    }
}

fn watch(app: &AppHandle) {
    let mut presence = Presence::default();
    loop {
        std::thread::sleep(Duration::from_secs(TICK_SECS));
        let system = System {
            locked: crate::gaze::screen_locked(),
            idle_secs: idle_secs(),
        };
        let github = app.state::<GithubState>();
        let tick = presence.tick(crate::state::now_ms(), system, github.answered_ms());
        if let Some(away) = tick.changed {
            AWAY.store(away, Ordering::Relaxed);
            let _ = app.emit_to("island", AWAY_EVENT, away);
        }
        // Only once the island knows: an answer must not reach it first.
        if tick.woke {
            github.refresh();
        }
    }
}

/// Seconds since the last mouse or keyboard input in this login session.
#[cfg(target_os = "macos")]
fn idle_secs() -> u64 {
    #[link(name = "CoreGraphics", kind = "framework")]
    extern "C" {
        fn CGEventSourceSecondsSinceLastEventType(state: i32, kind: u32) -> f64;
    }
    const COMBINED_SESSION_STATE: i32 = 0;
    const ANY_INPUT: u32 = !0;
    // SAFETY: a plain query with constant arguments; no pointers involved.
    let secs = unsafe { CGEventSourceSecondsSinceLastEventType(COMBINED_SESSION_STATE, ANY_INPUT) };
    if secs.is_finite() && secs > 0.0 {
        secs as u64
    } else {
        0
    }
}

#[cfg(windows)]
fn idle_secs() -> u64 {
    #[repr(C)]
    struct LastInputInfo {
        size: u32,
        time: u32,
    }
    #[link(name = "user32")]
    extern "system" {
        fn GetLastInputInfo(info: *mut LastInputInfo) -> i32;
    }
    #[link(name = "kernel32")]
    extern "system" {
        fn GetTickCount() -> u32;
    }
    let mut info = LastInputInfo {
        size: std::mem::size_of::<LastInputInfo>() as u32,
        time: 0,
    };
    // SAFETY: `info` is a live, correctly sized LASTINPUTINFO for the call.
    if unsafe { GetLastInputInfo(&mut info) } == 0 {
        return 0;
    }
    // Both are milliseconds since boot, wrapping every 49.7 days.
    // SAFETY: no arguments.
    u64::from(unsafe { GetTickCount() }.wrapping_sub(info.time)) / 1000
}

// shortcut: no idle time on Linux (it depends on the desktop: X11, Wayland,
// GNOME, KDE...), so only a sleep or a closed app count there; add one per
// desktop if Linux users ask.
#[cfg(not(any(target_os = "macos", windows)))]
fn idle_secs() -> u64 {
    0
}
