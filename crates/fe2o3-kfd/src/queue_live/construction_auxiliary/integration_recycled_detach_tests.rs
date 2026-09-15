//! Ordinary prepared owners and model loans; native operations remain CPU fixtures.

use super::*;
use crate::queue::dispatch_binding::control_release::RetainedControlSnapshotV1 as OwnerSnapshot;
use crate::queue::dispatch_binding::preparation::{
    RecycledDataExpectationV1, ordinary_recycled_in_memory_v1,
};
use crate::queue::live::recycled_detach::{
    RecycledDetachContextV1, RecycledDetachCustodyV1, RecycledDetachLedgerV1,
    SettledRecycledDetachV1, settle_recycled_detach_v1,
};
use crate::shared_memory::{
    CleanupStageV1, ControlReleaseMemorySnapshotV1 as MemorySnapshot, ControlReleasePrefixV1,
};

#[derive(Default)]
struct Ledger {
    generation: Option<u64>,
    count: usize,
    identities: Vec<Gfx942FixedDispatchStorageIdentityV1>,
    next: Option<usize>,
}

#[derive(Default)]
struct DetachTrace {
    loans: usize,
    entered: usize,
    ledger_calls: usize,
    before_cleanup: Option<MemorySnapshot>,
    before_retake: Option<MemorySnapshot>,
    retained: Option<RecycledDetachCustodyV1>,
    poisons: usize,
    panicked: bool,
    capacities: Option<(usize, usize)>,
    skip_callback: bool,
    panic_commit: bool,
    panic_poison: bool,
    envelope_after: Outcome,
}

struct Context<'a> {
    parent: &'a mut Parent,
    lane: Option<&'a mut ComputeAqlQueueLaneStateV1<Fixture>>,
    primary: &'a mut Ledger,
    trace: &'a mut DetachTrace,
}

impl RecycledDetachContextV1 for Context<'_> {
    type Memory = Memory;

    fn require_detachable(&self) -> Result<(), ComputeAqlQueueSessionErrorV1> {
        if self.parent.poisoned {
            return Err(Gfx942DispatchBindingErrorV1::Poisoned.into());
        }
        Ok(())
    }

    fn require_completed(&self) -> Result<(), ComputeAqlQueueSessionErrorV1> {
        let owner = self.lane.as_ref().map_or(
            &self
                .parent
                .original
                .as_ref()
                .unwrap()
                .primary
                .completed
                .as_ref()
                .unwrap()
                .completion_owner,
            |lane| &lane.completion_owner,
        );
        owner.ensure_releasable().map_err(Into::into)
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

    fn ledger(&mut self) -> RecycledDetachLedgerV1<'_> {
        self.trace.ledger_calls += 1;
        if self.trace.panic_commit && self.trace.ledger_calls == 2 {
            panic!("recycled detach ledger access");
        }
        if let Some(lane) = self.lane.as_mut() {
            RecycledDetachLedgerV1 {
                generation: &mut lane.detached_dispatch_generation,
                count: &mut lane.detached_data_count,
                identities: &mut lane.detached_data_identities,
                next: &mut lane.detached_next_insertion_index,
            }
        } else {
            RecycledDetachLedgerV1 {
                generation: &mut self.primary.generation,
                count: &mut self.primary.count,
                identities: &mut self.primary.identities,
                next: &mut self.primary.next,
            }
        }
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
        outcome("recycled-detach-envelope", self.trace.envelope_after)?;
        Ok(((), retake.and(callback)))
    }

    fn retain(&mut self, root: RecycledDetachCustodyV1) {
        assert!(self.trace.retained.is_none());
        self.trace.retained = Some(root);
    }

    fn poison(&mut self, panicked: bool) {
        assert!(
            self.trace.retained.is_some(),
            "root custody precedes poison"
        );
        self.trace.poisons += 1;
        self.trace.panicked |= panicked;
        self.parent.poison();
        if let Some(lane) = self.lane.as_mut() {
            lane.completion_owner.poison_owner();
            lane.submission.as_mut().unwrap().poison();
        }
        if self.trace.panic_poison {
            panic!("recycled detach poison");
        }
    }

    fn output_capacities(&self, count: usize) -> (usize, usize) {
        self.trace.capacities.unwrap_or((count, count))
    }
}

struct DetachFixture {
    scope: Box<Scope>,
    slot: Option<usize>,
    lane: Option<Box<ComputeAqlQueueLaneStateV1<Fixture>>>,
    primary: Ledger,
    trace: DetachTrace,
    displaced: DispatchResourceOwnerV1,
    expected: Vec<RecycledDataExpectationV1>,
    generation: u64,
    key: QueueKeyV1,
}

fn data_snapshot(data: &[Gfx942FixedDispatchDataV1]) -> Vec<RecycledDataExpectationV1> {
    data.iter()
        .map(|data| {
            (
                data.sdma_storage_identity(),
                data.layout(),
                data.is_fully_initialized(),
            )
        })
        .collect()
}

impl DetachFixture {
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
                prepared = Some(ordinary_recycled_in_memory_v1(memory, key, bindings));
                Ok(())
            })
            .unwrap();
        retake.unwrap();
        operation.unwrap();
        let (prepared, generation, expected) = prepared.unwrap();
        let displaced = if let Some(lane) = lane.as_mut() {
            lane.detached_dispatch_generation = None;
            lane.detached_data_count = 0;
            lane.detached_data_identities = Vec::with_capacity(5);
            lane.detached_next_insertion_index = None;
            lane.dispatch.replace(prepared).unwrap()
        } else {
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
            primary: Ledger {
                identities: Vec::with_capacity(3),
                ..Ledger::default()
            },
            trace: DetachTrace::default(),
            displaced,
            expected,
            generation,
            key,
        }
    }

    fn context(&mut self) -> Context<'_> {
        Context {
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
        self.memory().control_release_snapshot_v1(
            &self
                .scope
                .primary
                .completed
                .as_ref()
                .unwrap()
                .engine
                .foundation,
        )
    }

    fn owner_snapshot(&mut self) -> OwnerSnapshot {
        OwnerSnapshot::recycled_owner_v1(self.context().dispatch().as_ref().unwrap())
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

    fn detach(&mut self) -> SettledRecycledDetachV1 {
        let ledger = self.ledger_snapshot();
        let displaced = OwnerSnapshot::owner_v1(&self.displaced, 0);
        let entered = self.trace.entered;
        let retake = next_occurrence("auxiliary-retake");
        let result = settle_recycled_detach_v1(&mut self.context());
        assert_eq!(
            next_occurrence("auxiliary-retake") - retake,
            self.trace.entered - entered
        );
        if !matches!(result.result, Ok(Ok(_))) {
            assert_eq!(
                self.ledger_snapshot(),
                ledger,
                "failed settlement cannot alter any ledger field or backing"
            );
        }
        assert_eq!(OwnerSnapshot::owner_v1(&self.displaced, 0), displaced);
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
        self.trace.envelope_after = Outcome::Success;
        let before = self.snapshot();
        let retained = self.trace.retained.as_ref().unwrap();
        let root = retained.cleanup.as_ref().map(OwnerSnapshot::root_v1);
        let data = data_snapshot(&retained.data);
        let counts = (self.trace.loans, self.trace.entered, self.trace.poisons);
        let retry = self.detach();
        assert!(!retry.transport);
        assert!(matches!(
            retry.result,
            Ok(Err(ComputeAqlQueueSessionErrorV1::DispatchBinding(
                Gfx942DispatchBindingErrorV1::Poisoned
            )))
        ));
        assert_eq!(self.snapshot(), before);
        let retained = self.trace.retained.as_ref().unwrap();
        assert_eq!(retained.cleanup.as_ref().map(OwnerSnapshot::root_v1), root);
        assert_eq!(data_snapshot(&retained.data), data);
        assert_eq!(
            (self.trace.loans, self.trace.entered, self.trace.poisons),
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

fn next_occurrence(name: &str) -> usize {
    trace()
        .borrow()
        .calls
        .iter()
        .filter(|&&n| n == name)
        .count()
        + 1
}

#[test]
fn recycled_detach_returns_exact_data_and_commits_ledger_after_successful_retake() {
    for ordinal in 0..3 {
        for bindings in [1, 3] {
            let mut f = DetachFixture::new(ordinal, bindings);
            let owner = f.owner_snapshot();
            let count = owner.order_v1().len();
            let result = f.detach();
            assert!(!result.transport);
            let returned = result.result.unwrap().unwrap();
            assert_eq!(returned.dispatch_generation(), f.generation);
            let data = returned.into_data();
            assert_eq!(data_snapshot(&data), f.expected);
            assert!(data.iter().all(|data| data.initialized_content().is_none()));
            let generation = f.generation;
            let mut context = f.context();
            let ledger = context.ledger();
            assert_eq!(*ledger.generation, Some(generation));
            assert_eq!(*ledger.count, data.len());
            assert_eq!(
                *ledger.identities,
                data.iter()
                    .map(Gfx942FixedDispatchDataV1::storage_identity)
                    .collect::<Vec<_>>()
            );
            assert_eq!(*ledger.next, None);
            assert!(f.context().dispatch().is_none());
            assert!(f.trace.retained.is_none());
            assert_eq!((f.trace.loans, f.trace.entered, f.trace.poisons), (1, 1, 0));
            f.assert_prefix(&owner, (count, false, 0, None));
            f.restore();
        }
    }
}

#[test]
fn recycled_detach_opening_rejection_restores_exact_owner_without_retake_or_poison() {
    for ordinal in 0..3 {
        let mut f = DetachFixture::new(ordinal, 1);
        f.memory_mut().primary_expire_loan_generation_v1();
        let owner = f.owner_snapshot();
        let before = f.snapshot();
        let result = f.detach();
        assert!(!result.transport);
        assert!(matches!(
            result.result,
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
        assert_eq!((f.trace.loans, f.trace.entered, f.trace.poisons), (1, 0, 0));
        assert!(f.trace.retained.is_none());
        f.restore();
    }
}

#[test]
fn recycled_detach_both_output_reserves_reject_before_currentness_or_ownership_transfer() {
    for ordinal in 0..3 {
        for capacities in [(usize::MAX, 5), (5, usize::MAX), (0, 5), (5, 0)] {
            let mut f = DetachFixture::new(ordinal, 1);
            f.trace.capacities = Some(capacities);
            let owner = f.owner_snapshot();
            let before = f.snapshot();
            let result = f.detach();
            assert!(!result.transport);
            assert!(matches!(
                result.result,
                Ok(Err(ComputeAqlQueueSessionErrorV1::DispatchBinding(
                    Gfx942DispatchBindingErrorV1::HostAllocationCapacity {
                        operation: "detached dispatch output"
                    }
                )))
            ));
            assert_eq!(f.owner_snapshot(), owner);
            assert_eq!(f.snapshot(), before);
            assert_eq!((f.trace.loans, f.trace.entered, f.trace.poisons), (0, 0, 0));
            assert!(f.trace.retained.is_none());
            f.restore();
        }
    }
}

fn arm_retake(f: &mut DetachFixture, closing: usize) {
    match closing {
        0 => f.scope.parent.faults.reclaim_before = Outcome::Error,
        1 => f.scope.parent.faults.reclaim_before = Outcome::Panic,
        2 => f.scope.parent.faults.reclaim_after = Outcome::Error,
        3 => f.scope.parent.faults.reclaim_after = Outcome::Panic,
        4 => f.scope.parent.faults.regress_revision = true,
        _ => unreachable!(),
    }
}

fn assert_retake(result: &SettledRecycledDetachV1, closing: usize, pre: usize, post: usize) {
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
            Some(&("auxiliary-retake", pre))
        ),
        (3, Err(payload)) => assert_eq!(
            payload.downcast_ref::<(&str, usize)>(),
            Some(&("auxiliary-retake-complete", post))
        ),
        _ => panic!("unexpected recycled-detach retake provenance"),
    }
}

#[test]
fn recycled_detach_completed_returned_authorities_survive_every_failed_retake() {
    for ordinal in 0..3 {
        for bindings in [1, 3] {
            for closing in 0..5 {
                let mut f = DetachFixture::new(ordinal, bindings);
                let owner = f.owner_snapshot();
                let count = owner.order_v1().len();
                let pre = next_occurrence("auxiliary-retake");
                let post = next_occurrence("auxiliary-retake-complete");
                arm_retake(&mut f, closing);
                let result = f.detach();
                assert!(result.transport);
                assert_retake(&result, closing, pre, post);
                let root = f.trace.retained.as_ref().unwrap();
                owner.assert_recycled_prefix_v1(root.cleanup.as_ref().unwrap(), count, false, true);
                assert!(root.data.is_empty());
                assert!(f.context().dispatch().is_none());
                f.assert_prefix(&owner, (count, false, 0, None));
                f.assert_retry();
                f.restore();
            }
        }
    }
}

#[test]
fn recycled_detach_native_faults_preserve_data_and_exact_control_prefixes() {
    for ordinal in 0..3 {
        for bindings in [1, 3] {
            let controls = if bindings == 1 { 4 } else { 2 };
            for position in 0..controls {
                for (op, operation) in ["unmap_gpu", "unmap_cpu", "free", "release_va_reservation"]
                    .into_iter()
                    .enumerate()
                {
                    for panicked in [false, true] {
                        let mut f = DetachFixture::new(ordinal, bindings);
                        let owner = f.owner_snapshot();
                        assert_eq!(owner.order_v1().len(), controls);
                        f.memory_mut()
                            .arm_control_release_native_v1(position, operation, panicked);
                        let result = f.detach();
                        assert!(result.transport);
                        if panicked {
                            assert_eq!(
                                result.result.err().unwrap().downcast_ref::<(&str, &str)>(),
                                Some(&("N2 native panic", operation))
                            );
                        } else {
                            assert!(
                                matches!(result.result, Ok(Err(ComputeAqlQueueSessionErrorV1::DispatchBinding(Gfx942DispatchBindingErrorV1::Memory(MemorySessionError::Injected(name))))) if name == operation)
                            );
                        }
                        owner.assert_recycled_prefix_v1(
                            f.trace.retained.as_ref().unwrap().cleanup.as_ref().unwrap(),
                            position,
                            true,
                            false,
                        );
                        f.assert_prefix(
                            &owner,
                            (position, op > 0, op + 1, Some((op > 0, op, false, op == 2))),
                        );
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
fn recycled_detach_closing_error_precedes_lower_error_but_not_original_lower_panic() {
    for ordinal in 0..3 {
        for closing in 0..5 {
            for panicked in [false, true] {
                let mut f = DetachFixture::new(ordinal, 1);
                let owner = f.owner_snapshot();
                f.memory_mut()
                    .arm_control_release_native_v1(1, "free", panicked);
                let pre = next_occurrence("auxiliary-retake");
                let post = next_occurrence("auxiliary-retake-complete");
                arm_retake(&mut f, closing);
                let result = f.detach();
                assert!(result.transport);
                if panicked {
                    assert_eq!(
                        result.result.err().unwrap().downcast_ref::<(&str, &str)>(),
                        Some(&("N2 native panic", "free"))
                    );
                } else {
                    assert_retake(&result, closing, pre, post);
                }
                owner.assert_recycled_prefix_v1(
                    f.trace.retained.as_ref().unwrap().cleanup.as_ref().unwrap(),
                    1,
                    true,
                    false,
                );
                f.assert_prefix(&owner, (1, true, 3, Some((true, 2, false, true))));
                f.assert_retry();
                f.restore();
            }
        }
    }
}

#[test]
fn recycled_detach_missing_callback_and_incomplete_lower_success_cannot_commit() {
    for ordinal in 0..3 {
        for skip_callback in [false, true] {
            let mut f = DetachFixture::new(ordinal, 1);
            let owner = f.owner_snapshot();
            if skip_callback {
                f.trace.skip_callback = true;
            } else {
                f.memory_mut().skip_control_release_v1(0);
            }
            let result = f.detach();
            assert!(result.transport);
            if skip_callback {
                assert!(matches!(
                    result.result,
                    Ok(Err(ComputeAqlQueueSessionErrorV1::Contract(
                        "recycled dispatch callback did not execute"
                    )))
                ));
                owner.assert_restored_v1(f.owner_snapshot(), ordinal == 0);
                assert!(f.trace.retained.as_ref().unwrap().cleanup.is_none());
            } else {
                assert!(matches!(
                    result.result,
                    Ok(Err(ComputeAqlQueueSessionErrorV1::DispatchBinding(
                        Gfx942DispatchBindingErrorV1::ResourcePhase
                    )))
                ));
                owner.assert_recycled_prefix_v1(
                    f.trace.retained.as_ref().unwrap().cleanup.as_ref().unwrap(),
                    0,
                    true,
                    false,
                );
                f.assert_prefix(&owner, (0, false, 0, None));
            }
            f.assert_retry();
            f.restore();
        }
    }
}

#[test]
fn recycled_detach_post_conversion_panic_keeps_all_data_without_partial_ledger_commit() {
    for ordinal in 0..3 {
        let mut f = DetachFixture::new(ordinal, 1);
        let owner = f.owner_snapshot();
        f.trace.panic_commit = true;
        let result = f.detach();
        assert!(result.transport);
        assert_eq!(
            result.result.err().unwrap().downcast_ref::<&str>(),
            Some(&"recycled detach ledger access")
        );
        let root = f.trace.retained.as_ref().unwrap();
        assert!(root.cleanup.as_ref().unwrap().is_complete());
        assert_eq!(data_snapshot(&root.data), f.expected);
        f.assert_prefix(&owner, (owner.order_v1().len(), false, 0, None));
        f.assert_retry();
        f.restore();
    }
}

#[test]
fn recycled_detach_original_panic_survives_envelope_and_final_poison_panics() {
    for ordinal in 0..3 {
        let mut f = DetachFixture::new(ordinal, 1);
        f.memory_mut()
            .arm_control_release_native_v1(1, "free", true);
        f.trace.envelope_after = Outcome::Panic;
        f.trace.panic_poison = true;
        let result = f.detach();
        assert!(result.transport);
        assert_eq!(
            result.result.err().unwrap().downcast_ref::<(&str, &str)>(),
            Some(&("N2 native panic", "free"))
        );
        f.assert_retry();
        f.restore();
    }
}

#[test]
fn recycled_detach_malformed_shape_is_rejected_before_currentness_and_loan() {
    for ordinal in 0..3 {
        for invalid in 0..3 {
            let mut f = DetachFixture::new(ordinal, 1);
            let mut context = f.context();
            let dispatch = context.dispatch().as_mut().unwrap();
            match invalid {
                0 => dispatch.set_recycled_generation_for_test(Some(0)),
                1 => dispatch.set_recycled_generation_for_test(None),
                2 => dispatch.remove_recycled_premise_for_test(),
                _ => unreachable!(),
            }
            let owner = f.owner_snapshot();
            let before = f.snapshot();
            let result = f.detach();
            assert!(result.transport);
            match invalid {
                0 => assert!(matches!(
                    result.result,
                    Ok(Err(ComputeAqlQueueSessionErrorV1::Contract(
                        "detached dispatch generation was zero"
                    )))
                )),
                1 => assert!(matches!(
                    result.result,
                    Ok(Err(ComputeAqlQueueSessionErrorV1::DispatchBinding(
                        Gfx942DispatchBindingErrorV1::ResourcePhase
                    )))
                )),
                2 => assert!(matches!(
                    result.result,
                    Ok(Err(ComputeAqlQueueSessionErrorV1::DispatchBinding(
                        Gfx942DispatchBindingErrorV1::InvalidData {
                            index: 4,
                            detail: "retained data/premise cardinality"
                        }
                    )))
                )),
                _ => unreachable!(),
            }
            assert!(
                f.context().dispatch().is_some(),
                "borrowed shape rejection preserves original dispatch custody"
            );
            owner.assert_restored_v1(f.owner_snapshot(), ordinal == 0);
            assert_eq!(f.snapshot(), before);
            assert_eq!((f.trace.loans, f.trace.entered, f.trace.poisons), (0, 0, 1));
            f.assert_retry();
            f.restore();
        }
    }
}

#[test]
fn recycled_detach_currentness_failure_preserves_unconsumed_dispatch() {
    for ordinal in 0..3 {
        for panicked in [false, true] {
            let mut f = DetachFixture::new(ordinal, 1);
            let owner = f.owner_snapshot();
            f.memory_mut().primary_arm_native("currentness", panicked);
            let before = f.snapshot();
            let result = f.detach();
            assert!(result.transport);
            if panicked {
                assert_eq!(
                    result.result.err().unwrap().downcast_ref::<(&str, &str)>(),
                    Some(&("N2 native panic", "currentness"))
                );
            } else {
                assert!(matches!(
                    result.result,
                    Ok(Err(ComputeAqlQueueSessionErrorV1::Memory(
                        MemorySessionError::Injected("currentness")
                    )))
                ));
            }
            owner.assert_restored_v1(f.owner_snapshot(), ordinal == 0);
            before.assert_currentness_failure_v1(f.snapshot(), panicked);
            assert_eq!((f.trace.loans, f.trace.entered, f.trace.poisons), (0, 0, 1));
            f.assert_retry();
            f.restore();
        }
    }
}

#[test]
fn recycled_detach_each_nonempty_ledger_field_precedes_completion_and_loan() {
    for ordinal in 0..3 {
        for field in 0..4 {
            let mut f = DetachFixture::new(ordinal, 1);
            let mut unrelated = Memory::new(true);
            let identity = {
                let data = unrelated.host(false);
                let identity = data.storage_identity();
                // Separate unrelated allocation stays rooted for this entire case.
                (identity, data)
            };
            let mut context = f.context();
            let ledger = context.ledger();
            match field {
                0 => *ledger.generation = Some(17),
                1 => *ledger.count = 17,
                2 => ledger.identities.push(identity.0),
                3 => *ledger.next = Some(0),
                _ => unreachable!(),
            }
            if let Some(lane) = f.lane.as_mut() {
                lane.completion_owner.bind_barrier_probe().unwrap();
            } else {
                f.scope
                    .parent
                    .original
                    .as_mut()
                    .unwrap()
                    .primary
                    .completed
                    .as_mut()
                    .unwrap()
                    .completion_owner
                    .bind_barrier_probe()
                    .unwrap();
            }
            let owner = f.owner_snapshot();
            let before = f.snapshot();
            let result = f.detach();
            assert!(result.transport);
            assert!(matches!(
                result.result,
                Ok(Err(ComputeAqlQueueSessionErrorV1::Contract(
                    "detached dispatch-data ledger was not empty"
                )))
            ));
            owner.assert_restored_v1(f.owner_snapshot(), ordinal == 0);
            assert_eq!(f.snapshot(), before);
            assert_eq!((f.trace.loans, f.trace.entered, f.trace.poisons), (0, 0, 1));
            assert!(f.trace.retained.as_ref().unwrap().cleanup.is_none());
            f.assert_retry();
            assert_eq!(identity.1.storage_identity(), identity.0);
            f.restore();
        }
    }
}

#[test]
fn recycled_detach_projection_failure_retains_disposed_receipt_and_all_data() {
    for ordinal in 0..3 {
        for bindings in [1, 3] {
            let controls = if bindings == 1 { 4 } else { 2 };
            for position in 0..controls {
                for panicked in [false, true] {
                    let mut f = DetachFixture::new(ordinal, bindings);
                    let owner = f.owner_snapshot();
                    f.memory_mut().arm_control_release_projection_v1(
                        position,
                        CleanupStageV1::ReleaseCommit,
                        panicked,
                    );
                    let result = f.detach();
                    assert!(result.transport);
                    if panicked {
                        assert_eq!(
                            result
                                .result
                                .err()
                                .unwrap()
                                .downcast_ref::<(&str, CleanupStageV1)>(),
                            Some(&("control cleanup projection", CleanupStageV1::ReleaseCommit))
                        );
                    } else {
                        assert!(matches!(
                            result.result,
                            Ok(Err(ComputeAqlQueueSessionErrorV1::DispatchBinding(
                                Gfx942DispatchBindingErrorV1::Memory(MemorySessionError::Injected(
                                    "control cleanup projection"
                                ))
                            )))
                        ));
                    }
                    let root = f.trace.retained.as_ref().unwrap().cleanup.as_ref().unwrap();
                    owner.assert_recycled_prefix_v1(root, position, true, false);
                    let snapshot = OwnerSnapshot::root_v1(root);
                    let active = snapshot.active_v1().unwrap();
                    assert!(
                        active.native_disposed
                            && active.failed
                            && active.stage != CleanupStageV1::Complete
                    );
                    assert_eq!(active.owner, "NativeDisposed");
                    f.assert_prefix(&owner, (position, true, 4, Some((true, 4, true, true))));
                    f.assert_retry();
                    f.restore();
                }
            }
        }
    }
}
