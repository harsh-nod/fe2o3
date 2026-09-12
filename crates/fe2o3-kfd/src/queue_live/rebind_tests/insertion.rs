//! Real session/facade preflight and missing-engine settlement, not native success.

use super::*;

fn input() -> (Box<[u8]>, Gfx942DeviceContentDescriptorV1) {
    let bytes = vec![0x49; 17].into_boxed_slice();
    let role = crate::Gfx942DeviceContentRoleV1::new([0x49; 32], 7).unwrap();
    let content = Gfx942DeviceContentDescriptorV1::from_bytes(role, &bytes).unwrap();
    (bytes, content)
}

#[test]
fn device_insertion_concrete_facade_restores_before_transport_even_after_swallowed_error() {
    for ordinal in 0..3 {
        for mode in 0..4 {
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
                    selected
                        .session
                        .detached_data_identities
                        .try_reserve_exact(16)
                        .unwrap();
                })
                .unwrap();
            let snapshot = |s: &ComputeAqlQueueSessionV1| {
                (
                    s.key,
                    s.compute_lane_session,
                    s.observation,
                    s.detached_data_count,
                    s.detached_dispatch_generation,
                    s.detached_next_insertion_index,
                    (
                        s.detached_data_identities.clone(),
                        s.detached_data_identities.as_ptr(),
                        s.detached_data_identities.capacity(),
                    ),
                    s.completion_owner.custody_snapshot_for_test(),
                    s.dependency_owner.custody_snapshot_for_test(),
                    s.auxiliary_compute_lanes
                        .iter()
                        .map(|slot| {
                            (
                                slot.generation,
                                slot.state.as_ref().map(|s| {
                                    (
                                        s.key,
                                        s.observation,
                                        s.detached_data_count,
                                        s.detached_dispatch_generation,
                                        s.detached_next_insertion_index,
                                        s.detached_data_identities.clone(),
                                        s.detached_data_identities.as_ptr(),
                                        s.detached_data_identities.capacity(),
                                        s.completion_owner.custody_snapshot_for_test(),
                                    )
                                }),
                            )
                        })
                        .collect::<Vec<_>>(),
                )
            };
            let before = snapshot(&session);
            let retained = RefCell::new(None);
            let transfers = Cell::new(0);
            let _ = take_dispatch_terminal_process_gate_record_v1();
            let _ = take_lane_unwind_process_gate_record_v1();
            let result = catch_unwind(AssertUnwindSafe(|| {
                session.with_compute_lane_custody_v1(
                    lane,
                    |selected| {
                        let (bytes, content) = input();
                        let error = selected
                            .insert_initialized_fixed_dispatch_data(0, bytes, 4096, content)
                            .err()
                            .unwrap();
                        assert!(matches!(
                            error,
                            ComputeAqlQueueSessionErrorV1::Contract("missing queue engine")
                        ));
                        assert!(*selected.terminal_transport);
                        // A subsequent healthy preflight rejection must not clear the
                        // earlier operation's requirement to transport the whole parent.
                        let (bytes, content) = input();
                        let retry = selected
                            .insert_initialized_fixed_dispatch_data(0, bytes, 4096, content)
                            .err()
                            .unwrap();
                        assert!(matches!(
                            retry,
                            ComputeAqlQueueSessionErrorV1::DispatchBinding(
                                Gfx942DispatchBindingErrorV1::Poisoned
                            )
                        ));
                        assert!(*selected.terminal_transport);
                        match mode {
                            0 => Err(error),
                            1 => Ok(37),
                            2 => {
                                assert!(catch_unwind(|| panic!("caught caller panic")).is_err());
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
                            "exact lanes restored before whole-parent retention"
                        );
                        assert!(
                            parent.terminal_poisoned
                                && parent.completion_owner.is_poisoned_for_test()
                        );
                        for (index, slot) in parent.auxiliary_compute_lanes.iter().enumerate() {
                            if let Some(state) = &slot.state {
                                assert_eq!(
                                    state.completion_owner.is_poisoned_for_test(),
                                    index + 1 == ordinal
                                );
                            }
                        }
                        *retained.borrow_mut() = Some(root);
                    },
                )
            }));
            if mode == 3 {
                assert_eq!(
                    result.unwrap_err().downcast_ref::<&str>(),
                    Some(&"escaping caller panic")
                );
            } else {
                assert_eq!(result.unwrap().unwrap().is_err(), mode == 0);
            }
            assert_eq!(transfers.get(), 1);
            assert!(!take_dispatch_terminal_process_gate_record_v1());
            assert_eq!(take_lane_unwind_process_gate_record_v1(), mode == 3);
            assert_shell(&mut session, lane);
            let retained = retained.into_inner().unwrap();
            assert_eq!(snapshot(retained.as_ref().as_ref().unwrap()), before);
        }
    }
}

#[test]
fn device_insertion_concrete_preflight_keeps_error_precedence_and_healthy_parent() {
    let mut memory = crate::shared_memory::PreparationMemoryFixtureV1::new(true);
    let data = memory.roster();
    let identity = data[1].storage_identity();
    for mode in 0..5 {
        let (mut session, _) = parent(false, false);
        let count = if mode == 0 { 17 } else { 16 };
        session.detached_data_count = count;
        session.detached_data_identities = vec![identity; count];
        session.detached_next_insertion_index = None;
        match mode {
            1 => session.detached_dispatch_generation = None,
            2 => {
                session.completion_owner.bind_barrier_probe().unwrap();
            }
            3 => {
                session.detached_data_identities.pop();
            }
            _ => {}
        }
        let identities = session.detached_data_identities.clone();
        let capacity = session.detached_data_identities.capacity();
        let (bytes, content) = input();
        let settled = session.initialize_device_data_settled_v1(Some(99), bytes, 4096, content);
        assert!(!settled.transport);
        let error = settled.into_result().err().unwrap();
        match mode {
            0 => assert!(matches!(
                error,
                ComputeAqlQueueSessionErrorV1::Contract("detached dispatch-data ledger bound")
            )),
            1 => assert!(matches!(
                error,
                ComputeAqlQueueSessionErrorV1::DispatchBinding(
                    Gfx942DispatchBindingErrorV1::ResourcePhase
                )
            )),
            2 => assert!(matches!(
                error,
                ComputeAqlQueueSessionErrorV1::Completion(_)
            )),
            3 => assert!(matches!(
                error,
                ComputeAqlQueueSessionErrorV1::Contract("detached dispatch-data identity ledger")
            )),
            _ => assert!(matches!(
                error,
                ComputeAqlQueueSessionErrorV1::DispatchBinding(
                    Gfx942DispatchBindingErrorV1::DataLeaseCount {
                        requested: 17,
                        maximum: 16
                    }
                )
            )),
        }
        assert!(!session.terminal_poisoned);
        assert_eq!(session.detached_data_count, count);
        assert_eq!(session.detached_data_identities, identities);
        assert_eq!(session.detached_data_identities.capacity(), capacity);
    }
}

#[test]
fn device_insertion_concrete_bound_dispatch_precedes_full_and_malformed_ledger() {
    use crate::queue::dispatch_binding::{
        actual_persistent_control_test_program, prepare_public_fixed_dispatch_resources_in_place,
    };
    let mut memory = crate::shared_memory::PreparationMemoryFixtureV1::new(true);
    let data = memory.roster();
    let identity = data[0].storage_identity();
    let image = include_bytes!(
        "../../../../fe2o3-runtime/fixtures/trusted-gfx942-inplace-transform-v1/inplace_transform.hsaco"
    );
    let programs = [actual_persistent_control_test_program(image, [1; 32])];
    let mut args = [0; 16];
    args[8..].copy_from_slice(&1024_u64.to_le_bytes());
    let packet = Gfx942FixedDispatchPacketV1::new(
        0,
        fe2o3_aql::AqlDispatchGeometryV1::new([256, 1, 1], [256, 1, 1]).unwrap(),
        0,
        args.into(),
        vec![Gfx942DispatchBufferBindingV1::new(0, 0, 0, 4096)].into_boxed_slice(),
    );
    let mut preparation = FixedDispatchPreparationCustodyV1::new([packet], data);
    prepare_public_fixed_dispatch_resources_in_place(&mut memory, &programs, &mut preparation)
        .unwrap();
    let (mut session, _) = parent(false, false);
    session.dispatch = Some(preparation.take_completed().unwrap());
    let original = session
        .dispatch
        .as_ref()
        .unwrap()
        .primary_fixture_identities_v1();
    for count in [16, 17] {
        session.detached_data_count = count;
        session.detached_data_identities = vec![identity; count];
        let (bytes, content) = input();
        let settled = session.initialize_device_data_settled_v1(Some(99), bytes, 4096, content);
        assert!(!settled.transport);
        assert!(matches!(
            settled.result,
            Ok(Err(ComputeAqlQueueSessionErrorV1::DispatchBinding(
                Gfx942DispatchBindingErrorV1::ResourcePhase
            )))
        ));
        assert!(!session.terminal_poisoned);
        assert_eq!(
            session
                .dispatch
                .as_ref()
                .unwrap()
                .primary_fixture_identities_v1(),
            original
        );
    }
}

#[test]
fn device_insertion_production_routing_roots_reserves_settles_commits_then_extracts() {
    let source = include_str!("../data_insertion.rs");
    let sequence = source
        .split("pub(in crate::queue) fn settle_device_insertion_v1")
        .nth(1)
        .unwrap()
        .split("impl DeviceInsertionContextV1 for ComputeAqlQueueSessionV1")
        .next()
        .unwrap();
    let mut previous = 0;
    for marker in [
        "context.require_unbound()?",
        "validate_new_detached_data_index",
        "context.reserve()?",
        "context.prepare(&mut root, alignment)?",
        "root.completed()?",
        "insert_detached_identity_at",
        "*ledger.count = next_count",
        ".take_complete()",
    ] {
        let at = sequence.find(marker).unwrap();
        assert!(at >= previous, "{marker} preserves ownership order");
        previous = at;
    }
    let adapter = source
        .split("impl DeviceInsertionContextV1 for ComputeAqlQueueSessionV1")
        .nth(1)
        .unwrap();
    assert!(adapter.contains("self.with_live_queue_memory_model(|memory|"));
    assert!(adapter.contains(".prepare_device_initialization_in_place(root, alignment)"));
    assert!(!adapter.contains("take_complete"));
    assert!(adapter.contains("if root.requires_retention()"));
    assert!(adapter.contains("if let Some(engine) = self.engine.as_mut()"));
    let shared = include_str!("../../shared_memory.rs");
    assert!(shared.contains("#[cfg(all(target_os = \"linux\", target_arch = \"x86_64\"))]\n#[must_use = \"dropping the shared session performs no munmap, FREE, or retry\"]\npub struct SharedGttMemorySessionV1"));
    let prepare = shared
        .split("pub(crate) fn prepare_device_initialization_in_place(")
        .nth(1)
        .unwrap()
        .split("\n    }")
        .next()
        .unwrap();
    assert!(prepare.contains("root: &mut DeviceInitializationCustodyV1"));
    assert!(prepare.contains("root.prepare_with_engine(\n            &mut self.engine,\n            self.model_device.model_key(),\n            self.vm,\n            alignment,\n        )"));
    let retain = shared
        .split("pub(crate) fn retain_device_initialization_failure(")
        .nth(1)
        .unwrap()
        .split("\n    }")
        .next()
        .unwrap();
    assert!(retain.contains("root: DeviceInitializationCustodyV1"));
    assert!(retain.contains("root.retain_with_engine(&mut self.engine);"));
    let fixed = include_str!("../fixed_dispatch.rs");
    for name in [
        "pub fn initialize_fixed_dispatch_data(",
        "pub fn insert_initialized_fixed_dispatch_data(",
    ] {
        let body = fixed
            .split(name)
            .nth(1)
            .unwrap()
            .split("\n    pub fn ")
            .next()
            .unwrap();
        assert!(body.contains("initialize_device_data_settled_v1("));
        assert!(
            body.find("retain_terminal_rebind_parent_v1").unwrap()
                < body.find("settled.into_result()").unwrap()
        );
    }
    let facade = include_str!("../../queue_live.rs")
        .split("impl ComputeAqlQueueLaneDispatchV1<'_>")
        .nth(1)
        .unwrap()
        .split("pub fn insert_initialized_fixed_dispatch_data(")
        .nth(1)
        .unwrap()
        .split("pub fn overwrite_detached_initialized_host_visible_fixed_dispatch_data(")
        .next()
        .unwrap();
    assert!(
        facade
            .find("*self.terminal_transport |= settled.transport")
            .unwrap()
            < facade.find("settled.into_result()").unwrap()
    );
    assert!(!facade.contains("retain_terminal_rebind_parent"));
}
