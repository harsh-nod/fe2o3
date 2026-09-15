//! Prepared persistent owners and real model loans; native calls remain CPU fixtures.

use super::*;
use crate::queue::dispatch_binding::control_release::{
    RetainedControlSnapshotV1 as OwnerSnapshot, ReturningControlCleanupCustodyV1 as ControlRoot,
};
use crate::queue::dispatch_binding::preparation::{
    recycle_and_detach_persistent_fixture_v1, single_persistent_control_in_memory_v1,
    three_persistent_control_in_memory_v1,
};
use crate::queue::live::retained_control_release::{
    RetainedControlReleaseContextV1, SettledRetainedControlReleaseV1,
    settle_retained_control_release_v1,
};
use crate::shared_memory::{
    CleanupStageV1, ControlReleaseMemorySnapshotV1 as MemorySnapshot, ControlReleasePrefixV1,
};
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

struct SecondaryPanic(Arc<AtomicUsize>);

impl Drop for SecondaryPanic {
    fn drop(&mut self) {
        self.0.fetch_add(1, Ordering::SeqCst);
        panic!("secondary retained-control panic destructor");
    }
}

#[derive(Default)]
struct Ledger {
    generation: Option<u64>,
    count: usize,
    identities: Vec<Gfx942FixedDispatchStorageIdentityV1>,
    next: Option<usize>,
}

#[derive(Default)]
struct ControlTrace {
    loans: usize,
    entered: usize,
    before_cleanup: Option<MemorySnapshot>,
    before_retake: Option<MemorySnapshot>,
    retained: Option<ControlRoot>,
    poisons: usize,
    panicked: bool,
    skip_callback: bool,
    envelope_after: Outcome,
    panic_poison: bool,
    secondary_envelope: Option<Arc<AtomicUsize>>,
    secondary_poison: Option<Arc<AtomicUsize>>,
}

struct ControlContext<'a> {
    parent: &'a mut Parent,
    lane: Option<&'a mut ComputeAqlQueueLaneStateV1<Fixture>>,
    primary: &'a mut Ledger,
    trace: &'a mut ControlTrace,
}

impl RetainedControlReleaseContextV1 for ControlContext<'_> {
    type Memory = Memory;

    fn require_releasable(&self) -> Result<(), ComputeAqlQueueSessionErrorV1> {
        if self.parent.poisoned {
            return Err(Gfx942DispatchBindingErrorV1::Poisoned.into());
        }
        Ok(())
    }

    fn dispatch(&mut self) -> &mut Option<DispatchResourceOwnerV1> {
        if let Some(lane) = self.lane.as_mut() {
            &mut lane.dispatch
        } else {
            &mut self
                .parent
                .original
                .as_mut()
                .unwrap()
                .primary
                .completed
                .as_mut()
                .unwrap()
                .dispatch
        }
    }

    fn detached_generation(&self) -> Option<u64> {
        self.lane.as_ref().map_or(self.primary.generation, |lane| {
            lane.detached_dispatch_generation
        })
    }

    fn check_currentness(&mut self) -> Result<(), ComputeAqlQueueSessionErrorV1> {
        self.parent
            .original
            .as_mut()
            .unwrap()
            .primary
            .completed
            .as_mut()
            .unwrap()
            .engine
            .backend
            .session
            .primary_currentness()
            .map_err(Into::into)
    }

    fn with_memory_custody(
        &mut self,
        operation: impl FnOnce(&mut Memory),
    ) -> Result<((), Result<(), ComputeAqlQueueSessionErrorV1>), ComputeAqlQueueSessionErrorV1>
    {
        self.trace.loans += 1;
        if self.trace.skip_callback {
            return Ok(((), Ok(())));
        }
        let trace = &mut self.trace;
        let (callback, retake) = self.parent.with_preparation_custody(|memory| {
            trace.entered += 1;
            trace.before_cleanup = Some(memory.control_release_loan_snapshot_v1());
            operation(memory);
            trace.before_retake = Some(memory.control_release_loan_snapshot_v1());
            Ok(())
        })?;
        if let Some(drops) = &self.trace.secondary_envelope {
            std::panic::panic_any(SecondaryPanic(drops.clone()));
        }
        outcome("retained-control-envelope", self.trace.envelope_after)?;
        // Keep closing failure inside the envelope, distinct from an opening rejection.
        Ok(((), retake.and(callback)))
    }

    fn retain(&mut self, root: ControlRoot) {
        assert!(self.trace.retained.is_none());
        self.trace.retained = Some(root);
    }

    fn poison(&mut self, panicked: bool) {
        assert!(
            self.trace.retained.is_some() || self.dispatch().is_some(),
            "retain the cleanup root or restore the owner before poison"
        );
        self.trace.poisons += 1;
        self.trace.panicked |= panicked;
        self.parent.poison();
        if let Some(lane) = self.lane.as_mut() {
            lane.completion_owner.poison_owner();
            lane.submission.as_mut().unwrap().poison();
        }
        if let Some(drops) = &self.trace.secondary_poison {
            std::panic::panic_any(SecondaryPanic(drops.clone()));
        }
        if self.trace.panic_poison {
            std::panic::panic_any("retained control poison");
        }
    }
}

struct ControlFixture {
    scope: Box<Scope>,
    slot: Option<usize>,
    lane: Option<Box<ComputeAqlQueueLaneStateV1<Fixture>>>,
    primary: Ledger,
    trace: ControlTrace,
    displaced: DispatchResourceOwnerV1,
    data: Vec<Gfx942FixedDispatchDataV1>,
    generation: u64,
    key: QueueKeyV1,
}

impl ControlFixture {
    fn new(ordinal: usize, bindings: usize) -> Self {
        let (mut scope, result, _) = prefix_case_with_probe(
            false,
            |_| {},
            |_, _| {},
            run_auxiliary,
            |scope, failed| {
                assert!(!failed);
                assert_pair(scope);
            },
            ordinal != 0,
        );
        assert!(result.is_ok());
        if ordinal == 2 {
            let original = scope.parent.original.as_mut().unwrap();
            let state = original.lanes[0].state.take();
            let generation = original.lanes[0].generation;
            original
                .lanes
                .push(AuxiliaryComputeLaneSlotV1 { generation, state });
        }
        let slot = (ordinal != 0).then_some(ordinal.saturating_sub(1));
        let mut lane = slot.map(|index| {
            Box::new(
                scope.parent.original.as_mut().unwrap().lanes[index]
                    .state
                    .take()
                    .unwrap(),
            )
        });
        let key = lane
            .as_ref()
            .map_or(scope.primary.completed.as_ref().unwrap().key, |lane| {
                lane.key
            });
        let mut prepared = None;
        let (operation, retake) = scope
            .parent
            .with_preparation_custody(|memory| {
                prepared = Some(match bindings {
                    1 => single_persistent_control_in_memory_v1(memory, key),
                    3 => three_persistent_control_in_memory_v1(memory, key),
                    _ => unreachable!(),
                });
                Ok(())
            })
            .unwrap();
        retake.unwrap();
        operation.unwrap();
        let mut prepared = prepared.unwrap();
        let (generation, data) = recycle_and_detach_persistent_fixture_v1(&mut prepared);
        let mut primary = Ledger::default();
        let displaced = if let Some(lane) = lane.as_mut() {
            lane.detached_dispatch_generation = Some(generation);
            lane.detached_data_count = 0;
            lane.detached_data_identities = Vec::with_capacity(5);
            lane.detached_next_insertion_index = Some(0);
            lane.dispatch.replace(prepared).unwrap()
        } else {
            primary = Ledger {
                generation: Some(generation),
                count: 0,
                identities: Vec::with_capacity(3),
                next: Some(0),
            };
            scope
                .parent
                .original
                .as_mut()
                .unwrap()
                .primary
                .completed
                .as_mut()
                .unwrap()
                .dispatch
                .replace(prepared)
                .unwrap()
        };
        Self {
            scope,
            slot,
            lane,
            primary,
            trace: ControlTrace::default(),
            displaced,
            data,
            generation,
            key,
        }
    }

    fn context(&mut self) -> ControlContext<'_> {
        ControlContext {
            parent: &mut self.scope.parent,
            lane: self.lane.as_deref_mut(),
            primary: &mut self.primary,
            trace: &mut self.trace,
        }
    }

    fn memory(&self) -> &Memory {
        &self
            .scope
            .primary
            .completed
            .as_ref()
            .unwrap()
            .engine
            .backend
            .session
    }

    fn memory_mut(&mut self) -> &mut Memory {
        &mut self
            .scope
            .parent
            .original
            .as_mut()
            .unwrap()
            .primary
            .completed
            .as_mut()
            .unwrap()
            .engine
            .backend
            .session
    }

    fn snapshot(&self) -> MemorySnapshot {
        let engine = &self.scope.primary.completed.as_ref().unwrap().engine;
        self.memory()
            .control_release_snapshot_v1(&engine.foundation)
    }

    fn owner_snapshot(&mut self) -> OwnerSnapshot {
        let generation = self.generation;
        OwnerSnapshot::owner_v1(self.context().dispatch().as_ref().unwrap(), generation)
    }

    fn loan_state(&self) -> (u64, Option<u64>, u64) {
        let engine = &self.scope.primary.completed.as_ref().unwrap().engine;
        self.memory().primary_loan_state_v1(&engine.foundation)
    }

    fn ledger_snapshot(&self) -> impl std::fmt::Debug + PartialEq + use<> {
        let (generation, count, ids, next) = if let Some(lane) = &self.lane {
            (
                lane.detached_dispatch_generation,
                lane.detached_data_count,
                &lane.detached_data_identities,
                lane.detached_next_insertion_index,
            )
        } else {
            (
                self.primary.generation,
                self.primary.count,
                &self.primary.identities,
                self.primary.next,
            )
        };
        (
            generation,
            count,
            ids.clone(),
            ids.as_ptr(),
            ids.capacity(),
            next,
        )
    }

    fn outside_snapshot(&self) -> impl std::fmt::Debug + PartialEq + use<> {
        (
            OwnerSnapshot::owner_v1(&self.displaced, 0),
            self.data.as_ptr(),
            self.data.capacity(),
            self.data
                .iter()
                .map(|d| {
                    (
                        d.storage_identity(),
                        d.layout(),
                        d.is_fully_initialized(),
                        d.initialized_content(),
                    )
                })
                .collect::<Vec<_>>(),
        )
    }

    fn release(&mut self) -> SettledRetainedControlReleaseV1 {
        let ledger = self.ledger_snapshot();
        let outside = self.outside_snapshot();
        let entered = self.trace.entered;
        let retake = next_occurrence("auxiliary-retake");
        let result = settle_retained_control_release_v1(&mut self.context());
        assert_eq!(
            next_occurrence("auxiliary-retake") - retake,
            self.trace.entered - entered,
            "exactly one retake for an entered callback, none for a rejected opening"
        );
        assert_eq!(
            self.ledger_snapshot(),
            ledger,
            "control cleanup cannot change the detached ledger"
        );
        assert_eq!(
            self.outside_snapshot(),
            outside,
            "separate data and displaced owners stay rooted"
        );
        if self.trace.entered > entered {
            self.trace
                .before_retake
                .as_ref()
                .unwrap()
                .assert_after_retake_v1(
                    self.snapshot(),
                    self.trace.before_cleanup.as_ref().unwrap(),
                    self.scope.parent.faults.regress_revision,
                );
        }
        result
    }

    fn assert_prefix(&self, owner: &OwnerSnapshot, prefix: ControlReleasePrefixV1) {
        self.trace
            .before_cleanup
            .as_ref()
            .unwrap()
            .assert_control_transition_snapshot_v1(
                self.trace.before_retake.as_ref().unwrap(),
                (self.key.vm, self.memory().primary_session_id()),
                &owner.order_v1(),
                prefix,
            );
    }

    fn assert_retry(&mut self) {
        self.memory_mut().clear_control_release_faults_v1();
        self.scope.parent.faults = Faults::default();
        self.trace.panic_poison = false;
        self.trace.secondary_envelope = None;
        self.trace.secondary_poison = None;
        self.trace.envelope_after = Outcome::Success;
        let before = self.snapshot();
        let root = self.trace.retained.as_ref().map(OwnerSnapshot::root_v1);
        let counts = (
            self.trace.loans,
            self.trace.entered,
            self.trace.poisons,
            self.memory().control_release_calls_v1(),
        );
        let rejected = self.release();
        assert!(!rejected.transport);
        assert!(matches!(
            rejected.result,
            Ok(Err(ComputeAqlQueueSessionErrorV1::DispatchBinding(
                Gfx942DispatchBindingErrorV1::Poisoned
            )))
        ));
        assert_eq!(self.snapshot(), before);
        assert_eq!(
            self.trace.retained.as_ref().map(OwnerSnapshot::root_v1),
            root
        );
        assert_eq!(
            (
                self.trace.loans,
                self.trace.entered,
                self.trace.poisons,
                self.memory().control_release_calls_v1()
            ),
            counts
        );
    }

    fn restore(&mut self) {
        if let Some(index) = self.slot {
            let slot = &mut self.scope.parent.original.as_mut().unwrap().lanes[index];
            assert!(slot.state.is_none());
            slot.state = Some(*self.lane.take().unwrap());
        }
    }
}

#[test]
fn retained_control_prepared_primary_and_auxiliary_success_leave_data_and_ledger_unchanged() {
    for ordinal in 0..3 {
        for bindings in [1, 3] {
            let mut f = ControlFixture::new(ordinal, bindings);
            let owner = f.owner_snapshot();
            let count = owner.order_v1().len();
            let loan = f.loan_state();
            let released = f.release();
            assert!(!released.transport);
            assert!(matches!(released.result, Ok(Ok(true))));
            assert!(f.context().dispatch().is_none());
            assert!(f.trace.retained.is_none());
            assert_eq!((f.trace.loans, f.trace.entered, f.trace.poisons), (1, 1, 0));
            assert_eq!(f.memory().control_release_calls_v1(), count);
            assert_eq!(f.loan_state(), (loan.0, None, loan.2 + 1));
            f.assert_prefix(&owner, (count, false, 0, None));
            let before = f.snapshot();
            let released = f.release();
            assert!(!released.transport);
            assert!(matches!(released.result, Ok(Ok(false))));
            assert_eq!(f.snapshot(), before);
            f.restore();
        }
    }
}

#[test]
fn retained_control_exhausted_real_opening_restores_exact_owner_without_poison_or_retake() {
    for ordinal in 0..3 {
        for bindings in [1, 3] {
            let mut f = ControlFixture::new(ordinal, bindings);
            f.memory_mut().primary_expire_loan_generation_v1();
            let owner = f.owner_snapshot();
            let before = f.snapshot();
            let loan = f.loan_state();
            let released = f.release();
            assert!(!released.transport);
            assert!(matches!(
                released.result,
                Ok(Err(ComputeAqlQueueSessionErrorV1::Memory(
                    MemorySessionError::Model("fixture live foundation loan")
                )))
            ));
            assert!(
                f.context().dispatch().is_some(),
                "opening rejection restores dispatch custody"
            );
            assert_eq!(f.owner_snapshot(), owner);
            before.assert_currentness_only_v1(f.snapshot(), 1);
            assert_eq!(f.loan_state(), loan);
            assert_eq!((f.trace.loans, f.trace.entered, f.trace.poisons), (1, 0, 0));
            assert_eq!(f.memory().control_release_calls_v1(), 0);
            assert!(f.trace.retained.is_none() && !f.scope.parent.poisoned);
            f.restore();
        }
    }
}

fn arm_retake(f: &mut ControlFixture, closing: usize) {
    match closing {
        0 => f.scope.parent.faults.reclaim_before = Outcome::Error,
        1 => f.scope.parent.faults.reclaim_before = Outcome::Panic,
        2 => f.scope.parent.faults.reclaim_after = Outcome::Error,
        3 => f.scope.parent.faults.reclaim_after = Outcome::Panic,
        4 => f.scope.parent.faults.regress_revision = true,
        _ => unreachable!(),
    }
}

fn next_occurrence(name: &str) -> usize {
    trace()
        .borrow()
        .calls
        .iter()
        .filter(|&&n| n == name)
        .count()
        + 1
}

fn assert_retake(
    result: &SettledRetainedControlReleaseV1,
    closing: usize,
    before: usize,
    after: usize,
) {
    match (closing, &result.result) {
        (0, Ok(Err(ComputeAqlQueueSessionErrorV1::Contract("auxiliary-retake"))))
        | (2, Ok(Err(ComputeAqlQueueSessionErrorV1::Contract("auxiliary-retake-complete"))))
        | (
            4,
            Ok(Err(ComputeAqlQueueSessionErrorV1::Memory(MemorySessionError::Model(
                "fixture live foundation reclaim",
            )))),
        ) => {}
        (1, Err(payload)) => assert_eq!(
            payload.downcast_ref::<(&str, usize)>(),
            Some(&("auxiliary-retake", before))
        ),
        (3, Err(payload)) => assert_eq!(
            payload.downcast_ref::<(&str, usize)>(),
            Some(&("auxiliary-retake-complete", after))
        ),
        _ => panic!("unexpected retained-control retake provenance"),
    }
}

#[test]
fn retained_control_completed_cleanup_remains_rooted_across_every_failed_retake() {
    for ordinal in 0..3 {
        for bindings in [1, 3] {
            for closing in 0..5 {
                let mut f = ControlFixture::new(ordinal, bindings);
                let owner = f.owner_snapshot();
                let count = owner.order_v1().len();
                let loan = f.loan_state();
                let pre = next_occurrence("auxiliary-retake");
                let post = next_occurrence("auxiliary-retake-complete");
                arm_retake(&mut f, closing);
                let released = f.release();
                assert!(released.transport);
                assert_retake(&released, closing, pre, post);
                owner.assert_detached_prefix_v1(
                    f.trace.retained.as_ref().unwrap(),
                    count,
                    false,
                    true,
                );
                assert!(f.context().dispatch().is_none());
                f.assert_prefix(&owner, (count, false, 0, None));
                assert_eq!(
                    f.loan_state(),
                    (
                        loan.0,
                        (!matches!(closing, 2 | 3)).then_some(loan.2),
                        loan.2 + 1
                    )
                );
                assert_eq!((f.trace.loans, f.trace.entered, f.trace.poisons), (1, 1, 1));
                f.assert_retry();
                f.restore();
            }
        }
    }
}

#[test]
fn retained_control_native_failures_keep_exact_completed_prefix_and_unfinished_suffix() {
    for ordinal in 0..3 {
        for bindings in [1, 3] {
            let control_count = if bindings == 1 { 4 } else { 2 };
            for position in 0..control_count {
                for (op, operation) in ["unmap_gpu", "unmap_cpu", "free", "release_va_reservation"]
                    .into_iter()
                    .enumerate()
                {
                    for panicked in [false, true] {
                        let mut f = ControlFixture::new(ordinal, bindings);
                        let owner = f.owner_snapshot();
                        assert_eq!(owner.order_v1().len(), control_count);
                        f.memory_mut()
                            .arm_control_release_native_v1(position, operation, panicked);
                        let released = f.release();
                        assert!(released.transport);
                        if panicked {
                            assert_eq!(
                                released.result.unwrap_err().downcast_ref::<(&str, &str)>(),
                                Some(&("N2 native panic", operation))
                            );
                        } else {
                            assert!(
                                matches!(released.result, Ok(Err(ComputeAqlQueueSessionErrorV1::DispatchBinding(
                                Gfx942DispatchBindingErrorV1::Memory(MemorySessionError::Injected(name))))) if name == operation)
                            );
                        }
                        let root = f.trace.retained.as_ref().unwrap();
                        owner.assert_detached_prefix_v1(root, position, true, false);
                        let snapshot = OwnerSnapshot::root_v1(root);
                        let active = snapshot.active_v1().unwrap();
                        assert!(
                            active.started
                                && active.failed
                                && active.stage != CleanupStageV1::Complete
                                && !active.native_disposed
                        );
                        f.assert_prefix(
                            &owner,
                            (position, op > 0, op + 1, Some((op > 0, op, false, op == 2))),
                        );
                        assert_eq!(f.memory().control_release_calls_v1(), position + 1);
                        assert_eq!(f.trace.panicked, panicked);
                        f.assert_retry();
                        f.restore();
                    }
                }
            }
        }
    }
}

#[test]
fn retained_control_retake_error_precedes_lower_error_but_never_lower_panic() {
    for ordinal in 0..3 {
        for closing in 0..5 {
            for panicked in [false, true] {
                let mut f = ControlFixture::new(ordinal, 1);
                let owner = f.owner_snapshot();
                f.memory_mut()
                    .arm_control_release_native_v1(1, "free", panicked);
                let pre = next_occurrence("auxiliary-retake");
                let post = next_occurrence("auxiliary-retake-complete");
                arm_retake(&mut f, closing);
                let released = f.release();
                assert!(released.transport);
                if panicked {
                    assert_eq!(
                        released.result.unwrap_err().downcast_ref::<(&str, &str)>(),
                        Some(&("N2 native panic", "free"))
                    );
                } else {
                    assert_retake(&released, closing, pre, post);
                }
                owner.assert_detached_prefix_v1(f.trace.retained.as_ref().unwrap(), 1, true, false);
                f.assert_prefix(&owner, (1, true, 3, Some((true, 2, false, true))));
                f.assert_retry();
                f.restore();
            }
        }
    }
}

#[test]
fn retained_control_skipped_callback_restores_owner_and_incomplete_lower_success_is_terminal() {
    for ordinal in 0..3 {
        for skip_callback in [false, true] {
            let mut f = ControlFixture::new(ordinal, 1);
            let owner = f.owner_snapshot();
            if skip_callback {
                f.trace.skip_callback = true;
            } else {
                f.memory_mut().skip_control_release_v1(0);
            }
            let released = f.release();
            assert!(released.transport);
            if skip_callback {
                assert!(matches!(
                    released.result,
                    Ok(Err(ComputeAqlQueueSessionErrorV1::Contract(
                        "retained persistent control callback did not execute"
                    )))
                ));
                assert!(
                    f.context().dispatch().is_some(),
                    "skipped callback restores dispatch custody"
                );
                owner.assert_restored_v1(f.owner_snapshot(), ordinal == 0);
                assert!(f.trace.retained.is_none());
                assert_eq!(f.trace.entered, 0);
            } else {
                assert!(matches!(
                    released.result,
                    Ok(Err(ComputeAqlQueueSessionErrorV1::DispatchBinding(
                        Gfx942DispatchBindingErrorV1::ResourcePhase
                    )))
                ));
                owner.assert_detached_prefix_v1(f.trace.retained.as_ref().unwrap(), 0, true, false);
                f.assert_prefix(&owner, (0, false, 0, None));
            }
            f.assert_retry();
            f.restore();
        }
    }
}

#[test]
fn retained_control_opening_panic_preserves_payload_and_restores_original_controls() {
    for ordinal in 0..3 {
        let mut f = ControlFixture::new(ordinal, 1);
        let owner = f.owner_snapshot();
        let loan = f.loan_state();
        let occurrence = next_occurrence("auxiliary-loan");
        f.scope.parent.faults.loan = Outcome::Panic;
        let released = f.release();
        assert!(released.transport);
        assert_eq!(
            released.result.unwrap_err().downcast_ref::<(&str, usize)>(),
            Some(&("auxiliary-loan", occurrence))
        );
        assert!(
            f.context().dispatch().is_some(),
            "opening panic restores dispatch custody"
        );
        owner.assert_restored_v1(f.owner_snapshot(), ordinal == 0);
        assert_eq!(f.loan_state(), loan);
        assert!(f.trace.retained.is_none());
        assert_eq!(
            (f.trace.entered, f.memory().control_release_calls_v1()),
            (0, 0)
        );
        f.assert_retry();
        f.restore();
    }
}

#[test]
fn retained_control_final_poison_panic_stays_settled_and_preserves_existing_lower_panic() {
    for ordinal in 0..3 {
        for panicked in [false, true] {
            let mut f = ControlFixture::new(ordinal, 1);
            let owner = f.owner_snapshot();
            f.memory_mut()
                .arm_control_release_native_v1(1, "free", panicked);
            f.trace.panic_poison = true;
            let released = f.release();
            assert!(released.transport);
            let payload = released.result.unwrap_err();
            if panicked {
                assert_eq!(
                    payload.downcast_ref::<(&str, &str)>(),
                    Some(&("N2 native panic", "free"))
                );
            } else {
                assert_eq!(
                    payload.downcast_ref::<&str>(),
                    Some(&"retained control poison")
                );
            }
            owner.assert_detached_prefix_v1(f.trace.retained.as_ref().unwrap(), 1, true, false);
            f.assert_retry();
            f.restore();
        }
    }
}

#[test]
fn retained_control_malformed_post_callback_envelope_never_restores_disposed_owner() {
    for ordinal in 0..3 {
        for envelope in [Outcome::Error, Outcome::Panic] {
            let mut f = ControlFixture::new(ordinal, 3);
            let owner = f.owner_snapshot();
            f.trace.envelope_after = envelope;
            let occurrence = next_occurrence("retained-control-envelope");
            let released = f.release();
            assert!(released.transport);
            assert!(f.context().dispatch().is_none());
            owner.assert_detached_prefix_v1(
                f.trace.retained.as_ref().unwrap(),
                owner.order_v1().len(),
                false,
                true,
            );
            if envelope == Outcome::Panic {
                assert_eq!(
                    released.result.unwrap_err().downcast_ref::<(&str, usize)>(),
                    Some(&("retained-control-envelope", occurrence))
                );
            } else {
                assert!(matches!(
                    released.result,
                    Ok(Err(ComputeAqlQueueSessionErrorV1::Contract(
                        "retained-control-envelope"
                    )))
                ));
            }
            f.assert_retry();
            f.restore();
        }
    }
}

#[test]
fn retained_control_release_commit_failure_keeps_native_disposal_receipt() {
    for ordinal in 0..3 {
        for panicked in [false, true] {
            let mut f = ControlFixture::new(ordinal, 1);
            let owner = f.owner_snapshot();
            f.memory_mut().arm_control_release_projection_v1(
                1,
                CleanupStageV1::ReleaseCommit,
                panicked,
            );
            let released = f.release();
            assert!(released.transport);
            if panicked {
                assert_eq!(
                    released
                        .result
                        .unwrap_err()
                        .downcast_ref::<(&str, CleanupStageV1)>(),
                    Some(&("control cleanup projection", CleanupStageV1::ReleaseCommit))
                );
            } else {
                assert!(matches!(
                    released.result,
                    Ok(Err(ComputeAqlQueueSessionErrorV1::DispatchBinding(
                        Gfx942DispatchBindingErrorV1::Memory(MemorySessionError::Injected(
                            "control cleanup projection"
                        ))
                    )))
                ));
            }
            let root = f.trace.retained.as_ref().unwrap();
            owner.assert_detached_prefix_v1(root, 1, true, false);
            let snapshot = OwnerSnapshot::root_v1(root);
            let active = snapshot.active_v1().unwrap();
            assert!(
                active.native_disposed && active.failed && active.stage != CleanupStageV1::Complete
            );
            assert_eq!(active.owner, "NativeDisposed");
            f.assert_prefix(&owner, (1, true, 4, Some((true, 4, true, true))));
            f.assert_retry();
            f.restore();
        }
    }
}

#[test]
fn retained_control_generation_rejection_precedes_currentness_and_loan() {
    for ordinal in 0..3 {
        for invalid in [None, Some(0), Some(9)] {
            let mut f = ControlFixture::new(ordinal, 1);
            let owner = f.owner_snapshot();
            if let Some(lane) = f.lane.as_mut() {
                lane.detached_dispatch_generation = invalid;
            } else {
                f.primary.generation = invalid;
            }
            let before = f.snapshot();
            let released = f.release();
            assert!(released.transport);
            if invalid.is_none() {
                assert!(matches!(
                    released.result,
                    Ok(Err(ComputeAqlQueueSessionErrorV1::Contract(
                        "retained persistent control lost its detached generation"
                    )))
                ));
            } else {
                assert!(matches!(
                    released.result,
                    Ok(Err(ComputeAqlQueueSessionErrorV1::DispatchBinding(
                        Gfx942DispatchBindingErrorV1::ResourcePhase
                    )))
                ));
            }
            assert!(
                f.context().dispatch().is_some(),
                "preflight preserves original control custody"
            );
            owner.assert_restored_v1(f.owner_snapshot(), ordinal == 0);
            assert_eq!(f.snapshot(), before);
            assert_eq!(
                (
                    f.trace.loans,
                    f.trace.entered,
                    f.memory().control_release_calls_v1()
                ),
                (0, 0, 0)
            );
            assert!(f.trace.retained.is_none());
            f.assert_retry();
            f.restore();
        }
    }
}

#[test]
fn retained_control_currentness_failure_preserves_unconsumed_owner() {
    for ordinal in 0..3 {
        for panicked in [false, true] {
            let mut f = ControlFixture::new(ordinal, 1);
            let owner = f.owner_snapshot();
            let before = f.snapshot();
            let loan = f.loan_state();
            f.memory_mut().primary_arm_native("currentness", panicked);
            let released = f.release();
            assert!(released.transport);
            if panicked {
                assert_eq!(
                    released.result.unwrap_err().downcast_ref::<(&str, &str)>(),
                    Some(&("N2 native panic", "currentness"))
                );
            } else {
                assert!(matches!(
                    released.result,
                    Ok(Err(ComputeAqlQueueSessionErrorV1::Memory(
                        MemorySessionError::Injected("currentness")
                    )))
                ));
            }
            assert!(
                f.context().dispatch().is_some(),
                "currentness rejection preserves original control custody"
            );
            owner.assert_restored_v1(f.owner_snapshot(), ordinal == 0);
            before.assert_currentness_failure_v1(f.snapshot(), panicked);
            assert_eq!(f.loan_state(), loan);
            assert_eq!(
                (
                    f.trace.loans,
                    f.trace.entered,
                    f.memory().control_release_calls_v1()
                ),
                (0, 0, 0)
            );
            assert!(f.trace.retained.is_none());
            assert_eq!(f.trace.panicked, panicked);
            f.assert_retry();
            f.restore();
        }
    }
}

#[test]
fn retained_control_lower_panic_survives_model_driver_poison_panic() {
    for ordinal in 0..3 {
        let mut f = ControlFixture::new(ordinal, 1);
        let owner = f.owner_snapshot();
        f.memory_mut()
            .arm_control_release_native_v1(1, "free", true);
        f.scope.parent.faults.reclaim_before = Outcome::Error;
        f.scope.parent.faults.poison_panic = true;
        let poison_occurrence = next_occurrence("auxiliary-poison-panic");
        let released = f.release();
        assert!(released.transport);
        assert_eq!(
            released.result.unwrap_err().downcast_ref::<(&str, &str)>(),
            Some(&("N2 native panic", "free"))
        );
        assert!(f.context().dispatch().is_none());
        owner.assert_detached_prefix_v1(f.trace.retained.as_ref().unwrap(), 1, true, false);
        f.assert_prefix(&owner, (1, true, 3, Some((true, 2, false, true))));
        assert_eq!((f.trace.poisons, f.trace.panicked), (1, true));
        assert_eq!(
            next_occurrence("auxiliary-poison-panic"),
            poison_occurrence + 1
        );
        f.assert_retry();
        f.restore();
    }
}

#[test]
fn retained_control_secondary_panic_destructors_cannot_replace_lower_payload() {
    for poison in [false, true] {
        let mut f = ControlFixture::new(0, 1);
        let owner = f.owner_snapshot();
        let drops = Arc::new(AtomicUsize::new(0));
        if poison {
            f.trace.secondary_poison = Some(drops.clone());
        } else {
            f.trace.secondary_envelope = Some(drops.clone());
        }
        f.memory_mut()
            .arm_control_release_native_v1(1, "free", true);
        let released = f.release();
        assert!(released.transport);
        let payload = released.result.unwrap_err();
        if payload.is::<SecondaryPanic>() {
            core::mem::forget(payload);
            panic!("secondary panic replaced original cleanup payload");
        }
        assert_eq!(
            payload.downcast_ref::<(&str, &str)>(),
            Some(&("N2 native panic", "free"))
        );
        assert_eq!(drops.load(Ordering::SeqCst), 0);
        assert_eq!(
            Arc::strong_count(&drops),
            3,
            "secondary payload was constructed and retained"
        );
        owner.assert_detached_prefix_v1(f.trace.retained.as_ref().unwrap(), 1, true, false);
        f.assert_retry();
        assert_eq!(drops.load(Ordering::SeqCst), 0);
        assert_eq!(Arc::strong_count(&drops), 2);
        f.restore();
    }
}
