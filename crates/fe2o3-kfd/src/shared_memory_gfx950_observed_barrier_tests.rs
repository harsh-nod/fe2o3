use super::*;
use std::collections::VecDeque;

#[test]
fn v2_acquired_completion_preserves_zero_read_shadow_without_reusable_capacity() {
    let mut fake = Fake::new([fresh(), published(0, 1), published(0, 0)]);
    let completion =
        complete_one_barrier_mode(&mut fake, 0x30_000, Retirement::QueueDestroyV2).unwrap();
    assert_eq!(completion, published(0, 0));
    assert_eq!(fake.counters, (1, 0));
    assert_eq!(
        fake.calls.iter().filter(|&&call| call == "reserve").count(),
        1
    );
    assert_eq!(
        fake.calls
            .iter()
            .filter(|&&call| call == "doorbell")
            .count(),
        1
    );
    let mut signal = Signal::destroy_v2();
    signal.record_completion(completion).unwrap();
    assert!(!signal.release_ready());
    assert!(signal.record_completion(completion).is_err());
    // The strict V1 observation still refuses this exact completed/read-zero case.
    assert!(complete_one_barrier(&mut Fake::new([fresh(), completion]), 0x30_000).is_err());
}

#[test]
fn v2_release_requires_destroy_and_a_fresh_post_destroy_acquisition() {
    let mut signal = Signal::destroy_v2();
    assert!(signal.confirm_destroyed().is_err());
    assert!(signal.observe_completed(published(0, 0)).is_err());
    assert!(!signal.release_ready());
    signal.record_completion(published(0, 0)).unwrap();
    signal.observe_completed(published(0, 0)).unwrap();
    assert!(!signal.release_ready());
    signal.confirm_destroyed().unwrap();
    assert!(!signal.release_ready());
    signal.observe_completed(published(0, 0)).unwrap();
    assert!(signal.release_ready());
    assert_eq!(signal.after_destroy.unwrap().read, 0);
    assert!(signal.confirm_destroyed().is_err());
}

#[test]
fn v2_shadow_progress_is_monotonic_through_counter_and_snapshot_checks() {
    let mut signal = Signal::destroy_v2();
    signal.record_completion(published(0, 0)).unwrap();
    signal.observe_counters((1, 1)).unwrap();
    assert!(signal.observe_completed(published(0, 0)).is_err());
    assert_eq!(signal.last_read, 1);
    signal.confirm_destroyed().unwrap();
    signal.observe_completed(published(1, 0)).unwrap();
    assert!(signal.release_ready());
    for counters in [(1, 0), (1, 2), (0, 0), (2, 1), (u64::MAX, 1)] {
        assert!(signal.observe_counters(counters).is_err());
        assert_eq!(signal.last_read, 1);
    }
}

#[test]
fn v2_pending_and_malformed_observations_still_fail_closed() {
    for snapshot in [
        published(0, 1),
        published(1, 1),
        published(2, 0),
        Snapshot {
            write: 0,
            ..published(0, 0)
        },
        Snapshot {
            kind: 0,
            ..published(0, 0)
        },
        Snapshot {
            value: -1,
            ..published(0, 0)
        },
        Snapshot {
            value: 2,
            ..published(0, 0)
        },
        Snapshot {
            header: 0xffff,
            ..published(0, 0)
        },
        Snapshot {
            setup: 1,
            ..published(0, 0)
        },
    ] {
        let mut fake = Fake::new([fresh(), snapshot]);
        assert!(
            complete_one_barrier_mode(&mut fake, 0x30_000, Retirement::QueueDestroyV2).is_err()
        );
        assert_eq!(
            fake.calls
                .iter()
                .filter(|&&call| call == "doorbell")
                .count(),
            1
        );
    }
    let mut decreasing = Fake::new([fresh(), published(1, 1), published(0, 0)]);
    assert!(
        complete_one_barrier_mode(&mut decreasing, 0x30_000, Retirement::QueueDestroyV2).is_err()
    );
}

#[test]
fn v2_post_destroy_signal_counter_and_header_mutations_cannot_qualify_release() {
    for changed in [
        published(0, 1),
        published(2, 0),
        Snapshot {
            write: 0,
            ..published(0, 0)
        },
        Snapshot {
            kind: 0,
            ..published(0, 0)
        },
        Snapshot {
            header: 0xffff,
            ..published(0, 0)
        },
    ] {
        let mut signal = Signal::destroy_v2();
        signal.record_completion(published(0, 0)).unwrap();
        signal.confirm_destroyed().unwrap();
        assert!(signal.observe_completed(changed).is_err());
        assert!(!signal.release_ready());
    }
}

// This adapter drives the actual coordinator, publisher and Signal state.
// Queue/event syscalls remain explicitly injected, not hardware emulation.
struct DestroyCoordinator {
    fake: Fake,
    signal: Signal,
    steps: Vec<Step>,
    fail_step: Option<Step>,
    currentness_calls: usize,
    fail_currentness: Option<usize>,
    destroyed: bool,
    released: bool,
    poisoned: bool,
    post_destroy: Snapshot,
}
impl DestroyCoordinator {
    fn new() -> Self {
        Self {
            fake: Fake::new([fresh(), published(0, 0)]),
            signal: Signal::destroy_v2(),
            steps: vec![],
            fail_step: None,
            currentness_calls: 0,
            fail_currentness: None,
            destroyed: false,
            released: false,
            poisoned: false,
            post_destroy: published(0, 0),
        }
    }
}
impl LifecycleBackend for DestroyCoordinator {
    fn preflight(&mut self) -> QueueResult<()> {
        Ok(())
    }
    fn currentness(&mut self) -> QueueResult<()> {
        self.currentness_calls += 1;
        if self.fail_currentness == Some(self.currentness_calls) {
            Err("injected currentness".into())
        } else {
            Ok(())
        }
    }
    fn step(&mut self, step: Step) -> QueueResult<()> {
        self.steps.push(step);
        if self.fail_step == Some(step) {
            return Err("injected lifecycle operation".into());
        }
        match step {
            Step::CheckUnpublished => {
                let completion = complete_one_barrier_mode(
                    &mut self.fake,
                    0x30_000,
                    Retirement::QueueDestroyV2,
                )?;
                self.signal.record_completion(completion)?;
            }
            Step::Destroy => {
                self.signal.observe_counters((1, 0))?;
                self.signal.observe_completed(published(0, 0))?;
                assert!(!self.released && !self.signal.release_ready());
                // Confirmation comes only after the injected destroy succeeds.
                self.destroyed = true;
                self.signal.confirm_destroyed()?;
            }
            Step::DestroyEvent => {
                self.signal.observe_completed(self.post_destroy)?;
            }
            Step::ReleaseMemory => {
                self.signal.observe_completed(self.post_destroy)?;
                if !self.signal.release_ready() {
                    return Err("release prerequisite".into());
                }
                assert!(self.destroyed);
                assert!(self.steps.contains(&Step::DestroyEvent));
                assert!(self.steps.contains(&Step::DisableRuntime));
                assert!(self.steps.contains(&Step::ReleaseDoorbell));
                self.released = true;
            }
            Step::Finish => {
                assert!(self.destroyed && self.released);
            }
            _ => {}
        }
        Ok(())
    }
    fn quarantine(&mut self) {
        self.poisoned = true;
    }
}

#[test]
fn v2_actual_coordinator_releases_only_after_destroy_and_ordered_teardown() {
    let mut coordinator = DestroyCoordinator::new();
    run_lifecycle(&mut coordinator).unwrap();
    assert_eq!(coordinator.steps, STEPS);
    assert!(coordinator.destroyed && coordinator.released && !coordinator.poisoned);
    assert_eq!(coordinator.signal.completed.unwrap().read, 0);
    assert_eq!(coordinator.signal.after_destroy.unwrap().read, 0);
    assert_eq!(coordinator.fake.counters, (1, 0));
}

#[test]
fn v2_destroy_and_every_late_operation_failure_quarantine_without_success_or_retry() {
    let start = STEPS.iter().position(|&s| s == Step::Destroy).unwrap();
    for index in start..STEPS.len() {
        let mut coordinator = DestroyCoordinator::new();
        coordinator.fail_step = Some(STEPS[index]);
        assert!(run_lifecycle(&mut coordinator).is_err());
        assert!(coordinator.poisoned);
        assert_eq!(coordinator.steps, STEPS[..=index]);
        if index
            <= STEPS
                .iter()
                .position(|&s| s == Step::ReleaseMemory)
                .unwrap()
        {
            assert!(!coordinator.released);
        }
        if STEPS[index] == Step::Destroy {
            assert!(!coordinator.destroyed);
        }
        assert_eq!(
            coordinator
                .fake
                .calls
                .iter()
                .filter(|&&call| call == "doorbell")
                .count(),
            1
        );
    }
    let mut malformed = DestroyCoordinator::new();
    malformed.post_destroy = published(2, 0);
    assert!(run_lifecycle(&mut malformed).is_err());
    assert!(malformed.destroyed && malformed.poisoned && !malformed.released);
}

#[test]
fn v2_every_outer_currentness_failure_quarantines_and_stops_publication() {
    let mut healthy = DestroyCoordinator::new();
    run_lifecycle(&mut healthy).unwrap();
    for call in 1..=healthy.currentness_calls {
        let mut coordinator = DestroyCoordinator::new();
        coordinator.fail_currentness = Some(call);
        assert!(run_lifecycle(&mut coordinator).is_err());
        assert!(coordinator.poisoned);
        assert_eq!(coordinator.currentness_calls, call);
        assert!(
            coordinator
                .fake
                .calls
                .iter()
                .filter(|&&c| c == "doorbell")
                .count()
                <= 1
        );
        if !coordinator.destroyed {
            assert!(!coordinator.released);
        }
    }
}

#[test]
fn v2_diagnostic_wrapper_preserves_mode_calls_and_actual_read_zero() {
    let samples = [fresh(), published(0, 1), published(0, 0)];
    let mut plain = Fake::new(samples);
    let expected =
        complete_one_barrier_mode(&mut plain, 0x30_000, Retirement::QueueDestroyV2).unwrap();
    let mut traced = Fake::new(samples);
    let mut diagnostic = Diagnostic::default();
    let actual = complete_one_barrier_traced_mode(
        &mut traced,
        0x30_000,
        &mut diagnostic,
        Retirement::QueueDestroyV2,
    )
    .unwrap();
    assert_eq!(actual, expected);
    assert_eq!(traced.calls, plain.calls);
    assert_eq!(diagnostic.last_snapshot.unwrap().read, 0);
    for fail_at in 1..=plain.calls.len() {
        let mut failed = Fake::new(samples);
        failed.fail_at = Some(fail_at);
        assert!(
            complete_one_barrier_mode(&mut failed, 0x30_000, Retirement::QueueDestroyV2).is_err()
        );
        assert_eq!(failed.calls, plain.calls[..fail_at]);
    }
}

#[test]
fn diagnostics_preserve_success_calls_and_exact_result() {
    let observations = [fresh(), published(0, 1), published(0, 0), published(1, 0)];
    let mut plain = Fake::new(observations);
    let expected = complete_one_barrier(&mut plain, 0x30_000).unwrap();
    let mut traced = Fake::new(observations);
    let mut diagnostic = Diagnostic::default();
    assert_eq!(
        complete_one_barrier_traced(&mut traced, 0x30_000, &mut diagnostic).unwrap(),
        expected
    );
    assert_eq!(traced.calls, plain.calls);
    assert_eq!(traced.pauses, plain.pauses);
    assert_eq!(traced.packet, plain.packet);
    assert_eq!(traced.header, plain.header);
    assert_eq!(traced.doorbell, plain.doorbell);
    assert_eq!(diagnostic.last_snapshot, Some(expected));
    assert_eq!(diagnostic.snapshots, 4);
    assert_eq!(diagnostic.last_counters, Some((0, 0)));
    assert_eq!(diagnostic.reservation_old_write, Some(0));
    assert!(diagnostic.body_written && diagnostic.header_published && diagnostic.doorbell_stored);
    assert!(!diagnostic.deadline_observed_after_error);
}

#[test]
fn diagnostics_preserve_every_original_failure_cutoff_without_an_extra_native_read() {
    let observations = [fresh(), published(1, 0)];
    let mut healthy = Fake::new(observations);
    complete_one_barrier(&mut healthy, 0x30_000).unwrap();
    for fail_at in 1..=healthy.calls.len() {
        let mut plain = Fake::new(observations);
        plain.fail_at = Some(fail_at);
        let expected = complete_one_barrier(&mut plain, 0x30_000).unwrap_err();
        let mut traced = Fake::new(observations);
        traced.fail_at = Some(fail_at);
        let mut diagnostic = Diagnostic::default();
        let error =
            complete_one_barrier_traced(&mut traced, 0x30_000, &mut diagnostic).unwrap_err();
        assert!(error.starts_with(&format!("{expected}; barrier_diagnostic_v1=")));
        assert_eq!(traced.calls, plain.calls);
        assert_eq!(traced.pauses, plain.pauses);
        assert_eq!(traced.packet, plain.packet);
        assert_eq!(traced.header, plain.header);
        assert_eq!(traced.doorbell, plain.doorbell);
        assert!(!diagnostic.deadline_observed_after_error);
        assert_eq!(diagnostic.calls, traced.calls.len() as u64);
    }
}

#[test]
fn diagnostics_keep_pending_signal_and_unretired_counter_after_deadline() {
    for snapshot in [published(0, 1), published(1, 1), published(0, 0)] {
        let mut plain = Fake::new([fresh(), snapshot]);
        let expected = complete_one_barrier(&mut plain, 0x30_000).unwrap_err();
        let mut traced = Fake::new([fresh(), snapshot]);
        let mut diagnostic = Diagnostic::default();
        let error =
            complete_one_barrier_traced(&mut traced, 0x30_000, &mut diagnostic).unwrap_err();
        assert!(error.starts_with(&expected));
        assert_eq!(traced.calls, plain.calls);
        assert_eq!(traced.pauses, plain.pauses);
        assert_eq!(diagnostic.last_snapshot, Some(snapshot));
        assert_eq!(diagnostic.snapshots, 9);
        assert_eq!(diagnostic.attempted, Some(DiagnosticStage::Pause));
        assert!(diagnostic.deadline_observed_after_error);
        assert!(diagnostic.doorbell_stored);
        assert!(!error.contains("196608"));
        assert!(!error.contains("0x30000"));
        assert!(error.len() < 1024);
    }
}

#[test]
fn diagnostics_do_not_report_failed_side_effect_as_successful() {
    let observations = [fresh(), published(1, 0)];
    let mut healthy = Fake::new(observations);
    complete_one_barrier(&mut healthy, 0x30_000).unwrap();
    for (name, stage) in [
        ("reserve", DiagnosticStage::ReserveWrite),
        ("body", DiagnosticStage::PacketBody),
        ("header", DiagnosticStage::ReleaseHeader),
        ("doorbell", DiagnosticStage::Doorbell),
    ] {
        let mut fake = Fake::new(observations);
        fake.fail_at = Some(healthy.calls.iter().position(|call| *call == name).unwrap() + 1);
        let mut diagnostic = Diagnostic::default();
        assert!(complete_one_barrier_traced(&mut fake, 0x30_000, &mut diagnostic).is_err());
        assert_eq!(diagnostic.attempted, Some(stage));
        assert_ne!(diagnostic.completed, Some(stage));
        match name {
            "reserve" => assert_eq!(diagnostic.reservation_old_write, None),
            "body" => assert!(!diagnostic.body_written),
            "header" => assert!(!diagnostic.header_published),
            "doorbell" => assert!(!diagnostic.doorbell_stored),
            _ => unreachable!(),
        }
    }
}

#[test]
fn diagnostics_before_first_call_and_after_currentness_deadline_are_distinct() {
    let mut before = Fake::new([fresh(), published(1, 0)]);
    before.pause_limit = 0;
    let mut diagnostic = Diagnostic::default();
    assert!(complete_one_barrier_traced(&mut before, 0x30_000, &mut diagnostic).is_err());
    assert_eq!(diagnostic.attempted, None);
    assert_eq!(diagnostic.last_snapshot, None);
    assert!(diagnostic.deadline_observed_after_error);
    assert_eq!(diagnostic.calls, 0);

    let mut healthy = Fake::new([fresh(), published(1, 0)]);
    complete_one_barrier(&mut healthy, 0x30_000).unwrap();
    let doorbell = healthy
        .calls
        .iter()
        .position(|call| *call == "doorbell")
        .unwrap();
    let mut after = Fake::new([fresh(), published(1, 0)]);
    // The first polling-loop currentness check crosses the same deadline.
    after.deadline_at_call = Some(doorbell + 2);
    let mut diagnostic = Diagnostic::default();
    assert!(complete_one_barrier_traced(&mut after, 0x30_000, &mut diagnostic).is_err());
    assert_eq!(diagnostic.attempted, Some(DiagnosticStage::Currentness));
    assert_eq!(diagnostic.completed, Some(DiagnosticStage::Doorbell));
    assert_eq!(diagnostic.last_snapshot, Some(fresh()));
    assert!(diagnostic.doorbell_stored && diagnostic.deadline_observed_after_error);
}

fn fresh() -> Snapshot {
    Snapshot {
        write: 0,
        read: 0,
        slot: 0,
        header: AQL_INVALID_PACKET_HEADER_V1,
        setup: 0,
        kind: 1,
        value: 1,
    }
}
fn published(read: u64, value: i64) -> Snapshot {
    Snapshot {
        write: 1,
        read,
        header: AQL_SYSTEM_SCOPED_BARRIER_AND_HEADER_V1,
        value,
        ..fresh()
    }
}
struct Fake {
    snapshots: VecDeque<Snapshot>,
    last: Snapshot,
    calls: Vec<&'static str>,
    fail_at: Option<usize>,
    pauses: usize,
    pause_limit: usize,
    deadline_at_call: Option<usize>,
    counters: (u64, u64),
    packet: Option<[u8; 64]>,
    header: Option<u16>,
    doorbell: Option<u64>,
}
impl Fake {
    fn new(observations: impl IntoIterator<Item = Snapshot>) -> Self {
        Self {
            snapshots: observations.into_iter().collect(),
            last: fresh(),
            calls: vec![],
            fail_at: None,
            pauses: 0,
            pause_limit: 8,
            deadline_at_call: None,
            counters: (0, 0),
            packet: None,
            header: None,
            doorbell: None,
        }
    }
    fn call(&mut self, name: &'static str) -> Result<(), NativeAqlSubmissionErrorV1> {
        self.calls.push(name);
        if self.fail_at == Some(self.calls.len()) {
            Err(NativeAqlSubmissionErrorV1::Currentness)
        } else {
            Ok(())
        }
    }
}
impl NativeAqlSubmissionBackendV1 for Fake {
    fn check_currentness(&mut self) -> Result<(), NativeAqlSubmissionErrorV1> {
        self.call("current")?;
        if self.expired() {
            Err(NativeAqlSubmissionErrorV1::Currentness)
        } else {
            Ok(())
        }
    }
    fn observe_counters_acquire(&mut self) -> Result<(u64, u64), NativeAqlSubmissionErrorV1> {
        self.call("counters")?;
        Ok(self.counters)
    }
    fn fetch_add_write_acq_rel(
        &mut self,
        increment: u64,
    ) -> Result<u64, NativeAqlSubmissionErrorV1> {
        self.call("reserve")?;
        assert_eq!(increment, 1);
        let old = self.counters.0;
        self.counters.0 += increment;
        Ok(old)
    }
    fn write_unpublished(
        &mut self,
        slot: u32,
        packet: &[u8; 64],
    ) -> Result<(), NativeAqlSubmissionErrorV1> {
        self.call("body")?;
        assert_eq!(slot, 0);
        self.packet = Some(*packet);
        Ok(())
    }
    fn publish_release_header(
        &mut self,
        slot: u32,
        header: u16,
    ) -> Result<(), NativeAqlSubmissionErrorV1> {
        self.call("header")?;
        assert_eq!(slot, 0);
        self.header = Some(header);
        Ok(())
    }
    fn ring_doorbell_release(&mut self, packet_id: u64) -> Result<(), NativeAqlSubmissionErrorV1> {
        self.call("doorbell")?;
        assert_eq!(packet_id, 0);
        self.doorbell = Some(packet_id);
        Ok(())
    }
}
impl BarrierBackend for Fake {
    fn snapshot(&mut self) -> QueueResult<Snapshot> {
        self.call("snapshot").map_err(describe)?;
        if let Some(next) = self.snapshots.pop_front() {
            self.last = next;
        }
        Ok(self.last)
    }
    fn expired(&self) -> bool {
        self.pauses >= self.pause_limit
            || self.deadline_at_call.is_some_and(|n| self.calls.len() >= n)
    }
    fn pause(&mut self) {
        self.pauses += 1;
    }
}

#[test]
fn one_barrier_uses_actual_publisher_and_exact_zero_dependency_packet() {
    let mut fake = Fake::new([fresh(), published(1, 0)]);
    assert_eq!(
        complete_one_barrier(&mut fake, 0x30_000).unwrap(),
        published(1, 0)
    );
    let packet = fake.packet.unwrap();
    assert_eq!(
        u16::from_le_bytes(packet[..2].try_into().unwrap()),
        AQL_INVALID_PACKET_HEADER_V1
    );
    assert!(packet[2..56].iter().all(|byte| *byte == 0));
    assert_eq!(
        u64::from_le_bytes(packet[56..].try_into().unwrap()),
        0x30_000
    );
    assert_eq!(fake.header, Some(AQL_SYSTEM_SCOPED_BARRIER_AND_HEADER_V1));
    assert_eq!(fake.doorbell, Some(0));
    assert_eq!(fake.counters, (1, 0));
    assert_eq!(fake.calls.iter().filter(|&&s| s == "reserve").count(), 1);
    let at = |name| fake.calls.iter().position(|s| *s == name).unwrap();
    assert!(
        at("reserve") < at("body") && at("body") < at("header") && at("header") < at("doorbell")
    );
    assert_eq!(fake.calls[at("header") + 1], "current");
}

#[test]
fn signal_zero_alone_does_not_allow_retirement_or_early_success() {
    let mut fake = Fake::new([fresh(), published(0, 0), published(0, 0), published(1, 0)]);
    assert!(
        complete_one_barrier(&mut fake, 0x30_000)
            .unwrap()
            .complete_and_retired()
    );
    assert_eq!(fake.pauses, 2);
    let mut never_retired = Fake::new([fresh(), published(0, 0)]);
    assert!(
        complete_one_barrier(&mut never_retired, 0x30_000)
            .unwrap_err()
            .contains("deadline")
    );
    assert_eq!(never_retired.pauses, 8);
}

#[test]
fn read_one_without_signal_completion_waits_and_times_out() {
    let mut fake = Fake::new([fresh(), published(1, 1)]);
    assert!(
        complete_one_barrier(&mut fake, 0x30_000)
            .unwrap_err()
            .contains("deadline")
    );
    assert_eq!(fake.calls.iter().filter(|&&s| s == "doorbell").count(), 1);
}

#[test]
fn initially_completed_or_wrong_kind_or_nonempty_queue_never_publishes() {
    for changed in [
        Snapshot {
            value: 0,
            ..fresh()
        },
        Snapshot { kind: 0, ..fresh() },
        Snapshot {
            write: 1,
            ..fresh()
        },
        Snapshot { read: 1, ..fresh() },
        Snapshot {
            setup: 1,
            ..fresh()
        },
        Snapshot { slot: 1, ..fresh() },
        Snapshot {
            header: AQL_SYSTEM_SCOPED_BARRIER_AND_HEADER_V1,
            ..fresh()
        },
    ] {
        let mut fake = Fake::new([changed]);
        assert!(complete_one_barrier(&mut fake, 0x30_000).is_err());
        assert!(fake.packet.is_none() && fake.header.is_none() && fake.doorbell.is_none());
        assert_eq!(fake.counters, (0, 0));
    }
}

#[test]
fn malformed_postpublication_signal_counter_or_header_is_terminal() {
    for changed in [
        Snapshot {
            value: -1,
            ..published(1, 0)
        },
        Snapshot {
            value: 2,
            ..published(1, 0)
        },
        Snapshot {
            kind: 2,
            ..published(1, 0)
        },
        Snapshot {
            read: 2,
            ..published(1, 0)
        },
        Snapshot {
            write: 2,
            ..published(1, 0)
        },
        Snapshot {
            write: 0,
            ..published(1, 0)
        },
        Snapshot {
            setup: 1,
            ..published(1, 0)
        },
        Snapshot {
            header: 0xffff,
            ..published(1, 0)
        },
        Snapshot {
            slot: 1,
            ..published(1, 0)
        },
    ] {
        let mut fake = Fake::new([fresh(), changed]);
        assert!(complete_one_barrier(&mut fake, 0x30_000).is_err());
        assert_eq!(fake.calls.iter().filter(|&&s| s == "doorbell").count(), 1);
        assert_eq!(fake.pauses, 0);
    }
}

#[test]
fn counter_and_signal_regressions_after_partial_progress_are_rejected() {
    for observations in [
        [fresh(), published(1, 1), published(0, 1)],
        [fresh(), published(0, 0), published(1, 1)],
    ] {
        let mut fake = Fake::new(observations);
        assert!(
            complete_one_barrier(&mut fake, 0x30_000)
                .unwrap_err()
                .contains("regression")
        );
        assert_eq!(fake.pauses, 1);
    }
}

#[test]
fn consumed_packet_header_may_be_invalidated_but_setup_and_signal_stay_checked() {
    let completed = Snapshot {
        header: AQL_INVALID_PACKET_HEADER_V1,
        ..published(1, 0)
    };
    let mut fake = Fake::new([fresh(), completed]);
    assert_eq!(
        complete_one_barrier(&mut fake, 0x30_000).unwrap(),
        completed
    );
}

#[test]
fn every_publisher_and_completion_checkpoint_failure_stops_without_retry() {
    let mut healthy = Fake::new([fresh(), published(1, 0)]);
    complete_one_barrier(&mut healthy, 0x30_000).unwrap();
    for failure in 1..=healthy.calls.len() {
        let mut fake = Fake::new([fresh(), published(1, 0)]);
        fake.fail_at = Some(failure);
        assert!(complete_one_barrier(&mut fake, 0x30_000).is_err());
        assert_eq!(fake.calls, healthy.calls[..failure]);
        assert!(fake.calls.iter().filter(|&&s| s == "reserve").count() <= 1);
        assert!(fake.calls.iter().filter(|&&s| s == "doorbell").count() <= 1);
    }
}

#[test]
fn expired_before_publication_and_invalid_address_have_no_packet_effect() {
    let mut fake = Fake::new([fresh(), published(1, 0)]);
    fake.pause_limit = 0;
    assert!(complete_one_barrier(&mut fake, 0x30_000).is_err());
    assert!(fake.calls.is_empty());
    let mut fake = Fake::new([fresh(), published(1, 0)]);
    assert!(complete_one_barrier(&mut fake, 0).is_err());
    assert!(fake.packet.is_none() && fake.doorbell.is_none());
}

#[test]
fn deadline_crossing_any_completed_checkpoint_never_publishes_success() {
    let mut healthy = Fake::new([fresh(), published(1, 0)]);
    complete_one_barrier(&mut healthy, 0x30_000).unwrap();
    for deadline in 0..=healthy.calls.len() {
        let mut fake = Fake::new([fresh(), published(1, 0)]);
        fake.deadline_at_call = Some(deadline);
        assert!(complete_one_barrier(&mut fake, 0x30_000).is_err());
    }
}

#[test]
fn counter_change_between_priming_and_reservation_is_refused() {
    let mut fake = Fake::new([fresh(), published(1, 0)]);
    fake.counters = (1, 0);
    assert!(complete_one_barrier(&mut fake, 0x30_000).is_err());
    assert!(fake.packet.is_none() && fake.doorbell.is_none());
    assert!(!fake.calls.contains(&"reserve"));
}

struct Coordinator {
    fake: Fake,
    steps: Vec<Step>,
    poisoned: bool,
}
impl LifecycleBackend for Coordinator {
    fn preflight(&mut self) -> QueueResult<()> {
        Ok(())
    }
    fn currentness(&mut self) -> QueueResult<()> {
        Ok(())
    }
    fn step(&mut self, step: Step) -> QueueResult<()> {
        self.steps.push(step);
        if step == Step::CheckUnpublished {
            complete_one_barrier(&mut self.fake, 0x30_000)?;
        }
        Ok(())
    }
    fn quarantine(&mut self) {
        self.poisoned = true;
    }
}

#[test]
fn actual_coordinator_quarantines_failed_barrier_without_destroy_or_cleanup() {
    for observation in [
        published(0, 0),
        Snapshot {
            kind: 0,
            ..published(1, 0)
        },
    ] {
        let mut coordinator = Coordinator {
            fake: Fake::new([fresh(), observation]),
            steps: vec![],
            poisoned: false,
        };
        assert!(run_lifecycle(&mut coordinator).is_err());
        assert!(coordinator.poisoned);
        assert_eq!(coordinator.steps.last(), Some(&Step::CheckUnpublished));
        assert!(!coordinator.steps.contains(&Step::Destroy));
    }
    let mut coordinator = Coordinator {
        fake: Fake::new([fresh(), published(1, 0)]),
        steps: vec![],
        poisoned: false,
    };
    run_lifecycle(&mut coordinator).unwrap();
    assert_eq!(coordinator.steps, STEPS);
    assert!(!coordinator.poisoned);
}
