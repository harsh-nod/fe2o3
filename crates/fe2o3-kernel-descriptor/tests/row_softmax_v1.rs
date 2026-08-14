use fe2o3_kernel_descriptor::{
    AccessMode, BlockSizeV1, BuildEvidenceV1, CanonicalCodeObjectDigest, CapabilityV1,
    CodeObjectVersion, CompilerIdentityV1, DEVICE_DESCRIPTOR_VERSION, DeviceDescriptorTableV1,
    DeviceLayoutDescriptorV1, DeviceLayoutRecordV1, DeviceTargetV1, DimensionsV1, EvidenceDigest,
    EvidenceIdentity, KernelAbiLayoutV1, KernelDescriptorV1, KernelId, LaunchConstraintsV1,
    LogicalArgumentV1, ProducerIdentityV1, ROW_SOFTMAX_V1_DESCRIPTOR_SYMBOL,
    ROW_SOFTMAX_V1_ROW_ELEMENTS, RowSoftmaxV1StructuralDescriptorErrorV1,
    RowSoftmaxV1StructuralDescriptorExpectationV1, ScalarTypeV1, SourceTypeDescriptorV1,
    SourceTypeRecordV1, Text, ValidName, admit_row_softmax_v1_structural_descriptor_v1,
    decode_device_descriptor_table_v1, encode_device_descriptor_table_v1,
};

#[derive(Clone)]
struct Options {
    target: &'static str,
    code_object_version: CodeObjectVersion,
    entry: &'static str,
    descriptor: &'static str,
    workgroup: u32,
    max_grid: [u32; 3],
    max_flat: u32,
    explicit_bytes: u32,
    total_bytes: u32,
    static_lds: u32,
    dynamic_lds: u32,
    capabilities: Vec<CapabilityV1>,
    input_name: &'static str,
    input_scalar: ScalarTypeV1,
    output_access: AccessMode,
    output_is_shared: bool,
    source_evidence: BuildEvidenceV1,
    executable_ir_evidence: BuildEvidenceV1,
}

impl Options {
    fn exact() -> Self {
        Self {
            target: "gfx942:xnack-",
            code_object_version: CodeObjectVersion::V6,
            entry: "row_softmax_v1",
            descriptor: ROW_SOFTMAX_V1_DESCRIPTOR_SYMBOL,
            workgroup: 64,
            max_grid: [1, 1, 1],
            max_flat: 64,
            explicit_bytes: 32,
            total_bytes: 288,
            static_lds: 0,
            dynamic_lds: 0,
            capabilities: vec![CapabilityV1::Subgroup, CapabilityV1::AmdWave],
            input_name: "input",
            input_scalar: ScalarTypeV1::F32,
            output_access: AccessMode::ReadWrite,
            output_is_shared: false,
            source_evidence: evidence(0x21, 0x22),
            executable_ir_evidence: evidence(0x31, 0x32),
        }
    }
}

fn expectation(options: &Options) -> RowSoftmaxV1StructuralDescriptorExpectationV1 {
    RowSoftmaxV1StructuralDescriptorExpectationV1::new(
        KernelId::from_bytes([0x11; 32]),
        options.source_evidence,
        options.executable_ir_evidence,
    )
    .unwrap()
}

fn table(options: &Options) -> DeviceDescriptorTableV1 {
    let input_source =
        SourceTypeRecordV1::new(SourceTypeDescriptorV1::shared_slice(options.input_scalar));
    let input_layout =
        DeviceLayoutRecordV1::new(DeviceLayoutDescriptorV1::shared_slice(options.input_scalar));
    let output_shared_source =
        SourceTypeRecordV1::new(SourceTypeDescriptorV1::shared_slice(ScalarTypeV1::F32));
    let output_shared_layout =
        DeviceLayoutRecordV1::new(DeviceLayoutDescriptorV1::shared_slice(ScalarTypeV1::F32));
    let output_disjoint_source =
        SourceTypeRecordV1::new(SourceTypeDescriptorV1::disjoint_slice(ScalarTypeV1::F32));
    let output_disjoint_layout =
        DeviceLayoutRecordV1::new(DeviceLayoutDescriptorV1::disjoint_slice(ScalarTypeV1::F32));

    let output = if options.output_is_shared {
        LogicalArgumentV1::shared_slice(
            1,
            name("output"),
            &output_shared_source,
            &output_shared_layout,
            16,
        )
        .unwrap()
    } else {
        LogicalArgumentV1::disjoint_slice(
            1,
            name("output"),
            &output_disjoint_source,
            &output_disjoint_layout,
            options.output_access,
            16,
        )
        .unwrap()
    };
    let kernel = KernelDescriptorV1::new(
        KernelId::from_bytes([0x11; 32]),
        name(options.entry),
        name(options.entry),
        name(options.descriptor),
        options.source_evidence,
        options.executable_ir_evidence,
        options.capabilities.clone(),
        KernelAbiLayoutV1::new(options.explicit_bytes, options.total_bytes, 8).unwrap(),
        LaunchConstraintsV1::new(
            1,
            BlockSizeV1::Exact(DimensionsV1::new(options.workgroup, 1, 1).unwrap()),
            DimensionsV1::new(
                options.max_grid[0],
                options.max_grid[1],
                options.max_grid[2],
            )
            .unwrap(),
            options.max_flat,
            options.static_lds,
            options.dynamic_lds,
        )
        .unwrap(),
        vec![
            LogicalArgumentV1::shared_slice(
                0,
                name(options.input_name),
                &input_source,
                &input_layout,
                0,
            )
            .unwrap(),
            output,
        ],
    )
    .unwrap();

    let (output_source, output_layout) = if options.output_is_shared {
        (output_shared_source, output_shared_layout)
    } else {
        (output_disjoint_source, output_disjoint_layout)
    };
    let mut source_records = vec![input_source];
    let mut layout_records = vec![input_layout];
    if !source_records
        .iter()
        .any(|record| record.identity() == output_source.identity())
    {
        source_records.push(output_source);
        layout_records.push(output_layout);
    }

    DeviceDescriptorTableV1::new(
        CanonicalCodeObjectDigest::from_bytes([0; 32]),
        options.code_object_version,
        CompilerIdentityV1::new(text("rustc-codegen-fe2o3"), text("test"), [0x41; 20]),
        ProducerIdentityV1::new(text("rustc-codegen-fe2o3-worker-v2"), text("test")),
        DeviceTargetV1::parse(options.target).unwrap(),
        source_records,
        layout_records,
        vec![kernel],
    )
    .unwrap()
}

#[test]
fn fixed_row_profile_is_structurally_sealed_and_v1_round_trips() {
    let options = Options::exact();
    let encoded = encode_device_descriptor_table_v1(&table(&options)).unwrap();
    let decoded = decode_device_descriptor_table_v1(&encoded).unwrap();
    let reencoded = encode_device_descriptor_table_v1(&decoded).unwrap();
    let admitted =
        admit_row_softmax_v1_structural_descriptor_v1(&decoded, expectation(&options)).unwrap();

    assert_eq!(DEVICE_DESCRIPTOR_VERSION, 1);
    assert_eq!(reencoded, encoded);
    assert_eq!(
        admitted.declared_row_elements(),
        ROW_SOFTMAX_V1_ROW_ELEMENTS
    );
    assert_eq!(admitted.workgroup_size(), [64, 1, 1]);
    assert_eq!(admitted.max_grid_size(), [1, 1, 1]);
    assert_eq!(admitted.max_flat_workgroup_size(), 64);
    assert_eq!(admitted.explicit_kernarg_bytes(), 32);
    assert_eq!(admitted.implicit_kernarg_bytes(), 256);
    assert_eq!(admitted.total_kernarg_bytes(), 288);
    assert_eq!(
        decoded.kernels()[0].capabilities(),
        [CapabilityV1::Subgroup, CapabilityV1::AmdWave]
    );
    assert!(!admitted.authenticates_evidence_origin());
    assert!(!admitted.validates_runtime_slice_lengths());
    assert!(!admitted.validates_kernel_body());
    assert!(!admitted.proves_functional_softmax());
    assert!(!admitted.proves_exp_implementation());
    assert!(!admitted.proves_numerical_contract());
    assert!(!admitted.proves_race_freedom());
    assert!(!admitted.proves_verus_verification());
    assert!(!admitted.grants_publication_authority());
    assert!(!admitted.grants_load_authority());
    assert!(!admitted.grants_launch_authority());
}

#[test]
fn rejects_launch_kernarg_lds_target_and_symbol_substitutions() {
    let baseline = Options::exact();
    for (mutated, expected) in [
        (
            Options {
                workgroup: 256,
                max_flat: 256,
                ..baseline.clone()
            },
            RowSoftmaxV1StructuralDescriptorErrorV1::Launch("workgroup size"),
        ),
        (
            Options {
                max_grid: [2, 1, 1],
                ..baseline.clone()
            },
            RowSoftmaxV1StructuralDescriptorErrorV1::Launch("maximum grid size"),
        ),
        (
            Options {
                total_bytes: 320,
                ..baseline.clone()
            },
            RowSoftmaxV1StructuralDescriptorErrorV1::KernargLayout,
        ),
        (
            Options {
                static_lds: 256,
                ..baseline.clone()
            },
            RowSoftmaxV1StructuralDescriptorErrorV1::Launch("LDS"),
        ),
        (
            Options {
                target: "gfx942",
                ..baseline.clone()
            },
            RowSoftmaxV1StructuralDescriptorErrorV1::Target,
        ),
        (
            Options {
                descriptor: "row_softmax_alias.kd",
                ..baseline.clone()
            },
            RowSoftmaxV1StructuralDescriptorErrorV1::Symbol("descriptor symbol"),
        ),
        (
            Options {
                entry: "tiled_gemm_v1",
                descriptor: "tiled_gemm_v1.kd",
                total_bytes: 320,
                ..baseline.clone()
            },
            RowSoftmaxV1StructuralDescriptorErrorV1::Symbol("logical name"),
        ),
    ] {
        assert_eq!(
            admit_row_softmax_v1_structural_descriptor_v1(&table(&mutated), expectation(&mutated),),
            Err(expected)
        );
    }
}

#[test]
fn rejects_capability_type_ownership_access_and_evidence_drift() {
    let baseline = Options::exact();
    for (mutated, expected) in [
        (
            Options {
                capabilities: vec![
                    CapabilityV1::Subgroup,
                    CapabilityV1::Shuffle,
                    CapabilityV1::AmdWave,
                ],
                ..baseline.clone()
            },
            RowSoftmaxV1StructuralDescriptorErrorV1::CapabilityProvenance,
        ),
        (
            Options {
                input_scalar: ScalarTypeV1::U16,
                ..baseline.clone()
            },
            RowSoftmaxV1StructuralDescriptorErrorV1::Argument {
                index: 0,
                field: "type provenance",
            },
        ),
        (
            Options {
                output_is_shared: true,
                ..baseline.clone()
            },
            RowSoftmaxV1StructuralDescriptorErrorV1::Argument {
                index: 1,
                field: "slice semantics",
            },
        ),
        (
            Options {
                output_access: AccessMode::WriteOnly,
                ..baseline.clone()
            },
            RowSoftmaxV1StructuralDescriptorErrorV1::Argument {
                index: 1,
                field: "slice semantics",
            },
        ),
        (
            Options {
                input_name: "scores",
                ..baseline.clone()
            },
            RowSoftmaxV1StructuralDescriptorErrorV1::Argument {
                index: 0,
                field: "name",
            },
        ),
    ] {
        assert_eq!(
            admit_row_softmax_v1_structural_descriptor_v1(&table(&mutated), expectation(&mutated),),
            Err(expected)
        );
    }

    let descriptor_options = Options {
        executable_ir_evidence: evidence(0x71, 0x72),
        ..baseline.clone()
    };
    assert_eq!(
        admit_row_softmax_v1_structural_descriptor_v1(
            &table(&descriptor_options),
            expectation(&baseline),
        ),
        Err(RowSoftmaxV1StructuralDescriptorErrorV1::BuildEvidence(
            "executable IR evidence"
        ))
    );
}

#[test]
fn invalid_provenance_and_non_cov6_fail_closed() {
    let baseline = Options::exact();
    assert!(matches!(
        RowSoftmaxV1StructuralDescriptorExpectationV1::new(
            KernelId::from_bytes([0; 32]),
            baseline.source_evidence,
            baseline.executable_ir_evidence,
        ),
        Err(RowSoftmaxV1StructuralDescriptorErrorV1::InvalidExpectedProvenance(_))
    ));
    let cov5 = Options {
        code_object_version: CodeObjectVersion::V5,
        ..baseline
    };
    assert_eq!(
        admit_row_softmax_v1_structural_descriptor_v1(&table(&cov5), expectation(&cov5)),
        Err(RowSoftmaxV1StructuralDescriptorErrorV1::CodeObjectVersion)
    );
}

fn name(value: &str) -> ValidName {
    ValidName::new(value).unwrap()
}

fn text(value: &str) -> Text {
    Text::new(value).unwrap()
}

fn evidence(identity: u8, digest: u8) -> BuildEvidenceV1 {
    BuildEvidenceV1::new(
        EvidenceIdentity::from_opaque_bytes([identity; 32]),
        EvidenceDigest::from_sha256_bytes([digest; 32]),
    )
}
