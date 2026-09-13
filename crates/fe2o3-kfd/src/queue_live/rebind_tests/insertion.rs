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
    facade_cases(InsertionRoute::Device);
}

#[derive(Clone, Copy)]
enum InsertionRoute {
    Device,
    CoherentOwned,
    CoherentBorrowed,
    CoherentReplacement,
}

impl InsertionRoute {
    fn invoke(
        self,
        selected: &mut ComputeAqlQueueLaneDispatchV1<'_>,
    ) -> Result<Gfx942FixedDispatchDataV1, ComputeAqlQueueSessionErrorV1> {
        let source = [0x49; 17];
        let result = match self {
            Self::Device => {
                let (bytes, content) = input();
                selected.insert_initialized_fixed_dispatch_data(0, bytes, 4096, content)
            }
            Self::CoherentOwned => {
                selected.insert_initialized_host_visible_fixed_dispatch_data(0, Box::from(source))
            }
            Self::CoherentBorrowed => selected
                .insert_initialized_host_visible_fixed_dispatch_data_from_slice_v1(0, &source),
            Self::CoherentReplacement => {
                selected.initialize_host_visible_fixed_dispatch_data_from_slice_v1(&source)
            }
        };
        assert_eq!(source, [0x49; 17]);
        result
    }
}

#[test]
fn coherent_insertion_concrete_facade_restores_before_transport_for_all_routes() {
    for route in [
        InsertionRoute::CoherentOwned,
        InsertionRoute::CoherentBorrowed,
        InsertionRoute::CoherentReplacement,
    ] {
        facade_cases(route);
    }
}

fn invoke_coherent_public(
    route: usize,
    session: &mut ComputeAqlQueueSessionV1,
    lane: ComputeAqlQueueLaneV1,
    source: &[u8],
) -> Result<Gfx942FixedDispatchDataV1, ComputeAqlQueueSessionErrorV1> {
    match route {
        0 => session.insert_initialized_host_visible_fixed_dispatch_data(99, source.into()),
        1 => session.insert_initialized_host_visible_fixed_dispatch_data_from_slice_v1(99, source),
        2 => session.initialize_host_visible_fixed_dispatch_data(source.into()),
        3 => session.initialize_host_visible_fixed_dispatch_data_from_slice_v1(source),
        7 => session.insert_host_visible_fixed_dispatch_data(99, source.len()),
        8 => session.allocate_host_visible_fixed_dispatch_data(source.len()),
        _ => session.with_compute_lane_custody_v1(
            lane,
            |selected| {
                let result = match route {
                    4 => selected
                        .insert_initialized_host_visible_fixed_dispatch_data(99, source.into()),
                    5 => selected
                        .insert_initialized_host_visible_fixed_dispatch_data_from_slice_v1(
                            99, source,
                        ),
                    6 => selected.initialize_host_visible_fixed_dispatch_data_from_slice_v1(source),
                    _ => unreachable!(),
                };
                assert!(
                    !*selected.terminal_transport,
                    "public preflight must not transfer the parent"
                );
                result
            },
            |_| panic!("preflight cannot enter terminal transport"),
        )?,
    }
}

#[test]
fn coherent_insertion_all_public_entrypoints_preserve_preflight_precedence() {
    coherent_public_preflight_cases(0..7);
}

#[test]
fn coherent_allocation_insertion_public_preflight_preserves_owners_and_precedence() {
    coherent_public_preflight_cases(7..9);
}

fn coherent_public_preflight_cases(routes: core::ops::Range<usize>) {
    let mut memory = crate::shared_memory::PreparationMemoryFixtureV1::new(true);
    let data = memory.roster();
    let identity = data[1].storage_identity();
    for route in routes {
        for empty in [false, true] {
            for mode in 0..8 {
                let (mut session, lane) = parent(false, false);
                session.detached_data_count = if mode <= 2 {
                    17
                } else if mode <= 6 {
                    16
                } else {
                    4
                };
                session.detached_data_identities = vec![identity; session.detached_data_count];
                session.detached_next_insertion_index = None;
                match mode {
                    0 => session.terminal_poisoned = true,
                    1 => session.detached_dispatch_generation = None,
                    3 => {
                        session.detached_data_identities.pop();
                    }
                    4 => session.detached_next_insertion_index = Some(17),
                    5 => {
                        session.completion_owner.bind_barrier_probe().unwrap();
                    }
                    _ => {}
                }
                let snapshot = |s: &ComputeAqlQueueSessionV1| {
                    (
                        (s.key, s.compute_lane_session, s.observation),
                        s.detached_data_count,
                        s.detached_dispatch_generation,
                        s.detached_next_insertion_index,
                        s.detached_data_identities.clone(),
                        s.detached_data_identities.as_ptr(),
                        s.detached_data_identities.capacity(),
                        s.completion_owner.custody_snapshot_for_test(),
                        s.dependency_owner.custody_snapshot_for_test(),
                        s.terminal_poisoned,
                        s.completion_owner.is_poisoned_for_test(),
                        s.auxiliary_compute_lanes
                            .iter()
                            .map(|slot| {
                                (
                                    slot.generation,
                                    slot.state.as_ref().map(|state| {
                                        (
                                            state.key,
                                            state.observation,
                                            state.detached_data_count,
                                            state.detached_dispatch_generation,
                                            state.detached_next_insertion_index,
                                            state.detached_data_identities.clone(),
                                            state.detached_data_identities.as_ptr(),
                                            state.detached_data_identities.capacity(),
                                            state.completion_owner.custody_snapshot_for_test(),
                                            state.completion_owner.is_poisoned_for_test(),
                                        )
                                    }),
                                )
                            })
                            .collect::<Vec<_>>(),
                    )
                };
                let before = snapshot(&session);
                session.dependency_owner.ensure_idle().unwrap();
                let source = if empty { &[][..] } else { &[0x49; 17][..] };
                let error = invoke_coherent_public(route, &mut session, lane, source)
                    .err()
                    .unwrap();
                match mode {
                    0 => assert!(matches!(
                        error,
                        ComputeAqlQueueSessionErrorV1::DispatchBinding(
                            Gfx942DispatchBindingErrorV1::Poisoned
                        )
                    )),
                    1 => assert!(matches!(
                        error,
                        ComputeAqlQueueSessionErrorV1::DispatchBinding(
                            Gfx942DispatchBindingErrorV1::ResourcePhase
                        )
                    )),
                    2 => assert!(matches!(
                        error,
                        ComputeAqlQueueSessionErrorV1::Contract(
                            "detached dispatch-data ledger bound"
                        )
                    )),
                    3 | 4 => assert!(matches!(
                        error,
                        ComputeAqlQueueSessionErrorV1::Contract(
                            "detached dispatch-data identity ledger"
                        )
                    )),
                    5 => assert!(matches!(
                        error,
                        ComputeAqlQueueSessionErrorV1::Completion(
                            Gfx942CompletionErrorV1::Poisoned
                        )
                    )),
                    6 => assert!(matches!(
                        error,
                        ComputeAqlQueueSessionErrorV1::DispatchBinding(
                            Gfx942DispatchBindingErrorV1::DataLeaseCount {
                                requested: 17,
                                maximum: 16
                            }
                        )
                    )),
                    _ if matches!(route, 2 | 3 | 6 | 8) => assert!(matches!(
                        error,
                        ComputeAqlQueueSessionErrorV1::DispatchBinding(
                            Gfx942DispatchBindingErrorV1::ResourcePhase
                        )
                    )),
                    _ => assert!(matches!(
                        error,
                        ComputeAqlQueueSessionErrorV1::DispatchBinding(
                            Gfx942DispatchBindingErrorV1::InvalidData {
                                index: 4,
                                detail: "detached insertion ordinal"
                            }
                        )
                    )),
                }
                assert_eq!(snapshot(&session), before);
                session.dependency_owner.ensure_idle().unwrap();
                assert!(session.engine.is_none());
            }
        }
    }
}

fn facade_cases(route: InsertionRoute) {
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
                    if matches!(route, InsertionRoute::CoherentReplacement) {
                        selected.session.detached_next_insertion_index = Some(0);
                    }
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
                        let error = route.invoke(selected).err().unwrap();
                        assert!(matches!(
                            error,
                            ComputeAqlQueueSessionErrorV1::Contract("missing queue engine")
                        ));
                        assert!(*selected.terminal_transport);
                        // A subsequent healthy preflight rejection must not clear the
                        // earlier operation's requirement to transport the whole parent.
                        let retry = route.invoke(selected).err().unwrap();
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
    bound_dispatch_precedence(0..0);
}

#[test]
fn coherent_insertion_actual_bound_dispatch_precedes_full_and_malformed_ledger() {
    bound_dispatch_precedence(0..7);
}

#[test]
fn coherent_allocation_insertion_bound_dispatch_precedes_full_and_invalid_size() {
    bound_dispatch_precedence(7..9);
}

#[test]
fn coherent_allocation_insertion_missing_engine_retains_terminal_parent_without_panic_marker() {
    for replacement in [false, true] {
        for requested_bytes in [0, 17] {
            let (mut session, lane) = parent(false, false);
            session.detached_next_insertion_index = replacement.then_some(0);
            let _ = take_dispatch_terminal_process_gate_record_v1();
            let _ = take_lane_unwind_process_gate_record_v1();
            let result = if replacement {
                session.allocate_host_visible_fixed_dispatch_data(requested_bytes)
            } else {
                session.insert_host_visible_fixed_dispatch_data(0, requested_bytes)
            };
            assert!(matches!(
                result,
                Err(ComputeAqlQueueSessionErrorV1::Contract(
                    "missing queue engine"
                ))
            ));
            assert_shell(&mut session, lane);
            assert!(!take_dispatch_terminal_process_gate_record_v1());
            assert!(!take_lane_unwind_process_gate_record_v1());
        }
    }
}

#[test]
fn coherent_allocation_insertion_production_wiring_never_grants_initialized_authority() {
    let shared = include_str!("../../shared_memory.rs");
    let root = shared
        .split("impl CoherentAllocationCustodyV1")
        .nth(1)
        .unwrap()
        .split("pub(crate) struct DeviceInitializationCustodyV1")
        .next()
        .unwrap();
    let allocate = root
        .find("let allocation = memory.allocate(requested_bytes)?;")
        .unwrap();
    let map = root
        .find("self.completed = Some(memory.map(allocation)?);")
        .unwrap();
    assert!(allocate < map);
    assert!(root.contains("core::mem::replace(&mut self.started, true)"));
    assert!(root.contains("transitions::retain_coherent_insertion_output_v1(engine, completed)"));
    for forbidden in [
        "initialize_v1",
        "memory.copy(",
        "copy_from_slice",
        "retag(",
        "Box::new",
        "to_vec()",
    ] {
        assert!(
            !root.contains(forbidden),
            "uninitialized root cannot introduce {forbidden}"
        );
    }
    let source = include_str!("../data_insertion.rs");
    let adapter = source
        .split("impl DataInsertionRootV1<usize>")
        .nth(1)
        .unwrap()
        .split("pub(in crate::queue) trait DataInsertionContextV1")
        .next()
        .unwrap();
    assert!(adapter.contains("Gfx942FixedDispatchStorageIdentityV1::HostVisibleUninitialized"));
    assert!(adapter.contains(".map(Gfx942FixedDispatchDataV1::host_visible_uninitialized)"));
    assert!(adapter.contains("memory.prepare_coherent_allocation_in_place(self, requested_bytes)"));
    assert!(adapter.contains("memory.retain_coherent_allocation_failure(self)"));
    assert!(!adapter.contains("host_visible_initialized"));
    let fixed = include_str!("../fixed_dispatch.rs");
    for (name, argument) in [
        (
            "insert_host_visible_fixed_dispatch_data",
            "Some(data_index)",
        ),
        ("allocate_host_visible_fixed_dispatch_data", "None"),
    ] {
        let body = fixed
            .split(&format!("pub fn {name}("))
            .nth(1)
            .unwrap()
            .split("\n    }")
            .next()
            .unwrap();
        assert!(body.contains(&format!(
            "allocate_coherent_data_settled_v1({argument}, requested_bytes)"
        )));
        let retain = body
            .find("retain_terminal_rebind_parent_v1(core::mem::forget)")
            .unwrap();
        assert!(retain < body.find("settled.into_result()").unwrap());
        assert!(!body.contains("with_live_queue_memory_model"));
    }
}

#[test]
fn coherent_insertion_production_routing_preserves_root_and_terminal_owner() {
    let insertion = include_str!("../data_insertion.rs");
    let root = insertion
        .split("impl DataInsertionRootV1<&[u8]>")
        .nth(1)
        .unwrap()
        .split("impl DataInsertionRootV1<usize>")
        .next()
        .unwrap();
    assert!(root.contains("self.completed()?.storage_identity()"));
    assert!(root.contains("Gfx942FixedDispatchStorageIdentityV1::HostVisibleInitialized"));
    assert!(root.contains(".map(Gfx942FixedDispatchDataV1::host_visible_initialized)"));
    assert!(root.contains("memory.prepare_coherent_initialization_in_place(self, source)"));
    assert!(root.contains("memory.retain_coherent_initialization_failure(self)"));
    let shared = include_str!("../../shared_memory.rs");
    let forwarder = shared
        .split("pub(crate) fn retain_coherent_initialization_failure(")
        .nth(1)
        .unwrap()
        .split("\n    }")
        .next()
        .unwrap();
    assert!(forwarder.contains("root.retain_with_engine(&mut self.engine)"));
    let root = shared
        .split("impl CoherentInitializationCustodyV1")
        .nth(1)
        .unwrap()
        .split("pub(crate) struct CoherentAllocationCustodyV1")
        .next()
        .unwrap();
    assert!(root.contains(
        "transitions::retain_coherent_insertion_output_v1(engine, completed.into_token())"
    ));
    let transition = include_str!("../../shared_memory/transitions.rs");
    let retain = transition
        .split("pub(super) fn retain_coherent_insertion_output_v1")
        .nth(1)
        .unwrap()
        .split("struct TransitionOwnersV1")
        .next()
        .unwrap();
    assert!(retain.contains("engine.phase = SharedMemorySessionPhaseV1::Quarantined"));
    assert!(retain.contains("engine.terminal_transition.is_some()"));
    assert!(retain.contains("core::mem::ManuallyDrop::new(completed)"));
    assert!(retain.contains("stage: TransitionStageV1::LiveInsertion"));
    assert!(retain.contains("output: Some(TerminalTokenV1::from_token(completed))"));
    for body in [root, forwarder, retain] {
        for forbidden in [
            "check_currentness",
            "retake_",
            "retain_v1(",
            "copy_from_slice",
            "to_vec()",
            "Box::new",
        ] {
            assert!(
                !body.contains(forbidden),
                "outer custody must not introduce {forbidden}"
            );
        }
    }
}

fn bound_dispatch_precedence(routes: core::ops::Range<usize>) {
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
    let (mut session, lane) = parent(false, false);
    session.dispatch = Some(preparation.take_completed().unwrap());
    let original = session
        .dispatch
        .as_ref()
        .unwrap()
        .primary_fixture_identities_v1();
    for count in [16, 17] {
        session.detached_data_count = count;
        session.detached_data_identities = vec![identity; count];
        if !routes.is_empty() {
            let snapshot = (
                session.detached_data_identities.clone(),
                session.detached_data_identities.as_ptr(),
                session.detached_data_identities.capacity(),
                session.detached_dispatch_generation,
                session.detached_next_insertion_index,
            );
            for route in routes.clone() {
                for source in [&[][..], &[0x49; 17][..]] {
                    assert!(matches!(
                        invoke_coherent_public(route, &mut session, lane, source),
                        Err(ComputeAqlQueueSessionErrorV1::DispatchBinding(
                            Gfx942DispatchBindingErrorV1::ResourcePhase
                        ))
                    ));
                    assert_eq!(
                        session
                            .dispatch
                            .as_ref()
                            .unwrap()
                            .primary_fixture_identities_v1(),
                        original
                    );
                    assert_eq!(
                        (
                            session.detached_data_identities.clone(),
                            session.detached_data_identities.as_ptr(),
                            session.detached_data_identities.capacity(),
                            session.detached_dispatch_generation,
                            session.detached_next_insertion_index
                        ),
                        snapshot
                    );
                    assert_eq!(session.detached_data_count, count);
                    assert!(!session.terminal_poisoned);
                }
            }
        }
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
        .split("pub(in crate::queue) fn settle_data_insertion_v1")
        .nth(1)
        .unwrap()
        .split("impl<R: DataInsertionRootV1<P>, P> DataInsertionContextV1<R, P>")
        .next()
        .unwrap();
    let mut previous = 0;
    for marker in [
        "context.require_unbound()?",
        "validate_new_detached_data_index",
        "context.reserve()?",
        "context.prepare(&mut root, parameters)?",
        "root.completed_identity()?",
        "insert_detached_identity_at",
        "*ledger.count = next_count",
        ".take_data()",
    ] {
        let at = sequence.find(marker).unwrap();
        assert!(at >= previous, "{marker} preserves ownership order");
        previous = at;
    }
    let adapter = source
        .split("impl<R: DataInsertionRootV1<P>, P> DataInsertionContextV1<R, P>")
        .nth(1)
        .unwrap();
    assert!(adapter.contains("self.with_live_queue_memory_model(|memory|"));
    assert!(adapter.contains("root.prepare(memory, parameters)"));
    assert!(adapter.contains("root.retain(&mut engine.backend.session)"));
    assert!(adapter.contains("self.poison_terminal()"));
    assert!(!adapter.contains("take_data"));
    let device_root = source
        .split("impl DataInsertionRootV1<u64>")
        .nth(1)
        .unwrap()
        .split("impl DataInsertionRootV1<&[u8]>")
        .next()
        .unwrap();
    assert!(device_root.contains("memory.prepare_device_initialization_in_place(self, alignment)"));
    assert!(device_root.contains("memory.retain_device_initialization_failure(self)"));
    assert!(device_root.contains("self.completed()?.storage_identity()"));
    assert!(device_root.contains(".map(Gfx942FixedDispatchDataV1::initialized)"));
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
