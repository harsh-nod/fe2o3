//! Lower cleanup failures while the original primary retains the shared model.

use super::*;
use crate::shared_memory::CleanupStageV1 as Stage;

fn memory(scope: &mut Scope) -> &mut Memory {
    &mut scope
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

fn assert_native_failure(
    result: std::thread::Result<Result<(), ComputeAqlQueueSessionErrorV1>>,
    operation: &'static str,
    panic: bool,
) {
    if panic {
        assert_eq!(
            result.unwrap_err().downcast_ref::<(&str, &str)>(),
            Some(&("N2 native panic", operation))
        );
    } else {
        assert!(matches!(result.unwrap(),
            Err(ComputeAqlQueueSessionErrorV1::Memory(MemorySessionError::Injected(actual)))
            | Err(ComputeAqlQueueSessionErrorV1::DispatchBinding(Gfx942DispatchBindingErrorV1::Memory(MemorySessionError::Injected(actual))))
            if actual == operation));
    }
}

fn settled_engine(scope: &Scope) -> &NativeQueueEngineV1<PrimaryQueueBackendV1<Memory>> {
    let engine = &scope.primary.completed.as_ref().unwrap().engine;
    assert!(engine.backend.foundation_in_engine);
    assert_eq!(
        engine
            .backend
            .session
            .primary_loan_state_v1(&engine.foundation)
            .1,
        None
    );
    engine
}

#[test]
fn constructed_auxiliary_release_queue_native_failures_keep_exact_prefix_and_suffix() {
    let calls = [
        "unmap_gpu",
        "unmap_gpu",
        "unmap_gpu",
        "unmap_gpu",
        "unmap_cpu",
        "free",
        "release_va_reservation",
        "free",
        "unmap_cpu",
        "unmap_cpu",
        "free",
        "release_va_reservation",
        "unmap_cpu",
        "free",
        "release_va_reservation",
    ];
    for failed in [0, 3, 4, 6, 7, 8, 11, 14] {
        for panic in [false, true] {
            let (mut scope, lane, t) = constructed();
            let state = scope.lanes[0].state.as_ref().unwrap();
            let dispatch =
                RetainedControlSnapshotV1::ordinary_owner_v1(state.dispatch.as_ref().unwrap());
            let signals =
                Memory::primary_token_identity(state.completion_signals.as_ref().unwrap());
            memory(&mut scope).primary_fail_cleanup_call_v1(failed + 1, calls[failed], panic);
            assert_native_failure(
                catch_unwind(AssertUnwindSafe(|| {
                    release_in_place(&mut scope.parent, lane)
                })),
                calls[failed],
                panic,
            );
            let engine = settled_engine(&scope);
            let root = scope.release.as_ref().unwrap();
            t.borrow_mut()
                .release_snapshot
                .take()
                .unwrap()
                .assert_auxiliary_queue_failure_v1(
                    &engine.backend.session,
                    &engine.foundation,
                    root.resources.as_ref().unwrap(),
                    failed,
                    panic,
                );
            let state = scope.lanes[0].state.as_ref().unwrap();
            dispatch.assert_restored_v1(
                RetainedControlSnapshotV1::ordinary_owner_v1(state.dispatch.as_ref().unwrap()),
                false,
            );
            assert_eq!(
                Memory::primary_token_identity(state.completion_signals.as_ref().unwrap()),
                signals
            );
            assert!(root.dispatch.is_none() && root.signals.is_none() && !root.memory_complete);
            assert!(!t.borrow().calls.contains(&"complete-shadows"));
            assert_no_retry(&mut scope, lane);
        }
    }
}

#[test]
fn constructed_auxiliary_release_queue_currentness_keeps_disposal_receipts() {
    for point in [1, 8, 9, 12, 16, 20, 24] {
        for panic in [false, true] {
            let (mut scope, lane, t) = constructed();
            t.borrow_mut().queue_release_currentness_fault = Some((point, panic));
            assert_native_failure(
                catch_unwind(AssertUnwindSafe(|| {
                    release_in_place(&mut scope.parent, lane)
                })),
                "currentness",
                panic,
            );
            let engine = settled_engine(&scope);
            let root = scope.release.as_ref().unwrap();
            t.borrow_mut()
                .release_snapshot
                .take()
                .unwrap()
                .assert_auxiliary_queue_currentness_v1(
                    &engine.backend.session,
                    &engine.foundation,
                    root.resources.as_ref().unwrap(),
                    point,
                );
            assert!(root.dispatch.is_none() && root.signals.is_none() && !root.memory_complete);
            assert_no_retry(&mut scope, lane);
        }
    }
}

#[test]
fn constructed_auxiliary_release_dispatch_control_failures_keep_original_signal() {
    for index in [0, 3] {
        for (step, operation) in ["unmap_gpu", "unmap_cpu", "free", "release_va_reservation"]
            .into_iter()
            .enumerate()
        {
            for panic in [false, true] {
                let (mut scope, lane, t) = constructed();
                let state = scope.lanes[0].state.as_ref().unwrap();
                let dispatch =
                    RetainedControlSnapshotV1::ordinary_owner_v1(state.dispatch.as_ref().unwrap());
                let signal =
                    Memory::primary_token_identity(state.completion_signals.as_ref().unwrap());
                memory(&mut scope).arm_control_release_native_v1(index, operation, panic);
                assert_native_failure(
                    catch_unwind(AssertUnwindSafe(|| {
                        release_in_place(&mut scope.parent, lane)
                    })),
                    operation,
                    panic,
                );
                let engine = settled_engine(&scope);
                let root = scope.release.as_ref().unwrap();
                assert!(root.resources.as_ref().unwrap().is_complete());
                dispatch.assert_ordinary_control_prefix_v1(root.dispatch.as_ref().unwrap(), index);
                engine.backend.session.auxiliary_assert_control_failure_v1(
                    &engine.foundation,
                    &t.borrow_mut().post_resources_snapshot.take().unwrap(),
                    &dispatch.order_v1(),
                    index,
                    step,
                );
                assert_eq!(
                    Memory::primary_token_identity(
                        scope.lanes[0]
                            .state
                            .as_ref()
                            .unwrap()
                            .completion_signals
                            .as_ref()
                            .unwrap()
                    ),
                    signal
                );
                assert!(root.signals.is_none() && !root.memory_complete);
                assert_no_retry(&mut scope, lane);
            }
        }
    }
}

#[test]
fn constructed_auxiliary_release_dispatch_data_failures_keep_exact_owners_and_refunds() {
    for index in [0usize, 1, 3] {
        for point in 0..3 {
            for panic in [false, true] {
                let (mut scope, lane, t) = constructed();
                let dispatch = RetainedControlSnapshotV1::ordinary_owner_v1(
                    scope.lanes[0]
                        .state
                        .as_ref()
                        .unwrap()
                        .dispatch
                        .as_ref()
                        .unwrap(),
                );
                let operation = match point {
                    0 => "unmap_gpu",
                    1 => "release_va_reservation",
                    _ => "currentness",
                };
                if point == 2 {
                    memory(&mut scope).primary_arm_data_currentness_v1(
                        index,
                        if index == 0 { 5 } else { 6 },
                        panic,
                    );
                } else {
                    memory(&mut scope).primary_arm_data_native_v1(index, operation, panic);
                }
                assert_native_failure(
                    catch_unwind(AssertUnwindSafe(|| {
                        release_in_place(&mut scope.parent, lane)
                    })),
                    operation,
                    panic,
                );
                let engine = settled_engine(&scope);
                let root = scope.release.as_ref().unwrap();
                assert!(root.resources.as_ref().unwrap().is_complete());
                let active =
                    dispatch.assert_ordinary_data_prefix_v1(root.dispatch.as_ref().unwrap(), index);
                assert!(active.started && active.failed && !active.complete);
                assert_eq!(active.native_disposed, point == 2);
                t.borrow_mut()
                    .post_resources_snapshot
                    .take()
                    .unwrap()
                    .assert_auxiliary_data_prefix_v1(
                        &engine.backend.session,
                        &engine.foundation,
                        &dispatch.order_v1(),
                        &dispatch.data_order_v1(),
                        index,
                        &active,
                    );
                assert!(root.signals.is_none() && !root.memory_complete);
                assert!(
                    scope.lanes[0]
                        .state
                        .as_ref()
                        .unwrap()
                        .completion_signals
                        .is_some()
                );
                assert_no_retry(&mut scope, lane);
            }
        }
    }
}

#[test]
fn constructed_auxiliary_release_signal_native_failures_keep_charge_and_receipts() {
    for (step, operation) in ["unmap_gpu", "unmap_cpu", "free", "release_va_reservation"]
        .into_iter()
        .enumerate()
    {
        for panic in [false, true] {
            let (mut scope, lane, t) = constructed();
            let signal = Memory::primary_token_identity(
                scope.lanes[0]
                    .state
                    .as_ref()
                    .unwrap()
                    .completion_signals
                    .as_ref()
                    .unwrap(),
            );
            t.borrow_mut().signal_fault = Some((operation, panic));
            assert_native_failure(
                catch_unwind(AssertUnwindSafe(|| {
                    release_in_place(&mut scope.parent, lane)
                })),
                operation,
                panic,
            );
            let engine = settled_engine(&scope);
            let root = scope.release.as_ref().unwrap();
            assert!(
                root.resources.as_ref().unwrap().is_complete()
                    && root.dispatch.as_ref().unwrap().is_complete()
            );
            assert_eq!(
                root.signals.as_ref().unwrap().observation().identity,
                signal
            );
            engine.backend.session.auxiliary_assert_control_failure_v1(
                &engine.foundation,
                &t.borrow_mut().signal_snapshot.take().unwrap(),
                &[signal],
                0,
                step,
            );
            assert!(!root.memory_complete && root.gate.is_some());
            assert_no_retry(&mut scope, lane);
        }
    }
}

#[test]
fn constructed_auxiliary_release_signal_currentness_keeps_unsettled_refund() {
    for point in 1..=6 {
        for panic in [false, true] {
            let (mut scope, lane, t) = constructed();
            t.borrow_mut().signal_currentness_fault = Some((point, panic));
            assert_native_failure(
                catch_unwind(AssertUnwindSafe(|| {
                    release_in_place(&mut scope.parent, lane)
                })),
                "currentness",
                panic,
            );
            let engine = settled_engine(&scope);
            let root = scope.release.as_ref().unwrap();
            assert!(
                root.resources.as_ref().unwrap().is_complete()
                    && root.dispatch.as_ref().unwrap().is_complete()
            );
            t.borrow_mut()
                .signal_snapshot
                .take()
                .unwrap()
                .assert_auxiliary_signal_currentness_v1(
                    &engine.backend.session,
                    &engine.foundation,
                    root.signals.as_ref().unwrap(),
                    point,
                );
            assert!(!root.memory_complete && root.gate.is_some());
            assert_no_retry(&mut scope, lane);
        }
    }
}

#[test]
fn constructed_auxiliary_release_cleanup_and_retake_failures_preserve_first_panic() {
    for cleanup_panic in [false, true] {
        for retake_panic in [false, true] {
            let (mut scope, lane, t) = constructed();
            t.borrow_mut().signal_fault = Some(("release_va_reservation", cleanup_panic));
            scope.parent.faults.reclaim_before = if retake_panic {
                Outcome::Panic
            } else {
                Outcome::Error
            };
            scope.parent.faults.poison_panic = true;
            let result = catch_unwind(AssertUnwindSafe(|| {
                release_in_place(&mut scope.parent, lane)
            }));
            if cleanup_panic {
                assert_eq!(
                    result.unwrap_err().downcast_ref::<(&str, &str)>(),
                    Some(&("N2 native panic", "release_va_reservation"))
                );
            } else if retake_panic {
                assert_eq!(
                    result.unwrap_err().downcast_ref::<(&str, usize)>(),
                    Some(&("auxiliary-release-retake", 1))
                );
            } else {
                assert!(matches!(
                    result.unwrap(),
                    Err(ComputeAqlQueueSessionErrorV1::Contract(
                        "auxiliary-release-retake"
                    ))
                ));
            }
            let engine = &scope.primary.completed.as_ref().unwrap().engine;
            assert!(!engine.backend.foundation_in_engine);
            let root = scope.release.as_ref().unwrap();
            assert!(
                root.resources.as_ref().unwrap().is_complete()
                    && root.dispatch.as_ref().unwrap().is_complete()
            );
            let signal = root.signals.as_ref().unwrap().observation();
            assert!(signal.started && signal.failed && !signal.native_disposed);
            assert!(!root.memory_complete && root.gate.is_some());
            assert_no_retry(&mut scope, lane);
        }
    }
}

#[test]
fn constructed_auxiliary_release_queue_commit_failures_keep_model_and_native_progress_distinct() {
    for index in [0, 3] {
        for stage in [Stage::UnmapCommit, Stage::ReleaseCommit] {
            for panic in [false, true] {
                let (mut scope, lane, t) = constructed();
                t.borrow_mut().queue_release_projection_fault = Some((index, stage, panic));
                let result = catch_unwind(AssertUnwindSafe(|| {
                    release_in_place(&mut scope.parent, lane)
                }));
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
                let engine = settled_engine(&scope);
                let root = scope.release.as_ref().unwrap();
                t.borrow_mut()
                    .release_snapshot
                    .take()
                    .unwrap()
                    .assert_auxiliary_queue_projection_v1(
                        &engine.backend.session,
                        &engine.foundation,
                        root.resources.as_ref().unwrap(),
                        index,
                        stage,
                    );
                assert!(root.dispatch.is_none() && root.signals.is_none() && !root.memory_complete);
                assert_no_retry(&mut scope, lane);
            }
        }
    }
}

#[test]
fn constructed_auxiliary_release_closing_currentness_failure_retains_complete_receipts() {
    for panic in [false, true] {
        let (mut scope, lane, t) = constructed();
        let occurrence = t
            .borrow()
            .calls
            .iter()
            .filter(|&&call| call == "currentness")
            .count()
            + 4;
        t.borrow_mut().fault = Some(("currentness", occurrence, panic));
        let result = catch_unwind(AssertUnwindSafe(|| {
            release_in_place(&mut scope.parent, lane)
        }));
        if panic {
            assert_eq!(
                result.unwrap_err().downcast_ref::<(&str, usize)>(),
                Some(&("currentness", occurrence))
            );
        } else {
            assert!(matches!(
                result.unwrap(),
                Err(ComputeAqlQueueSessionErrorV1::Native(
                    "queue currentness lost"
                ))
            ));
        }
        settled_engine(&scope);
        let root = scope.release.as_ref().unwrap();
        assert!(root.memory_complete && root.gate.is_some());
        assert!(
            root.resources.as_ref().unwrap().is_complete()
                && root.dispatch.as_ref().unwrap().is_complete()
                && root.signals.as_ref().unwrap().is_complete()
        );
        assert!(scope.lanes[0].state.is_some());
        assert_eq!(t.borrow().local_gate.as_ref().unwrap().teardown_count(), 1);
        assert_no_retry(&mut scope, lane);
    }
}
