//! Bounded idle backoff; this changes no traced-process admission decision.

use super::POLL_INTERVAL;
use std::time::Duration;

const FAST_POLLS: u8 = 4;
const FAST_INTERVAL: Duration = Duration::from_micros(50);

#[derive(Default)]
pub(super) struct ActivePoll {
    empty_scans: u8,
}

impl ActivePoll {
    pub(super) fn after_scan(&mut self, progressed: bool) -> Option<Duration> {
        if progressed {
            self.empty_scans = 0;
            return None;
        }
        // Promptly repeated ptrace stops should not each pay the coarse idle
        // delay. A quiet solver returns to the existing interval after four polls.
        Some(if self.empty_scans < FAST_POLLS {
            self.empty_scans += 1;
            FAST_INTERVAL
        } else {
            POLL_INTERVAL
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn initial_burst_is_bounded_and_idle_counter_cannot_wrap() {
        let mut poll = ActivePoll::default();
        for index in 0..65_536 {
            let expected = if index < usize::from(FAST_POLLS) {
                FAST_INTERVAL
            } else {
                POLL_INTERVAL
            };
            let delay = poll.after_scan(false).unwrap();
            assert_eq!(delay, expected);
            assert!(!delay.is_zero() && delay <= POLL_INTERVAL);
        }
        assert_eq!(poll.empty_scans, FAST_POLLS);
    }

    #[test]
    fn progress_after_idle_restores_exactly_one_fast_burst() {
        let mut poll = ActivePoll::default();
        for _ in 0..10 {
            assert!(poll.after_scan(false).is_some());
        }
        assert_eq!(poll.after_scan(true), None);
        for _ in 0..FAST_POLLS {
            assert_eq!(poll.after_scan(false), Some(FAST_INTERVAL));
        }
        assert_eq!(poll.after_scan(false), Some(POLL_INTERVAL));
    }

    #[test]
    fn progress_partway_through_a_burst_resets_without_sleeping() {
        let mut poll = ActivePoll::default();
        for count in 0..FAST_POLLS {
            for _ in 0..count {
                assert_eq!(poll.after_scan(false), Some(FAST_INTERVAL));
            }
            assert_eq!(poll.after_scan(true), None);
            assert_eq!(poll.empty_scans, 0);
        }
    }

    #[test]
    fn consecutive_progress_never_sleeps() {
        let mut poll = ActivePoll::default();
        for _ in 0..1_024 {
            assert_eq!(poll.after_scan(true), None);
        }
        assert_eq!(poll.after_scan(false), Some(FAST_INTERVAL));
    }

    #[test]
    fn each_proof_has_its_own_backoff() {
        let mut idle = ActivePoll::default();
        for _ in 0..FAST_POLLS {
            assert_eq!(idle.after_scan(false), Some(FAST_INTERVAL));
        }
        let mut fresh = ActivePoll::default();
        assert_eq!(fresh.after_scan(false), Some(FAST_INTERVAL));
        assert_eq!(idle.after_scan(false), Some(POLL_INTERVAL));
        assert_eq!(fresh.after_scan(true), None);
        assert_eq!(idle.after_scan(false), Some(POLL_INTERVAL));
    }
}
