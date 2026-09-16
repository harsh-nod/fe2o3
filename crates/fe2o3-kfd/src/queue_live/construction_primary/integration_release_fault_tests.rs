use super::*;
use crate::shared_memory::CleanupStageV1 as Stage;
use fe2o3_runtime_model::{QueueHistoryEventKindV1, QueueSyscallStatusV1};

#[test]
fn constructed_primary_release_after_pristine_abort_preserves_original_queue_and_refunds() {
    use crate::shared_memory::{DataCleanupCustodyV1, DispatchDataReleaseV1};

    let (mut parent, t, gate) = constructed(true);
    let ids = original_resource_ids(&parent);
    let signal_id = Memory::primary_token_identity(parent.signals.as_ref().unwrap());
    let before = RetainedControlSnapshotV1::ordinary_owner_v1(parent.dispatch.as_ref().unwrap());
    let buffers = parent
        .dispatch
        .as_ref()
        .unwrap()
        .prepare_pristine_abort_v1()
        .unwrap();
    let engine = &mut parent.engine;
    let loan = engine
        .backend
        .session
        .primary_loan(&mut engine.foundation)
        .unwrap();
    let mut abort = parent
        .dispatch
        .take()
        .unwrap()
        .begin_pristine_abort_v1(buffers);
    abort.release_controls(&mut engine.backend.session).unwrap();
    let (continuation, data, identities) = abort.into_detached();
    assert_eq!(
        data.iter()
            .map(Gfx942FixedDispatchDataV1::sdma_storage_identity)
            .collect::<Vec<_>>(),
        before.data_order_v1()
    );
    assert_eq!(
        data.iter()
            .map(Gfx942FixedDispatchDataV1::storage_identity)
            .collect::<Vec<_>>(),
        identities
    );
    let mut returned: Vec<_> = data.into_iter().map(DataCleanupCustodyV1::new).collect();
    for owner in &mut returned {
        engine.backend.session.release_data(owner).unwrap();
        assert!(owner.is_complete());
    }
    engine
        .backend
        .session
        .primary_reclaim(&mut engine.foundation, loan)
        .unwrap();
    let returned_before: Vec<_> = returned
        .iter()
        .map(DataCleanupCustodyV1::observation)
        .collect();
    let mut state = PrimaryReleaseStateV1::<Fixture>::new();
    state.release_in_place(&mut parent).unwrap();
    assert_eq!(
        state
            .resources
            .as_ref()
            .unwrap()
            .observation()
            .controls
            .map(|c| c.identity),
        ids
    );
    assert_eq!(
        state.signals.as_ref().unwrap().observation().identity,
        signal_id
    );
    assert!(state.dispatch.is_none() && state.complete && !parent.poisoned);
    assert_eq!(
        returned
            .iter()
            .map(DataCleanupCustodyV1::observation)
            .collect::<Vec<_>>(),
        returned_before
    );
    parent
        .engine
        .backend
        .session
        .primary_assert_all_released_v1();
    assert_no_retry(&mut parent, &mut state, &t);
    drop(state);
    drop(parent);
    drop((continuation, returned));
    assert_eq!(gate.teardown_count(), 0);
    assert_eq!(gate.observation(), (false, false));
    assert_eq!(t.borrow().local_resources.live(), (0, 0, 0));
}

#[derive(Clone, Copy, Debug)]
enum DataFault {
    Native(&'static str),
    Currentness(usize),
    Projection(Stage),
}

fn data_failure(index: usize, fault: DataFault, panic: bool) {
    let (mut parent, t, gate) = constructed(true);
    let dispatch = RetainedControlSnapshotV1::ordinary_owner_v1(parent.dispatch.as_ref().unwrap());
    let order = dispatch.data_order_v1();
    assert_eq!(order.len(), 4);
    let signal_id = Memory::primary_token_identity(parent.signals.as_ref().unwrap());
    let memory = &mut parent.engine.backend.session;
    match fault {
        DataFault::Native(operation) => memory.primary_arm_data_native_v1(index, operation, panic),
        DataFault::Currentness(offset) => {
            memory.primary_arm_data_currentness_v1(index, offset, panic)
        }
        DataFault::Projection(stage) => memory.primary_arm_data_projection_v1(index, stage, panic),
    }
    let mut state = PrimaryReleaseStateV1::<Fixture>::new();
    let result = catch_unwind(AssertUnwindSafe(|| state.release_in_place(&mut parent)));
    if panic {
        let payload = result.unwrap_err();
        match fault {
            DataFault::Native(operation) => assert_eq!(
                payload.downcast_ref::<(&str, &str)>(),
                Some(&("N2 native panic", operation))
            ),
            DataFault::Currentness(_) => assert_eq!(
                payload.downcast_ref::<(&str, &str)>(),
                Some(&("N2 native panic", "currentness"))
            ),
            DataFault::Projection(stage) => assert_eq!(
                payload.downcast_ref::<(&str, Stage)>(),
                Some(&("control cleanup projection", stage))
            ),
        }
    } else {
        let expected = match fault {
            DataFault::Native(operation) => operation,
            DataFault::Currentness(_) => "currentness",
            DataFault::Projection(_) => "control cleanup projection",
        };
        assert!(
            matches!(result.unwrap(), Err(ComputeAqlQueueSessionErrorV1::DispatchBinding(Gfx942DispatchBindingErrorV1::Memory(MemorySessionError::Injected(actual)))) if actual == expected)
        );
    }
    assert!(state.resources.as_ref().unwrap().is_complete());
    let active = dispatch.assert_ordinary_data_prefix_v1(state.dispatch.as_ref().unwrap(), index);
    assert!(active.started && active.failed && !active.complete);
    match fault {
        DataFault::Native(operation) => {
            assert_eq!(
                active.owner,
                if operation == "unmap_gpu" {
                    "Mapped"
                } else {
                    "Unmapped"
                }
            );
            assert!(!active.native_disposed);
        }
        DataFault::Currentness(offset) => {
            let terminal = offset == if index.is_multiple_of(2) { 5 } else { 6 };
            assert_eq!(
                active.owner,
                if offset <= 2 {
                    "Mapped"
                } else if terminal {
                    "NativeDisposed"
                } else {
                    "Unmapped"
                }
            );
            assert_eq!(active.native_disposed, terminal);
        }
        DataFault::Projection(stage) => {
            assert_eq!(active.stage, stage);
            assert_eq!(active.native_disposed, stage == Stage::ReleaseCommit);
            assert_eq!(
                active.owner,
                if stage == Stage::ReleaseCommit {
                    "NativeDisposed"
                } else {
                    "Unmapped"
                }
            );
        }
    }
    t.borrow_mut()
        .post_resources_snapshot
        .take()
        .unwrap()
        .assert_primary_data_prefix_v1(
            &parent.engine.backend.session,
            &dispatch.order_v1(),
            &order,
            index,
            Some(&active),
        );
    assert_eq!(
        Memory::primary_token_identity(parent.signals.as_ref().unwrap()),
        signal_id
    );
    assert!(
        parent.dispatch.is_none() && parent.poisoned && state.signals.is_none() && !state.complete
    );
    assert_eq!(
        state.platform.as_ref().unwrap().progress(),
        (("disabled", false), 3)
    );
    assert_eq!(gate.teardown_count(), 1);
    assert!(gate.observation().1);
    assert!(!t.borrow().calls.contains(&"release-signals"));
    assert_no_retry(&mut parent, &mut state, &t);
}

#[test]
fn constructed_primary_release_dispatch_data_native_failures_keep_original_suffix() {
    for index in 0usize..4 {
        let operations: &[&str] = if index.is_multiple_of(2) {
            &["unmap_gpu", "free", "release_va_reservation"]
        } else {
            &["unmap_gpu", "unmap_cpu", "free", "release_va_reservation"]
        };
        for &operation in operations {
            for panic in [false, true] {
                data_failure(index, DataFault::Native(operation), panic);
            }
        }
    }
}

#[test]
fn constructed_primary_release_dispatch_data_currentness_keeps_disposal_receipts() {
    for index in 0usize..4 {
        for offset in 1..=if index.is_multiple_of(2) { 5 } else { 6 } {
            for panic in [false, true] {
                data_failure(index, DataFault::Currentness(offset), panic);
            }
        }
    }
}

#[test]
fn constructed_primary_release_dispatch_host_data_model_boundaries_retain_charge_truth() {
    for index in [1, 3] {
        for stage in [
            Stage::UnmapProjection,
            Stage::UnmapCommit,
            Stage::ReleaseProjection,
            Stage::ReleaseCommit,
        ] {
            for panic in [false, true] {
                data_failure(index, DataFault::Projection(stage), panic);
            }
        }
    }
}

#[test]
fn constructed_primary_release_resource_model_boundaries_keep_native_and_model_prefixes() {
    for index in 0..4 {
        for stage in [
            Stage::UnmapProjection,
            Stage::UnmapCommit,
            Stage::ReleaseProjection,
            Stage::ReleaseCommit,
        ] {
            for panic in [false, true] {
                let (mut parent, t, gate) = constructed(true);
                let dispatch =
                    RetainedControlSnapshotV1::ordinary_owner_v1(parent.dispatch.as_ref().unwrap());
                let signal_id = Memory::primary_token_identity(parent.signals.as_ref().unwrap());
                t.borrow_mut().queue_release_projection_fault = Some((index, stage, panic));
                let mut state = PrimaryReleaseStateV1::<Fixture>::new();
                let result = catch_unwind(AssertUnwindSafe(|| state.release_in_place(&mut parent)));
                if panic {
                    assert_eq!(
                        result.unwrap_err().downcast_ref::<(&str, Stage)>(),
                        Some(&("control cleanup projection", stage))
                    );
                } else {
                    assert!(matches!(
                        result.unwrap(),
                        Err(ComputeAqlQueueSessionErrorV1::Memory(
                            MemorySessionError::Injected("control cleanup projection")
                        ))
                    ));
                }
                t.borrow_mut()
                    .release_snapshot
                    .take()
                    .unwrap()
                    .assert_constructed_queue_projection_v1(
                        &parent.engine.backend.session,
                        state.resources.as_ref().unwrap(),
                        index,
                        stage,
                    );
                dispatch.assert_restored_v1(
                    RetainedControlSnapshotV1::ordinary_owner_v1(parent.dispatch.as_ref().unwrap()),
                    true,
                );
                assert_eq!(
                    Memory::primary_token_identity(parent.signals.as_ref().unwrap()),
                    signal_id
                );
                assert!(
                    state.dispatch.is_none()
                        && state.signals.is_none()
                        && !state.complete
                        && parent.poisoned
                );
                assert_eq!(gate.teardown_count(), 1);
                assert!(!t.borrow().calls.contains(&"complete-shadows"));
                assert_no_retry(&mut parent, &mut state, &t);
            }
        }
    }
}

#[test]
fn constructed_primary_release_publication_commit_rejection_retains_engine_authority() {
    let (mut parent, t, gate) = constructed(true);
    let ids = original_resource_ids(&parent);
    let memory = parent.engine.foundation.memory().clone();
    let dispatch = RetainedControlSnapshotV1::ordinary_owner_v1(parent.dispatch.as_ref().unwrap());
    let signal_id = Memory::primary_token_identity(parent.signals.as_ref().unwrap());
    parent.engine.release_fault = Some(PrimaryReleaseFaultV1::PublicationReturn);
    let mut state = PrimaryReleaseStateV1::<Fixture>::new();
    assert!(matches!(
        state.release_in_place(&mut parent),
        Err(ComputeAqlQueueSessionErrorV1::Native(
            "queue model projection"
        ))
    ));
    assert_eq!(parent.engine.foundation.memory(), &memory);
    assert_eq!(
        parent
            .engine
            .foundation
            .certificate_snapshot_for_test()
            .unwrap()
            .6,
        u64::MAX
    );
    assert_eq!(
        parent.engine.phase(parent.key),
        Some(ComputeAqlQueuePhaseV1::Destroyed)
    );
    assert_eq!(original_resource_ids(&parent), ids);
    assert!(parent.engine.release_fault.is_none() && parent.engine.backend.foundation_in_engine);
    assert_eq!(
        state.destroy.returned,
        Some((
            fe2o3_kfd_uapi::KfdIoctlDestroyQueueArgs::new(parent.queue_id),
            QueueSyscallStatusV1::Succeeded
        ))
    );
    assert!(
        state.authority.is_none()
            && state.resources.is_none()
            && state.dispatch.is_none()
            && state.signals.is_none()
    );
    dispatch.assert_restored_v1(
        RetainedControlSnapshotV1::ordinary_owner_v1(parent.dispatch.as_ref().unwrap()),
        true,
    );
    assert_eq!(
        Memory::primary_token_identity(parent.signals.as_ref().unwrap()),
        signal_id
    );
    assert!(!t.borrow().calls.contains(&"restore-foundation"));
    assert_eq!(
        state.platform.as_ref().unwrap().progress(),
        (("disabled", true), 3)
    );
    assert_eq!(gate.teardown_count(), 1);
    assert_no_retry(&mut parent, &mut state, &t);
}

#[test]
fn constructed_primary_release_corrupt_destroy_observation_retains_exact_native_receipt() {
    let (mut parent, t, gate) = constructed(true);
    let ids = original_resource_ids(&parent);
    let before = parent
        .engine
        .backend
        .session
        .primary_release_memory_snapshot_v1(&parent.engine.foundation);
    let history = parent.engine.model.history().to_vec();
    let mut absent = parent.key;
    absent.id.0 += 1;
    assert!(
        !parent
            .engine
            .model
            .queues()
            .iter()
            .any(|q| q.plan.queue == absent)
    );
    parent.engine.release_fault = Some(PrimaryReleaseFaultV1::DestroyObservation(absent));
    let mut state = PrimaryReleaseStateV1::<Fixture>::new();
    assert!(matches!(
        state.release_in_place(&mut parent),
        Err(ComputeAqlQueueSessionErrorV1::Native(
            "queue model projection"
        ))
    ));
    assert_eq!(
        parent.engine.phase(parent.key),
        Some(ComputeAqlQueuePhaseV1::DestroyPending)
    );
    assert!(parent.engine.authority_poisoned && parent.engine.release_fault.is_none());
    assert_eq!(&parent.engine.model.history()[..history.len()], history);
    assert_eq!(parent.engine.model.history().len(), history.len() + 1);
    assert_eq!(
        parent.engine.model.history().last().unwrap().event,
        QueueHistoryEventKindV1::DestroyBegan
    );
    assert_eq!(
        state.destroy.request,
        Some(fe2o3_kfd_uapi::KfdIoctlDestroyQueueArgs::new(
            parent.queue_id
        ))
    );
    assert_eq!(
        state.destroy.returned,
        Some((
            state.destroy.request.unwrap(),
            QueueSyscallStatusV1::Succeeded
        ))
    );
    assert!(
        state.destroy.attempted && parent.engine.backend.foundation_in_engine && parent.poisoned
    );
    assert_eq!(original_resource_ids(&parent), ids);
    before.assert_currentness_only_v1(
        parent
            .engine
            .backend
            .session
            .primary_release_memory_snapshot_v1(&parent.engine.foundation),
        1,
    );
    assert_eq!(
        state.platform.as_ref().unwrap().observation(),
        (true, (true, true), false, false)
    );
    assert!(
        parent.dispatch.is_some()
            && parent.signals.is_some()
            && state.authority.is_none()
            && state.resources.is_none()
    );
    assert!(!t.borrow().calls.contains(&"destroy-event"));
    assert_eq!(gate.teardown_count(), 1);
    assert_no_retry(&mut parent, &mut state, &t);
}
