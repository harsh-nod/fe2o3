//! One original-start monotonic clock; cleanup has a separate bounded window.
use fe2o3_private_one_stop_protocol::{DEADLINE_NS, Refusal};
use std::time::{Duration, Instant};
#[derive(Clone, Copy)]
pub(super) struct Clock {
    started: Instant,
}
impl Clock {
    pub(super) fn start() -> Self {
        Self {
            started: Instant::now(),
        }
    }
    pub(super) fn elapsed(&self) -> Result<u64, Refusal> {
        u64::try_from(self.started.elapsed().as_nanos()).map_err(|_| Refusal::Deadline)
    }
    pub(super) fn check(&self) -> Result<(), Refusal> {
        check_ns(self.elapsed()?)
    }
    pub(super) fn deadline(&self) -> Instant {
        self.started + Duration::from_nanos(DEADLINE_NS)
    }
}
pub(super) fn check_ns(ns: u64) -> Result<(), Refusal> {
    if ns >= DEADLINE_NS {
        Err(Refusal::Deadline)
    } else {
        Ok(())
    }
}
pub(super) fn before(deadline: Instant) -> Result<(), Refusal> {
    if Instant::now() >= deadline {
        Err(Refusal::Deadline)
    } else {
        Ok(())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn original_deadline_never_accepts_late_positive() {
        assert!(check_ns(DEADLINE_NS - 1).is_ok());
        assert_eq!(check_ns(DEADLINE_NS), Err(Refusal::Deadline));
        assert_eq!(check_ns(DEADLINE_NS + 1), Err(Refusal::Deadline));
    }
    #[test]
    fn already_expired_completion_does_not_gain_a_new_window() {
        let clock = Clock {
            started: Instant::now() - Duration::from_secs(61),
        };
        assert!(clock.check().is_err());
        assert!(before(clock.deadline()).is_err());
    }
}
