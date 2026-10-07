//! The captured 272-byte Worker artifact; real sequencing with CPU native adapters.

use super::*;
use fe2o3_amdhsa_loader::{AdmittedProfile, KernelGlobalBufferAbiV1, validate};
use fe2o3_hsaco::ArgumentAccess;
use fe2o3_kernel_analysis::PhysicalMachineEffectRequestV1;

#[path = "native_fill_cohort/tests.rs"]
mod native_fill_cohort_tests;

fn payload() -> PhysicalMachineEffectRequestV1 {
    PhysicalMachineEffectRequestV1::decode_canonical(include_bytes!(
        "../../../fe2o3-kernel-analysis/src/gfx942_fill_analysis_v1/fill.request"
    ))
    .unwrap()
}

fn program(bytes: &[u8]) -> ValidatedKernelEnvelope<'_> {
    let kernel = validate(bytes, AdmittedProfile::Gfx942XnackOffCov6)
        .unwrap()
        .bind_kernel("fill_write_only")
        .unwrap();
    let name = kernel.selected_kernel().explicit_arguments()[0]
        .name()
        .unwrap()
        .to_owned();
    kernel
        .reconcile_dispatch_abi(
            [0x51; 32],
            &[KernelGlobalBufferAbiV1::new(
                0,
                &name,
                0,
                4,
                ArgumentAccess::WriteOnly,
            )],
        )
        .unwrap()
}

fn input(count: u64, grid: u32) -> Gfx942FixedDispatchPacketV1 {
    let mut bytes = vec![0; 272];
    bytes[8..16].copy_from_slice(&count.to_le_bytes());
    Gfx942FixedDispatchPacketV1::new(
        0,
        AqlDispatchGeometryV1::new([grid, 1, 1], [64, 1, 1]).unwrap(),
        0,
        bytes.into_boxed_slice(),
        Box::new([Gfx942DispatchBufferBindingV1::new(0, 0, 0, count * 4)]),
    )
    .require_conditional_fill_v1()
}

fn data(memory: &mut Memory, bytes: usize) -> Gfx942FixedDispatchDataV1 {
    let token = memory.allocate::<HostVisibleCoherentGttV1>(bytes).unwrap();
    Gfx942FixedDispatchDataV1::host_visible_uninitialized(memory.map(token).unwrap())
}

fn prepare(
    memory: &mut Memory,
    custody: &mut FixedDispatchPreparationCustodyV1<1>,
    bytes: &[u8],
) -> Result<(), Gfx942DispatchBindingErrorV1> {
    custody.prepare_in_place(
        memory,
        &[program(bytes)],
        DispatchGenerationOwnerV1::new(),
        PersistentFixedDispatchControlStateV1::Ordinary,
    )
}

fn fixture(count: u64, grid: u32) -> (Memory, DispatchResourceOwnerV1, QueueKeyV1) {
    let mut memory = Memory::new(true);
    let owner = owner(&mut memory, count, grid);
    let mut queue = test_dispatch_queue_v1();
    queue.vm = memory.primary_vm();
    (memory, owner, queue)
}

fn owner(memory: &mut Memory, count: u64, grid: u32) -> DispatchResourceOwnerV1 {
    let output = data(memory, (count * 4) as usize);
    let mut custody = FixedDispatchPreparationCustodyV1::new([input(count, grid)], vec![output]);
    prepare(memory, &mut custody, payload().exact_payload_bytes()).unwrap();
    custody.take_completed().unwrap()
}

fn cleanup(owner: DispatchResourceOwnerV1, memory: &mut Memory) {
    use crate::queue::dispatch_binding::control_release::{
        ReturningControlCleanupCustodyV1, ReturningControlModeV1,
    };
    let mut cleanup =
        ReturningControlCleanupCustodyV1::new(owner, ReturningControlModeV1::Ordinary);
    cleanup.release_ordinary_in_place(memory).unwrap();
    assert!(cleanup.is_complete());
}

#[test]
fn conditional_fill_full64_native_preparation_and_one_generation_binding() {
    for (count, grid) in [(1, 64), (64, 64), (65, 128), (65, 192), (1024, 1024)] {
        let (mut memory, mut owner, queue) = fixture(count, grid);
        assert!(owner.conditional_fill.is_some());
        let actual = memory.mapped_bytes(owner.kernarg.facts().mapping());
        assert_eq!(actual.len(), 272);
        assert_eq!(u64::from_le_bytes(actual[8..16].try_into().unwrap()), count);
        let (templates, epoch) = owner.bind_templates::<1>(queue).unwrap();
        assert_eq!(templates.len(), 1);
        let state = owner.source_failure_snapshot_v1();
        assert!(owner.bind_templates::<1>(queue).is_err());
        assert_eq!(owner.source_failure_snapshot_v1(), state);
        owner.cancel_binding(epoch).unwrap();
        let state = owner.source_failure_snapshot_v1();
        assert!(owner.bind_templates::<1>(queue).is_err());
        assert_eq!(owner.source_failure_snapshot_v1(), state);
        cleanup(owner, &mut memory);
        memory.primary_assert_all_released_v1();
    }
}

#[test]
fn conditional_fill_pristine_cleanup_and_foreign_native_owners() {
    let (mut memory, mut first, queue) = fixture(65, 128);
    let mut second = owner(&mut memory, 65, 128);
    let before = first.source_failure_snapshot_v1();
    std::mem::swap(&mut first.kernarg, &mut second.kernarg);
    assert!(first.bind_templates::<1>(queue).is_err());
    std::mem::swap(&mut first.kernarg, &mut second.kernarg);
    std::mem::swap(&mut first.data, &mut second.data);
    assert!(first.bind_templates::<1>(queue).is_err());
    std::mem::swap(&mut first.data, &mut second.data);
    std::mem::swap(&mut first.code, &mut second.code);
    assert!(first.bind_templates::<1>(queue).is_err());
    std::mem::swap(&mut first.code, &mut second.code);
    assert_eq!(first.source_failure_snapshot_v1(), before);
    cleanup(first, &mut memory);
    cleanup(second, &mut memory);
    memory.primary_assert_all_released_v1();
}

#[test]
fn conditional_fill_equal_mapping_facts_do_not_hide_a_foreign_memory_session() {
    let (mut left_memory, mut left, queue) = fixture(65, 128);
    let (mut right_memory, mut right, _) = fixture(65, 128);
    assert_eq!(left.code[0].facts(), right.code[0].facts());
    assert_eq!(left.kernarg.facts(), right.kernarg.facts());
    assert_ne!(
        left.kernarg.storage_identity(),
        right.kernarg.storage_identity()
    );
    let before = left.source_failure_snapshot_v1();
    std::mem::swap(&mut left.code, &mut right.code);
    assert!(left.bind_templates::<1>(queue).is_err());
    std::mem::swap(&mut left.code, &mut right.code);
    std::mem::swap(&mut left.kernarg, &mut right.kernarg);
    assert!(left.bind_templates::<1>(queue).is_err());
    std::mem::swap(&mut left.kernarg, &mut right.kernarg);
    std::mem::swap(&mut left.data, &mut right.data);
    assert!(left.bind_templates::<1>(queue).is_err());
    std::mem::swap(&mut left.data, &mut right.data);
    assert_eq!(left.source_failure_snapshot_v1(), before);
    cleanup(left, &mut left_memory);
    cleanup(right, &mut right_memory);
    left_memory.primary_assert_all_released_v1();
    right_memory.primary_assert_all_released_v1();
}

#[test]
fn conditional_fill_rejects_every_patched_image_byte_substitution() {
    let mut memory = Memory::new(true);
    let output = data(&mut memory, 65 * 4);
    let mut custody = FixedDispatchPreparationCustodyV1::new([input(65, 128)], vec![output]);
    custody.fault = Some((PreparationStageV1::Commit, false));
    let captured = payload();
    let bytes = captured.exact_payload_bytes();
    assert!(prepare(&mut memory, &mut custody, bytes).is_err());
    let model = fe2o3_kernel_analysis::Gfx942FillKernelV1::inspect(bytes, 0).unwrap();
    let KernargStageV1::Retained(kernarg) = &custody.kernarg else {
        unreachable!()
    };
    let check = |image| {
        conditional_fill::PreparedConditionalFillPremisesV1::check(
            &model,
            &custody.packets[0],
            custody.plan.as_ref().unwrap(),
            conditional_fill::NativeFillCustodyV1 {
                code: &custody.code[0],
                code_identity: custody.code_identity[0],
                kernarg,
                output: &custody.retained_data.as_ref().unwrap()[0].authority,
                generation: custody.generation.as_ref().unwrap(),
            },
            image,
            custody.prepared_packets[0],
        )
    };
    let original = custody.conditional_fill.as_ref().unwrap().kernarg.unwrap();
    assert_eq!(
        &memory.mapped_bytes(kernarg.facts().mapping())[..272],
        &original
    );
    assert!(check(original).is_ok());
    for byte in 0..272 {
        let mut changed = original;
        changed[byte] ^= 1;
        assert!(check(changed).is_err(), "byte {byte}");
    }
}

#[test]
fn conditional_fill_rejects_changed_machine_entry_and_short_profile() {
    let captured = payload();
    let original = captured.exact_payload_bytes();
    let model = fe2o3_kernel_analysis::Gfx942FillKernelV1::inspect(original, 0).unwrap();
    let entry = model.binding().entry_file_offset() as usize;
    for byte in [entry + 8, entry + 32, entry + 60] {
        let mut changed = original.to_vec();
        changed[byte] ^= 1;
        let mut memory = Memory::new(true);
        let output = data(&mut memory, 65 * 4);
        let mut custody = FixedDispatchPreparationCustodyV1::new([input(65, 128)], vec![output]);
        assert!(prepare(&mut memory, &mut custody, &changed).is_err());
        assert!(!custody.native_started);
    }
    let short = include_bytes!(
        "../../../fe2o3-hsaco/tests/fixtures/rust-fill-write-only-gfx942/kernel.hsaco"
    );
    let mut packet = input(65, 128);
    packet.kernarg_bytes = packet.kernarg_bytes[..16].into();
    let mut memory = Memory::new(true);
    let output = data(&mut memory, 65 * 4);
    let mut custody = FixedDispatchPreparationCustodyV1::new([packet], vec![output]);
    assert!(prepare(&mut memory, &mut custody, short).is_err());
    assert!(!custody.native_started);
}

#[test]
fn conditional_fill_rejects_geometry_and_shape_before_native_controls() {
    for case in 0..10 {
        let mut memory = Memory::new(true);
        let output = data(&mut memory, 65 * 4);
        let mut packet = input(65, 128);
        match case {
            0 => packet.geometry = AqlDispatchGeometryV1::new([65, 1, 1], [64, 1, 1]).unwrap(),
            1 => packet.geometry = AqlDispatchGeometryV1::new([64, 1, 1], [64, 1, 1]).unwrap(),
            2 => packet.geometry = AqlDispatchGeometryV1::new([128, 2, 1], [64, 1, 1]).unwrap(),
            3 => packet.geometry = AqlDispatchGeometryV1::new([128, 1, 1], [32, 1, 1]).unwrap(),
            4 => packet.kernarg_bytes = vec![0; 16].into_boxed_slice(),
            5 => packet.kernarg_bytes = vec![0; 264].into_boxed_slice(),
            6 => packet.kernarg_bytes = vec![0; 280].into_boxed_slice(),
            7 => packet.kernarg_bytes[8..16].copy_from_slice(&64u64.to_le_bytes()),
            8 => packet.kernarg_bytes[0] = 1,
            9 => packet.kernarg_bytes[271] = 1,
            _ => unreachable!(),
        }
        let before = memory.observation();
        let mut custody = FixedDispatchPreparationCustodyV1::new([packet], vec![output]);
        assert!(
            preflight_gfx942_fixed_dispatch_replacement(
                4096,
                &[program(payload().exact_payload_bytes())],
                &custody.packets,
                &custody.original_data,
                1,
            )
            .is_err(),
            "replacement case {case}"
        );
        assert!(
            prepare(&mut memory, &mut custody, payload().exact_payload_bytes()).is_err(),
            "case {case}"
        );
        assert!(!custody.native_started);
        assert!(custody.completed().is_err());
        assert_eq!(memory.observation(), before);
        assert_eq!(custody.original_data.len(), 1);
        assert!(custody.retained_data.is_none());
    }
}

#[test]
fn conditional_fill_rejects_persistent_replay_and_extra_rosters() {
    let captured = payload();
    let kernel = program(captured.exact_payload_bytes());
    assert!(
        preflight_gfx942_persistent_compute_dispatch_v1(
            std::slice::from_ref(&kernel),
            &[input(65, 128)],
            Gfx942FixedDispatchDataLayoutV1::device_local(260, 4096),
            false,
        )
        .is_err()
    );
    for extra_program in [false, true] {
        let mut memory = Memory::new(true);
        let mut outputs = vec![data(&mut memory, 260)];
        let mut programs = vec![program(captured.exact_payload_bytes())];
        if extra_program {
            programs.push(program(captured.exact_payload_bytes()));
        } else {
            outputs.push(data(&mut memory, 260));
        }
        let before = memory.observation();
        let mut custody = FixedDispatchPreparationCustodyV1::new([input(65, 128)], outputs);
        assert!(
            custody
                .prepare_in_place(
                    &mut memory,
                    &programs,
                    DispatchGenerationOwnerV1::new(),
                    PersistentFixedDispatchControlStateV1::Ordinary,
                )
                .is_err()
        );
        assert!(!custody.native_started);
        assert_eq!(memory.observation(), before);
    }
    let mut memory = Memory::new(true);
    let outputs = vec![data(&mut memory, 260)];
    let mut custody =
        FixedDispatchPreparationCustodyV1::new([input(65, 128), input(65, 128)], outputs);
    assert!(
        custody
            .prepare_in_place(
                &mut memory,
                &[kernel],
                DispatchGenerationOwnerV1::new(),
                PersistentFixedDispatchControlStateV1::Ordinary,
            )
            .is_err()
    );
    assert!(!custody.native_started);
}

#[test]
fn conditional_fill_device_local_output_requires_a_separate_session_bound_profile() {
    let mut memory = Memory::new(true);
    let output = memory.device(false);
    assert_eq!(output.layout().requested_bytes(), 4096);
    let before = memory.observation();
    let mut custody = FixedDispatchPreparationCustodyV1::new([input(1024, 1024)], vec![output]);
    assert!(prepare(&mut memory, &mut custody, payload().exact_payload_bytes()).is_err());
    assert!(!custody.native_started);
    assert_eq!(custody.original_data.len(), 1);
    assert_eq!(memory.observation(), before);
}

#[test]
fn conditional_fill_binding_rejects_original_owner_and_requirement_substitution() {
    for case in 0..9 {
        let (mut memory, mut owner, mut queue) = fixture(65, 128);
        let before = owner.source_failure_snapshot_v1();
        match case {
            0 => owner.packets[0].conditional_fill = false,
            1 => {
                let retained = owner.conditional_fill.take();
                assert!(owner.bind_templates::<1>(queue).is_err());
                owner.conditional_fill = retained;
                cleanup(owner, &mut memory);
                continue;
            }
            2 => owner.code_identity[0].materialized_sha256[0] ^= 1,
            3 => owner.packets[0].kernarg_mapping.id.0 += 1,
            4 => {
                owner.packets[0].geometry =
                    AqlDispatchGeometryV1::new([192, 1, 1], [64, 1, 1]).unwrap()
            }
            5 => owner.data_premises[0].valid_bytes -= 4,
            6 => owner.generation.recipe_occurrence += 1,
            7 => owner.generation.next_generation += 1,
            8 => queue.vm.device.generation.0 += 1,
            _ => unreachable!(),
        }
        let mutated = owner.source_failure_snapshot_v1();
        assert!(owner.bind_templates::<1>(queue).is_err(), "case {case}");
        assert_eq!(owner.source_failure_snapshot_v1(), mutated);
        owner.generation.recipe_occurrence = before.recipe_occurrence;
        owner.generation.next_generation = before.next_generation;
        cleanup(owner, &mut memory);
    }
}

#[test]
fn conditional_fill_failure_or_unwind_retains_native_custody_before_commit() {
    for stage in [
        PreparationStageV1::PacketResolve(0),
        PreparationStageV1::ConditionalFill,
        PreparationStageV1::Commit,
    ] {
        for panic in [false, true] {
            let mut memory = Memory::new(true);
            let output = data(&mut memory, 65 * 4);
            let mut custody =
                FixedDispatchPreparationCustodyV1::new([input(65, 128)], vec![output]);
            custody.fault = Some((stage, panic));
            let result = catch_unwind(AssertUnwindSafe(|| {
                prepare(&mut memory, &mut custody, payload().exact_payload_bytes())
            }));
            assert!(if panic {
                result.is_err()
            } else {
                result.unwrap().is_err()
            });
            assert!(custody.failed);
            assert!(custody.completed().is_err());
            assert!(
                custody
                    .generation
                    .as_ref()
                    .unwrap()
                    .ensure_prepared()
                    .is_ok()
            );
            assert_eq!(custody.code.len(), 1);
            assert!(matches!(custody.kernarg, KernargStageV1::Retained(_)));
            assert_eq!(custody.retained_data.as_ref().unwrap().len(), 1);
            assert_eq!(
                memory.observation().phase,
                SharedMemorySessionPhaseV1::Quarantined
            );
            assert!(custody.conditional_fill.as_ref().unwrap().kernarg.is_some());
        }
    }
}
