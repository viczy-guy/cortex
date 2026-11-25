//! Whether anybody is looking (#157). While nobody is (the screen locked, ten
//! minutes without the mouse or the keyboard, or the computer asleep), the
//! island rings nothing and keeps what comes for a summary once somebody is
//! back. The shell asks the system; everything here only decides.

/// Ten minutes without the mouse or the keyboard: away. The lively mascot
/// naps after the same time.
pub const IDLE_SECS: u64 = 10 * 60;
/// The shell looks this often, and asks the system each time.
pub const TICK_SECS: u64 = 2;
/// More than this on the wall clock between two looks: the computer slept.
pub const SLEPT_SECS: u64 = 60;
/// After a sleep, how long coming back waits for a GitHub answer asked for
/// after the wake, so what came meanwhile is in the summary rather than rung
/// one by one.
pub const WAKE_WAIT_SECS: u64 = 60;

/// What the system said at this look.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct System {
    pub locked: bool,
    /// Seconds since the last mouse or keyboard input; 0 where unknown.
    pub idle_secs: u64,
}

/// One look's outcome.
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
    /// When the computer woke, while coming back waits for GitHub.
    woke_ms: Option<u64>,
}

impl Presence {
    pub fn away(&self) -> bool {
        self.away
    }

    /// One look, every [`TICK_SECS`]. `github`: when the last good GitHub
    /// answer was asked for (wall clock, ms; 0 for none yet), `None` while
    /// GitHub is off.
    pub fn tick(&mut self, now_ms: u64, system: System, github: Option<u64>) -> Tick {
        let last = self.last_tick_ms.replace(now_ms);
        let woke = last.is_some_and(|last| now_ms > last.saturating_add(SLEPT_SECS * 1000));
        if woke {
            self.woke_ms = Some(now_ms);
        }
        // Only an answer asked for since the wake counts (one stamped later
        // than now was asked before the clock went back); a clock set back
        // also ends the wait rather than stretching it.
        let waiting = self.woke_ms.is_some_and(|at| {
            now_ms >= at
                && now_ms - at < WAKE_WAIT_SECS * 1000
                && github.is_some_and(|asked| asked < at || asked > now_ms)
        });
        if !waiting {
            self.woke_ms = None;
        }
        let away = woke || waiting || system.locked || system.idle_secs >= IDLE_SECS;
        let changed = (away != self.away).then_some(away);
        self.away = away;
        Tick { changed, woke }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const S: u64 = 1000;

    fn system(locked: bool, idle_secs: u64) -> System {
        System { locked, idle_secs }
    }

    #[test]
    fn somebody_at_the_mouse_is_there_from_the_first_tick() {
        let mut p = Presence::default();
        assert_eq!(p.tick(0, system(false, 3), Some(0)).changed, None);
        assert!(!p.away());
    }

    #[test]
    fn a_locked_screen_or_ten_minutes_idle_is_away_and_a_move_is_back() {
        let mut p = Presence::default();
        p.tick(0, system(false, 0), None);
        assert_eq!(p.tick(2 * S, system(true, 2), None).changed, Some(true));
        assert_eq!(p.tick(4 * S, system(false, 0), None).changed, Some(false));
        assert_eq!(
            p.tick(6 * S, system(false, IDLE_SECS - 1), None).changed,
            None
        );
        assert_eq!(
            p.tick(8 * S, system(false, IDLE_SECS), None).changed,
            Some(true)
        );
        assert_eq!(p.tick(10 * S, system(false, 1), None).changed, Some(false));
    }

    #[test]
    fn a_jump_of_the_wall_clock_is_a_sleep_and_coming_back_waits_for_an_answer_asked_after_it() {
        let mut p = Presence::default();
        // GitHub answered many times before the night.
        p.tick(0, system(false, 0), Some(0));
        let night = 8 * 3600 * S;
        let woke = p.tick(night, system(false, 1), Some(10 * S));
        assert_eq!(
            woke,
            Tick {
                changed: Some(true),
                woke: true
            }
        );
        // At the keyboard already, but the answer asked for at once is not in.
        assert_eq!(
            p.tick(night + 2 * S, system(false, 0), Some(10 * S))
                .changed,
            None
        );
        assert_eq!(
            p.tick(night + 4 * S, system(false, 0), Some(night)).changed,
            Some(false)
        );
        // Later answers no longer matter.
        assert_eq!(
            p.tick(night + 6 * S, system(false, 0), Some(10 * S))
                .changed,
            None
        );
    }

    #[test]
    fn without_github_or_after_a_minute_coming_back_waits_no_more() {
        let mut p = Presence::default();
        p.tick(0, system(false, 0), None);
        let wake = 3600 * S;
        assert!(p.tick(wake, system(false, 0), None).woke);
        assert_eq!(
            p.tick(wake + 2 * S, system(false, 0), None).changed,
            Some(false)
        );
        let wake = wake + 3600 * S;
        p.tick(wake, system(false, 0), Some(0));
        let late = wake + (WAKE_WAIT_SECS - 1) * S;
        assert_eq!(p.tick(late, system(false, 0), Some(0)).changed, None);
        assert_eq!(
            p.tick(late + 2 * S, system(false, 0), Some(0)).changed,
            Some(false)
        );
    }

    #[test]
    fn an_answer_stamped_before_the_clock_went_back_does_not_count() {
        let mut p = Presence::default();
        p.tick(10 * 3600 * S, system(false, 0), Some(10 * 3600 * S));
        // The clock goes back two hours; GitHub fails from then on.
        p.tick(8 * 3600 * S, system(false, 0), Some(10 * 3600 * S));
        let wake = 9 * 3600 * S;
        assert!(p.tick(wake, system(false, 0), Some(10 * 3600 * S)).woke);
        assert_eq!(
            p.tick(wake + 2 * S, system(false, 0), Some(10 * 3600 * S))
                .changed,
            None
        );
        assert!(p.away());
    }

    #[test]
    fn an_answer_does_not_end_a_lock() {
        let mut p = Presence::default();
        p.tick(0, system(false, 0), Some(0));
        let wake = 3600 * S;
        p.tick(wake, system(true, 0), Some(0));
        assert_eq!(
            p.tick(wake + 2 * S, system(true, 0), Some(wake)).changed,
            None
        );
        assert!(p.away());
        assert_eq!(
            p.tick(wake + 4 * S, system(false, 0), Some(wake)).changed,
            Some(false)
        );
    }

    #[test]
    fn a_short_gap_is_no_sleep_and_a_clock_set_back_ends_the_wait() {
        let mut p = Presence::default();
        p.tick(100 * S, system(false, 0), Some(0));
        assert!(
            !p.tick(100 * S + SLEPT_SECS * S, system(false, 0), Some(0))
                .woke
        );
        assert!(!p.away());
        let wake = 3600 * S;
        assert!(p.tick(wake, system(false, 0), Some(0)).woke);
        assert_eq!(
            p.tick(wake - 600 * S, system(false, 0), Some(0)).changed,
            Some(false)
        );
    }
}
