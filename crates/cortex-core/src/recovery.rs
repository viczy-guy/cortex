//! When the island cannot be drawn, the app restarts itself (#115). The copy
//! it starts carries the time of that restart, so it neither shows the
//! opening screen nor restarts again soon: a WebKit that cannot draw at all
//! must not loop. The argument outlives the moment (an update restarts with
//! the same arguments), so it is a time, not a flag.

/// Prefix of the argument; the restart time in Unix seconds follows.
pub const RECOVERED_ARG_PREFIX: &str = "--recovered-at=";

/// How long after a restart another one is refused.
pub const RECOVERY_QUIET_SECS: u64 = 600;

/// The argument for a restart now.
pub fn recovered_arg(now_secs: u64) -> String {
    format!("{RECOVERED_ARG_PREFIX}{now_secs}")
}

/// Started by a restart less than [`RECOVERY_QUIET_SECS`] ago. A time that
/// far ahead (the clock was corrected since) is stale too, not "recent"
/// until the clock catches up.
pub fn recovered_recently<S: AsRef<str>>(args: impl IntoIterator<Item = S>, now_secs: u64) -> bool {
    args.into_iter().any(|arg| {
        arg.as_ref()
            .strip_prefix(RECOVERED_ARG_PREFIX)
            .and_then(|secs| secs.parse::<u64>().ok())
            .is_some_and(|at| now_secs.abs_diff(at) < RECOVERY_QUIET_SECS)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_fresh_restart_is_recent_and_an_old_one_is_not() {
        let arg = recovered_arg(1_000);
        assert!(recovered_recently(["cortex", arg.as_str()], 1_000));
        assert!(recovered_recently(["cortex", arg.as_str()], 1_599));
        assert!(!recovered_recently(["cortex", arg.as_str()], 1_600));
    }

    #[test]
    fn other_or_malformed_arguments_are_not_a_restart() {
        assert!(!recovered_recently(["cortex", "--autostart"], 1_000));
        assert!(!recovered_recently(
            ["--recovered-at=", "--recovered-at=x"],
            1_000
        ));
        assert!(!recovered_recently(Vec::<String>::new(), 1_000));
    }

    #[test]
    fn a_small_clock_correction_keeps_it_recent_and_a_large_one_does_not() {
        assert!(recovered_recently([recovered_arg(1_300)], 1_000));
        assert!(!recovered_recently([recovered_arg(1_000 + 86_400)], 1_000));
    }

    #[test]
    fn an_argument_carried_by_a_later_update_restart_is_stale() {
        let arg = recovered_arg(1_000);
        assert!(!recovered_recently(
            ["cortex", arg.as_str()],
            1_000 + 3 * 86_400
        ));
    }
}
