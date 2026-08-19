#![forbid(unsafe_code)]

//! Public V2 handoff model and canonical-codec conformance tests.
//!
//! These tests make schema and byte-codec claims only. They deliberately record
//! public-API gaps instead of treating an unavailable encoding as exercised.

use fe2o3_llvm_handoff::{
    AddressSpaceV1, AxisV2, BasicBlockV2, BinaryOperationV2, BlockIdV2, CallTargetV2,
    CallingConventionV2, CastOperationV2, ComparePredicateV2, DecodeHandoffErrorV2,
    DefinitionKindV2, DeviceLibraryInputV1, EvidenceV2, ExecutableModuleV2, FloatBinaryOperationV2,
    FunctionAttributeV1, FunctionAttributeV2, FunctionIdV2, FunctionKindV2, FunctionParameterV2,
    FunctionV2, Gfx942HandoffInputV1, Gfx942HandoffV1, Gfx942HandoffV2, Gfx942TargetPolicyV1,
    GlobalIdV2, GlobalLinkageV2, GlobalV2, HandoffDiagnosticV2, IdentityV1, InstructionKindV2,
    InstructionV2, IntegerBinaryOperationV2, IntrinsicReferenceV2, IntrinsicV2, KernelEntryV1,
    KernelParameterV1, KernelValueTypeV1, ModuleFlagV1, ModuleMetadataV1, NamedMetadataV1,
    ObligationKindV1, ObligationV1, OriginKindV1, OriginV1, ParameterAttributeV1, ReturnTypeV2,
    ScalarConstantV2, ScalarTypeV1, StageIdentitiesV1, TerminatorV2, TypedValueV2, ValueIdV2,
    ValueTypeV2, WireSectionV2, WorkgroupSizeRangeV1,
};

const I1: ValueTypeV2 = ValueTypeV2::Scalar(ScalarTypeV1::I1);
const I32: ValueTypeV2 = ValueTypeV2::Scalar(ScalarTypeV1::I32);
const I64: ValueTypeV2 = ValueTypeV2::Scalar(ScalarTypeV1::I64);
const F32: ValueTypeV2 = ValueTypeV2::Scalar(ScalarTypeV1::F32);

const PUBLIC_V2_UNREPRESENTABLE_GAPS: [&str; 4] = [
    "arbitrary intrinsic declarations and calls",
    "atomic instructions and memory orderings",
    "switch terminators and cases",
    "aggregate values and operations",
];

#[test]
fn v2_handoff_public_canonical_bytes_are_stable_and_identity_sensitive() {
    let ordered = handoff_fixture(false, 0x3f80_0000);
    let permuted = handoff_fixture(true, 0x3f80_0000);

    assert_eq!(ordered, permuted);
    assert_eq!(ordered.module().identity(), permuted.module().identity());
    assert_eq!(ordered.identity(), permuted.identity());
    assert_eq!(ordered.encode_canonical(), permuted.encode_canonical());

    let canonical = ordered.encode_canonical();
    let base_len = u32::from_le_bytes(canonical.as_bytes()[16..20].try_into().unwrap()) as usize;
    assert_eq!(
        &canonical.as_bytes()[20..20 + base_len],
        ordered.base().encode_canonical().as_bytes()
    );
    let decoded = Gfx942HandoffV2::decode_canonical(canonical.as_bytes()).unwrap();
    assert_eq!(decoded, ordered);
    assert_eq!(decoded.encode_canonical(), canonical);

    let changed = handoff_fixture(false, 0x4000_0000);
    assert_ne!(changed.module().identity(), ordered.module().identity());
    assert_ne!(changed.identity(), ordered.identity());
    assert_ne!(changed.encode_canonical(), ordered.encode_canonical());
    assert_eq!(
        Gfx942HandoffV2::decode_canonical(changed.encode_canonical().as_bytes()).unwrap(),
        changed
    );
}

#[test]
fn v2_handoff_rejects_duplicate_ids_symbols_blocks_parameters_and_values() {
    let base = base_fixture();
    let global_a = external_global(
        &base,
        GlobalIdV2::new(1),
        "global_a",
        AddressSpaceV1::Global,
    );
    let same_id = external_global(&base, GlobalIdV2::new(1), "global_b", AddressSpaceV1::Local);
    let same_symbol = external_global(
        &base,
        GlobalIdV2::new(2),
        "global_a",
        AddressSpaceV1::Private,
    );
    let function = empty_helper(&base, FunctionIdV2::new(1), "helper_a");

    assert_eq!(
        module_from_parts(
            &base,
            vec![global_a.clone(), same_id],
            vec![],
            vec![function.clone()]
        ),
        Err(HandoffDiagnosticV2::DuplicateDefinition(
            DefinitionKindV2::Global
        ))
    );
    assert_eq!(
        module_from_parts(
            &base,
            vec![global_a.clone(), same_symbol],
            vec![],
            vec![function.clone()],
        ),
        Err(HandoffDiagnosticV2::DuplicateDefinition(
            DefinitionKindV2::Global
        ))
    );

    let same_function_id = empty_helper(&base, FunctionIdV2::new(1), "helper_b");
    let same_function_symbol = empty_helper(&base, FunctionIdV2::new(2), "helper_a");
    assert_eq!(
        module_from_parts(
            &base,
            vec![],
            vec![],
            vec![function.clone(), same_function_id],
        ),
        Err(HandoffDiagnosticV2::DuplicateDefinition(
            DefinitionKindV2::Function
        ))
    );
    assert_eq!(
        module_from_parts(
            &base,
            vec![],
            vec![],
            vec![function.clone(), same_function_symbol],
        ),
        Err(HandoffDiagnosticV2::DuplicateDefinition(
            DefinitionKindV2::Function
        ))
    );

    let cross_namespace_global =
        external_global(&base, GlobalIdV2::new(3), "helper_a", AddressSpaceV1::Flat);
    assert_eq!(
        module_from_parts(&base, vec![cross_namespace_global], vec![], vec![function],),
        Err(HandoffDiagnosticV2::DuplicateDefinition(
            DefinitionKindV2::Symbol
        ))
    );

    let duplicate_blocks = FunctionV2::new(
        FunctionIdV2::new(10),
        "duplicate_blocks",
        FunctionKindV2::Helper,
        CallingConventionV2::C,
        ReturnTypeV2::Void,
        vec![],
        vec![FunctionAttributeV2::NoUnwind],
        BlockIdV2::new(0),
        vec![empty_block(0), empty_block(0)],
        evidence(&base, false),
    );
    assert_eq!(
        duplicate_blocks,
        Err(HandoffDiagnosticV2::DuplicateDefinition(
            DefinitionKindV2::Block
        ))
    );

    let duplicate_parameter_id = function_with_parameters(
        &base,
        vec![
            parameter(1, "left", I32, vec![]),
            parameter(1, "right", I64, vec![]),
        ],
    );
    let duplicate_parameter_name = function_with_parameters(
        &base,
        vec![
            parameter(1, "same", I32, vec![]),
            parameter(2, "same", I64, vec![]),
        ],
    );
    assert_eq!(
        duplicate_parameter_id,
        Err(HandoffDiagnosticV2::DuplicateDefinition(
            DefinitionKindV2::Parameter
        ))
    );
    assert_eq!(
        duplicate_parameter_name,
        Err(HandoffDiagnosticV2::DuplicateDefinition(
            DefinitionKindV2::Parameter
        ))
    );

    let first = constant_instruction(&base, 7, ScalarTypeV1::I32, 1);
    let second = constant_instruction(&base, 7, ScalarTypeV1::I32, 2);
    let duplicate_values = helper_with_blocks(
        &base,
        FunctionIdV2::new(11),
        "duplicate_values",
        vec![],
        vec![FunctionAttributeV2::NoUnwind],
        vec![BasicBlockV2::new(
            BlockIdV2::new(0),
            vec![first, second],
            TerminatorV2::Return(None),
        )],
    );
    assert_eq!(
        module_from_parts(&base, vec![], vec![], vec![duplicate_values]),
        Err(HandoffDiagnosticV2::DuplicateDefinition(
            DefinitionKindV2::Value
        ))
    );
}

#[test]
fn v2_handoff_def_use_is_function_local_ordered_and_entry_seeded() {
    let base = base_fixture();
    let forward_use = InstructionV2::new(
        Some(TypedValueV2::new(ValueIdV2::new(3), I32)),
        InstructionKindV2::Binary {
            operation: BinaryOperationV2::Integer(IntegerBinaryOperationV2::Add),
            left: ValueIdV2::new(2),
            right: ValueIdV2::new(2),
        },
        evidence(&base, false),
    )
    .unwrap();
    let late_definition = constant_instruction(&base, 2, ScalarTypeV1::I32, 1);
    let forward_function = helper_with_blocks(
        &base,
        FunctionIdV2::new(20),
        "forward_use",
        vec![],
        vec![FunctionAttributeV2::NoUnwind],
        vec![BasicBlockV2::new(
            BlockIdV2::new(0),
            vec![forward_use, late_definition],
            TerminatorV2::Return(None),
        )],
    );
    assert_eq!(
        module_from_parts(&base, vec![], vec![], vec![forward_function]),
        Err(HandoffDiagnosticV2::UnsupportedInstruction)
    );

    let missing_return_value = FunctionV2::new(
        FunctionIdV2::new(21),
        "missing_return_value",
        FunctionKindV2::Helper,
        CallingConventionV2::C,
        ReturnTypeV2::Value(I32),
        vec![],
        vec![FunctionAttributeV2::NoUnwind],
        BlockIdV2::new(0),
        vec![BasicBlockV2::new(
            BlockIdV2::new(0),
            vec![],
            TerminatorV2::Return(Some(ValueIdV2::new(99))),
        )],
        evidence(&base, false),
    )
    .unwrap();
    assert_eq!(
        module_from_parts(&base, vec![], vec![], vec![missing_return_value]),
        Err(HandoffDiagnosticV2::MissingValueReference(ValueIdV2::new(
            99
        )))
    );

    let sibling_definition = constant_instruction(&base, 7, ScalarTypeV1::I32, 1);
    let sibling_use = InstructionV2::new(
        Some(TypedValueV2::new(ValueIdV2::new(8), I32)),
        InstructionKindV2::Binary {
            operation: BinaryOperationV2::Integer(IntegerBinaryOperationV2::Add),
            left: ValueIdV2::new(7),
            right: ValueIdV2::new(7),
        },
        evidence(&base, false),
    )
    .unwrap();
    let sibling_function = helper_with_blocks(
        &base,
        FunctionIdV2::new(22),
        "sibling_use",
        vec![],
        vec![FunctionAttributeV2::NoUnwind],
        vec![
            BasicBlockV2::new(
                BlockIdV2::new(0),
                vec![],
                TerminatorV2::ConditionalBranch {
                    condition: ValueIdV2::new(1),
                    then_block: BlockIdV2::new(1),
                    else_block: BlockIdV2::new(2),
                },
            ),
            BasicBlockV2::new(
                BlockIdV2::new(1),
                vec![sibling_definition],
                TerminatorV2::Branch(BlockIdV2::new(2)),
            ),
            BasicBlockV2::new(
                BlockIdV2::new(2),
                vec![sibling_use],
                TerminatorV2::Return(None),
            ),
        ],
    );
    let sibling_function = FunctionV2::new(
        sibling_function.id(),
        sibling_function.symbol(),
        sibling_function.kind(),
        sibling_function.calling_convention(),
        sibling_function.return_type(),
        vec![parameter(1, "condition", I1, vec![])],
        sibling_function.attributes().to_vec(),
        sibling_function.entry(),
        sibling_function.blocks().to_vec(),
        sibling_function.evidence().clone(),
    )
    .unwrap();
    assert_eq!(
        module_from_parts(&base, vec![], vec![], vec![sibling_function]),
        Err(HandoffDiagnosticV2::UnsupportedInstruction)
    );

    let entry_definition = constant_instruction(&base, 7, ScalarTypeV1::I32, 1);
    let entry_use = InstructionV2::new(
        Some(TypedValueV2::new(ValueIdV2::new(8), I32)),
        InstructionKindV2::Binary {
            operation: BinaryOperationV2::Integer(IntegerBinaryOperationV2::Add),
            left: ValueIdV2::new(7),
            right: ValueIdV2::new(7),
        },
        evidence(&base, false),
    )
    .unwrap();
    let entry_seeded = helper_with_blocks(
        &base,
        FunctionIdV2::new(23),
        "entry_seeded",
        vec![],
        vec![FunctionAttributeV2::NoUnwind],
        vec![
            BasicBlockV2::new(
                BlockIdV2::new(0),
                vec![entry_definition],
                TerminatorV2::Branch(BlockIdV2::new(1)),
            ),
            BasicBlockV2::new(
                BlockIdV2::new(1),
                vec![entry_use],
                TerminatorV2::Return(None),
            ),
        ],
    );
    assert!(module_from_parts(&base, vec![], vec![], vec![entry_seeded]).is_ok());
}

#[test]
fn v2_handoff_rejects_wrong_instruction_operand_result_and_terminator_types() {
    let base = base_fixture();

    assert_eq!(
        instruction_graph_error(
            &base,
            vec![
                parameter(1, "left", I32, vec![]),
                parameter(2, "right", I64, vec![])
            ],
            Some(TypedValueV2::new(ValueIdV2::new(3), I32)),
            InstructionKindV2::Binary {
                operation: BinaryOperationV2::Integer(IntegerBinaryOperationV2::Add),
                left: ValueIdV2::new(1),
                right: ValueIdV2::new(2),
            },
        ),
        HandoffDiagnosticV2::ValueTypeMismatch(ValueIdV2::new(2))
    );
    assert_eq!(
        instruction_graph_error(
            &base,
            vec![
                parameter(1, "left", I32, vec![]),
                parameter(2, "right", I32, vec![])
            ],
            Some(TypedValueV2::new(ValueIdV2::new(3), I64)),
            InstructionKindV2::Binary {
                operation: BinaryOperationV2::Integer(IntegerBinaryOperationV2::Add),
                left: ValueIdV2::new(1),
                right: ValueIdV2::new(2),
            },
        ),
        HandoffDiagnosticV2::ValueTypeMismatch(ValueIdV2::new(3))
    );
    assert_eq!(
        instruction_graph_error(
            &base,
            vec![
                parameter(1, "left", I32, vec![]),
                parameter(2, "right", I32, vec![])
            ],
            Some(TypedValueV2::new(ValueIdV2::new(3), I32)),
            InstructionKindV2::Binary {
                operation: BinaryOperationV2::Float(FloatBinaryOperationV2::Add),
                left: ValueIdV2::new(1),
                right: ValueIdV2::new(2),
            },
        ),
        HandoffDiagnosticV2::UnsupportedInstruction
    );
    assert_eq!(
        instruction_graph_error(
            &base,
            vec![parameter(1, "value", I32, vec![])],
            Some(TypedValueV2::new(ValueIdV2::new(2), I32)),
            InstructionKindV2::Compare {
                predicate: ComparePredicateV2::IntegerEqual,
                left: ValueIdV2::new(1),
                right: ValueIdV2::new(1),
            },
        ),
        HandoffDiagnosticV2::ValueTypeMismatch(ValueIdV2::new(2))
    );
    assert_eq!(
        instruction_graph_error(
            &base,
            vec![parameter(1, "value", I32, vec![])],
            Some(TypedValueV2::new(ValueIdV2::new(2), I64)),
            InstructionKindV2::Cast {
                operation: CastOperationV2::ZeroExtend,
                value: ValueIdV2::new(1),
                to: I32,
            },
        ),
        HandoffDiagnosticV2::UnsupportedInstruction
    );
    assert_eq!(
        instruction_graph_error(
            &base,
            vec![parameter(1, "scalar", I32, vec![])],
            Some(TypedValueV2::new(ValueIdV2::new(2), I32)),
            InstructionKindV2::GetElementPtr {
                base: ValueIdV2::new(1),
                indices: vec![ValueIdV2::new(1)],
            },
        ),
        HandoffDiagnosticV2::UnsupportedInstruction
    );

    let pointer = ValueTypeV2::Pointer {
        pointee: ScalarTypeV1::F32,
        address_space: AddressSpaceV1::Global,
    };
    assert_eq!(
        instruction_graph_error(
            &base,
            vec![parameter(1, "pointer", pointer, vec![])],
            Some(TypedValueV2::new(ValueIdV2::new(2), I32)),
            InstructionKindV2::Load {
                pointer: ValueIdV2::new(1),
                value_type: ScalarTypeV1::I32,
                alignment: 4,
            },
        ),
        HandoffDiagnosticV2::ValueTypeMismatch(ValueIdV2::new(1))
    );
    assert_eq!(
        instruction_graph_error(
            &base,
            vec![
                parameter(1, "pointer", pointer, vec![]),
                parameter(2, "value", I32, vec![]),
            ],
            None,
            InstructionKindV2::Store {
                pointer: ValueIdV2::new(1),
                value: ValueIdV2::new(2),
                value_type: ScalarTypeV1::F32,
                alignment: 4,
            },
        ),
        HandoffDiagnosticV2::ValueTypeMismatch(ValueIdV2::new(2))
    );

    let wrong_condition = helper_with_blocks(
        &base,
        FunctionIdV2::new(31),
        "wrong_condition",
        vec![parameter(1, "condition", I32, vec![])],
        vec![FunctionAttributeV2::NoUnwind],
        vec![BasicBlockV2::new(
            BlockIdV2::new(0),
            vec![],
            TerminatorV2::ConditionalBranch {
                condition: ValueIdV2::new(1),
                then_block: BlockIdV2::new(0),
                else_block: BlockIdV2::new(0),
            },
        )],
    );
    assert_eq!(
        module_from_parts(&base, vec![], vec![], vec![wrong_condition]),
        Err(HandoffDiagnosticV2::ValueTypeMismatch(ValueIdV2::new(1)))
    );

    let wrong_return = FunctionV2::new(
        FunctionIdV2::new(32),
        "wrong_return",
        FunctionKindV2::Helper,
        CallingConventionV2::C,
        ReturnTypeV2::Value(I32),
        vec![parameter(1, "returned", I64, vec![])],
        vec![FunctionAttributeV2::NoUnwind],
        BlockIdV2::new(0),
        vec![BasicBlockV2::new(
            BlockIdV2::new(0),
            vec![],
            TerminatorV2::Return(Some(ValueIdV2::new(1))),
        )],
        evidence(&base, false),
    )
    .unwrap();
    assert_eq!(
        module_from_parts(&base, vec![], vec![], vec![wrong_return]),
        Err(HandoffDiagnosticV2::ValueTypeMismatch(ValueIdV2::new(1)))
    );
}

#[test]
fn v2_handoff_rejects_foreign_references_and_cfg_targets() {
    let base = base_fixture();
    let missing_global = instruction_graph_error(
        &base,
        vec![],
        Some(TypedValueV2::new(
            ValueIdV2::new(1),
            ValueTypeV2::Pointer {
                pointee: ScalarTypeV1::I32,
                address_space: AddressSpaceV1::Global,
            },
        )),
        InstructionKindV2::GlobalAddress(GlobalIdV2::new(404)),
    );
    assert_eq!(
        missing_global,
        HandoffDiagnosticV2::MissingGlobalReference(GlobalIdV2::new(404))
    );

    let missing_function = instruction_graph_error(
        &base,
        vec![],
        None,
        InstructionKindV2::Call {
            target: CallTargetV2::Function(FunctionIdV2::new(404)),
            arguments: vec![],
        },
    );
    assert_eq!(
        missing_function,
        HandoffDiagnosticV2::MissingFunctionReference(FunctionIdV2::new(404))
    );

    let missing_intrinsic = instruction_graph_error(
        &base,
        vec![],
        None,
        InstructionKindV2::Call {
            target: CallTargetV2::Intrinsic(IntrinsicV2::AmdGpuBarrier),
            arguments: vec![],
        },
    );
    assert_eq!(
        missing_intrinsic,
        HandoffDiagnosticV2::MissingIntrinsicReference
    );

    let foreign_block = BlockIdV2::new(99);
    let source = helper_with_blocks(
        &base,
        FunctionIdV2::new(1),
        "source",
        vec![],
        vec![FunctionAttributeV2::NoUnwind],
        vec![BasicBlockV2::new(
            BlockIdV2::new(0),
            vec![],
            TerminatorV2::Branch(foreign_block),
        )],
    );
    let target = FunctionV2::new(
        FunctionIdV2::new(2),
        "target",
        FunctionKindV2::Helper,
        CallingConventionV2::C,
        ReturnTypeV2::Void,
        vec![],
        vec![FunctionAttributeV2::NoUnwind],
        foreign_block,
        vec![BasicBlockV2::new(
            foreign_block,
            vec![],
            TerminatorV2::Return(None),
        )],
        evidence(&base, false),
    )
    .unwrap();
    assert_eq!(
        module_from_parts(&base, vec![], vec![], vec![source, target]),
        Err(HandoffDiagnosticV2::MissingBlockReference(foreign_block))
    );

    let missing_entry = FunctionV2::new(
        FunctionIdV2::new(3),
        "missing_entry",
        FunctionKindV2::Helper,
        CallingConventionV2::C,
        ReturnTypeV2::Void,
        vec![],
        vec![FunctionAttributeV2::NoUnwind],
        BlockIdV2::new(77),
        vec![empty_block(0)],
        evidence(&base, false),
    );
    assert_eq!(
        missing_entry,
        Err(HandoffDiagnosticV2::MissingEntryBlock(BlockIdV2::new(77)))
    );
}

#[test]
fn v2_handoff_rejects_illegal_calling_conventions_and_attributes() {
    let base = base_fixture();
    let block = vec![empty_block(0)];
    let unsupported_signatures = [
        (
            FunctionKindV2::Kernel,
            CallingConventionV2::C,
            ReturnTypeV2::Void,
        ),
        (
            FunctionKindV2::Helper,
            CallingConventionV2::AmdGpuKernel,
            ReturnTypeV2::Void,
        ),
        (
            FunctionKindV2::Kernel,
            CallingConventionV2::AmdGpuKernel,
            ReturnTypeV2::Value(I32),
        ),
    ];
    for (index, (kind, convention, return_type)) in unsupported_signatures.into_iter().enumerate() {
        assert_eq!(
            FunctionV2::new(
                FunctionIdV2::new(u32::try_from(index).unwrap()),
                "unsupported_signature",
                kind,
                convention,
                return_type,
                vec![],
                vec![FunctionAttributeV2::NoUnwind],
                BlockIdV2::new(0),
                block.clone(),
                evidence(&base, false),
            ),
            Err(HandoffDiagnosticV2::UnsupportedCallingConvention)
        );
    }

    assert_eq!(
        FunctionV2::new(
            FunctionIdV2::new(10),
            "missing_nounwind",
            FunctionKindV2::Helper,
            CallingConventionV2::C,
            ReturnTypeV2::Void,
            vec![],
            vec![],
            BlockIdV2::new(0),
            block.clone(),
            evidence(&base, false),
        ),
        Err(HandoffDiagnosticV2::UnsupportedCallingConvention)
    );
    assert_eq!(
        FunctionV2::new(
            FunctionIdV2::new(11),
            "duplicate_attribute",
            FunctionKindV2::Helper,
            CallingConventionV2::C,
            ReturnTypeV2::Void,
            vec![],
            vec![FunctionAttributeV2::NoUnwind, FunctionAttributeV2::NoUnwind],
            BlockIdV2::new(0),
            block.clone(),
            evidence(&base, false),
        ),
        Err(HandoffDiagnosticV2::DuplicateDefinition(
            DefinitionKindV2::Function
        ))
    );
    assert_eq!(
        FunctionV2::new(
            FunctionIdV2::new(12),
            "conflicting_attribute",
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
            block,
            evidence(&base, false),
        ),
        Err(HandoffDiagnosticV2::ConflictingFunctionAttributes)
    );

    let pointer = ValueTypeV2::Pointer {
        pointee: ScalarTypeV1::F32,
        address_space: AddressSpaceV1::Global,
    };
    let load = InstructionV2::new(
        Some(TypedValueV2::new(ValueIdV2::new(2), F32)),
        InstructionKindV2::Load {
            pointer: ValueIdV2::new(1),
            value_type: ScalarTypeV1::F32,
            alignment: 4,
        },
        evidence(&base, false),
    )
    .unwrap();
    let read_none_load = helper_with_blocks(
        &base,
        FunctionIdV2::new(13),
        "read_none_load",
        vec![parameter(1, "pointer", pointer, vec![])],
        vec![FunctionAttributeV2::NoUnwind, FunctionAttributeV2::ReadNone],
        vec![BasicBlockV2::new(
            BlockIdV2::new(0),
            vec![load],
            TerminatorV2::Return(None),
        )],
    );
    assert_eq!(
        module_from_parts(&base, vec![], vec![], vec![read_none_load]),
        Err(HandoffDiagnosticV2::InvalidFunctionAttribute)
    );

    assert_eq!(
        FunctionParameterV2::new(
            TypedValueV2::new(ValueIdV2::new(1), I32),
            "scalar",
            vec![ParameterAttributeV1::NoAlias],
        ),
        Err(HandoffDiagnosticV2::AttributeRequiresPointer)
    );
    assert_eq!(
        FunctionParameterV2::new(
            TypedValueV2::new(ValueIdV2::new(1), pointer),
            "conflicting",
            vec![
                ParameterAttributeV1::ReadOnly,
                ParameterAttributeV1::WriteOnly
            ],
        ),
        Err(HandoffDiagnosticV2::ConflictingParameterAttributes)
    );
    assert_eq!(
        FunctionParameterV2::new(
            TypedValueV2::new(ValueIdV2::new(1), pointer),
            "duplicate_align",
            vec![
                ParameterAttributeV1::Align(4),
                ParameterAttributeV1::Align(8)
            ],
        ),
        Err(HandoffDiagnosticV2::DuplicateDefinition(
            DefinitionKindV2::Parameter
        ))
    );
    assert_eq!(
        FunctionParameterV2::new(
            TypedValueV2::new(ValueIdV2::new(1), pointer),
            "zero_dereferenceable",
            vec![ParameterAttributeV1::Dereferenceable(0)],
        ),
        Err(HandoffDiagnosticV2::InvalidParameterAttribute)
    );
}

#[test]
fn v2_handoff_bounds_alignments_and_closes_address_space_tags() {
    let base = base_fixture();
    let pointer = ValueTypeV2::Pointer {
        pointee: ScalarTypeV1::F32,
        address_space: AddressSpaceV1::Global,
    };
    for alignment in [0_u16, 3, 512] {
        assert_eq!(
            InstructionV2::new(
                Some(TypedValueV2::new(ValueIdV2::new(2), F32)),
                InstructionKindV2::Load {
                    pointer: ValueIdV2::new(1),
                    value_type: ScalarTypeV1::F32,
                    alignment,
                },
                evidence(&base, false),
            ),
            Err(HandoffDiagnosticV2::InvalidAlignment)
        );
        assert_eq!(
            FunctionParameterV2::new(
                TypedValueV2::new(ValueIdV2::new(1), pointer),
                "pointer",
                vec![ParameterAttributeV1::Align(alignment)],
            ),
            Err(HandoffDiagnosticV2::InvalidParameterAttribute)
        );
    }
    for alignment in [1_u16, 2, 4, 8, 16, 32, 64, 128, 256] {
        assert!(
            InstructionV2::new(
                Some(TypedValueV2::new(ValueIdV2::new(2), F32)),
                InstructionKindV2::Load {
                    pointer: ValueIdV2::new(1),
                    value_type: ScalarTypeV1::F32,
                    alignment,
                },
                evidence(&base, false),
            )
            .is_ok()
        );
        assert!(
            FunctionParameterV2::new(
                TypedValueV2::new(ValueIdV2::new(1), pointer),
                "pointer",
                vec![ParameterAttributeV1::Align(alignment)],
            )
            .is_ok()
        );
    }

    let address_spaces = [
        AddressSpaceV1::Flat,
        AddressSpaceV1::Global,
        AddressSpaceV1::Region,
        AddressSpaceV1::Local,
        AddressSpaceV1::Constant,
        AddressSpaceV1::Private,
    ];
    let globals = address_spaces
        .into_iter()
        .enumerate()
        .map(|(index, address_space)| {
            external_global(
                &base,
                GlobalIdV2::new(u32::try_from(index + 1).unwrap()),
                &format!("address_space_{index}"),
                address_space,
            )
        })
        .collect::<Vec<_>>();
    let normal = valid_module(&base, false, 0x3f80_0000);
    let all_spaces = module_from_parts(
        &base,
        globals,
        normal.intrinsics().to_vec(),
        normal.functions().to_vec(),
    )
    .unwrap();
    let handoff = Gfx942HandoffV2::new(base.clone(), all_spaces).unwrap();
    let decoded = Gfx942HandoffV2::decode_canonical(handoff.encode_canonical().as_bytes()).unwrap();
    assert_eq!(
        decoded
            .module()
            .globals()
            .iter()
            .map(GlobalV2::address_space)
            .collect::<Vec<_>>(),
        address_spaces
    );

    let canonical = handoff_fixture(false, 0x3f80_0000).encode_canonical();
    let offsets = wire_offsets(canonical.as_bytes());
    let mut unknown_address_space = canonical.as_bytes().to_vec();
    unknown_address_space[offsets.first_global_address_space] = 0xff;
    assert_eq!(
        Gfx942HandoffV2::decode_canonical(&unknown_address_space),
        Err(DecodeHandoffErrorV2::UnknownTag {
            section: WireSectionV2::AddressSpace,
            tag: 0xff,
        })
    );
}

#[test]
fn v2_handoff_checks_origin_and_obligation_references_on_nested_evidence() {
    let base = base_fixture();
    let obligation = base.obligations()[0].identity();
    assert_eq!(
        EvidenceV2::new(base.origins()[0].identity(), vec![obligation, obligation],),
        Err(HandoffDiagnosticV2::DuplicateDefinition(
            DefinitionKindV2::Obligation
        ))
    );

    let normal = valid_module(&base, false, 0x3f80_0000);
    let alien_origin = OriginV1::new(OriginKindV1::AmdgcnIr, identity(0xee), None);
    let replacement_global = GlobalV2::new(
        GlobalIdV2::new(10),
        "seed",
        GlobalLinkageV2::Internal,
        AddressSpaceV1::Constant,
        false,
        ScalarTypeV1::F32,
        Some(ScalarConstantV2::new(ScalarTypeV1::F32, 0x3f80_0000).unwrap()),
        EvidenceV2::new(alien_origin.identity(), vec![]).unwrap(),
    )
    .unwrap();
    let globals = normal
        .globals()
        .iter()
        .map(|global| {
            if global.id() == GlobalIdV2::new(10) {
                replacement_global.clone()
            } else {
                global.clone()
            }
        })
        .collect();
    let alien_origin_module = module_from_parts(
        &base,
        globals,
        normal.intrinsics().to_vec(),
        normal.functions().to_vec(),
    )
    .unwrap();
    assert_eq!(
        Gfx942HandoffV2::new(base.clone(), alien_origin_module),
        Err(HandoffDiagnosticV2::MissingOriginReference)
    );

    let alien_obligation = ObligationV1::new(
        ObligationKindV1::MaintainOriginCoverage,
        identity(0xef),
        base.origins()[0].identity(),
    );
    let replacement_intrinsic = IntrinsicReferenceV2::new(
        IntrinsicV2::SqrtF32,
        EvidenceV2::new(
            base.origins()[0].identity(),
            vec![alien_obligation.identity()],
        )
        .unwrap(),
    );
    let intrinsics = normal
        .intrinsics()
        .iter()
        .map(|intrinsic| {
            if intrinsic.intrinsic() == IntrinsicV2::SqrtF32 {
                replacement_intrinsic.clone()
            } else {
                intrinsic.clone()
            }
        })
        .collect();
    let alien_obligation_module = module_from_parts(
        &base,
        normal.globals().to_vec(),
        intrinsics,
        normal.functions().to_vec(),
    )
    .unwrap();
    assert_eq!(
        Gfx942HandoffV2::new(base, alien_obligation_module),
        Err(HandoffDiagnosticV2::MissingObligationReference)
    );
}

#[test]
fn v2_handoff_wire_rejects_unknown_intrinsics_opcodes_and_missing_terminator() {
    let canonical = handoff_fixture(false, 0x3f80_0000).encode_canonical();
    let offsets = wire_offsets(canonical.as_bytes());

    let mut unknown_intrinsic = canonical.as_bytes().to_vec();
    unknown_intrinsic[offsets.first_intrinsic] = 0xff;
    assert_eq!(
        Gfx942HandoffV2::decode_canonical(&unknown_intrinsic),
        Err(DecodeHandoffErrorV2::UnknownTag {
            section: WireSectionV2::Intrinsic,
            tag: 0xff,
        })
    );

    let mut unknown_instruction = canonical.as_bytes().to_vec();
    unknown_instruction[offsets.first_instruction] = 0xff;
    assert_eq!(
        Gfx942HandoffV2::decode_canonical(&unknown_instruction),
        Err(DecodeHandoffErrorV2::UnknownTag {
            section: WireSectionV2::Instruction,
            tag: 0xff,
        })
    );

    let mut missing_terminator = canonical.as_bytes()[..canonical.len() - 1].to_vec();
    let shortened_len = u32::try_from(missing_terminator.len()).unwrap();
    missing_terminator[12..16].copy_from_slice(&shortened_len.to_le_bytes());
    assert_eq!(
        Gfx942HandoffV2::decode_canonical(&missing_terminator),
        Err(DecodeHandoffErrorV2::Truncated {
            offset: missing_terminator.len(),
        })
    );
}

#[test]
fn v2_handoff_public_schema_records_supported_families_and_remaining_gaps() {
    let instructions = [
        InstructionKindV2::Constant(ScalarConstantV2::new(ScalarTypeV1::I32, 0).unwrap()),
        InstructionKindV2::VectorZero {
            element_type: ScalarTypeV1::I16,
        },
        InstructionKindV2::GlobalAddress(GlobalIdV2::new(1)),
        InstructionKindV2::Binary {
            operation: BinaryOperationV2::Integer(IntegerBinaryOperationV2::Add),
            left: ValueIdV2::new(1),
            right: ValueIdV2::new(2),
        },
        InstructionKindV2::Compare {
            predicate: ComparePredicateV2::IntegerEqual,
            left: ValueIdV2::new(1),
            right: ValueIdV2::new(2),
        },
        InstructionKindV2::Cast {
            operation: CastOperationV2::ZeroExtend,
            value: ValueIdV2::new(1),
            to: I64,
        },
        InstructionKindV2::GetElementPtr {
            base: ValueIdV2::new(1),
            indices: vec![ValueIdV2::new(2)],
        },
        InstructionKindV2::Load {
            pointer: ValueIdV2::new(1),
            value_type: ScalarTypeV1::I32,
            alignment: 4,
        },
        InstructionKindV2::VectorLoad4 {
            pointer: ValueIdV2::new(1),
            element_type: ScalarTypeV1::I16,
            alignment: 8,
        },
        InstructionKindV2::Store {
            pointer: ValueIdV2::new(1),
            value: ValueIdV2::new(2),
            value_type: ScalarTypeV1::I32,
            alignment: 4,
        },
        InstructionKindV2::Call {
            target: CallTargetV2::Intrinsic(IntrinsicV2::AmdGpuBarrier),
            arguments: vec![],
        },
        InstructionKindV2::Phi {
            incoming: vec![(ValueIdV2::new(1), BlockIdV2::new(0))],
        },
        InstructionKindV2::InsertElement {
            vector: ValueIdV2::new(1),
            element: ValueIdV2::new(2),
            index: ValueIdV2::new(3),
        },
        InstructionKindV2::ExtractElement {
            vector: ValueIdV2::new(1),
            index: ValueIdV2::new(2),
        },
    ];
    assert_eq!(
        instructions
            .iter()
            .map(public_instruction_family)
            .collect::<Vec<_>>(),
        [
            "constant",
            "vector-zero",
            "global-address",
            "binary",
            "compare",
            "cast",
            "getelementptr",
            "load",
            "vector-load4",
            "store",
            "call",
            "phi",
            "insert-element",
            "extract-element",
        ]
    );

    let intrinsics = [
        IntrinsicV2::AmdGpuWorkitemId(AxisV2::X),
        IntrinsicV2::AmdGpuWorkgroupId(AxisV2::Y),
        IntrinsicV2::AmdGpuBarrier,
        IntrinsicV2::FmaF32,
        IntrinsicV2::SqrtF32,
        IntrinsicV2::Trap,
        IntrinsicV2::AmdGpuMfmaF32_16x16x16Bf16_1k,
    ];
    assert_eq!(
        intrinsics.map(public_intrinsic_family),
        [
            "workitem-id",
            "workgroup-id",
            "barrier",
            "fma-f32",
            "sqrt-f32",
            "trap",
            "amdgcn-mfma-f32-16x16x16bf16-1k",
        ]
    );
    assert_eq!(
        [
            public_value_shape(I32),
            public_value_shape(ValueTypeV2::Vector {
                element: ScalarTypeV1::I16,
                lanes: 4,
            }),
            public_value_shape(ValueTypeV2::Pointer {
                pointee: ScalarTypeV1::I32,
                address_space: AddressSpaceV1::Global,
            }),
            public_value_shape(ValueTypeV2::ArrayPointer {
                element: ScalarTypeV1::I16,
                elements: 256,
                address_space: AddressSpaceV1::Local,
            }),
        ],
        ["scalar", "vector", "pointer", "array-pointer"]
    );

    assert_eq!(
        PUBLIC_V2_UNREPRESENTABLE_GAPS,
        [
            "arbitrary intrinsic declarations and calls",
            "atomic instructions and memory orderings",
            "switch terminators and cases",
            "aggregate values and operations",
        ]
    );
}

fn identity(byte: u8) -> IdentityV1 {
    IdentityV1::new([byte; 32]).unwrap()
}

fn base_fixture() -> Gfx942HandoffV1 {
    let origin = OriginV1::new(OriginKindV1::AmdgcnIr, identity(0x31), None);
    let attributes = vec![
        ParameterAttributeV1::NoCapture,
        ParameterAttributeV1::NonNull,
        ParameterAttributeV1::Align(4),
        ParameterAttributeV1::Dereferenceable(64),
    ];
    let kernel = KernelEntryV1::new(
        "v2_kernel",
        vec![
            KernelParameterV1::new(
                "output",
                KernelValueTypeV1::Pointer {
                    pointee: ScalarTypeV1::F32,
                    address_space: AddressSpaceV1::Global,
                },
                attributes,
            )
            .unwrap(),
        ],
        FunctionAttributeV1::gfx942_kernel_defaults(WorkgroupSizeRangeV1::new(64, 256).unwrap()),
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
            ObligationKindV1::MaintainOriginCoverage,
            identity(0x42),
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

fn evidence(base: &Gfx942HandoffV1, reversed: bool) -> EvidenceV2 {
    let mut obligations = base
        .obligations()
        .iter()
        .map(|obligation| obligation.identity())
        .collect::<Vec<_>>();
    if reversed {
        obligations.reverse();
    }
    EvidenceV2::new(base.origins()[0].identity(), obligations).unwrap()
}

fn valid_module(base: &Gfx942HandoffV1, reversed: bool, seed_bits: u64) -> ExecutableModuleV2 {
    let seed = GlobalV2::new(
        GlobalIdV2::new(10),
        "seed",
        GlobalLinkageV2::Internal,
        AddressSpaceV1::Constant,
        false,
        ScalarTypeV1::F32,
        Some(ScalarConstantV2::new(ScalarTypeV1::F32, seed_bits).unwrap()),
        evidence(base, reversed),
    )
    .unwrap();
    let external = external_global(
        base,
        GlobalIdV2::new(40),
        "external_counter",
        AddressSpaceV1::Global,
    );
    let mut globals = vec![seed, external];
    let mut intrinsics = vec![
        IntrinsicReferenceV2::new(IntrinsicV2::SqrtF32, evidence(base, reversed)),
        IntrinsicReferenceV2::new(
            IntrinsicV2::AmdGpuWorkitemId(AxisV2::X),
            evidence(base, reversed),
        ),
    ];
    let mut functions = vec![
        helper_fixture(base, reversed),
        kernel_fixture(base, reversed),
    ];
    let mut flags = base.module().flags().to_vec();
    let mut named = base.module().named_metadata().to_vec();
    if reversed {
        globals.reverse();
        intrinsics.reverse();
        functions.reverse();
        flags.reverse();
        named.reverse();
    }
    ExecutableModuleV2::new(flags, named, globals, intrinsics, functions).unwrap()
}

fn handoff_fixture(reversed: bool, seed_bits: u64) -> Gfx942HandoffV2 {
    let base = base_fixture();
    let module = valid_module(&base, reversed, seed_bits);
    Gfx942HandoffV2::new(base, module).unwrap()
}

fn helper_fixture(base: &Gfx942HandoffV1, reversed: bool) -> FunctionV2 {
    let call = InstructionV2::new(
        Some(TypedValueV2::new(ValueIdV2::new(2), F32)),
        InstructionKindV2::Call {
            target: CallTargetV2::Intrinsic(IntrinsicV2::SqrtF32),
            arguments: vec![ValueIdV2::new(1)],
        },
        evidence(base, reversed),
    )
    .unwrap();
    let mut attributes = vec![
        FunctionAttributeV2::NoUnwind,
        FunctionAttributeV2::AlwaysInline,
        FunctionAttributeV2::ReadNone,
    ];
    if reversed {
        attributes.reverse();
    }
    FunctionV2::new(
        FunctionIdV2::new(20),
        "sqrt_helper",
        FunctionKindV2::Helper,
        CallingConventionV2::C,
        ReturnTypeV2::Value(F32),
        vec![parameter(1, "value", F32, vec![])],
        attributes,
        BlockIdV2::new(0),
        vec![BasicBlockV2::new(
            BlockIdV2::new(0),
            vec![call],
            TerminatorV2::Return(Some(ValueIdV2::new(2))),
        )],
        evidence(base, reversed),
    )
    .unwrap()
}

fn kernel_fixture(base: &Gfx942HandoffV1, reversed: bool) -> FunctionV2 {
    let pointer = ValueTypeV2::Pointer {
        pointee: ScalarTypeV1::F32,
        address_space: AddressSpaceV1::Global,
    };
    let constant = constant_instruction(base, 2, ScalarTypeV1::F32, 0);
    let store = InstructionV2::new(
        None,
        InstructionKindV2::Store {
            pointer: ValueIdV2::new(1),
            value: ValueIdV2::new(2),
            value_type: ScalarTypeV1::F32,
            alignment: 4,
        },
        evidence(base, reversed),
    )
    .unwrap();
    let v1 = &base.kernels()[0];
    let mut attributes = v1
        .function_attributes()
        .iter()
        .copied()
        .map(FunctionAttributeV2::from)
        .collect::<Vec<_>>();
    if reversed {
        attributes.reverse();
    }
    FunctionV2::new(
        FunctionIdV2::new(30),
        "v2_kernel",
        FunctionKindV2::Kernel,
        CallingConventionV2::AmdGpuKernel,
        ReturnTypeV2::Void,
        vec![parameter(
            1,
            "output",
            pointer,
            v1.parameters()[0].attributes().to_vec(),
        )],
        attributes,
        BlockIdV2::new(0),
        vec![BasicBlockV2::new(
            BlockIdV2::new(0),
            vec![constant, store],
            TerminatorV2::Return(None),
        )],
        evidence(base, reversed),
    )
    .unwrap()
}

fn external_global(
    base: &Gfx942HandoffV1,
    id: GlobalIdV2,
    symbol: &str,
    address_space: AddressSpaceV1,
) -> GlobalV2 {
    GlobalV2::new(
        id,
        symbol,
        GlobalLinkageV2::External,
        address_space,
        true,
        ScalarTypeV1::I32,
        None,
        evidence(base, false),
    )
    .unwrap()
}

fn parameter(
    id: u32,
    name: &str,
    value_type: ValueTypeV2,
    attributes: Vec<ParameterAttributeV1>,
) -> FunctionParameterV2 {
    FunctionParameterV2::new(
        TypedValueV2::new(ValueIdV2::new(id), value_type),
        name,
        attributes,
    )
    .unwrap()
}

fn empty_block(id: u32) -> BasicBlockV2 {
    BasicBlockV2::new(BlockIdV2::new(id), vec![], TerminatorV2::Return(None))
}

fn empty_helper(base: &Gfx942HandoffV1, id: FunctionIdV2, symbol: &str) -> FunctionV2 {
    helper_with_blocks(
        base,
        id,
        symbol,
        vec![],
        vec![FunctionAttributeV2::NoUnwind],
        vec![empty_block(0)],
    )
}

fn helper_with_blocks(
    base: &Gfx942HandoffV1,
    id: FunctionIdV2,
    symbol: &str,
    parameters: Vec<FunctionParameterV2>,
    attributes: Vec<FunctionAttributeV2>,
    blocks: Vec<BasicBlockV2>,
) -> FunctionV2 {
    FunctionV2::new(
        id,
        symbol,
        FunctionKindV2::Helper,
        CallingConventionV2::C,
        ReturnTypeV2::Void,
        parameters,
        attributes,
        BlockIdV2::new(0),
        blocks,
        evidence(base, false),
    )
    .unwrap()
}

fn function_with_parameters(
    base: &Gfx942HandoffV1,
    parameters: Vec<FunctionParameterV2>,
) -> Result<FunctionV2, HandoffDiagnosticV2> {
    FunctionV2::new(
        FunctionIdV2::new(10),
        "parameters",
        FunctionKindV2::Helper,
        CallingConventionV2::C,
        ReturnTypeV2::Void,
        parameters,
        vec![FunctionAttributeV2::NoUnwind],
        BlockIdV2::new(0),
        vec![empty_block(0)],
        evidence(base, false),
    )
}

fn constant_instruction(
    base: &Gfx942HandoffV1,
    id: u32,
    scalar_type: ScalarTypeV1,
    bits: u64,
) -> InstructionV2 {
    InstructionV2::new(
        Some(TypedValueV2::new(
            ValueIdV2::new(id),
            ValueTypeV2::Scalar(scalar_type),
        )),
        InstructionKindV2::Constant(ScalarConstantV2::new(scalar_type, bits).unwrap()),
        evidence(base, false),
    )
    .unwrap()
}

fn module_from_parts(
    base: &Gfx942HandoffV1,
    globals: Vec<GlobalV2>,
    intrinsics: Vec<IntrinsicReferenceV2>,
    functions: Vec<FunctionV2>,
) -> Result<ExecutableModuleV2, HandoffDiagnosticV2> {
    ExecutableModuleV2::new(
        base.module().flags().to_vec(),
        base.module().named_metadata().to_vec(),
        globals,
        intrinsics,
        functions,
    )
}

fn instruction_graph_error(
    base: &Gfx942HandoffV1,
    parameters: Vec<FunctionParameterV2>,
    result: Option<TypedValueV2>,
    kind: InstructionKindV2,
) -> HandoffDiagnosticV2 {
    let instruction = InstructionV2::new(result, kind, evidence(base, false)).unwrap();
    let function = helper_with_blocks(
        base,
        FunctionIdV2::new(50),
        "hostile_instruction",
        parameters,
        vec![FunctionAttributeV2::NoUnwind],
        vec![BasicBlockV2::new(
            BlockIdV2::new(0),
            vec![instruction],
            TerminatorV2::Unreachable,
        )],
    );
    module_from_parts(base, vec![], vec![], vec![function]).unwrap_err()
}

fn public_instruction_family(instruction: &InstructionKindV2) -> &'static str {
    match instruction {
        InstructionKindV2::Constant(_) => "constant",
        InstructionKindV2::VectorZero { .. } => "vector-zero",
        InstructionKindV2::GlobalAddress(_) => "global-address",
        InstructionKindV2::Binary { .. } => "binary",
        InstructionKindV2::Compare { .. } => "compare",
        InstructionKindV2::Cast { .. } => "cast",
        InstructionKindV2::GetElementPtr { .. } => "getelementptr",
        InstructionKindV2::Load { .. } => "load",
        InstructionKindV2::VectorLoad4 { .. } => "vector-load4",
        InstructionKindV2::Store { .. } => "store",
        InstructionKindV2::Call { .. } => "call",
        InstructionKindV2::Phi { .. } => "phi",
        InstructionKindV2::InsertElement { .. } => "insert-element",
        InstructionKindV2::ExtractElement { .. } => "extract-element",
    }
}

fn public_intrinsic_family(intrinsic: IntrinsicV2) -> &'static str {
    match intrinsic {
        IntrinsicV2::AmdGpuWorkitemId(_) => "workitem-id",
        IntrinsicV2::AmdGpuWorkgroupId(_) => "workgroup-id",
        IntrinsicV2::AmdGpuBarrier => "barrier",
        IntrinsicV2::FmaF32 => "fma-f32",
        IntrinsicV2::SqrtF32 => "sqrt-f32",
        IntrinsicV2::Trap => "trap",
        IntrinsicV2::AmdGpuMfmaF32_16x16x16Bf16_1k => "amdgcn-mfma-f32-16x16x16bf16-1k",
    }
}

fn public_value_shape(value_type: ValueTypeV2) -> &'static str {
    match value_type {
        ValueTypeV2::Scalar(_) => "scalar",
        ValueTypeV2::Vector { .. } => "vector",
        ValueTypeV2::Pointer { .. } => "pointer",
        ValueTypeV2::ArrayPointer { .. } => "array-pointer",
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct WireOffsets {
    first_global_address_space: usize,
    first_intrinsic: usize,
    first_instruction: usize,
}

fn wire_offsets(bytes: &[u8]) -> WireOffsets {
    let base_len = u32::from_le_bytes(bytes[16..20].try_into().unwrap()) as usize;
    let mut cursor = Cursor::new(bytes, 20 + base_len + 32);

    let flag_count = cursor.u8() as usize;
    cursor.take(flag_count);
    let metadata_count = cursor.u8() as usize;
    for _ in 0..metadata_count {
        if cursor.u8() == 3 {
            cursor.take(32);
        }
    }

    let global_count = cursor.u16() as usize;
    let mut first_global_address_space = None;
    for _ in 0..global_count {
        cursor.take(4);
        cursor.string();
        cursor.take(1);
        first_global_address_space.get_or_insert(cursor.take(1));
        cursor.take(2);
        if cursor.u8() == 1 {
            cursor.take(9);
        }
        cursor.evidence();
    }

    let intrinsic_count = cursor.u16() as usize;
    let mut first_intrinsic = None;
    for _ in 0..intrinsic_count {
        let position = cursor.take(1);
        first_intrinsic.get_or_insert(position);
        if matches!(bytes[position], 1 | 2) {
            cursor.take(1);
        }
        cursor.evidence();
    }

    let function_count = cursor.u16() as usize;
    assert!(function_count > 0);
    cursor.take(4);
    cursor.string();
    cursor.take(2);
    if cursor.u8() == 2 {
        cursor.value_type();
    }
    cursor.evidence();
    let parameter_count = cursor.u16() as usize;
    for _ in 0..parameter_count {
        cursor.take(4);
        cursor.value_type();
        cursor.string();
        let attribute_count = cursor.u8() as usize;
        for _ in 0..attribute_count {
            match cursor.u8() {
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
        match cursor.u8() {
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
    assert!(block_count > 0);
    cursor.take(4);
    let instruction_count = cursor.u32() as usize;
    assert!(instruction_count > 0);
    if cursor.u8() == 1 {
        cursor.take(4);
        cursor.value_type();
    }
    let first_instruction = cursor.take(1);

    WireOffsets {
        first_global_address_space: first_global_address_space.unwrap(),
        first_intrinsic: first_intrinsic.unwrap(),
        first_instruction,
    }
}

struct Cursor<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl<'a> Cursor<'a> {
    const fn new(bytes: &'a [u8], offset: usize) -> Self {
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

    fn evidence(&mut self) {
        self.take(32);
        let obligation_count = self.u8() as usize;
        self.take(obligation_count * 32);
    }

    fn value_type(&mut self) {
        let tag = self.u8();
        self.take(1);
        if tag == 2 {
            self.take(1);
        }
    }
}
