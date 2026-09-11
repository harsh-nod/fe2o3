//! Real planner/loader and production sequencer; native calls remain CPU fixtures.

use super::*;
use crate::shared_memory::{
    PreparationMemoryCallV1 as Call, PreparationMemoryFixtureV1 as Memory,
    PreparationMemoryObservationV1, PreparationNativeFaultV1 as NativeFault,
    SharedMemorySessionPhaseV1,
};
use std::panic::{AssertUnwindSafe, catch_unwind};

#[test]
fn preparation_bind_callers_root_before_loan_and_transfer_after_settlement() {
    let source = include_str!("../queue_live/fixed_dispatch.rs");
    let blocks: Vec<_> = source
        .split("let mut preparation = FixedDispatchPreparationCustodyV1::new")
        .skip(1)
        .collect();
    assert_eq!(blocks.len(), 2);
    for block in blocks {
        let construction = block.split("let validation = {").next().unwrap();
        let catch = construction.find("std::panic::catch_unwind").unwrap();
        let loan = construction
            .find("with_live_queue_memory_model_custody")
            .unwrap();
        let borrow = construction.find("&mut preparation").unwrap();
        let transfer = construction.find("preparation.take_completed()").unwrap();
        assert!(catch < loan && loan < borrow && borrow < transfer);
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
    }
}

const IMAGE: &[u8] = include_bytes!(
    "../../../fe2o3-runtime/fixtures/trusted-gfx942-inplace-transform-v1/inplace_transform.hsaco"
);

fn programs() -> Vec<ValidatedKernelEnvelope<'static>> {
    (1..=3)
        .map(|index| {
            super::super::tests::actual_persistent_control_test_program(IMAGE, [index; 32])
        })
        .collect()
}

fn packet(program_index: usize) -> Gfx942FixedDispatchPacketV1 {
    let mut bytes = [0; 16];
    bytes[8..].copy_from_slice(&1024_u64.to_le_bytes());
    Gfx942FixedDispatchPacketV1::new(
        program_index,
        AqlDispatchGeometryV1::new([256, 1, 1], [256, 1, 1]).unwrap(),
        0,
        bytes.into(),
        vec![Gfx942DispatchBufferBindingV1::new(0, 0, 0, 4096)].into_boxed_slice(),
    )
}

#[derive(Debug, Eq, PartialEq)]
struct Input {
    identity: Gfx942FixedDispatchStorageIdentityV1,
    storage: Gfx942SdmaBufferStorageIdentityV1,
    layout: Gfx942FixedDispatchDataLayoutV1,
    initialized: bool,
    content: Option<Gfx942DeviceContentDescriptorV1>,
}

fn inputs(data: &[Gfx942FixedDispatchDataV1]) -> Vec<Input> {
    data.iter()
        .map(|d| Input {
            identity: d.storage_identity(),
            storage: d.sdma_storage_identity(),
            layout: d.layout(),
            initialized: d.is_fully_initialized(),
            content: d.initialized_content(),
        })
        .collect()
}

fn assert_inputs<const N: usize>(owner: &FixedDispatchPreparationCustodyV1<N>, expected: &[Input]) {
    if let Some(completed) = &owner.completed {
        assert_eq!(completed.data.len(), expected.len());
        assert_eq!(completed.data_premises.len(), expected.len());
        for (index, ((authority, premise), input)) in completed
            .data
            .iter()
            .zip(&completed.data_premises)
            .zip(expected)
            .enumerate()
        {
            assert_eq!(Memory::data_storage(authority), input.storage);
            assert_eq!(premise.layout, input.layout);
            assert_eq!(premise.initialized_content, input.content);
            assert_eq!(premise.fully_initialized, input.initialized);
            assert_eq!(premise.valid_bytes, input.layout.requested_bytes());
            assert_eq!(
                premise.effect,
                (index == 0).then_some(DeviceDataEffectV1::ReadWrite)
            );
            let ranges = if index == 0 {
                vec![
                    CompletedWritableRangeV1 {
                        offset: 0,
                        byte_len: 4096
                    };
                    N
                ]
            } else {
                Vec::new()
            };
            assert_eq!(premise.writable_ranges.as_ref(), ranges);
            assert!(premise.completed_snapshots.is_empty());
        }
    } else if let Some(retained) = &owner.retained_data {
        assert_eq!(retained.len(), expected.len());
        for (retained, input) in retained.iter().zip(expected) {
            assert_eq!(Memory::data_storage(&retained.authority), input.storage);
            assert_eq!(retained.layout, input.layout);
            assert_eq!(retained.initialized_content, input.content);
            assert_eq!(retained.fully_initialized, input.initialized);
        }
        let plan = owner.plan.as_ref().unwrap();
        assert_eq!(plan.data.len(), expected.len());
        for (index, plan) in plan.data.iter().enumerate() {
            assert_eq!(plan.layout, expected[index].layout);
            assert_eq!(
                plan.effect,
                (index == 0).then_some(DeviceDataEffectV1::ReadWrite)
            );
            if index == 0 {
                assert_eq!(
                    plan.writable_ranges.as_ref(),
                    vec![
                        CompletedWritableRangeV1 {
                            offset: 0,
                            byte_len: 4096
                        };
                        N
                    ]
                );
            } else {
                assert!(plan.writable_ranges.is_empty());
            }
            assert!(plan.completed_snapshots.is_empty());
        }
    } else {
        assert_eq!(inputs(&owner.original_data), expected);
    }
}

fn assert_custody<const N: usize>(memory: &Memory, owner: &FixedDispatchPreparationCustodyV1<N>) {
    let mut ids = owner
        .code
        .iter()
        .map(Memory::code_identity)
        .collect::<Vec<_>>();
    let mut markers = Vec::new();
    match &owner.current_code {
        CodeStageV1::Cpu(t) => ids.push(t.storage_identity()),
        CodeStageV1::Immutable(t) => ids.push(t.storage_identity()),
        CodeStageV1::Mapped(t) => ids.push(t.storage_identity()),
        CodeStageV1::Retained(a) => ids.push(Memory::code_identity(a)),
        CodeStageV1::InSession(id) => markers.push(*id),
        CodeStageV1::Empty | CodeStageV1::Allocating => {}
    }
    match &owner.kernarg {
        KernargStageV1::Cpu(t) => ids.push(t.storage_identity()),
        KernargStageV1::Mapped(t) => ids.push(t.storage_identity()),
        KernargStageV1::Retained(a) => ids.push(Memory::kernarg_identity(a)),
        KernargStageV1::InSession(id) => markers.push(*id),
        KernargStageV1::Empty | KernargStageV1::Allocating => {}
    }
    if let Some(completed) = &owner.completed {
        ids.extend(completed.code.iter().map(Memory::code_identity));
        ids.push(Memory::kernarg_identity(&completed.kernarg));
    }
    let allocating = if matches!(owner.current_code, CodeStageV1::Allocating) {
        Some(crate::shared_memory::SharedGttProfileV1::Executable)
    } else if matches!(owner.kernarg, KernargStageV1::Allocating) {
        Some(crate::shared_memory::SharedGttProfileV1::Kernarg)
    } else {
        None
    };
    memory.assert_control_custody(&ids, &markers, allocating);
}

fn assert_backing(memory: &Memory, before: &PreparationMemoryObservationV1) {
    memory.assert_data_unchanged(before);
    let after = memory.observation();
    assert_eq!(after.host, before.host);
    assert_eq!(after.device, before.device);
    assert_eq!(
        &after.calls[5..],
        &before.calls[5..],
        "preparation must not clean up or refund"
    );
    let allocations = after.calls[2] - before.calls[2];
    assert!(allocations == after.controls || after.pending && allocations == after.controls + 1);
}

fn run<const N: usize>(
    owner: &mut FixedDispatchPreparationCustodyV1<N>,
    memory: &mut Memory,
) -> Result<(), Gfx942DispatchBindingErrorV1> {
    owner.prepare_in_place(
        memory,
        &programs(),
        DispatchGenerationOwnerV1::new(),
        PersistentFixedDispatchControlStateV1::Ordinary,
    )
}

fn stage_fault(stage: PreparationStageV1, panic: bool, configured: bool) {
    let mut memory = Memory::new(configured);
    let data = memory.roster();
    let expected = inputs(&data);
    let before = memory.observation();
    let mut owner = FixedDispatchPreparationCustodyV1::new([packet(2)], data);
    let original_bytes = owner.packets[0].kernarg_bytes.clone();
    owner.fault = Some((stage, panic));
    let result = catch_unwind(AssertUnwindSafe(|| run(&mut owner, &mut memory)));
    if panic {
        assert_eq!(
            result
                .unwrap_err()
                .downcast_ref::<(&str, PreparationStageV1)>(),
            Some(&("dispatch preparation", stage))
        );
    } else {
        assert!(result.unwrap().is_err());
    }
    assert_eq!(owner.stage, stage);
    assert!(owner.failed);
    assert!(owner.take_completed().is_err());
    assert_eq!(owner.packets[0].kernarg_bytes, original_bytes);
    assert_inputs(&owner, &expected);
    assert_custody(&memory, &owner);
    assert_backing(&memory, &before);
    assert_eq!(
        memory.observation().phase,
        if owner.native_started {
            SharedMemorySessionPhaseV1::Quarantined
        } else {
            SharedMemorySessionPhaseV1::Active
        }
    );
    if !owner.native_started {
        assert_eq!(memory.observation(), before);
    }
    let (prefix, controls) = match stage {
        PreparationStageV1::Generation
        | PreparationStageV1::Plan
        | PreparationStageV1::Capacity
        | PreparationStageV1::DataRetention => (0, 0),
        PreparationStageV1::CodeAllocate(index) => (index, index),
        PreparationStageV1::CodeMaterialize(index)
        | PreparationStageV1::CodeSeal(index)
        | PreparationStageV1::CodeMap(index)
        | PreparationStageV1::CodeRetain(index)
        | PreparationStageV1::CodeResolve(index) => (index, index + 1),
        PreparationStageV1::KernargAllocate => (3, 3),
        PreparationStageV1::Complete => {
            assert_eq!(owner.completed.as_ref().unwrap().code.len(), 3);
            (0, 4)
        }
        _ => (3, 4),
    };
    assert_eq!(owner.code.len(), prefix);
    assert_eq!(memory.observation().controls, controls);
    let unchanged = memory.observation();
    assert!(run(&mut owner, &mut memory).is_err());
    assert_eq!(memory.observation(), unchanged);
    assert_custody(&memory, &owner);
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

fn native_fault(call: Call, fault: NativeFault, panic: bool, configured: bool) {
    let mut memory = Memory::new(configured);
    let data = memory.roster();
    let expected = inputs(&data);
    let before = memory.observation();
    memory.fault = Some((call, fault));
    let expected_stage = match call {
        Call::AllocateCode(i) => PreparationStageV1::CodeAllocate(i),
        Call::WriteCode(i) => PreparationStageV1::CodeMaterialize(i),
        Call::SealCode(i) => PreparationStageV1::CodeSeal(i),
        Call::MapCode(i) => PreparationStageV1::CodeMap(i),
        Call::AllocateKernarg => PreparationStageV1::KernargAllocate,
        Call::WriteKernarg => PreparationStageV1::KernargMaterialize,
        Call::MapKernarg => PreparationStageV1::KernargMap,
    };
    let mut owner = FixedDispatchPreparationCustodyV1::new([packet(2)], data);
    let result = catch_unwind(AssertUnwindSafe(|| run(&mut owner, &mut memory)));
    if panic {
        let operation = match fault {
            NativeFault::Panic(operation) => operation,
            NativeFault::CurrentnessPanic(_) => "currentness",
            NativeFault::AccessPanic => "with_bytes_mut",
            _ => unreachable!("panic test requires panic fault"),
        };
        assert_eq!(
            result.unwrap_err().downcast_ref::<(&str, &str)>(),
            Some(&("N2 native panic", operation))
        );
    } else {
        assert!(
            result.unwrap().is_err(),
            "native fault {call:?} must reject"
        );
    }
    assert_eq!(owner.stage, expected_stage);
    assert!(owner.failed);
    assert_eq!(
        memory.observation().phase,
        SharedMemorySessionPhaseV1::Quarantined
    );
    assert_inputs(&owner, &expected);
    assert_custody(&memory, &owner);
    assert_backing(&memory, &before);
    let prefix = match call {
        Call::AllocateCode(i) | Call::WriteCode(i) | Call::SealCode(i) | Call::MapCode(i) => i,
        _ => 3,
    };
    assert_eq!(owner.code.len(), prefix);
    assert_eq!(owner.code_identity.len(), prefix);
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
    let mut memory = Memory::new(true);
    let data = memory.device(true);
    let queue = super::super::tests::persistent_control_test_queue(41);
    let programs = programs();
    let packets = [packet(0)];
    let role = Gfx942DeviceContentRoleV1::new([0x61; 32], 0).unwrap();
    let identity = persistent_fixed_dispatch_control_identity_v1(
        queue,
        &programs,
        &packets,
        data.layout(),
        true,
        role,
        data.sdma_storage_identity(),
    )
    .unwrap();
    let expected = inputs(core::slice::from_ref(&data));
    let mut owner = FixedDispatchPreparationCustodyV1::new(packets, vec![data]);
    prepare_persistent_fixed_dispatch_resources_v1(
        &mut memory,
        &programs,
        &mut owner,
        Some(7),
        identity,
    )
    .unwrap();
    assert_inputs(&owner, &expected);
    assert_custody(&memory, &owner);
    let completed = owner.completed.as_ref().unwrap();
    assert_eq!(completed.data_premises[0].role_identity, role.identity());
    assert_eq!(completed.generation.next_generation, 8);
    assert_eq!(completed.generation.recycled_generation, None);
    assert_eq!(
        completed.persistent_control,
        PersistentFixedDispatchControlStateV1::Attached(
            BoundedPersistentFixedDispatchControlIdentityV1::from_single(identity)
        )
    );
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
        crate::persistent_compute::Gfx942PersistentComputeTerminalStageV1::Preparing
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
    use fe2o3_amdhsa_loader::{AdmittedProfile, KernelGlobalBufferAbiV1, validate};
    let image =
        include_bytes!("../../../fe2o3-runtime/fixtures/trusted-gfx942-vecadd-v1/vecadd.hsaco");
    for fault in [
        None,
        Some(PreparationStageV1::CodeResolve(0)),
        Some(PreparationStageV1::KernargRetain),
    ] {
        let program = validate(image, AdmittedProfile::Gfx942XnackOffCov6)
            .unwrap()
            .bind_kernel("vecadd")
            .unwrap()
            .reconcile_dispatch_abi(
                [0x71; 32],
                &[
                    KernelGlobalBufferAbiV1::new(0, "arg0.data", 0, 4, ArgumentAccess::ReadOnly),
                    KernelGlobalBufferAbiV1::new(2, "arg1.data", 16, 4, ArgumentAccess::ReadOnly),
                    KernelGlobalBufferAbiV1::new(4, "arg2.data", 32, 4, ArgumentAccess::WriteOnly),
                ],
            )
            .unwrap();
        let mut bytes = [0; 48];
        for offset in [8, 24, 40] {
            bytes[offset..offset + 8].copy_from_slice(&1024_u64.to_le_bytes());
        }
        let packets = [Gfx942FixedDispatchPacketV1::new(
            0,
            AqlDispatchGeometryV1::new([1024, 1, 1], [256, 1, 1]).unwrap(),
            0,
            bytes.into(),
            vec![
                Gfx942DispatchBufferBindingV1::new(0, 0, 0, 4096),
                Gfx942DispatchBufferBindingV1::new(2, 1, 0, 4096),
                Gfx942DispatchBufferBindingV1::new(4, 2, 0, 4096),
            ]
            .into_boxed_slice(),
        )];
        let mut memory = Memory::new(true);
        let data = vec![
            memory.device(true),
            memory.device(true),
            memory.device(true),
        ];
        let expected = inputs(&data);
        let before = memory.observation();
        let roles = core::array::from_fn(|index| {
            Gfx942DeviceContentRoleV1::new([0x62; 32], index as u32).unwrap()
        });
        let identity = three_binding_persistent_fixed_dispatch_control_identity_v1(
            super::super::tests::persistent_control_test_queue(42),
            core::slice::from_ref(&program),
            &packets,
            core::array::from_fn(|index| data[index].layout()),
            [true, true, true],
            roles,
            core::array::from_fn(|index| data[index].sdma_storage_identity()),
        )
        .unwrap();
        let mut owner = FixedDispatchPreparationCustodyV1::new(packets, data);
        owner.fault = fault.map(|stage| (stage, false));
        let result = prepare_three_binding_persistent_fixed_dispatch_resources_v1(
            &mut memory,
            &[program],
            &mut owner,
            None,
            identity,
        );
        assert_eq!(result.is_err(), fault.is_some());
        assert_custody(&memory, &owner);
        assert_backing(&memory, &before);
        let effects = [
            DeviceDataEffectV1::ReadOnly,
            DeviceDataEffectV1::ReadOnly,
            DeviceDataEffectV1::WriteOnly,
        ];
        if let Some(completed) = &owner.completed {
            assert_eq!(completed.data.len(), 3);
            for index in 0..3 {
                let premise = &completed.data_premises[index];
                assert_eq!(
                    Memory::data_storage(&completed.data[index]),
                    expected[index].storage
                );
                assert_eq!(premise.initialized_content, expected[index].content);
                assert_eq!(premise.fully_initialized, expected[index].initialized);
                assert_eq!(premise.role_identity, roles[index].identity());
                assert_eq!(premise.effect, Some(effects[index]));
            }
        } else {
            let retained = owner.retained_data.as_ref().unwrap();
            assert_eq!(retained.len(), 3);
            for index in 0..3 {
                assert_eq!(
                    Memory::data_storage(&retained[index].authority),
                    expected[index].storage
                );
                assert_eq!(retained[index].initialized_content, expected[index].content);
                assert_eq!(
                    owner.plan.as_ref().unwrap().data[index].effect,
                    Some(effects[index])
                );
            }
        }
    }
}
