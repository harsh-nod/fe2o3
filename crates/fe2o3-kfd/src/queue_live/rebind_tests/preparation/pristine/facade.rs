//! Inject the same-memory preparation into the production-used lane settlement.

use super::*;

#[test]
fn pristine_rebind_facade_retains_restored_primary_and_later_auxiliary_slots() {
    for ordinal in 0..3 {
        for mode in 0..5 {
            let (f, data, continuation, occurrence) = fixture(7);
            let before = f.memory.observation();
            let vm = f.foundation.identity().vms()[0].key;
            let f = RefCell::new(f);
            let (mut session, mut lane) = parent_in_vm(ordinal != 0, false, vm);
            if ordinal == 2 {
                let mut key = test_queue_key(412, 9);
                key.vm = vm;
                session
                    .auxiliary_compute_lanes
                    .push(AuxiliaryComputeLaneSlotV1 {
                        generation: 11,
                        state: Some(compute_lane_state_for_multi_inflight_test(key)),
                    });
                lane.ordinal = 2;
                lane.generation = 11;
            }
            session
                .with_compute_lane_v1(lane, |selected| {
                    selected.session.detached_dispatch_generation = None;
                    selected.session.detached_data_count = data.len();
                    selected.session.detached_data_identities =
                        fixed_dispatch_storage_identities(&data);
                    selected.session.detached_next_insertion_index = Some(data.len());
                    selected.session.unpublished_dispatch.continuation = Some(continuation);
                    selected.session.observation.ring_bytes = 4096;
                })
                .unwrap();
            let snapshot = |s: &ComputeAqlQueueSessionV1| {
                (
                    (
                        s.key,
                        s.compute_lane_session,
                        s.observation,
                        s.detached_data_count,
                        s.detached_dispatch_generation,
                        s.detached_next_insertion_index,
                        s.detached_data_identities.as_ptr(),
                        s.detached_data_identities.capacity(),
                        s.detached_data_identities.clone(),
                    ),
                    (
                        s.completion_owner.custody_snapshot_for_test(),
                        s.dependency_owner.custody_snapshot_for_test(),
                    ),
                    (
                        s.auxiliary_compute_lanes.as_ptr(),
                        s.auxiliary_compute_lanes.capacity(),
                    ),
                    s.auxiliary_compute_lanes
                        .iter()
                        .map(|slot| {
                            let s = slot.state.as_ref().unwrap();
                            (
                                slot.generation,
                                s.key,
                                s.observation,
                                s.detached_data_count,
                                s.detached_dispatch_generation,
                                s.detached_next_insertion_index,
                                s.detached_data_identities.as_ptr(),
                                s.detached_data_identities.capacity(),
                                s.detached_data_identities.clone(),
                                s.completion_owner.custody_snapshot_for_test(),
                            )
                        })
                        .collect::<Vec<_>>(),
                    (
                        s.sdma_device_pool.limits,
                        s.sdma_device_pool.activity_started,
                        s.sdma_host_pool_limits,
                        s.sdma_pool_reuse_count,
                    ),
                )
            };
            let original = snapshot(&session);
            let packets = [packet(0), packet(1), packet(2)];
            let mut descriptors = PrimaryPreparationSnapshotV1::packets(&packets);
            descriptors.capture_data_vector_v1(&data, data.capacity());
            let programs = programs();
            let program_storage = (programs.as_ptr(), programs.len(), programs.capacity());
            let program_ids = programs
                .iter()
                .map(|p| (p.identity_inputs(), p.dispatch_abi_identity()))
                .collect::<Vec<_>>();
            let root = LiveRebindRootV1::new(programs, packets, data, None);
            let input_root = &*root as *const _;
            let retained_inputs = RefCell::new(None);
            let retained_parent = RefCell::new(None);
            let transfers = Cell::new(0);
            let _ = take_dispatch_terminal_process_gate_record_v1();
            let _ = take_lane_unwind_process_gate_record_v1();
            let result = catch_unwind(AssertUnwindSafe(|| {
                session.with_compute_lane_custody_v1(
                lane,
                |selected| {
                    let settled = selected.session.settle_fixed_dispatch_rebind_with_v1(
                        root,
                        |session, programs, preparation, predecessor, continuation| {
                            assert!(predecessor.is_none());
                            assert!(session.unpublished_dispatch.continuation.is_none());
                            assert_eq!(continuation.as_ref().unwrap().next_generation_for_test(), 7);
                            preparation.primary_assert_descriptors_v1(&descriptors, None);
                            preparation.primary_inject_stage_v1(PreparationStageV1::CodeResolve(0), mode == 2);
                            let mut f = f.borrow_mut();
                            let (result, closing) = execute_live_model_custody_v1(
                                &mut *f,
                                |f| {
                                    f.calls[0] += 1;
                                    f.memory.primary_loan(&mut f.foundation).map_err(ComputeAqlQueueSessionErrorV1::from)
                                },
                                |f| {
                                    f.calls[1] += 1;
                                    prepare_public_fixed_dispatch_resources_after_pristine_abort_in_place_v1(
                                        &mut f.memory, programs, preparation, continuation.take().unwrap(),
                                    ).map_err(Into::into)
                                },
                                |f, loan| {
                                    f.calls[2] += 1;
                                    f.memory.primary_reclaim(&mut f.foundation, loan).map_err(Into::into)
                                },
                                |f| f.poisoned = true,
                            )?;
                            closing?;
                            result
                        },
                        |_, _| panic!("failed preparation entered validation"),
                        |root| {
                            assert_eq!(&*root as *const _, input_root);
                            *retained_inputs.borrow_mut() = Some(root);
                        },
                    );
                    assert!(settled.transport);
                    *selected.terminal_transport |= settled.transport;
                    let outcome = catch_unwind(AssertUnwindSafe(|| settled.into_result()));
                    if mode == 2 {
                        assert_eq!(outcome.unwrap_err().downcast_ref::<(&str, PreparationStageV1)>(),
                            Some(&("dispatch preparation", PreparationStageV1::CodeResolve(0))));
                    } else {
                        assert!(matches!(outcome.unwrap(), Err(ComputeAqlQueueSessionErrorV1::DispatchBinding(
                            Gfx942DispatchBindingErrorV1::InvalidCode("injected preparation stage")))));
                    }
                    // Exercise the real public facade after the injected terminal operation.
                    let error = selected.bind_fixed_dispatch::<0>(Vec::new(), [], Vec::new()).unwrap_err();
                    assert!(matches!(error, ComputeAqlQueueSessionErrorV1::DispatchBinding(Gfx942DispatchBindingErrorV1::Poisoned)));
                    match mode {
                        0 => Err(error),
                        3 => {
                            let caught = catch_unwind(|| std::panic::panic_any("caught caller panic"));
                            assert_eq!(caught.unwrap_err().downcast_ref::<&str>(), Some(&"caught caller panic"));
                            Ok(37)
                        }
                        4 => std::panic::panic_any("escaping caller panic"),
                        _ => Ok(37),
                    }
                },
                |root| {
                    transfers.set(transfers.get() + 1);
                    let parent = root.as_ref().as_ref().unwrap();
                    assert!(parent.terminal_poisoned);
                    assert_eq!(snapshot(parent), original);
                    assert!(parent.completion_owner.is_poisoned_for_test());
                    for (index, slot) in parent.auxiliary_compute_lanes.iter().enumerate() {
                        let state = slot.state.as_ref().unwrap();
                        assert_eq!(state.completion_owner.is_poisoned_for_test(), ordinal == index + 1);
                        assert!(state.unpublished_dispatch.continuation.is_none());
                    }
                    assert!(parent.unpublished_dispatch.continuation.is_none());
                    *retained_parent.borrow_mut() = Some(root);
                },
            )
            }));
            if mode == 4 {
                assert_eq!(
                    result.unwrap_err().downcast_ref::<&str>(),
                    Some(&"escaping caller panic")
                );
            } else {
                let result = result.unwrap().unwrap();
                assert_eq!(result.is_err(), mode == 0);
                if mode != 0 {
                    assert_eq!(result.unwrap(), 37);
                }
            }
            assert_eq!(transfers.get(), 1);
            assert!(take_dispatch_terminal_process_gate_record_v1());
            assert_eq!(take_lane_unwind_process_gate_record_v1(), mode == 4);
            assert_shell(&mut session, lane);
            drop(session);
            let parent = retained_parent.into_inner().unwrap();
            assert_eq!(snapshot(parent.as_ref().as_ref().unwrap()), original);
            let root = retained_inputs.into_inner().unwrap();
            assert!(root.continuation.is_none());
            let programs = root.programs.as_ref().unwrap();
            assert_eq!(
                (programs.as_ptr(), programs.len(), programs.capacity()),
                program_storage
            );
            assert_eq!(
                programs
                    .iter()
                    .map(|p| (p.identity_inputs(), p.dispatch_abi_identity()))
                    .collect::<Vec<_>>(),
                program_ids
            );
            let preparation = root.preparation.as_ref().unwrap();
            preparation.primary_assert_descriptors_v1(&descriptors, None);
            preparation.primary_assert_failed_stage_v1(PreparationStageV1::CodeResolve(0));
            preparation.primary_assert_replacement_generation_v1(Some(7), None);
            preparation.assert_fresh_pristine_occurrence_for_test(occurrence);
            let mut owners = PreparationOwnerRefsV1::default();
            preparation.primary_collect_owners_v1(&mut owners);
            let f = f.borrow();
            assert_eq!(f.calls, [1, 1, 1, 0]);
            assert_eq!(f.poisoned, mode == 2);
            f.memory.primary_assert_device_partition_v1(
                &owners.device_leases,
                &owners.device_authorities,
            );
            f.memory.primary_assert_shared_layouts_v1(&owners.shared);
            let mut shared = owners.shared.iter().map(|(id, _)| *id).collect::<Vec<_>>();
            shared.extend(f.memory.primary_terminal_identities());
            let expected = f.memory.primary_identities();
            assert_eq!(shared.len(), expected.len());
            for id in expected {
                assert_eq!(shared.iter().filter(|&&owner| owner == id).count(), 1);
            }
            f.memory.primary_assert_accounts_after_disposal_v1(
                f.memory.primary_session_id(),
                before.calls[5..].try_into().unwrap(),
            );
            f.memory.assert_data_unchanged(&before);
        }
    }
}
