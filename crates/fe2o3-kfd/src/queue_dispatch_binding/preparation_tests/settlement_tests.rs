use super::*;

#[test]
fn preparation_bind_callers_root_before_loan_and_transfer_after_settlement() {
    let source = crate::queue::fixed_dispatch_production_source_for_tests_v1();
    let blocks: Vec<_> = source
        .split("let mut preparation = FixedDispatchPreparationCustodyV1::new")
        .skip(1)
        .collect();
    assert_eq!(blocks.len(), 2);
    for block in blocks {
        let construction = block
            .split("let binding = PersistentComputeBindingKeyV1")
            .next()
            .unwrap();
        let catch = construction
            .find("settle_persistent_bind_preparation_v1")
            .unwrap();
        let loan = construction.find("with_live_queue_memory_model").unwrap();
        let borrow = construction.find("&mut preparation").unwrap();
        let validation = construction
            .find("Self::validate_persistent_bind_preparation_v1")
            .unwrap();
        let transfer = construction.find("preparation.take_completed()").unwrap();
        assert!(catch < borrow && borrow < loan && loan < validation && validation < transfer);
        let compact: String = construction.split_whitespace().collect();
        let terminal_owners: Vec<_> = compact
            .split("PersistentComputeTerminalNativeCustodyV1::Preparation(")
            .skip(1)
            .map(|call| {
                let argument = call.split_once(')').unwrap().0;
                argument.strip_suffix(',').unwrap_or(argument)
            })
            .collect();
        assert_eq!(terminal_owners, ["preparation", "preparation"]);
        assert!(!construction.contains("failure.data"));
        assert!(!construction.contains("retained_data.take()"));
        let commit = block
            .split("let binding = PersistentComputeBindingKeyV1")
            .nth(1)
            .unwrap()
            .split("\n    }")
            .next()
            .unwrap();
        assert!(commit.contains("self.dispatch = Some(prepared_dispatch)"));
        assert!(commit.contains("self.next_persistent_compute_generation ="));
        for forbidden in [
            "?",
            "Err(",
            "with_live_queue_memory_model",
            "validate_",
            "push(",
        ] {
            assert!(
                !commit.contains(forbidden),
                "fallible bind commit: {forbidden}"
            );
        }
    }
    let validation = source
        .split("fn validate_persistent_bind_preparation_v1")
        .nth(1)
        .unwrap()
        .split("pub(in super::super) const fn has_any_persistent_compute_attachment_v1")
        .next()
        .unwrap();
    assert!(validation.contains("preparation.completed()?"));
    assert!(validation.contains("validate_live_queue_dispatch_memory"));
    assert!(!validation.contains("take_completed"));
}

#[test]
fn ordinary_constructor_root_preserves_real_preparation_before_control_entry() {
    use std::cell::RefCell;
    for prepared in [false, true] {
        for panics in [false, true] {
            let mut memory = Memory::new(true);
            let data = memory.roster();
            let expected = inputs(&data);
            let before = memory.observation();
            let owner = FixedDispatchPreparationCustodyV1::new([packet(0)], data);
            let root = Box::new((memory, owner));
            let original = &*root as *const _;
            let retained = RefCell::new(None);
            let result = catch_unwind(AssertUnwindSafe(|| {
                crate::queue::live::settle_queue_constructor_fixture_v1(
                    root,
                    |root| {
                        if prepared {
                            run(&mut root.1, &mut root.0)?;
                        }
                        if panics {
                            std::panic::panic_any("before USERPTR control");
                        }
                        validate_fixed_batch_ring::<0>(4096)?;
                        panic!("empty fixed batch unexpectedly accepted");
                    },
                    |root| *retained.borrow_mut() = Some(root),
                )
            }));
            match result {
                Err(payload) => {
                    assert!(panics);
                    assert_eq!(
                        payload.downcast_ref::<&str>(),
                        Some(&"before USERPTR control")
                    );
                }
                Ok(Err(error)) => assert!(!panics && !error.is_terminal_creation()),
                Ok(Ok(_)) => panic!("injected construction rejection succeeded"),
            }
            let root = retained.into_inner().unwrap();
            assert_eq!(&*root as *const _, original);
            assert_inputs(&root.1, &expected);
            assert_custody(&root.0, &root.1);
            assert_backing(&root.0, &before);
            assert_eq!(root.1.completed.is_some(), prepared);
            if !prepared {
                assert_eq!(root.0.observation(), before);
            }
        }
    }
}

#[test]
fn preparation_preserves_mixed_roster_and_full_successful_images() {
    for configured in [false, true] {
        let mut memory = Memory::new(configured);
        let data = memory.roster();
        let expected = inputs(&data);
        let before = memory.observation();
        let mut owner = FixedDispatchPreparationCustodyV1::new([packet(2)], data);
        run(&mut owner, &mut memory).unwrap();
        assert_inputs(&owner, &expected);
        assert_custody(&memory, &owner);
        assert_backing(&memory, &before);
        let completed = owner.completed.as_ref().unwrap();
        assert_eq!(completed.code.len(), 3);
        assert_eq!(completed.code_identity.len(), 3);
        for ((authority, identity), kernel) in completed
            .code
            .iter()
            .zip(&completed.code_identity)
            .zip(programs())
        {
            let mut expected = vec![0; authority.layout().requested_bytes()];
            kernel.materialize_into(&mut expected).unwrap();
            let actual = memory.mapped_bytes(authority.facts().mapping());
            assert_eq!(actual, expected);
            assert_eq!(
                identity.materialized_sha256,
                <[u8; 32]>::from(Sha256::digest(actual))
            );
            assert_eq!(identity.authenticated, kernel.identity_inputs());
            assert_eq!(
                identity.dispatch_abi_identity,
                kernel.dispatch_abi_identity().unwrap()
            );
            assert_eq!(identity.mapping, authority.facts().mapping());
            let descriptor_offset = kernel
                .selected_binding()
                .descriptor_address()
                .checked_sub(kernel.envelope().plan().image_start())
                .unwrap();
            assert_eq!(
                identity.descriptor_address,
                ObservedGpuAddressV1::new(
                    authority
                        .facts()
                        .checked_gpu_subrange(descriptor_offset, KERNEL_DESCRIPTOR_BYTES_V1, 64,)
                        .unwrap()
                )
                .unwrap()
            );
        }
        let mut expected_kernarg = [0; 16];
        expected_kernarg[..8].copy_from_slice(
            &completed.data[0]
                .checked_gpu_subrange(0, 4096, 1)
                .unwrap()
                .to_le_bytes(),
        );
        expected_kernarg[8..].copy_from_slice(&1024_u64.to_le_bytes());
        assert_eq!(
            memory.mapped_bytes(completed.kernarg.facts().mapping()),
            expected_kernarg
        );
        assert_eq!(completed.packets[0].code_index, 2);
        assert_eq!(
            completed.packets[0].kernarg_layout_identity,
            programs()[2].dispatch_abi_identity().unwrap()
        );
        assert!(completed.data_premises[0].initialized_content.is_some());
        let dispatch = owner.take_completed().unwrap();
        assert_eq!(dispatch.data.len(), 4);
        assert!(owner.take_completed().is_err());
        assert!(run(&mut owner, &mut memory).is_err());
    }
}

#[test]
fn preparation_every_program_stage_retains_exact_prefix_on_error_and_panic() {
    for configured in [false, true] {
        for index in 0..3 {
            for stage in [
                PreparationStageV1::CodeAllocate(index),
                PreparationStageV1::CodeMaterialize(index),
                PreparationStageV1::CodeSeal(index),
                PreparationStageV1::CodeMap(index),
                PreparationStageV1::CodeRetain(index),
                PreparationStageV1::CodeResolve(index),
            ] {
                for panic in [false, true] {
                    stage_fault(stage, panic, configured);
                }
            }
        }
    }
}

#[test]
fn preparation_kernarg_and_commit_stages_retain_all_controls() {
    for configured in [false, true] {
        for stage in [
            PreparationStageV1::KernargAllocate,
            PreparationStageV1::KernargMaterialize,
            PreparationStageV1::KernargMap,
            PreparationStageV1::KernargRetain,
            PreparationStageV1::PacketResolve(0),
            PreparationStageV1::Commit,
            PreparationStageV1::Complete,
        ] {
            for panic in [false, true] {
                stage_fault(stage, panic, configured);
            }
        }
    }
}

#[test]
fn preparation_pre_effect_stage_failures_leave_native_state_unchanged() {
    for configured in [false, true] {
        for stage in [
            PreparationStageV1::Generation,
            PreparationStageV1::Plan,
            PreparationStageV1::Capacity,
            PreparationStageV1::DataRetention,
        ] {
            for panic in [false, true] {
                stage_fault(stage, panic, configured);
            }
        }
    }
}

#[test]
fn preparation_allocation_native_failures_keep_prior_code_and_pending_custody() {
    for configured in [false, true] {
        for call in [
            Call::AllocateCode(0),
            Call::AllocateCode(1),
            Call::AllocateCode(2),
            Call::AllocateKernarg,
        ] {
            for operation in ["reserve_va", "alloc", "map_cpu", "prepare_cpu_mapping"] {
                native_fault(call, NativeFault::Error(operation), false, configured);
                native_fault(call, NativeFault::Panic(operation), true, configured);
            }
            for delta in 1..=3 {
                native_fault(
                    call,
                    NativeFault::CurrentnessError(delta),
                    false,
                    configured,
                );
                native_fault(call, NativeFault::CurrentnessPanic(delta), true, configured);
            }
        }
    }
}

#[test]
fn preparation_allocation_projection_failure_keeps_actual_returned_control() {
    for configured in [false, true] {
        for call in [
            Call::AllocateCode(0),
            Call::AllocateCode(1),
            Call::AllocateCode(2),
            Call::AllocateKernarg,
        ] {
            native_fault(call, NativeFault::ProjectionRejection, false, configured);
        }
    }
}

#[test]
fn preparation_seal_native_and_currentness_failures_keep_exact_session_handoff() {
    for configured in [false, true] {
        for index in 0..3 {
            let call = Call::SealCode(index);
            native_fault(
                call,
                NativeFault::Error("protect_cpu_read_only"),
                false,
                configured,
            );
            native_fault(
                call,
                NativeFault::Panic("protect_cpu_read_only"),
                true,
                configured,
            );
            for delta in [1, 2] {
                native_fault(
                    call,
                    NativeFault::CurrentnessError(delta),
                    false,
                    configured,
                );
                native_fault(call, NativeFault::CurrentnessPanic(delta), true, configured);
            }
        }
    }
}

#[test]
fn preparation_partial_mapping_keeps_current_session_token_and_prefix() {
    for configured in [false, true] {
        for call in [
            Call::MapCode(0),
            Call::MapCode(1),
            Call::MapCode(2),
            Call::MapKernarg,
        ] {
            for prefix in 0..=2 {
                for errno in [false, true] {
                    if prefix != 1 || errno {
                        native_fault(
                            call,
                            NativeFault::PartialMap(prefix, errno),
                            false,
                            configured,
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn preparation_mapping_panics_and_currentness_failures_keep_exact_custody() {
    for configured in [false, true] {
        for call in [
            Call::MapCode(0),
            Call::MapCode(1),
            Call::MapCode(2),
            Call::MapKernarg,
        ] {
            native_fault(call, NativeFault::Panic("map_gpu"), true, configured);
            for delta in [1, 2] {
                native_fault(
                    call,
                    NativeFault::CurrentnessError(delta),
                    false,
                    configured,
                );
                native_fault(call, NativeFault::CurrentnessPanic(delta), true, configured);
            }
        }
    }
}

#[test]
fn preparation_materialization_access_and_currentness_panics_keep_borrowed_token() {
    for configured in [false, true] {
        for call in [
            Call::WriteCode(0),
            Call::WriteCode(1),
            Call::WriteCode(2),
            Call::WriteKernarg,
        ] {
            native_fault(call, NativeFault::AccessPanic, true, configured);
            for delta in [1, 2] {
                native_fault(
                    call,
                    NativeFault::CurrentnessError(delta),
                    false,
                    configured,
                );
                native_fault(call, NativeFault::CurrentnessPanic(delta), true, configured);
            }
        }
    }
}

#[test]
fn preparation_invalid_generation_and_plan_keep_original_inputs() {
    for invalid_generation in [false, true] {
        let mut memory = Memory::new(true);
        let data = memory.roster();
        let expected = inputs(&data);
        let before = memory.observation();
        let mut owner = FixedDispatchPreparationCustodyV1::new(
            [packet(if invalid_generation { 0 } else { 3 })],
            data,
        );
        let generation = if invalid_generation {
            DispatchGenerationOwnerV1::after_recycled(u64::MAX)
        } else {
            DispatchGenerationOwnerV1::new()
        };
        assert!(
            owner
                .prepare_in_place(
                    &mut memory,
                    &programs(),
                    generation,
                    PersistentFixedDispatchControlStateV1::Ordinary
                )
                .is_err()
        );
        assert_inputs(&owner, &expected);
        assert_eq!(memory.observation(), before);
    }
}

#[test]
fn preparation_foreign_data_rejection_keeps_the_complete_original_roster() {
    let mut memory = Memory::new(true);
    let mut foreign = Memory::new(true);
    let mut data = memory.roster();
    let displaced = core::mem::replace(&mut data[2], foreign.device(false));
    let expected = inputs(&data);
    let before = memory.observation();
    let mut owner = FixedDispatchPreparationCustodyV1::new([packet(0)], data);
    assert!(run(&mut owner, &mut memory).is_err());
    assert_inputs(&owner, &expected);
    assert_eq!(memory.observation(), before);
    assert!(!displaced.is_fully_initialized());
}

#[test]
fn preparation_completed_owner_survives_caller_unwind_before_transfer() {
    let mut memory = Memory::new(true);
    let data = memory.roster();
    let expected = inputs(&data);
    let mut owner = FixedDispatchPreparationCustodyV1::new([packet(2)], data);
    let result = catch_unwind(AssertUnwindSafe(|| {
        run(&mut owner, &mut memory).unwrap();
        std::panic::panic_any("caller after preparation");
    }));
    assert_eq!(
        result.unwrap_err().downcast_ref::<&str>(),
        Some(&"caller after preparation")
    );
    assert_inputs(&owner, &expected);
    assert_custody(&memory, &owner);
    assert_eq!(owner.completed.as_ref().unwrap().code.len(), 3);
}

#[test]
fn preparation_late_packet_resolution_keeps_earlier_packet_and_all_controls() {
    let mut memory = Memory::new(true);
    let data = memory.roster();
    let expected = inputs(&data);
    let mut owner = FixedDispatchPreparationCustodyV1::new([packet(0), packet(2)], data);
    owner.fault = Some((PreparationStageV1::PacketResolve(1), true));
    assert!(catch_unwind(AssertUnwindSafe(|| run(&mut owner, &mut memory))).is_err());
    assert_eq!(owner.prepared_packets.len(), 1);
    assert_eq!(owner.code.len(), 3);
    assert_inputs(&owner, &expected);
    assert_custody(&memory, &owner);
}

#[test]
fn preparation_single_persistent_wrapper_preserves_role_and_generation() {
    check_single_bind_settlement(None, BindFault::None, BindFault::None);
}

#[test]
fn persistent_single_bind_settlement_retains_real_data_controls_and_generation() {
    for fault in [
        None,
        Some((PreparationStageV1::CodeResolve(0), false)),
        Some((PreparationStageV1::CodeResolve(0), true)),
    ] {
        for closing in [BindFault::None, BindFault::Error, BindFault::Panic] {
            check_single_bind_settlement(fault, closing, BindFault::None);
        }
    }
    for validation in [BindFault::Error, BindFault::Panic] {
        check_single_bind_settlement(None, BindFault::None, validation);
    }
}

#[test]
fn preparation_terminal_variant_retains_actual_completed_owner() {
    let mut memory = Memory::new(true);
    let data = memory.roster();
    let expected = inputs(&data);
    let mut owner = FixedDispatchPreparationCustodyV1::new([packet(2)], data);
    run(&mut owner, &mut memory).unwrap();
    let terminal =
        crate::persistent_compute::PersistentComputeTerminalNativeCustodyV1::Preparation(owner);
    assert_eq!(
        terminal.stage(),
        Some(crate::persistent_compute::Gfx942PersistentComputeTerminalStageV1::Preparing)
    );
    let crate::persistent_compute::PersistentComputeTerminalNativeCustodyV1::Preparation(owner) =
        terminal
    else {
        unreachable!()
    };
    assert_inputs(&owner, &expected);
    assert_custody(&memory, &owner);
}

#[test]
fn preparation_generation_owner_precedes_the_first_fallible_stage() {
    for panic in [false, true] {
        let mut memory = Memory::new(true);
        let mut owner = FixedDispatchPreparationCustodyV1::new([packet(0)], memory.roster());
        owner.fault = Some((PreparationStageV1::Generation, panic));
        let generation = DispatchGenerationOwnerV1::new().unwrap();
        let expected = (
            generation.recipe_occurrence,
            generation.next_generation,
            generation.slots.as_ptr(),
        );
        let result = catch_unwind(AssertUnwindSafe(|| {
            owner.prepare_in_place(
                &mut memory,
                &programs(),
                Ok(generation),
                PersistentFixedDispatchControlStateV1::Ordinary,
            )
        }));
        assert!(result.is_err() || result.unwrap().is_err());
        let retained = owner.generation.as_ref().unwrap();
        assert_eq!(
            (
                retained.recipe_occurrence,
                retained.next_generation,
                retained.slots.as_ptr()
            ),
            expected
        );
        assert!(owner.failed);
        assert_eq!(memory.observation().controls, 0);
    }
}

#[test]
fn preparation_three_binding_wrapper_preserves_exact_roles_and_data_effects() {
    for stage in [
        None,
        Some(PreparationStageV1::CodeResolve(0)),
        Some(PreparationStageV1::KernargRetain),
    ] {
        check_three_bind_settlement(
            stage.map(|stage| (stage, false)),
            BindFault::None,
            BindFault::None,
        );
    }
}

#[test]
fn persistent_three_bind_settlement_retains_real_roster_controls_roles_and_generation() {
    for fault in [
        None,
        Some((PreparationStageV1::CodeResolve(0), false)),
        Some((PreparationStageV1::CodeResolve(0), true)),
    ] {
        for closing in [BindFault::None, BindFault::Error, BindFault::Panic] {
            check_three_bind_settlement(fault, closing, BindFault::None);
        }
    }
    for validation in [BindFault::Error, BindFault::Panic] {
        check_three_bind_settlement(None, BindFault::None, validation);
    }
}

#[test]
fn persistent_write_replay_replaces_stale_initialization_with_actual_incoming_storage() {
    use control_release::{ReturningControlCleanupCustodyV1, ReturningControlModeV1};
    for initialized in [false, true] {
        let mut memory = Memory::new(true);
        let data = memory.device(true);
        let storage = data.sdma_storage_identity();
        let mut dispatch = persistent_cancel_control_in_memory_v1(
            &mut memory,
            super::super::super::tests::persistent_control_test_queue(45),
            vec![data],
            None,
        );
        let PersistentFixedDispatchControlStateV1::Attached(identity) = dispatch.persistent_control
        else {
            panic!("attached")
        };
        let identity = identity.as_single().unwrap();
        assert_eq!(identity.effect, DeviceDataEffectV1::WriteOnly);
        let (predecessor, mut detached) = recycle_and_detach_persistent_fixture_v1(&mut dispatch);
        assert!(dispatch.data_premises[0].fully_initialized);
        let DispatchDataInputStorageV1::Device(lease) =
            detached.pop().unwrap().into_parts().storage
        else {
            panic!("device")
        };
        let mut incoming = Some(if initialized {
            Gfx942FixedDispatchDataV1::initialized_storage(lease)
        } else {
            Gfx942FixedDispatchDataV1::uninitialized(lease)
        });
        let code: Vec<_> = dispatch.code.iter().map(Memory::code_identity).collect();
        let kernarg = Memory::kernarg_identity(&dispatch.kernarg);
        let occurrence = dispatch.generation.recipe_occurrence;
        dispatch
            .retain_persistent_replay_data_with_v1(identity, &mut incoming, predecessor, |data| {
                memory.retain_replay(data, || {})
            })
            .unwrap();
        assert!(incoming.is_none());
        assert_eq!(dispatch.data_premises[0].fully_initialized, initialized);
        assert!(dispatch.data_premises[0].initialized_content.is_none());
        assert_eq!(
            dispatch
                .code
                .iter()
                .map(Memory::code_identity)
                .collect::<Vec<_>>(),
            code
        );
        assert_eq!(Memory::kernarg_identity(&dispatch.kernarg), kernarg);
        assert_eq!(dispatch.generation.recipe_occurrence, occurrence);
        let mut cleanup = ReturningControlCleanupCustodyV1::new(
            dispatch,
            ReturningControlModeV1::PersistentBeforePublication,
        );
        cleanup.release_in_place(&mut memory).unwrap();
        let (_, mut returned) = cleanup.take_persistent_data().unwrap();
        let data = returned.pop().unwrap();
        assert_eq!(data.sdma_storage_identity(), storage);
        assert_eq!(data.is_fully_initialized(), initialized);
        assert!(data.initialized_content().is_none());
        let mut data = crate::shared_memory::DataCleanupCustodyV1::new(data);
        crate::shared_memory::DispatchDataReleaseV1::release_data(&mut memory, &mut data).unwrap();
        assert!(data.is_complete());
    }
}

#[test]
fn persistent_replay_settlement_keeps_real_control_and_data_without_rebuilding() {
    use crate::queue::live::model_loan::execute_live_model_custody_v1;
    for fault in [BindFault::None, BindFault::Error, BindFault::Panic] {
        for closing in [BindFault::None, BindFault::Error, BindFault::Panic] {
            let mut memory = Memory::new(true);
            let data = memory.device(true);
            let descriptor = data.initialized_content().unwrap();
            let storage_identity = data.sdma_storage_identity();
            let programs = programs();
            let packets = [packet(0)];
            let identity = persistent_fixed_dispatch_control_identity_v1(
                super::super::super::tests::persistent_control_test_queue(45),
                &programs,
                &packets,
                data.layout(),
                true,
                Gfx942DeviceContentRoleV1::new([0x61; 32], 0).unwrap(),
                storage_identity,
            )
            .unwrap();
            let mut preparation = FixedDispatchPreparationCustodyV1::new(packets, vec![data]);
            prepare_persistent_fixed_dispatch_resources_v1(
                &mut memory,
                &programs,
                &mut preparation,
                None,
                identity,
            )
            .unwrap();
            let mut dispatch = preparation.take_completed().unwrap();
            // Model-only completion: no GPU execution or content result is claimed.
            let generation = dispatch.generation.next().unwrap();
            dispatch.generation.commit_begin(generation);
            dispatch.generation.complete(generation).unwrap();
            dispatch.generation.recycle(generation).unwrap();
            let (predecessor, mut detached) = dispatch
                .detach_persistent_replay_data_after_recycle_v1()
                .unwrap();
            let input = detached.pop().unwrap().into_parts();
            let DispatchDataInputStorageV1::Device(lease) = input.storage else {
                unreachable!()
            };
            // The fixture's bytes did not execute; reuse their actual descriptor.
            let initialized =
                match Gfx942InitializedDeviceMemoryV1::from_authenticated_full_transfer(
                    lease, descriptor,
                ) {
                    Ok(initialized) => initialized,
                    Err(_) => panic!("exact original extent"),
                };
            let mut data = Some(Gfx942FixedDispatchDataV1::initialized(initialized));
            let before = memory.observation();
            let code: Vec<_> = dispatch.code.iter().map(Memory::code_identity).collect();
            let kernarg = Memory::kernarg_identity(&dispatch.kernarg);
            let capacity = dispatch.data.capacity();
            let occurrence = dispatch.generation.recipe_occurrence;
            let mut poisoned = false;
            let result = catch_unwind(AssertUnwindSafe(|| {
                execute_live_model_custody_v1(
                    &mut memory,
                    |_| Ok(()),
                    |memory| {
                        dispatch.retain_persistent_replay_data_with_v1(
                            identity,
                            &mut data,
                            predecessor,
                            |data| {
                                if fault == BindFault::Error {
                                    return Err(MemorySessionError::InvalidDeviceMemoryAuthority);
                                }
                                memory.retain_replay(data, || {
                                    if fault == BindFault::Panic {
                                        std::panic::panic_any("replay validation");
                                    }
                                })
                            },
                        )
                    },
                    |_, ()| match closing {
                        BindFault::None => Ok(()),
                        BindFault::Error => Err(Gfx942DispatchBindingErrorV1::Poisoned),
                        BindFault::Panic => std::panic::panic_any("replay retake"),
                    },
                    |_| poisoned = true,
                )
            }));
            if fault == BindFault::Panic || closing == BindFault::Panic {
                let payload = result.unwrap_err();
                assert_eq!(
                    payload.downcast_ref::<&str>(),
                    Some(&if fault == BindFault::Panic {
                        "replay validation"
                    } else {
                        "replay retake"
                    })
                );
            } else {
                let (operation, retake) = result.unwrap().unwrap();
                assert_eq!(operation.is_err(), fault == BindFault::Error);
                assert_eq!(retake.is_err(), closing == BindFault::Error);
            }
            assert_eq!(
                poisoned,
                fault == BindFault::Panic || closing != BindFault::None
            );
            assert_eq!(memory.observation(), before);
            assert_eq!(
                dispatch
                    .code
                    .iter()
                    .map(Memory::code_identity)
                    .collect::<Vec<_>>(),
                code
            );
            assert_eq!(Memory::kernarg_identity(&dispatch.kernarg), kernarg);
            assert_eq!(dispatch.generation.recipe_occurrence, occurrence);
            assert_eq!(
                dispatch.generation.returned_generation().unwrap(),
                predecessor
            );
            assert_eq!(dispatch.data.capacity(), capacity);
            if fault == BindFault::None {
                assert!(data.is_none());
                assert_eq!(dispatch.data.len(), 1);
                assert_eq!(Memory::data_storage(&dispatch.data[0]), storage_identity);
                assert_eq!(
                    dispatch.data_premises[0].initialized_content,
                    Some(descriptor)
                );
                assert!(!dispatch.persistent_data_is_detached_v1());
            } else {
                assert!(dispatch.data.is_empty());
                let data = data
                    .as_ref()
                    .expect("rejected replay retains original token");
                assert_eq!(data.sdma_storage_identity(), storage_identity);
                assert_eq!(data.initialized_content(), Some(descriptor));
                assert!(dispatch.persistent_data_is_detached_v1());
            }
        }
    }
}
