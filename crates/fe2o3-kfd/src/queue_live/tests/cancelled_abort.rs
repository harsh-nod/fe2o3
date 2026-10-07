use super::*;
use crate::queue::dispatch_binding::pristine_abort::cancel_unpublished_fixture_epoch_v1;
use crate::queue::live::rebind::LiveRebindRootV1;
use std::cell::RefCell;
use std::panic::{AssertUnwindSafe, catch_unwind};

fn cancel(session: &mut ComputeAqlQueueSessionV1) {
    cancel_unpublished_fixture_epoch_v1(session.dispatch.as_mut().unwrap(), session.key);
}

fn abort_cancelled(
    memory: &mut PristineAbortMemoryFixtureV1,
    session: &mut ComputeAqlQueueSessionV1,
    closing: u8,
) -> Result<Vec<Gfx942FixedDispatchDataV1>, ComputeAqlQueueSessionErrorV1> {
    abort_admission_with_observer(
        memory,
        session,
        AbortAdmissionV1::CancelledOnly,
        closing,
        |_, _| {},
    )
}

#[test]
fn cancelled_abort_accepts_repeated_cancellation_without_fabricating_recycle() {
    for cancellations in [1, 3] {
        let (mut memory, mut session) = fixture();
        for _ in 0..cancellations {
            cancel(&mut session);
        }
        let calls = memory.native_calls();
        assert!(session.abort_unpublished_fixed_dispatch_v1().is_err());
        assert_eq!(calls, memory.native_calls());
        let data = abort_cancelled(&mut memory, &mut session, 0).unwrap();
        assert_eq!(data.len(), 5);
        assert_eq!(
            session.detached_data_identities,
            fixed_dispatch_storage_identities(&data)
        );
        assert_eq!(session.detached_data_count, 5);
        assert!(session.detached_dispatch_generation.is_none());
        let continuation = session.unpublished_dispatch.continuation.as_ref().unwrap();
        assert_eq!(continuation.next_generation_for_test(), 8 + cancellations);
        assert!(continuation.matches_queue(session.key));
        session.require_unbound_fixed_dispatch().unwrap();
        assert!(session.detach_recycled_fixed_dispatch().is_err());
        assert!(session.abort_cancelled_fixed_dispatch_v1().is_err());
        assert!(!session.terminal_poisoned);
        assert!(memory.data_is_retained());
    }
}

#[test]
fn cancelled_abort_rejects_pristine_foreign_ledger_and_completion_state_before_effects() {
    for mutation in 0..5 {
        let (mut memory, mut session) = fixture();
        if mutation != 0 {
            cancel(&mut session);
        }
        match mutation {
            1 => session.key.generation = fe2o3_runtime_model::QueueGenerationV1(99),
            2 => session.detached_dispatch_generation = Some(0),
            3 => session.detached_data_count = 1,
            4 => session.completion_owner.poison_owner(),
            _ => {}
        }
        let before = session.completion_owner.custody_snapshot_for_test();
        let calls = memory.native_calls();
        assert!(abort_cancelled(&mut memory, &mut session, 0).is_err());
        assert!(session.dispatch.is_some());
        assert!(session.unpublished_dispatch.is_clear());
        assert_eq!(memory.native_calls(), calls);
        assert_eq!(session.completion_owner.custody_snapshot_for_test(), before);
        assert!(!session.terminal_poisoned);
    }
}

#[test]
fn cancelled_abort_loan_rejection_preserves_retryable_owner() {
    let (mut memory, mut session) = fixture();
    cancel(&mut session);
    let calls = memory.native_calls();
    assert!(abort_cancelled(&mut memory, &mut session, 3).is_err());
    assert!(session.dispatch.is_some());
    assert!(session.unpublished_dispatch.is_clear());
    assert!(!session.terminal_poisoned);
    assert_eq!(calls, memory.native_calls());
    assert_eq!(
        abort_cancelled(&mut memory, &mut session, 0).unwrap().len(),
        5
    );
}

#[test]
fn cancelled_abort_failure_restores_all_lanes_before_parent_retention() {
    for ordinal in 0..3 {
        for failure in 0..4 {
            let (mut memory, mut session, lane) = transport_tests::selected_parent(ordinal);
            session
                .with_compute_lane_v1(lane, |selected| cancel(selected.session))
                .unwrap();
            if failure < 2 {
                memory.fail_control(2, "free", failure == 1);
            }
            let closing = if failure >= 2 { failure - 1 } else { 0 };
            let primary_key = session.key;
            let initial = RefCell::new(None);
            let expected = RefCell::new(None);
            let retained = RefCell::new(None);
            let result = catch_unwind(AssertUnwindSafe(|| {
                session.with_compute_lane_custody_v1(
                    lane,
                    |selected| {
                        selected.forward_pristine_abort_v1(|s| {
                            let settled = s.settle_pristine_abort_with_v1(|s| {
                                abort_admission_with_observer(
                                    &mut memory,
                                    s,
                                    AbortAdmissionV1::CancelledOnly,
                                    closing,
                                    |abort, _| {
                                        *initial.borrow_mut() =
                                            Some(abort.custody_snapshot_for_test());
                                    },
                                )
                            });
                            assert!(settled.transport);
                            let snapshot = s
                                .unpublished_dispatch
                                .terminal_abort
                                .as_ref()
                                .unwrap()
                                .custody_snapshot_for_test();
                            snapshot.assert_preserved_inputs(
                                initial.borrow().as_ref().unwrap(),
                                failure >= 2,
                            );
                            *expected.borrow_mut() = Some(snapshot);
                            settled
                        })
                    },
                    |parent| {
                        let state = parent.as_ref().as_ref().unwrap();
                        assert_eq!(state.key, primary_key);
                        assert!(state.terminal_poisoned);
                        let abort = if ordinal == 0 {
                            &state.unpublished_dispatch
                        } else {
                            assert!(state.unpublished_dispatch.is_clear());
                            &state.auxiliary_compute_lanes[ordinal - 1]
                                .state
                                .as_ref()
                                .unwrap()
                                .unpublished_dispatch
                        };
                        assert_eq!(
                            abort
                                .terminal_abort
                                .as_ref()
                                .unwrap()
                                .custody_snapshot_for_test(),
                            *expected.borrow().as_ref().unwrap()
                        );
                        *retained.borrow_mut() = Some(parent);
                    },
                )
            }));
            if failure % 2 == 1 {
                assert!(result.is_err());
            } else {
                assert!(result.unwrap().unwrap().is_err());
            }
            assert!(retained.borrow().is_some());
            assert!(memory.data_is_retained());
        }
    }
}

#[test]
fn cancelled_rebind_rejects_foreign_queue_or_exhaustion_before_consuming_custody() {
    for exhausted in [false, true] {
        let next = if exhausted { u64::MAX - 1 } else { 8 };
        let (mut memory, owner) = pristine_dispatch_fixture_v1(next);
        let (_, mut session) = fixture();
        session.dispatch = Some(owner);
        session.observation.ring_bytes = 4096;
        cancel(&mut session);
        let data = abort_cancelled(&mut memory, &mut session, 0).unwrap();
        if !exhausted {
            // Defensive private-state corruption, not a public queue migration.
            session.key.generation = fe2o3_runtime_model::QueueGenerationV1(99);
        }
        let original_data = (data.as_ptr(), data.len(), data.capacity());
        let calls = memory.native_calls();
        let ledger = session.detached_data_identities.clone();
        let ledger_storage = session.detached_data_identities.as_ptr();
        let continuation =
            std::ptr::from_ref(session.unpublished_dispatch.continuation.as_ref().unwrap());
        let _ = take_dispatch_terminal_process_gate_record_v1();
        let error = match session
            .preallocate_next_fixed_dispatch_v1::<1>(session.primary_compute_lane_v1())
        {
            Err(error) => error,
            Ok(_) => panic!("invalid continuation admitted preallocation"),
        };
        assert!(matches!(
            (exhausted, error),
            (
                true,
                ComputeAqlQueueSessionErrorV1::DispatchBinding(
                    Gfx942DispatchBindingErrorV1::GenerationExhausted
                )
            ) | (
                false,
                ComputeAqlQueueSessionErrorV1::DispatchBinding(
                    Gfx942DispatchBindingErrorV1::ResourcePhase
                )
            )
        ));
        let packet = Gfx942FixedDispatchPacketV1::new(
            0,
            fe2o3_aql::AqlDispatchGeometryV1::new([1, 1, 1], [1, 1, 1]).unwrap(),
            0,
            Vec::new().into_boxed_slice(),
            Vec::new().into_boxed_slice(),
        );
        let root = LiveRebindRootV1::new(Vec::new(), [packet], data, None);
        let retained = RefCell::new(None);
        let settled = session.settle_fixed_dispatch_rebind_with_v1(
            root,
            |_, _, _, _, _, _| panic!("rejected continuation entered preparation"),
            |_, _| panic!("rejected continuation entered validation"),
            |root| *retained.borrow_mut() = Some(root),
        );
        let error = settled.result.unwrap().unwrap_err();
        assert!(matches!(
            (exhausted, error),
            (
                true,
                ComputeAqlQueueSessionErrorV1::DispatchBinding(
                    Gfx942DispatchBindingErrorV1::GenerationExhausted
                )
            ) | (
                false,
                ComputeAqlQueueSessionErrorV1::DispatchBinding(
                    Gfx942DispatchBindingErrorV1::ResourcePhase
                )
            )
        ));
        assert!(!settled.transport && !session.terminal_poisoned);
        assert_eq!(
            std::ptr::from_ref(session.unpublished_dispatch.continuation.as_ref().unwrap()),
            continuation
        );
        assert_eq!(
            session
                .unpublished_dispatch
                .continuation
                .as_ref()
                .unwrap()
                .next_generation_for_test(),
            next + 1
        );
        assert_eq!(session.detached_data_identities, ledger);
        assert_eq!(session.detached_data_identities.as_ptr(), ledger_storage);
        assert_eq!(session.detached_data_count, 5);
        assert!(session.detached_dispatch_generation.is_none());
        assert!(session.detached_next_insertion_index.is_none());
        let retained = retained.borrow();
        let root = retained.as_ref().unwrap();
        assert!(root.preparation.is_none() && root.continuation.is_none());
        assert!(root.prepared_generation.is_none() && root.predecessor.is_none());
        assert!(root.programs.as_ref().unwrap().is_empty());
        assert!(root.packets.is_some());
        let data = root.data.as_ref().unwrap();
        assert_eq!((data.as_ptr(), data.len(), data.capacity()), original_data);
        assert_eq!(fixed_dispatch_storage_identities(data), ledger);
        assert_eq!(memory.native_calls(), calls);
        assert!(memory.data_is_retained());
        assert!(!take_dispatch_terminal_process_gate_record_v1());
    }
}
