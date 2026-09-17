// Included in generated_kfd_arguments::tests to reuse its private ABI fixtures.

fn descriptor_slice_plan(read_count: u16) -> GeneratedArgumentPackingPlanV1 {
    use fe2o3_kernel_descriptor::{
        AccessMode, BlockSizeV1, BuildEvidenceV1, CanonicalCodeObjectDigest, CodeObjectVersion,
        CompilerIdentityV1, DeviceDescriptorTableV1, DeviceLayoutDescriptorV1,
        DeviceLayoutRecordV1, DeviceTargetV1, DimensionsV1, EvidenceDigest, EvidenceIdentity,
        KernelAbiLayoutV1, KernelDescriptorV1, LaunchConstraintsV1, LogicalArgumentV1,
        ProducerIdentityV1, ScalarTypeV1, SourceTypeDescriptorV1, SourceTypeRecordV1, Text,
        ValidName,
    };
    let shared = SourceTypeRecordV1::new(SourceTypeDescriptorV1::shared_slice(ScalarTypeV1::F32));
    let exclusive =
        SourceTypeRecordV1::new(SourceTypeDescriptorV1::disjoint_slice(ScalarTypeV1::F32));
    let shared_layout =
        DeviceLayoutRecordV1::new(DeviceLayoutDescriptorV1::shared_slice(ScalarTypeV1::F32));
    let exclusive_layout =
        DeviceLayoutRecordV1::new(DeviceLayoutDescriptorV1::disjoint_slice(ScalarTypeV1::F32));
    let mut arguments = Vec::new();
    let mut fields = Vec::new();
    for index in 0..=read_count {
        let name = format!("arg{index}");
        let offset = u32::from(index) * 16;
        arguments.push(if index == read_count {
            LogicalArgumentV1::disjoint_slice(
                index,
                ValidName::new(&name).unwrap(),
                &exclusive,
                &exclusive_layout,
                AccessMode::ReadWrite,
                offset,
            )
            .unwrap()
        } else {
            LogicalArgumentV1::shared_slice(
                index,
                ValidName::new(&name).unwrap(),
                &shared,
                &shared_layout,
                offset,
            )
            .unwrap()
        });
        fields.push(slice_field::<f32>(
            &name,
            u64::from(offset),
            index == read_count,
        ));
    }
    let explicit = (u32::from(read_count) + 1) * 16;
    let evidence = BuildEvidenceV1::new(
        EvidenceIdentity::from_opaque_bytes([1; 32]),
        EvidenceDigest::from_sha256_bytes([2; 32]),
    );
    let kernel = KernelDescriptorV1::new(
        KernelId::from_bytes([3; 32]),
        ValidName::new("slice_test").unwrap(),
        ValidName::new("slice_test").unwrap(),
        ValidName::new("slice_test.kd").unwrap(),
        evidence,
        evidence,
        vec![],
        KernelAbiLayoutV1::new(explicit, explicit + 256, 8).unwrap(),
        LaunchConstraintsV1::new(
            1,
            BlockSizeV1::Exact(DimensionsV1::new(256, 1, 1).unwrap()),
            DimensionsV1::new(u32::MAX, 1, 1).unwrap(),
            256,
            0,
            0,
        )
        .unwrap(),
        arguments,
    )
    .unwrap();
    let (types, layouts) = if read_count == 0 {
        (vec![exclusive], vec![exclusive_layout])
    } else {
        (
            vec![shared, exclusive],
            vec![shared_layout, exclusive_layout],
        )
    };
    let table = DeviceDescriptorTableV1::new(
        CanonicalCodeObjectDigest::from_bytes([4; 32]),
        CodeObjectVersion::V6,
        CompilerIdentityV1::new(
            Text::new("rustc").unwrap(),
            Text::new("test").unwrap(),
            [5; 20],
        ),
        ProducerIdentityV1::new(
            Text::new("cargo-fe2o3").unwrap(),
            Text::new("test").unwrap(),
        ),
        DeviceTargetV1::new(fe2o3_amd_target::AmdTargetId::parse("gfx942:xnack-").unwrap()),
        types,
        layouts,
        vec![kernel],
    )
    .unwrap();
    let generated = CompilerGeneratedArgumentLayoutV1::new(
        u64::from(explicit),
        8,
        PointerWidth::Bits64,
        fields,
    )
    .unwrap();
    validate_worker_v3_argument_packing(&table, &table.kernels()[0], &generated).unwrap()
}

fn packed_vecadd<'allocation>(
    plan: &GeneratedArgumentPackingPlanV1,
    left: &'allocation [f32],
    right: &'allocation [f32],
    output: &'allocation mut [f32],
) -> GeneratedKfdPackedArguments<'allocation> {
    GeneratedKfdArgumentBinding::from_compiler_generated_parts(
        vec![],
        vec![
            GeneratedKfdReadSlice::new(left)
                .bind_argument(plan, 0)
                .unwrap(),
            GeneratedKfdReadSlice::new(right)
                .bind_argument(plan, 1)
                .unwrap(),
            GeneratedKfdReadWriteSlice::new(output)
                .bind_argument(plan, 2)
                .unwrap(),
        ],
    )
    .pack(plan)
    .unwrap()
}

fn refresh_packing_observation(packed: &mut GeneratedKfdPackedArguments<'_>) {
    packed.packing_observation.explicit_kernarg_sha256 =
        Sha256::digest(&packed.explicit_kernarg).into();
    packed.packing_observation.identity =
        packing_observation_identity(&packed.packing_observation).unwrap();
}

#[test]
fn packed_consistency_checks_descriptor_vecadd_and_single_output_abis() {
    let plan = descriptor_slice_plan(2);
    for length in [0, 1, 255, 256, 257, 1023] {
        let input = vec![1.0; length];
        let mut output = vec![-7.0; length];
        let packed = packed_vecadd(&plan, &input, &input, &mut output);
        assert_eq!(packed.explicit_kernarg().len(), 48);
        assert_eq!(packed.alignment(), 8);
        packed.validate_packed_consistency(&plan).unwrap();
        drop(packed);
        assert_eq!(output, vec![-7.0; length]);
    }
    let plan = descriptor_slice_plan(0);
    let mut output = [-7.0_f32; 3];
    let packed = GeneratedKfdArgumentBinding::from_compiler_generated_parts(
        vec![],
        vec![
            GeneratedKfdReadWriteSlice::new(&mut output)
                .bind_argument(&plan, 0)
                .unwrap(),
        ],
    )
    .pack(&plan)
    .unwrap();
    assert_eq!(packed.explicit_kernarg().len(), 16);
    packed.validate_packed_consistency(&plan).unwrap();
}

#[test]
fn packed_consistency_does_not_discharge_cross_argument_access_bounds() {
    let plan = descriptor_slice_plan(2);
    let mut output = [-7.0; 2];
    let packed = packed_vecadd(&plan, &[1.0], &[2.0; 3], &mut output);
    // Each length describes its own storage; no kernel is authorized or executed here.
    packed.validate_packed_consistency(&plan).unwrap();
}

#[test]
fn packed_consistency_accepts_reordered_bindings_and_empty_arguments() {
    let plan = descriptor_slice_plan(2);
    let mut output = [-7.0_f32; 2];
    let packed = GeneratedKfdArgumentBinding::from_compiler_generated_parts(
        vec![],
        vec![
            GeneratedKfdReadWriteSlice::new(&mut output)
                .bind_argument(&plan, 2)
                .unwrap(),
            GeneratedKfdReadSlice::new(&[1.0_f32])
                .bind_argument(&plan, 1)
                .unwrap(),
            GeneratedKfdReadSlice::<f32>::new(&[])
                .bind_argument(&plan, 0)
                .unwrap(),
        ],
    )
    .pack(&plan)
    .unwrap();
    packed.validate_packed_consistency(&plan).unwrap();
}

fn assert_packed_substitution_rejected(mutate: impl FnOnce(&mut GeneratedKfdPackedArguments<'_>)) {
    let plan = descriptor_slice_plan(2);
    let mut output = [-7.0; 2];
    let mut packed = packed_vecadd(&plan, &[1.0; 2], &[1.0; 2], &mut output);
    mutate(&mut packed);
    // Rebind descriptive hashes so semantic consistency, not a stale digest, must reject.
    refresh_packing_observation(&mut packed);
    assert!(packed.validate_packed_consistency(&plan).is_err());
    drop(packed);
    assert_eq!(output, [-7.0; 2]);
}

#[test]
fn packed_consistency_rejects_storage_fixup_and_completion_substitution() {
    assert_packed_substitution_rejected(|packed| packed.explicit_kernarg[0] = 1);
    assert_packed_substitution_rejected(|packed| {
        packed.explicit_kernarg[8..16].copy_from_slice(&3_u64.to_le_bytes());
    });
    assert_packed_substitution_rejected(|packed| {
        packed.buffers[0] =
            Gfx942RuntimeDispatchBufferV1::new(vec![0; 4], Gfx942RuntimeBufferAccessV1::ReadOnly)
                .unwrap();
    });
    assert_packed_substitution_rejected(|packed| {
        packed.buffers[0] = Gfx942RuntimeDispatchBufferV1::new(
            packed.buffers[0].bytes().to_vec(),
            Gfx942RuntimeBufferAccessV1::ReadWrite,
        )
        .unwrap();
    });
    assert_packed_substitution_rejected(|packed| {
        packed.pointer_fixups.pop();
    });
    assert_packed_substitution_rejected(|packed| {
        packed.pointer_fixups.push(packed.pointer_fixups[0]);
    });
    assert_packed_substitution_rejected(|packed| {
        packed.pointer_fixups[1] = packed.pointer_fixups[0];
    });
    for fixup in [
        Gfx942KfdDispatchPointerFixupV1::new(8, 0, 0, 4),
        Gfx942KfdDispatchPointerFixupV1::new(0, 1, 0, 4),
        Gfx942KfdDispatchPointerFixupV1::new(0, 0, 4, 4),
        Gfx942KfdDispatchPointerFixupV1::new(0, 0, 0, 1),
    ] {
        assert_packed_substitution_rejected(|packed| packed.pointer_fixups[0] = fixup);
    }
    assert_packed_substitution_rejected(|packed| {
        packed.packing_observation.buffers[0].argument_index = 1;
    });
    assert_packed_substitution_rejected(|packed| {
        packed.packing_observation.buffers[0].buffer_index = Some(1);
    });
    assert_packed_substitution_rejected(|packed| {
        packed.packing_observation.buffers.pop();
    });
    assert_packed_substitution_rejected(|packed| {
        packed.buffers.push(
            Gfx942RuntimeDispatchBufferV1::new(vec![0; 8], Gfx942RuntimeBufferAccessV1::ReadOnly)
                .unwrap(),
        );
    });
    assert_packed_substitution_rejected(|packed| {
        packed.completion.buffers[2].writeback = None;
    });
    assert_packed_substitution_rejected(|packed| {
        packed.completion.buffers[2].byte_len = 4;
    });
}

#[test]
fn packed_consistency_checks_length_multiplication_and_empty_fixups() {
    let plan = descriptor_slice_plan(2);
    let mut output = [-7.0; 2];
    let mut packed = packed_vecadd(&plan, &[1.0], &[2.0], &mut output);
    packed.explicit_kernarg[8..16].copy_from_slice(&u64::MAX.to_le_bytes());
    refresh_packing_observation(&mut packed);
    assert!(matches!(
        packed.validate_packed_consistency(&plan),
        Err(GeneratedKfdPrepareError::Bind(
            GeneratedKfdArgumentError::BufferByteLength { argument_index: 0 }
        ))
    ));
    drop(packed);
    assert_eq!(output, [-7.0; 2]);

    let mut packed = packed_vecadd(&plan, &[], &[2.0], &mut output);
    packed.pointer_fixups[0] = Gfx942KfdDispatchPointerFixupV1::new(0, 0, 0, 4);
    assert!(matches!(
        packed.validate_packed_consistency(&plan),
        Err(GeneratedKfdPrepareError::PackedSubstitution)
    ));
}

#[test]
fn packed_consistency_checks_scalar_and_slice_abi() {
    let plan = plan();
    let input = [1_i32, 2, 3];
    let mut output = [i32::MIN, i32::MIN];
    let scalar = plan.scalar(2, 7_u32).unwrap();
    let input = GeneratedKfdReadSlice::new(&input)
        .bind_argument(&plan, 0)
        .unwrap();
    let output_binding = GeneratedKfdReadWriteSlice::new(&mut output)
        .bind_argument(&plan, 1)
        .unwrap();
    let packed = GeneratedKfdArgumentBinding::from_compiler_generated_parts(
        vec![scalar],
        vec![input, output_binding],
    )
    .pack(&plan)
    .unwrap();

    packed.validate_packed_consistency(&plan).unwrap();
    drop(packed);
    assert_eq!(output, [i32::MIN, i32::MIN]);
}

#[test]
fn packed_consistency_checks_write_only_abi() {
    let plan = write_only_plan(None);
    let mut output = [0x1122_3344_i32, 0x5566_7788];
    let binding = GeneratedKfdWriteSlice::new(&mut output)
        .bind_argument(&plan, 0)
        .unwrap();
    let packed =
        GeneratedKfdArgumentBinding::from_compiler_generated_parts(Vec::new(), vec![binding])
            .pack(&plan)
            .unwrap();

    packed.validate_packed_consistency(&plan).unwrap();
    drop(packed);
    assert_eq!(output, [0x1122_3344, 0x5566_7788]);
}
