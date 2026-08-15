use fe2o3_kernel_ir::{
    AtomicAddressSpaceV1, AtomicOperationV1, AtomicOrderingV1, AtomicScopeV1,
    LdsReductionProfileV1, OutputOwnershipV1, ScopedAtomicProfileV1, TargetCapability, WaveWidth,
    WorkgroupSize, WorkgroupSyncArgumentRoleV1, WorkgroupSyncV1Error, lds_reduction_v1_kernel_ir,
    scoped_atomic_v1_kernel_ir, verify_lds_reduction_v1, verify_scoped_atomic_v1,
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
        |value| value.descriptor.explicit_kernarg_bytes = 32,
        |value| value.descriptor.complete_kernarg_bytes = 40,
        |value| value.descriptor.export_name.push_str("_substitution"),
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
    let mut wrong_element = lds_reduction_v1_kernel_ir();
    wrong_element.lds.element = fe2o3_kernel_ir::ScalarType::U32;
    reject_lds(wrong_element);

    let mut wrong_extent = lds_reduction_v1_kernel_ir();
    wrong_extent.lds.elements = 256;
    reject_lds(wrong_extent);

    let mut wrong_bytes = lds_reduction_v1_kernel_ir();
    wrong_bytes.lds.bytes = 1024;
    reject_lds(wrong_bytes);

    let mut aliases = lds_reduction_v1_kernel_ir();
    aliases.lds.allocation_count = 2;
    reject_lds(aliases);

    let mut escapes = lds_reduction_v1_kernel_ir();
    escapes.lds.pointer_escape = true;
    reject_lds(escapes);

    let mut nonconvergent = lds_reduction_v1_kernel_ir();
    nonconvergent.barriers[0].convergent_threads = 63;
    reject_lds(nonconvergent);

    let mut reordered = lds_reduction_v1_kernel_ir();
    reordered.barriers.swap(0, 1);
    reject_lds(reordered);

    let mut wrong_owner = lds_reduction_v1_kernel_ir();
    wrong_owner.output_ownership = OutputOwnershipV1::AnyLaneMayWrite;
    reject_lds(wrong_owner);

    let mut wrong_role = lds_reduction_v1_kernel_ir();
    wrong_role.arguments[2].role = WorkgroupSyncArgumentRoleV1::Values;
    reject_lds(wrong_role);
}

#[test]
fn atomic_profile_and_exact_semantics_are_closed() {
    let mut profile = ScopedAtomicProfileV1::exact_gfx942_xnack_minus_cov6();
    profile.namespace[31] ^= 1;
    assert_eq!(
        verify_scoped_atomic_v1(&scoped_atomic_v1_kernel_ir(), &profile),
        Err(WorkgroupSyncV1Error::UnsupportedProfile)
    );

    let mutations: Vec<fn(&mut fe2o3_kernel_ir::ScopedAtomicKernelIrV1)> = vec![
        |value| value.operation = AtomicOperationV1::Exchange,
        |value| value.scalar = fe2o3_kernel_ir::ScalarType::I32,
        |value| value.scope = AtomicScopeV1::Workgroup,
        |value| value.ordering = AtomicOrderingV1::SequentiallyConsistent,
        |value| value.address_space = AtomicAddressSpaceV1::Workgroup,
        |value| value.unique_host_borrow = false,
        |value| value.device_lanes_alias_one_atomic = false,
        |value| value.arguments[2].offset = 40,
        |value| value.arguments[2].role = WorkgroupSyncArgumentRoleV1::ReductionOutput,
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
