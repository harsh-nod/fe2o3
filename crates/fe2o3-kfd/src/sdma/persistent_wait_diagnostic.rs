//! Success-only observations for the closed persistent-window sleep experiment.

use std::time::{Duration, Instant};

use super::multi_queue::tail_wait_cpu::{
    ThreadWaitCpuMeasurementV1, finish_tail_cpu_measurement_v1, profile_tail_cpu_snapshot_v1,
};
use super::{
    CompletedPersistentSdmaWindowV1, Gfx942SdmaCopyTicketV1, Gfx942SdmaErrorV1,
    Gfx942SdmaQueueOwnerV1, Gfx942SdmaQueueSetV1,
};
use crate::shared_memory::SharedGttMemorySessionV1;
use crate::wait::{MonotonicWaitV1, WaitActionV1};

/// Sidecar to the unchanged base SDMA contract; this experiment is not a proof.
pub const GFX942_PERSISTENT_SDMA_WAIT_DIAGNOSTIC_MANIFEST_V1: &str = concat!(
    "fe2o3.gfx942-persistent-sdma-wait-diagnostic.v1\n",
    "admission=hardware-diagnostic-feature,explicit-opt-in,closed-sleep-ceilings:1000000ns-or-25000ns,active-spin-floor:50000ns\n",
    "wait=whole-window-observation-before-deadline,same-ticket-validation-and-opening-and-closing-currentness,success-only-after-completed-custody\n",
    "counters=full-scan-rounds,packet-observations,spin-yield-sleep-pauses,total-and-maximum-requested-not-actual-sleep,checked-overflow-invalidates-counters\n",
    "timing=host-scan-including-cpu-observation-overhead,thread-cpu-and-context-switch-deltas:available-or-unavailable-or-invalid,no-device-time-or-engine-counters\n",
    "authority=none,ordinary-policy-unchanged,no-new-formal-or-parity-or-performance-claim\n",
);

/// Closed diagnostic policy; never selected by an ordinary persistent wait.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Gfx942SdmaPersistentDiagnosticSleepCeilingV1 {
    Millis1,
    Micros25,
}

impl Gfx942SdmaPersistentDiagnosticSleepCeilingV1 {
    pub const fn nanoseconds(self) -> u64 {
        match self {
            Self::Millis1 => 1_000_000,
            Self::Micros25 => 25_000,
        }
    }

    fn cursor(self, deadline: Instant) -> MonotonicWaitV1 {
        MonotonicWaitV1::until_with_active_spin_floor_and_sleep_ceiling(
            deadline,
            Duration::from_micros(50),
            Duration::from_nanos(self.nanoseconds()),
        )
    }
}

/// Host observations, not physical DMA measurements or completion authority.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Gfx942SdmaPersistentWaitCountersV1 {
    pub scan_rounds: u64,
    pub completion_observations: u64,
    pub spin_pauses: u64,
    pub yield_pauses: u64,
    pub sleep_pauses: u64,
    pub requested_sleep_ns: u64,
    pub max_requested_sleep_ns: u64,
}

/// Best-effort thread-cost observations. Invalid/unavailable data is not zero.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Gfx942SdmaPersistentWaitCpuV1 {
    Available {
        thread_cpu_ns: u64,
        voluntary_context_switches: u64,
        involuntary_context_switches: u64,
    },
    Unavailable,
    Invalid,
}

/// Returned only with completed custody after the queue's closing checks.
/// Timing includes diagnostic overhead and has no device-duration interpretation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Gfx942SdmaPersistentWaitDiagnosticsV1 {
    pub sleep_ceiling: Gfx942SdmaPersistentDiagnosticSleepCeilingV1,
    pub packet_count: usize,
    pub counters: Option<Gfx942SdmaPersistentWaitCountersV1>,
    pub scan_ns: Option<u64>,
    pub cpu: Gfx942SdmaPersistentWaitCpuV1,
}

trait WaitCursorV1 {
    fn expired(&self) -> bool;
    fn pause(&mut self) -> WaitActionV1;
}

impl WaitCursorV1 for MonotonicWaitV1 {
    fn expired(&self) -> bool {
        self.expired()
    }
    fn pause(&mut self) -> WaitActionV1 {
        self.pause_observed()
    }
}

fn count_scan(counters: &mut Option<Gfx942SdmaPersistentWaitCountersV1>, packets: usize) {
    *counters = counters.and_then(|mut value| {
        value.scan_rounds = value.scan_rounds.checked_add(1)?;
        value.completion_observations = value
            .completion_observations
            .checked_add(u64::try_from(packets).ok()?)?;
        Some(value)
    });
}

fn count_pause(counters: &mut Option<Gfx942SdmaPersistentWaitCountersV1>, action: WaitActionV1) {
    *counters = counters.and_then(|mut value| {
        match action {
            WaitActionV1::Spin => value.spin_pauses = value.spin_pauses.checked_add(1)?,
            WaitActionV1::Yield => value.yield_pauses = value.yield_pauses.checked_add(1)?,
            WaitActionV1::Sleep(duration) => {
                let requested = u64::try_from(duration.as_nanos()).ok()?;
                value.sleep_pauses = value.sleep_pauses.checked_add(1)?;
                value.requested_sleep_ns = value.requested_sleep_ns.checked_add(requested)?;
                value.max_requested_sleep_ns = value.max_requested_sleep_ns.max(requested);
            }
        }
        Some(value)
    });
}

#[derive(Debug, Eq, PartialEq)]
enum ScanOutcomeV1 {
    Completed(Option<Gfx942SdmaPersistentWaitCountersV1>),
    TimedOut,
}

fn scan_v1<E>(
    wait: &mut impl WaitCursorV1,
    packets: usize,
    mut observe: impl FnMut() -> Result<bool, E>,
) -> Result<ScanOutcomeV1, E> {
    let mut counters = Some(Gfx942SdmaPersistentWaitCountersV1::default());
    loop {
        let ready = observe()?;
        count_scan(&mut counters, packets);
        if ready {
            return Ok(ScanOutcomeV1::Completed(counters));
        }
        if wait.expired() {
            return Ok(ScanOutcomeV1::TimedOut);
        }
        count_pause(&mut counters, wait.pause());
    }
}

fn cpu_observation(value: ThreadWaitCpuMeasurementV1) -> Gfx942SdmaPersistentWaitCpuV1 {
    match value {
        ThreadWaitCpuMeasurementV1::Available {
            thread_cpu_ns,
            voluntary_context_switches,
            involuntary_context_switches,
        } => Gfx942SdmaPersistentWaitCpuV1::Available {
            thread_cpu_ns,
            voluntary_context_switches,
            involuntary_context_switches,
        },
        ThreadWaitCpuMeasurementV1::Unavailable => Gfx942SdmaPersistentWaitCpuV1::Unavailable,
        ThreadWaitCpuMeasurementV1::Invalid => Gfx942SdmaPersistentWaitCpuV1::Invalid,
    }
}

impl Gfx942SdmaQueueOwnerV1 {
    fn wait_persistent_window_profiled_for_v1(
        &mut self,
        memory: &mut SharedGttMemorySessionV1,
        tickets: &[Gfx942SdmaCopyTicketV1],
        timeout: Duration,
        policy: Gfx942SdmaPersistentDiagnosticSleepCeilingV1,
    ) -> Result<
        (
            CompletedPersistentSdmaWindowV1,
            Gfx942SdmaPersistentWaitDiagnosticsV1,
        ),
        Gfx942SdmaErrorV1,
    > {
        self.require_live()?;
        self.validate_persistent_window_tickets(tickets)?;
        let deadline = Instant::now()
            .checked_add(timeout)
            .ok_or(Gfx942SdmaErrorV1::Contract(
                "persistent SDMA window wait deadline",
            ))?;
        memory.check_queue_operational_currentness()?;
        let mut wait = policy.cursor(deadline);
        let started = Instant::now();
        let cpu_started = profile_tail_cpu_snapshot_v1::<true>();
        let counters = scan_v1(&mut wait, tickets.len(), || {
            self.observe_persistent_window_completion(memory, tickets)
        })?;
        let cpu = cpu_observation(finish_tail_cpu_measurement_v1(cpu_started));
        let scan_ns = u64::try_from(started.elapsed().as_nanos()).ok();
        memory.check_queue_operational_currentness()?;
        let ScanOutcomeV1::Completed(counters) = counters else {
            return Err(Gfx942SdmaErrorV1::Timeout);
        };
        let completed = self.complete_persistent_window(tickets);
        Ok((
            completed,
            Gfx942SdmaPersistentWaitDiagnosticsV1 {
                sleep_ceiling: policy,
                packet_count: tickets.len(),
                counters,
                scan_ns,
                cpu,
            },
        ))
    }
}

impl Gfx942SdmaQueueSetV1 {
    pub(crate) fn wait_persistent_window_profiled_for_v1(
        &mut self,
        memory: &mut SharedGttMemorySessionV1,
        tickets: &[Gfx942SdmaCopyTicketV1],
        timeout: Duration,
        policy: Gfx942SdmaPersistentDiagnosticSleepCeilingV1,
    ) -> Result<
        (
            CompletedPersistentSdmaWindowV1,
            Gfx942SdmaPersistentWaitDiagnosticsV1,
        ),
        Gfx942SdmaErrorV1,
    > {
        self.owner_for_tickets(tickets)?
            .wait_persistent_window_profiled_for_v1(memory, tickets, timeout, policy)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::VecDeque;

    struct ScriptedWait {
        expired: bool,
        pauses: VecDeque<WaitActionV1>,
    }

    impl WaitCursorV1 for ScriptedWait {
        fn expired(&self) -> bool {
            self.expired
        }

        fn pause(&mut self) -> WaitActionV1 {
            self.pauses.pop_front().expect("unexpected pause")
        }
    }

    #[test]
    fn ready_is_observed_before_an_expired_deadline() {
        let mut wait = ScriptedWait {
            expired: true,
            pauses: VecDeque::new(),
        };
        let result = scan_v1(&mut wait, 63, || Ok::<_, ()>(true)).unwrap();
        assert_eq!(
            result,
            ScanOutcomeV1::Completed(Some(Gfx942SdmaPersistentWaitCountersV1 {
                scan_rounds: 1,
                completion_observations: 63,
                ..Default::default()
            }))
        );
    }

    #[test]
    fn pending_at_deadline_and_observer_errors_emit_no_capture() {
        let mut wait = ScriptedWait {
            expired: true,
            pauses: VecDeque::new(),
        };
        assert_eq!(
            scan_v1(&mut wait, 2, || Ok::<_, ()>(false)),
            Ok(ScanOutcomeV1::TimedOut)
        );
        assert_eq!(
            scan_v1(&mut wait, 2, || Err::<bool, _>("observer")),
            Err("observer")
        );
    }

    #[test]
    fn counters_decompose_full_scans_and_actual_requested_pauses() {
        let mut wait = ScriptedWait {
            expired: false,
            pauses: [
                WaitActionV1::Spin,
                WaitActionV1::Yield,
                WaitActionV1::Sleep(Duration::from_nanos(25_000)),
                WaitActionV1::Sleep(Duration::from_nanos(17)),
            ]
            .into(),
        };
        let mut ready = [false, false, false, false, true].into_iter();
        let result = scan_v1(&mut wait, 63, || Ok::<_, ()>(ready.next().unwrap())).unwrap();
        assert_eq!(
            result,
            ScanOutcomeV1::Completed(Some(Gfx942SdmaPersistentWaitCountersV1 {
                scan_rounds: 5,
                completion_observations: 315,
                spin_pauses: 1,
                yield_pauses: 1,
                sleep_pauses: 2,
                requested_sleep_ns: 25_017,
                max_requested_sleep_ns: 25_000,
            }))
        );
        assert!(wait.pauses.is_empty());
        assert_eq!(ready.next(), None);
    }

    #[test]
    fn counter_overflow_invalidates_measurements_without_fabricating_values() {
        let defaults = Gfx942SdmaPersistentWaitCountersV1::default();
        for original in [
            Gfx942SdmaPersistentWaitCountersV1 {
                scan_rounds: u64::MAX,
                ..defaults
            },
            Gfx942SdmaPersistentWaitCountersV1 {
                completion_observations: u64::MAX,
                ..defaults
            },
        ] {
            let mut counters = Some(original);
            count_scan(&mut counters, 2);
            assert_eq!(counters, None);
            count_scan(&mut counters, 2);
            assert_eq!(counters, None);
        }
        for (original, pause) in [
            (
                Gfx942SdmaPersistentWaitCountersV1 {
                    spin_pauses: u64::MAX,
                    ..defaults
                },
                WaitActionV1::Spin,
            ),
            (
                Gfx942SdmaPersistentWaitCountersV1 {
                    yield_pauses: u64::MAX,
                    ..defaults
                },
                WaitActionV1::Yield,
            ),
            (
                Gfx942SdmaPersistentWaitCountersV1 {
                    sleep_pauses: u64::MAX,
                    ..defaults
                },
                WaitActionV1::Sleep(Duration::ZERO),
            ),
            (
                Gfx942SdmaPersistentWaitCountersV1 {
                    requested_sleep_ns: u64::MAX,
                    ..defaults
                },
                WaitActionV1::Sleep(Duration::from_nanos(1)),
            ),
            (defaults, WaitActionV1::Sleep(Duration::MAX)),
        ] {
            let mut counters = Some(original);
            count_pause(&mut counters, pause);
            assert_eq!(counters, None);
            count_pause(&mut counters, WaitActionV1::Spin);
            assert_eq!(counters, None);
        }
    }

    #[test]
    fn cpu_missing_and_invalid_states_are_not_zero_cost_measurements() {
        assert_eq!(
            cpu_observation(ThreadWaitCpuMeasurementV1::Unavailable),
            Gfx942SdmaPersistentWaitCpuV1::Unavailable
        );
        assert_eq!(
            cpu_observation(ThreadWaitCpuMeasurementV1::Invalid),
            Gfx942SdmaPersistentWaitCpuV1::Invalid
        );
        assert_eq!(
            cpu_observation(ThreadWaitCpuMeasurementV1::Available {
                thread_cpu_ns: 7,
                voluntary_context_switches: 3,
                involuntary_context_switches: 2,
            }),
            Gfx942SdmaPersistentWaitCpuV1::Available {
                thread_cpu_ns: 7,
                voluntary_context_switches: 3,
                involuntary_context_switches: 2,
            }
        );
        assert_eq!(
            Gfx942SdmaPersistentDiagnosticSleepCeilingV1::Millis1.nanoseconds(),
            1_000_000
        );
        assert_eq!(
            Gfx942SdmaPersistentDiagnosticSleepCeilingV1::Micros25.nanoseconds(),
            25_000
        );
    }

    #[test]
    fn matched_baseline_remains_bound_to_the_ordinary_policy() {
        assert!(include_str!("../queue_live.rs").contains(
            "const PERSISTENT_SDMA_ACTIVE_SPIN_FLOOR_V1: Duration = Duration::from_micros(50);"
        ));
        assert!(
            include_str!("../wait.rs")
                .contains("const MAX_SLEEP_V1: Duration = Duration::from_millis(1);")
        );
        assert!(
            GFX942_PERSISTENT_SDMA_WAIT_DIAGNOSTIC_MANIFEST_V1
                .contains("no-new-formal-or-parity-or-performance-claim")
        );
    }
}
