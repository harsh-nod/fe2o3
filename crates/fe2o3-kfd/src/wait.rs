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
    attempts: u32,
    next_sleep: Duration,
}

impl MonotonicWaitV1 {
    pub(crate) fn without_deadline() -> Self {
        Self {
            deadline: None,
            attempts: 0,
            next_sleep: INITIAL_SLEEP_V1,
        }
    }

    pub(crate) fn until(deadline: Instant) -> Self {
        Self {
            deadline: Some(deadline),
            attempts: 0,
            next_sleep: INITIAL_SLEEP_V1,
        }
    }

    pub(crate) fn expired(&self) -> bool {
        self.deadline
            .is_some_and(|deadline| Instant::now() >= deadline)
    }

    fn next_action(&mut self) -> WaitActionV1 {
        self.attempts = self.attempts.saturating_add(1);
        if self.attempts <= SPIN_ATTEMPTS_V1 {
            return WaitActionV1::Spin;
        }
        if self.attempts <= SPIN_ATTEMPTS_V1 + YIELD_ATTEMPTS_V1 {
            return WaitActionV1::Yield;
        }
        let sleep = self
            .deadline
            .map(|deadline| deadline.saturating_duration_since(Instant::now()))
            .map_or(self.next_sleep, |remaining| remaining.min(self.next_sleep));
        self.next_sleep = self.next_sleep.saturating_mul(2).min(MAX_SLEEP_V1);
        WaitActionV1::Sleep(sleep)
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
    fn zero_deadline_is_immediately_expired() {
        let wait = MonotonicWaitV1::until(Instant::now());
        assert!(wait.expired());
    }
}
