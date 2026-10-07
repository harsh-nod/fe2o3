//! Representation/capacity controls over genuine checked preparation, CPU only.

use super::*;
use crate::shared_memory::PreparationMemoryFixtureV1 as Memory;
use fe2o3_amdhsa_loader::{KernelGlobalBufferAbiV1, validate};
use fe2o3_resource_accounting::{ResourceKindV1, ResourceVectorV1};

fn account(bytes: u64) -> ResourceCreditAccountV1 {
    ResourceCreditAccountV1::new(
        ResourceVectorV1::ZERO.with(ResourceKindV1::ControlResidentBytes, bytes),
        SLOTS + 4,
    )
    .unwrap()
}

fn prepared() -> (
    Memory,
    DispatchResourceOwnerV1,
    Gfx942NativeFillArenaStorageV1,
    ResourceCreditAccountV1,
) {
    let captured =
        fe2o3_kernel_analysis::PhysicalMachineEffectRequestV1::decode_canonical(include_bytes!(
            "../../../../fe2o3-kernel-analysis/src/gfx942_fill_analysis_v1/fill.request"
        ))
        .unwrap();
    let program = validate(
        captured.exact_payload_bytes(),
        AdmittedProfile::Gfx942XnackOffCov6,
    )
    .unwrap()
    .bind_kernel("fill_write_only")
    .unwrap();
    let name = program.selected_kernel().explicit_arguments()[0]
        .name()
        .unwrap()
        .to_owned();
    let program = program
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
        .unwrap();
    let account = account(64 << 20);
    let packets = Gfx942NativeFillArenaPacketsV1::try_new(&account, |index| {
        let mut bytes = vec![0; 272];
        bytes[8..16].copy_from_slice(&1u64.to_le_bytes());
        Gfx942FixedDispatchPacketV1::new(
            0,
            AqlDispatchGeometryV1::new([64, 1, 1], [64, 1, 1]).unwrap(),
            0,
            bytes.into_boxed_slice(),
            Box::new([Gfx942DispatchBufferBindingV1::new(
                0,
                0,
                index as u64 * 4,
                4,
            )]),
        )
        .require_conditional_fill_v1()
    })
    .unwrap();
    let mut memory = Memory::new(true);
    let token = memory
        .allocate::<HostVisibleCoherentGttV1>(SLOTS * 4)
        .unwrap();
    let data = Gfx942FixedDispatchDataV1::host_visible_uninitialized(memory.map(token).unwrap());
    let inputs = Gfx942NativeFillArenaInputsV1::admit(program, packets, data).unwrap();
    let plan = plan_public_fixed_dispatch_resources(
        &inputs.programs,
        inputs.packets.packets(),
        &[inputs.data[0].layout()],
        &[false],
    )
    .unwrap();
    assert!(
        conditional_fill::check_plan(
            &inputs.programs,
            inputs.packets.packets(),
            &plan,
            PersistentFixedDispatchControlStateV1::Ordinary
        )
        .is_err(),
        "ordinary family must stay singleton-only"
    );
    let mut storage = Gfx942NativeFillArenaStorageV1::preallocate(account.clone()).unwrap();
    let mut custody = FixedDispatchPreparationCustodyV1::new_native_fill_arena(
        inputs.packets,
        inputs.data,
        storage.premises.take().unwrap(),
    );
    storage
        .prepare(&mut memory, &inputs.programs, &mut custody)
        .unwrap();
    let owner = custody.take_completed().unwrap();
    (memory, owner, storage, account)
}

#[test]
fn arena_selected_native_root_and_packet_substitution_refuse_without_reservation() {
    let (_memory, mut owner, mut storage, account) = prepared();
    let before = account.usage();
    let packet = owner.packets[7];
    owner.packets[7] = owner.packets[8];
    assert!(
        owner
            .arena_premises_v1()
            .unwrap()
            .selected(&owner, 7, None)
            .is_err()
    );
    assert!(owner.require_arena_backing_v1().is_err());
    owner.packets[7] = packet;
    let original = owner.code_identity[0];
    owner.code_identity[0].dispatch_abi_identity = [0x23; 32];
    assert!(
        owner
            .arena_premises_v1()
            .unwrap()
            .selected(&owner, 7, None)
            .is_err()
    );
    owner.code_identity[0] = original;
    let original = owner.data_premises[0].writable_ranges[7];
    owner.data_premises[0].writable_ranges[7].offset += 4;
    assert!(
        owner
            .arena_premises_v1()
            .unwrap()
            .selected(&owner, 7, None)
            .is_err()
    );
    owner.data_premises[0].writable_ranges[7] = original;
    owner.require_arena_backing_v1().unwrap();
    assert!(storage.recipe(SLOTS).is_err());
    assert!(storage.settled());
    assert_eq!(account.usage(), before);
}

#[test]
fn arena_common_generation_cannot_be_reserved_or_replaced_by_a_slot() {
    let (_memory, mut owner, storage, account) = prepared();
    let before = account.usage();
    owner.generation.next_generation += 1;
    assert!(owner.require_arena_backing_v1().is_err());
    owner.generation.next_generation -= 1;
    owner.require_arena_backing_v1().unwrap();
    assert_eq!(owner.code.len(), 1);
    assert_eq!(owner.data.len(), 1);
    assert_eq!(owner.packets.len(), SLOTS);
    assert!(storage.settled());
    assert_eq!(account.usage(), before);
}

#[test]
fn arena_metadata_exact_payload_and_one_byte_short_are_prepaid() {
    let funded = account(64 << 20);
    let storage = Gfx942NativeFillArenaStorageV1::preallocate(funded.clone()).unwrap();
    let bytes = funded
        .usage()
        .used
        .get(ResourceKindV1::ControlResidentBytes);
    assert_eq!(funded.usage().retained_records, SLOTS + 3);
    let short = account(bytes - 1);
    assert!(Gfx942NativeFillArenaStorageV1::preallocate(short.clone()).is_err());
    assert_eq!(short.usage().used, ResourceVectorV1::ZERO);
    assert_eq!(short.usage().retained_records, 0);
    let exact = account(bytes);
    let exact_storage = Gfx942NativeFillArenaStorageV1::preallocate(exact.clone()).unwrap();
    assert_eq!(
        exact.usage().used.get(ResourceKindV1::ControlResidentBytes),
        bytes
    );
    drop(exact_storage);
    drop(storage);
    assert_eq!(funded.usage().used, ResourceVectorV1::ZERO);
    assert_eq!(exact.usage().used, ResourceVectorV1::ZERO);
}
