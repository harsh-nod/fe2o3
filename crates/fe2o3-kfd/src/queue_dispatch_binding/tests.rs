use super::*;
use fe2o3_amdhsa_loader::{AdmittedProfile, KernelGlobalBufferAbiV1, validate};

fn premise(seed: u8, effect: DeviceDataEffectV1) -> DeviceDataPremiseV1 {
    DeviceDataPremiseV1::new([seed; 32], 4096, effect)
}

fn typed(bytes: usize, patches: impl Into<Box<[DevicePointerPatchV1]>>) -> TypedKernargImageV1 {
    TypedKernargImageV1::new(
        [0x51; 32],
        vec![0; bytes].into_boxed_slice(),
        patches.into(),
    )
}

// Pure preflight uses the same layout values as real C3 leases. The native
// lifecycle and fault boundaries are covered by shared_memory's backend
// fault matrix; these tests target dispatch-specific mutation ordering.
fn fake_input(seed: u64, premise: DeviceDataPremiseV1) -> DeviceDataAllocationInputV1 {
    let _ = seed;
    DeviceDataAllocationInputV1 {
        requested_bytes: 4096,
        alignment: 4096,
        premise,
    }
}

fn implicit_plan(byte_offset: usize, kinds: &[HiddenValueKind]) -> Cov6ImplicitKernargPlanV1 {
    Cov6ImplicitKernargPlanV1 {
        byte_offset,
        fields: kinds
            .iter()
            .map(|kind| admitted_cov6_implicit_kernarg_field(*kind).unwrap())
            .collect(),
    }
}

fn inspected(
    pointer_offset: u64,
    actual_access: Option<ArgumentAccess>,
    pointee_alignment: Option<u64>,
) -> InspectedBufferContractV1 {
    InspectedBufferContractV1 {
        pointer_offset,
        declared_access: actual_access,
        actual_access,
        pointee_alignment,
    }
}

pub(super) fn persistent_control_test_queue(queue: u64) -> QueueKeyV1 {
    QueueKeyV1 {
        vm: fe2o3_runtime_model::VmKeyV1 {
            device: fe2o3_runtime_model::DeviceKeyV1 {
                physical: fe2o3_runtime_model::PhysicalDeviceIdV1(7),
                generation: fe2o3_runtime_model::DeviceGenerationV1(11),
            },
            id: fe2o3_runtime_model::VmIdV1(13),
        },
        id: fe2o3_runtime_model::QueueInstanceIdV1(queue),
        generation: fe2o3_runtime_model::QueueGenerationV1(17),
    }
}

fn persistent_control_test_identity(
    queue: QueueKeyV1,
    storage: Gfx942SdmaBufferStorageIdentityV1,
) -> PersistentFixedDispatchControlIdentityV1 {
    PersistentFixedDispatchControlIdentityV1 {
        queue,
        semantic_sha256: [0x51; 32],
        content_role: Gfx942DeviceContentRoleV1::new([0x61; 32], 0).unwrap(),
        data_layout: Gfx942FixedDispatchDataLayoutV1::device_local(4096, 4096),
        data_storage: storage,
        effect: DeviceDataEffectV1::ReadWrite,
    }
}

pub(in crate::queue) fn actual_persistent_control_test_program<'a>(
    image: &'a [u8],
    signature: [u8; 32],
) -> ValidatedKernelEnvelope<'a> {
    validate(image, AdmittedProfile::Gfx942XnackOffCov6)
        .unwrap()
        .bind_kernel("inplace_transform")
        .unwrap()
        .reconcile_dispatch_abi(
            signature,
            &[KernelGlobalBufferAbiV1::new(
                0,
                "data",
                0,
                1,
                ArgumentAccess::ReadWrite,
            )],
        )
        .unwrap()
}

#[allow(clippy::too_many_arguments)]
fn actual_persistent_control_test_identity(
    image: &[u8],
    signature: [u8; 32],
    grid_x: u32,
    ordering: AqlDispatchOrderingV1,
    dynamic_group_segment_bytes: u32,
    scalar: u64,
    data_byte_offset: u64,
    byte_len: u64,
) -> Result<PersistentFixedDispatchControlIdentityV1, Gfx942DispatchBindingErrorV1> {
    let queue = persistent_control_test_queue(41);
    let (device, _) = crate::sdma::persistent_sdma_buffers_for_test(queue, 606);
    let program = actual_persistent_control_test_program(image, signature);
    let mut kernarg = [0_u8; 16];
    kernarg[8..].copy_from_slice(&scalar.to_le_bytes());
    let packet = Gfx942FixedDispatchPacketV1::new_with_ordering(
        0,
        AqlDispatchGeometryV1::new([grid_x, 1, 1], [256, 1, 1]).unwrap(),
        ordering,
        dynamic_group_segment_bytes,
        kernarg.into(),
        vec![Gfx942DispatchBufferBindingV1::new(
            0,
            0,
            data_byte_offset,
            byte_len,
        )]
        .into_boxed_slice(),
    );
    persistent_fixed_dispatch_control_identity_v1(
        queue,
        &[program],
        &[packet],
        Gfx942FixedDispatchDataLayoutV1::device_local(4096, 4096),
        true,
        Gfx942DeviceContentRoleV1::new([0x61; 32], 0).unwrap(),
        device.storage_identity(),
    )
}

fn readback_premise(
    kind: Gfx942FixedDispatchDataKindV1,
    effect: DeviceDataEffectV1,
    ranges: &[(u64, u64)],
) -> RetainedDataPremiseV1 {
    RetainedDataPremiseV1 {
        layout: Gfx942FixedDispatchDataLayoutV1 {
            kind,
            requested_bytes: 256,
            alignment: 64,
        },
        role_identity: [0; 32],
        valid_bytes: 256,
        effect: Some(effect),
        initialized_content: None,
        fully_initialized: false,
        writable_ranges: ranges
            .iter()
            .map(|&(offset, byte_len)| CompletedWritableRangeV1 { offset, byte_len })
            .collect(),
        completed_snapshots: Box::new([]),
    }
}

fn snapshot_readback_premise(
    kind: Gfx942FixedDispatchDataKindV1,
    fully_initialized: bool,
) -> RetainedDataPremiseV1 {
    let mut premise = readback_premise(kind, DeviceDataEffectV1::WriteOnly, &[(64, 64)]);
    premise.fully_initialized = fully_initialized;
    premise.completed_snapshots = Box::new([CompletedSnapshotRangeV1 {
        offset: 32,
        byte_len: 128,
        interior_offset: 64,
        interior_byte_len: 64,
    }]);
    premise
}

#[path = "tests/abi_and_epoch_tests.rs"]
mod abi_and_epoch_tests;
#[path = "tests/occurrence_tests.rs"]
mod occurrence_tests;
#[path = "tests/readback_tests.rs"]
mod readback_tests;
