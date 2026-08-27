use fe2o3_kernel_descriptor::{
    AccessMode, BlockSizeV1, BuildEvidenceV1, CanonicalCodeObjectDigest, CapabilityV1,
    CodeObjectVersion, CompilerIdentityV1, DeviceDescriptorTableV1, DeviceLayoutDescriptorV1,
    DeviceLayoutRecordV1, DeviceTargetV1, DimensionsV1, EvidenceDigest, EvidenceIdentity,
    KernelAbiLayoutV1, KernelDescriptorV1, KernelId, LaunchConstraintsV1, LogicalArgumentV1,
    ProducerIdentityV1, RUSTC_CODEGEN_FE2O3_PRODUCTION_V3_PRODUCER_NAME_V1, ScalarTypeV1,
    SourceTypeDescriptorV1, SourceTypeRecordV1,
    TILED_GEMM_FRAGMENT_FRONTEND_PROBE_V1_EXPLICIT_KERNARG_BYTES,
    TILED_GEMM_FRAGMENT_FRONTEND_PROBE_V1_TOTAL_KERNARG_BYTES, TILED_GEMM_V1_DESCRIPTOR_SYMBOL,
    Text, TiledGemmV1StructuralDescriptorErrorV1, TiledGemmV1StructuralDescriptorExpectationV1,
    ValidName, admit_tiled_gemm_v1_structural_descriptor_v1, decode_device_descriptor_table_v1,
    encode_device_descriptor_table_v1,
};

#[derive(Clone)]
struct Options {
    target: &'static str,
    code_object_version: CodeObjectVersion,
    entry: &'static str,
    descriptor: &'static str,
    workgroup: u32,
    max_flat: u32,
    explicit_bytes: u32,
    total_bytes: u32,
    static_lds: u32,
    dynamic_lds: u32,
    capabilities: Vec<CapabilityV1>,
    first_name: &'static str,
    first_scalar: ScalarTypeV1,
    output_access: AccessMode,
    source_evidence: BuildEvidenceV1,
    executable_ir_evidence: BuildEvidenceV1,
}

impl Options {
    fn exact() -> Self {
        Self {
            target: "gfx942:xnack-",
            code_object_version: CodeObjectVersion::V6,
            entry: "tiled_gemm_v1",
            descriptor: TILED_GEMM_V1_DESCRIPTOR_SYMBOL,
            workgroup: 64,
            max_flat: 64,
            explicit_bytes: 64,
            total_bytes: 320,
            static_lds: 0,
            dynamic_lds: 0,
            capabilities: vec![
                CapabilityV1::Subgroup,
                CapabilityV1::MatrixMultiply,
                CapabilityV1::AmdWave,
                CapabilityV1::AmdMfma,
            ],
            first_name: "a",
            first_scalar: ScalarTypeV1::U16,
            output_access: AccessMode::ReadWrite,
            source_evidence: evidence(0x21, 0x22),
            executable_ir_evidence: evidence(0x31, 0x32),
        }
    }
}

fn expectation(options: &Options) -> TiledGemmV1StructuralDescriptorExpectationV1 {
    TiledGemmV1StructuralDescriptorExpectationV1::new(
        KernelId::from_bytes([0x11; 32]),
        options.source_evidence,
        options.executable_ir_evidence,
    )
    .unwrap()
}

fn table(options: &Options) -> DeviceDescriptorTableV1 {
    let u16_source =
        SourceTypeRecordV1::new(SourceTypeDescriptorV1::shared_slice(ScalarTypeV1::U16));
    let u16_layout =
        DeviceLayoutRecordV1::new(DeviceLayoutDescriptorV1::shared_slice(ScalarTypeV1::U16));
    let f32_source =
        SourceTypeRecordV1::new(SourceTypeDescriptorV1::shared_slice(ScalarTypeV1::F32));
    let f32_layout =
        DeviceLayoutRecordV1::new(DeviceLayoutDescriptorV1::shared_slice(ScalarTypeV1::F32));
    let output_source =
        SourceTypeRecordV1::new(SourceTypeDescriptorV1::disjoint_slice(ScalarTypeV1::F32));
    let output_layout =
        DeviceLayoutRecordV1::new(DeviceLayoutDescriptorV1::disjoint_slice(ScalarTypeV1::F32));
    let alternate_first = (options.first_scalar != ScalarTypeV1::U16).then(|| {
        (
            SourceTypeRecordV1::new(SourceTypeDescriptorV1::shared_slice(options.first_scalar)),
            DeviceLayoutRecordV1::new(DeviceLayoutDescriptorV1::shared_slice(options.first_scalar)),
        )
    });
    let (first_source, first_layout) = alternate_first
        .as_ref()
        .map_or((&u16_source, &u16_layout), |(source, layout)| {
            (source, layout)
        });

    let arguments = vec![
        LogicalArgumentV1::shared_slice(0, name(options.first_name), first_source, first_layout, 0)
            .unwrap(),
        LogicalArgumentV1::shared_slice(1, name("b"), &u16_source, &u16_layout, 16).unwrap(),
        LogicalArgumentV1::shared_slice(2, name("c"), &f32_source, &f32_layout, 32).unwrap(),
        LogicalArgumentV1::disjoint_slice(
            3,
            name("d"),
            &output_source,
            &output_layout,
            options.output_access,
            48,
        )
        .unwrap(),
    ];

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
            DimensionsV1::new(u32::MAX, 1, 1).unwrap(),
            options.max_flat,
            options.static_lds,
            options.dynamic_lds,
        )
        .unwrap(),
        arguments,
    )
    .unwrap();

    let mut sources = vec![u16_source, f32_source, output_source];
    let mut layouts = vec![u16_layout, f32_layout, output_layout];
    if let Some((source, layout)) = alternate_first {
        sources.push(source);
        layouts.push(layout);
    }
    descriptor_table(options, sources, layouts, kernel)
}

fn fragment_probe_table(options: &Options) -> DeviceDescriptorTableV1 {
    let u16_source = SourceTypeRecordV1::new(SourceTypeDescriptorV1::scalar(ScalarTypeV1::U16));
    let u16_layout = DeviceLayoutRecordV1::new(DeviceLayoutDescriptorV1::scalar(ScalarTypeV1::U16));
    let f32_source = SourceTypeRecordV1::new(SourceTypeDescriptorV1::scalar(ScalarTypeV1::F32));
    let f32_layout = DeviceLayoutRecordV1::new(DeviceLayoutDescriptorV1::scalar(ScalarTypeV1::F32));
    let mut arguments = Vec::new();
    for index in 0..8_u16 {
        arguments.push(
            LogicalArgumentV1::scalar(
                index,
                name(&format!("arg{index}")),
                &u16_source,
                &u16_layout,
                u32::from(index) * 2,
            )
            .unwrap(),
        );
    }
    for index in 8..12_u16 {
        arguments.push(
            LogicalArgumentV1::scalar(
                index,
                name(&format!("arg{index}")),
                &f32_source,
                &f32_layout,
                16 + (u32::from(index) - 8) * 4,
            )
            .unwrap(),
        );
    }
    let kernel = KernelDescriptorV1::new(
        KernelId::from_bytes([0x11; 32]),
        name(options.entry),
        name(options.entry),
        name(options.descriptor),
        options.source_evidence,
        options.executable_ir_evidence,
        options.capabilities.clone(),
        KernelAbiLayoutV1::new(
            TILED_GEMM_FRAGMENT_FRONTEND_PROBE_V1_EXPLICIT_KERNARG_BYTES,
            TILED_GEMM_FRAGMENT_FRONTEND_PROBE_V1_TOTAL_KERNARG_BYTES,
            8,
        )
        .unwrap(),
        LaunchConstraintsV1::new(
            1,
            BlockSizeV1::Exact(DimensionsV1::new(64, 1, 1).unwrap()),
            DimensionsV1::new(u32::MAX, 1, 1).unwrap(),
            64,
            0,
            0,
        )
        .unwrap(),
        arguments,
    )
    .unwrap();
    descriptor_table(
        options,
        vec![u16_source, f32_source],
        vec![u16_layout, f32_layout],
        kernel,
    )
}

fn descriptor_table(
    options: &Options,
    sources: Vec<SourceTypeRecordV1>,
    layouts: Vec<DeviceLayoutRecordV1>,
    kernel: KernelDescriptorV1,
) -> DeviceDescriptorTableV1 {
    DeviceDescriptorTableV1::new(
        CanonicalCodeObjectDigest::from_bytes([0; 32]),
        options.code_object_version,
        CompilerIdentityV1::new(text("rustc-codegen-fe2o3"), text("test"), [0x41; 20]),
        ProducerIdentityV1::new(
            text(RUSTC_CODEGEN_FE2O3_PRODUCTION_V3_PRODUCER_NAME_V1),
            text("test"),
        ),
        DeviceTargetV1::parse(options.target).unwrap(),
        sources,
        layouts,
        vec![kernel],
    )
    .unwrap()
}

#[test]
fn structural_direct_global_profile_is_sealed_and_round_trips() {
    let options = Options::exact();
    let encoded = encode_device_descriptor_table_v1(&table(&options)).unwrap();
    let decoded = decode_device_descriptor_table_v1(&encoded).unwrap();
    let admitted =
        admit_tiled_gemm_v1_structural_descriptor_v1(&decoded, expectation(&options)).unwrap();

    assert_eq!(admitted.workgroup_size(), [64, 1, 1]);
    assert_eq!(admitted.max_flat_workgroup_size(), 64);
    assert_eq!(admitted.explicit_kernarg_bytes(), 64);
    assert_eq!(admitted.implicit_kernarg_bytes(), 256);
    assert_eq!(admitted.total_kernarg_bytes(), 320);
    assert_eq!(admitted.source_evidence(), options.source_evidence);
    assert_eq!(
        admitted.executable_ir_evidence(),
        options.executable_ir_evidence
    );
    assert_eq!(
        decoded.kernels()[0].capabilities(),
        [
            CapabilityV1::Subgroup,
            CapabilityV1::MatrixMultiply,
            CapabilityV1::AmdWave,
            CapabilityV1::AmdMfma,
        ]
    );
    assert!(!admitted.authenticates_evidence_origin());
    assert!(!admitted.validates_kernel_body());
    assert!(!admitted.proves_bf16_isa_semantics());
    assert!(!admitted.proves_mfma_isa_semantics());
    assert!(!admitted.grants_launch_authority());
}

#[test]
fn fragment_frontend_probe_is_preserved_but_cannot_substitute_for_structural_abi() {
    let options = Options::exact();
    let probe = fragment_probe_table(&options);
    assert_eq!(probe.kernels()[0].abi_layout().explicit_argument_size(), 32);
    assert_eq!(probe.kernels()[0].abi_layout().kernarg_segment_size(), 288);
    assert_eq!(probe.kernels()[0].arguments().len(), 12);
    assert_eq!(
        admit_tiled_gemm_v1_structural_descriptor_v1(&probe, expectation(&options)),
        Err(TiledGemmV1StructuralDescriptorErrorV1::KernargLayout)
    );
}

#[test]
fn rejects_wg256_kernarg_lds_target_and_symbol_substitutions() {
    let baseline = Options::exact();
    for (mutated, expected) in [
        (
            Options {
                workgroup: 256,
                max_flat: 256,
                ..baseline.clone()
            },
            TiledGemmV1StructuralDescriptorErrorV1::Launch("workgroup size"),
        ),
        (
            Options {
                total_bytes: 384,
                ..baseline.clone()
            },
            TiledGemmV1StructuralDescriptorErrorV1::KernargLayout,
        ),
        (
            Options {
                static_lds: 1024,
                ..baseline.clone()
            },
            TiledGemmV1StructuralDescriptorErrorV1::Launch("LDS"),
        ),
        (
            Options {
                target: "gfx942",
                ..baseline.clone()
            },
            TiledGemmV1StructuralDescriptorErrorV1::Target,
        ),
        (
            Options {
                descriptor: "tiled_gemm_v1_alias.kd",
                ..baseline.clone()
            },
            TiledGemmV1StructuralDescriptorErrorV1::Symbol("descriptor symbol"),
        ),
    ] {
        assert_eq!(
            admit_tiled_gemm_v1_structural_descriptor_v1(&table(&mutated), expectation(&mutated),),
            Err(expected)
        );
    }
}

#[test]
fn rejects_capability_omission_substitution_and_evidence_drift() {
    let baseline = Options::exact();
    for capabilities in [
        vec![
            CapabilityV1::Subgroup,
            CapabilityV1::MatrixMultiply,
            CapabilityV1::AmdWave,
        ],
        vec![
            CapabilityV1::Subgroup,
            CapabilityV1::MatrixMultiply,
            CapabilityV1::AmdWave,
            CapabilityV1::AmdWmma,
        ],
    ] {
        let mutated = Options {
            capabilities,
            ..baseline.clone()
        };
        assert_eq!(
            admit_tiled_gemm_v1_structural_descriptor_v1(&table(&mutated), expectation(&mutated),),
            Err(TiledGemmV1StructuralDescriptorErrorV1::CapabilityProvenance)
        );
    }

    let descriptor_options = Options {
        executable_ir_evidence: evidence(0x71, 0x72),
        ..baseline.clone()
    };
    assert_eq!(
        admit_tiled_gemm_v1_structural_descriptor_v1(
            &table(&descriptor_options),
            expectation(&baseline),
        ),
        Err(TiledGemmV1StructuralDescriptorErrorV1::BuildEvidence(
            "executable IR evidence"
        ))
    );
}

#[test]
fn rejects_slice_type_name_access_and_invalid_expected_provenance() {
    let baseline = Options::exact();
    for (mutated, expected) in [
        (
            Options {
                first_scalar: ScalarTypeV1::F16,
                ..baseline.clone()
            },
            TiledGemmV1StructuralDescriptorErrorV1::Argument {
                index: 0,
                field: "type provenance",
            },
        ),
        (
            Options {
                first_name: "lhs",
                ..baseline.clone()
            },
            TiledGemmV1StructuralDescriptorErrorV1::Argument {
                index: 0,
                field: "name",
            },
        ),
        (
            Options {
                output_access: AccessMode::WriteOnly,
                ..baseline.clone()
            },
            TiledGemmV1StructuralDescriptorErrorV1::Argument {
                index: 3,
                field: "slice semantics",
            },
        ),
    ] {
        assert_eq!(
            admit_tiled_gemm_v1_structural_descriptor_v1(&table(&mutated), expectation(&mutated),),
            Err(expected)
        );
    }
    assert!(matches!(
        TiledGemmV1StructuralDescriptorExpectationV1::new(
            KernelId::from_bytes([0; 32]),
            baseline.source_evidence,
            baseline.executable_ir_evidence,
        ),
        Err(TiledGemmV1StructuralDescriptorErrorV1::InvalidExpectedProvenance(_))
    ));
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
