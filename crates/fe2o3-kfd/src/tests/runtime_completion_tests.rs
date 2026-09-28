//! Shared runtime receipt transitions with production lower CPU owners, not GPU signals.

// Keep the same inline, move-only failure receipts as the native facade.
#![allow(clippy::result_large_err)]

use super::super::runtime_materialized_completion_receipt::{
    MaterializedCompletionReceiptV1 as Receipt, MaterializedConsumeV1 as Consume,
};
use super::runtime_publication_tests::{fixture, owner, restored};
use super::*;
use crate::queue::completion::{
    CompletionBatchOccurrenceV1, CompletionCustodySnapshotV1, Gfx942ComputeEventOccurrenceV1,
};
use crate::queue::dispatch_binding::TestOnlyMultiInflightDispatchOwnerV1 as DispatchOwner;
use fe2o3_aql::AqlCompletionObservationV1;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Fault {
    Currentness(usize),
    Observe,
    Reset,
}

pub(super) struct CpuSignals {
    observation: AqlCompletionObservationV1,
    fault: Option<Fault>,
    panic: bool,
    pub(super) checks: usize,
    pub(super) observed: Vec<u32>,
    pub(super) resets: Vec<u32>,
}

impl CpuSignals {
    pub(super) fn new(observation: AqlCompletionObservationV1) -> Self {
        Self {
            observation,
            fault: None,
            panic: false,
            checks: 0,
            observed: Vec::new(),
            resets: Vec::new(),
        }
    }

    fn check_fault(
        &self,
        at: Fault,
        error: Gfx942CompletionErrorV1,
    ) -> Result<(), Gfx942CompletionErrorV1> {
        if self.fault == Some(at) {
            assert!(!self.panic, "completion-cell-signal-unwind");
            Err(error)
        } else {
            Ok(())
        }
    }

    fn assert_prefix(&self, operation: Consume, slot: u32) {
        assert_eq!(
            self.checks,
            match self.fault {
                Some(Fault::Currentness(1) | Fault::Observe | Fault::Reset) => 1,
                _ => 2,
            }
        );
        let entered = self.fault != Some(Fault::Currentness(1));
        let expected = [slot];
        assert_eq!(
            self.observed.as_slice(),
            if entered && operation == Consume::Poll {
                &expected[..]
            } else {
                &[]
            }
        );
        assert_eq!(
            self.resets.as_slice(),
            if entered && operation == Consume::Recycle {
                &expected[..]
            } else {
                &[]
            }
        );
        if entered && operation == Consume::Recycle {
            assert!(matches!(
                self.observation,
                AqlCompletionObservationV1::Pending
            ));
        }
    }
}

impl NativeCompletionSignalBackendV1 for CpuSignals {
    fn check_currentness(&mut self) -> Result<(), Gfx942CompletionErrorV1> {
        self.checks += 1;
        self.check_fault(
            Fault::Currentness(self.checks),
            Gfx942CompletionErrorV1::Currentness,
        )
    }

    fn observe_one_acquire_in_current_scope(
        &mut self,
        _: u32,
    ) -> Result<AqlCompletionObservationV1, Gfx942CompletionErrorV1> {
        panic!("ordinary batch polling must use its batch signal backend")
    }

    fn observe_batch_acquire_in_current_scope(
        &mut self,
        slots: &[u32],
    ) -> Result<Vec<AqlCompletionObservationV1>, Gfx942CompletionErrorV1> {
        self.observed.extend_from_slice(slots);
        self.check_fault(Fault::Observe, Gfx942CompletionErrorV1::Observation)?;
        Ok(vec![self.observation; slots.len()])
    }

    fn reset_pending_release(&mut self, slot: u32) -> Result<(), Gfx942CompletionErrorV1> {
        self.resets.push(slot);
        // A reset may have taken effect even when its callback fails or unwinds.
        self.observation = AqlCompletionObservationV1::Pending;
        self.check_fault(Fault::Reset, Gfx942CompletionErrorV1::Recycle)
    }
}

type Publication = (
    DispatchEpochIdentityV1,
    CompletionBatchOccurrenceV1,
    u32,
    Vec<Gfx942ComputeEventOccurrenceV1>,
    Receipt,
);

fn publication(
    session: &mut ComputeAqlQueueSessionV1,
    dispatch: &mut DispatchOwner,
    pinned: bool,
) -> Publication {
    let packet_id = dispatch.next_generation();
    let template = test_completion_template(session.key, packet_id);
    let identity = dispatch.reserve_one(session.key, template).unwrap();
    let bound = session.completion_owner.bind_batch([template]).unwrap();
    let events = if pinned {
        let acceptance = session.dependency_owner.reserve_acceptance_epoch().unwrap();
        session
            .completion_owner
            .record_dependency_event_batch_for_bound_v1(
                acceptance.session_occurrence(),
                acceptance.epoch(),
                &bound,
            )
            .unwrap()
    } else {
        Vec::new()
    };
    let (_, retention) = bound.into_parts();
    let completion = session
        .completion_owner
        .mark_published(retention, packet_id)
        .unwrap();
    let events = if pinned {
        session
            .completion_owner
            .bind_dependency_event_batch_v1(events, &completion)
            .unwrap()
    } else {
        events
    };
    dispatch.mark_published(identity, &completion).unwrap();
    let occurrence = completion.occurrence_v1().unwrap();
    let slot = completion.first_packet_and_signal_slot().unwrap().1;
    (
        identity,
        occurrence,
        slot,
        events,
        Receipt::Published(wrap_published(completion, identity)),
    )
}

fn poll_cpu(
    session: &mut ComputeAqlQueueSessionV1,
    dispatch: &mut DispatchOwner,
    batch: Gfx942DispatchBatchV1<1>,
    signals: &mut CpuSignals,
    expected_identity: DispatchEpochIdentityV1,
    expected_occurrence: CompletionBatchOccurrenceV1,
) -> Result<Gfx942DispatchPollV1<1>, ComputeAqlQueueSessionErrorV1> {
    let result = (|| {
        let (completion, identity) = unwrap_published(batch);
        assert_eq!(identity, expected_identity);
        assert_eq!(
            dispatch.validate_published(identity, &completion)?,
            expected_occurrence
        );
        match session
            .completion_owner
            .observe_once_with_progress(completion, signals)?
        {
            Gfx942CompletionPollWithProgressV1::Pending { batch, .. } => {
                assert_eq!(batch.occurrence_v1()?, expected_occurrence);
                Ok(Gfx942DispatchPollV1::Pending(wrap_published(
                    batch, identity,
                )))
            }
            Gfx942CompletionPollWithProgressV1::Ready { completed, .. } => {
                assert_eq!(completed.occurrence_v1()?, expected_occurrence);
                dispatch.mark_completed(identity, expected_occurrence)?;
                Ok(Gfx942DispatchPollV1::Ready(wrap_completed(
                    completed, identity,
                )))
            }
        }
    })();
    if result.is_err() {
        dispatch.poison();
    }
    session.terminalize_fixed_dispatch_observation_result_v1(result)
}

fn recycle_cpu(
    session: &mut ComputeAqlQueueSessionV1,
    dispatch: &mut DispatchOwner,
    completed: Gfx942CompletedDispatchBatchV1<1>,
    signals: &mut CpuSignals,
    expected_identity: DispatchEpochIdentityV1,
    expected_occurrence: CompletionBatchOccurrenceV1,
) -> Result<Gfx942CompletionRecycleObservationV1, Gfx942FixedDispatchRecycleFailureV1<1>> {
    let (completion, identity) = unwrap_completed(completed);
    assert_eq!(identity, expected_identity);
    assert_eq!(
        dispatch.validate_completed(identity, &completion).unwrap(),
        expected_occurrence
    );
    let result = match session
        .completion_owner
        .recycle_retaining(completion, signals)
    {
        Ok(observation) => {
            dispatch
                .mark_recycled(identity, expected_occurrence)
                .unwrap();
            Ok(observation)
        }
        Err((error, completion)) => Err(
            Gfx942FixedDispatchRecycleFailureV1::from_completion_failure(
                error.into(),
                completion,
                identity,
            ),
        ),
    };
    if result
        .as_ref()
        .is_err_and(|f| f.retryable_completed.is_none())
    {
        dispatch.poison();
    }
    session.terminalize_fixed_dispatch_recycle_result_v1(result)
}

struct Fixture {
    session: ComputeAqlQueueSessionV1,
    lane: ComputeAqlQueueLaneV1,
    ordinal: usize,
    dispatch: DispatchOwner,
    receipt: Receipt,
    identity: DispatchEpochIdentityV1,
    occurrence: CompletionBatchOccurrenceV1,
    slot: u32,
    events: Vec<Gfx942ComputeEventOccurrenceV1>,
    available: usize,
    untouched: CompletionCustodySnapshotV1,
}

impl Fixture {
    fn new(auxiliary: bool, pinned: bool) -> Self {
        let (mut session, lane) = fixture(auxiliary);
        let ordinal = usize::from(auxiliary);
        let available = owner(&session, ordinal).state_snapshot_for_test().1;
        let untouched = owner(&session, 1 - ordinal).custody_snapshot_for_test();
        let mut dispatch = DispatchOwner::new();
        let (identity, occurrence, slot, events, receipt) = session
            .with_compute_lane_v1(lane, |selected| {
                publication(selected.session, &mut dispatch, pinned)
            })
            .unwrap();
        Self {
            session,
            lane,
            ordinal,
            dispatch,
            receipt,
            identity,
            occurrence,
            slot,
            events,
            available,
            untouched,
        }
    }

    fn in_lane<T>(
        &mut self,
        operation: impl FnOnce(
            &mut ComputeAqlQueueSessionV1,
            &mut DispatchOwner,
            &mut Receipt,
            DispatchEpochIdentityV1,
            CompletionBatchOccurrenceV1,
        ) -> T,
    ) -> T {
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            self.session.with_compute_lane_v1(self.lane, |selected| {
                operation(
                    selected.session,
                    &mut self.dispatch,
                    &mut self.receipt,
                    self.identity,
                    self.occurrence,
                )
            })
        }));
        match result {
            Ok(result) => result.unwrap(),
            Err(payload) => {
                // The test dispatch owner is external to the real lane's owner.
                self.dispatch.poison();
                std::panic::resume_unwind(payload)
            }
        }
    }

    fn poll(&mut self, signals: &mut CpuSignals) -> Result<bool, ComputeAqlQueueSessionErrorV1> {
        self.in_lane(|s, d, receipt, id, occurrence| {
            receipt.poll_ready(|batch| poll_cpu(s, d, batch, signals, id, occurrence))
        })
    }

    fn recycle(&mut self, signals: &mut CpuSignals) -> Result<bool, ComputeAqlQueueSessionErrorV1> {
        self.in_lane(|s, d, receipt, id, occurrence| {
            receipt
                .recycle_retired(|completed| recycle_cpu(s, d, completed, signals, id, occurrence))
        })
    }

    fn ready(&mut self) {
        let mut signals = CpuSignals::new(AqlCompletionObservationV1::Completed);
        assert!(self.poll(&mut signals).unwrap());
        signals.assert_prefix(Consume::Poll, self.slot);
        assert!(matches!(self.receipt, Receipt::Completed(_)));
    }

    fn release_events(&mut self) {
        let events = std::mem::take(&mut self.events);
        let count = events.len();
        assert_eq!(
            self.in_lane(|s, _, _, _, _| {
                s.completion_owner
                    .release_dependency_event_batch_v1(events)
                    .unwrap()
            }),
            count
        );
    }

    fn assert_clean(&self) {
        assert!(matches!(&self.receipt, Receipt::Retired(o) if o.packet_count() == 1));
        assert!(!self.session.terminal_poisoned);
        assert!(self.events.is_empty());
        assert_eq!(self.dispatch.live_epoch_count(), 0);
        self.dispatch.ensure_releasable().unwrap();
        owner(&self.session, self.ordinal)
            .ensure_releasable()
            .unwrap();
        assert_eq!(
            owner(&self.session, self.ordinal)
                .state_snapshot_for_test()
                .1,
            self.available
        );
        assert_eq!(
            owner(&self.session, 1 - self.ordinal).custody_snapshot_for_test(),
            self.untouched
        );
        restored(&self.session);
    }

    fn assert_terminal(&mut self) {
        assert!(self.session.terminal_poisoned);
        assert!(self.dispatch.ensure_releasable().is_err());
        assert_eq!(
            owner(&self.session, 1 - self.ordinal).custody_snapshot_for_test(),
            self.untouched
        );
        let retained = owner(&self.session, self.ordinal).custody_snapshot_for_test();
        assert!(
            self.session
                .with_compute_lane_v1(self.lane, |_| panic!("terminal reentry"))
                .is_err()
        );
        assert_eq!(
            owner(&self.session, self.ordinal).custody_snapshot_for_test(),
            retained
        );
        restored(&self.session);
    }
}

#[test]
fn runtime_completion_cell_preserves_pending_ready_and_retired_occurrences() {
    for auxiliary in [false, true] {
        let mut f = Fixture::new(auxiliary, false);
        let before = owner(&f.session, f.ordinal).custody_snapshot_for_test();
        for _ in 0..2 {
            let mut signals = CpuSignals::new(AqlCompletionObservationV1::Pending);
            assert!(!f.poll(&mut signals).unwrap());
            assert!(matches!(f.receipt, Receipt::Published(_)));
            signals.assert_prefix(Consume::Poll, f.slot);
            assert_eq!(
                owner(&f.session, f.ordinal).custody_snapshot_for_test(),
                before
            );
            assert_eq!(f.dispatch.live_epoch_count(), 1);
        }
        f.ready();
        let mut signals = CpuSignals::new(AqlCompletionObservationV1::Completed);
        assert!(f.recycle(&mut signals).unwrap());
        signals.assert_prefix(Consume::Recycle, f.slot);
        f.assert_clean();
    }
}

#[test]
fn runtime_completion_cell_genuine_pin_retry_preserves_then_reuses_generation() {
    for auxiliary in [false, true] {
        let mut f = Fixture::new(auxiliary, true);
        let first = owner(&f.session, f.ordinal)
            .dependency_source_identity_v1(&f.events[0])
            .unwrap();
        f.ready();
        let before = owner(&f.session, f.ordinal).custody_snapshot_for_test();
        let mut signals = CpuSignals::new(AqlCompletionObservationV1::Completed);
        assert!(!f.recycle(&mut signals).unwrap());
        assert!(matches!(f.receipt, Receipt::Completed(_)));
        assert_eq!(
            (signals.checks, signals.observed.len(), signals.resets.len()),
            (0, 0, 0)
        );
        assert_eq!(
            owner(&f.session, f.ordinal).custody_snapshot_for_test(),
            before
        );
        assert_eq!(
            owner(&f.session, f.ordinal)
                .dependency_source_identity_v1(&f.events[0])
                .unwrap(),
            first
        );
        assert_eq!(f.dispatch.live_epoch_count(), 1);
        f.release_events();
        assert!(f.recycle(&mut signals).unwrap());
        signals.assert_prefix(Consume::Recycle, f.slot);
        f.assert_clean();
        let (identity, occurrence, slot, events, receipt) =
            f.in_lane(|s, d, _, _, _| publication(s, d, true));
        f.identity = identity;
        f.occurrence = occurrence;
        f.slot = slot;
        f.events = events;
        f.receipt = receipt;
        let next = owner(&f.session, f.ordinal)
            .dependency_source_identity_v1(&f.events[0])
            .unwrap();
        assert_eq!(next.slot_index, first.slot_index);
        assert_eq!(next.slot_generation, first.slot_generation + 1);
        assert_eq!(next.dispatch_generation, first.dispatch_generation + 1);
        assert!(next.acceptance_epoch > first.acceptance_epoch);
        f.release_events();
        f.ready();
        assert!(
            f.recycle(&mut CpuSignals::new(AqlCompletionObservationV1::Completed))
                .unwrap()
        );
        f.assert_clean();
    }
}

#[test]
fn runtime_completion_cell_wrong_phase_never_consumes_or_invokes_callbacks() {
    for auxiliary in [false, true] {
        let mut f = Fixture::new(auxiliary, false);
        let before = owner(&f.session, f.ordinal).custody_snapshot_for_test();
        assert!(
            f.receipt
                .recycle_retired(|_| panic!("published recycle callback"))
                .is_err()
        );
        assert!(matches!(f.receipt, Receipt::Published(_)));
        assert_eq!(
            owner(&f.session, f.ordinal).custody_snapshot_for_test(),
            before
        );
        f.ready();
        assert!(
            f.receipt
                .poll_ready(|_| panic!("completed poll callback"))
                .is_err()
        );
        assert!(matches!(f.receipt, Receipt::Completed(_)));
        assert!(
            f.recycle(&mut CpuSignals::new(AqlCompletionObservationV1::Completed))
                .unwrap()
        );
        assert!(
            f.receipt
                .poll_ready(|_| panic!("retired poll callback"))
                .is_err()
        );
        assert!(
            f.receipt
                .recycle_retired(|_| panic!("retired recycle callback"))
                .is_err()
        );
        f.assert_clean();
    }
}

#[test]
fn runtime_completion_cell_returned_custody_not_diagnostic_authorizes_retry() {
    for auxiliary in [false, true] {
        let mut f = Fixture::new(auxiliary, false);
        f.ready();
        // Inject a facade diagnostic with a genuine completed receipt. This is
        // not a claim that the native preselection path executed in this fixture.
        let before = owner(&f.session, f.ordinal).custody_snapshot_for_test();
        assert!(
            !f.receipt
                .recycle_retired(|completed| Err(Gfx942FixedDispatchRecycleFailureV1 {
                    error: Gfx942DispatchBindingErrorV1::ResourcePhase.into(),
                    retryable_completed: Some(completed),
                }))
                .unwrap()
        );
        assert!(matches!(f.receipt, Receipt::Completed(_)));
        assert_eq!(
            owner(&f.session, f.ordinal).custody_snapshot_for_test(),
            before
        );
        assert!(
            f.recycle(&mut CpuSignals::new(AqlCompletionObservationV1::Completed))
                .unwrap()
        );
        f.assert_clean();
    }
}

#[test]
fn runtime_completion_cell_poll_errors_never_mint_completed_custody() {
    for auxiliary in [false, true] {
        for fault in [
            Some(Fault::Currentness(1)),
            Some(Fault::Observe),
            Some(Fault::Currentness(2)),
            None,
        ] {
            assert!(!take_dispatch_terminal_process_gate_record_v1());
            let mut f = Fixture::new(auxiliary, false);
            let before = owner(&f.session, f.ordinal).custody_snapshot_for_test();
            let mut signals = CpuSignals::new(if fault.is_none() {
                AqlCompletionObservationV1::Unexpected(-7)
            } else {
                AqlCompletionObservationV1::Completed
            });
            signals.fault = fault;
            assert!(f.poll(&mut signals).is_err());
            assert!(matches!(f.receipt, Receipt::Consuming(Consume::Poll)));
            signals.assert_prefix(Consume::Poll, f.slot);
            assert_eq!(
                owner(&f.session, f.ordinal).custody_snapshot_for_test(),
                before
            );
            assert!(take_dispatch_terminal_process_gate_record_v1());
            assert!(owner(&f.session, f.ordinal).is_poisoned_for_test());
            assert_eq!(f.dispatch.live_epoch_count(), 1);
            assert!(
                f.receipt
                    .poll_ready(|_| panic!("consumed poll callback"))
                    .is_err()
            );
            assert!(matches!(f.receipt, Receipt::Consuming(Consume::Poll)));
            f.assert_terminal();
        }
    }
}

#[test]
fn runtime_completion_cell_recycle_faults_retain_generation_and_consuming_state() {
    for auxiliary in [false, true] {
        for fault in [Fault::Currentness(1), Fault::Reset, Fault::Currentness(2)] {
            assert!(!take_dispatch_terminal_process_gate_record_v1());
            let mut f = Fixture::new(auxiliary, false);
            f.ready();
            let before = owner(&f.session, f.ordinal).custody_snapshot_for_test();
            let mut signals = CpuSignals::new(AqlCompletionObservationV1::Completed);
            signals.fault = Some(fault);
            assert!(f.recycle(&mut signals).is_err());
            assert!(matches!(f.receipt, Receipt::Consuming(Consume::Recycle)));
            signals.assert_prefix(Consume::Recycle, f.slot);
            assert_eq!(
                owner(&f.session, f.ordinal).custody_snapshot_for_test(),
                before
            );
            assert!(take_dispatch_terminal_process_gate_record_v1());
            assert!(owner(&f.session, f.ordinal).is_poisoned_for_test());
            assert_eq!(f.dispatch.live_epoch_count(), 1);
            assert!(
                f.receipt
                    .recycle_retired(|_| panic!("consumed recycle callback"))
                    .is_err()
            );
            assert!(matches!(f.receipt, Receipt::Consuming(Consume::Recycle)));
            f.assert_terminal();
        }
    }
}

#[test]
fn runtime_completion_cell_signal_unwind_keeps_consuming_custody() {
    for auxiliary in [false, true] {
        for operation in [Consume::Poll, Consume::Recycle] {
            for fault in [
                Fault::Currentness(1),
                if operation == Consume::Poll {
                    Fault::Observe
                } else {
                    Fault::Reset
                },
                Fault::Currentness(2),
            ] {
                assert!(!take_lane_unwind_process_gate_record_v1());
                let mut f = Fixture::new(auxiliary, false);
                if operation == Consume::Recycle {
                    f.ready();
                }
                let before = owner(&f.session, f.ordinal).custody_snapshot_for_test();
                let mut signals = CpuSignals::new(AqlCompletionObservationV1::Completed);
                signals.fault = Some(fault);
                signals.panic = true;
                let panic =
                    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| match operation {
                        Consume::Poll => f.poll(&mut signals).unwrap(),
                        Consume::Recycle => f.recycle(&mut signals).unwrap(),
                    }))
                    .unwrap_err();
                assert_eq!(
                    panic.downcast_ref::<&str>(),
                    Some(&"completion-cell-signal-unwind")
                );
                assert!(matches!(f.receipt, Receipt::Consuming(actual) if actual == operation));
                signals.assert_prefix(operation, f.slot);
                assert_eq!(
                    owner(&f.session, f.ordinal).custody_snapshot_for_test(),
                    before
                );
                assert_eq!(f.dispatch.live_epoch_count(), 1);
                assert!(take_lane_unwind_process_gate_record_v1());
                assert!(owner(&f.session, 0).is_poisoned_for_test());
                f.assert_terminal();
            }
        }
    }
}

#[test]
fn runtime_completion_cell_outer_unwind_preserves_each_deposited_receipt() {
    for auxiliary in [false, true] {
        for outcome in 0..4 {
            assert!(!take_lane_unwind_process_gate_record_v1());
            let mut f = Fixture::new(auxiliary, outcome == 2);
            if outcome >= 2 {
                f.ready();
            }
            let mut retained = None;
            let slot = f.slot;
            let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                f.in_lane(|s, d, receipt, id, occurrence| {
                    let mut signals = CpuSignals::new(if outcome == 0 {
                        AqlCompletionObservationV1::Pending
                    } else {
                        AqlCompletionObservationV1::Completed
                    });
                    if outcome < 2 {
                        assert_eq!(
                            receipt
                                .poll_ready(|batch| poll_cpu(
                                    s,
                                    d,
                                    batch,
                                    &mut signals,
                                    id,
                                    occurrence
                                ))
                                .unwrap(),
                            outcome == 1
                        );
                        signals.assert_prefix(Consume::Poll, slot);
                    } else {
                        assert_eq!(
                            receipt
                                .recycle_retired(|completed| recycle_cpu(
                                    s,
                                    d,
                                    completed,
                                    &mut signals,
                                    id,
                                    occurrence
                                ))
                                .unwrap(),
                            outcome == 3
                        );
                        if outcome == 2 {
                            assert_eq!(
                                (signals.checks, signals.observed.len(), signals.resets.len()),
                                (0, 0, 0)
                            );
                        } else {
                            signals.assert_prefix(Consume::Recycle, slot);
                        }
                    }
                    retained = Some(s.completion_owner.custody_snapshot_for_test());
                    std::panic::panic_any("completion-cell-outer-unwind");
                });
            }))
            .unwrap_err();
            assert_eq!(
                panic.downcast_ref::<&str>(),
                Some(&"completion-cell-outer-unwind")
            );
            match outcome {
                0 => assert!(matches!(f.receipt, Receipt::Published(_))),
                1 | 2 => assert!(matches!(f.receipt, Receipt::Completed(_))),
                3 => assert!(matches!(&f.receipt, Receipt::Retired(o) if o.packet_count() == 1)),
                _ => unreachable!(),
            }
            assert_eq!(f.dispatch.live_epoch_count(), usize::from(outcome != 3));
            assert_eq!(
                owner(&f.session, f.ordinal).custody_snapshot_for_test(),
                retained.unwrap()
            );
            assert!(take_lane_unwind_process_gate_record_v1());
            f.assert_terminal();
            // No clearing poison or simulated cleanup after terminal custody.
            assert!(f.session.destroy().is_err());
        }
    }
}

fn ready_publication(
    session: &mut ComputeAqlQueueSessionV1,
    dispatch: &mut DispatchOwner,
    publication: &mut Publication,
) {
    let (identity, occurrence, slot, _, receipt) = publication;
    let mut signals = CpuSignals::new(AqlCompletionObservationV1::Completed);
    assert!(
        receipt
            .poll_ready(|batch| poll_cpu(
                session,
                dispatch,
                batch,
                &mut signals,
                *identity,
                *occurrence
            ))
            .unwrap()
    );
    signals.assert_prefix(Consume::Poll, *slot);
    assert!(matches!(receipt, Receipt::Completed(_)));
}

fn retire_publication(
    session: &mut ComputeAqlQueueSessionV1,
    dispatch: &mut DispatchOwner,
    publication: &mut Publication,
) {
    let (identity, occurrence, slot, events, receipt) = publication;
    assert_eq!(
        session
            .completion_owner
            .release_dependency_event_batch_v1(std::mem::take(events))
            .unwrap(),
        1
    );
    let mut signals = CpuSignals::new(AqlCompletionObservationV1::Completed);
    assert!(
        receipt
            .recycle_retired(|completed| recycle_cpu(
                session,
                dispatch,
                completed,
                &mut signals,
                *identity,
                *occurrence
            ))
            .unwrap()
    );
    signals.assert_prefix(Consume::Recycle, *slot);
    assert!(matches!(receipt, Receipt::Retired(o) if o.packet_count() == 1));
}

#[test]
fn runtime_completion_cell_three_inflight_preserve_survivors_across_slot_reuse() {
    for auxiliary in [false, true] {
        let (mut session, lane) = fixture(auxiliary);
        let ordinal = usize::from(auxiliary);
        let available = owner(&session, ordinal).state_snapshot_for_test().1;
        let untouched = owner(&session, 1 - ordinal).custody_snapshot_for_test();
        let mut dispatch = DispatchOwner::new();
        let mut entries = session
            .with_compute_lane_v1(lane, |selected| {
                (0..3)
                    .map(|_| publication(selected.session, &mut dispatch, true))
                    .collect::<Vec<_>>()
            })
            .unwrap();
        assert_eq!(entries.iter().map(|p| p.2).collect::<Vec<_>>(), [0, 1, 2]);
        assert_eq!(
            entries
                .iter()
                .map(|p| p.1.first_packet_id)
                .collect::<Vec<_>>(),
            [1, 2, 3]
        );
        let original_events = entries
            .iter()
            .map(|entry| {
                owner(&session, ordinal)
                    .dependency_source_identity_v1(&entry.3[0])
                    .unwrap()
            })
            .collect::<Vec<_>>();
        assert_eq!(dispatch.live_epoch_count(), 3);
        assert_eq!(dispatch.next_generation(), 4);
        session
            .with_compute_lane_v1(lane, |selected| {
                ready_publication(selected.session, &mut dispatch, &mut entries[2]);
                ready_publication(selected.session, &mut dispatch, &mut entries[1]);
                retire_publication(selected.session, &mut dispatch, &mut entries[1]);
            })
            .unwrap();
        assert!(matches!(entries[0].4, Receipt::Published(_)));
        assert!(matches!(entries[2].4, Receipt::Completed(_)));
        assert_eq!(dispatch.live_epoch_count(), 2);
        assert_eq!(
            owner(&session, ordinal).state_snapshot_for_test().1,
            available - 2
        );
        for index in [0, 2] {
            assert_eq!(
                owner(&session, ordinal)
                    .dependency_source_identity_v1(&entries[index].3[0])
                    .unwrap(),
                original_events[index]
            );
        }
        let replacement = session
            .with_compute_lane_v1(lane, |selected| {
                publication(selected.session, &mut dispatch, true)
            })
            .unwrap();
        let replacement_event = owner(&session, ordinal)
            .dependency_source_identity_v1(&replacement.3[0])
            .unwrap();
        assert_eq!(replacement.2, entries[1].2);
        assert_eq!(
            replacement_event.slot_generation,
            original_events[1].slot_generation + 1
        );
        assert_eq!(replacement.1.first_packet_id, 4);
        assert!(replacement_event.dispatch_generation > original_events[2].dispatch_generation);
        assert!(replacement_event.acceptance_epoch > original_events[2].acceptance_epoch);
        assert_eq!(dispatch.live_epoch_count(), 3);
        assert_eq!(dispatch.next_generation(), 5);
        for index in [0, 2] {
            assert_eq!(
                owner(&session, ordinal)
                    .dependency_source_identity_v1(&entries[index].3[0])
                    .unwrap(),
                original_events[index]
            );
        }
        entries.push(replacement);
        for (retired, index) in [3, 2, 0].into_iter().enumerate() {
            session
                .with_compute_lane_v1(lane, |selected| {
                    if matches!(entries[index].4, Receipt::Published(_)) {
                        ready_publication(selected.session, &mut dispatch, &mut entries[index]);
                    }
                    retire_publication(selected.session, &mut dispatch, &mut entries[index]);
                })
                .unwrap();
            assert_eq!(dispatch.live_epoch_count(), 2 - retired);
            assert_eq!(
                owner(&session, ordinal).state_snapshot_for_test().1,
                available - 2 + retired
            );
            assert_eq!(
                owner(&session, 1 - ordinal).custody_snapshot_for_test(),
                untouched
            );
            restored(&session);
        }
        for entry in entries {
            assert!(matches!(entry.4, Receipt::Retired(o) if o.packet_count() == 1));
            assert!(entry.3.is_empty());
        }
        dispatch.ensure_releasable().unwrap();
        owner(&session, ordinal).ensure_releasable().unwrap();
        assert!(!session.terminal_poisoned);
    }
}
