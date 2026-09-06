//! Monotonic, bounded waiting shared by native completion paths.

use std::time::{Duration, Instant};

const SPIN_ATTEMPTS_V1: u32 = 64;
const YIELD_ATTEMPTS_V1: u32 = 16;
const INITIAL_SLEEP_V1: Duration = Duration::from_micros(25);
const MAX_SLEEP_V1: Duration = Duration::from_millis(1);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum WaitActionV1 {
    Spin,
    Yield,
    Sleep(Duration),
}

/// A process-local wait cursor with an optional monotonic deadline.
///
/// The first observations retain low-latency spin behavior. Sustained waits
/// yield briefly and then sleep with bounded exponential backoff, preventing a
/// missing GPU completion from monopolizing a host core.
pub(crate) struct MonotonicWaitV1 {
    deadline: Option<Instant>,
    active_spin_until: Option<Instant>,
    attempts: u32,
    next_sleep: Duration,
}

impl MonotonicWaitV1 {
    pub(crate) fn without_deadline() -> Self {
        Self {
            deadline: None,
            active_spin_until: None,
            attempts: 0,
            next_sleep: INITIAL_SLEEP_V1,
        }
    }

    pub(crate) fn until(deadline: Instant) -> Self {
        Self {
            deadline: Some(deadline),
            active_spin_until: None,
            attempts: 0,
            next_sleep: INITIAL_SLEEP_V1,
        }
    }

    pub(crate) fn until_with_active_spin_floor(
        deadline: Instant,
        active_spin_floor: Duration,
    ) -> Self {
        Self::until_with_active_spin_floor_from(Instant::now(), deadline, active_spin_floor)
    }

    fn until_with_active_spin_floor_from(
        started: Instant,
        deadline: Instant,
        active_spin_floor: Duration,
    ) -> Self {
        let active_spin_until = started
            .checked_add(active_spin_floor)
            .map_or(deadline, |spin_until| spin_until.min(deadline));
        Self {
            deadline: Some(deadline),
            active_spin_until: Some(active_spin_until),
            attempts: 0,
            next_sleep: INITIAL_SLEEP_V1,
        }
    }

    pub(crate) fn expired(&self) -> bool {
        self.deadline
            .is_some_and(|deadline| Instant::now() >= deadline)
    }

    fn next_action_at(&mut self, now: Instant) -> WaitActionV1 {
        self.attempts = self.attempts.saturating_add(1);
        if self
            .active_spin_until
            .is_some_and(|active_spin_until| now < active_spin_until)
        {
            return WaitActionV1::Spin;
        }
        if self.attempts <= SPIN_ATTEMPTS_V1 {
            return WaitActionV1::Spin;
        }
        if self.attempts <= SPIN_ATTEMPTS_V1 + YIELD_ATTEMPTS_V1 {
            return WaitActionV1::Yield;
        }
        let sleep = self
            .deadline
            .map(|deadline| deadline.saturating_duration_since(now))
            .map_or(self.next_sleep, |remaining| remaining.min(self.next_sleep));
        self.next_sleep = self.next_sleep.saturating_mul(2).min(MAX_SLEEP_V1);
        WaitActionV1::Sleep(sleep)
    }

    fn next_action(&mut self) -> WaitActionV1 {
        self.next_action_at(Instant::now())
    }

    pub(crate) fn pause(&mut self) {
        match self.next_action() {
            WaitActionV1::Spin => core::hint::spin_loop(),
            WaitActionV1::Yield => std::thread::yield_now(),
            WaitActionV1::Sleep(duration) if !duration.is_zero() => std::thread::sleep(duration),
            WaitActionV1::Sleep(_) => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sustained_waits_progress_from_spin_to_yield_to_bounded_sleep() {
        let mut wait = MonotonicWaitV1::without_deadline();
        for _ in 0..SPIN_ATTEMPTS_V1 {
            assert_eq!(wait.next_action(), WaitActionV1::Spin);
        }
        for _ in 0..YIELD_ATTEMPTS_V1 {
            assert_eq!(wait.next_action(), WaitActionV1::Yield);
        }
        assert_eq!(wait.next_action(), WaitActionV1::Sleep(INITIAL_SLEEP_V1));
        assert_eq!(
            wait.next_action(),
            WaitActionV1::Sleep(INITIAL_SLEEP_V1 * 2)
        );
        for _ in 0..32 {
            let WaitActionV1::Sleep(duration) = wait.next_action() else {
                panic!("backoff returned to an active wait")
            };
            assert!(duration <= MAX_SLEEP_V1);
        }
    }

    #[test]
    fn deadline_without_floor_preserves_the_default_action_sequence() {
        let started = Instant::now();
        let mut wait = MonotonicWaitV1::until(started + Duration::from_secs(1));
        for _ in 0..SPIN_ATTEMPTS_V1 {
            assert_eq!(wait.next_action_at(started), WaitActionV1::Spin);
        }
        for _ in 0..YIELD_ATTEMPTS_V1 {
            assert_eq!(wait.next_action_at(started), WaitActionV1::Yield);
        }
        assert_eq!(
            wait.next_action_at(started),
            WaitActionV1::Sleep(INITIAL_SLEEP_V1)
        );
    }

    #[test]
    fn zero_deadline_is_immediately_expired() {
        let wait = MonotonicWaitV1::until(Instant::now());
        assert!(wait.expired());
    }

    #[test]
    fn active_spin_floor_is_elapsed_and_clamped_to_deadline() {
        let started = Instant::now();
        let deadline = started + Duration::from_micros(100);
        let mut wait = MonotonicWaitV1::until_with_active_spin_floor_from(
            started,
            deadline,
            Duration::from_micros(50),
        );

        for _ in 0..SPIN_ATTEMPTS_V1 {
            assert_eq!(
                wait.next_action_at(started + Duration::from_micros(49)),
                WaitActionV1::Spin
            );
        }
        assert_eq!(wait.attempts, SPIN_ATTEMPTS_V1);
        assert_eq!(
            wait.next_action_at(started + Duration::from_micros(50)),
            WaitActionV1::Yield
        );
        assert_eq!(wait.attempts, SPIN_ATTEMPTS_V1 + 1);

        let clamped = MonotonicWaitV1::until_with_active_spin_floor_from(
            started,
            deadline,
            Duration::from_micros(200),
        );
        assert_eq!(clamped.active_spin_until, Some(deadline));
    }
}
