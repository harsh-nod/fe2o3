use fe2o3_kernel_ir::{
    AtomicAddressSpaceV1, AtomicOperationV1, AtomicOrderingV1, AtomicParticipationV1,
    AtomicScopeV1, Cov6HiddenDynamicLdsSizeV1, LdsEpochV1, LdsReductionProfileV1,
    OutputOwnershipV1, ScopedAtomicProfileV1, TargetCapability, WaveWidth, WorkgroupBarrierKindV1,
    WorkgroupSize, WorkgroupSyncArgumentRoleV1, WorkgroupSyncArgumentShapeV1, WorkgroupSyncV1Error,
    lds_reduction_v1_kernel_ir, scoped_atomic_v1_kernel_ir, verify_lds_reduction_v1,
    verify_scoped_atomic_v1,
};

#[test]
fn exact_lds_and_atomic_profiles_are_admitted() {
    verify_lds_reduction_v1(
        &lds_reduction_v1_kernel_ir(),
        &LdsReductionProfileV1::exact_gfx942_xnack_minus_cov6(),
    )
    .unwrap();
    verify_scoped_atomic_v1(
        &scoped_atomic_v1_kernel_ir(),
        &ScopedAtomicProfileV1::exact_gfx942_xnack_minus_cov6(),
    )
    .unwrap();
}

#[test]
fn lds_profile_identity_is_closed() {
    let mutations: Vec<fn(&mut LdsReductionProfileV1)> = vec![
        |value| value.source_sha256[0] ^= 1,
        |value| value.namespace[0] ^= 1,
        |value| {
            value.target = TargetCapability::Extension {
                namespace: "amdgpu".into(),
                name: "gfx942:xnack+".into(),
            }
        },
        |value| value.code_object_version = 5,
        |value| value.wave_width = WaveWidth::Wave32,
        |value| value.workgroup_size = WorkgroupSize::new(256, 1, 1),
        |value| value.grid = [2, 1, 1],
        |value| value.descriptor.static_lds_bytes = 256,
        |value| value.descriptor.required_dynamic_lds_bytes = 0,
        |value| value.descriptor.required_dynamic_lds_bytes = 252,
        |value| value.descriptor.maximum_dynamic_lds_bytes = 1024,
        |value| value.descriptor.hidden_dynamic_lds_size = None,
        |value| {
            value
                .descriptor
                .hidden_dynamic_lds_size
                .as_mut()
                .unwrap()
                .relative_offset = 124
        },
        |value| {
            value
                .descriptor
                .hidden_dynamic_lds_size
                .as_mut()
                .unwrap()
                .field_size = 8
        },
        |value| {
            value
                .descriptor
                .hidden_dynamic_lds_size
                .as_mut()
                .unwrap()
                .required_launch_value = 252
        },
        |value| value.descriptor.explicit_kernarg_bytes = 40,
        |value| value.descriptor.complete_kernarg_bytes = 40,
        |value| value.descriptor.logical_name.push_str("_substitution"),
        |value| value.descriptor.export_name.push_str("_substitution"),
        |value| value.descriptor.descriptor_symbol.push_str("_substitution"),
        |value| value.descriptor.code_object_version = 5,
        |value| value.descriptor.workgroup_size = WorkgroupSize::new(32, 1, 1),
        |value| value.descriptor.wave_width = WaveWidth::Wave32,
    ];
    for mutate in mutations {
        let mut profile = LdsReductionProfileV1::exact_gfx942_xnack_minus_cov6();
        mutate(&mut profile);
        assert_eq!(
            verify_lds_reduction_v1(&lds_reduction_v1_kernel_ir(), &profile),
            Err(WorkgroupSyncV1Error::UnsupportedProfile)
        );
    }
}

#[test]
fn lds_descriptor_distinguishes_static_and_exact_dynamic_storage() {
    let profile = LdsReductionProfileV1::exact_gfx942_xnack_minus_cov6();
    assert_eq!(profile.descriptor.static_lds_bytes, 0);
    assert_eq!(profile.descriptor.required_dynamic_lds_bytes, 256);
    assert_eq!(profile.descriptor.maximum_dynamic_lds_bytes, 256);
    assert_eq!(
        profile.descriptor.hidden_dynamic_lds_size,
        Some(Cov6HiddenDynamicLdsSizeV1 {
            relative_offset: 120,
            field_size: 4,
            required_launch_value: 256,
        })
    );

    let mut static_substitution = profile.clone();
    static_substitution.descriptor.static_lds_bytes = 256;
    static_substitution.descriptor.required_dynamic_lds_bytes = 0;
    assert_eq!(
        verify_lds_reduction_v1(&lds_reduction_v1_kernel_ir(), &static_substitution),
        Err(WorkgroupSyncV1Error::UnsupportedProfile)
    );

    let mut wrong_launch_bytes = profile;
    wrong_launch_bytes.descriptor.required_dynamic_lds_bytes = 512;
    wrong_launch_bytes.descriptor.maximum_dynamic_lds_bytes = 512;
    assert_eq!(
        verify_lds_reduction_v1(&lds_reduction_v1_kernel_ir(), &wrong_launch_bytes),
        Err(WorkgroupSyncV1Error::UnsupportedProfile)
    );
}

#[test]
fn lds_capability_barriers_epochs_and_ownership_are_closed() {
    let mutations: Vec<fn(&mut fe2o3_kernel_ir::LdsReductionKernelIrV1)> = vec![
        |value| value.module_id = "substitution",
        |value| value.function_id = "substitution",
        |value| value.kernel_id = "substitution",
        |value| value.arguments[0].role = WorkgroupSyncArgumentRoleV1::Eligibility,
        |value| value.arguments[0].shape = WorkgroupSyncArgumentShapeV1::Scalar,
        |value| value.arguments[0].scalar = fe2o3_kernel_ir::ScalarType::U32,
        |value| value.arguments[0].offset = 4,
        |value| value.arguments[0].size = 8,
        |value| value.arguments[0].alignment = 4,
        |value| value.arguments[1].role = WorkgroupSyncArgumentRoleV1::Values,
        |value| value.arguments[1].shape = WorkgroupSyncArgumentShapeV1::SharedReadOnlySlice64,
        |value| value.arguments[1].scalar = fe2o3_kernel_ir::ScalarType::U32,
        |value| value.arguments[1].offset = 24,
        |value| value.arguments[1].size = 8,
        |value| value.arguments[1].alignment = 4,
        |value| value.lds.element = fe2o3_kernel_ir::ScalarType::U32,
        |value| value.lds.elements = 256,
        |value| value.lds.bytes = 1024,
        |value| value.lds.alignment = 8,
        |value| value.lds.allocation_count = 2,
        |value| value.lds.initial_epoch = LdsEpochV1::LaneInitialized,
        |value| value.lds.final_epoch = LdsEpochV1::Read,
        |value| value.lds.pointer_escape = true,
        |value| value.barriers[0].ordinal = 1,
        |value| value.barriers[0].kind = WorkgroupBarrierKindV1::ReadToReuse,
        |value| value.barriers[0].convergent_threads = 63,
        |value| value.barriers[1].ordinal = 0,
        |value| value.barriers[1].kind = WorkgroupBarrierKindV1::PublishToRead,
        |value| value.barriers[1].convergent_threads = 256,
        |value| value.barriers.swap(0, 1),
        |value| value.output_ownership = OutputOwnershipV1::AnyLaneMayWrite,
    ];
    for mutate in mutations {
        let mut ir = lds_reduction_v1_kernel_ir();
        mutate(&mut ir);
        reject_lds(ir);
    }
}

#[test]
fn atomic_profile_and_exact_semantics_are_closed() {
    let profile = ScopedAtomicProfileV1::exact_gfx942_xnack_minus_cov6();
    assert_eq!(profile.descriptor.hidden_dynamic_lds_size, None);

    let profile_mutations: Vec<fn(&mut ScopedAtomicProfileV1)> = vec![
        |value| value.source_sha256[0] ^= 1,
        |value| value.namespace[31] ^= 1,
        |value| {
            value.target = TargetCapability::Extension {
                namespace: "amdgpu".into(),
                name: "gfx942:xnack+".into(),
            }
        },
        |value| value.code_object_version = 5,
        |value| value.wave_width = WaveWidth::Wave32,
        |value| value.workgroup_size = WorkgroupSize::new(32, 1, 1),
        |value| value.grid = [2, 1, 1],
        |value| value.descriptor.logical_name.push_str("_substitution"),
        |value| value.descriptor.export_name.push_str("_substitution"),
        |value| value.descriptor.descriptor_symbol.push_str("_substitution"),
        |value| value.descriptor.code_object_version = 5,
        |value| value.descriptor.explicit_kernarg_bytes = 32,
        |value| value.descriptor.complete_kernarg_bytes = 40,
        |value| value.descriptor.workgroup_size = WorkgroupSize::new(32, 1, 1),
        |value| value.descriptor.wave_width = WaveWidth::Wave32,
        |value| value.descriptor.static_lds_bytes = 256,
        |value| value.descriptor.required_dynamic_lds_bytes = 256,
        |value| value.descriptor.maximum_dynamic_lds_bytes = 256,
        |value| {
            value.descriptor.hidden_dynamic_lds_size = Some(Cov6HiddenDynamicLdsSizeV1 {
                relative_offset: 120,
                field_size: 4,
                required_launch_value: 256,
            })
        },
    ];
    for mutate in profile_mutations {
        let mut candidate = profile.clone();
        mutate(&mut candidate);
        assert_eq!(
            verify_scoped_atomic_v1(&scoped_atomic_v1_kernel_ir(), &candidate),
            Err(WorkgroupSyncV1Error::UnsupportedProfile)
        );
    }

    let mut hidden_dynamic_lds = ScopedAtomicProfileV1::exact_gfx942_xnack_minus_cov6();
    hidden_dynamic_lds.descriptor.hidden_dynamic_lds_size = Some(Cov6HiddenDynamicLdsSizeV1 {
        relative_offset: 120,
        field_size: 4,
        required_launch_value: 256,
    });
    assert_eq!(
        verify_scoped_atomic_v1(&scoped_atomic_v1_kernel_ir(), &hidden_dynamic_lds),
        Err(WorkgroupSyncV1Error::UnsupportedProfile)
    );

    let mutations: Vec<fn(&mut fe2o3_kernel_ir::ScopedAtomicKernelIrV1)> = vec![
        |value| value.module_id = "substitution",
        |value| value.function_id = "substitution",
        |value| value.kernel_id = "substitution",
        |value| value.arguments[0].role = WorkgroupSyncArgumentRoleV1::Eligibility,
        |value| value.arguments[0].shape = WorkgroupSyncArgumentShapeV1::Scalar,
        |value| value.arguments[0].scalar = fe2o3_kernel_ir::ScalarType::I32,
        |value| value.arguments[0].offset = 4,
        |value| value.arguments[0].size = 8,
        |value| value.arguments[0].alignment = 4,
        |value| value.arguments[1].role = WorkgroupSyncArgumentRoleV1::Values,
        |value| value.arguments[1].shape = WorkgroupSyncArgumentShapeV1::Scalar,
        |value| value.arguments[1].scalar = fe2o3_kernel_ir::ScalarType::I32,
        |value| value.arguments[1].offset = 24,
        |value| value.arguments[1].size = 8,
        |value| value.arguments[1].alignment = 4,
        |value| value.arguments[2].role = WorkgroupSyncArgumentRoleV1::ReductionOutput,
        |value| value.arguments[2].shape = WorkgroupSyncArgumentShapeV1::LaneZeroOwnedWriteSlice1,
        |value| value.arguments[2].scalar = fe2o3_kernel_ir::ScalarType::I32,
        |value| value.operation = AtomicOperationV1::Exchange,
        |value| value.scalar = fe2o3_kernel_ir::ScalarType::I32,
        |value| value.scope = AtomicScopeV1::Workgroup,
        |value| value.ordering = AtomicOrderingV1::SequentiallyConsistent,
        |value| value.address_space = AtomicAddressSpaceV1::Workgroup,
        |value| value.participation = AtomicParticipationV1::EveryLaneExactlyOnce,
        |value| value.unique_host_borrow = false,
        |value| value.device_lanes_alias_one_atomic = false,
        |value| value.arguments[2].offset = 40,
        |value| value.arguments[2].size = 4,
        |value| value.arguments[2].alignment = 4,
    ];
    for mutate in mutations {
        let mut ir = scoped_atomic_v1_kernel_ir();
        mutate(&mut ir);
        assert_eq!(
            verify_scoped_atomic_v1(&ir, &ScopedAtomicProfileV1::exact_gfx942_xnack_minus_cov6()),
            Err(WorkgroupSyncV1Error::NonCanonicalKernelIr)
        );
    }
}

fn reject_lds(ir: fe2o3_kernel_ir::LdsReductionKernelIrV1) {
    assert_eq!(
        verify_lds_reduction_v1(&ir, &LdsReductionProfileV1::exact_gfx942_xnack_minus_cov6()),
        Err(WorkgroupSyncV1Error::NonCanonicalKernelIr)
    );
}
