//! Actual constructed auxiliary owners through the production teardown driver.

use super::*;
use crate::queue::dispatch_binding::control_release::RetainedControlSnapshotV1;
use crate::queue::live::auxiliary_release::{
    AuxiliaryReleaseContextV1, AuxiliaryReleasePartsV1, release_in_place,
};
use crate::queue_linux::primary_fixture::LocalRuntimeObservationV1;

#[path = "integration_release_fault_tests.rs"]
mod fault_cases;

impl AuxiliaryReleaseContextV1 for Parent {
    type Environment = Fixture;

    fn admit(&self) -> Result<(), ComputeAqlQueueSessionErrorV1> {
        if self.poisoned {
            return Err(Gfx942DispatchBindingErrorV1::Poisoned.into());
        }
        Ok(())
    }

    fn parts(
        &mut self,
    ) -> Result<AuxiliaryReleasePartsV1<'_, Fixture>, ComputeAqlQueueSessionErrorV1> {
        let original = self.original.as_mut().unwrap();
        let primary = original.primary.completed.as_mut().unwrap();
        Ok(AuxiliaryReleasePartsV1 {
            engine: &mut primary.engine,
            session_key: primary.key,
            dependency: &primary.dependency_owner,
            lanes: &mut original.lanes,
            custody: &mut original.release,
        })
    }

    fn loan(&mut self) -> Result<LiveQueueModelFoundationLoanV1, ComputeAqlQueueSessionErrorV1> {
        outcome("auxiliary-release-loan", self.faults.loan)?;
        let engine = &mut self
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
        Ok(loan)
    }

    fn retake(
        &mut self,
        loan: LiveQueueModelFoundationLoanV1,
    ) -> Result<(), ComputeAqlQueueSessionErrorV1> {
        outcome("auxiliary-release-retake", self.faults.reclaim_before)?;
        let engine = &mut self
            .original
            .as_mut()
            .unwrap()
            .primary
            .completed
            .as_mut()
            .unwrap()
            .engine;
        assert!(!engine.backend.foundation_in_engine);
        if self.faults.regress_revision {
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
        outcome(
            "auxiliary-release-retake-complete",
            self.faults.reclaim_after,
        )
    }

    fn poison(&mut self) {
        Parent::poison(self);
        if self.faults.poison_panic {
            std::panic::panic_any("auxiliary release poison");
        }
    }
}

fn constructed() -> (Box<Scope>, ComputeAqlQueueLaneV1, Rc<RefCell<Trace>>) {
    let (memory, t) = setup_memory_with_host_budget(2 << 20);
    t.borrow_mut().local_gate = Some(LocalGateV1::new());
    let (primary, t) = setup_with_memory(memory, t);
    let (primary, result) = run(primary, QueueRingBackingV1::AqlSpecial, false);
    assert!(result.is_ok(), "completed primary prerequisite");
    let (programs, packets) = recipe();
    let preparation = PrimaryPreparationSnapshotV1::packets(&packets);
    let scope = Box::new(Scope {
        parent: Parent {
            original: Some(Original {
                primary,
                lanes: Vec::with_capacity(1),
                sdma: None,
                striped_sdma: None,
                release: None,
                data: Rc::new(RefCell::new(None)),
                preparation: Rc::new(RefCell::new(preparation)),
            }),
            poisoned: false,
            faults: Faults::default(),
        },
        construction: AuxiliaryConstructionV1::new(packets),
        terminal_parent: None,
    });
    let (scope, result) = run_auxiliary(scope, programs);
    assert!(result.is_ok(), "completed auxiliary prerequisite");
    let primary = scope.primary.completed.as_ref().unwrap();
    let lane = ComputeAqlQueueLaneV1 {
        session: primary.key,
        ordinal: 1,
        generation: scope.lanes[0].generation,
    };
    assert_pair_custody(&scope);
    t.borrow_mut().destroy = Some(0);
    (scope, lane, t)
}

fn assert_no_retry(scope: &mut Scope, lane: ComputeAqlQueueLaneV1) {
    fn snapshot(scope: &Scope) -> impl core::fmt::Debug + PartialEq + use<> {
        let primary = scope.primary.completed.as_ref().unwrap();
        let engine = &primary.engine;
        let state = scope.lanes[0].state.as_ref().unwrap();
        let root = scope.release.as_ref().unwrap();
        (
            (
                root.identity,
                root.destroy,
                root.memory_complete,
                root.gate.is_some(),
            ),
            (
                root.authority.as_ref().map(authority_ids),
                root.resources.as_ref().map(|r| r.observation()),
                root.signals.as_ref().map(|s| s.observation()),
                root.dispatch
                    .as_ref()
                    .map(RetainedControlSnapshotV1::ordinary_root_v1),
                root.platform
                    .as_ref()
                    .map(|p| (p.identities(), p.observation(), p.progress())),
            ),
            (
                scope.lanes.as_ptr() as usize,
                scope.lanes.capacity(),
                scope.lanes[0].generation,
                state.key,
                state
                    .exception
                    .as_ref()
                    .map(|e| [e.runtime.identity, e.event.identity, e.shadows.identity]),
                state.doorbell.as_ref().map(|d| d.identity),
                state
                    .dispatch
                    .as_ref()
                    .map(RetainedControlSnapshotV1::ordinary_owner_v1),
                state
                    .completion_signals
                    .as_ref()
                    .map(Memory::primary_token_identity),
            ),
            (
                engine
                    .backend
                    .session
                    .retained_queue_memory_snapshot_v1(&engine.foundation),
                engine.model.clone(),
                engine.authority_poisoned,
                engine.backend.foundation_in_engine,
                engine
                    .resources
                    .iter()
                    .map(|r| (r.key, r.authority.as_ref().map(authority_ids)))
                    .collect::<Vec<_>>(),
                primary.completion_owner.custody_snapshot_for_test(),
                primary.dependency_owner.custody_snapshot_for_test(),
            ),
            trace().borrow().calls.clone(),
            trace().borrow().local_gate.as_ref().unwrap().observation(),
            trace()
                .borrow()
                .local_gate
                .as_ref()
                .unwrap()
                .runtime_observation(),
            trace()
                .borrow()
                .local_gate
                .as_ref()
                .unwrap()
                .teardown_count(),
        )
    }
    assert!(scope.parent.poisoned && trace().borrow().poison);
    let before = snapshot(scope);
    assert!(release_in_place(&mut scope.parent, lane).is_err());
    assert_eq!(snapshot(scope), before);
}

#[test]
fn constructed_auxiliary_release_success_preserves_primary_and_slot_generation() {
    let (mut scope, lane, t) = constructed();
    let primary = scope.primary.completed.as_ref().unwrap();
    let key = primary.key;
    let primary_address = &*scope.primary as *const Root as usize;
    let engine_address = &primary.engine as *const _ as usize;
    let primary_platform = platform::platform_identities(&scope.primary, None);
    let completion = primary.completion_owner.custody_snapshot_for_test();
    let dependency = primary.dependency_owner.custody_snapshot_for_test();
    let primary_signals = Memory::primary_token_identity(&primary.completion_signals);
    let primary_resources = authority_ids(
        primary
            .engine
            .resources
            .iter()
            .find(|r| r.key == key)
            .unwrap()
            .authority
            .as_ref()
            .unwrap(),
    );
    let aux_key = scope.lanes[0].state.as_ref().unwrap().key;
    let aux_record = primary
        .engine
        .resources
        .iter()
        .find(|r| r.key == aux_key)
        .unwrap();
    let aux_history = (aux_record.view.plan, aux_record.create_outputs);
    let publications: Vec<_> = primary
        .engine
        .foundation
        .memory()
        .publications()
        .iter()
        .filter(|p| {
            p.owner == fe2o3_runtime_model::MemoryPublicationOwnerV1::ComputeAqlQueue(aux_key)
        })
        .map(|p| p.key)
        .collect();
    assert_eq!(publications.len(), 4);
    let gate = t.borrow().local_gate.as_ref().unwrap().clone();
    let roster = (scope.lanes.as_ptr(), scope.lanes.capacity());
    let generation = scope.lanes[0].generation;
    let call_start = t.borrow().calls.len();
    release_in_place(&mut scope.parent, lane).unwrap();
    assert!(scope.release.is_none() && scope.lanes[0].state.is_none());
    assert!(!scope.parent.poisoned && !t.borrow().poison);
    assert_eq!((scope.lanes.as_ptr(), scope.lanes.capacity()), roster);
    assert_eq!(scope.lanes[0].generation, generation);
    let primary = scope.primary.completed.as_ref().unwrap();
    assert_eq!(&*scope.primary as *const Root as usize, primary_address);
    assert_eq!(&primary.engine as *const _ as usize, engine_address);
    assert_eq!(
        platform::platform_identities(&scope.primary, None),
        primary_platform
    );
    assert_eq!(
        primary.completion_owner.custody_snapshot_for_test(),
        completion
    );
    assert_eq!(
        primary.dependency_owner.custody_snapshot_for_test(),
        dependency
    );
    assert_eq!(
        Memory::primary_token_identity(&primary.completion_signals),
        primary_signals
    );
    assert_eq!(
        authority_ids(
            primary
                .engine
                .resources
                .iter()
                .find(|r| r.key == key)
                .unwrap()
                .authority
                .as_ref()
                .unwrap()
        ),
        primary_resources
    );
    assert!(primary.engine.backend.foundation_in_engine);
    assert_eq!(
        primary.engine.phase(key),
        Some(ComputeAqlQueuePhaseV1::Active)
    );
    let records: Vec<_> = primary
        .engine
        .resources
        .iter()
        .filter(|r| r.key == aux_key)
        .collect();
    assert_eq!(records.len(), 1);
    assert!(records[0].authority.is_none());
    assert_eq!(
        (records[0].view.plan, records[0].create_outputs),
        aux_history
    );
    assert_eq!(
        primary.engine.phase(aux_key),
        Some(ComputeAqlQueuePhaseV1::Destroyed)
    );
    for key in publications {
        let record = primary
            .engine
            .foundation
            .memory()
            .publications()
            .iter()
            .find(|p| p.key == key)
            .unwrap();
        assert_eq!(
            record.owner,
            fe2o3_runtime_model::MemoryPublicationOwnerV1::ComputeAqlQueue(aux_key)
        );
        assert_eq!(
            record.state,
            fe2o3_runtime_model::MemoryPublicationStateV1::Released
        );
    }
    primary
        .engine
        .backend
        .session
        .assert_original_records_unchanged(t.borrow().initial_data.as_ref().unwrap());
    scope.primary.preparation.1.primary_assert_snapshot_v1(
        &primary.engine.backend.session,
        t.borrow().initial_preparation.as_ref().unwrap(),
        primary.dispatch.as_ref(),
    );
    assert_eq!(
        gate.runtime_observation(),
        LocalRuntimeObservationV1::Enabled {
            opener_pid: std::process::id(),
            leases: 1
        }
    );
    assert_eq!(gate.teardown_count(), 0);
    assert!(admit_compute_lane_v1(key, &scope.lanes, lane).is_err());
    let next = prepare_auxiliary_compute_lane_slot_v1(&scope.lanes).unwrap();
    assert_eq!(
        (next.index, next.generation, next.append),
        (0, generation + 1, false)
    );
    let calls = t.borrow().calls[call_start..].to_vec();
    assert!(!calls.contains(&"restore-foundation"));
    assert!(
        !calls.contains(&"disable-runtime"),
        "primary retains its runtime lease"
    );
    let order = [
        "destroy",
        "destroy-event",
        "release-doorbell",
        "auxiliary-release-loan",
        "release-resources",
        "complete-shadows",
        "release-signals",
        "auxiliary-release-retake",
        "auxiliary-release-retake-complete",
    ];
    let positions: Vec<_> = order
        .iter()
        .map(|name| calls.iter().position(|call| call == name).unwrap())
        .collect();
    assert!(positions.windows(2).all(|pair| pair[0] < pair[1]));
    let complete = scope
        .parent
        .original
        .as_mut()
        .unwrap()
        .primary
        .completed
        .take()
        .unwrap();
    super::super::release_cases::release_completed_parent_after_auxiliary(complete);
    assert_eq!(
        gate.runtime_observation(),
        LocalRuntimeObservationV1::Disabled
    );
}

#[test]
fn constructed_auxiliary_release_reuses_vacant_slot_with_fresh_native_generation() {
    let (mut scope, old_lane, t) = constructed();
    let old_key = scope.lanes[0].state.as_ref().unwrap().key;
    let roster = (scope.lanes.as_ptr(), scope.lanes.capacity());
    let engine_address = &scope.primary.completed.as_ref().unwrap().engine as *const _;
    let old_create = t.borrow().create_returns[1];
    release_in_place(&mut scope.parent, old_lane).unwrap();
    let (programs, packets) = recipe();
    *scope.preparation.borrow_mut() = PrimaryPreparationSnapshotV1::packets(&packets);
    scope.construction = AuxiliaryConstructionV1::new(packets);
    let (mut scope, result) = run_auxiliary(scope, programs);
    assert!(result.is_ok(), "reconstructed auxiliary prerequisite");
    assert_eq!((scope.lanes.as_ptr(), scope.lanes.capacity()), roster);
    assert_eq!(scope.lanes.len(), 1);
    let primary = scope.primary.completed.as_ref().unwrap();
    assert_eq!(&primary.engine as *const _, engine_address);
    let slot = &scope.lanes[0];
    assert_eq!(slot.generation, old_lane.generation + 1);
    let state = slot.state.as_ref().unwrap();
    assert_ne!(state.key, old_key);
    assert_eq!(t.borrow().create_returns.len(), 3);
    assert_ne!(t.borrow().create_returns[2], old_create);
    assert_eq!(state.observation.queue_id(), t.borrow().create_returns[2].0);
    let new_lane = ComputeAqlQueueLaneV1 {
        generation: slot.generation,
        ..old_lane
    };
    assert!(matches!(
        admit_compute_lane_v1(primary.key, &scope.lanes, new_lane),
        Ok(AdmittedComputeLaneV1::Auxiliary(0))
    ));
    assert!(admit_compute_lane_v1(primary.key, &scope.lanes, old_lane).is_err());
    let history = primary
        .engine
        .resources
        .iter()
        .find(|r| r.key == old_key)
        .unwrap();
    assert!(history.authority.is_none());
    assert_eq!(
        primary.engine.phase(old_key),
        Some(ComputeAqlQueuePhaseV1::Destroyed)
    );
    assert!(
        primary
            .engine
            .resources
            .iter()
            .find(|r| r.key == state.key)
            .unwrap()
            .authority
            .is_some()
    );
    let memory = &primary.engine.backend.session;
    scope.primary.preparation.1.primary_assert_snapshot_v1(
        memory,
        t.borrow().initial_preparation.as_ref().unwrap(),
        primary.dispatch.as_ref(),
    );
    scope
        .construction
        .preparation
        .as_ref()
        .unwrap()
        .primary_assert_snapshot_v1(memory, &scope.preparation.borrow(), state.dispatch.as_ref());
    release_in_place(&mut scope.parent, new_lane).unwrap();
    assert!(scope.lanes[0].state.is_none() && scope.release.is_none());
    assert_eq!(scope.lanes[0].generation, new_lane.generation);
    let complete = scope
        .parent
        .original
        .as_mut()
        .unwrap()
        .primary
        .completed
        .take()
        .unwrap();
    super::super::release_cases::release_completed_parent_after_auxiliary(complete);
}

#[test]
fn constructed_auxiliary_release_preflight_rejection_leaves_original_owners_live() {
    for case in 0..4 {
        let (mut scope, mut lane, t) = constructed();
        match case {
            0 => lane.ordinal = 0,
            1 => lane.generation += 1,
            2 => {
                scope.parent.original.as_mut().unwrap().lanes[0]
                    .state
                    .as_mut()
                    .unwrap()
                    .completion_owner
                    .bind_barrier_probe()
                    .unwrap();
            }
            _ => {
                scope
                    .parent
                    .original
                    .as_mut()
                    .unwrap()
                    .primary
                    .completed
                    .as_mut()
                    .unwrap()
                    .dependency_owner
                    .poison();
            }
        }
        let primary = scope.primary.completed.as_ref().unwrap();
        let before = primary
            .engine
            .backend
            .session
            .retained_queue_memory_snapshot_v1(&primary.engine.foundation);
        let model = primary.engine.model.clone();
        let dependencies = primary.dependency_owner.custody_snapshot_for_test();
        let completions = scope.lanes[0]
            .state
            .as_ref()
            .unwrap()
            .completion_owner
            .custody_snapshot_for_test();
        let calls = t.borrow().calls.clone();
        assert!(
            release_in_place(&mut scope.parent, lane).is_err(),
            "preflight case {case}"
        );
        assert!(scope.release.is_none() && !scope.parent.poisoned && !t.borrow().poison);
        assert_eq!(t.borrow().calls, calls);
        let primary = scope.primary.completed.as_ref().unwrap();
        assert_eq!(
            primary
                .engine
                .backend
                .session
                .retained_queue_memory_snapshot_v1(&primary.engine.foundation),
            before
        );
        assert_eq!(primary.engine.model, model);
        assert_eq!(
            primary.dependency_owner.custody_snapshot_for_test(),
            dependencies
        );
        assert_eq!(
            scope.lanes[0]
                .state
                .as_ref()
                .unwrap()
                .completion_owner
                .custody_snapshot_for_test(),
            completions
        );
        assert_pair_custody(&scope);
    }
}

#[test]
fn constructed_auxiliary_release_destroy_errors_and_panic_retain_original_slot() {
    for mode in 1..=4 {
        let (mut scope, lane, t) = constructed();
        let key = scope.lanes[0].state.as_ref().unwrap().key;
        let state = scope.lanes[0].state.as_ref().unwrap();
        let exception = state.exception.as_ref().unwrap();
        let platform = [
            exception.runtime.identity,
            exception.event.identity,
            exception.shadows.identity,
            state.doorbell.as_ref().unwrap().identity,
        ];
        t.borrow_mut().destroy = Some(mode);
        let result = catch_unwind(AssertUnwindSafe(|| {
            release_in_place(&mut scope.parent, lane)
        }));
        if mode == 3 {
            assert_eq!(
                result.unwrap_err().downcast_ref::<&str>(),
                Some(&"DESTROY panic")
            );
        } else {
            assert!(result.unwrap().is_err());
        }
        assert_eq!(scope.lanes[0].state.as_ref().unwrap().key, key);
        assert_eq!(
            scope
                .release
                .as_ref()
                .unwrap()
                .platform
                .as_ref()
                .unwrap()
                .identities(),
            platform
        );
        assert!(scope.release.as_ref().unwrap().resources.is_none());
        assert_no_retry(&mut scope, lane);
    }
}

#[test]
fn constructed_auxiliary_release_platform_and_cleanup_errors_preserve_prefixes() {
    for stage in [
        "destroy-event",
        "zero-payload",
        "protect-payload",
        "unmap-payload",
        "release-doorbell",
        "release-validate-platform",
        "release-resources",
        "complete-shadows",
        "release-signals",
    ] {
        for panic in [false, true] {
            let (mut scope, lane, t) = constructed();
            let nth = t
                .borrow()
                .calls
                .iter()
                .filter(|&&name| name == stage)
                .count()
                + 1;
            t.borrow_mut().fault = Some((stage, nth, panic));
            let result = catch_unwind(AssertUnwindSafe(|| {
                release_in_place(&mut scope.parent, lane)
            }));
            if panic {
                assert_eq!(
                    result.unwrap_err().downcast_ref::<(&str, usize)>(),
                    Some(&(stage, nth))
                );
            } else {
                assert!(result.unwrap().is_err(), "{stage}");
            }
            assert!(scope.lanes[0].state.is_some());
            assert!(!scope.release.as_ref().unwrap().memory_complete);
            assert!(scope.release.as_ref().unwrap().gate.is_some());
            assert_no_retry(&mut scope, lane);
        }
    }
}

#[test]
fn constructed_auxiliary_release_model_failures_keep_settled_receipts_and_first_panic() {
    for boundary in 0..3 {
        for panic in [false, true] {
            for poison_panic in [false, true] {
                let (mut scope, lane, _) = constructed();
                let failure = if panic {
                    Outcome::Panic
                } else {
                    Outcome::Error
                };
                let stage = match boundary {
                    0 => {
                        scope.parent.faults.loan = failure;
                        "auxiliary-release-loan"
                    }
                    1 => {
                        scope.parent.faults.reclaim_before = failure;
                        "auxiliary-release-retake"
                    }
                    _ => {
                        scope.parent.faults.reclaim_after = failure;
                        "auxiliary-release-retake-complete"
                    }
                };
                scope.parent.faults.poison_panic = poison_panic;
                let result = catch_unwind(AssertUnwindSafe(|| {
                    release_in_place(&mut scope.parent, lane)
                }));
                if panic {
                    assert_eq!(
                        result.unwrap_err().downcast_ref::<(&str, usize)>(),
                        Some(&(stage, 1))
                    );
                } else {
                    assert!(
                        matches!(result.unwrap(), Err(ComputeAqlQueueSessionErrorV1::Contract(name)) if name == stage)
                    );
                }
                let root = scope.release.as_ref().unwrap();
                assert_eq!(root.memory_complete, boundary != 0);
                assert_eq!(
                    root.resources.as_ref().unwrap().is_complete(),
                    boundary != 0
                );
                assert_eq!(
                    scope
                        .primary
                        .completed
                        .as_ref()
                        .unwrap()
                        .engine
                        .backend
                        .foundation_in_engine,
                    boundary != 1
                );
                assert_no_retry(&mut scope, lane);
            }
        }
    }
}
