use fe2o3_llvm_handoff::{
    AddressSpaceV1, CallingConventionV1, CodeModelV1, CodeObjectVersionV1, DecodeHandoffErrorV1,
    DeviceLibraryInputV1, DeviceLibraryKindV1, FunctionAttributeV1, GFX942_AMDHSA_DATA_LAYOUT_V1,
    GFX942_AMDHSA_TARGET_TRIPLE_V1, Gfx942HandoffInputV1, Gfx942HandoffV1, Gfx942TargetPolicyV1,
    HandoffDiagnosticV1, HandoffLimitV1, IdentityV1, KernelEntryV1, KernelParameterV1,
    KernelReturnTypeV1, KernelValueTypeV1, MAX_CANONICAL_HANDOFF_BYTES_V1,
    MAX_DEVICE_LIBRARY_BYTES_V1, MAX_KERNEL_PARAMETERS_V1, MAX_SOURCE_PATH_BYTES_V1,
    MAX_SYMBOL_BYTES_V1, ModuleFlagV1, ModuleMetadataV1, NamedMetadataV1, ObligationKindV1,
    ObligationV1, OptimizationLevelV1, OriginKindV1, OriginV1, ParameterAttributeV1,
    RelocationModelV1, ScalarTypeV1, SourceSpanV1, StageIdentitiesV1, TargetFeatureV1,
    WavesPerEuV1, WireSectionV1, WorkgroupSizeRangeV1,
};

fn identity(byte: u8) -> IdentityV1 {
    IdentityV1::new([byte; 32]).unwrap()
}

fn kernel_attributes(permuted: bool) -> Vec<FunctionAttributeV1> {
    let mut attributes =
        FunctionAttributeV1::gfx942_kernel_defaults(WorkgroupSizeRangeV1::new(64, 256).unwrap());
    attributes.push(FunctionAttributeV1::WavesPerEu(
        WavesPerEuV1::new(2, 8).unwrap(),
    ));
    if permuted {
        attributes.reverse();
    }
    attributes
}

fn alpha_kernel(origin: &OriginV1, swap_parameters: bool, permuted: bool) -> KernelEntryV1 {
    let mut pointer_attributes = vec![
        ParameterAttributeV1::NoAlias,
        ParameterAttributeV1::NoCapture,
        ParameterAttributeV1::NonNull,
        ParameterAttributeV1::Align(4),
        ParameterAttributeV1::Dereferenceable(4_096),
    ];
    if permuted {
        pointer_attributes.reverse();
    }
    let output = KernelParameterV1::new(
        "output",
        KernelValueTypeV1::Pointer {
            pointee: ScalarTypeV1::F32,
            address_space: AddressSpaceV1::Global,
        },
        pointer_attributes,
    )
    .unwrap();
    let length = KernelParameterV1::new(
        "length",
        KernelValueTypeV1::Scalar(ScalarTypeV1::I64),
        vec![],
    )
    .unwrap();
    let parameters = if swap_parameters {
        vec![length, output]
    } else {
        vec![output, length]
    };
    KernelEntryV1::new(
        "alpha_kernel",
        parameters,
        kernel_attributes(permuted),
        origin.identity(),
    )
    .unwrap()
}

fn zeta_kernel(origin: &OriginV1, permuted: bool) -> KernelEntryV1 {
    let mut attributes = vec![
        ParameterAttributeV1::NoAlias,
        ParameterAttributeV1::NoCapture,
        ParameterAttributeV1::NonNull,
        ParameterAttributeV1::ReadOnly,
        ParameterAttributeV1::Align(2),
        ParameterAttributeV1::Dereferenceable(2_048),
    ];
    if permuted {
        attributes.reverse();
    }
    let input = KernelParameterV1::new(
        "input",
        KernelValueTypeV1::Pointer {
            pointee: ScalarTypeV1::Bf16,
            address_space: AddressSpaceV1::Constant,
        },
        attributes,
    )
    .unwrap();
    KernelEntryV1::new(
        "zeta_kernel",
        vec![input],
        kernel_attributes(permuted),
        origin.identity(),
    )
    .unwrap()
}

fn fixture_input(permuted: bool) -> Gfx942HandoffInputV1 {
    let source = OriginV1::new(
        OriginKindV1::RustSource,
        identity(0x11),
        Some(SourceSpanV1::new("crates/example/src/lib.rs", 10, 1, 24, 2).unwrap()),
    );
    let amdgcn = OriginV1::new(OriginKindV1::AmdgcnIr, identity(0x22), None);

    let mut kernels = vec![
        alpha_kernel(&source, false, permuted),
        zeta_kernel(&amdgcn, permuted),
    ];
    let mut flags = vec![
        ModuleFlagV1::CodeObjectVersion6,
        ModuleFlagV1::PicLevel2,
        ModuleFlagV1::WcharSize4,
    ];
    let mut named = vec![
        NamedMetadataV1::OpenClVersion2_0,
        NamedMetadataV1::OpenClSpirVersion2_0,
        NamedMetadataV1::ProducerIdentity(identity(0x33)),
    ];
    let mut libraries = vec![
        DeviceLibraryInputV1::new(DeviceLibraryKindV1::Ocml, [0x44; 32], 48_000).unwrap(),
        DeviceLibraryInputV1::new(DeviceLibraryKindV1::Ockl, [0x55; 32], 32_000).unwrap(),
        DeviceLibraryInputV1::new(DeviceLibraryKindV1::OclcIsaVersion942, [0x66; 32], 512).unwrap(),
    ];
    let mut origins = vec![source.clone(), amdgcn.clone()];
    let mut obligations = vec![
        ObligationV1::new(
            ObligationKindV1::PreserveKernelAbi,
            identity(0x77),
            source.identity(),
        ),
        ObligationV1::new(
            ObligationKindV1::PreserveTargetFeatures,
            identity(0x88),
            amdgcn.identity(),
        ),
        ObligationV1::new(
            ObligationKindV1::AuthenticateDeviceLibraries,
            identity(0x99),
            amdgcn.identity(),
        ),
        ObligationV1::new(
            ObligationKindV1::MaintainOriginCoverage,
            identity(0xaa),
            source.identity(),
        ),
    ];
    if permuted {
        kernels.reverse();
        flags.reverse();
        named.reverse();
        libraries.reverse();
        origins.reverse();
        obligations.reverse();
    }

    Gfx942HandoffInputV1 {
        stage_identities: StageIdentitiesV1::new([1; 32], [2; 32], [3; 32]).unwrap(),
        target: Gfx942TargetPolicyV1::canonical(),
        kernels,
        module: ModuleMetadataV1::new(flags, named, libraries).unwrap(),
        origins,
        obligations,
    }
}

fn fixture(permuted: bool) -> Gfx942HandoffV1 {
    Gfx942HandoffV1::new(fixture_input(permuted)).unwrap()
}

const ABI_ATTRIBUTES_V1: [FunctionAttributeV1; 6] = [
    FunctionAttributeV1::NoCompletionAction,
    FunctionAttributeV1::NoDefaultQueue,
    FunctionAttributeV1::NoHeapPointer,
    FunctionAttributeV1::NoHostcallPointer,
    FunctionAttributeV1::NoMultigridSyncArgument,
    FunctionAttributeV1::NoQueuePointer,
];

const ABI_ATTRIBUTE_NAMES: [&str; 6] = [
    "amdgpu-no-completion-action",
    "amdgpu-no-default-queue",
    "amdgpu-no-heap-ptr",
    "amdgpu-no-hostcall-ptr",
    "amdgpu-no-multigrid-sync-arg",
    "amdgpu-no-queue-ptr",
];

#[test]
fn abi_function_attributes_are_opt_in_named_and_round_trip() {
    let defaults =
        FunctionAttributeV1::gfx942_kernel_defaults(WorkgroupSizeRangeV1::new(64, 256).unwrap());
    assert_eq!(defaults.len(), 9);
    assert!(
        ABI_ATTRIBUTES_V1
            .iter()
            .all(|attribute| !defaults.contains(attribute))
    );
    assert_eq!(
        ABI_ATTRIBUTES_V1.map(FunctionAttributeV1::canonical_name),
        ABI_ATTRIBUTE_NAMES
    );

    let mut input = fixture_input(false);
    let original = input.kernels[0].clone();
    let mut attributes = original.function_attributes().to_vec();
    attributes.extend(ABI_ATTRIBUTES_V1);
    input.kernels[0] = KernelEntryV1::new(
        original.symbol(),
        original.parameters().to_vec(),
        attributes,
        original.origin(),
    )
    .unwrap();
    let handoff = Gfx942HandoffV1::new(input).unwrap();
    let encoded = handoff.encode_canonical();
    assert_eq!(
        Gfx942HandoffV1::decode_canonical(encoded.as_bytes()).unwrap(),
        handoff
    );
}

#[test]
fn positive_gfx942_handoff_round_trips_with_exact_policy() {
    let handoff = fixture(false);
    let target = handoff.target();
    assert_eq!(target.target_triple(), GFX942_AMDHSA_TARGET_TRIPLE_V1);
    assert_eq!(target.data_layout(), GFX942_AMDHSA_DATA_LAYOUT_V1);
    assert_eq!(target.cpu(), "gfx942");
    assert_eq!(target.code_object_version(), CodeObjectVersionV1::V6);
    assert_eq!(target.optimization_level(), OptimizationLevelV1::O2);
    assert_eq!(target.relocation_model(), RelocationModelV1::Pic);
    assert_eq!(target.code_model(), CodeModelV1::Small);
    assert_eq!(
        target
            .features()
            .iter()
            .map(|feature| (feature.feature(), feature.enabled()))
            .collect::<Vec<_>>(),
        vec![
            (TargetFeatureV1::WavefrontSize32, false),
            (TargetFeatureV1::WavefrontSize64, true),
            (TargetFeatureV1::Xnack, false),
        ]
    );

    assert_eq!(handoff.kernels().len(), 2);
    assert_eq!(handoff.kernels()[0].symbol(), "alpha_kernel");
    assert_eq!(
        handoff.kernels()[0].calling_convention(),
        CallingConventionV1::AmdGpuKernel
    );
    assert_eq!(
        handoff.kernels()[0].calling_convention().llvm_name(),
        "amdgpu_kernel"
    );
    assert_eq!(handoff.kernels()[0].return_type(), KernelReturnTypeV1::Void);
    assert_eq!(handoff.module().device_libraries().len(), 3);
    assert_eq!(handoff.origins().len(), 2);
    assert_eq!(handoff.obligations().len(), 4);

    let encoded = handoff.encode_canonical();
    assert!(encoded.as_bytes().starts_with(b"F2LLVMH1"));
    assert!(encoded.len() < MAX_CANONICAL_HANDOFF_BYTES_V1);
    let decoded = Gfx942HandoffV1::decode_canonical(encoded.as_bytes()).unwrap();
    assert_eq!(decoded, handoff);
    assert_eq!(decoded.encode_canonical(), encoded);
    assert_eq!(
        handoff.identity().to_string(),
        "b54703016850889957726a998f90746c61b5d8b37c778e7ce5f428af2ecaf133"
    );
}

#[test]
fn unordered_inputs_have_identical_encoding_and_identity() {
    let ordered = fixture(false);
    let permuted = fixture(true);
    assert_eq!(ordered, permuted);
    assert_eq!(ordered.encode_canonical(), permuted.encode_canonical());
    assert_eq!(ordered.identity(), permuted.identity());

    let repeated = fixture(false);
    assert_eq!(ordered.encode_canonical(), repeated.encode_canonical());
    assert_eq!(ordered.identity(), repeated.identity());
}

#[test]
fn semantic_parameter_order_changes_the_handoff_identity() {
    let baseline = fixture(false);
    let mut changed = fixture_input(false);
    let source = changed
        .origins
        .iter()
        .find(|origin| origin.kind() == OriginKindV1::RustSource)
        .unwrap()
        .clone();
    changed.kernels[0] = alpha_kernel(&source, true, false);
    let changed = Gfx942HandoffV1::new(changed).unwrap();
    assert_ne!(baseline.encode_canonical(), changed.encode_canonical());
    assert_ne!(baseline.identity(), changed.identity());
}

#[test]
fn exact_text_and_numeric_boundaries_are_enforced() {
    let origin = OriginV1::new(OriginKindV1::KernelIr, identity(1), None);
    let maximum_symbol = "a".repeat(MAX_SYMBOL_BYTES_V1);
    KernelEntryV1::new(
        &maximum_symbol,
        vec![],
        kernel_attributes(false),
        origin.identity(),
    )
    .unwrap();
    assert!(matches!(
        KernelEntryV1::new(
            &"a".repeat(MAX_SYMBOL_BYTES_V1 + 1),
            vec![],
            kernel_attributes(false),
            origin.identity(),
        ),
        Err(HandoffDiagnosticV1::LimitExceeded {
            limit: HandoffLimitV1::SymbolBytes,
            ..
        })
    ));

    SourceSpanV1::new(&"a".repeat(MAX_SOURCE_PATH_BYTES_V1), 1, 1, 1, 1).unwrap();
    assert!(matches!(
        SourceSpanV1::new(&"a".repeat(MAX_SOURCE_PATH_BYTES_V1 + 1), 1, 1, 1, 1,),
        Err(HandoffDiagnosticV1::LimitExceeded {
            limit: HandoffLimitV1::SourcePathBytes,
            ..
        })
    ));

    DeviceLibraryInputV1::new(
        DeviceLibraryKindV1::OpenCl,
        [1; 32],
        MAX_DEVICE_LIBRARY_BYTES_V1,
    )
    .unwrap();
    assert_eq!(
        DeviceLibraryInputV1::new(
            DeviceLibraryKindV1::OpenCl,
            [1; 32],
            MAX_DEVICE_LIBRARY_BYTES_V1 + 1,
        ),
        Err(HandoffDiagnosticV1::InvalidDeviceLibrarySize)
    );
    WorkgroupSizeRangeV1::new(1, 64).unwrap();
    WorkgroupSizeRangeV1::new(63, 1_024).unwrap();
    assert_eq!(
        WorkgroupSizeRangeV1::new(0, 64),
        Err(HandoffDiagnosticV1::InvalidWorkgroupSizeRange)
    );
    assert_eq!(
        WorkgroupSizeRangeV1::new(65, 64),
        Err(HandoffDiagnosticV1::InvalidWorkgroupSizeRange)
    );
    assert_eq!(
        WorkgroupSizeRangeV1::new(64, 1_025),
        Err(HandoffDiagnosticV1::InvalidWorkgroupSizeRange)
    );
    WavesPerEuV1::new(1, 10).unwrap();
    assert_eq!(
        WavesPerEuV1::new(1, 11),
        Err(HandoffDiagnosticV1::InvalidWavesPerEu)
    );
}

#[test]
fn parameter_count_boundary_is_exact() {
    let origin = OriginV1::new(OriginKindV1::KernelIr, identity(1), None);
    let parameters = (0..MAX_KERNEL_PARAMETERS_V1)
        .map(|index| {
            KernelParameterV1::new(
                &format!("p{index}"),
                KernelValueTypeV1::Scalar(ScalarTypeV1::I32),
                vec![],
            )
            .unwrap()
        })
        .collect::<Vec<_>>();
    KernelEntryV1::new(
        "boundary",
        parameters.clone(),
        kernel_attributes(false),
        origin.identity(),
    )
    .unwrap();
    let mut too_many = parameters;
    too_many.push(
        KernelParameterV1::new(
            "overflow",
            KernelValueTypeV1::Scalar(ScalarTypeV1::I32),
            vec![],
        )
        .unwrap(),
    );
    assert!(matches!(
        KernelEntryV1::new(
            "boundary",
            too_many,
            kernel_attributes(false),
            origin.identity(),
        ),
        Err(HandoffDiagnosticV1::LimitExceeded {
            limit: HandoffLimitV1::KernelParameters,
            ..
        })
    ));
}

#[test]
fn duplicate_conflicting_and_dangling_model_inputs_fail_closed() {
    let pointer = KernelValueTypeV1::Pointer {
        pointee: ScalarTypeV1::F32,
        address_space: AddressSpaceV1::Global,
    };
    assert!(matches!(
        KernelParameterV1::new(
            "p",
            pointer,
            vec![
                ParameterAttributeV1::Align(4),
                ParameterAttributeV1::Align(8),
            ],
        ),
        Err(HandoffDiagnosticV1::DuplicateParameterAttribute("align"))
    ));
    assert_eq!(
        KernelParameterV1::new(
            "p",
            pointer,
            vec![
                ParameterAttributeV1::ReadOnly,
                ParameterAttributeV1::WriteOnly,
            ],
        ),
        Err(HandoffDiagnosticV1::ConflictingParameterAttributes)
    );
    assert!(matches!(
        KernelParameterV1::new(
            "scalar",
            KernelValueTypeV1::Scalar(ScalarTypeV1::I32),
            vec![ParameterAttributeV1::NoAlias],
        ),
        Err(HandoffDiagnosticV1::AttributeRequiresPointer("noalias"))
    ));

    let origin = OriginV1::new(OriginKindV1::KernelIr, identity(1), None);
    let mut duplicate_attributes = kernel_attributes(false);
    duplicate_attributes.push(FunctionAttributeV1::NoUnwind);
    assert!(matches!(
        KernelEntryV1::new(
            "duplicate_attribute",
            vec![],
            duplicate_attributes,
            origin.identity(),
        ),
        Err(HandoffDiagnosticV1::DuplicateFunctionAttribute("nounwind"))
    ));
    assert!(matches!(
        KernelEntryV1::new(
            "missing_attribute",
            vec![],
            vec![FunctionAttributeV1::NoUnwind],
            origin.identity(),
        ),
        Err(HandoffDiagnosticV1::MissingFunctionAttribute(_))
    ));

    assert!(matches!(
        ModuleMetadataV1::new(
            vec![
                ModuleFlagV1::CodeObjectVersion6,
                ModuleFlagV1::PicLevel2,
                ModuleFlagV1::PicLevel2,
            ],
            vec![],
            vec![],
        ),
        Err(HandoffDiagnosticV1::DuplicateModuleFlag(_))
    ));
    assert!(matches!(
        ModuleMetadataV1::new(
            vec![ModuleFlagV1::CodeObjectVersion6, ModuleFlagV1::PicLevel2,],
            vec![],
            vec![
                DeviceLibraryInputV1::new(DeviceLibraryKindV1::Ocml, [1; 32], 1).unwrap(),
                DeviceLibraryInputV1::new(DeviceLibraryKindV1::Ocml, [2; 32], 2).unwrap(),
            ],
        ),
        Err(HandoffDiagnosticV1::DuplicateDeviceLibrary("ocml"))
    ));

    let mut duplicate_kernel = fixture_input(false);
    duplicate_kernel
        .kernels
        .push(duplicate_kernel.kernels[0].clone());
    assert!(matches!(
        Gfx942HandoffV1::new(duplicate_kernel),
        Err(HandoffDiagnosticV1::DuplicateKernel(_))
    ));
    let mut duplicate_origin = fixture_input(false);
    duplicate_origin
        .origins
        .push(duplicate_origin.origins[0].clone());
    assert_eq!(
        Gfx942HandoffV1::new(duplicate_origin),
        Err(HandoffDiagnosticV1::DuplicateOrigin)
    );
    let mut duplicate_obligation = fixture_input(false);
    duplicate_obligation
        .obligations
        .push(duplicate_obligation.obligations[0]);
    assert_eq!(
        Gfx942HandoffV1::new(duplicate_obligation),
        Err(HandoffDiagnosticV1::DuplicateObligation)
    );
    let mut dangling = fixture_input(false);
    dangling.origins.remove(0);
    assert_eq!(
        Gfx942HandoffV1::new(dangling),
        Err(HandoffDiagnosticV1::MissingOriginReference)
    );
}

#[test]
fn malformed_headers_lengths_and_noncanonical_order_fail_closed() {
    let canonical = fixture(false).encode_canonical();

    let mut bad_magic = canonical.as_bytes().to_vec();
    bad_magic[0] ^= 1;
    assert_eq!(
        Gfx942HandoffV1::decode_canonical(&bad_magic),
        Err(DecodeHandoffErrorV1::BadMagic)
    );

    let mut bad_version = canonical.as_bytes().to_vec();
    bad_version[8..10].copy_from_slice(&2_u16.to_le_bytes());
    assert_eq!(
        Gfx942HandoffV1::decode_canonical(&bad_version),
        Err(DecodeHandoffErrorV1::UnsupportedVersion(2))
    );

    let truncated = &canonical.as_bytes()[..canonical.len() - 1];
    assert!(matches!(
        Gfx942HandoffV1::decode_canonical(truncated),
        Err(DecodeHandoffErrorV1::LengthMismatch { .. })
    ));
    assert!(matches!(
        Gfx942HandoffV1::decode_canonical(&vec![0; MAX_CANONICAL_HANDOFF_BYTES_V1 + 1]),
        Err(DecodeHandoffErrorV1::TooLong { .. })
    ));

    let mut reordered_features = canonical.as_bytes().to_vec();
    reordered_features.swap(20, 22);
    reordered_features.swap(21, 23);
    assert_eq!(
        Gfx942HandoffV1::decode_canonical(&reordered_features),
        Err(DecodeHandoffErrorV1::NonCanonical)
    );
}

#[test]
fn every_unknown_semantic_wire_family_fails_closed() {
    let canonical = fixture(false).encode_canonical();
    let offsets = wire_offsets(canonical.as_bytes());
    let cases = [
        (offsets.target_triple, WireSectionV1::TargetTriple),
        (offsets.data_layout, WireSectionV1::DataLayout),
        (offsets.cpu, WireSectionV1::Cpu),
        (offsets.target_feature, WireSectionV1::TargetFeature),
        (offsets.code_object, WireSectionV1::CodeObjectPolicy),
        (offsets.calling_convention, WireSectionV1::CallingConvention),
        (offsets.value_type, WireSectionV1::ValueType),
        (offsets.scalar_type, WireSectionV1::ScalarType),
        (offsets.address_space, WireSectionV1::AddressSpace),
        (
            offsets.parameter_attribute,
            WireSectionV1::ParameterAttribute,
        ),
        (offsets.function_attribute, WireSectionV1::FunctionAttribute),
        (offsets.module_flag, WireSectionV1::ModuleFlag),
        (offsets.named_metadata, WireSectionV1::NamedMetadata),
        (offsets.device_library, WireSectionV1::DeviceLibrary),
        (offsets.origin, WireSectionV1::Origin),
        (offsets.obligation, WireSectionV1::Obligation),
    ];

    for (offset, section) in cases {
        let mut hostile = canonical.as_bytes().to_vec();
        hostile[offset] = 0xff;
        assert_eq!(
            Gfx942HandoffV1::decode_canonical(&hostile),
            Err(DecodeHandoffErrorV1::UnknownTag { section, tag: 0xff }),
            "mutation at offset {offset} did not fail in {section:?}"
        );
    }
}

#[test]
fn authenticated_origin_and_obligation_payload_mutations_fail_closed() {
    let canonical = fixture(false).encode_canonical();
    let offsets = wire_offsets(canonical.as_bytes());
    for offset in [offsets.origin_identity, offsets.obligation_identity] {
        let mut hostile = canonical.as_bytes().to_vec();
        hostile[offset] ^= 1;
        assert_eq!(
            Gfx942HandoffV1::decode_canonical(&hostile),
            Err(DecodeHandoffErrorV1::NonCanonical)
        );
    }
}

#[derive(Debug)]
struct WireOffsets {
    target_triple: usize,
    data_layout: usize,
    cpu: usize,
    target_feature: usize,
    code_object: usize,
    calling_convention: usize,
    value_type: usize,
    scalar_type: usize,
    address_space: usize,
    parameter_attribute: usize,
    function_attribute: usize,
    module_flag: usize,
    named_metadata: usize,
    device_library: usize,
    origin_identity: usize,
    origin: usize,
    obligation_identity: usize,
    obligation: usize,
}

fn wire_offsets(bytes: &[u8]) -> WireOffsets {
    let mut cursor = Cursor::new(bytes, 16);
    let target_triple = cursor.take(1);
    let data_layout = cursor.take(1);
    let cpu = cursor.take(1);
    let feature_count = cursor.u8() as usize;
    let target_feature = cursor.offset;
    cursor.take(feature_count * 2);
    let code_object = cursor.take(1);
    cursor.take(3);
    cursor.take(32 * 3);

    let kernel_count = cursor.u16() as usize;
    let mut calling_convention = None;
    let mut value_type = None;
    let mut scalar_type = None;
    let mut address_space = None;
    let mut parameter_attribute = None;
    let mut function_attribute = None;
    for _ in 0..kernel_count {
        cursor.string();
        cursor.take(32);
        calling_convention.get_or_insert(cursor.take(1));
        cursor.take(1);
        let parameter_count = cursor.u16() as usize;
        for _ in 0..parameter_count {
            cursor.string();
            let value_position = cursor.take(1);
            value_type.get_or_insert(value_position);
            let tag = bytes[value_position];
            scalar_type.get_or_insert(cursor.take(1));
            if tag == 2 {
                address_space.get_or_insert(cursor.take(1));
            }
            let attribute_count = cursor.u8() as usize;
            for _ in 0..attribute_count {
                let position = cursor.take(1);
                parameter_attribute.get_or_insert(position);
                match bytes[position] {
                    6 => {
                        cursor.take(2);
                    }
                    7 => {
                        cursor.take(4);
                    }
                    _ => {}
                }
            }
        }
        let attribute_count = cursor.u8() as usize;
        for _ in 0..attribute_count {
            let position = cursor.take(1);
            function_attribute.get_or_insert(position);
            match bytes[position] {
                2 => {
                    cursor.take(4);
                }
                3 => {
                    cursor.take(2);
                }
                _ => {}
            }
        }
    }

    let flag_count = cursor.u8() as usize;
    let module_flag = cursor.offset;
    cursor.take(flag_count);
    let metadata_count = cursor.u8() as usize;
    let named_metadata = cursor.offset;
    for _ in 0..metadata_count {
        let tag = bytes[cursor.take(1)];
        if tag == 3 {
            cursor.take(32);
        }
    }
    let library_count = cursor.u8() as usize;
    let device_library = cursor.offset;
    for _ in 0..library_count {
        cursor.take(1 + 32 + 8);
    }

    let origin_count = cursor.u16() as usize;
    let mut origin_identity = None;
    let mut origin = None;
    for _ in 0..origin_count {
        origin_identity.get_or_insert(cursor.take(32));
        origin.get_or_insert(cursor.take(1));
        cursor.take(32);
        let has_span = cursor.u8();
        if has_span == 1 {
            cursor.string();
            cursor.take(16);
        }
    }
    let obligation_count = cursor.u16() as usize;
    let mut obligation_identity = None;
    let mut obligation = None;
    for _ in 0..obligation_count {
        obligation_identity.get_or_insert(cursor.take(32));
        obligation.get_or_insert(cursor.take(1));
        cursor.take(64);
    }
    assert_eq!(cursor.offset, bytes.len());

    WireOffsets {
        target_triple,
        data_layout,
        cpu,
        target_feature,
        code_object,
        calling_convention: calling_convention.unwrap(),
        value_type: value_type.unwrap(),
        scalar_type: scalar_type.unwrap(),
        address_space: address_space.unwrap(),
        parameter_attribute: parameter_attribute.unwrap(),
        function_attribute: function_attribute.unwrap(),
        module_flag,
        named_metadata,
        device_library,
        origin_identity: origin_identity.unwrap(),
        origin: origin.unwrap(),
        obligation_identity: obligation_identity.unwrap(),
        obligation: obligation.unwrap(),
    }
}

struct Cursor<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl<'a> Cursor<'a> {
    fn new(bytes: &'a [u8], offset: usize) -> Self {
        Self { bytes, offset }
    }

    fn take(&mut self, count: usize) -> usize {
        let start = self.offset;
        self.offset += count;
        assert!(self.offset <= self.bytes.len());
        start
    }

    fn u8(&mut self) -> u8 {
        let position = self.take(1);
        self.bytes[position]
    }

    fn u16(&mut self) -> u16 {
        let position = self.take(2);
        u16::from_le_bytes([self.bytes[position], self.bytes[position + 1]])
    }

    fn string(&mut self) {
        let length = self.u16() as usize;
        self.take(length);
    }
}
