//! Best-effort Linux thread-cost observations for the profiled striped-tail wait.

use core::mem::MaybeUninit;
use rustix::time::{ClockId, DynamicClockId, Timespec, clock_gettime_dynamic};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct ThreadWaitCpuSnapshotV1 {
    thread_cpu_ns: u64,
    voluntary_context_switches: u64,
    involuntary_context_switches: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum ThreadWaitCpuSnapshotOutcomeV1 {
    Available(ThreadWaitCpuSnapshotV1),
    Unavailable,
    Invalid,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum ThreadWaitCpuMeasurementV1 {
    Available {
        thread_cpu_ns: u64,
        voluntary_context_switches: u64,
        involuntary_context_switches: u64,
    },
    Unavailable,
    Invalid,
}

#[inline]
pub(super) fn profile_tail_cpu_snapshot_v1<const PROFILE: bool>()
-> Option<ThreadWaitCpuSnapshotOutcomeV1> {
    profile_thread_wait_cpu_snapshot_with_v1::<PROFILE>(observe_thread_wait_cpu_snapshot_v1)
}

#[inline]
fn profile_thread_wait_cpu_snapshot_with_v1<const PROFILE: bool>(
    observe: impl FnOnce() -> ThreadWaitCpuSnapshotOutcomeV1,
) -> Option<ThreadWaitCpuSnapshotOutcomeV1> {
    if PROFILE { Some(observe()) } else { None }
}

pub(super) fn finish_tail_cpu_measurement_v1(
    started: Option<ThreadWaitCpuSnapshotOutcomeV1>,
) -> ThreadWaitCpuMeasurementV1 {
    finish_thread_wait_cpu_measurement_with_v1(started, observe_thread_wait_cpu_snapshot_v1)
}

fn finish_thread_wait_cpu_measurement_with_v1(
    started: Option<ThreadWaitCpuSnapshotOutcomeV1>,
    observe: impl FnOnce() -> ThreadWaitCpuSnapshotOutcomeV1,
) -> ThreadWaitCpuMeasurementV1 {
    let Some(started) = started else {
        return ThreadWaitCpuMeasurementV1::Unavailable;
    };
    match started {
        ThreadWaitCpuSnapshotOutcomeV1::Available(started) => match observe() {
            ThreadWaitCpuSnapshotOutcomeV1::Available(finished) => {
                thread_wait_cpu_delta_v1(started, finished)
            }
            ThreadWaitCpuSnapshotOutcomeV1::Unavailable => ThreadWaitCpuMeasurementV1::Unavailable,
            ThreadWaitCpuSnapshotOutcomeV1::Invalid => ThreadWaitCpuMeasurementV1::Invalid,
        },
        ThreadWaitCpuSnapshotOutcomeV1::Unavailable => ThreadWaitCpuMeasurementV1::Unavailable,
        ThreadWaitCpuSnapshotOutcomeV1::Invalid => ThreadWaitCpuMeasurementV1::Invalid,
    }
}

fn thread_wait_cpu_delta_v1(
    started: ThreadWaitCpuSnapshotV1,
    finished: ThreadWaitCpuSnapshotV1,
) -> ThreadWaitCpuMeasurementV1 {
    let Some(thread_cpu_ns) = finished.thread_cpu_ns.checked_sub(started.thread_cpu_ns) else {
        return ThreadWaitCpuMeasurementV1::Invalid;
    };
    let Some(voluntary_context_switches) = finished
        .voluntary_context_switches
        .checked_sub(started.voluntary_context_switches)
    else {
        return ThreadWaitCpuMeasurementV1::Invalid;
    };
    let Some(involuntary_context_switches) = finished
        .involuntary_context_switches
        .checked_sub(started.involuntary_context_switches)
    else {
        return ThreadWaitCpuMeasurementV1::Invalid;
    };
    ThreadWaitCpuMeasurementV1::Available {
        thread_cpu_ns,
        voluntary_context_switches,
        involuntary_context_switches,
    }
}

fn observe_thread_wait_cpu_snapshot_v1() -> ThreadWaitCpuSnapshotOutcomeV1 {
    let Ok(clock) = clock_gettime_dynamic(DynamicClockId::Known(ClockId::ThreadCPUTime)) else {
        return ThreadWaitCpuSnapshotOutcomeV1::Unavailable;
    };
    let Some(thread_cpu_ns) = timespec_ns_v1(clock) else {
        return ThreadWaitCpuSnapshotOutcomeV1::Invalid;
    };

    let mut usage = MaybeUninit::<libc::rusage>::zeroed();
    // SAFETY: `usage` is writable storage for one rusage and RUSAGE_THREAD owns no resources.
    if unsafe { libc::getrusage(libc::RUSAGE_THREAD, usage.as_mut_ptr()) } != 0 {
        return ThreadWaitCpuSnapshotOutcomeV1::Unavailable;
    }
    // SAFETY: successful getrusage initialized the rusage.
    let usage = unsafe { usage.assume_init() };
    let (Ok(voluntary_context_switches), Ok(involuntary_context_switches)) = (
        u64::try_from(usage.ru_nvcsw),
        u64::try_from(usage.ru_nivcsw),
    ) else {
        return ThreadWaitCpuSnapshotOutcomeV1::Invalid;
    };
    ThreadWaitCpuSnapshotOutcomeV1::Available(ThreadWaitCpuSnapshotV1 {
        thread_cpu_ns,
        voluntary_context_switches,
        involuntary_context_switches,
    })
}

fn timespec_ns_v1(clock: Timespec) -> Option<u64> {
    let seconds = u64::try_from(clock.tv_sec).ok()?;
    let nanoseconds = u64::try_from(clock.tv_nsec).ok()?;
    if nanoseconds >= 1_000_000_000 {
        return None;
    }
    seconds
        .checked_mul(1_000_000_000)
        .and_then(|value| value.checked_add(nanoseconds))
}

#[cfg(test)]
mod tests {
    use super::{
        ThreadWaitCpuMeasurementV1, ThreadWaitCpuSnapshotOutcomeV1, ThreadWaitCpuSnapshotV1,
        finish_thread_wait_cpu_measurement_with_v1, profile_thread_wait_cpu_snapshot_with_v1,
        thread_wait_cpu_delta_v1, timespec_ns_v1,
    };
    use rustix::time::Timespec;
    use std::cell::Cell;

    #[test]
    fn compile_time_disabled_profile_does_not_invoke_snapshot_observer() {
        let calls = Cell::new(0_u8);
        let observed = profile_thread_wait_cpu_snapshot_with_v1::<false>(|| {
            calls.set(calls.get() + 1);
            ThreadWaitCpuSnapshotOutcomeV1::Unavailable
        });
        assert_eq!(observed, None);
        assert_eq!(calls.get(), 0);
    }

    #[test]
    fn unavailable_or_invalid_start_is_diagnostic_only_and_skips_the_end_observer() {
        for started in [
            None,
            Some(ThreadWaitCpuSnapshotOutcomeV1::Unavailable),
            Some(ThreadWaitCpuSnapshotOutcomeV1::Invalid),
        ] {
            let calls = Cell::new(0_u8);
            let observed = finish_thread_wait_cpu_measurement_with_v1(started, || {
                calls.set(calls.get() + 1);
                ThreadWaitCpuSnapshotOutcomeV1::Unavailable
            });
            assert_eq!(calls.get(), 0);
            assert_eq!(
                observed,
                if matches!(started, Some(ThreadWaitCpuSnapshotOutcomeV1::Invalid)) {
                    ThreadWaitCpuMeasurementV1::Invalid
                } else {
                    ThreadWaitCpuMeasurementV1::Unavailable
                }
            );
        }
    }

    #[test]
    fn unavailable_or_invalid_end_remains_a_diagnostic_status() {
        let started = Some(ThreadWaitCpuSnapshotOutcomeV1::Available(
            ThreadWaitCpuSnapshotV1 {
                thread_cpu_ns: 10,
                voluntary_context_switches: 20,
                involuntary_context_switches: 30,
            },
        ));
        for (finished, expected) in [
            (
                ThreadWaitCpuSnapshotOutcomeV1::Unavailable,
                ThreadWaitCpuMeasurementV1::Unavailable,
            ),
            (
                ThreadWaitCpuSnapshotOutcomeV1::Invalid,
                ThreadWaitCpuMeasurementV1::Invalid,
            ),
        ] {
            assert_eq!(
                finish_thread_wait_cpu_measurement_with_v1(started, || finished),
                expected
            );
        }
    }

    #[test]
    fn snapshot_conversion_rejects_negative_invalid_and_overflowing_clocks() {
        assert_eq!(
            timespec_ns_v1(Timespec {
                tv_sec: 2,
                tv_nsec: 7,
            }),
            Some(2_000_000_007)
        );
        assert_eq!(
            timespec_ns_v1(Timespec {
                tv_sec: -1,
                tv_nsec: 0,
            }),
            None
        );
        assert_eq!(
            timespec_ns_v1(Timespec {
                tv_sec: 0,
                tv_nsec: 1_000_000_000,
            }),
            None
        );
        assert_eq!(
            timespec_ns_v1(Timespec {
                tv_sec: rustix::time::Secs::MAX,
                tv_nsec: 0,
            }),
            None
        );
    }

    #[test]
    fn regressing_snapshot_is_an_invalid_diagnostic() {
        let started = ThreadWaitCpuSnapshotV1 {
            thread_cpu_ns: 10,
            voluntary_context_switches: 20,
            involuntary_context_switches: 30,
        };
        for finished in [
            ThreadWaitCpuSnapshotV1 {
                thread_cpu_ns: 9,
                ..started
            },
            ThreadWaitCpuSnapshotV1 {
                voluntary_context_switches: 19,
                ..started
            },
            ThreadWaitCpuSnapshotV1 {
                involuntary_context_switches: 29,
                ..started
            },
        ] {
            assert_eq!(
                thread_wait_cpu_delta_v1(started, finished),
                ThreadWaitCpuMeasurementV1::Invalid
            );
        }
    }

    #[test]
    fn available_snapshot_delta_preserves_each_measurement() {
        let observed = thread_wait_cpu_delta_v1(
            ThreadWaitCpuSnapshotV1 {
                thread_cpu_ns: 10,
                voluntary_context_switches: 20,
                involuntary_context_switches: 30,
            },
            ThreadWaitCpuSnapshotV1 {
                thread_cpu_ns: 110,
                voluntary_context_switches: 23,
                involuntary_context_switches: 35,
            },
        );
        assert_eq!(
            observed,
            ThreadWaitCpuMeasurementV1::Available {
                thread_cpu_ns: 100,
                voluntary_context_switches: 3,
                involuntary_context_switches: 5,
            }
        );
    }
}
