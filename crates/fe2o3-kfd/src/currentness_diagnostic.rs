//! Owner-free, opt-in host intervals. Disabled timing has no state or clock reads.

#![forbid(unsafe_code)]

pub(crate) trait Timing<const N: usize>: Sized {
    fn new() -> Self;
    fn measure<T>(&mut self, phase: usize, operation: impl FnOnce() -> T) -> T;
}

pub(crate) trait Mode {
    type Timer<const N: usize>: Timing<N>;
    type Topology;
    type Pair;
    fn topology(timer: Self::Timer<4>) -> Self::Topology;
    fn pair(timer: Self::Timer<6>, topology: Self::Topology) -> Self::Pair;
}

pub(crate) struct Disabled;

impl<const N: usize> Timing<N> for Disabled {
    #[inline]
    fn new() -> Self {
        Self
    }

    #[inline]
    fn measure<T>(&mut self, _: usize, operation: impl FnOnce() -> T) -> T {
        operation()
    }
}

impl Mode for Disabled {
    type Timer<const N: usize> = Self;
    type Topology = ();
    type Pair = ();

    #[inline]
    fn topology(_: Self::Timer<4>) {}

    #[inline]
    fn pair(_: Self::Timer<6>, _: ()) {}
}

#[cfg(any(feature = "hardware-diagnostic", test))]
pub(crate) use enabled::Enabled;
#[cfg(feature = "hardware-diagnostic")]
pub use enabled::{Gfx942TopologyDiscoveryDiagnosticsV1, Gfx942XgmiPairCurrentnessDiagnosticsV1};

#[cfg(any(feature = "hardware-diagnostic", test))]
mod enabled {
    use super::{Mode, Timing};
    use std::time::Instant;

    fn now() -> Instant {
        #[cfg(test)]
        super::tests::clock_read();
        Instant::now()
    }

    fn elapsed(start: Instant, end: Instant) -> Option<u64> {
        u64::try_from(end.checked_duration_since(start)?.as_nanos()).ok()
    }

    pub(crate) struct Measured<const N: usize> {
        start: Instant,
        elapsed: [Option<u64>; N],
        visited: [bool; N],
        invalid: bool,
    }

    impl<const N: usize> Timing<N> for Measured<N> {
        fn new() -> Self {
            Self {
                start: now(),
                elapsed: [None; N],
                visited: [false; N],
                invalid: false,
            }
        }

        fn measure<T>(&mut self, phase: usize, operation: impl FnOnce() -> T) -> T {
            let start = now();
            let result = operation();
            self.record(phase, elapsed(start, now()));
            result
        }
    }

    impl<const N: usize> Measured<N> {
        fn record(&mut self, phase: usize, elapsed: Option<u64>) {
            if phase >= N {
                self.invalid = true;
                return;
            }
            self.elapsed[phase] = if self.visited[phase] { None } else { elapsed };
            self.visited[phase] = true;
        }

        fn finish(self) -> ([Option<u64>; N], Option<u64>) {
            let total = elapsed(self.start, now());
            (self.elapsed, if self.invalid { None } else { total })
        }
    }

    fn valid<const N: usize>(phases: [Option<u64>; N], total: Option<u64>) -> bool {
        let sum = phases
            .into_iter()
            .try_fold(0_u64, |sum, phase| sum.checked_add(phase?));
        matches!((sum, total), (Some(sum), Some(total)) if sum <= total)
    }

    /// Host intervals for one fresh whole-host discovery, not an atomic kernel
    /// snapshot, device timing, authenticated profiler event, or authority.
    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub struct Gfx942TopologyDiscoveryDiagnosticsV1 {
        pub topology_tree_ns: Option<u64>,
        pub initial_identity_ns: Option<u64>,
        pub render_correlation_ns: Option<u64>,
        pub closing_identity_ns: Option<u64>,
        pub total_ns: Option<u64>,
    }

    impl Gfx942TopologyDiscoveryDiagnosticsV1 {
        pub fn is_complete(self) -> bool {
            valid(
                [
                    self.topology_tree_ns,
                    self.initial_identity_ns,
                    self.render_correlation_ns,
                    self.closing_identity_ns,
                ],
                self.total_ns,
            )
        }
    }

    /// Host intervals for one successful full pair check. Opening and closing
    /// observations are phase-local; neither alone establishes a batch bracket.
    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub struct Gfx942XgmiPairCurrentnessDiagnosticsV1 {
        pub source_before_ns: Option<u64>,
        pub peer_before_ns: Option<u64>,
        pub topology_discovery_ns: Option<u64>,
        pub route_and_equality_ns: Option<u64>,
        pub source_after_ns: Option<u64>,
        pub peer_after_ns: Option<u64>,
        pub total_ns: Option<u64>,
        pub topology: Gfx942TopologyDiscoveryDiagnosticsV1,
    }

    impl Gfx942XgmiPairCurrentnessDiagnosticsV1 {
        pub fn is_complete(self) -> bool {
            valid(
                [
                    self.source_before_ns,
                    self.peer_before_ns,
                    self.topology_discovery_ns,
                    self.route_and_equality_ns,
                    self.source_after_ns,
                    self.peer_after_ns,
                ],
                self.total_ns,
            ) && self.topology.is_complete()
                && matches!((self.topology.total_ns, self.topology_discovery_ns),
                            (Some(inner), Some(outer)) if inner <= outer)
        }
    }

    pub(crate) struct Enabled;

    impl Mode for Enabled {
        type Timer<const N: usize> = Measured<N>;
        type Topology = Gfx942TopologyDiscoveryDiagnosticsV1;
        type Pair = Gfx942XgmiPairCurrentnessDiagnosticsV1;

        fn topology(timer: Self::Timer<4>) -> Self::Topology {
            let ([tree, initial, render, closing], total) = timer.finish();
            Gfx942TopologyDiscoveryDiagnosticsV1 {
                topology_tree_ns: tree,
                initial_identity_ns: initial,
                render_correlation_ns: render,
                closing_identity_ns: closing,
                total_ns: total,
            }
        }

        fn pair(timer: Self::Timer<6>, topology: Self::Topology) -> Self::Pair {
            let (
                [
                    source_before,
                    peer_before,
                    discovery,
                    route,
                    source_after,
                    peer_after,
                ],
                total,
            ) = timer.finish();
            Gfx942XgmiPairCurrentnessDiagnosticsV1 {
                source_before_ns: source_before,
                peer_before_ns: peer_before,
                topology_discovery_ns: discovery,
                route_and_equality_ns: route,
                source_after_ns: source_after,
                peer_after_ns: peer_after,
                total_ns: total,
                topology,
            }
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn invalid_duplicate_missing_and_overflow_intervals_are_unavailable() {
            assert!(!valid([Some(u64::MAX), Some(1)], Some(u64::MAX)));
            assert!(!valid([None, Some(1)], Some(2)));
            assert!(!valid([Some(2), Some(1)], Some(2)));
            assert!(valid([Some(2), Some(1)], Some(3)));
            let mut timer = Measured::<1>::new();
            timer.record(0, None);
            timer.record(0, Some(1));
            assert_eq!(timer.finish().0, [None]);
            let mut timer = Measured::<1>::new();
            timer.record(1, Some(0));
            assert_eq!(timer.finish().1, None);
        }

        #[test]
        fn clock_inversion_and_nested_containment_are_checked() {
            let now = Instant::now();
            let later = now.checked_add(std::time::Duration::from_nanos(1)).unwrap();
            assert_eq!(elapsed(later, now), None);
            assert_eq!(elapsed(now, later), Some(1));
            let topology = Gfx942TopologyDiscoveryDiagnosticsV1 {
                topology_tree_ns: Some(1),
                initial_identity_ns: Some(1),
                render_correlation_ns: Some(1),
                closing_identity_ns: Some(1),
                total_ns: Some(4),
            };
            let valid = Gfx942XgmiPairCurrentnessDiagnosticsV1 {
                source_before_ns: Some(1),
                peer_before_ns: Some(1),
                topology_discovery_ns: Some(4),
                route_and_equality_ns: Some(1),
                source_after_ns: Some(1),
                peer_after_ns: Some(1),
                total_ns: Some(9),
                topology,
            };
            assert!(valid.is_complete());
            for malformed in [
                Gfx942XgmiPairCurrentnessDiagnosticsV1 {
                    total_ns: Some(8),
                    ..valid
                },
                Gfx942XgmiPairCurrentnessDiagnosticsV1 {
                    source_before_ns: None,
                    ..valid
                },
                Gfx942XgmiPairCurrentnessDiagnosticsV1 {
                    topology_discovery_ns: Some(3),
                    ..valid
                },
                Gfx942XgmiPairCurrentnessDiagnosticsV1 {
                    topology: Gfx942TopologyDiscoveryDiagnosticsV1 {
                        closing_identity_ns: None,
                        ..topology
                    },
                    ..valid
                },
                Gfx942XgmiPairCurrentnessDiagnosticsV1 {
                    total_ns: Some(u64::MAX),
                    source_before_ns: Some(u64::MAX),
                    ..valid
                },
            ] {
                assert!(!malformed.is_complete());
            }
        }
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use std::cell::Cell;
    use std::panic::{AssertUnwindSafe, catch_unwind};

    #[cfg(all(target_arch = "x86_64", feature = "hardware-diagnostic"))]
    mod public_surface {
        use crate::{
            Gfx942NativeXgmiSdmaBatchV1 as Batch, Gfx942NativeXgmiSdmaQueueV1 as Queue,
            Gfx942SdmaErrorV1 as Error, Gfx942XgmiPairCurrentnessDiagnosticsV1 as Detail,
            SharedGttMemorySessionV1 as Session,
        };

        type BeginDiagnostic = for<'a> fn(
            &'a mut Queue,
            &'a mut Session,
            &'a mut Session,
        ) -> Result<(Batch<'a>, Detail), Error>;
        const _: BeginDiagnostic = Queue::begin_batch_currentness_diagnostic_v1;
        const _: for<'a> fn(Batch<'a>) -> Result<Detail, Error> =
            |batch| batch.finish_currentness_diagnostic_v1();
        const _: for<'a> fn(Batch<'a>) -> Result<Detail, Error> =
            |batch| batch.finish_terminal_currentness_diagnostic_v1();
    }

    thread_local! { static CLOCK_READS: Cell<usize> = const { Cell::new(0) }; }

    pub(super) fn clock_read() {
        CLOCK_READS.set(CLOCK_READS.get() + 1);
    }

    pub(crate) fn clock_reads() -> usize {
        CLOCK_READS.get()
    }

    #[test]
    fn disabled_timer_is_zero_sized_and_never_reads_the_clock() {
        assert_eq!(std::mem::size_of::<Disabled>(), 0);
        let before = CLOCK_READS.get();
        let mut timer = <Disabled as Timing<4>>::new();
        assert_eq!(
            <Disabled as Timing<4>>::measure(&mut timer, 0, || Err::<(), _>(17)),
            Err(17)
        );
        let payload = Box::new(41_u64);
        let address = &*payload as *const u64;
        let panic = catch_unwind(AssertUnwindSafe(|| {
            <Disabled as Timing<4>>::measure(&mut timer, 1, || std::panic::panic_any(payload))
        }))
        .unwrap_err();
        assert_eq!(
            &**panic.downcast_ref::<Box<u64>>().unwrap() as *const u64,
            address
        );
        assert_eq!(CLOCK_READS.get(), before);
    }

    #[test]
    fn measured_operations_run_once_and_preserve_errors_and_panics() {
        let mut timer = <Enabled as Mode>::Timer::<4>::new();
        let mut calls = 0;
        assert_eq!(
            timer.measure(0, || {
                calls += 1;
                Err::<(), _>(17)
            }),
            Err(17)
        );
        let panic = catch_unwind(AssertUnwindSafe(|| {
            timer.measure(1, || {
                calls += 1;
                std::panic::panic_any(41_u64);
            })
        }))
        .unwrap_err();
        assert_eq!(panic.downcast_ref::<u64>(), Some(&41));
        assert_eq!(calls, 2);
        assert!(!Enabled::topology(timer).is_complete());
    }
}
