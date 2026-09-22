use super::*;
use fe2o3_hsaco_finalize::{
    NOMINAL_DESCRIPTOR_SCRATCH_STORAGE_V3 as SCRATCH, NominalFinalizationErrorV3,
    derive_unfinalized_nominal_hsaco_v3, finalize_unfinalized_nominal_hsaco_v3,
    inspect_finalized_nominal_hsaco_v3, inspect_unfinalized_nominal_hsaco_v3,
};
use fe2o3_kernel_descriptor::*;

fn free(_: usize) -> Result<(), &'static str> {
    Ok(())
}
const NOMINAL: [SourceTypeDescriptorV3; 2] =
    [SourceTypeDescriptorV3::Usize, SourceTypeDescriptorV3::Isize];

fn wire(kinds: [SourceTypeDescriptorV3; 2]) -> Vec<u8> {
    wire_for_target(kinds, GENERAL_V3_TARGET)
}

fn wire_for_target(kinds: [SourceTypeDescriptorV3; 2], target: &str) -> Vec<u8> {
    let compiler = CompilerIdentityV1::new(
        Text::new("rustc").unwrap(),
        Text::new("fixture").unwrap(),
        [7; 20],
    );
    let producer = ProducerIdentityV1::new(
        Text::new("inert-nominal-policy4-output-v3").unwrap(),
        Text::new("test").unwrap(),
    );
    let by_argument = kinds.map(|k| SourceTypeRecordV3::new(k, &mut free).unwrap());
    let mut sources = by_argument;
    sources.sort_by_key(|r| r.identity());
    let by_layout = [ScalarTypeV1::U64, ScalarTypeV1::I64]
        .map(|s| device_layout_record_v3(DeviceLayoutDescriptorV1::scalar(s), &mut free).unwrap());
    let mut layouts = by_layout.clone();
    layouts.sort_by_key(|r| r.identity());
    let components = [ScalarTypeV1::U64, ScalarTypeV1::I64].map(|s| PhysicalComponentV3 {
        kind: PhysicalAbiComponentKind::ScalarByValue(s),
        offset: if s == ScalarTypeV1::U64 { 0 } else { 8 },
        size: 8,
        alignment: 8,
        access: AccessMode::ByValue,
        alias: AliasSemantics::Value,
    });
    let args = [0, 1].map(|i| LogicalArgumentInputV3 {
        source_index: i as u16,
        name: ["count", "delta"][i],
        source_type: by_argument[i].identity(),
        device_layout: by_layout[i].identity(),
        ownership: OwnershipSemantics::ByValue,
        access: AccessMode::ByValue,
        alias: AliasSemantics::Value,
        components: &components[i..i + 1],
    });
    let launch = LaunchConstraintsV1::new(
        1,
        BlockSizeV1::Exact(DimensionsV1::new(256, 1, 1).unwrap()),
        DimensionsV1::new(u32::MAX, 1, 1).unwrap(),
        256,
        0,
        0,
    )
    .unwrap();
    let evidence = BuildEvidenceV1::new(
        EvidenceIdentity::from_opaque_bytes([11; 32]),
        EvidenceDigest::from_sha256_bytes([12; 32]),
    );
    let id = KernelId::from_bytes([0xa1; 32]);
    let kernels = [KernelDescriptorInputV3 {
        kernel_id: id,
        logical_name: "alpha",
        entry_name: "alpha",
        descriptor_symbol: "alpha.kd",
        source_evidence: evidence,
        executable_ir_evidence: evidence,
        capabilities: &[CapabilityV1::AmdWave],
        abi_layout: KernelAbiLayoutV1::new(16, 272, 8).unwrap(),
        launch: &launch,
        arguments: &args,
    }];
    let requirements = [KernelTargetRequirementsV2::new(
        id,
        LdsRequirementsV2::new(0, 0).unwrap(),
        RequiredWavefrontWidthV2::Wave64,
        false,
        SynchronizationRequirementsV2::empty(),
        AtomicRequirementsV2::empty(),
    )];
    let input = DeviceDescriptorTableInputV3 {
        canonical_code_object_digest: CanonicalCodeObjectDigest::from_bytes([0; 32]),
        code_object_version: CodeObjectVersion::V6,
        compiler: &compiler,
        producer: &producer,
        device_target: DeviceTargetV1::parse(target).unwrap(),
        type_records: &sources,
        layout_records: &layouts,
        kernels: &kernels,
        requirements: &requirements,
    };
    let mut bytes = vec![0; encoded_device_descriptor_table_v3_len(&input, &mut free).unwrap()];
    encode_device_descriptor_table_v3(&input, &mut bytes, &mut free).unwrap();
    bytes
}

fn nominal_fixture(
    wire: &[u8],
    hidden: bool,
    size: u32,
    mutate: impl FnOnce(&mut Value),
    count: usize,
    extra: &[&str],
) -> Fixture {
    nominal_fixture_for_target(wire, hidden, size, mutate, count, extra, GENERAL_V3_TARGET)
}

fn nominal_fixture_for_target(
    wire: &[u8],
    hidden: bool,
    size: u32,
    mutate: impl FnOnce(&mut Value),
    count: usize,
    extra: &[&str],
    target: &str,
) -> Fixture {
    let mut root = rmpv::decode::read_value(
        &mut general_v3_metadata(GeneralV3Kernel::Alpha, size, 8, target).as_slice(),
    )
    .unwrap();
    let Value::Array(kernels) = field_mut(&mut root, "amdhsa.kernels") else {
        panic!("kernels");
    };
    let kernel = &mut kernels[0];
    if target == "gfx1151" {
        set_field(kernel, ".wavefront_size", 32.into());
        set_field(kernel, ".agpr_count", 0.into());
        set_field(kernel, ".vgpr_count", 7.into());
    }
    let mut args = vec![
        argument(Some("count"), 0, 8, "by_value", None),
        argument(Some("delta"), 8, 8, "by_value", None),
    ];
    for (arg, scalar) in args.iter_mut().zip(["u64", "i64"]) {
        map_mut(arg).push((Value::from(".value_type"), Value::from(scalar)));
    }
    if hidden {
        args.extend(v5_hidden_arguments(16));
    }
    set_field(kernel, ".args", Value::Array(args));
    mutate(kernel);
    let mut metadata = Vec::new();
    write_value(&mut metadata, &root).unwrap();
    let mut fixture =
        build_fixture_for_kernels(wire, &metadata, count, extra, &[("alpha", "alpha.kd")], 0);
    fixture.bytes[8] = 4;
    if target == GENERAL_V3_TARGET {
        write_u32(&mut fixture.bytes, 48, 0x64c);
    }
    for offset in fixture.kernel_descriptor_offsets.iter().copied() {
        write_u32(&mut fixture.bytes, offset + 8, size);
        if target == GENERAL_V3_TARGET {
            write_u32(&mut fixture.bytes, offset + 44, 1);
            write_u32(&mut fixture.bytes, offset + 48, 0x00af_0081);
            write_u16(&mut fixture.bytes, offset + 56, 0x001e);
        }
    }
    if count != 0 {
        let name = section_name_file_offset(&fixture.bytes, DESCRIPTOR_SECTION_INDEX);
        fixture.bytes[name..name + 12].copy_from_slice(b".fe2o3.kd.v3");
    }
    fixture
}

#[test]
fn exact_digest_round_trip_preserves_nominal_types_and_both_cov6_forms() {
    let source = wire(NOMINAL);
    for (hidden, size) in [(false, 16), (true, 272)] {
        let fixture = nominal_fixture(&source, hidden, size, |_| {}, 1, &[]);
        let mut hash = Sha256::new();
        hash.update(b"FE2O3/AMDHSA-CODE-OBJECT/V1\0");
        hash.update((fixture.bytes.len() as u64).to_le_bytes());
        hash.update(&fixture.bytes);
        let digest: [u8; 32] = hash.finalize().into();
        let mut expected = fixture.bytes.clone();
        let start = fixture.descriptor_offsets[0];
        expected[start + 16..start + 48].copy_from_slice(&digest);
        let result =
            finalize_unfinalized_nominal_hsaco_v3(&fixture.bytes, &source, SCRATCH, &mut free)
                .unwrap();
        assert_eq!(result.as_bytes(), expected);
        assert!(result.artifact_byte_capacity() >= expected.len());
        assert_eq!(result.digest().as_bytes(), &digest);
        assert!(!result.grants_launch_authority());
        let view =
            inspect_finalized_nominal_hsaco_v3(result.as_bytes(), SCRATCH, &mut free).unwrap();
        let table = view.descriptor_table();
        let kernel = table.kernel(0, &mut free).unwrap();
        assert_eq!(kernel.kernel_id(), KernelId::from_bytes([0xa1; 32]));
        assert_eq!(
            kernel.abi_layout(),
            KernelAbiLayoutV1::new(16, 272, 8).unwrap()
        );
        let mut args = kernel.arguments();
        for expected in NOMINAL {
            let arg = args.next(&mut free).unwrap().unwrap();
            assert_eq!(
                table
                    .source_type(arg.source_type(), &mut free)
                    .unwrap()
                    .descriptor(),
                expected
            );
        }
        assert!(args.next(&mut free).unwrap().is_none());
        assert_eq!(
            derive_unfinalized_nominal_hsaco_v3(result.as_bytes(), SCRATCH, &mut free).unwrap(),
            fixture.bytes
        );
        assert!(matches!(
            inspect_unfinalized(&fixture.bytes),
            Err(FinalizationError::DescriptorSectionVersionMismatch)
        ));
    }
}

#[test]
fn physically_equal_nominal_substitutions_cannot_match_source_receipt() {
    let source = wire(NOMINAL);
    for kinds in [
        [
            SourceTypeDescriptorV3::Scalar(ScalarTypeV1::U64),
            NOMINAL[1],
        ],
        [
            NOMINAL[0],
            SourceTypeDescriptorV3::Scalar(ScalarTypeV1::I64),
        ],
    ] {
        let substituted = wire(kinds);
        assert_ne!(source, substituted);
        let fixture = nominal_fixture(&substituted, false, 16, |_| {}, 1, &[]);
        let view =
            inspect_unfinalized_nominal_hsaco_v3(&fixture.bytes, SCRATCH, &mut free).unwrap();
        assert!(!view.grants_launch_authority());
        assert!(matches!(
            finalize_unfinalized_nominal_hsaco_v3(&fixture.bytes, &source, SCRATCH, &mut free),
            Err(NominalFinalizationErrorV3::DescriptorSourceMismatch)
        ));
    }
}

#[test]
fn rejects_noncanonical_tail_and_physical_disagreement() {
    let source = wire(NOMINAL);
    for (hidden, size) in [(true, 264), (true, 280), (false, 272)] {
        let fixture = nominal_fixture(&source, hidden, size, |_| {}, 1, &[]);
        assert!(inspect_unfinalized_nominal_hsaco_v3(&fixture.bytes, SCRATCH, &mut free).is_err());
    }
    let mut fixture = nominal_fixture(&source, false, 16, |_| {}, 1, &[]);
    let kd = fixture.kernel_descriptor_offsets[0];
    write_u32(&mut fixture.bytes, kd + 8, 272);
    assert!(inspect_unfinalized_nominal_hsaco_v3(&fixture.bytes, SCRATCH, &mut free).is_err());
}

#[test]
fn rejects_launch_and_argument_changes() {
    let source = wire(NOMINAL);
    for field in [
        ".max_flat_workgroup_size",
        ".reqd_workgroup_size",
        ".group_segment_fixed_size",
        ".kernarg_segment_align",
        ".args",
    ] {
        let fixture = nominal_fixture(
            &source,
            false,
            16,
            |kernel| match field {
                ".reqd_workgroup_size" => set_field(
                    kernel,
                    field,
                    Value::Array(vec![128.into(), 1.into(), 1.into()]),
                ),
                ".args" => {
                    set_field(&mut arguments_mut(kernel)[0], ".value_type", "i64".into());
                }
                ".kernarg_segment_align" => set_field(kernel, field, 16.into()),
                _ => set_field(kernel, field, 128.into()),
            },
            1,
            &[],
        );
        assert!(
            inspect_unfinalized_nominal_hsaco_v3(&fixture.bytes, SCRATCH, &mut free).is_err(),
            "{field}"
        );
    }
}

#[test]
fn requires_declared_wave_width_not_just_metadata_hardware_agreement() {
    // gfx1151 supports both widths; its fixture metadata and hardware agree on
    // Wave32, while this independently canonical V3 table requires Wave64.
    let source = wire_for_target(NOMINAL, "gfx1151");
    let fixture = nominal_fixture_for_target(&source, false, 16, |_| {}, 1, &[], "gfx1151");
    let error =
        inspect_unfinalized_nominal_hsaco_v3(&fixture.bytes, SCRATCH, &mut free).unwrap_err();
    assert!(
        matches!(
            error,
            NominalFinalizationErrorV3::Artifact(FinalizationError::KernelWavefrontSizeMismatch {
                descriptor: 64,
                metadata: 32,
                hardware: 32,
                ..
            })
        ),
        "{error:?}"
    );
}

#[test]
fn descriptor_target_must_match_artifact_target() {
    let source = wire_for_target(NOMINAL, "gfx950:xnack-");
    let fixture = nominal_fixture(&source, false, 16, |_| {}, 1, &[]);
    assert!(matches!(
        inspect_unfinalized_nominal_hsaco_v3(&fixture.bytes, SCRATCH, &mut free),
        Err(NominalFinalizationErrorV3::Artifact(
            FinalizationError::DeviceTargetMismatch
        ))
    ));
}

#[test]
fn rejects_missing_duplicate_mixed_and_wrong_version_sections() {
    let source = wire(NOMINAL);
    for count in [0, 2] {
        let fixture = nominal_fixture(&source, false, 16, |_| {}, count, &[]);
        assert!(matches!(
            inspect_unfinalized_nominal_hsaco_v3(&fixture.bytes, SCRATCH, &mut free),
            Err(NominalFinalizationErrorV3::Artifact(
                FinalizationError::MissingDescriptorSection
                    | FinalizationError::DuplicateDescriptorSection
            ))
        ));
    }
    let legacy = valid_fixture();
    assert!(matches!(
        inspect_unfinalized_nominal_hsaco_v3(&legacy.bytes, SCRATCH, &mut free),
        Err(NominalFinalizationErrorV3::Artifact(
            FinalizationError::DescriptorSectionVersionMismatch
        ))
    ));
    let mut fixture = nominal_fixture(&source, false, 16, |_| {}, 1, &[]);
    let start = fixture.descriptor_offsets[0];
    fixture.bytes[start + 8..start + 10].copy_from_slice(&1u16.to_le_bytes());
    assert!(matches!(
        inspect_unfinalized_nominal_hsaco_v3(&fixture.bytes, SCRATCH, &mut free),
        Err(NominalFinalizationErrorV3::Wire(_))
    ));

    // Both sections hold independently valid wire, in both section-header orders.
    let legacy_view = inspect_unfinalized(&legacy.bytes).unwrap();
    let legacy_wire = encode_device_descriptor_table_v1(legacy_view.descriptor_table()).unwrap();
    let mut mixed = nominal_fixture(&source, false, 16, |_| {}, 1, &[".fe2o3.kd.v1"]);
    align(&mut mixed.bytes, 8);
    let offset = mixed.bytes.len();
    mixed.bytes.extend_from_slice(&legacy_wire);
    let extra = mixed.extra_headers[0];
    write_u64(&mut mixed.bytes, extra + 24, offset as u64);
    write_u64(&mut mixed.bytes, extra + 32, legacy_wire.len() as u64);
    write_u64(&mut mixed.bytes, extra + 48, 8);
    for swap in [false, true] {
        if swap {
            let first = mixed.descriptor_headers[0];
            for i in 0..64 {
                mixed.bytes.swap(first + i, extra + i);
            }
        }
        assert!(matches!(
            inspect_unfinalized_nominal_hsaco_v3(&mixed.bytes, SCRATCH, &mut free),
            Err(NominalFinalizationErrorV3::Artifact(
                FinalizationError::DescriptorSectionVersionMismatch
            ))
        ));
        assert!(matches!(
            inspect_unfinalized(&mixed.bytes),
            Err(FinalizationError::DescriptorSectionVersionMismatch)
        ));
    }
}

#[test]
fn digest_states_and_tampering_fail_without_mutating_input() {
    let source = wire(NOMINAL);
    let fixture = nominal_fixture(&source, false, 16, |_| {}, 1, &[]);
    assert!(matches!(
        inspect_finalized_nominal_hsaco_v3(&fixture.bytes, SCRATCH, &mut free),
        Err(NominalFinalizationErrorV3::Artifact(
            FinalizationError::ExpectedFinalizedDigest
        ))
    ));
    let final_artifact =
        finalize_unfinalized_nominal_hsaco_v3(&fixture.bytes, &source, SCRATCH, &mut free).unwrap();
    assert!(matches!(
        finalize_unfinalized_nominal_hsaco_v3(
            final_artifact.as_bytes(),
            &source,
            SCRATCH,
            &mut free
        ),
        Err(NominalFinalizationErrorV3::Artifact(
            FinalizationError::ExpectedZeroDigest
        ))
    ));
    for index in [
        final_artifact.location().digest_offset(),
        fixture.descriptor_offsets[0] - 1,
    ] {
        let mut corrupt = final_artifact.as_bytes().to_vec();
        corrupt[index] ^= 1;
        let before = corrupt.clone();
        assert!(inspect_finalized_nominal_hsaco_v3(&corrupt, SCRATCH, &mut free).is_err());
        assert!(derive_unfinalized_nominal_hsaco_v3(&corrupt, SCRATCH, &mut free).is_err());
        assert_eq!(corrupt, before);
    }
}

#[test]
fn scratch_and_work_refusals_leave_input_unchanged() {
    let source = wire(NOMINAL);
    let fixture = nominal_fixture(&source, false, 16, |_| {}, 1, &[]);
    let before = fixture.bytes.clone();
    let mut calls = 0;
    assert!(matches!(
        finalize_unfinalized_nominal_hsaco_v3(&fixture.bytes, &source, SCRATCH - 1, &mut |_| {
            calls += 1;
            Ok::<_, &'static str>(())
        }),
        Err(NominalFinalizationErrorV3::Scratch { .. })
    ));
    assert_eq!(calls, 0);
    let mut total = 0usize;
    finalize_unfinalized_nominal_hsaco_v3(&fixture.bytes, &source, SCRATCH, &mut |n| {
        total += n;
        Ok::<_, &'static str>(())
    })
    .unwrap();
    for initial in [0, total - 1] {
        let mut remaining = initial;
        let error =
            finalize_unfinalized_nominal_hsaco_v3(&fixture.bytes, &source, SCRATCH, &mut |n| {
                remaining = remaining.checked_sub(n).ok_or("exhausted")?;
                Ok(())
            })
            .unwrap_err();
        assert!(matches!(
            error,
            NominalFinalizationErrorV3::Work("exhausted")
                | NominalFinalizationErrorV3::Wire(DescriptorWireErrorV3::Work("exhausted"))
        ));
    }
    assert_eq!(fixture.bytes, before);
}

#[test]
fn table_and_kernel_comparisons_have_independent_work_charges() {
    let source = wire(NOMINAL);
    let fixture = nominal_fixture(&source, false, 16, |_| {}, 1, &[]);
    let mut decode_calls = 0;
    let table = decode_device_descriptor_table_v3(&source, &mut |_| {
        decode_calls += 1;
        free(0)
    })
    .unwrap();
    let mut kernel_calls = 0;
    table
        .kernel(0, &mut |_| {
            kernel_calls += 1;
            free(0)
        })
        .unwrap();
    // The initial inspector charge precedes decoding; table facts and the first
    // kernel's fixed checks are additional to every decoder/query callback.
    for (stop, expected) in [
        (1 + decode_calls, 64),
        (2 + decode_calls + kernel_calls, "alpha.kd".len() + 128),
    ] {
        let mut calls = 0;
        let error = inspect_unfinalized_nominal_hsaco_v3(&fixture.bytes, SCRATCH, &mut |n| {
            let current = calls;
            calls += 1;
            if current == stop {
                assert_eq!(n, expected);
                Err("comparison denied")
            } else {
                Ok(())
            }
        })
        .unwrap_err();
        assert_eq!(calls, stop + 1);
        assert!(matches!(
            error,
            NominalFinalizationErrorV3::Work("comparison denied")
        ));
    }
}
