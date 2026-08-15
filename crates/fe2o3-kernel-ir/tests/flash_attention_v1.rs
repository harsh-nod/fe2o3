use fe2o3_kernel_ir::*;

fn exact() -> (FlashAttentionKernelIrV1, FlashAttentionProfileV1) {
    (
        flash_attention_v1_kernel_ir(),
        FlashAttentionProfileV1::exact_gfx942_xnack_minus_cov6(),
    )
}

#[test]
fn exact_profile_and_closed_sidecar_verify() {
    let (ir, profile) = exact();
    verify_flash_attention_v1(&ir, &profile).unwrap();
    assert_eq!(
        ir.shape,
        FlashAttentionShapeV1 {
            batches: 1,
            heads: 1,
            sequence_length: 8,
            head_dimension: 16,
        }
    );
    assert_eq!(ir.recurrence.len(), 10);
    assert_eq!(profile.descriptor.explicit_kernarg_bytes, 64);
    assert_eq!(profile.descriptor.complete_kernarg_bytes, 320);
    assert_eq!(profile.descriptor.resources.static_lds_bytes, 0);
    assert_eq!(profile.descriptor.resources.required_dynamic_lds_bytes, 0);
}

#[test]
fn profile_mutations_fail_closed() {
    let (ir, profile) = exact();
    let mutations: Vec<Box<dyn Fn(&mut FlashAttentionProfileV1)>> = vec![
        Box::new(|p| p.source_sha256[0] ^= 1),
        Box::new(|p| p.namespace[0] ^= 1),
        Box::new(|p| p.target = TargetCapability::Subgroups),
        Box::new(|p| p.code_object_version = 5),
        Box::new(|p| p.wave_width = WaveWidth::Wave32),
        Box::new(|p| p.workgroup_size = WorkgroupSize::new(32, 1, 1)),
        Box::new(|p| p.grid = [2, 1, 1]),
        Box::new(|p| p.descriptor.explicit_kernarg_bytes = 56),
        Box::new(|p| p.descriptor.complete_kernarg_bytes = 312),
        Box::new(|p| p.descriptor.resources.static_lds_bytes = 4),
        Box::new(|p| p.descriptor.resources.maximum_dynamic_lds_bytes = 4),
        Box::new(|p| p.descriptor.export_name.push_str("_mutated")),
    ];
    for mutate in mutations {
        let mut candidate = profile.clone();
        mutate(&mut candidate);
        assert_eq!(
            verify_flash_attention_v1(&ir, &candidate),
            Err(FlashAttentionV1Error::UnsupportedProfile)
        );
    }
}

#[test]
fn semantic_mutations_fail_closed() {
    let (ir, profile) = exact();
    let mutations: Vec<Box<dyn Fn(&mut FlashAttentionKernelIrV1)>> = vec![
        Box::new(|m| m.module_id.push_str("_mutated")),
        Box::new(|m| m.arguments.swap(0, 1)),
        Box::new(|m| m.arguments[3].offset = 40),
        Box::new(|m| m.shape.batches = 2),
        Box::new(|m| m.shape.heads = 2),
        Box::new(|m| m.shape.sequence_length = 7),
        Box::new(|m| m.shape.head_dimension = 8),
        Box::new(|m| {
            m.recurrence[1] = FlashAttentionRecurrenceStepV1::ScaleByExactF32Bits(0x3f00_0000)
        }),
        Box::new(|m| m.recurrence.swap(3, 4)),
        Box::new(|m| m.ownership.physical_lanes = 32),
        Box::new(|m| m.ownership.elements_per_lane = 1),
        Box::new(|m| m.ownership.total = false),
        Box::new(|m| m.ownership.injective = false),
        Box::new(|m| m.ownership.in_bounds = false),
    ];
    for mutate in mutations {
        let mut candidate = ir.clone();
        mutate(&mut candidate);
        assert_eq!(
            verify_flash_attention_v1(&candidate, &profile),
            Err(FlashAttentionV1Error::NonCanonicalKernelIr)
        );
    }
}

#[test]
fn exact_abi_roles_and_recurrence_order_are_explicit() {
    let (ir, _) = exact();
    assert_eq!(
        ir.arguments.map(|argument| argument.role),
        [
            FlashAttentionArgumentRoleV1::Query,
            FlashAttentionArgumentRoleV1::Key,
            FlashAttentionArgumentRoleV1::Value,
            FlashAttentionArgumentRoleV1::Output,
        ]
    );
    assert_eq!(
        ir.recurrence,
        [
            FlashAttentionRecurrenceStepV1::SequentialDotD16,
            FlashAttentionRecurrenceStepV1::ScaleByExactF32Bits(0x3e80_0000),
            FlashAttentionRecurrenceStepV1::FirstKeyInitializesMaxSumAndNumerator,
            FlashAttentionRecurrenceStepV1::NextMax,
            FlashAttentionRecurrenceStepV1::PreviousWeightExp,
            FlashAttentionRecurrenceStepV1::CurrentWeightExp,
            FlashAttentionRecurrenceStepV1::RescaleDenominator,
            FlashAttentionRecurrenceStepV1::RescaleNumeratorPair,
            FlashAttentionRecurrenceStepV1::CommitMaximum,
            FlashAttentionRecurrenceStepV1::DivideNumeratorPairByDenominator,
        ]
    );
}
