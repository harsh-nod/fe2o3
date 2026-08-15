use fe2o3_kernel_ir::{
    ScalarType, TargetCapability, Wave64ArgumentRoleV1, Wave64CollectiveKindV1,
    Wave64CollectivesProfileV1, Wave64CollectivesV1Error, Wave64F32PolicyV1, WaveWidth,
    WorkgroupSize, verify_wave64_collectives_v1, wave64_collectives_v1_kernel_ir,
};

type ProfileMutation = fn(&mut Wave64CollectivesProfileV1);

#[test]
fn exact_profile_and_semantic_ir_are_admitted() {
    verify_wave64_collectives_v1(
        &wave64_collectives_v1_kernel_ir(),
        &Wave64CollectivesProfileV1::exact_gfx942_xnack_minus_cov6(),
    )
    .unwrap();
}

#[test]
fn source_namespace_target_cov_wave_and_launch_are_closed() {
    let mutations: Vec<ProfileMutation> = vec![
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
        |value| value.workgroup_size = WorkgroupSize::new(32, 1, 1),
        |value| value.grid = [2, 1, 1],
        |value| value.f32_policy = Wave64F32PolicyV1::FiniteOnly,
    ];
    for mutate in mutations {
        let mut profile = Wave64CollectivesProfileV1::exact_gfx942_xnack_minus_cov6();
        mutate(&mut profile);
        assert_eq!(
            verify_wave64_collectives_v1(&wave64_collectives_v1_kernel_ir(), &profile),
            Err(Wave64CollectivesV1Error::UnsupportedProfile)
        );
    }
}

#[test]
fn active_mask_type_and_abi_are_closed() {
    let mut wrong_type = wave64_collectives_v1_kernel_ir();
    wrong_type.arguments[1].scalar = ScalarType::U32;
    reject_ir(wrong_type);

    let mut wrong_offset = wave64_collectives_v1_kernel_ir();
    wrong_offset.arguments[2].offset = 32;
    reject_ir(wrong_offset);

    let mut wrong_alignment = wave64_collectives_v1_kernel_ir();
    wrong_alignment.arguments[1].alignment = 4;
    reject_ir(wrong_alignment);

    let mut wrong_role = wave64_collectives_v1_kernel_ir();
    wrong_role.arguments[1].role = Wave64ArgumentRoleV1::Input;
    reject_ir(wrong_role);
}

#[test]
fn collective_kind_order_and_count_are_closed() {
    let mut wrong_kind = wave64_collectives_v1_kernel_ir();
    wrong_kind.collectives[0].kind = Wave64CollectiveKindV1::InclusiveScanSum;
    reject_ir(wrong_kind);

    let mut wrong_order = wave64_collectives_v1_kernel_ir();
    wrong_order.collectives.swap(0, 1);
    reject_ir(wrong_order);

    let mut wrong_ordinal = wave64_collectives_v1_kernel_ir();
    wrong_ordinal.collectives[2].ordinal = 1;
    reject_ir(wrong_ordinal);

    let mut missing = wave64_collectives_v1_kernel_ir();
    missing.collectives.pop();
    reject_ir(missing);

    let mut divergent = wave64_collectives_v1_kernel_ir();
    divergent.collectives[1].participation =
        fe2o3_kernel_ir::Wave64ParticipationV1::DivergentLogicalParticipants;
    reject_ir(divergent);
}

#[test]
fn output_role_ownership_and_inactive_policy_are_closed() {
    let mut substituted = wave64_collectives_v1_kernel_ir();
    substituted.outputs[0].argument = Wave64ArgumentRoleV1::InclusiveOutput;
    reject_ir(substituted);

    let mut wrong_source = wave64_collectives_v1_kernel_ir();
    wrong_source.outputs[2].source = Wave64CollectiveKindV1::InclusiveScanSum;
    reject_ir(wrong_source);

    let mut missing = wave64_collectives_v1_kernel_ir();
    missing.outputs.pop();
    reject_ir(missing);

    let mut wrong_ownership = wave64_collectives_v1_kernel_ir();
    wrong_ownership.outputs[0].ownership =
        fe2o3_kernel_ir::Wave64OutputOwnershipV1::LaneZeroOwnsEveryIndex;
    reject_ir(wrong_ownership);

    let mut wrong_inactive = wave64_collectives_v1_kernel_ir();
    wrong_inactive.outputs[1].inactive_policy =
        fe2o3_kernel_ir::Wave64InactivePolicyV1::PublishCollectiveResult;
    reject_ir(wrong_inactive);
}

#[test]
fn descriptor_and_abi_identity_are_closed_by_the_profile() {
    let mutations: Vec<ProfileMutation> = vec![
        |value| value.descriptor.export_name.push_str("_substitution"),
        |value| value.descriptor.descriptor_symbol = "wave64_collectives_v1.bad".into(),
        |value| value.descriptor.code_object_version = 5,
        |value| value.descriptor.explicit_kernarg_bytes = 64,
        |value| value.descriptor.complete_kernarg_bytes = 72,
        |value| value.descriptor.workgroup_size = WorkgroupSize::new(32, 1, 1),
        |value| value.descriptor.wave_width = WaveWidth::Wave32,
    ];
    for mutate in mutations {
        let mut profile = Wave64CollectivesProfileV1::exact_gfx942_xnack_minus_cov6();
        mutate(&mut profile);
        assert_eq!(
            verify_wave64_collectives_v1(&wave64_collectives_v1_kernel_ir(), &profile),
            Err(Wave64CollectivesV1Error::UnsupportedProfile)
        );
    }
}

fn reject_ir(ir: fe2o3_kernel_ir::Wave64CollectivesKernelIrV1) {
    assert_eq!(
        verify_wave64_collectives_v1(
            &ir,
            &Wave64CollectivesProfileV1::exact_gfx942_xnack_minus_cov6()
        ),
        Err(Wave64CollectivesV1Error::NonCanonicalKernelIr)
    );
}
