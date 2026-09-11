//! Same-engine construction through the production outer ownership boundary.

use super::*;
use crate::queue::live::construction_auxiliary::{
    AuxiliaryConstructionScopeV1, AuxiliaryConstructionV1, AuxiliaryParentV1,
    AuxiliaryQueueTargetV1, PreparationResultV1, run_auxiliary_construction_with_v1,
};

type Scope = AuxiliaryConstructionScopeV1<3, Parent>;

#[path = "integration_prefix_tests.rs"]
mod prefix_cases;

struct Original {
    primary: Box<Root>,
    lanes: Vec<AuxiliaryComputeLaneSlotV1<ComputeAqlQueueLaneStateV1<Fixture>>>,
    data: Rc<RefCell<Option<crate::shared_memory::PreparationMemoryObservationV1>>>,
    preparation: Rc<RefCell<PrimaryPreparationSnapshotV1>>,
}

#[derive(Clone, Copy, Default, Debug, PartialEq, Eq)]
enum Outcome {
    #[default]
    Success,
    Error,
    Panic,
}

fn outcome(name: &'static str, value: Outcome) -> Result<(), ComputeAqlQueueSessionErrorV1> {
    let occurrence = record(name);
    match value {
        Outcome::Success => Ok(()),
        Outcome::Error => Err(ComputeAqlQueueSessionErrorV1::Contract(name)),
        Outcome::Panic => std::panic::panic_any((name, occurrence)),
    }
}

#[derive(Default)]
struct Faults {
    loan: Outcome,
    prepare_data: Outcome,
    operation: Outcome,
    reclaim_before: Outcome,
    reclaim_after: Outcome,
    regress_revision: bool,
}

struct Parent {
    original: Option<Original>,
    poisoned: bool,
    faults: Faults,
}

impl core::ops::Deref for Scope {
    type Target = Original;

    fn deref(&self) -> &Original {
        self.parent
            .original
            .as_ref()
            .or(self.terminal_parent.as_ref())
            .expect("complete original parent retained")
    }
}

impl Parent {
    fn poison(&mut self) {
        self.poisoned = true;
        let primary = self
            .original
            .as_mut()
            .unwrap()
            .primary
            .completed
            .as_mut()
            .unwrap();
        primary.dependency_owner.poison();
        primary.completion_owner.poison_owner();
        if let Some(dispatch) = primary.dispatch.as_mut() {
            dispatch.poison();
        }
        primary.submission.poison();
    }
}

impl AuxiliaryParentV1 for Parent {
    type Environment = Fixture;
    type TerminalParent = Original;

    fn check_currentness(&mut self) -> Result<(), ComputeAqlQueueSessionErrorV1> {
        self.original
            .as_mut()
            .unwrap()
            .primary
            .completed
            .as_mut()
            .unwrap()
            .engine
            .prepare_operation()
            .map_err(map_native)
    }

    fn with_preparation_custody(
        &mut self,
        work: impl FnOnce(&mut Memory) -> Result<(), ComputeAqlQueueSessionErrorV1>,
    ) -> PreparationResultV1 {
        execute_live_model_custody_v1(
            self,
            |parent| {
                outcome("auxiliary-loan", parent.faults.loan)?;
                let engine = &mut parent
                    .original
                    .as_mut()
                    .unwrap()
                    .primary
                    .completed
                    .as_mut()
                    .unwrap()
                    .engine;
                assert!(engine.backend.foundation_in_engine);
                let loan = engine
                    .backend
                    .session
                    .primary_loan(&mut engine.foundation)?;
                engine.backend.foundation_in_engine = false;
                record("auxiliary-loan-complete");
                Ok::<_, ComputeAqlQueueSessionErrorV1>(loan)
            },
            |parent| {
                let memory = &mut parent
                    .original
                    .as_mut()
                    .unwrap()
                    .primary
                    .completed
                    .as_mut()
                    .unwrap()
                    .engine
                    .backend
                    .session;
                work(memory)?;
                outcome("auxiliary-operation-return", parent.faults.operation)
            },
            |parent, loan| {
                outcome("auxiliary-retake", parent.faults.reclaim_before)?;
                let engine = &mut parent
                    .original
                    .as_mut()
                    .unwrap()
                    .primary
                    .completed
                    .as_mut()
                    .unwrap()
                    .engine;
                assert!(!engine.backend.foundation_in_engine);
                if parent.faults.regress_revision {
                    engine
                        .backend
                        .session
                        .primary_regress_loan_revision_v1(&loan);
                }
                engine
                    .backend
                    .session
                    .primary_reclaim(&mut engine.foundation, loan)?;
                engine.backend.foundation_in_engine = true;
                outcome("auxiliary-retake-complete", parent.faults.reclaim_after)
            },
            |parent| {
                parent.poison();
                Fixture::poison();
            },
        )
    }

    fn target(&mut self) -> AuxiliaryQueueTargetV1<'_, Fixture> {
        let original = self.original.as_mut().unwrap();
        let primary = original.primary.completed.as_mut().unwrap();
        AuxiliaryQueueTargetV1 {
            engine: &mut primary.engine,
            primary: &primary.observation,
            lanes: &mut original.lanes,
            sdma: None,
            striped_sdma: None,
        }
    }

    fn take_terminal_parent(&mut self) -> Original {
        self.poison();
        record("auxiliary-parent-retain");
        self.original.take().unwrap()
    }
}

fn run_auxiliary(
    scope: Box<Scope>,
    programs: Vec<ValidatedKernelEnvelope<'static>>,
) -> (Box<Scope>, Result<(), Box<dyn std::any::Any + Send>>) {
    let mut retained = None;
    let mut success = None;
    let slot = prepare_auxiliary_compute_lane_slot_v1(&scope.lanes).unwrap();
    let data_snapshot = scope.data.clone();
    let preparation = scope.preparation.clone();
    let prepare_data = scope.parent.faults.prepare_data;
    let result = catch_unwind(AssertUnwindSafe(|| {
        let result = run_auxiliary_construction_with_v1(
            scope,
            4096,
            &programs,
            slot,
            |memory| {
                outcome("auxiliary-data", prepare_data)?;
                let data = memory.roster();
                preparation.borrow_mut().capture_data(&data);
                *data_snapshot.borrow_mut() = Some(memory.observation());
                Ok(data)
            },
            |scope| {
                record("retain-auxiliary-root");
                retained = Some(scope);
            },
        );
        match result {
            Ok(scope) => success = Some(scope),
            Err(error) => std::panic::panic_any(error),
        }
    }));
    (
        retained
            .or(success)
            .expect("whole primary and auxiliary owners retained"),
        result,
    )
}

fn assert_pair(scope: &Scope) {
    let primary = scope.primary.completed.as_ref().unwrap();
    let memory = &primary.engine.backend.session;
    let installed = scope.lanes.first().and_then(|s| s.state.as_ref());
    let lane = installed.or(scope.construction.completed.as_ref());
    let dispatch = lane
        .and_then(|l| l.dispatch.as_ref())
        .or(scope.construction.dispatch.as_ref())
        .unwrap();
    let primary_dispatch = primary.dispatch.as_ref().unwrap();
    let mut owners = primary_dispatch.primary_fixture_identities_v1();
    owners.extend(dispatch.primary_fixture_identities_v1());
    let mut markers = Vec::new();
    mutable_ids(
        &scope.construction.completion,
        &mut owners,
        &mut markers,
        Memory::primary_token_identity,
    );
    if let Some(lane) = lane {
        assert!(
            lane.completion_owner.0.is_some(),
            "completed lane retains its completion ledger"
        );
        assert!(
            lane.submission.is_some(),
            "completed lane retains its submission owner"
        );
        assert!(scope.construction.completion_owner.is_none());
        assert!(scope.construction.submission.is_none());
        owners.push(Memory::primary_token_identity(
            lane.completion_signals.as_ref().unwrap(),
        ));
    }
    assert!(markers.is_empty());
    assert_partition(&scope.primary, owners);
    let primary_devices = primary_dispatch.device_authorities_inline_v1();
    let auxiliary_devices = dispatch.device_authorities_inline_v1();
    let devices: Vec<_> = primary_devices
        .iter()
        .chain(auxiliary_devices.iter())
        .copied()
        .collect();
    memory.primary_assert_device_owners(&devices);
    memory.primary_assert_accounts_and_records(trace().borrow().session);
    memory.assert_original_data_unchanged(scope.data.borrow().as_ref().unwrap());
    scope.primary.preparation.1.primary_assert_snapshot_v1(
        memory,
        trace().borrow().initial_preparation.as_ref().unwrap(),
        Some(primary_dispatch),
    );
    scope
        .construction
        .preparation
        .as_ref()
        .unwrap()
        .primary_assert_snapshot_v1(memory, &scope.preparation.borrow(), Some(dispatch));
    platform::assert_auxiliary_platform(&scope.primary, &scope.construction, installed);
    assert!(primary.engine.backend.foundation_in_engine);
    memory
        .primary_authenticate(&primary.engine.foundation)
        .unwrap();
    assert_eq!(primary.engine.resources.len(), 2);
    assert_ne!(primary.key, scope.construction.key.unwrap());
    assert_eq!(primary.engine.native_queue_id(primary.key), Some(7));
    if trace().borrow().create_collision {
        assert_eq!(
            primary.engine.phase(scope.construction.key.unwrap()),
            Some(fe2o3_runtime_model::ComputeAqlQueuePhaseV1::Ambiguous)
        );
        assert!(
            primary
                .engine
                .create_outputs(scope.construction.key.unwrap())
                .is_none()
        );
        assert!(
            primary
                .engine
                .native_queue_id(scope.construction.key.unwrap())
                .is_none()
        );
        assert!(scope.construction.outputs.is_none());
    } else {
        let outputs = primary
            .engine
            .create_outputs(scope.construction.key.unwrap())
            .unwrap();
        assert_eq!(
            primary
                .engine
                .native_queue_id(scope.construction.key.unwrap()),
            Some(outputs.queue_id().value())
        );
        assert_eq!(
            trace().borrow().create_returns[1],
            (outputs.queue_id().value(), outputs.doorbell_offset().raw())
        );
        if let Some(recovered) = scope.construction.outputs {
            assert_eq!(recovered, outputs);
        }
    }
    let t = trace();
    let t = t.borrow();
    for (name, count) in [
        ("foundation", 1),
        ("dependency", 1),
        ("auxiliary-loan", 1),
        ("auxiliary-retake", 1),
        ("create", 2),
        ("publish", 2),
    ] {
        assert_eq!(
            t.calls.iter().filter(|&&s| s == name).count(),
            count,
            "{name}"
        );
    }
    assert_eq!(
        t.cleanup, 0,
        "published shadows cannot be cleaned as unpublished"
    );
    assert_eq!(t.drops, 0);
}

fn assert_parent_transport(scope: &Scope, failed: bool) {
    assert_eq!(scope.parent.original.is_none(), failed);
    assert_eq!(scope.terminal_parent.is_some(), failed);
    assert_eq!(scope.parent.poisoned, failed);
    let primary = scope.primary.completed.as_ref().unwrap();
    assert_eq!(primary.completion_owner.is_poisoned_for_test(), failed);
    assert_eq!(primary.submission.is_poisoned_for_test(), failed);
    assert_eq!(
        matches!(
            primary.dispatch.as_ref().unwrap().ensure_releasable(),
            Err(Gfx942DispatchBindingErrorV1::Poisoned)
        ),
        failed
    );
    assert_eq!(
        matches!(
            primary.dependency_owner.ensure_idle(),
            Err(ComputeDependencyTargetUseErrorV1::Poisoned)
        ),
        failed
    );
}

fn assert_early_pair(scope: &Scope) {
    use crate::queue::dispatch_binding::preparation::PreparationOwnerRefsV1;
    let primary = scope.primary.completed.as_ref().unwrap();
    let memory = &primary.engine.backend.session;
    let root = &scope.construction;
    assert!(scope.lanes.is_empty());
    assert_eq!(primary.engine.resources.len(), 1);
    assert_eq!(primary.engine.native_queue_id(primary.key), Some(7));
    let mut refs = PreparationOwnerRefsV1::default();
    let primary_dispatch = primary.dispatch.as_ref().unwrap();
    refs.dispatch(primary_dispatch);
    scope.primary.preparation.1.primary_assert_snapshot_v1(
        memory,
        trace().borrow().initial_preparation.as_ref().unwrap(),
        Some(primary_dispatch),
    );
    if let Some(data) = &root.data {
        refs.data(data);
        scope.preparation.borrow().assert_data(data);
    }
    if let Some(preparation) = &root.preparation {
        preparation.primary_collect_owners_v1(&mut refs);
        preparation
            .primary_assert_descriptors_v1(&scope.preparation.borrow(), root.dispatch.as_ref());
    }
    if let Some(dispatch) = &root.dispatch {
        refs.dispatch(dispatch);
    }
    memory.primary_assert_device_partition_v1(&refs.device_leases, &refs.device_authorities);
    memory.primary_assert_shared_layouts_v1(&refs.shared);
    let mut owners = refs.shared.iter().map(|(id, _)| *id).collect();
    let mut markers = refs.in_session;
    ring_ids(&root.ring, &mut owners, &mut markers);
    mutable_ids(
        &root.control,
        &mut owners,
        &mut markers,
        Memory::primary_token_identity,
    );
    mutable_ids(
        &root.completion,
        &mut owners,
        &mut markers,
        Memory::primary_token_identity,
    );
    executable_ids(
        &root.eop,
        &mut owners,
        &mut markers,
        Memory::primary_token_identity,
    );
    executable_ids(
        &root.context,
        &mut owners,
        &mut markers,
        Memory::primary_token_identity,
    );
    if let Some(prefix) = &root.resource_prefix {
        owners.extend(prefix.primary_fixture_identities_v1());
    }
    if let Some(authority) = &root.authority {
        owners.extend(authority_ids(authority));
    }
    assert_partition_with_markers(&scope.primary, owners, markers);
    memory.primary_assert_accounts_and_records(trace().borrow().session);
    memory.assert_original_records_unchanged(trace().borrow().initial_data.as_ref().unwrap());
    if let Some(data) = scope.data.borrow().as_ref() {
        memory.assert_original_data_unchanged(data);
    }
    platform::assert_auxiliary_early_platform(&scope.primary, root);
    let t = trace();
    let t = t.borrow();
    for name in ["foundation", "dependency", "create", "publish"] {
        assert_eq!(t.calls.iter().filter(|&&s| s == name).count(), 1, "{name}");
    }
    assert_eq!(t.drops, 0);
}

fn exercise(fault: Option<(&'static str, bool)>, collision: bool) {
    let (memory, trace) = setup_memory_with_host_budget(2 << 20);
    let (primary, trace) = setup_with_memory(memory, trace);
    let primary_address = &*primary as *const Root as usize;
    let (mut primary, result) = run(primary, QueueRingBackingV1::AqlSpecial, false);
    assert!(result.is_ok(), "primary fixture prerequisite");
    assert_root(&primary, primary_address, &trace);
    let complete = primary.completed.as_mut().unwrap();
    complete.completion_owner.bind_barrier_probe().unwrap();
    complete
        .dependency_owner
        .reserve_acceptance_epoch()
        .unwrap();
    let completion = complete.completion_owner.custody_snapshot_for_test();
    let dependency = complete.dependency_owner.custody_snapshot_for_test();
    let engine_address = &complete.engine as *const _ as usize;
    let before = complete.engine.backend.session.observation();
    let primary_platform = platform::platform_identities(&primary, None);
    let call_prefix = trace.borrow().calls.clone();
    let (programs, packets) = recipe();
    let preparation = PrimaryPreparationSnapshotV1::packets(&packets);
    if let Some((name, panic)) = fault {
        let nth = trace.borrow().calls.iter().filter(|&&s| s == name).count() + 1;
        trace.borrow_mut().fault = Some((name, nth, panic));
    }
    trace.borrow_mut().create_collision = collision;
    let scope = Box::new(Scope {
        parent: Parent {
            original: Some(Original {
                primary,
                lanes: Vec::with_capacity(1),
                data: Rc::new(RefCell::new(None)),
                preparation: Rc::new(RefCell::new(preparation)),
            }),
            poisoned: false,
            faults: Faults::default(),
        },
        construction: AuxiliaryConstructionV1::new(packets),
        terminal_parent: None,
    });
    let scope_address = &*scope as *const Scope as usize;
    let slot_storage = scope.lanes.as_ptr();
    let (scope, result) = run_auxiliary(scope, programs);
    assert_parent_transport(&scope, result.is_err());
    assert_eq!(
        trace
            .borrow()
            .calls
            .iter()
            .filter(|&&s| s == "create")
            .count(),
        2,
        "unexpected early failure: {:?}; trace {:?}",
        result
            .as_ref()
            .err()
            .and_then(|p| p.downcast_ref::<ComputeAqlQueueSessionErrorV1>()),
        trace.borrow().calls
    );
    assert_eq!(&*scope as *const Scope as usize, scope_address);
    assert_eq!(&*scope.primary as *const Root as usize, primary_address);
    let primary = scope.primary.completed.as_ref().unwrap();
    assert_eq!(&primary.engine as *const _ as usize, engine_address);
    assert_eq!(
        primary.completion_owner.custody_snapshot_for_test(),
        completion
    );
    assert_eq!(
        primary.dependency_owner.custody_snapshot_for_test(),
        dependency
    );
    assert_eq!(
        platform::platform_identities(&scope.primary, None),
        primary_platform
    );
    assert_eq!(&trace.borrow().calls[..call_prefix.len()], call_prefix);
    primary
        .engine
        .backend
        .session
        .assert_original_records_unchanged(&before);
    assert_pair(&scope);
    let after = primary.engine.backend.session.observation();
    assert_eq!(
        after.device.unwrap().used_backing_bytes - before.device.unwrap().used_backing_bytes,
        8192
    );
    assert_eq!(
        after.device.unwrap().used_allocation_records
            - before.device.unwrap().used_allocation_records,
        2
    );
    assert_eq!(
        after.host.unwrap().used_backing_bytes - before.host.unwrap().used_backing_bytes,
        8192 + COMPLETION_SIGNAL_ARENA_BYTES_V1 as u64
    );
    assert_eq!(
        after.host.unwrap().used_allocation_records - before.host.unwrap().used_allocation_records,
        3
    );
    assert_eq!(scope.lanes.as_ptr(), slot_storage);
    if fault.is_some() || collision {
        assert!(result.is_err(), "injected late failure must reject");
        assert!(
            scope.lanes.is_empty(),
            "failure must not install auxiliary lane"
        );
        assert!(trace.borrow().poison);
        assert!(trace.borrow().calls.contains(&"retain-auxiliary-root"));
        if let Some((name, true)) = fault {
            let nth = call_prefix.iter().filter(|&&s| s == name).count() + 1;
            assert_eq!(
                result.unwrap_err().downcast_ref::<(&str, usize)>(),
                Some(&(name, nth))
            );
        } else {
            let (stage, message) = match fault {
                None => ("CREATE_QUEUE result", "queue syscall result indeterminate"),
                Some(("recover-outputs", false)) => {
                    ("CREATE_QUEUE output recovery", "missing CREATE outputs")
                }
                Some(("recover-id", false)) => {
                    ("CREATE_QUEUE identity recovery", "missing queue id")
                }
                Some(("doorbell", false)) => ("doorbell mapping", "doorbell"),
                Some(("gate-finish", false)) => {
                    ("USERPTR auxiliary queue-control creation", "gate-finish")
                }
                _ => unreachable!("named late fault"),
            };
            let error = result
                .as_ref()
                .unwrap_err()
                .downcast_ref::<ComputeAqlQueueSessionErrorV1>()
                .unwrap();
            let ComputeAqlQueueSessionErrorV1::TerminalCreation {
                stage: actual,
                source,
            } = error
            else {
                panic!("expected terminal creation: {error:?}");
            };
            assert_eq!(*actual, stage);
            if collision {
                assert!(
                    matches!(&**source, ComputeAqlQueueSessionErrorV1::Native(actual) if *actual == message),
                    "{error:?}"
                );
            } else {
                assert!(
                    matches!(&**source, ComputeAqlQueueSessionErrorV1::Contract(actual) if *actual == message),
                    "{error:?}"
                );
            }
        }
        assert_eq!(
            scope.construction.completed.is_none(),
            collision
                || fault.is_some_and(|(name, _)| matches!(name, "recover-outputs" | "recover-id"))
        );
    } else {
        assert!(
            result.is_ok(),
            "auxiliary success: {:?}",
            result
                .err()
                .and_then(|p| p.downcast::<ComputeAqlQueueSessionErrorV1>().ok())
        );
        assert!(!trace.borrow().poison);
        assert_eq!(scope.lanes.len(), 1);
        assert_eq!(scope.lanes[0].generation, 1);
        let lane = scope.lanes[0].state.as_ref().unwrap();
        assert_eq!(lane.observation.queue_id, 8);
        assert_eq!(lane.observation.event_id, 12);
        assert_eq!(lane.observation.doorbell_byte_offset, 16);
        assert!(scope.construction.completed.is_none());
        let t = trace.borrow();
        assert_eq!(
            &t.calls[t.calls.len() - 5..],
            &[
                "currentness",
                "doorbell",
                "doorbell-observe",
                "currentness",
                "gate-finish"
            ]
        );
    }
}

#[test]
fn same_engine_auxiliary_success_preserves_primary_and_exact_owner_partition() {
    exercise(None, false);
}

#[test]
fn same_engine_auxiliary_late_failures_retain_both_queues_without_installation() {
    exercise(None, true);
    for (name, panic) in [
        ("recover-outputs", false),
        ("recover-id", false),
        ("doorbell", false),
        ("doorbell", true),
        ("doorbell-observe", true),
        ("gate-finish", false),
        ("gate-finish", true),
    ] {
        exercise(Some((name, panic)), false);
    }
}
