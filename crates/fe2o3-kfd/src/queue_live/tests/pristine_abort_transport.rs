use super::super::super::rebind_tests::{assert_shell, parent};
use super::*;
use std::cell::{Cell, RefCell};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

fn selected_parent(
    ordinal: usize,
) -> (
    PristineAbortMemoryFixtureV1,
    ComputeAqlQueueSessionV1,
    ComputeAqlQueueLaneV1,
) {
    let (memory, owner) = pristine_dispatch_fixture_v1(8);
    let (mut session, mut lane) = parent(ordinal != 0, false);
    if ordinal == 2 {
        let state = session.auxiliary_compute_lanes[0].state.take();
        session
            .auxiliary_compute_lanes
            .push(AuxiliaryComputeLaneSlotV1 {
                generation: lane.generation,
                state,
            });
        lane.ordinal = 2;
    }
    session
        .with_compute_lane_v1(lane, |selected| {
            let s = &mut selected.session;
            s.detached_dispatch_generation = None;
            s.detached_data_count = 0;
            s.detached_data_identities.clear();
            s.detached_next_insertion_index = None;
            s.dispatch = Some(owner);
        })
        .unwrap();
    (memory, session, lane)
}

#[test]
fn abort_facade_restores_all_lanes_before_retaining_failed_control_parent() {
    for ordinal in 0..3 {
        for control in 1..=3 {
            for failure in 0..7 {
                for handling in 0..4 {
                    let (mut memory, mut session, lane) = selected_parent(ordinal);
                    match failure {
                        0 => memory.fail_control(control, "free", false),
                        1 => memory.fail_control(control, "free", true),
                        2 => memory.fail_control_commit(control, true, false),
                        5 => memory.fail_control(control, "unmap_gpu", false),
                        6 => memory.fail_control(control, "unmap_gpu", true),
                        _ => {}
                    }
                    let closing = if failure == 3 {
                        1
                    } else if failure == 4 {
                        2
                    } else {
                        0
                    };
                    let snapshot = |s: &ComputeAqlQueueSessionV1| {
                        (
                            (s.key, s.compute_lane_session, s.observation),
                            (
                                s.completion_owner.custody_snapshot_for_test(),
                                s.dependency_owner.custody_snapshot_for_test(),
                            ),
                            (
                                s.detached_data_count,
                                s.detached_dispatch_generation,
                                s.detached_next_insertion_index,
                                s.detached_data_identities.clone(),
                                s.detached_data_identities.as_ptr(),
                                s.detached_data_identities.capacity(),
                            ),
                            (
                                s.auxiliary_compute_lanes.as_ptr(),
                                s.auxiliary_compute_lanes.capacity(),
                                s.auxiliary_compute_lanes
                                    .iter()
                                    .map(|slot| {
                                        (
                                            slot.generation,
                                            slot.state.as_ref().map(|s| {
                                                (
                                                    s.key,
                                                    s.observation,
                                                    s.completion_owner.custody_snapshot_for_test(),
                                                    s.detached_data_count,
                                                    s.detached_dispatch_generation,
                                                    s.detached_next_insertion_index,
                                                    s.detached_data_identities.clone(),
                                                    s.detached_data_identities.as_ptr(),
                                                    s.detached_data_identities.capacity(),
                                                )
                                            }),
                                        )
                                    })
                                    .collect::<Vec<_>>(),
                            ),
                            (
                                s.sdma_device_pool.limits,
                                s.sdma_device_pool.activity_started,
                                s.sdma_host_pool_limits,
                                s.sdma_pool_reuse_count,
                            ),
                        )
                    };
                    let before = snapshot(&session);
                    let expected_abort = RefCell::new(None);
                    let initial_abort = RefCell::new(None);
                    let initial_records = RefCell::new(None);
                    let retained = RefCell::new(None);
                    let transfers = Cell::new(0);
                    let _ = take_dispatch_terminal_process_gate_record_v1();
                    let _ = take_lane_unwind_process_gate_record_v1();
                    let result = catch_unwind(AssertUnwindSafe(|| {
                        session.with_compute_lane_custody_v1(
                            lane,
                            |selected| {
                                let first = catch_unwind(AssertUnwindSafe(|| {
                                    selected.forward_pristine_abort_v1(|s| {
                                        let settled = s.settle_pristine_abort_with_v1(|s| {
                                            abort_with_observer(
                                                &mut memory,
                                                s,
                                                closing,
                                                |abort, memory| {
                                                    let before = abort.custody_snapshot_for_test();
                                                    let order =
                                                        std::iter::once(before.kernarg.unwrap())
                                                            .chain(
                                                                before.code.iter().rev().copied(),
                                                            )
                                                            .collect();
                                                    *initial_records.borrow_mut() =
                                                        Some(memory.control_record_snapshot(order));
                                                    *initial_abort.borrow_mut() = Some(before);
                                                },
                                            )
                                        });
                                        assert!(settled.transport);
                                        let abort = s
                                            .unpublished_dispatch
                                            .terminal_abort
                                            .as_ref()
                                            .unwrap()
                                            .custody_snapshot_for_test();
                                        let initial = initial_abort.borrow();
                                        let initial = initial.as_ref().unwrap();
                                        let complete = matches!(failure, 3 | 4);
                                        abort.assert_preserved_inputs(initial, complete);
                                        assert_eq!(abort.kernarg, None);
                                        assert_eq!(
                                            abort.code,
                                            initial.code[..if complete { 0 } else { 3 - control }]
                                        );
                                        if !complete {
                                            let active = abort
                                                .active
                                                .as_ref()
                                                .expect("failed active control remains retained");
                                            assert_eq!(
                                                active.owner,
                                                if failure >= 5 {
                                                    "Mapped"
                                                } else if failure == 2 {
                                                    "NativeDisposed"
                                                } else {
                                                    "Unmapped"
                                                }
                                            );
                                            assert_eq!(
                                                active.identity,
                                                if control == 1 {
                                                    initial.kernarg.unwrap()
                                                } else {
                                                    initial.code[3 - control]
                                                }
                                            );
                                            assert!(active.failed);
                                        } else {
                                            assert!(abort.active.is_none());
                                        }
                                        initial_records.borrow().as_ref().unwrap().assert_after(
                                            &memory,
                                            if complete {
                                                3
                                            } else {
                                                control - 1 + usize::from(failure == 2)
                                            },
                                            if complete { 3 } else { control },
                                        );
                                        *expected_abort.borrow_mut() = Some(abort);
                                        settled
                                    })
                                }));
                                assert!(
                                    *selected.terminal_transport,
                                    "transport must precede return or panic resumption"
                                );
                                assert_eq!(first.is_err(), matches!(failure, 1 | 4 | 6));
                                if let Err(payload) = &first {
                                    if matches!(failure, 1 | 6) {
                                        assert_eq!(
                                            payload.downcast_ref::<(&str, &str)>(),
                                            Some(&(
                                                "N2 native panic",
                                                if failure == 1 { "free" } else { "unmap_gpu" }
                                            ))
                                        );
                                    } else {
                                        assert_eq!(
                                            payload.downcast_ref::<&str>(),
                                            Some(&"closing retake panic")
                                        );
                                    }
                                } else {
                                    assert!(first.as_ref().unwrap().is_err());
                                }
                                let calls = memory.cleanup_call_count();
                                assert!(matches!(
                                    selected.abort_unpublished_fixed_dispatch_v1(),
                                    Err(ComputeAqlQueueSessionErrorV1::DispatchBinding(
                                        Gfx942DispatchBindingErrorV1::Poisoned
                                    ))
                                ));
                                assert!(
                                    *selected.terminal_transport,
                                    "poisoned retry cannot clear an earlier transfer"
                                );
                                assert_eq!(memory.cleanup_call_count(), calls);
                                match handling {
                                    0 => match first {
                                        Ok(result) => result.map(|_| 37),
                                        Err(payload) => std::panic::resume_unwind(payload),
                                    },
                                    1 => Ok(37),
                                    2 => {
                                        assert!(
                                            catch_unwind(|| panic!("caught caller panic")).is_err()
                                        );
                                        Ok(37)
                                    }
                                    _ => panic!("escaping caller panic"),
                                }
                            },
                            |root| {
                                transfers.set(transfers.get() + 1);
                                let parent = root.as_ref().as_ref().unwrap();
                                assert_eq!(
                                    snapshot(parent),
                                    before,
                                    "restore lane identity and storage before parent transfer"
                                );
                                assert!(parent.terminal_poisoned);
                                let abort = if ordinal == 0 {
                                    &parent.unpublished_dispatch
                                } else {
                                    &parent.auxiliary_compute_lanes[ordinal - 1]
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
                                    *expected_abort.borrow().as_ref().unwrap()
                                );
                                assert!(!abort.is_detached());
                                *retained.borrow_mut() = Some(root);
                            },
                        )
                    }));
                    let escaping = handling == 3 || handling == 0 && matches!(failure, 1 | 4 | 6);
                    assert_eq!(result.is_err(), escaping);
                    if !escaping {
                        assert_eq!(result.unwrap().unwrap().is_err(), handling == 0);
                    }
                    assert_eq!(transfers.get(), 1);
                    assert_eq!(take_lane_unwind_process_gate_record_v1(), escaping);
                    assert_shell(&mut session, lane);
                    assert_eq!(
                        snapshot(
                            retained
                                .borrow()
                                .as_ref()
                                .unwrap()
                                .as_ref()
                                .as_ref()
                                .unwrap()
                        ),
                        before
                    );
                    assert!(memory.data_is_retained());
                }
            }
        }
    }
}

#[test]
fn abort_public_facade_preflight_and_missing_engine_do_not_transfer_healthy_parent() {
    for ordinal in 0..3 {
        for malformed in [false, true] {
            let (memory, mut session, lane) = selected_parent(ordinal);
            let calls = memory.native_calls();
            let key = session.key;
            let result = session
                .with_compute_lane_custody_v1(
                    lane,
                    |selected| {
                        if malformed {
                            selected.session.detached_data_count = 1;
                        }
                        let result = selected.abort_unpublished_fixed_dispatch_v1();
                        assert!(!*selected.terminal_transport);
                        assert!(!selected.session.terminal_poisoned);
                        assert!(selected.session.dispatch.is_some());
                        assert!(selected.session.unpublished_dispatch.is_clear());
                        result
                    },
                    |_| panic!("healthy rejection cannot retain the parent"),
                )
                .unwrap();
            if malformed {
                assert!(matches!(
                    result,
                    Err(ComputeAqlQueueSessionErrorV1::DispatchBinding(
                        Gfx942DispatchBindingErrorV1::ResourcePhase
                    ))
                ));
            } else {
                assert!(matches!(
                    result,
                    Err(ComputeAqlQueueSessionErrorV1::Contract(
                        "missing queue engine"
                    ))
                ));
            }
            assert_eq!(session.key, key);
            assert!(!session.terminal_poisoned);
            assert_eq!(memory.native_calls(), calls);
        }
    }
}

struct SecondaryDrop {
    drops: Arc<AtomicUsize>,
    panic: bool,
}

fn direct_failure(failure: u8, control: usize, panic: bool) {
    let (mut memory, mut session, lane) = selected_parent(0);
    match failure {
        0 => memory.fail_control(control, "unmap_gpu", panic),
        1 | 4 => memory.fail_control(control, "free", panic),
        2 => memory.fail_control_commit(control, true, panic),
        _ => {}
    }
    let closing = if failure >= 3 {
        if panic { 2 } else { 1 }
    } else {
        0
    };
    let key = session.key;
    let observation = session.observation;
    let lane_session = session.compute_lane_session;
    let completion = session.completion_owner.custody_snapshot_for_test();
    let dependency = session.dependency_owner.custody_snapshot_for_test();
    let auxiliary_storage = (
        session.auxiliary_compute_lanes.as_ptr(),
        session.auxiliary_compute_lanes.capacity(),
    );
    let auxiliary = session
        .auxiliary_compute_lanes
        .iter()
        .map(|slot| {
            (
                slot.generation,
                slot.state.as_ref().map(|s| {
                    (
                        s.key,
                        s.observation,
                        s.completion_owner.custody_snapshot_for_test(),
                    )
                }),
            )
        })
        .collect::<Vec<_>>();
    let mut initial_abort = None;
    let mut initial_records = None;
    let settled = session.settle_pristine_abort_with_v1(|s| {
        abort_with_observer(&mut memory, s, closing, |abort, memory| {
            let before = abort.custody_snapshot_for_test();
            let order = std::iter::once(before.kernarg.unwrap())
                .chain(before.code.iter().rev().copied())
                .collect();
            initial_records = Some(memory.control_record_snapshot(order));
            initial_abort = Some(before);
        })
    });
    assert!(settled.transport);
    let payload_pointer = settled
        .result
        .as_ref()
        .err()
        .map(|p| p.as_ref() as *const _ as *const () as usize);
    let expected_abort = session
        .unpublished_dispatch
        .terminal_abort
        .as_ref()
        .unwrap()
        .custody_snapshot_for_test();
    let complete = failure == 3;
    expected_abort.assert_preserved_inputs(initial_abort.as_ref().unwrap(), complete);
    let calls = memory.cleanup_call_count();
    let mut retained = None;
    let transfers = Cell::new(0);
    let result = catch_unwind(AssertUnwindSafe(|| {
        session.finish_pristine_abort_v1(settled, |root| {
            transfers.set(transfers.get() + 1);
            let parent = root.as_ref().as_ref().unwrap();
            assert!(parent.terminal_poisoned);
            assert_eq!(
                (parent.key, parent.observation, parent.compute_lane_session),
                (key, observation, lane_session)
            );
            assert_eq!(
                parent.completion_owner.custody_snapshot_for_test(),
                completion
            );
            assert_eq!(
                parent.dependency_owner.custody_snapshot_for_test(),
                dependency
            );
            assert_eq!(
                (
                    parent.auxiliary_compute_lanes.as_ptr(),
                    parent.auxiliary_compute_lanes.capacity()
                ),
                auxiliary_storage
            );
            assert_eq!(
                parent
                    .auxiliary_compute_lanes
                    .iter()
                    .map(|slot| {
                        (
                            slot.generation,
                            slot.state.as_ref().map(|s| {
                                (
                                    s.key,
                                    s.observation,
                                    s.completion_owner.custody_snapshot_for_test(),
                                )
                            }),
                        )
                    })
                    .collect::<Vec<_>>(),
                auxiliary
            );
            assert_eq!(
                parent
                    .unpublished_dispatch
                    .terminal_abort
                    .as_ref()
                    .unwrap()
                    .custody_snapshot_for_test(),
                expected_abort
            );
            assert!(!parent.unpublished_dispatch.is_detached());
            retained = Some(root);
        })
    }));
    assert_eq!(
        transfers.get(),
        1,
        "direct parent must be retained before return or panic resumption"
    );
    assert_eq!(memory.cleanup_call_count(), calls);
    if panic {
        let payload = result.unwrap_err();
        assert_eq!(
            Some(payload.as_ref() as *const _ as *const () as usize),
            payload_pointer
        );
        match failure {
            0 | 1 | 4 => assert_eq!(
                payload.downcast_ref::<(&str, &str)>(),
                Some(&(
                    "N2 native panic",
                    if failure == 0 { "unmap_gpu" } else { "free" }
                ))
            ),
            2 => assert_eq!(
                payload.downcast_ref::<(&str, crate::shared_memory::CleanupStageV1)>(),
                Some(&(
                    "control cleanup projection",
                    crate::shared_memory::CleanupStageV1::ReleaseCommit
                ))
            ),
            _ => assert_eq!(
                payload.downcast_ref::<&str>(),
                Some(&"closing retake panic")
            ),
        }
    } else if failure == 3 {
        assert!(matches!(
            result.unwrap(),
            Err(ComputeAqlQueueSessionErrorV1::Contract(
                "closing retake failed"
            ))
        ));
    } else {
        let expected = match failure {
            0 => "unmap_gpu",
            1 => "free",
            _ => "control cleanup projection",
        };
        assert!(
            matches!(result.unwrap(), Err(ComputeAqlQueueSessionErrorV1::DispatchBinding(Gfx942DispatchBindingErrorV1::Memory(MemorySessionError::Injected(op)))) if op == expected)
        );
    }
    initial_records.as_ref().unwrap().assert_after(
        &memory,
        if complete {
            3
        } else {
            control - 1 + usize::from(failure == 2)
        },
        if complete { 3 } else { control },
    );
    assert_shell(&mut session, lane);
    assert_eq!(
        retained
            .as_ref()
            .unwrap()
            .as_ref()
            .as_ref()
            .unwrap()
            .unpublished_dispatch
            .terminal_abort
            .as_ref()
            .unwrap()
            .custody_snapshot_for_test(),
        expected_abort
    );
    assert_eq!(memory.cleanup_call_count(), calls);
    assert!(memory.data_is_retained());
}

#[test]
fn abort_direct_finisher_retains_exact_parent_before_returned_error() {
    for control in 1..=3 {
        for failure in 0..4 {
            direct_failure(failure, control, false);
        }
    }
}

#[test]
fn abort_direct_finisher_retains_exact_parent_before_original_panic() {
    for control in 1..=3 {
        for failure in 0..5 {
            direct_failure(failure, control, true);
        }
    }
}

#[test]
fn abort_direct_finisher_returns_success_without_transport() {
    let (mut memory, mut session, _) = selected_parent(0);
    let usage = memory.usage();
    let settled = session.settle_pristine_abort_with_v1(|s| abort_with(&mut memory, s, 0));
    assert!(!settled.transport);
    let calls = memory.cleanup_call_count();
    let data = session
        .finish_pristine_abort_v1(settled, |_| panic!("success cannot transport parent"))
        .unwrap();
    assert_eq!(data.len(), 5);
    assert_eq!(
        session.detached_data_identities,
        fixed_dispatch_storage_identities(&data)
    );
    assert_eq!(session.detached_data_count, data.len());
    assert!(session.unpublished_dispatch.is_detached());
    assert_eq!(
        session
            .unpublished_dispatch
            .continuation
            .as_ref()
            .unwrap()
            .next_generation_for_test(),
        8
    );
    assert!(!session.terminal_poisoned);
    assert!(session.dispatch.is_none());
    assert_eq!(memory.disposed_controls(), 3);
    assert_eq!(memory.cleanup_call_count(), calls);
    assert_eq!(memory.usage(), usage);
    assert!(memory.data_is_retained());
}

#[test]
fn abort_direct_rejection_and_terminal_retry_do_not_transport_parent() {
    for case in 0..4 {
        let (mut memory, mut session, _) = selected_parent(0);
        if case == 1 {
            session.detached_data_count = 1;
        } else if case == 3 {
            memory.fail_control(2, "free", false);
            let first = session.settle_pristine_abort_with_v1(|s| abort_with(&mut memory, s, 0));
            assert!(first.transport && first.result.unwrap().is_err());
        }
        let key = session.key;
        let completion = session.completion_owner.custody_snapshot_for_test();
        let before_abort = session
            .unpublished_dispatch
            .terminal_abort
            .as_ref()
            .map(|a| a.custody_snapshot_for_test());
        let calls = memory.native_calls();
        let cleanup_calls = memory.cleanup_call_count();
        let settled = if case == 0 {
            session.settle_pristine_abort_with_v1(|s| abort_with(&mut memory, s, 3))
        } else {
            session.abort_unpublished_settled_v1()
        };
        assert!(!settled.transport);
        let result = session
            .finish_pristine_abort_v1(settled, |_| panic!("rejected call cannot transport parent"));
        match case {
            0 => assert!(matches!(
                result,
                Err(ComputeAqlQueueSessionErrorV1::Contract("loan rejected"))
            )),
            1 => assert!(matches!(
                result,
                Err(ComputeAqlQueueSessionErrorV1::DispatchBinding(
                    Gfx942DispatchBindingErrorV1::ResourcePhase
                ))
            )),
            2 => assert!(matches!(
                result,
                Err(ComputeAqlQueueSessionErrorV1::Contract(
                    "missing queue engine"
                ))
            )),
            _ => assert!(matches!(
                result,
                Err(ComputeAqlQueueSessionErrorV1::DispatchBinding(
                    Gfx942DispatchBindingErrorV1::Poisoned
                ))
            )),
        }
        assert!(session.abort_unpublished_fixed_dispatch_v1().is_err());
        assert_eq!(session.key, key);
        assert_eq!(
            session.completion_owner.custody_snapshot_for_test(),
            completion
        );
        assert_eq!(session.terminal_poisoned, case == 3);
        assert_eq!(session.dispatch.is_some(), case != 3);
        assert_eq!(
            session
                .unpublished_dispatch
                .terminal_abort
                .as_ref()
                .map(|a| a.custody_snapshot_for_test()),
            before_abort
        );
        assert_eq!(memory.native_calls(), calls);
        assert_eq!(memory.cleanup_call_count(), cleanup_calls);
        assert!(memory.data_is_retained());
    }
}

impl Drop for SecondaryDrop {
    fn drop(&mut self) {
        self.drops.fetch_add(1, Ordering::SeqCst);
        assert!(!self.panic, "secondary destructor must never execute");
    }
}

fn secondary_payload(panic: bool) {
    let (mut memory, mut session) = fixture();
    memory.fail("free", true);
    let drops = Arc::new(AtomicUsize::new(0));
    let primary_pointer = Cell::new(0usize);
    let poisoned = Cell::new(false);
    let result = catch_unwind(AssertUnwindSafe(|| {
        session.abort_unpublished_with_v1(
            |_, buffers, dispatch, abort, cleanup| {
                *abort = Some(dispatch.take().unwrap().begin_pristine_abort_v1(buffers));
                let result = catch_unwind(AssertUnwindSafe(|| {
                    abort.as_mut().unwrap().release_controls(&mut memory)
                }));
                primary_pointer
                    .set(result.as_ref().unwrap_err().as_ref() as *const _ as *const () as usize);
                *cleanup = Some(result);
                std::panic::panic_any(SecondaryDrop {
                    drops: drops.clone(),
                    panic,
                });
            },
            || poisoned.set(true),
        )
    }));
    let payload = result.unwrap_err();
    assert_eq!(
        payload.as_ref() as *const _ as *const () as usize,
        primary_pointer.get()
    );
    assert_eq!(
        payload.downcast_ref::<(&str, &str)>(),
        Some(&("N2 native panic", "free"))
    );
    assert_eq!(
        drops.load(Ordering::SeqCst),
        0,
        "secondary retake payload was destroyed"
    );
    assert!(poisoned.get() && session.terminal_poisoned);
    let root = session
        .unpublished_dispatch
        .terminal_abort
        .as_ref()
        .unwrap()
        .custody_snapshot_for_test();
    assert_eq!(root.active.as_ref().unwrap().owner, "Unmapped");
    assert!(memory.data_is_retained());
}

#[test]
fn abort_cleanup_panic_keeps_secondary_payload_destructor_inert() {
    secondary_payload(false);
}

#[test]
fn abort_cleanup_panic_survives_panicking_secondary_destructor() {
    secondary_payload(true);
}

#[test]
fn abort_public_paths_use_settled_transport_before_return_or_resume() {
    let facade = include_str!("../../queue_live.rs")
        .split("pub fn abort_unpublished_fixed_dispatch_v1(")
        .nth(1)
        .unwrap()
        .split("pub fn release_retained_persistent_fixed_dispatch_control_v1(")
        .next()
        .unwrap();
    assert!(facade.contains(
        "self.forward_pristine_abort_v1(ComputeAqlQueueSessionV1::abort_unpublished_settled_v1)"
    ));
    let direct = include_str!("../pristine_abort.rs")
        .split("pub fn abort_unpublished_fixed_dispatch_v1(")
        .nth(1)
        .unwrap()
        .split("pub(super) fn abort_unpublished_settled_v1(")
        .next()
        .unwrap();
    assert!(direct.contains("self.finish_pristine_abort_v1(settled, core::mem::forget)"));
    let finishing = direct.split("fn finish_pristine_abort_v1(").nth(1).unwrap();
    assert!(
        finishing.find("retain_terminal_rebind_parent_v1").unwrap()
            < finishing.find("settled.into_result()").unwrap()
    );
}
