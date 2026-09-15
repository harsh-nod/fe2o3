//! Concrete release routing and restoration with genuine data tokens and no Linux queue engine.
use super::*;

pub(super) fn selected_parent(ordinal: usize) -> (ComputeAqlQueueSessionV1, ComputeAqlQueueLaneV1) {
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
    (session, lane)
}

pub(super) fn snapshot(s: &ComputeAqlQueueSessionV1) -> impl std::fmt::Debug + PartialEq + use<> {
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
        (
            s.sdma_device_pool.limits,
            s.sdma_device_pool.activity_started,
            s.sdma_host_pool_limits,
            s.sdma_pool_reuse_count,
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
        ),
    )
}

pub(super) fn poison_snapshot(s: &ComputeAqlQueueSessionV1) -> (bool, bool, Vec<Option<bool>>) {
    (
        s.terminal_poisoned,
        s.completion_owner.is_poisoned_for_test(),
        s.auxiliary_compute_lanes
            .iter()
            .map(|slot| {
                slot.state
                    .as_ref()
                    .map(|state| state.completion_owner.is_poisoned_for_test())
            })
            .collect(),
    )
}

fn preflight_setup(
    session: &mut ComputeAqlQueueSessionV1,
    identity: Gfx942FixedDispatchStorageIdentityV1,
    mode: usize,
) {
    session.detached_data_identities.push(identity);
    session.detached_data_count = 1;
    session.detached_next_insertion_index = Some(0);
    session.completion_owner.bind_barrier_probe().unwrap();
    match mode {
        0 => {
            session.terminal_poisoned = true;
            session.detached_dispatch_generation = None;
            session.detached_data_count = 17;
        }
        1 => {
            session.detached_dispatch_generation = None;
            session.detached_data_count = 17;
        }
        2 => session.detached_data_count = 17,
        3 => session.detached_data_count = 2,
        4 => session.detached_next_insertion_index = Some(2),
        5 => {}
        _ => unreachable!(),
    }
}

fn assert_preflight(error: ComputeAqlQueueSessionErrorV1, mode: usize) {
    match (mode, error) {
        (
            0,
            ComputeAqlQueueSessionErrorV1::DispatchBinding(Gfx942DispatchBindingErrorV1::Poisoned),
        )
        | (
            1,
            ComputeAqlQueueSessionErrorV1::DispatchBinding(
                Gfx942DispatchBindingErrorV1::ResourcePhase,
            ),
        )
        | (2, ComputeAqlQueueSessionErrorV1::Contract("detached dispatch-data ledger bound"))
        | (
            3 | 4,
            ComputeAqlQueueSessionErrorV1::Contract("detached dispatch-data identity ledger"),
        )
        | (5, ComputeAqlQueueSessionErrorV1::Completion(Gfx942CompletionErrorV1::Poisoned)) => {}
        (_, other) => panic!("unexpected release preflight error: {other:?}"),
    }
}

#[test]
fn detached_release_public_preflight_preserves_precedence_and_owners() {
    for route in 0usize..4 {
        for mode in 0..6 {
            let mut memory = crate::shared_memory::PreparationMemoryFixtureV1::new(true);
            let mut input = Some(memory.host(true));
            let identity = input.as_ref().unwrap().storage_identity();
            let memory_before = memory.observation();
            let (mut session, lane) = selected_parent(route.saturating_sub(1));
            let _ = take_dispatch_terminal_process_gate_record_v1();
            let _ = take_lane_unwind_process_gate_record_v1();
            if route == 0 {
                preflight_setup(&mut session, identity, mode);
                let before = snapshot(&session);
                let poison_before = poison_snapshot(&session);
                let error = session
                    .release_detached_fixed_dispatch_data(input.take().unwrap())
                    .unwrap_err();
                assert_preflight(error, mode);
                assert_eq!(snapshot(&session), before);
                assert_eq!(poison_snapshot(&session), poison_before);
            } else {
                session
                    .with_compute_lane_custody_v1(
                        lane,
                        |selected| {
                            // Inject after lane admission so this exercises release's own preflight.
                            preflight_setup(selected.session, identity, mode);
                            let before = snapshot(selected.session);
                            let poison_before = poison_snapshot(selected.session);
                            let error = selected
                                .release_detached_fixed_dispatch_data(input.take().unwrap())
                                .unwrap_err();
                            assert_preflight(error, mode);
                            assert!(!*selected.terminal_transport);
                            assert_eq!(snapshot(selected.session), before);
                            assert_eq!(poison_snapshot(selected.session), poison_before);
                        },
                        |_| panic!("preflight must not transfer the parent"),
                    )
                    .unwrap();
            }
            assert!(input.is_none());
            assert_eq!(memory.observation(), memory_before);
            assert_eq!(session.terminal_poisoned, mode == 0);
            assert!(!take_dispatch_terminal_process_gate_record_v1());
            assert!(!take_lane_unwind_process_gate_record_v1());
        }
    }
}

#[test]
fn detached_release_direct_missing_engine_transports_parent() {
    let mut memory = crate::shared_memory::PreparationMemoryFixtureV1::new(true);
    let data = memory.device(false);
    let (mut session, lane) = parent(false, false);
    session
        .detached_data_identities
        .push(data.storage_identity());
    session.detached_data_count = 1;
    let before = memory.observation();
    let _ = take_dispatch_terminal_process_gate_record_v1();
    let _ = take_lane_unwind_process_gate_record_v1();
    assert!(matches!(
        session.release_detached_fixed_dispatch_data(data),
        Err(ComputeAqlQueueSessionErrorV1::Contract(
            "missing queue engine"
        ))
    ));
    assert_eq!(memory.observation(), before);
    assert_shell(&mut session, lane);
    assert!(!take_dispatch_terminal_process_gate_record_v1());
    assert!(!take_lane_unwind_process_gate_record_v1());
}

#[test]
fn detached_release_facade_restores_before_transport_even_after_suppressed_error() {
    for ordinal in 0..3 {
        for failure in 0..3 {
            for mode in 0..4 {
                let mut memory = crate::shared_memory::PreparationMemoryFixtureV1::new(true);
                let mut first = Some(memory.host(true));
                let mut second = Some(memory.device(false));
                let first_id = first.as_ref().unwrap().storage_identity();
                let second_id = second.as_ref().unwrap().storage_identity();
                let (mut session, lane) = selected_parent(ordinal);
                session
                    .with_compute_lane_v1(lane, |selected| {
                        let ids: &[Gfx942FixedDispatchStorageIdentityV1] = match failure {
                            0 => &[first_id, second_id],
                            1 => &[second_id],
                            _ => &[first_id, first_id],
                        };
                        selected
                            .session
                            .detached_data_identities
                            .extend_from_slice(ids);
                        selected.session.detached_data_count = ids.len();
                        selected.session.detached_next_insertion_index = Some(1);
                    })
                    .unwrap();
                let before = snapshot(&session);
                let memory_before = memory.observation();
                let retained = RefCell::new(None);
                let transfers = Cell::new(0);
                let _ = take_dispatch_terminal_process_gate_record_v1();
                let _ = take_lane_unwind_process_gate_record_v1();
                let result = catch_unwind(AssertUnwindSafe(|| {
                    session.with_compute_lane_custody_v1(
                        lane,
                        |selected| {
                            let error = selected
                                .release_detached_fixed_dispatch_data(first.take().unwrap())
                                .unwrap_err();
                            match (failure, &error) {
                                (
                                    0,
                                    ComputeAqlQueueSessionErrorV1::Contract("missing queue engine"),
                                )
                                | (
                                    1,
                                    ComputeAqlQueueSessionErrorV1::DispatchBinding(
                                        Gfx942DispatchBindingErrorV1::InvalidData {
                                            index: 1,
                                            detail: "detached release storage identity",
                                        },
                                    ),
                                )
                                | (
                                    2,
                                    ComputeAqlQueueSessionErrorV1::Contract(
                                        "duplicate detached storage identity",
                                    ),
                                ) => {}
                                _ => panic!("unexpected release error: {error:?}"),
                            }
                            assert!(*selected.terminal_transport);
                            let retry = selected
                                .release_detached_fixed_dispatch_data(second.take().unwrap())
                                .unwrap_err();
                            assert!(matches!(
                                retry,
                                ComputeAqlQueueSessionErrorV1::DispatchBinding(
                                    Gfx942DispatchBindingErrorV1::Poisoned
                                )
                            ));
                            assert!(
                                *selected.terminal_transport,
                                "preflight cannot clear earlier transport"
                            );
                            match mode {
                                0 => Err(error),
                                1 => Ok(37),
                                2 => {
                                    assert!(
                                        catch_unwind(|| panic!("caught release caller panic"))
                                            .is_err()
                                    );
                                    Ok(37)
                                }
                                _ => panic!("escaping release caller panic"),
                            }
                        },
                        |root| {
                            transfers.set(transfers.get() + 1);
                            let parent = root.as_ref().as_ref().unwrap();
                            assert_eq!(
                                snapshot(parent),
                                before,
                                "restore exact selected slot before parent retention"
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
                        Some(&"escaping release caller panic")
                    );
                } else {
                    let admitted = match result {
                        Ok(admitted) => admitted,
                        Err(payload) => std::panic::resume_unwind(payload),
                    };
                    assert_eq!(admitted.unwrap().is_err(), mode == 0);
                }
                assert!(first.is_none() && second.is_none());
                assert_eq!(memory.observation(), memory_before);
                assert_eq!(transfers.get(), 1);
                assert!(!take_dispatch_terminal_process_gate_record_v1());
                assert_eq!(take_lane_unwind_process_gate_record_v1(), mode == 3);
                assert_shell(&mut session, lane);
                let retained = retained.into_inner().unwrap();
                assert_eq!(snapshot(retained.as_ref().as_ref().unwrap()), before);
            }
        }
    }
}

#[test]
fn detached_release_bound_dispatch_precedes_bad_ledger() {
    use crate::queue::dispatch_binding::{
        actual_persistent_control_test_program, prepare_public_fixed_dispatch_resources_in_place,
    };
    for route in 0usize..4 {
        let mut memory = crate::shared_memory::PreparationMemoryFixtureV1::new(true);
        let data = memory.roster();
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
        let original = preparation
            .completed()
            .unwrap()
            .primary_fixture_identities_v1();
        let mut dispatch = Some(preparation.take_completed().unwrap());
        let mut input = Some(memory.host(true));
        let memory_before = memory.observation();
        let (mut session, lane) = selected_parent(route.saturating_sub(1));
        let poison_before = poison_snapshot(&session);
        let setup = |session: &mut ComputeAqlQueueSessionV1, dispatch| {
            session.dispatch = dispatch;
            assert!(session.detached_dispatch_generation.is_some());
            session.detached_data_count = 17;
        };
        if route == 0 {
            setup(&mut session, dispatch.take());
            let before = snapshot(&session);
            assert_preflight(
                session
                    .release_detached_fixed_dispatch_data(input.take().unwrap())
                    .unwrap_err(),
                1,
            );
            assert_eq!(snapshot(&session), before);
            assert_eq!(
                session
                    .dispatch
                    .as_ref()
                    .unwrap()
                    .primary_fixture_identities_v1(),
                original
            );
        } else {
            session
                .with_compute_lane_custody_v1(
                    lane,
                    |selected| {
                        setup(selected.session, dispatch.take());
                        let before = snapshot(selected.session);
                        assert_preflight(
                            selected
                                .release_detached_fixed_dispatch_data(input.take().unwrap())
                                .unwrap_err(),
                            1,
                        );
                        assert!(!*selected.terminal_transport);
                        assert_eq!(snapshot(selected.session), before);
                        assert_eq!(
                            selected
                                .session
                                .dispatch
                                .as_ref()
                                .unwrap()
                                .primary_fixture_identities_v1(),
                            original
                        );
                    },
                    |_| panic!("bound-dispatch preflight transferred parent"),
                )
                .unwrap();
        }
        assert!(input.is_none());
        assert_eq!(poison_snapshot(&session), poison_before);
        if lane.ordinal == 0 {
            assert_eq!(
                session
                    .dispatch
                    .as_ref()
                    .unwrap()
                    .primary_fixture_identities_v1(),
                original
            );
        } else {
            assert!(session.dispatch.is_none());
        }
        for (index, slot) in session.auxiliary_compute_lanes.iter().enumerate() {
            if index + 1 == lane.ordinal {
                assert_eq!(
                    slot.state
                        .as_ref()
                        .unwrap()
                        .dispatch
                        .as_ref()
                        .unwrap()
                        .primary_fixture_identities_v1(),
                    original
                );
            } else if let Some(state) = &slot.state {
                assert!(state.dispatch.is_none());
            }
        }
        assert_eq!(memory.observation(), memory_before);
    }
}
