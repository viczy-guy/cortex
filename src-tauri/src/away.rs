//! Is anybody looking (#157)? A thread of its own ticks every 2 seconds: the
//! wall clock tells it when the computer slept, and the system is asked about
//! the lock and the last input every 15 seconds (every tick while nobody is
//! there). The island hears only when that changes.

use cortex_core::away::{Presence, System, TICK_SECS};
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager};

use crate::github::GithubState;

pub const AWAY_EVENT: &str = "cortex://away";

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
    // GitHub's answers counted when the computer woke.
    let mut answers_at_wake = 0;
    loop {
        std::thread::sleep(Duration::from_secs(TICK_SECS));
        let now = crate::state::now_ms();
        let system = presence.asks(now).then(|| System {
            locked: crate::gaze::screen_locked(),
            idle_secs: idle_secs(),
        });
        let github = app.state::<GithubState>();
        let answers = github.answers();
        let tick = presence.tick(now, system, !github.enabled() || answers != answers_at_wake);
        if tick.woke {
            answers_at_wake = answers;
            github.refresh();
        }
        if let Some(away) = tick.changed {
            let _ = app.emit_to("island", AWAY_EVENT, away);
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
