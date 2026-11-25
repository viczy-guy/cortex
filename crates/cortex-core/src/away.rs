//! Whether anybody is looking (#157). While nobody is (the screen locked, ten
//! minutes without the mouse or the keyboard, or the computer asleep), the
//! island rings nothing and keeps what comes for a summary once somebody is
//! back. The shell asks the system; everything here only decides.

/// Ten minutes without the mouse or the keyboard: away. The lively mascot
/// naps after the same time.
pub const IDLE_SECS: u64 = 10 * 60;
/// The shell looks this often: a wall clock that jumped further between two
/// looks means the computer slept.
pub const TICK_SECS: u64 = 2;
/// How often the system is asked while somebody is there; while nobody is,
/// on every tick, so coming back shows at once.
pub const ASK_SECS: u64 = 15;
/// More than this on the wall clock between two ticks: the computer slept.
pub const SLEPT_SECS: u64 = 60;
/// After a sleep, how long coming back waits for GitHub's first answer, so
/// what came meanwhile is in the summary rather than rung one by one.
pub const WAKE_WAIT_SECS: u64 = 60;

/// What the system said when last asked.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct System {
    pub locked: bool,
    /// Seconds since the last mouse or keyboard input; 0 where unknown.
    pub idle_secs: u64,
}

/// One tick's outcome.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Tick {
    /// Away (`true`) or back (`false`), when that changed.
    pub changed: Option<bool>,
    /// The computer just woke: the shell asks GitHub at once.
    pub woke: bool,
}

#[derive(Debug, Default)]
pub struct Presence {
    away: bool,
    last_tick_ms: Option<u64>,
    asked_ms: Option<u64>,
    system: System,
    /// When the computer woke, while coming back waits for GitHub.
    woke_ms: Option<u64>,
}

impl Presence {
    pub fn away(&self) -> bool {
        self.away
    }

    /// Whether this tick asks the system.
    pub fn asks(&self, now_ms: u64) -> bool {
        self.away
            || self
                .asked_ms
                .is_none_or(|at| now_ms.saturating_sub(at) >= ASK_SECS * 1000)
    }

    /// One tick, every [`TICK_SECS`]: `system` is what the system said, when
    /// [`Presence::asks`]; `answered`, whether GitHub answered since the
    /// last wake (or is off).
    pub fn tick(&mut self, now_ms: u64, system: Option<System>, answered: bool) -> Tick {
        let woke = self
            .last_tick_ms
            .is_some_and(|last| now_ms.saturating_sub(last) > SLEPT_SECS * 1000);
        self.last_tick_ms = Some(now_ms);
        if let Some(system) = system {
            self.system = system;
            self.asked_ms = Some(now_ms);
        }
        if woke {
            self.woke_ms = Some(now_ms);
        }
        let waiting = self
            .woke_ms
            .is_some_and(|at| !answered && now_ms.saturating_sub(at) < WAKE_WAIT_SECS * 1000);
        if !waiting {
            self.woke_ms = None;
        }
        let away = woke || waiting || self.system.locked || self.system.idle_secs >= IDLE_SECS;
        let changed = (away != self.away).then_some(away);
        self.away = away;
        Tick { changed, woke }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const S: u64 = 1000;

    fn system(locked: bool, idle_secs: u64) -> Option<System> {
        Some(System { locked, idle_secs })
    }

    #[test]
    fn somebody_at_the_mouse_is_there_from_the_first_tick() {
        let mut p = Presence::default();
        assert!(p.asks(0));
        assert_eq!(p.tick(0, system(false, 3), true).changed, None);
        assert!(!p.away());
    }

    #[test]
    fn a_locked_screen_or_ten_minutes_idle_is_away_and_a_move_is_back() {
        let mut p = Presence::default();
        p.tick(0, system(false, 0), true);
        assert_eq!(p.tick(2 * S, system(true, 2), true).changed, Some(true));
        assert_eq!(p.tick(4 * S, system(false, 0), true).changed, Some(false));
        assert_eq!(
            p.tick(6 * S, system(false, IDLE_SECS - 1), true).changed,
            None
        );
        assert_eq!(
            p.tick(8 * S, system(false, IDLE_SECS), true).changed,
            Some(true)
        );
        assert_eq!(p.tick(10 * S, system(false, 1), true).changed, Some(false));
    }

    #[test]
    fn the_system_is_asked_every_15_s_while_somebody_is_there_and_on_every_tick_while_nobody_is() {
        let mut p = Presence::default();
        p.tick(0, system(false, 0), true);
        assert!(!p.asks(2 * S));
        // Not asked: what it said last stands.
        assert_eq!(p.tick(2 * S, None, true).changed, None);
        assert!(p.asks(ASK_SECS * S));
        p.tick(ASK_SECS * S, system(true, 0), true);
        assert!(p.asks((ASK_SECS + 2) * S));
    }

    #[test]
    fn a_jump_of_the_wall_clock_is_a_sleep_and_coming_back_waits_for_github() {
        let mut p = Presence::default();
        p.tick(0, system(false, 0), true);
        let night = 8 * 3600 * S;
        let woke = p.tick(night, system(false, 1), false);
        assert_eq!(
            woke,
            Tick {
                changed: Some(true),
                woke: true
            }
        );
        // At the keyboard already, but GitHub has not answered yet.
        assert_eq!(p.tick(night + 2 * S, system(false, 0), false).changed, None);
        assert_eq!(
            p.tick(night + 4 * S, system(false, 0), true).changed,
            Some(false)
        );
        // Later answers no longer matter.
        assert_eq!(p.tick(night + 6 * S, system(false, 0), false).changed, None);
    }

    #[test]
    fn a_wake_waits_at_most_a_minute_and_a_lock_keeps_it_away_after() {
        let mut p = Presence::default();
        p.tick(0, system(false, 0), true);
        let wake = 2 * 3600 * S;
        assert!(p.tick(wake, system(true, 0), false).woke);
        assert_eq!(
            p.tick(wake + (WAKE_WAIT_SECS - 1) * S, system(false, 0), false)
                .changed,
            None
        );
        assert_eq!(
            p.tick(wake + WAKE_WAIT_SECS * S, system(false, 0), false)
                .changed,
            Some(false)
        );
        let wake = wake + 3600 * S;
        p.tick(wake, system(true, 0), true);
        // GitHub answered, yet the screen is still locked.
        assert_eq!(p.tick(wake + 2 * S, system(true, 0), true).changed, None);
        assert!(p.away());
    }

    #[test]
    fn a_short_gap_or_a_clock_set_back_is_no_sleep() {
        let mut p = Presence::default();
        p.tick(100 * S, system(false, 0), true);
        assert!(!p.tick(100 * S + SLEPT_SECS * S, None, true).woke);
        assert!(!p.tick(5 * S, None, true).woke);
        assert!(!p.away());
    }
}
