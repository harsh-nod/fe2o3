//! Allocation-free host timing around the unchanged XGMI batch/poll operations.

use std::time::Instant;

/// Host-only attribution for one successful lower-level call. This is not a
/// device duration, completion certificate, or authorization to omit checks.
/// `None` means unavailable/invalid; preparation is also absent for polling.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[cfg(feature = "hardware-diagnostic")]
pub struct Gfx942XgmiCopyCallDiagnosticsV1 {
    pub opening_currentness_ns: Option<u64>,
    pub preparation_ns: Option<u64>,
    pub native_call_ns: Option<u64>,
    pub closing_currentness_ns: Option<u64>,
    pub total_ns: Option<u64>,
}

#[derive(Clone, Copy)]
pub(super) enum Phase {
    Opening,
    Preparation,
    Native,
    Closing,
}

/// Const-disabled instances have no clock reads, allocation, or callbacks.
pub(super) struct CallTimer<const ENABLED: bool> {
    elapsed: [Option<u64>; 4],
    visited: [bool; 4],
}

impl<const ENABLED: bool> CallTimer<ENABLED> {
    pub(super) const fn new() -> Self {
        Self {
            elapsed: [None; 4],
            visited: [false; 4],
        }
    }

    #[inline]
    pub(super) fn measure<T>(&mut self, phase: Phase, operation: impl FnOnce() -> T) -> T {
        if !ENABLED {
            return operation();
        }
        let start = Instant::now();
        let result = operation();
        self.record(phase, u64::try_from(start.elapsed().as_nanos()).ok());
        result
    }

    fn record(&mut self, phase: Phase, elapsed: Option<u64>) {
        let index = phase as usize;
        self.elapsed[index] = if self.visited[index] { None } else { elapsed };
        self.visited[index] = true;
    }

    #[cfg(feature = "hardware-diagnostic")]
    pub(super) fn finish(self, start: Instant) -> Gfx942XgmiCopyCallDiagnosticsV1 {
        Gfx942XgmiCopyCallDiagnosticsV1 {
            opening_currentness_ns: self.elapsed[Phase::Opening as usize],
            preparation_ns: self.elapsed[Phase::Preparation as usize],
            native_call_ns: self.elapsed[Phase::Native as usize],
            closing_currentness_ns: self.elapsed[Phase::Closing as usize],
            total_ns: u64::try_from(start.elapsed().as_nanos()).ok(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn disabled_timer_runs_each_operation_once_without_recording() {
        let mut timer = CallTimer::<false>::new();
        let mut calls = 0;
        let error = timer.measure(Phase::Opening, || {
            calls += 1;
            Err::<(), _>("original error")
        });
        assert_eq!(error, Err("original error"));
        assert_eq!(calls, 1);
        assert_eq!(timer.elapsed, [None; 4]);
        assert_eq!(timer.visited, [false; 4]);
    }

    #[test]
    fn enabled_timer_preserves_results_and_phase_order() {
        let mut timer = CallTimer::<true>::new();
        let mut order = Vec::new();
        for phase in [
            Phase::Opening,
            Phase::Preparation,
            Phase::Native,
            Phase::Closing,
        ] {
            assert_eq!(
                timer.measure(phase, || {
                    order.push(phase as usize);
                    17
                }),
                17
            );
        }
        assert_eq!(order, [0, 1, 2, 3]);
        assert!(timer.elapsed.iter().all(Option::is_some));
        assert_eq!(timer.visited, [true; 4]);
        assert_eq!(timer.measure(Phase::Closing, || Err::<(), _>(7)), Err(7));
        assert_eq!(timer.elapsed[3], None);
    }

    #[test]
    fn unwinding_operation_is_never_retried_or_replaced() {
        let mut timer = CallTimer::<true>::new();
        let mut calls = 0;
        let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            timer.measure(Phase::Native, || {
                calls += 1;
                std::panic::panic_any(53_u32);
            });
        }))
        .unwrap_err();
        assert_eq!(panic.downcast_ref::<u32>(), Some(&53));
        assert_eq!(calls, 1);
        assert_eq!(timer.visited, [false; 4]);
    }

    #[test]
    fn duplicate_or_unavailable_measurement_cannot_become_valid_again() {
        let mut timer = CallTimer::<true>::new();
        timer.record(Phase::Opening, None);
        timer.record(Phase::Opening, Some(7));
        timer.record(Phase::Opening, Some(8));
        assert_eq!(timer.elapsed[0], None);
    }
}
