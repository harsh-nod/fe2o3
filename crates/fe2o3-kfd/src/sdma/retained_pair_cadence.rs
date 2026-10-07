//! Explicit retained-wait experiment; ordinary entrypoints retain their policy.

use std::time::{Duration, Instant};

use crate::wait::MonotonicWaitV1;

macro_rules! cadence_rust_items {
    ($($item:item)*) => { $($item)* };
}

include!("retained_pair_cadence_declarations.rs");
include!("retained_pair_cadence_body.rs");
retained_pair_cadence_declarations_v1!(cadence_rust_items);

impl Gfx942XgmiRetainedWaitCadenceV1 {
    /// Maximum requested sleep, not a bound on actual scheduler latency.
    pub const fn sleep_ceiling_ns(self) -> u64 {
        let cadence = self;
        retained_pair_cadence_ceiling_body_v1!(cadence)
    }

    #[cfg(any(feature = "hardware-diagnostic", test))]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Ordinary1ms => "ordinary-1ms",
            Self::Ceiling25us => "ceiling-25us",
        }
    }

    pub(super) fn cursor(self, deadline: Instant) -> MonotonicWaitV1 {
        match self {
            Self::Ordinary1ms => MonotonicWaitV1::until(deadline),
            Self::Ceiling25us => MonotonicWaitV1::until_with_sleep_ceiling(
                deadline,
                Duration::from_nanos(self.sleep_ceiling_ns()),
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selector_has_exact_closed_positive_ceilings() {
        assert_eq!(
            Gfx942XgmiRetainedWaitCadenceV1::Ordinary1ms.sleep_ceiling_ns(),
            1_000_000
        );
        assert_eq!(
            Gfx942XgmiRetainedWaitCadenceV1::Ceiling25us.sleep_ceiling_ns(),
            25_000
        );
        assert_eq!(
            Gfx942XgmiRetainedWaitCadenceV1::Ordinary1ms.name(),
            "ordinary-1ms"
        );
        assert_eq!(
            Gfx942XgmiRetainedWaitCadenceV1::Ceiling25us.name(),
            "ceiling-25us"
        );
    }

    #[test]
    fn both_cursors_observe_before_an_already_expired_deadline() {
        for cadence in [
            Gfx942XgmiRetainedWaitCadenceV1::Ordinary1ms,
            Gfx942XgmiRetainedWaitCadenceV1::Ceiling25us,
        ] {
            for ready in [false, true] {
                let mut cursor = cadence.cursor(Instant::now());
                assert!(cursor.expired());
                let mut observations = 0;
                assert_eq!(
                    cursor.observe_until_ready(|| {
                        observations += 1;
                        Ok::<_, ()>(ready)
                    }),
                    Ok(ready)
                );
                assert_eq!(observations, 1);
            }
        }
    }

    #[test]
    fn both_cursors_preserve_observation_errors_without_a_pause() {
        for cadence in [
            Gfx942XgmiRetainedWaitCadenceV1::Ordinary1ms,
            Gfx942XgmiRetainedWaitCadenceV1::Ceiling25us,
        ] {
            let mut cursor = cadence.cursor(Instant::now());
            let mut observations = 0;
            assert_eq!(
                cursor.observe_until_ready(|| {
                    observations += 1;
                    Err::<bool, _>("injected")
                }),
                Err("injected")
            );
            assert_eq!(observations, 1);
        }
    }
}
