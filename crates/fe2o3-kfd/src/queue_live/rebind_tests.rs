//! Public lane restoration uses CPU owners, independently of preparation's memory fixture.

use super::tests::{
    compute_lane_state_for_multi_inflight_test, persistent_compute_cancellation_test_session,
    test_queue_key,
};
use super::*;
use std::cell::{Cell, RefCell};
use std::panic::{AssertUnwindSafe, catch_unwind};

#[path = "rebind_tests/preparation.rs"]
mod preparation;

#[test]
fn ordinary_rebind_production_routing_roots_before_loan_and_commits_after_validation() {
    let source = include_str!("rebind.rs");
    let entry = source
        .split("fn bind_fixed_dispatch_settled_v1")
        .nth(1)
        .unwrap()
        .split("pub(in crate::queue) fn settle_fixed_dispatch_rebind_with_v1")
        .next()
        .unwrap();
    let root = entry.find("LiveRebindRootV1::new(").unwrap();
    let settle = entry
        .find("self.settle_fixed_dispatch_rebind_with_v1(")
        .unwrap();
    let loan = entry.find("session.with_live_queue_memory_model(").unwrap();
    let prepare = entry
        .find("prepare_public_fixed_dispatch_resources_after_detach_in_place(")
        .unwrap();
    let validation = entry
        .find("Self::validate_persistent_bind_preparation_v1")
        .unwrap();
    assert!(root < settle && settle < loan && loan < prepare && prepare < validation);
    assert_eq!(entry.matches("with_live_queue_memory_model(").count(), 1);
    let settlement = source
        .split("pub(in crate::queue) fn settle_fixed_dispatch_rebind_with_v1")
        .nth(1)
        .unwrap()
        .split("fn preflight_fixed_dispatch_rebind_v1")
        .next()
        .unwrap();
    let rooted_preparation = settlement.find("root.preparation = Some(").unwrap();
    let preparation_call = settlement.find("prepare(\n").unwrap();
    let validated = settlement
        .find("Some(preparation) => validate(session, preparation)")
        .unwrap();
    let transfer = settlement.find("preparation.take_completed()?").unwrap();
    let install = settlement.find("self.dispatch = Some(prepared)").unwrap();
    assert!(
        rooted_preparation < preparation_call
            && preparation_call < validated
            && validated < transfer
            && transfer < install
    );
    let binding = include_str!("../queue_dispatch_binding.rs");
    let forwarder = binding
        .split("fn prepare_public_fixed_dispatch_resources_after_detach_in_place")
        .nth(1)
        .unwrap()
        .split("\n}")
        .next()
        .unwrap();
    assert!(forwarder.contains("custody.prepare_in_place("));
    assert!(
        forwarder.contains("DispatchGenerationOwnerV1::after_detached(predecessor_generation),")
    );
    assert!(!forwarder.contains("after_recycled"));
    assert!(!forwarder.contains('?'));
    let facade = include_str!("../queue_live.rs")
        .split("impl ComputeAqlQueueLaneDispatchV1<'_>")
        .nth(1)
        .unwrap()
        .split("pub fn preflight_fixed_dispatch_data_insertion")
        .next()
        .unwrap();
    assert!(
        facade
            .find("*self.terminal_transport |= settled.transport")
            .unwrap()
            < facade.find("settled.into_result()").unwrap()
    );
}

fn parent(auxiliary: bool, terminal: bool) -> (ComputeAqlQueueSessionV1, ComputeAqlQueueLaneV1) {
    let mut session =
        persistent_compute_cancellation_test_session(test_queue_key(410, 7), None, None);
    session.detached_dispatch_generation = Some(29);
    session.detached_data_identities = Vec::with_capacity(3);
    session.detached_next_insertion_index = Some(0);
    session.observation = ComputeAqlQueueObservationV1 {
        queue_id: 410,
        ring_bytes: 4096,
        doorbell_slice_bytes: 8192,
        doorbell_byte_offset: 64,
        event_id: 17,
        cwsr_shadow_pages: 2,
    };
    let mut lane = compute_lane_state_for_multi_inflight_test(test_queue_key(411, 8));
    lane.detached_dispatch_generation = Some(37);
    lane.detached_data_identities = Vec::with_capacity(5);
    lane.detached_next_insertion_index = None;
    lane.observation = ComputeAqlQueueObservationV1 {
        queue_id: 411,
        event_id: 23,
        doorbell_byte_offset: 128,
        ..session.observation
    };
    if terminal {
        session.completion_owner.bind_barrier_probe().unwrap();
        lane.completion_owner.bind_barrier_probe().unwrap();
        session.dependency_owner.reserve_acceptance_epoch().unwrap();
        session.dependency_owner.reserve_acceptance_epoch().unwrap();
        if auxiliary {
            lane.detached_data_count = 1;
        } else {
            session.detached_data_count = 1;
        }
    }
    session
        .auxiliary_compute_lanes
        .push(AuxiliaryComputeLaneSlotV1 {
            generation: 9,
            state: Some(lane),
        });
    session.sdma_device_pool.limits = Some(Gfx942DevicePoolLimitsV1::new(1 << 20, 16).unwrap());
    session.sdma_device_pool.activity_started = true;
    session.sdma_host_pool_limits = Some(Gfx942HostPoolLimitsV1::new(1 << 20, 16).unwrap());
    session.sdma_pool_reuse_count = 31;
    let selected = if auxiliary {
        ComputeAqlQueueLaneV1 {
            session: session.compute_lane_session,
            ordinal: 1,
            generation: 9,
        }
    } else {
        session.primary_compute_lane_v1()
    };
    (session, selected)
}

fn assert_shell(session: &mut ComputeAqlQueueSessionV1, lane: ComputeAqlQueueLaneV1) {
    assert!(session.terminal_poisoned);
    assert!(session.engine.is_none());
    assert!(session.completion_owner.0.is_none());
    assert!(session.dependency_owner.0.is_none());
    assert!(session.dispatch.is_none());
    assert!(session.auxiliary_compute_lanes.is_empty());
    session.poison_terminal();
    assert!(
        session
            .with_compute_lane_v1(lane, |_| panic!("terminal callback"))
            .is_err()
    );
    assert!(
        session
            .bind_fixed_dispatch::<0>(Vec::new(), [], Vec::new())
            .is_err()
    );
    assert!(
        session
            .submit_fixed_dispatch_with_dependencies_v1(lane, Vec::new())
            .is_err()
    );
    assert!(session.sdma_device_pool_usage_v1().is_err());
    assert!(session.sdma_host_pool_usage_v1().is_err());
}

#[test]
fn ordinary_rebind_healthy_preflight_retains_parent_without_new_poison() {
    for auxiliary in [false, true] {
        let (mut session, lane) = parent(auxiliary, false);
        let _ = take_dispatch_terminal_process_gate_record_v1();
        let _ = take_lane_unwind_process_gate_record_v1();
        let completion = session.completion_owner.custody_snapshot_for_test();
        let dependency = session.dependency_owner.custody_snapshot_for_test();
        let roster = session.auxiliary_compute_lanes.as_ptr();
        let result = session
            .with_compute_lane_custody_v1(
                lane,
                |selected| selected.bind_fixed_dispatch::<0>(Vec::new(), [], Vec::new()),
                |_| panic!("healthy rejection transported parent"),
            )
            .unwrap();
        assert!(matches!(
            result,
            Err(ComputeAqlQueueSessionErrorV1::DispatchBinding(
                Gfx942DispatchBindingErrorV1::ZeroPacketCount
            ))
        ));
        assert!(!session.terminal_poisoned);
        assert_eq!(
            session.completion_owner.custody_snapshot_for_test(),
            completion
        );
        assert_eq!(
            session.dependency_owner.custody_snapshot_for_test(),
            dependency
        );
        assert_eq!(session.auxiliary_compute_lanes.as_ptr(), roster);
        assert_eq!(session.with_compute_lane_v1(lane, |_| 17).unwrap(), 17);
        assert!(!take_dispatch_terminal_process_gate_record_v1());
        assert!(!take_lane_unwind_process_gate_record_v1());
    }
}

#[test]
fn ordinary_rebind_terminal_facade_restores_exact_parent_before_transport() {
    for auxiliary in [false, true] {
        // Returned Err, swallowed Err, caught caller panic, escaping caller panic.
        for mode in 0..4 {
            let (mut session, lane) = parent(auxiliary, true);
            let _ = take_dispatch_terminal_process_gate_record_v1();
            let _ = take_lane_unwind_process_gate_record_v1();
            let primary_key = session.key;
            let primary_observation = session.observation;
            let completion = session.completion_owner.custody_snapshot_for_test();
            let dependency = session.dependency_owner.custody_snapshot_for_test();
            let roster = (
                session.auxiliary_compute_lanes.as_ptr(),
                session.auxiliary_compute_lanes.capacity(),
            );
            let primary_data = (
                session.detached_data_count,
                session.detached_data_identities.as_ptr(),
                session.detached_data_identities.capacity(),
            );
            let other = session.auxiliary_compute_lanes[0].state.as_ref().unwrap();
            let auxiliary_key = other.key;
            let auxiliary_observation = other.observation;
            let auxiliary_completion = other.completion_owner.custody_snapshot_for_test();
            let auxiliary_data = (
                other.detached_data_count,
                other.detached_data_identities.as_ptr(),
                other.detached_data_identities.capacity(),
            );
            let retained = RefCell::new(None);
            let transfers = Cell::new(0);
            let result = catch_unwind(AssertUnwindSafe(|| {
                session.with_compute_lane_custody_v1(
                    lane,
                    |selected| {
                        assert_eq!(
                            selected.observation().queue_id,
                            if auxiliary { 411 } else { 410 }
                        );
                        let error = selected
                            .bind_fixed_dispatch::<0>(Vec::new(), [], Vec::new())
                            .unwrap_err();
                        assert!(matches!(
                            error,
                            ComputeAqlQueueSessionErrorV1::Contract(
                                "detached dispatch-data identity ledger cardinality"
                            )
                        ));
                        // A later rejection cannot clear a pending terminal disposition.
                        assert!(matches!(
                            selected.bind_fixed_dispatch::<0>(Vec::new(), [], Vec::new()),
                            Err(ComputeAqlQueueSessionErrorV1::DispatchBinding(
                                Gfx942DispatchBindingErrorV1::Poisoned
                            ))
                        ));
                        match mode {
                            0 => Err(error),
                            1 => Ok(37),
                            2 => {
                                let caught = catch_unwind(AssertUnwindSafe(|| {
                                    selected
                                        .bind_fixed_dispatch::<0>(Vec::new(), [], Vec::new())
                                        .unwrap();
                                }));
                                assert!(caught.is_err());
                                Ok(37)
                            }
                            _ => std::panic::panic_any("caller after terminal bind"),
                        }
                    },
                    |root| {
                        transfers.set(transfers.get() + 1);
                        // Callback unwind poisoning must precede parent transport.
                        assert_eq!(take_lane_unwind_process_gate_record_v1(), mode == 3);
                        let parent = root.as_ref().as_ref().unwrap();
                        assert!(parent.terminal_poisoned);
                        assert_eq!(parent.compute_lane_session, lane.session);
                        assert_eq!(parent.key, primary_key);
                        assert_eq!(parent.observation, primary_observation);
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
                            roster
                        );
                        assert_eq!(parent.auxiliary_compute_lanes.len(), 1);
                        assert_eq!(parent.auxiliary_compute_lanes[0].generation, 9);
                        assert_eq!(
                            (
                                parent.detached_data_count,
                                parent.detached_data_identities.as_ptr(),
                                parent.detached_data_identities.capacity()
                            ),
                            primary_data
                        );
                        assert_eq!(parent.detached_dispatch_generation, Some(29));
                        assert_eq!(parent.detached_next_insertion_index, Some(0));
                        let other = parent.auxiliary_compute_lanes[0].state.as_ref().unwrap();
                        assert_eq!(other.key, auxiliary_key);
                        assert_eq!(other.observation, auxiliary_observation);
                        assert_eq!(
                            other.completion_owner.custody_snapshot_for_test(),
                            auxiliary_completion
                        );
                        assert_eq!(
                            (
                                other.detached_data_count,
                                other.detached_data_identities.as_ptr(),
                                other.detached_data_identities.capacity()
                            ),
                            auxiliary_data
                        );
                        assert_eq!(other.detached_dispatch_generation, Some(37));
                        assert_eq!(other.detached_next_insertion_index, None);
                        assert!(matches!(
                            parent.completion_owner.ensure_releasable(),
                            Err(Gfx942CompletionErrorV1::Poisoned)
                        ));
                        assert!(matches!(
                            parent.dependency_owner.ensure_idle(),
                            Err(ComputeDependencyTargetUseErrorV1::Poisoned)
                        ));
                        assert!(parent.completion_owner.is_poisoned_for_test());
                        assert_eq!(other.completion_owner.is_poisoned_for_test(), auxiliary);
                        assert_eq!(parent.sdma_pool_reuse_count, 31);
                        assert_eq!(
                            parent.sdma_device_pool.limits,
                            Some(Gfx942DevicePoolLimitsV1::new(1 << 20, 16).unwrap())
                        );
                        assert!(parent.sdma_device_pool.activity_started);
                        assert_eq!(
                            parent.sdma_host_pool_limits,
                            Some(Gfx942HostPoolLimitsV1::new(1 << 20, 16).unwrap())
                        );
                        *retained.borrow_mut() = Some(root);
                    },
                )
            }));
            if mode == 3 {
                assert_eq!(
                    result.unwrap_err().downcast_ref::<&str>(),
                    Some(&"caller after terminal bind")
                );
            } else {
                let value = result.unwrap().unwrap();
                assert_eq!(value.is_err(), mode == 0);
                if mode == 0 {
                    assert!(matches!(
                        value,
                        Err(ComputeAqlQueueSessionErrorV1::Contract(
                            "detached dispatch-data identity ledger cardinality"
                        ))
                    ));
                } else {
                    assert_eq!(value.unwrap(), 37);
                }
            }
            assert_eq!(transfers.get(), 1);
            assert!(
                !take_dispatch_terminal_process_gate_record_v1(),
                "returned bind error stays process-local"
            );
            assert!(!take_lane_unwind_process_gate_record_v1());
            assert_shell(&mut session, lane);
            drop(session);
            let root = retained.into_inner().unwrap();
            let parent = root.as_ref().as_ref().unwrap();
            assert_eq!(
                parent.completion_owner.custody_snapshot_for_test(),
                completion
            );
            assert_eq!(
                parent.dependency_owner.custody_snapshot_for_test(),
                dependency
            );
            assert_eq!(parent.auxiliary_compute_lanes.as_ptr(), roster.0);
        }
    }
}

#[test]
fn ordinary_rebind_caught_internal_preflight_panic_still_transports() {
    for auxiliary in [false, true] {
        let (mut session, lane) = parent(auxiliary, false);
        // Deliberate missing-owner inconsistency exercises an internal preflight unwind.
        if auxiliary {
            session.auxiliary_compute_lanes[0]
                .state
                .as_mut()
                .unwrap()
                .completion_owner
                .0 = None;
        } else {
            session.completion_owner.0 = None;
        }
        let _ = take_dispatch_terminal_process_gate_record_v1();
        let _ = take_lane_unwind_process_gate_record_v1();
        let retained = RefCell::new(None);
        let transfers = Cell::new(0);
        let dependency = session.dependency_owner.custody_snapshot_for_test();
        let untouched_completion = if auxiliary {
            session.completion_owner.custody_snapshot_for_test()
        } else {
            session.auxiliary_compute_lanes[0]
                .state
                .as_ref()
                .unwrap()
                .completion_owner
                .custody_snapshot_for_test()
        };
        let value = session
            .with_compute_lane_custody_v1(
                lane,
                |selected| {
                    let payload = catch_unwind(AssertUnwindSafe(|| {
                        selected.bind_fixed_dispatch::<0>(Vec::new(), [], Vec::new())
                    }))
                    .unwrap_err();
                    assert_eq!(
                        payload.downcast_ref::<String>().map(String::as_str),
                        Some("queue owner retained in terminal custody")
                    );
                    91usize
                },
                |root| {
                    transfers.set(transfers.get() + 1);
                    *retained.borrow_mut() = Some(root);
                },
            )
            .unwrap();
        assert_eq!(value, 91);
        assert!(take_dispatch_terminal_process_gate_record_v1());
        assert!(!take_lane_unwind_process_gate_record_v1());
        assert_shell(&mut session, lane);
        let root = retained.into_inner().unwrap();
        let parent = root.as_ref().as_ref().unwrap();
        assert_eq!(transfers.get(), 1);
        assert!(parent.terminal_poisoned);
        assert_eq!(parent.compute_lane_session, lane.session);
        assert_eq!(
            parent.dependency_owner.custody_snapshot_for_test(),
            dependency
        );
        let completion = if auxiliary {
            &parent.completion_owner
        } else {
            &parent.auxiliary_compute_lanes[0]
                .state
                .as_ref()
                .unwrap()
                .completion_owner
        };
        assert_eq!(completion.custody_snapshot_for_test(), untouched_completion);
        assert_eq!(parent.key, test_queue_key(410, 7));
        assert_eq!(
            parent.auxiliary_compute_lanes[0]
                .state
                .as_ref()
                .unwrap()
                .key,
            test_queue_key(411, 8)
        );
    }
}

#[test]
fn ordinary_rebind_public_direct_wrapper_transports_terminal_parent() {
    let (mut session, lane) = parent(false, true);
    let _ = take_dispatch_terminal_process_gate_record_v1();
    assert!(matches!(
        session.bind_fixed_dispatch::<0>(Vec::new(), [], Vec::new()),
        Err(ComputeAqlQueueSessionErrorV1::Contract(
            "detached dispatch-data identity ledger cardinality"
        ))
    ));
    assert_shell(&mut session, lane);
    assert!(!take_dispatch_terminal_process_gate_record_v1());
}
