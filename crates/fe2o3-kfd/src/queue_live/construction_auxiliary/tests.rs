use super::*;
use crate::queue::live::tests::{
    compute_lane_state_for_multi_inflight_test, persistent_compute_cancellation_test_session,
    test_queue_key,
};
use crate::shared_memory::QueueConstructionMemoryFixtureV1;
use std::cell::{Cell, RefCell};
use std::panic::{AssertUnwindSafe, catch_unwind};

#[test]
fn auxiliary_production_glue_uses_shared_phases_and_original_target_after_retake() {
    let source = include_str!("../construction_auxiliary.rs");
    let wrapper = source
        .split("pub(super) fn construct_auxiliary_compute_lane_v1")
        .nth(1)
        .unwrap()
        .split("fn settle_auxiliary_construction_with_v1")
        .next()
        .unwrap();
    let loan = wrapper
        .find("with_live_queue_memory_model_custody(")
        .unwrap();
    let preparation = wrapper.find("root.prepare_dispatch(").unwrap();
    let retake = wrapper.find("retake?;").unwrap();
    let result = wrapper.find("result?;").unwrap();
    let create = wrapper.find("root.create_and_install(").unwrap();
    assert!(loan < preparation && preparation < retake && retake < result && result < create);
    for binding in [
        "engine: scope.parent.engine.as_mut().expect(\"checked queue engine\")",
        "primary: &scope.parent.observation",
        "lanes: &mut scope.parent.auxiliary_compute_lanes",
        "sdma: scope.parent.sdma.as_ref()",
        "striped_sdma: scope.parent.striped_sdma.as_ref()",
    ] {
        assert!(
            wrapper.contains(binding),
            "original target binding: {binding}"
        );
    }
    let creation = source
        .split("pub(super) fn create_and_install(")
        .nth(1)
        .unwrap()
        .split("pub(super) fn construct_auxiliary_compute_lane_v1")
        .next()
        .unwrap();
    assert_eq!(
        creation
            .matches("parent.engine.prepare_operation()")
            .count(),
        2
    );
    let vacancy = creation
        .find("check_auxiliary_compute_lane_slot_v1(")
        .unwrap();
    let finish = creation.find("E::finish_creation(").unwrap();
    let install = creation
        .find("install_auxiliary_compute_lane_slot_v1(")
        .unwrap();
    assert!(vacancy < finish && finish < install);
}

struct DropProbe {
    identity: usize,
    drops: Rc<Cell<usize>>,
}

impl Drop for DropProbe {
    fn drop(&mut self) {
        self.drops.set(self.drops.get() + 1);
    }
}

#[test]
fn auxiliary_vacancy_rejects_drift_without_consuming_or_overwriting_owners() {
    for case in 0..11 {
        let drops = Rc::new(Cell::new(0));
        let completed = Some(DropProbe {
            identity: 42,
            drops: drops.clone(),
        });
        let mut slots = Vec::with_capacity(1);
        let mut plan = PreparedAuxiliaryComputeLaneSlotV1 {
            index: 0,
            generation: 1,
            append: true,
        };
        match case {
            0 => slots = Vec::new(),
            1 => plan.index = 1,
            2 => plan.generation = 2,
            3 => plan.append = false,
            4 => {
                slots.push(AuxiliaryComputeLaneSlotV1 {
                    generation: 4,
                    state: Some(DropProbe {
                        identity: 7,
                        drops: drops.clone(),
                    }),
                });
                plan = PreparedAuxiliaryComputeLaneSlotV1 {
                    index: 0,
                    generation: 5,
                    append: false,
                };
            }
            5 => {
                slots.push(AuxiliaryComputeLaneSlotV1 {
                    generation: 4,
                    state: None,
                });
                plan = PreparedAuxiliaryComputeLaneSlotV1 {
                    index: 0,
                    generation: 6,
                    append: false,
                };
            }
            6 => {
                slots.push(AuxiliaryComputeLaneSlotV1 {
                    generation: u64::MAX,
                    state: None,
                });
                plan = PreparedAuxiliaryComputeLaneSlotV1 {
                    index: 0,
                    generation: 0,
                    append: false,
                };
            }
            7 => {
                slots.push(AuxiliaryComputeLaneSlotV1 {
                    generation: 0,
                    state: None,
                });
                plan.append = false;
            }
            8 => {
                slots.push(AuxiliaryComputeLaneSlotV1 {
                    generation: 1,
                    state: None,
                });
                slots.push(AuxiliaryComputeLaneSlotV1 {
                    generation: 1,
                    state: None,
                });
                plan.generation = 2;
                plan.append = false;
            }
            9 => {
                let destination = check_auxiliary_compute_lane_slot_v1(&mut slots, plan).unwrap();
                install_auxiliary_compute_lane_slot_v1(
                    destination,
                    DropProbe {
                        identity: 7,
                        drops: drops.clone(),
                    },
                );
            }
            10 => {
                slots.push(AuxiliaryComputeLaneSlotV1 {
                    generation: 4,
                    state: None,
                });
                plan = PreparedAuxiliaryComputeLaneSlotV1 {
                    index: 1,
                    generation: 5,
                    append: false,
                };
            }
            _ => unreachable!(),
        }
        let snapshot = |slots: &[AuxiliaryComputeLaneSlotV1<DropProbe>]| {
            slots
                .iter()
                .map(|s| (s.generation, s.state.as_ref().map(|s| s.identity)))
                .collect::<Vec<_>>()
        };
        let before = snapshot(&slots);
        let storage = slots.as_ptr();
        let capacity = slots.capacity();
        assert!(
            check_auxiliary_compute_lane_slot_v1(&mut slots, plan).is_err(),
            "case {case}"
        );
        assert_eq!(snapshot(&slots), before, "case {case}");
        assert_eq!(slots.as_ptr(), storage);
        assert_eq!(slots.capacity(), capacity);
        assert_eq!(completed.as_ref().unwrap().identity, 42);
        assert_eq!(drops.get(), 0, "case {case}");
    }
}

#[test]
fn auxiliary_vacancy_success_moves_once_without_vector_growth() {
    let drops = Rc::new(Cell::new(0));
    let mut slots = Vec::with_capacity(1);
    let storage = slots.as_ptr();
    let capacity = slots.capacity();
    for identity in 1..=3 {
        let plan = prepare_auxiliary_compute_lane_slot_v1(&slots).unwrap();
        assert_eq!(plan.generation, identity as u64);
        let mut owner = Some(DropProbe {
            identity,
            drops: drops.clone(),
        });
        let destination = check_auxiliary_compute_lane_slot_v1(&mut slots, plan).unwrap();
        install_auxiliary_compute_lane_slot_v1(destination, owner.take().unwrap());
        assert!(owner.is_none());
        assert_eq!(slots[0].state.as_ref().unwrap().identity, identity);
        assert_eq!(slots.as_ptr(), storage);
        assert_eq!(slots.capacity(), capacity);
        assert_eq!(drops.get(), identity - 1);
        assert!(check_auxiliary_compute_lane_slot_v1(&mut slots, plan).is_err());
        drop(slots[0].state.take().unwrap());
        assert_eq!(drops.get(), identity);
    }
}

fn parent_fixture() -> ComputeAqlQueueSessionV1 {
    let queue = test_queue_key(410, 7);
    let mut parent = persistent_compute_cancellation_test_session(queue, None, None);
    parent.completion_owner.bind_barrier_probe().unwrap();
    parent.dependency_owner.reserve_acceptance_epoch().unwrap();
    parent.dependency_owner.reserve_acceptance_epoch().unwrap();
    let mut lane = compute_lane_state_for_multi_inflight_test(test_queue_key(411, 8));
    lane.completion_owner.bind_barrier_probe().unwrap();
    parent
        .auxiliary_compute_lanes
        .push(AuxiliaryComputeLaneSlotV1 {
            generation: 9,
            state: Some(lane),
        });
    parent.sdma_device_pool.limits = Some(Gfx942DevicePoolLimitsV1::new(1 << 20, 16).unwrap());
    parent.sdma_device_pool.activity_started = true;
    parent.sdma_host_pool_limits = Some(Gfx942HostPoolLimitsV1::new(1 << 20, 16).unwrap());
    parent.sdma_pool_reuse_count = 31;
    parent.detached_dispatch_generation = Some(29);
    parent
}

fn assert_terminal_shell(parent: &mut ComputeAqlQueueSessionV1) {
    assert!(parent.terminal_poisoned);
    assert!(parent.engine.is_none());
    assert!(parent.completion_owner.0.is_none());
    assert!(parent.dependency_owner.0.is_none());
    parent.poison_terminal();
    parent.poison_terminal();
    let primary = parent.primary_compute_lane_v1();
    let auxiliary = ComputeAqlQueueLaneV1 {
        session: primary.session,
        ordinal: 1,
        generation: 9,
    };
    for lane in [primary, auxiliary] {
        assert!(
            parent
                .with_compute_lane_v1(lane, |_| panic!("terminal lane callback"))
                .is_err()
        );
        assert!(matches!(
            parent.destroy_auxiliary_compute_lane_v1(lane),
            Err(ComputeAqlQueueSessionErrorV1::DispatchBinding(
                Gfx942DispatchBindingErrorV1::Poisoned
            ))
        ));
        assert!(
            parent
                .submit_fixed_dispatch_with_dependencies_v1(lane, Vec::new())
                .is_err()
        );
    }
    assert!(parent.abort_unpublished_fixed_dispatch_v1().is_err());
    assert!(parent.sdma_device_pool_usage_v1().is_err());
    assert!(parent.sdma_host_pool_usage_v1().is_err());
    assert!(
        parent
            .create_auxiliary_compute_lane_with_fixed_dispatch(4096, Vec::new(), [], |_| panic!(
                "terminal preparation callback"
            ))
            .is_err()
    );
    assert_eq!(parent.auxiliary_compute_lane_count_v1(), 0);
}

fn assert_retained_parent_poisoned(parent: &ComputeAqlQueueSessionV1) {
    assert!(parent.terminal_poisoned, "original parent terminal");
    assert!(matches!(
        parent.completion_owner.ensure_releasable(),
        Err(Gfx942CompletionErrorV1::Poisoned)
    ));
    assert!(matches!(
        parent.dependency_owner.ensure_idle(),
        Err(ComputeDependencyTargetUseErrorV1::Poisoned)
    ));
}

#[test]
fn auxiliary_opening_is_rooted_before_preparation_loan_and_retake() {
    let public = include_str!("../../queue_live.rs")
        .split("pub fn create_auxiliary_compute_lane_with_fixed_dispatch")
        .nth(1)
        .unwrap()
        .split("pub fn auxiliary_compute_lane_count_v1")
        .next()
        .unwrap();
    assert!(!public.contains("self.check_currentness()"));
    assert!(public.contains("engine.preflight_operation()"));
    let construction = include_str!("../construction_auxiliary.rs");
    assert!(construction.contains("ComputeAqlQueueSessionV1::check_currentness,"));

    for opening in 0..3 {
        let mut parent = parent_fixture();
        let completion = parent.completion_owner.custody_snapshot_for_test();
        let dependency = parent.dependency_owner.custody_snapshot_for_test();
        let scope = Box::new(AuxiliaryConstructionScopeV1 {
            terminal_parent: None,
            parent: &mut parent,
            construction: AuxiliaryConstructionV1::new([]),
        });
        let identity = &*scope as *const _;
        let retained = RefCell::new(None);
        let trace = RefCell::new(Vec::new());
        let result = catch_unwind(AssertUnwindSafe(|| {
            settle_auxiliary_construction_with_v1(
                scope,
                |parent| {
                    trace.borrow_mut().push("opening");
                    match opening {
                        0 => Ok(()),
                        1 => parent.check_currentness(),
                        _ => std::panic::panic_any("auxiliary opening panic"),
                    }
                },
                |_, _| {
                    let (result, retake) = execute_live_model_custody_v1(
                        &mut (),
                        |_| {
                            trace.borrow_mut().push("loan");
                            Ok::<_, ComputeAqlQueueSessionErrorV1>(())
                        },
                        |_| {
                            trace.borrow_mut().push("preparation");
                            Ok::<_, ComputeAqlQueueSessionErrorV1>(())
                        },
                        |_, ()| {
                            trace.borrow_mut().push("retake");
                            Ok(())
                        },
                        |_| panic!("unexpected loan poison"),
                    )?;
                    retake?;
                    result
                },
                &|| panic!("pre-USERPTR process poison"),
                |scope| {
                    assert_eq!(&*scope as *const _, identity);
                    let parent = scope.terminal_parent.as_ref().unwrap();
                    assert_retained_parent_poisoned(parent);
                    assert_eq!(
                        parent.completion_owner.custody_snapshot_for_test(),
                        completion
                    );
                    assert_eq!(
                        parent.dependency_owner.custody_snapshot_for_test(),
                        dependency
                    );
                    *retained.borrow_mut() = Some(scope);
                },
            )
        }));
        if opening == 0 {
            let scope = result.unwrap().unwrap();
            assert_eq!(
                &*trace.borrow(),
                &["opening", "loan", "preparation", "retake"]
            );
            assert!(retained.borrow().is_none());
            assert!(scope.terminal_parent.is_none());
            assert!(!scope.parent.terminal_poisoned);
            assert_eq!(
                scope.parent.completion_owner.custody_snapshot_for_test(),
                completion
            );
            assert_eq!(
                scope.parent.dependency_owner.custody_snapshot_for_test(),
                dependency
            );
        } else {
            if opening == 1 {
                assert!(matches!(
                    result.unwrap(),
                    Err(ComputeAqlQueueSessionErrorV1::Contract(
                        "missing queue engine"
                    ))
                ));
            } else {
                assert_eq!(
                    result.err().unwrap().downcast_ref::<&str>(),
                    Some(&"auxiliary opening panic")
                );
            }
            assert_eq!(&*trace.borrow(), &["opening"]);
            let scope = retained
                .into_inner()
                .expect("opening failure retained scope");
            assert_terminal_shell(scope.parent);
            assert!(scope.construction.packets.is_some());
            assert!(scope.construction.data.is_none());
            assert!(scope.construction.preparation.is_none());
            assert!(scope.construction.dispatch.is_none());
            assert!(scope.construction.ring.is_none());
            assert!(scope.construction.control.cpu.is_none());
            assert!(scope.construction.completion.cpu.is_none());
            assert!(scope.construction.completed.is_none());
        }
    }
}

#[test]
fn auxiliary_terminal_transfer_preserves_exact_ledgers_after_caller_drop() {
    let mut parent = parent_fixture();
    let completion = parent.completion_owner.custody_snapshot_for_test();
    let dependency = parent.dependency_owner.custody_snapshot_for_test();
    let auxiliary = parent.auxiliary_compute_lanes[0]
        .state
        .as_ref()
        .unwrap()
        .completion_owner
        .custody_snapshot_for_test();
    let roster = parent.auxiliary_compute_lanes.as_ptr();
    let observation = parent.observation;
    let mut retained = parent.take_for_terminal_auxiliary_construction_v1();
    assert_retained_parent_poisoned(&retained);
    assert_terminal_shell(&mut parent);
    assert_eq!(parent.observation, observation);
    drop(parent);
    assert_eq!(
        retained.completion_owner.custody_snapshot_for_test(),
        completion
    );
    assert_eq!(
        retained.dependency_owner.custody_snapshot_for_test(),
        dependency
    );
    assert_eq!(retained.auxiliary_compute_lanes.as_ptr(), roster);
    assert_eq!(retained.auxiliary_compute_lanes[0].generation, 9);
    assert_eq!(
        retained.auxiliary_compute_lanes[0]
            .state
            .as_ref()
            .unwrap()
            .completion_owner
            .custody_snapshot_for_test(),
        auxiliary
    );
    assert_eq!(retained.sdma_pool_reuse_count, 31);
    assert_eq!(retained.detached_dispatch_generation, Some(29));
    retained.poison_terminal();
    assert_eq!(
        retained.completion_owner.custody_snapshot_for_test(),
        completion
    );
}

#[test]
fn auxiliary_settlement_retains_completed_lane_and_original_scope_on_error_or_panic() {
    for panics in [false, true] {
        let mut parent = parent_fixture();
        let before = parent.completion_owner.custody_snapshot_for_test();
        let mut construction = AuxiliaryConstructionV1::new([]);
        construction.completed = Some(compute_lane_state_for_multi_inflight_test(test_queue_key(
            412, 1,
        )));
        let completed_before = construction
            .completed
            .as_ref()
            .unwrap()
            .completion_owner
            .custody_snapshot_for_test();
        let scope = Box::new(AuxiliaryConstructionScopeV1 {
            terminal_parent: None,
            parent: &mut parent,
            construction,
        });
        let identity = &*scope as *const _;
        let retained = RefCell::new(None);
        let poisons = Cell::new(0);
        let result = catch_unwind(AssertUnwindSafe(|| {
            settle_auxiliary_construction_with_v1(
                scope,
                |_| Ok(()),
                |scope, entry| {
                    entry.enter("USERPTR auxiliary queue-control creation");
                    let plan = PreparedAuxiliaryComputeLaneSlotV1 {
                        index: 0,
                        generation: 10,
                        append: false,
                    };
                    assert!(
                        check_auxiliary_compute_lane_slot_v1(
                            &mut scope.parent.auxiliary_compute_lanes,
                            plan
                        )
                        .is_err()
                    );
                    if panics {
                        std::panic::panic_any("late auxiliary panic");
                    }
                    Err(ComputeAqlQueueSessionErrorV1::Contract(
                        "late auxiliary error",
                    ))
                },
                &|| poisons.set(poisons.get() + 1),
                |scope| *retained.borrow_mut() = Some(scope),
            )
        }));
        if panics {
            assert_eq!(
                result.err().unwrap().downcast_ref::<&str>(),
                Some(&"late auxiliary panic")
            );
        } else {
            assert!(result.unwrap().err().unwrap().is_terminal_creation());
        }
        assert_eq!(poisons.get(), 1);
        let scope = retained.into_inner().unwrap();
        assert_retained_parent_poisoned(scope.terminal_parent.as_ref().unwrap());
        assert_eq!(&*scope as *const _, identity);
        assert_eq!(
            scope
                .construction
                .completed
                .as_ref()
                .expect("completed auxiliary lane retained")
                .completion_owner
                .custody_snapshot_for_test(),
            completed_before
        );
        assert_eq!(
            scope
                .terminal_parent
                .as_ref()
                .unwrap()
                .completion_owner
                .custody_snapshot_for_test(),
            before
        );
        assert_terminal_shell(scope.parent);
    }
}

#[test]
fn auxiliary_returned_completion_survives_composed_operation_and_retake_matrix() {
    for operation in 0..3 {
        for retake in 0..3 {
            let mut parent = parent_fixture();
            let parent_before = parent.completion_owner.custody_snapshot_for_test();
            let mut memory = QueueConstructionMemoryFixtureV1::new();
            let scope = Box::new(AuxiliaryConstructionScopeV1 {
                terminal_parent: None,
                parent: &mut parent,
                construction: AuxiliaryConstructionV1::new([]),
            });
            let retained = RefCell::new(None);
            let token_identity = Cell::new(None);
            let token_usage = Cell::new(None);
            let trace = RefCell::new(Vec::new());
            let result = catch_unwind(AssertUnwindSafe(|| {
                settle_auxiliary_construction_with_v1(
                    scope,
                    |_| Ok(()),
                    |scope, entry| {
                        entry.enter("USERPTR auxiliary queue-control creation");
                        let (outcome, closed) = execute_live_model_custody_v1(
                            &mut (),
                            |_| {
                                trace.borrow_mut().push("open");
                                Ok::<_, ComputeAqlQueueSessionErrorV1>(())
                            },
                            |_| {
                                trace.borrow_mut().push("operation");
                                let token = memory.allocate::<HostVisibleCoherentGttV1>(
                                    COMPLETION_SIGNAL_ARENA_BYTES_V1,
                                );
                                token_identity.set(Some(token.storage_identity()));
                                token_usage.set(Some(memory.host_usage()));
                                scope.construction.completion.cpu = Some(token);
                                match operation {
                                    0 => Ok(()),
                                    1 => Err(ComputeAqlQueueSessionErrorV1::Contract(
                                        "operation error",
                                    )),
                                    _ => std::panic::panic_any("operation panic"),
                                }
                            },
                            |_, ()| {
                                trace.borrow_mut().push("retake");
                                match retake {
                                    0 => Ok(()),
                                    1 => {
                                        Err(ComputeAqlQueueSessionErrorV1::Contract("retake error"))
                                    }
                                    _ => std::panic::panic_any("retake panic"),
                                }
                            },
                            |_| trace.borrow_mut().push("loan poison"),
                        )?;
                        closed?;
                        outcome?;
                        Ok(())
                    },
                    &|| {},
                    |scope| *retained.borrow_mut() = Some(scope),
                )
            }));
            assert_eq!(&trace.borrow()[..3], ["open", "operation", "retake"]);
            let scope = if operation == 0 && retake == 0 {
                let scope = result.unwrap().unwrap();
                assert!(retained.borrow().is_none());
                assert!(scope.terminal_parent.is_none());
                assert_eq!(
                    scope.parent.completion_owner.custody_snapshot_for_test(),
                    parent_before
                );
                scope
            } else {
                if operation == 2 || retake == 2 {
                    assert_eq!(
                        result.err().unwrap().downcast_ref::<&str>(),
                        Some(&if operation == 2 {
                            "operation panic"
                        } else {
                            "retake panic"
                        })
                    );
                } else {
                    assert!(result.unwrap().err().unwrap().is_terminal_creation());
                }
                let scope = retained.into_inner().unwrap();
                assert_retained_parent_poisoned(scope.terminal_parent.as_ref().unwrap());
                assert_eq!(
                    scope
                        .terminal_parent
                        .as_ref()
                        .unwrap()
                        .completion_owner
                        .custody_snapshot_for_test(),
                    parent_before
                );
                assert_terminal_shell(scope.parent);
                scope
            };
            assert_eq!(
                Some(
                    scope
                        .construction
                        .completion
                        .cpu
                        .as_ref()
                        .unwrap()
                        .storage_identity()
                ),
                token_identity.get()
            );
            assert_eq!(Some(memory.host_usage()), token_usage.get());
            assert_eq!(&memory.calls()[5..], &[0, 0, 0]);
        }
    }
}

#[test]
fn auxiliary_success_installs_only_new_lane_and_leaves_parent_ledgers_unchanged() {
    let mut parent =
        persistent_compute_cancellation_test_session(test_queue_key(500, 3), None, None);
    let before = parent.completion_owner.custody_snapshot_for_test();
    let dependency = parent.dependency_owner.custody_snapshot_for_test();
    let slot = prepare_auxiliary_compute_lane_slot_v1(&parent.auxiliary_compute_lanes).unwrap();
    parent.auxiliary_compute_lanes.try_reserve_exact(1).unwrap();
    let roster = parent.auxiliary_compute_lanes.as_ptr();
    let mut construction = AuxiliaryConstructionV1::new([]);
    construction.completed = Some(compute_lane_state_for_multi_inflight_test(test_queue_key(
        501, 1,
    )));
    let completed = construction
        .completed
        .as_ref()
        .unwrap()
        .completion_owner
        .custody_snapshot_for_test();
    let scope = settle_auxiliary_construction_with_v1(
        Box::new(AuxiliaryConstructionScopeV1 {
            terminal_parent: None,
            parent: &mut parent,
            construction,
        }),
        |_| Ok(()),
        |scope, entry| {
            entry.enter("USERPTR auxiliary queue-control creation");
            let destination = check_auxiliary_compute_lane_slot_v1(
                &mut scope.parent.auxiliary_compute_lanes,
                slot,
            )?;
            install_auxiliary_compute_lane_slot_v1(
                destination,
                scope.construction.completed.take().unwrap(),
            );
            Ok(())
        },
        &|| panic!("success poison"),
        |_| panic!("success terminal retention"),
    )
    .unwrap();
    assert!(scope.construction.completed.is_none());
    assert!(scope.terminal_parent.is_none());
    assert!(!scope.parent.terminal_poisoned);
    assert_eq!(
        scope.parent.completion_owner.custody_snapshot_for_test(),
        before
    );
    assert_eq!(
        scope.parent.dependency_owner.custody_snapshot_for_test(),
        dependency
    );
    assert_eq!(scope.parent.auxiliary_compute_lanes.as_ptr(), roster);
    assert_eq!(
        scope.parent.auxiliary_compute_lanes[0]
            .state
            .as_ref()
            .unwrap()
            .completion_owner
            .custody_snapshot_for_test(),
        completed
    );
}
