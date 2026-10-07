//! Success-only host observations, never completion or device-time authority.

use std::time::Instant;

use super::multi_queue::tail_wait_cpu::{
    ThreadWaitCpuMeasurementV1, ThreadWaitCpuSnapshotOutcomeV1, finish_tail_cpu_measurement_v1,
    profile_tail_cpu_snapshot_v1,
};
use crate::wait::{MonotonicWaitV1, WaitActionV1};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
/// Counts every attempted completion-slot observation, including pending values.
pub struct Gfx942XgmiRetainedWaitCountersV1 {
    pub scan_rounds: u64,
    pub completion_observations: u64,
    pub spin_pauses: u64,
    pub yield_pauses: u64,
    pub sleep_pauses: u64,
    pub requested_sleep_ns: u64,
    pub max_requested_sleep_ns: u64,
}

/// Unavailable or invalid observations must not be interpreted as zero cost.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Gfx942XgmiRetainedWaitCpuV1 {
    Available {
        thread_cpu_ns: u64,
        voluntary_context_switches: u64,
        involuntary_context_switches: u64,
    },
    Unavailable,
    Invalid,
}

/// Host timings include instrumentation and overlapping device progress.
/// First/all offsets start at the scan boundary, not at device execution.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[cfg(any(feature = "hardware-diagnostic", test))]
pub struct Gfx942XgmiRetainedWaitDiagnosticsV1 {
    pub opening_currentness_ns: Option<u64>,
    pub validation_ns: Option<u64>,
    pub scan_ns: Option<u64>,
    pub retirement_ns: Option<u64>,
    pub closing_currentness_ns: Option<u64>,
    pub total_ns: Option<u64>,
    pub first_observed_completion_ns: Option<u64>,
    pub all_observed_completion_ns: Option<u64>,
    pub counters: Option<Gfx942XgmiRetainedWaitCountersV1>,
    pub cpu: Gfx942XgmiRetainedWaitCpuV1,
}

#[derive(Clone, Copy)]
pub(super) enum Phase {
    Opening,
    Validation,
    Scan,
    Retirement,
    Closing,
}

pub(super) struct WaitTimer<const ENABLED: bool> {
    elapsed: [Option<u64>; 5],
    visited: [bool; 5],
    scan_started: Option<Instant>,
    cpu_started: Option<ThreadWaitCpuSnapshotOutcomeV1>,
    first_observed: Option<u64>,
    first_seen: bool,
    all_observed: Option<u64>,
    counters: Option<Gfx942XgmiRetainedWaitCountersV1>,
    cpu: Gfx942XgmiRetainedWaitCpuV1,
}

impl<const ENABLED: bool> WaitTimer<ENABLED> {
    pub(super) fn new() -> Self {
        Self {
            elapsed: [None; 5],
            visited: [false; 5],
            scan_started: None,
            cpu_started: None,
            first_observed: None,
            first_seen: false,
            all_observed: None,
            counters: if ENABLED {
                Some(Default::default())
            } else {
                None
            },
            cpu: Gfx942XgmiRetainedWaitCpuV1::Unavailable,
        }
    }

    #[inline]
    pub(super) fn start(&self) -> Option<Instant> {
        if ENABLED { Some(Instant::now()) } else { None }
    }

    #[inline]
    pub(super) fn end(&mut self, phase: Phase, started: Option<Instant>) {
        if ENABLED {
            let index = phase as usize;
            self.elapsed[index] = if self.visited[index] {
                None
            } else {
                started.and_then(|start| u64::try_from(start.elapsed().as_nanos()).ok())
            };
            self.visited[index] = true;
        }
    }

    #[inline]
    pub(super) fn measure<T>(&mut self, phase: Phase, operation: impl FnOnce() -> T) -> T {
        let started = self.start();
        let result = operation();
        self.end(phase, started);
        result
    }

    #[inline]
    pub(super) fn begin_scan(&mut self) {
        if ENABLED {
            self.scan_started = self.start();
            self.cpu_started = profile_tail_cpu_snapshot_v1::<true>();
        }
    }

    #[inline]
    pub(super) fn scan_round(&mut self) {
        if ENABLED {
            self.counters = self.counters.and_then(|mut counts| {
                counts.scan_rounds = counts.scan_rounds.checked_add(1)?;
                Some(counts)
            });
        }
    }

    #[inline]
    pub(super) fn observation(&mut self, completed: bool) {
        if ENABLED {
            self.counters = self.counters.and_then(|mut counts| {
                counts.completion_observations = counts.completion_observations.checked_add(1)?;
                Some(counts)
            });
            if completed && !self.first_seen {
                self.first_seen = true;
                self.first_observed = self.scan_elapsed();
            }
        }
    }

    fn scan_elapsed(&self) -> Option<u64> {
        self.scan_started
            .and_then(|start| u64::try_from(start.elapsed().as_nanos()).ok())
    }

    #[inline]
    pub(super) fn pause(&mut self, wait: &mut MonotonicWaitV1) {
        self.pause_cursor(
            wait,
            MonotonicWaitV1::pause,
            MonotonicWaitV1::pause_observed,
        );
    }

    #[inline]
    fn pause_cursor<C>(
        &mut self,
        cursor: &mut C,
        ordinary: impl FnOnce(&mut C),
        observed: impl FnOnce(&mut C) -> WaitActionV1,
    ) {
        if ENABLED {
            self.count_pause(observed(cursor));
        } else {
            ordinary(cursor);
        }
    }

    fn count_pause(&mut self, action: WaitActionV1) {
        self.counters = self.counters.and_then(|mut counts| {
            match action {
                WaitActionV1::Spin => counts.spin_pauses = counts.spin_pauses.checked_add(1)?,
                WaitActionV1::Yield => counts.yield_pauses = counts.yield_pauses.checked_add(1)?,
                WaitActionV1::Sleep(duration) => {
                    let requested = u64::try_from(duration.as_nanos()).ok()?;
                    counts.sleep_pauses = counts.sleep_pauses.checked_add(1)?;
                    counts.requested_sleep_ns = counts.requested_sleep_ns.checked_add(requested)?;
                    counts.max_requested_sleep_ns = counts.max_requested_sleep_ns.max(requested);
                }
            }
            Some(counts)
        });
    }

    #[inline]
    pub(super) fn finish_scan(&mut self) {
        if ENABLED {
            self.all_observed = self.scan_elapsed();
            self.cpu = match finish_tail_cpu_measurement_v1(self.cpu_started.take()) {
                ThreadWaitCpuMeasurementV1::Available {
                    thread_cpu_ns,
                    voluntary_context_switches,
                    involuntary_context_switches,
                } => Gfx942XgmiRetainedWaitCpuV1::Available {
                    thread_cpu_ns,
                    voluntary_context_switches,
                    involuntary_context_switches,
                },
                ThreadWaitCpuMeasurementV1::Unavailable => Gfx942XgmiRetainedWaitCpuV1::Unavailable,
                ThreadWaitCpuMeasurementV1::Invalid => Gfx942XgmiRetainedWaitCpuV1::Invalid,
            };
            self.end(Phase::Scan, self.scan_started);
        }
    }

    #[cfg(any(feature = "hardware-diagnostic", test))]
    pub(super) fn finish(self, started: Instant) -> Gfx942XgmiRetainedWaitDiagnosticsV1 {
        Gfx942XgmiRetainedWaitDiagnosticsV1 {
            opening_currentness_ns: self.elapsed[Phase::Opening as usize],
            validation_ns: self.elapsed[Phase::Validation as usize],
            scan_ns: self.elapsed[Phase::Scan as usize],
            retirement_ns: self.elapsed[Phase::Retirement as usize],
            closing_currentness_ns: self.elapsed[Phase::Closing as usize],
            total_ns: if ENABLED {
                u64::try_from(started.elapsed().as_nanos()).ok()
            } else {
                None
            },
            first_observed_completion_ns: self.first_observed,
            all_observed_completion_ns: self.all_observed,
            counters: self.counters,
            cpu: self.cpu,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn disabled_instrumentation_leaves_every_observation_absent() {
        let mut timer = WaitTimer::<false>::new();
        let value = timer.measure(Phase::Opening, || Err::<(), _>(17));
        assert_eq!(value, Err(17));
        timer.begin_scan();
        timer.scan_round();
        timer.observation(true);
        timer.finish_scan();
        assert!(timer.elapsed.iter().all(Option::is_none));
        assert_eq!(timer.visited, [false; 5]);
        assert_eq!(timer.scan_started, None);
        assert_eq!(timer.cpu_started, None);
        assert_eq!(timer.first_observed, None);
        assert_eq!(timer.all_observed, None);
        assert_eq!(timer.counters, None);
    }

    #[test]
    fn counters_record_requested_sleep_and_invalidate_on_overflow() {
        let mut timer = WaitTimer::<true>::new();
        timer.scan_round();
        timer.observation(false);
        timer.count_pause(WaitActionV1::Spin);
        timer.count_pause(WaitActionV1::Yield);
        timer.count_pause(WaitActionV1::Sleep(Duration::from_micros(25)));
        timer.count_pause(WaitActionV1::Sleep(Duration::from_micros(50)));
        assert_eq!(
            timer.counters,
            Some(Gfx942XgmiRetainedWaitCountersV1 {
                scan_rounds: 1,
                completion_observations: 1,
                spin_pauses: 1,
                yield_pauses: 1,
                sleep_pauses: 2,
                requested_sleep_ns: 75_000,
                max_requested_sleep_ns: 50_000,
            })
        );
        timer.counters.as_mut().unwrap().scan_rounds = u64::MAX;
        timer.scan_round();
        timer.observation(true);
        assert_eq!(timer.counters, None);
    }

    #[test]
    fn disabled_pause_calls_only_the_exact_ordinary_cursor_once() {
        let mut trace = Vec::new();
        let mut ordinary = WaitTimer::<false>::new();
        ordinary.pause_cursor(
            &mut trace,
            |trace| trace.push("ordinary"),
            |_| panic!("profiled pause"),
        );
        assert_eq!(trace, ["ordinary"]);
        assert_eq!(ordinary.counters, None);
        let mut profiled = WaitTimer::<true>::new();
        profiled.pause_cursor(
            &mut trace,
            |_| panic!("ordinary pause"),
            |trace| {
                trace.push("observed");
                WaitActionV1::Spin
            },
        );
        assert_eq!(trace, ["ordinary", "observed"]);
        assert_eq!(profiled.counters.unwrap().spin_pauses, 1);
    }

    #[test]
    fn duplicate_phase_cannot_become_valid_again_and_panics_are_not_retried() {
        let mut timer = WaitTimer::<true>::new();
        for _ in 0..3 {
            timer.measure(Phase::Opening, || ());
        }
        assert_eq!(timer.elapsed[0], None);
        let mut calls = 0;
        let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            timer.measure(Phase::Closing, || {
                calls += 1;
                std::panic::panic_any(53_u32);
            });
        }))
        .unwrap_err();
        assert_eq!(calls, 1);
        assert_eq!(panic.downcast_ref::<u32>(), Some(&53));
        assert!(!timer.visited[Phase::Closing as usize]);
    }
}
