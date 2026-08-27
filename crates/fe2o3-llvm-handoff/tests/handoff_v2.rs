use fe2o3_llvm_handoff::{
    AddressSpaceV1, AxisV2, BasicBlockV2, BinaryOperationV2, BlockIdV2, CallTargetV2,
    CallingConventionV2, CastOperationV2, ComparePredicateV2, DecodeHandoffErrorV2,
    DefinitionKindV2, DeviceLibraryInputV1, EvidenceV2, ExecutableModuleV2, FloatBinaryOperationV2,
    FunctionAttributeV1, FunctionAttributeV2, FunctionIdV2, FunctionKindV2, FunctionParameterV2,
    FunctionV2, Gfx942HandoffInputV1, Gfx942HandoffV1, Gfx942HandoffV2, Gfx942TargetPolicyV1,
    GlobalIdV2, GlobalLinkageV2, GlobalV2, HandoffDiagnosticV2, HandoffLimitV2, IdentityV1,
    InstructionKindV2, InstructionV2, IntegerBinaryOperationV2, IntrinsicReferenceV2, IntrinsicV2,
    KernelEntryV1, KernelParameterV1, KernelValueTypeV1, MAX_ARRAY_ELEMENTS_V2,
    MAX_CANONICAL_HANDOFF_BYTES_V2, MAX_FUNCTIONS_V2, MAX_LOCAL_ARRAY_BYTES_V2,
    MAX_VECTOR_LANES_V2, ModuleFlagV1, ModuleMetadataV1, NamedMetadataV1, ObligationKindV1,
    ObligationV1, OriginKindV1, OriginV1, ParameterAttributeV1, ReturnTypeV2, ScalarConstantV2,
    ScalarTypeV1, SourceSpanV1, StageIdentitiesV1, TerminatorV2, TypedValueV2, ValueIdV2,
    ValueTypeV2, WireSectionV2, WorkgroupSizeRangeV1,
};

fn identity(byte: u8) -> IdentityV1 {
    IdentityV1::new([byte; 32]).unwrap()
}

fn kernel_attributes() -> Vec<FunctionAttributeV1> {
    FunctionAttributeV1::gfx942_kernel_defaults(WorkgroupSizeRangeV1::new(64, 256).unwrap())
}

fn base_fixture() -> Gfx942HandoffV1 {
    let origin = OriginV1::new(
        OriginKindV1::AmdgcnIr,
        identity(0x31),
        Some(SourceSpanV1::new("crates/example/src/lib.rs", 7, 1, 20, 2).unwrap()),
    );
    let output_attributes = vec![
        ParameterAttributeV1::NoAlias,
        ParameterAttributeV1::NoCapture,
        ParameterAttributeV1::NonNull,
        ParameterAttributeV1::Align(4),
        ParameterAttributeV1::Dereferenceable(4_096),
    ];
    let kernel = KernelEntryV1::new(
        "alpha_kernel",
        vec![
            KernelParameterV1::new(
                "output",
                KernelValueTypeV1::Pointer {
                    pointee: ScalarTypeV1::F32,
                    address_space: AddressSpaceV1::Global,
                },
                output_attributes,
            )
            .unwrap(),
            KernelParameterV1::new(
                "length",
                KernelValueTypeV1::Scalar(ScalarTypeV1::I64),
                vec![],
            )
            .unwrap(),
        ],
        kernel_attributes(),
        origin.identity(),
    )
    .unwrap();
    let obligations = vec![
        ObligationV1::new(
            ObligationKindV1::PreserveKernelAbi,
            identity(0x41),
            origin.identity(),
        ),
        ObligationV1::new(
            ObligationKindV1::PreserveCallingConvention,
            identity(0x42),
            origin.identity(),
        ),
        ObligationV1::new(
            ObligationKindV1::MaintainOriginCoverage,
            identity(0x43),
            origin.identity(),
        ),
    ];
    Gfx942HandoffV1::new(Gfx942HandoffInputV1 {
        stage_identities: StageIdentitiesV1::new([1; 32], [2; 32], [3; 32]).unwrap(),
        target: Gfx942TargetPolicyV1::canonical(),
        kernels: vec![kernel],
        module: ModuleMetadataV1::new(
            vec![
                ModuleFlagV1::CodeObjectVersion6,
                ModuleFlagV1::PicLevel2,
                ModuleFlagV1::WcharSize4,
            ],
            vec![
                NamedMetadataV1::OpenClVersion2_0,
                NamedMetadataV1::ProducerIdentity(identity(0x51)),
            ],
            Vec::<DeviceLibraryInputV1>::new(),
        )
        .unwrap(),
        origins: vec![origin],
        obligations,
    })
    .unwrap()
}

fn evidence(base: &Gfx942HandoffV1, permuted: bool) -> EvidenceV2 {
    let mut obligations = base
        .obligations()
        .iter()
        .map(|obligation| obligation.identity())
        .collect::<Vec<_>>();
    if permuted {
        obligations.reverse();
    }
    EvidenceV2::new(base.origins()[0].identity(), obligations).unwrap()
}

fn instruction(
    base: &Gfx942HandoffV1,
    result: Option<TypedValueV2>,
    kind: InstructionKindV2,
    permuted: bool,
) -> InstructionV2 {
    InstructionV2::new(result, kind, evidence(base, permuted)).unwrap()
}

fn helper_function(
    base: &Gfx942HandoffV1,
    permuted: bool,
    wrong_binary_result: bool,
) -> FunctionV2 {
    let f32_type = ValueTypeV2::Scalar(ScalarTypeV1::F32);
    let global_pointer = ValueTypeV2::Pointer {
        pointee: ScalarTypeV1::F32,
        address_space: AddressSpaceV1::Constant,
    };
    let binary_result_type = if wrong_binary_result {
        ValueTypeV2::Scalar(ScalarTypeV1::I32)
    } else {
        f32_type
    };
    let instructions = vec![
        instruction(
            base,
            Some(TypedValueV2::new(ValueIdV2::new(2), global_pointer)),
            InstructionKindV2::GlobalAddress(GlobalIdV2::new(1)),
            permuted,
        ),
        instruction(
            base,
            Some(TypedValueV2::new(ValueIdV2::new(3), f32_type)),
            InstructionKindV2::Load {
                pointer: ValueIdV2::new(2),
                value_type: ScalarTypeV1::F32,
                alignment: 4,
            },
            permuted,
        ),
        instruction(
            base,
            Some(TypedValueV2::new(ValueIdV2::new(4), f32_type)),
            InstructionKindV2::Constant(ScalarConstantV2::new(ScalarTypeV1::F32, 0).unwrap()),
            permuted,
        ),
        instruction(
            base,
            Some(TypedValueV2::new(ValueIdV2::new(5), binary_result_type)),
            InstructionKindV2::Binary {
                operation: BinaryOperationV2::Float(FloatBinaryOperationV2::Multiply),
                left: ValueIdV2::new(1),
                right: ValueIdV2::new(3),
            },
            permuted,
        ),
        instruction(
            base,
            Some(TypedValueV2::new(ValueIdV2::new(6), f32_type)),
            InstructionKindV2::Call {
                target: CallTargetV2::Intrinsic(IntrinsicV2::FmaF32),
                arguments: vec![ValueIdV2::new(1), ValueIdV2::new(3), ValueIdV2::new(4)],
            },
            permuted,
        ),
    ];
    let mut attributes = vec![
        FunctionAttributeV2::NoUnwind,
        FunctionAttributeV2::AlwaysInline,
        FunctionAttributeV2::WillReturn,
    ];
    if permuted {
        attributes.reverse();
    }
    FunctionV2::new(
        FunctionIdV2::new(2),
        "scale_value",
        FunctionKindV2::Helper,
        CallingConventionV2::C,
        ReturnTypeV2::Value(f32_type),
        vec![
            FunctionParameterV2::new(
                TypedValueV2::new(ValueIdV2::new(1), f32_type),
                "value",
                vec![],
            )
            .unwrap(),
        ],
        attributes,
        BlockIdV2::new(0),
        vec![BasicBlockV2::new(
            BlockIdV2::new(0),
            instructions,
            TerminatorV2::Return(Some(ValueIdV2::new(6))),
        )],
        evidence(base, permuted),
    )
    .unwrap()
}

fn kernel_function(base: &Gfx942HandoffV1, permuted: bool, renamed_parameter: bool) -> FunctionV2 {
    let v1 = &base.kernels()[0];
    let pointer_type = ValueTypeV2::Pointer {
        pointee: ScalarTypeV1::F32,
        address_space: AddressSpaceV1::Global,
    };
    let i1_type = ValueTypeV2::Scalar(ScalarTypeV1::I1);
    let i32_type = ValueTypeV2::Scalar(ScalarTypeV1::I32);
    let i64_type = ValueTypeV2::Scalar(ScalarTypeV1::I64);
    let f32_type = ValueTypeV2::Scalar(ScalarTypeV1::F32);
    let output_name = if renamed_parameter {
        "changed"
    } else {
        "output"
    };
    let parameters = vec![
        FunctionParameterV2::new(
            TypedValueV2::new(ValueIdV2::new(1), pointer_type),
            output_name,
            v1.parameters()[0].attributes().to_vec(),
        )
        .unwrap(),
        FunctionParameterV2::new(
            TypedValueV2::new(ValueIdV2::new(2), i64_type),
            "length",
            vec![],
        )
        .unwrap(),
    ];
    let entry = BasicBlockV2::new(
        BlockIdV2::new(0),
        vec![
            instruction(
                base,
                Some(TypedValueV2::new(ValueIdV2::new(10), i32_type)),
                InstructionKindV2::Call {
                    target: CallTargetV2::Intrinsic(IntrinsicV2::AmdGpuWorkitemId(AxisV2::X)),
                    arguments: vec![],
                },
                permuted,
            ),
            instruction(
                base,
                Some(TypedValueV2::new(ValueIdV2::new(11), i64_type)),
                InstructionKindV2::Cast {
                    operation: CastOperationV2::ZeroExtend,
                    value: ValueIdV2::new(10),
                    to: i64_type,
                },
                permuted,
            ),
            instruction(
                base,
                Some(TypedValueV2::new(ValueIdV2::new(12), i64_type)),
                InstructionKindV2::Constant(ScalarConstantV2::new(ScalarTypeV1::I64, 0).unwrap()),
                permuted,
            ),
            instruction(
                base,
                Some(TypedValueV2::new(ValueIdV2::new(13), i64_type)),
                InstructionKindV2::Binary {
                    operation: BinaryOperationV2::Integer(IntegerBinaryOperationV2::Add),
                    left: ValueIdV2::new(11),
                    right: ValueIdV2::new(12),
                },
                permuted,
            ),
            instruction(
                base,
                Some(TypedValueV2::new(ValueIdV2::new(14), i1_type)),
                InstructionKindV2::Compare {
                    predicate: ComparePredicateV2::UnsignedLessThan,
                    left: ValueIdV2::new(13),
                    right: ValueIdV2::new(2),
                },
                permuted,
            ),
        ],
        TerminatorV2::ConditionalBranch {
            condition: ValueIdV2::new(14),
            then_block: BlockIdV2::new(1),
            else_block: BlockIdV2::new(2),
        },
    );
    let body = BasicBlockV2::new(
        BlockIdV2::new(1),
        vec![
            instruction(
                base,
                Some(TypedValueV2::new(ValueIdV2::new(15), pointer_type)),
                InstructionKindV2::GetElementPtr {
                    base: ValueIdV2::new(1),
                    indices: vec![ValueIdV2::new(13)],
                },
                permuted,
            ),
            instruction(
                base,
                Some(TypedValueV2::new(ValueIdV2::new(16), f32_type)),
                InstructionKindV2::Constant(
                    ScalarConstantV2::new(ScalarTypeV1::F32, 0x3f80_0000).unwrap(),
                ),
                permuted,
            ),
            instruction(
                base,
                Some(TypedValueV2::new(ValueIdV2::new(17), f32_type)),
                InstructionKindV2::Call {
                    target: CallTargetV2::Function(FunctionIdV2::new(2)),
                    arguments: vec![ValueIdV2::new(16)],
                },
                permuted,
            ),
            instruction(
                base,
                None,
                InstructionKindV2::Store {
                    pointer: ValueIdV2::new(15),
                    value: ValueIdV2::new(17),
                    value_type: ScalarTypeV1::F32,
                    alignment: 4,
                },
                permuted,
            ),
        ],
        TerminatorV2::Branch(BlockIdV2::new(2)),
    );
    let exit = BasicBlockV2::new(BlockIdV2::new(2), vec![], TerminatorV2::Return(None));
    let mut blocks = vec![entry, body, exit];
    let mut attributes = v1
        .function_attributes()
        .iter()
        .copied()
        .map(FunctionAttributeV2::from)
        .collect::<Vec<_>>();
    if permuted {
        blocks.reverse();
        attributes.reverse();
    }
    FunctionV2::new(
        FunctionIdV2::new(10),
        "alpha_kernel",
        FunctionKindV2::Kernel,
        CallingConventionV2::AmdGpuKernel,
        ReturnTypeV2::Void,
        parameters,
        attributes,
        BlockIdV2::new(0),
        blocks,
        evidence(base, permuted),
    )
    .unwrap()
}

fn module_fixture(base: &Gfx942HandoffV1, permuted: bool) -> ExecutableModuleV2 {
    let global = GlobalV2::new(
        GlobalIdV2::new(1),
        "scale",
        GlobalLinkageV2::Internal,
        AddressSpaceV1::Constant,
        false,
        ScalarTypeV1::F32,
        Some(ScalarConstantV2::new(ScalarTypeV1::F32, 0x4000_0000).unwrap()),
        evidence(base, permuted),
    )
    .unwrap();
    let external_global = GlobalV2::new(
        GlobalIdV2::new(3),
        "extern_counter",
        GlobalLinkageV2::External,
        AddressSpaceV1::Global,
        true,
        ScalarTypeV1::I64,
        None,
        evidence(base, permuted),
    )
    .unwrap();
    let mut intrinsics = vec![
        IntrinsicReferenceV2::new(
            IntrinsicV2::AmdGpuWorkitemId(AxisV2::X),
            evidence(base, permuted),
        ),
        IntrinsicReferenceV2::new(IntrinsicV2::FmaF32, evidence(base, permuted)),
    ];
    let mut functions = vec![
        helper_function(base, permuted, false),
        kernel_function(base, permuted, false),
    ];
    let mut flags = base.module().flags().to_vec();
    let mut named = base.module().named_metadata().to_vec();
    let mut globals = vec![global, external_global];
    if permuted {
        globals.reverse();
        intrinsics.reverse();
        functions.reverse();
        flags.reverse();
        named.reverse();
    }
    ExecutableModuleV2::new(flags, named, globals, intrinsics, functions).unwrap()
}

fn fixture(permuted: bool) -> Gfx942HandoffV2 {
    let base = base_fixture();
    let module = module_fixture(&base, permuted);
    Gfx942HandoffV2::new(base, module).unwrap()
}

#[test]
fn positive_gfx942_executable_module_round_trips_and_embeds_exact_v1() {
    let handoff = fixture(false);
    assert_eq!(handoff.module().globals().len(), 2);
    assert_eq!(handoff.module().globals()[0].symbol(), "scale");
    assert_eq!(handoff.module().intrinsics().len(), 2);
    assert_eq!(handoff.module().functions().len(), 2);
    assert_eq!(handoff.module().functions()[0].symbol(), "scale_value");
    assert_eq!(handoff.module().functions()[1].symbol(), "alpha_kernel");

    let encoded = handoff.encode_canonical();
    assert_eq!(
        handoff.identity().to_string(),
        "12e729d08aba77fb96e11a4d07011febed57b67edea30a01f07af30c7d7d250b"
    );
    assert_eq!(
        handoff.module().identity().to_string(),
        "bad5494c2ccc19a9c39e4b9ddbc247feaef972bd7ab2ad89f354e96e777e6b48"
    );
    assert!(encoded.as_bytes().starts_with(b"F2LLVMH2"));
    assert!(encoded.len() < MAX_CANONICAL_HANDOFF_BYTES_V2);
    let base_len = u32::from_le_bytes(encoded.as_bytes()[16..20].try_into().unwrap()) as usize;
    assert_eq!(
        &encoded.as_bytes()[20..20 + base_len],
        handoff.base().encode_canonical().as_bytes()
    );
    let base_identity = handoff.base().identity();
    let decoded = Gfx942HandoffV2::decode_canonical(encoded.as_bytes()).unwrap();
    assert_eq!(decoded, handoff);
    assert_eq!(decoded.encode_canonical(), encoded);
    assert_eq!(decoded.base().identity(), base_identity);
    assert_eq!(decoded.module().identity(), handoff.module().identity());
}

#[test]
fn unordered_v2_collections_have_one_encoding_and_identity() {
    let ordered = fixture(false);
    let permuted = fixture(true);
    assert_eq!(ordered, permuted);
    assert_eq!(ordered.encode_canonical(), permuted.encode_canonical());
    assert_eq!(ordered.identity(), permuted.identity());
    assert_eq!(ordered.module().identity(), permuted.module().identity());
}

#[test]
fn duplicate_conflicting_and_oversized_models_fail_closed() {
    let base = base_fixture();
    let module = module_fixture(&base, false);
    let mut globals = module.globals().to_vec();
    globals.push(globals[0].clone());
    assert_eq!(
        ExecutableModuleV2::new(
            module.flags().to_vec(),
            module.named_metadata().to_vec(),
            globals,
            module.intrinsics().to_vec(),
            module.functions().to_vec(),
        ),
        Err(HandoffDiagnosticV2::DuplicateDefinition(
            DefinitionKindV2::Global
        ))
    );

    let duplicate_obligation = base.obligations()[0].identity();
    assert!(matches!(
        EvidenceV2::new(
            base.origins()[0].identity(),
            vec![duplicate_obligation, duplicate_obligation],
        ),
        Err(HandoffDiagnosticV2::DuplicateDefinition(_))
    ));

    assert_eq!(
        FunctionV2::new(
            FunctionIdV2::new(90),
            "conflict",
            FunctionKindV2::Helper,
            CallingConventionV2::C,
            ReturnTypeV2::Void,
            vec![],
            vec![
                FunctionAttributeV2::NoUnwind,
                FunctionAttributeV2::AlwaysInline,
                FunctionAttributeV2::NoInline,
            ],
            BlockIdV2::new(0),
            vec![BasicBlockV2::new(
                BlockIdV2::new(0),
                vec![],
                TerminatorV2::Return(None),
            )],
            evidence(&base, false),
        ),
        Err(HandoffDiagnosticV2::ConflictingFunctionAttributes)
    );

    let functions = vec![helper_function(&base, false, false); MAX_FUNCTIONS_V2 + 1];
    assert!(matches!(
        ExecutableModuleV2::new(
            module.flags().to_vec(),
            module.named_metadata().to_vec(),
            module.globals().to_vec(),
            module.intrinsics().to_vec(),
            functions,
        ),
        Err(HandoffDiagnosticV2::LimitExceeded {
            limit: HandoffLimitV2::Functions,
            ..
        })
    ));
}

#[test]
fn graph_metadata_kernel_and_provenance_mismatches_fail_closed() {
    let base = base_fixture();
    let valid = module_fixture(&base, false);
    let wrong_helper = helper_function(&base, false, true);
    assert_eq!(
        ExecutableModuleV2::new(
            valid.flags().to_vec(),
            valid.named_metadata().to_vec(),
            valid.globals().to_vec(),
            valid.intrinsics().to_vec(),
            vec![wrong_helper, kernel_function(&base, false, false)],
        ),
        Err(HandoffDiagnosticV2::ValueTypeMismatch(ValueIdV2::new(5)))
    );

    assert!(matches!(
        ExecutableModuleV2::new(
            valid.flags().to_vec(),
            valid.named_metadata().to_vec(),
            valid.globals().to_vec(),
            vec![valid.intrinsics()[0].clone()],
            valid.functions().to_vec(),
        ),
        Err(HandoffDiagnosticV2::MissingIntrinsicReference)
    ));

    let renamed = ExecutableModuleV2::new(
        valid.flags().to_vec(),
        valid.named_metadata().to_vec(),
        valid.globals().to_vec(),
        valid.intrinsics().to_vec(),
        vec![
            helper_function(&base, false, false),
            kernel_function(&base, false, true),
        ],
    )
    .unwrap();
    assert_eq!(
        Gfx942HandoffV2::new(base.clone(), renamed),
        Err(HandoffDiagnosticV2::KernelSignatureMismatch)
    );

    let mut bad_flags = valid.flags().to_vec();
    bad_flags.pop();
    let bad_metadata = ExecutableModuleV2::new(
        bad_flags,
        valid.named_metadata().to_vec(),
        valid.globals().to_vec(),
        valid.intrinsics().to_vec(),
        valid.functions().to_vec(),
    )
    .unwrap();
    assert_eq!(
        Gfx942HandoffV2::new(base.clone(), bad_metadata),
        Err(HandoffDiagnosticV2::MetadataMismatch)
    );

    let alien = OriginV1::new(OriginKindV1::AmdgcnIr, identity(0xee), None);
    let alien_global = GlobalV2::new(
        GlobalIdV2::new(1),
        "scale",
        GlobalLinkageV2::Internal,
        AddressSpaceV1::Constant,
        false,
        ScalarTypeV1::F32,
        Some(ScalarConstantV2::new(ScalarTypeV1::F32, 0).unwrap()),
        EvidenceV2::new(alien.identity(), vec![]).unwrap(),
    )
    .unwrap();
    let alien_module = ExecutableModuleV2::new(
        valid.flags().to_vec(),
        valid.named_metadata().to_vec(),
        vec![alien_global],
        valid.intrinsics().to_vec(),
        valid.functions().to_vec(),
    )
    .unwrap();
    assert_eq!(
        Gfx942HandoffV2::new(base, alien_module),
        Err(HandoffDiagnosticV2::MissingOriginReference)
    );
}

#[test]
fn truncation_headers_and_oversized_wire_counts_fail_closed() {
    let encoded = fixture(false).encode_canonical();
    for length in 0..encoded.len() {
        assert!(
            Gfx942HandoffV2::decode_canonical(&encoded.as_bytes()[..length]).is_err(),
            "truncated prefix of {length} bytes was accepted"
        );
    }

    let mut bad_magic = encoded.as_bytes().to_vec();
    bad_magic[0] ^= 1;
    assert_eq!(
        Gfx942HandoffV2::decode_canonical(&bad_magic),
        Err(DecodeHandoffErrorV2::BadMagic)
    );
    let mut bad_version = encoded.as_bytes().to_vec();
    bad_version[8..10].copy_from_slice(&3_u16.to_le_bytes());
    assert_eq!(
        Gfx942HandoffV2::decode_canonical(&bad_version),
        Err(DecodeHandoffErrorV2::UnsupportedVersion(3))
    );
    assert!(matches!(
        Gfx942HandoffV2::decode_canonical(&vec![0; MAX_CANONICAL_HANDOFF_BYTES_V2 + 1]),
        Err(DecodeHandoffErrorV2::TooLong { .. })
    ));

    let offsets = wire_offsets(encoded.as_bytes());
    let mut oversized = encoded.as_bytes().to_vec();
    oversized[offsets.function_count..offsets.function_count + 2]
        .copy_from_slice(&u16::MAX.to_le_bytes());
    assert_eq!(
        Gfx942HandoffV2::decode_canonical(&oversized),
        Err(DecodeHandoffErrorV2::LimitExceeded {
            limit: HandoffLimitV2::Functions,
            observed: u16::MAX as usize,
            maximum: MAX_FUNCTIONS_V2,
        })
    );
}

#[test]
fn unknown_v2_semantic_tags_fail_with_typed_sections() {
    let encoded = fixture(false).encode_canonical();
    let offsets = wire_offsets(encoded.as_bytes());
    let cases = [
        (offsets.module_flag, WireSectionV2::ModuleFlag),
        (offsets.named_metadata, WireSectionV2::NamedMetadata),
        (offsets.global_linkage, WireSectionV2::GlobalLinkage),
        (offsets.address_space, WireSectionV2::AddressSpace),
        (offsets.scalar_type, WireSectionV2::ScalarType),
        (offsets.intrinsic, WireSectionV2::Intrinsic),
        (offsets.axis, WireSectionV2::Axis),
        (offsets.function_kind, WireSectionV2::FunctionKind),
        (offsets.calling_convention, WireSectionV2::CallingConvention),
        (offsets.return_type, WireSectionV2::ReturnType),
        (offsets.value_type, WireSectionV2::ValueType),
        (
            offsets.parameter_attribute,
            WireSectionV2::ParameterAttribute,
        ),
        (offsets.function_attribute, WireSectionV2::FunctionAttribute),
        (offsets.instruction, WireSectionV2::Instruction),
        (offsets.binary, WireSectionV2::BinaryOperation),
        (
            offsets.integer_binary,
            WireSectionV2::IntegerBinaryOperation,
        ),
        (offsets.float_binary, WireSectionV2::FloatBinaryOperation),
        (offsets.compare, WireSectionV2::ComparePredicate),
        (offsets.cast, WireSectionV2::CastOperation),
        (offsets.call_target, WireSectionV2::CallTarget),
        (offsets.terminator, WireSectionV2::Terminator),
    ];
    for (offset, section) in cases {
        let mut hostile = encoded.as_bytes().to_vec();
        hostile[offset] = 0xff;
        assert_eq!(
            Gfx942HandoffV2::decode_canonical(&hostile),
            Err(DecodeHandoffErrorV2::UnknownTag { section, tag: 0xff }),
            "mutation at {offset} did not fail in {section:?}"
        );
    }
}

#[test]
fn bounded_machine_shapes_and_descriptor_policy_fail_closed() {
    let base = base_fixture();
    assert_eq!(
        GlobalV2::new_private_constant_bytes(
            GlobalIdV2::new(90),
            "descriptor",
            ".unreviewed",
            vec![1],
            1,
            evidence(&base, false),
        ),
        Err(HandoffDiagnosticV2::UnsupportedInstruction)
    );
    assert_eq!(
        GlobalV2::new_private_constant_bytes(
            GlobalIdV2::new(90),
            "descriptor",
            fe2o3_llvm_handoff::KERNEL_DESCRIPTOR_SECTION_V2,
            vec![],
            1,
            evidence(&base, false),
        ),
        Err(HandoffDiagnosticV2::UnsupportedInstruction)
    );

    let invalid_workgroup = FunctionV2::new(
        FunctionIdV2::new(90),
        "invalid_workgroup",
        FunctionKindV2::Kernel,
        CallingConventionV2::AmdGpuKernel,
        ReturnTypeV2::Void,
        vec![],
        vec![
            FunctionAttributeV2::NoUnwind,
            FunctionAttributeV2::RequiredWorkgroupSize([64, 0, 1]),
        ],
        BlockIdV2::new(0),
        vec![BasicBlockV2::new(
            BlockIdV2::new(0),
            vec![],
            TerminatorV2::Return(None),
        )],
        evidence(&base, false),
    );
    assert_eq!(
        invalid_workgroup,
        Err(HandoffDiagnosticV2::InvalidFunctionAttribute)
    );

    assert_eq!(
        ValueTypeV2::fixed_vector(ScalarTypeV1::I8, 0),
        Err(HandoffDiagnosticV2::EmptyCollection("vector lanes"))
    );
    assert_eq!(
        ValueTypeV2::fixed_vector(ScalarTypeV1::F64, MAX_VECTOR_LANES_V2 + 1),
        Err(HandoffDiagnosticV2::LimitExceeded {
            limit: HandoffLimitV2::VectorLanes,
            observed: (MAX_VECTOR_LANES_V2 + 1) as u64,
            maximum: MAX_VECTOR_LANES_V2 as u64,
        })
    );
    assert_eq!(
        ValueTypeV2::fixed_vector(ScalarTypeV1::I32, usize::MAX),
        Err(HandoffDiagnosticV2::LimitExceeded {
            limit: HandoffLimitV2::VectorLanes,
            observed: u64::MAX,
            maximum: MAX_VECTOR_LANES_V2 as u64,
        })
    );
    for lanes in [1, MAX_VECTOR_LANES_V2] {
        assert!(ValueTypeV2::fixed_vector(ScalarTypeV1::F16, lanes).is_ok());
    }
    assert_eq!(
        ValueTypeV2::array_pointer(ScalarTypeV1::I16, 0, AddressSpaceV1::Global),
        Err(HandoffDiagnosticV2::EmptyCollection("array elements"))
    );
    assert_eq!(
        ValueTypeV2::array_pointer(
            ScalarTypeV1::I16,
            MAX_ARRAY_ELEMENTS_V2 + 1,
            AddressSpaceV1::Global,
        ),
        Err(HandoffDiagnosticV2::LimitExceeded {
            limit: HandoffLimitV2::ArrayElements,
            observed: (MAX_ARRAY_ELEMENTS_V2 + 1) as u64,
            maximum: MAX_ARRAY_ELEMENTS_V2 as u64,
        })
    );
    assert_eq!(
        ValueTypeV2::array_pointer(ScalarTypeV1::I8, usize::MAX, AddressSpaceV1::Global,),
        Err(HandoffDiagnosticV2::LimitExceeded {
            limit: HandoffLimitV2::ArrayElements,
            observed: u64::MAX,
            maximum: MAX_ARRAY_ELEMENTS_V2 as u64,
        })
    );
    for elements in [1, MAX_ARRAY_ELEMENTS_V2] {
        assert!(
            ValueTypeV2::array_pointer(ScalarTypeV1::F32, elements, AddressSpaceV1::Global,)
                .is_ok()
        );
    }
    assert_eq!(
        ValueTypeV2::array_pointer(
            ScalarTypeV1::F64,
            MAX_LOCAL_ARRAY_BYTES_V2 / 8 + 1,
            AddressSpaceV1::Local,
        ),
        Err(HandoffDiagnosticV2::LimitExceeded {
            limit: HandoffLimitV2::LocalArrayBytes,
            observed: (MAX_LOCAL_ARRAY_BYTES_V2 + 8) as u64,
            maximum: MAX_LOCAL_ARRAY_BYTES_V2 as u64,
        })
    );

    let legal_vector = FunctionV2::new(
        FunctionIdV2::new(91),
        "legal_vector_shape",
        FunctionKindV2::Helper,
        CallingConventionV2::C,
        ReturnTypeV2::Value(ValueTypeV2::fixed_vector(ScalarTypeV1::I8, 3).unwrap()),
        vec![],
        vec![FunctionAttributeV2::NoUnwind],
        BlockIdV2::new(0),
        vec![BasicBlockV2::new(
            BlockIdV2::new(0),
            vec![],
            TerminatorV2::Unreachable,
        )],
        evidence(&base, false),
    )
    .unwrap();
    let existing = module_fixture(&base, false);
    let mut globals = existing.globals().to_vec();
    globals.push(
        GlobalV2::new_local_array(
            GlobalIdV2::new(90),
            "local_i8_17",
            ScalarTypeV1::I8,
            17,
            1,
            evidence(&base, false),
        )
        .unwrap(),
    );
    globals.push(
        GlobalV2::new_local_array(
            GlobalIdV2::new(91),
            "local_f64_1024",
            ScalarTypeV1::F64,
            1024,
            32,
            evidence(&base, false),
        )
        .unwrap(),
    );
    assert_eq!(
        GlobalV2::new_local_array(
            GlobalIdV2::new(92),
            "too_large",
            ScalarTypeV1::F64,
            MAX_LOCAL_ARRAY_BYTES_V2 / 8 + 1,
            16,
            evidence(&base, false),
        ),
        Err(HandoffDiagnosticV2::LimitExceeded {
            limit: HandoffLimitV2::LocalArrayBytes,
            observed: (MAX_LOCAL_ARRAY_BYTES_V2 + 8) as u64,
            maximum: MAX_LOCAL_ARRAY_BYTES_V2 as u64,
        })
    );
    assert!(
        GlobalV2::new_local_array(
            GlobalIdV2::new(92),
            "exact_local_cap",
            ScalarTypeV1::F64,
            MAX_LOCAL_ARRAY_BYTES_V2 / 8,
            256,
            evidence(&base, false),
        )
        .is_ok()
    );
    assert_eq!(
        GlobalV2::new_local_array(
            GlobalIdV2::new(92),
            "invalid_alignment",
            ScalarTypeV1::I8,
            1,
            0,
            evidence(&base, false),
        ),
        Err(HandoffDiagnosticV2::InvalidAlignment)
    );
    let mut functions = existing.functions().to_vec();
    functions.push(legal_vector);
    let module = ExecutableModuleV2::new(
        existing.flags().to_vec(),
        existing.named_metadata().to_vec(),
        globals,
        existing.intrinsics().to_vec(),
        functions,
    )
    .unwrap();
    let handoff = Gfx942HandoffV2::new(base, module).unwrap();
    let encoded = handoff.encode_canonical();
    assert_eq!(
        Gfx942HandoffV2::decode_canonical(encoded.as_bytes()).unwrap(),
        handoff
    );
}

#[test]
fn authenticated_semantic_origin_and_obligation_mutations_fail_closed() {
    let encoded = fixture(false).encode_canonical();
    let offsets = wire_offsets(encoded.as_bytes());
    for offset in [
        offsets.float_binary,
        offsets.evidence_origin,
        offsets.evidence_obligation,
    ] {
        let mut hostile = encoded.as_bytes().to_vec();
        hostile[offset] ^= 1;
        assert_eq!(
            Gfx942HandoffV2::decode_canonical(&hostile),
            Err(DecodeHandoffErrorV2::NonCanonical),
            "authenticated semantic mutation at {offset} was not rejected"
        );
    }
}

#[derive(Debug)]
struct WireOffsets {
    module_flag: usize,
    named_metadata: usize,
    global_linkage: usize,
    address_space: usize,
    scalar_type: usize,
    intrinsic: usize,
    axis: usize,
    function_count: usize,
    function_kind: usize,
    calling_convention: usize,
    return_type: usize,
    value_type: usize,
    parameter_attribute: usize,
    function_attribute: usize,
    instruction: usize,
    binary: usize,
    integer_binary: usize,
    float_binary: usize,
    compare: usize,
    cast: usize,
    call_target: usize,
    terminator: usize,
    evidence_origin: usize,
    evidence_obligation: usize,
}

fn wire_offsets(bytes: &[u8]) -> WireOffsets {
    let base_len = u32::from_le_bytes(bytes[16..20].try_into().unwrap()) as usize;
    let mut cursor = Cursor::new(bytes, 20 + base_len + 32);
    let flag_count = cursor.u8() as usize;
    let module_flag = cursor.offset;
    cursor.take(flag_count);
    let metadata_count = cursor.u8() as usize;
    let named_metadata = cursor.offset;
    for _ in 0..metadata_count {
        let tag = cursor.u8();
        if tag == 3 {
            cursor.take(32);
        }
    }
    let global_count = cursor.u16() as usize;
    let mut global_linkage = None;
    let mut address_space = None;
    let mut scalar_type = None;
    let mut evidence_origin = None;
    let mut evidence_obligation = None;
    for _ in 0..global_count {
        cursor.take(4);
        cursor.string();
        global_linkage.get_or_insert(cursor.take(1));
        address_space.get_or_insert(cursor.take(1));
        cursor.take(1);
        scalar_type.get_or_insert(cursor.take(1));
        let initializer = cursor.u8();
        if initializer == 1 {
            cursor.take(1 + 8);
        }
        cursor.evidence(&mut evidence_origin, &mut evidence_obligation);
    }
    let intrinsic_count = cursor.u16() as usize;
    let mut intrinsic = None;
    let mut axis = None;
    for _ in 0..intrinsic_count {
        let tag_position = cursor.take(1);
        intrinsic.get_or_insert(tag_position);
        if matches!(bytes[tag_position], 1 | 2) {
            axis.get_or_insert(cursor.take(1));
        }
        cursor.evidence(&mut evidence_origin, &mut evidence_obligation);
    }
    let function_count = cursor.offset;
    let functions = cursor.u16() as usize;
    let mut function_kind = None;
    let mut calling_convention = None;
    let mut return_type = None;
    let mut value_type = None;
    let mut parameter_attribute = None;
    let mut function_attribute = None;
    let mut instruction = None;
    let mut binary = None;
    let mut integer_binary = None;
    let mut float_binary = None;
    let mut compare = None;
    let mut cast = None;
    let mut call_target = None;
    let mut terminator = None;
    for _ in 0..functions {
        cursor.take(4);
        cursor.string();
        function_kind.get_or_insert(cursor.take(1));
        calling_convention.get_or_insert(cursor.take(1));
        let return_position = cursor.take(1);
        return_type.get_or_insert(return_position);
        if bytes[return_position] == 2 {
            cursor.value_type(&mut value_type, &mut scalar_type, &mut address_space);
        }
        cursor.evidence(&mut evidence_origin, &mut evidence_obligation);
        let parameter_count = cursor.u16() as usize;
        for _ in 0..parameter_count {
            cursor.take(4);
            cursor.value_type(&mut value_type, &mut scalar_type, &mut address_space);
            cursor.string();
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
                6 => {
                    cursor.take(4);
                }
                7 => {
                    cursor.take(2);
                }
                _ => {}
            }
        }
        cursor.take(4);
        let block_count = cursor.u16() as usize;
        for _ in 0..block_count {
            cursor.take(4);
            let instruction_count = cursor.u32() as usize;
            for _ in 0..instruction_count {
                if cursor.u8() == 1 {
                    cursor.take(4);
                    cursor.value_type(&mut value_type, &mut scalar_type, &mut address_space);
                }
                let opcode = cursor.take(1);
                instruction.get_or_insert(opcode);
                match bytes[opcode] {
                    1 => {
                        cursor.take(1 + 8);
                    }
                    2 => {
                        cursor.take(4);
                    }
                    3 => {
                        let category = cursor.take(1);
                        binary.get_or_insert(category);
                        let operation = cursor.take(1);
                        if bytes[category] == 1 {
                            integer_binary.get_or_insert(operation);
                        } else {
                            float_binary.get_or_insert(operation);
                        }
                        cursor.take(8);
                    }
                    4 => {
                        compare.get_or_insert(cursor.take(1));
                        cursor.take(8);
                    }
                    5 => {
                        cast.get_or_insert(cursor.take(1));
                        cursor.take(4);
                        cursor.value_type(&mut value_type, &mut scalar_type, &mut address_space);
                    }
                    6 => {
                        cursor.take(4);
                        let count = cursor.u8() as usize;
                        cursor.take(count * 4);
                    }
                    7 => {
                        cursor.take(4 + 1 + 2);
                    }
                    8 => {
                        cursor.take(4 + 4 + 1 + 2);
                    }
                    9 => {
                        let target = cursor.take(1);
                        call_target.get_or_insert(target);
                        if bytes[target] == 1 {
                            cursor.take(4);
                        } else {
                            let intrinsic_tag = cursor.u8();
                            if matches!(intrinsic_tag, 1 | 2) {
                                cursor.take(1);
                            }
                        }
                        let count = cursor.u16() as usize;
                        cursor.take(count * 4);
                    }
                    tag => panic!("unexpected fixture opcode {tag}"),
                }
                cursor.evidence(&mut evidence_origin, &mut evidence_obligation);
            }
            let terminal = cursor.take(1);
            terminator.get_or_insert(terminal);
            match bytes[terminal] {
                1 | 5 => {}
                2 | 3 => {
                    cursor.take(4);
                }
                4 => {
                    cursor.take(12);
                }
                tag => panic!("unexpected fixture terminator {tag}"),
            }
        }
    }
    assert_eq!(cursor.offset, bytes.len());
    WireOffsets {
        module_flag,
        named_metadata,
        global_linkage: global_linkage.unwrap(),
        address_space: address_space.unwrap(),
        scalar_type: scalar_type.unwrap(),
        intrinsic: intrinsic.unwrap(),
        axis: axis.unwrap(),
        function_count,
        function_kind: function_kind.unwrap(),
        calling_convention: calling_convention.unwrap(),
        return_type: return_type.unwrap(),
        value_type: value_type.unwrap(),
        parameter_attribute: parameter_attribute.unwrap(),
        function_attribute: function_attribute.unwrap(),
        instruction: instruction.unwrap(),
        binary: binary.unwrap(),
        integer_binary: integer_binary.unwrap(),
        float_binary: float_binary.unwrap(),
        compare: compare.unwrap(),
        cast: cast.unwrap(),
        call_target: call_target.unwrap(),
        terminator: terminator.unwrap(),
        evidence_origin: evidence_origin.unwrap(),
        evidence_obligation: evidence_obligation.unwrap(),
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

    fn u32(&mut self) -> u32 {
        let position = self.take(4);
        u32::from_le_bytes(self.bytes[position..position + 4].try_into().unwrap())
    }

    fn string(&mut self) {
        let length = self.u16() as usize;
        self.take(length);
    }

    fn evidence(&mut self, origin: &mut Option<usize>, obligation: &mut Option<usize>) {
        origin.get_or_insert(self.take(32));
        let count = self.u8() as usize;
        for _ in 0..count {
            obligation.get_or_insert(self.take(32));
        }
    }

    fn value_type(
        &mut self,
        value_type: &mut Option<usize>,
        scalar_type: &mut Option<usize>,
        address_space: &mut Option<usize>,
    ) {
        let position = self.take(1);
        value_type.get_or_insert(position);
        scalar_type.get_or_insert(self.take(1));
        if self.bytes[position] == 2 {
            address_space.get_or_insert(self.take(1));
        }
    }
}
