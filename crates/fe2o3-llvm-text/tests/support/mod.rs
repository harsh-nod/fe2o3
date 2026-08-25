use fe2o3_llvm_handoff::{
    AddressSpaceV1, AxisV2, BasicBlockV2, BinaryOperationV2, BlockIdV2, CallTargetV2,
    CallingConventionV2, CastOperationV2, ComparePredicateV2, DeviceLibraryInputV1, EvidenceV2,
    ExecutableModuleV2, FloatBinaryOperationV2, FunctionAttributeV1, FunctionAttributeV2,
    FunctionIdV2, FunctionKindV2, FunctionParameterV2, FunctionV2, Gfx942HandoffInputV1,
    Gfx942HandoffV1, Gfx942HandoffV2, Gfx942TargetPolicyV1, GlobalIdV2, GlobalLinkageV2, GlobalV2,
    HandoffDiagnosticV2, IdentityV1, InstructionKindV2, InstructionV2, IntrinsicReferenceV2,
    IntrinsicV2, KernelEntryV1, KernelParameterV1, KernelValueTypeV1, ModuleFlagV1,
    ModuleMetadataV1, NamedMetadataV1, ObligationKindV1, ObligationV1, OriginKindV1, OriginV1,
    ParameterAttributeV1, ReturnTypeV2, ScalarConstantV2, ScalarTypeV1, SourceSpanV1,
    StageIdentitiesV1, TerminatorV2, TypedValueV2, ValueIdV2, ValueTypeV2, WorkgroupSizeRangeV1,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Hostile {
    None,
    MultiIndexGep,
    UnreachableBlock,
    EntryPredecessor,
    KernelCall,
    ReservedSymbol,
}

fn identity(byte: u8) -> IdentityV1 {
    IdentityV1::new([byte; 32]).unwrap()
}

fn kernel_attributes() -> Vec<FunctionAttributeV1> {
    FunctionAttributeV1::gfx942_kernel_defaults(WorkgroupSizeRangeV1::new(64, 256).unwrap())
}

pub fn base_fixture() -> Gfx942HandoffV1 {
    let origin = OriginV1::new(
        OriginKindV1::AmdgcnIr,
        identity(0x31),
        Some(SourceSpanV1::new("kernel.rs", 1, 1, 9, 2).unwrap()),
    );
    let kernel = KernelEntryV1::new(
        "write_scaled",
        vec![
            KernelParameterV1::new(
                "output",
                KernelValueTypeV1::Pointer {
                    pointee: ScalarTypeV1::F32,
                    address_space: AddressSpaceV1::Global,
                },
                vec![
                    ParameterAttributeV1::NoAlias,
                    ParameterAttributeV1::NoCapture,
                    ParameterAttributeV1::NonNull,
                    ParameterAttributeV1::WriteOnly,
                    ParameterAttributeV1::Align(4),
                    ParameterAttributeV1::Dereferenceable(4_096),
                ],
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
    let obligations = [
        (ObligationKindV1::PreserveKernelAbi, 0x41),
        (ObligationKindV1::PreserveCallingConvention, 0x42),
        (ObligationKindV1::MaintainOriginCoverage, 0x43),
    ]
    .into_iter()
    .map(|(kind, byte)| ObligationV1::new(kind, identity(byte), origin.identity()))
    .collect();

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
                NamedMetadataV1::OpenClSpirVersion2_0,
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

pub fn evidence(base: &Gfx942HandoffV1, permuted: bool) -> EvidenceV2 {
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

pub fn instruction(
    base: &Gfx942HandoffV1,
    result: Option<TypedValueV2>,
    kind: InstructionKindV2,
    permuted: bool,
) -> InstructionV2 {
    InstructionV2::new(result, kind, evidence(base, permuted)).unwrap()
}

fn helper_function(base: &Gfx942HandoffV1, permuted: bool, reserved: bool) -> FunctionV2 {
    let f32_type = ValueTypeV2::Scalar(ScalarTypeV1::F32);
    let instructions = vec![
        instruction(
            base,
            Some(TypedValueV2::new(ValueIdV2::new(2), f32_type)),
            InstructionKindV2::Constant(
                ScalarConstantV2::new(ScalarTypeV1::F32, 0x3f00_0000).unwrap(),
            ),
            permuted,
        ),
        instruction(
            base,
            Some(TypedValueV2::new(ValueIdV2::new(3), f32_type)),
            InstructionKindV2::Binary {
                operation: BinaryOperationV2::Float(FloatBinaryOperationV2::Multiply),
                left: ValueIdV2::new(1),
                right: ValueIdV2::new(2),
            },
            permuted,
        ),
        instruction(
            base,
            Some(TypedValueV2::new(ValueIdV2::new(4), f32_type)),
            InstructionKindV2::Call {
                target: CallTargetV2::Intrinsic(IntrinsicV2::SqrtF32),
                arguments: vec![ValueIdV2::new(3)],
            },
            permuted,
        ),
    ];
    let mut attributes = vec![
        FunctionAttributeV2::NoUnwind,
        FunctionAttributeV2::AlwaysInline,
        FunctionAttributeV2::ReadNone,
        FunctionAttributeV2::WillReturn,
    ];
    if permuted {
        attributes.reverse();
    }
    FunctionV2::new(
        FunctionIdV2::new(2),
        if reserved {
            "llvm.user_helper"
        } else {
            "scale"
        },
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
            TerminatorV2::Return(Some(ValueIdV2::new(4))),
        )],
        evidence(base, permuted),
    )
    .unwrap()
}

fn kernel_function(base: &Gfx942HandoffV1, permuted: bool, hostile: Hostile) -> FunctionV2 {
    let v1 = &base.kernels()[0];
    let output_pointer = ValueTypeV2::Pointer {
        pointee: ScalarTypeV1::F32,
        address_space: AddressSpaceV1::Global,
    };
    let constant_pointer = ValueTypeV2::Pointer {
        pointee: ScalarTypeV1::F32,
        address_space: AddressSpaceV1::Constant,
    };
    let i1_type = ValueTypeV2::Scalar(ScalarTypeV1::I1);
    let i32_type = ValueTypeV2::Scalar(ScalarTypeV1::I32);
    let i64_type = ValueTypeV2::Scalar(ScalarTypeV1::I64);
    let f32_type = ValueTypeV2::Scalar(ScalarTypeV1::F32);
    let parameters = vec![
        FunctionParameterV2::new(
            TypedValueV2::new(ValueIdV2::new(1), output_pointer),
            "output",
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
    let mut entry_instructions = vec![
        instruction(
            base,
            Some(TypedValueV2::new(ValueIdV2::new(8), constant_pointer)),
            InstructionKindV2::GlobalAddress(GlobalIdV2::new(1)),
            permuted,
        ),
        instruction(
            base,
            Some(TypedValueV2::new(ValueIdV2::new(9), f32_type)),
            InstructionKindV2::Load {
                pointer: ValueIdV2::new(8),
                value_type: ScalarTypeV1::F32,
                alignment: 4,
            },
            permuted,
        ),
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
            Some(TypedValueV2::new(ValueIdV2::new(12), i1_type)),
            InstructionKindV2::Compare {
                predicate: ComparePredicateV2::UnsignedLessThan,
                left: ValueIdV2::new(11),
                right: ValueIdV2::new(2),
            },
            permuted,
        ),
    ];
    if hostile == Hostile::KernelCall {
        entry_instructions.push(instruction(
            base,
            None,
            InstructionKindV2::Call {
                target: CallTargetV2::Function(FunctionIdV2::new(10)),
                arguments: vec![ValueIdV2::new(1), ValueIdV2::new(2)],
            },
            permuted,
        ));
    }
    let entry = BasicBlockV2::new(
        BlockIdV2::new(0),
        entry_instructions,
        TerminatorV2::ConditionalBranch {
            condition: ValueIdV2::new(12),
            then_block: BlockIdV2::new(1),
            else_block: BlockIdV2::new(2),
        },
    );
    let indices = if hostile == Hostile::MultiIndexGep {
        vec![ValueIdV2::new(11), ValueIdV2::new(11)]
    } else {
        vec![ValueIdV2::new(11)]
    };
    let body = BasicBlockV2::new(
        BlockIdV2::new(1),
        vec![
            instruction(
                base,
                Some(TypedValueV2::new(ValueIdV2::new(13), output_pointer)),
                InstructionKindV2::GetElementPtr {
                    base: ValueIdV2::new(1),
                    indices,
                },
                permuted,
            ),
            instruction(
                base,
                Some(TypedValueV2::new(ValueIdV2::new(14), f32_type)),
                InstructionKindV2::Call {
                    target: CallTargetV2::Function(FunctionIdV2::new(2)),
                    arguments: vec![ValueIdV2::new(9)],
                },
                permuted,
            ),
            instruction(
                base,
                None,
                InstructionKindV2::Store {
                    pointer: ValueIdV2::new(13),
                    value: ValueIdV2::new(14),
                    value_type: ScalarTypeV1::F32,
                    alignment: 4,
                },
                permuted,
            ),
            instruction(
                base,
                None,
                InstructionKindV2::Call {
                    target: CallTargetV2::Intrinsic(IntrinsicV2::AmdGpuBarrier),
                    arguments: vec![],
                },
                permuted,
            ),
        ],
        TerminatorV2::Branch(BlockIdV2::new(2)),
    );
    let exit = BasicBlockV2::new(
        BlockIdV2::new(2),
        vec![],
        if hostile == Hostile::EntryPredecessor {
            TerminatorV2::Branch(BlockIdV2::new(0))
        } else {
            TerminatorV2::Return(None)
        },
    );
    let mut blocks = vec![entry, body, exit];
    if hostile == Hostile::UnreachableBlock {
        blocks.push(BasicBlockV2::new(
            BlockIdV2::new(99),
            vec![],
            TerminatorV2::Unreachable,
        ));
    }
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
        "write_scaled",
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

pub fn module_fixture(
    base: &Gfx942HandoffV1,
    permuted: bool,
    hostile: Hostile,
) -> ExecutableModuleV2 {
    try_module_fixture(base, permuted, hostile).unwrap()
}

pub fn try_module_fixture(
    base: &Gfx942HandoffV1,
    permuted: bool,
    hostile: Hostile,
) -> Result<ExecutableModuleV2, HandoffDiagnosticV2> {
    let global = GlobalV2::new(
        GlobalIdV2::new(1),
        "factor",
        GlobalLinkageV2::Internal,
        AddressSpaceV1::Constant,
        false,
        ScalarTypeV1::F32,
        Some(ScalarConstantV2::new(ScalarTypeV1::F32, 0x4080_0000).unwrap()),
        evidence(base, permuted),
    )
    .unwrap();
    let external = GlobalV2::new(
        GlobalIdV2::new(2),
        "counter",
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
        IntrinsicReferenceV2::new(IntrinsicV2::AmdGpuBarrier, evidence(base, permuted)),
        IntrinsicReferenceV2::new(IntrinsicV2::SqrtF32, evidence(base, permuted)),
    ];
    let mut functions = vec![
        helper_function(base, permuted, hostile == Hostile::ReservedSymbol),
        kernel_function(base, permuted, hostile),
    ];
    let mut globals = vec![global, external];
    let mut flags = base.module().flags().to_vec();
    let mut named = base.module().named_metadata().to_vec();
    if permuted {
        globals.reverse();
        intrinsics.reverse();
        functions.reverse();
        flags.reverse();
        named.reverse();
    }
    ExecutableModuleV2::new(flags, named, globals, intrinsics, functions)
}

pub fn fixture(permuted: bool, hostile: Hostile) -> Gfx942HandoffV2 {
    let base = base_fixture();
    let module = module_fixture(&base, permuted, hostile);
    Gfx942HandoffV2::new(base, module).unwrap()
}

pub fn bad_global_address_helper(base: &Gfx942HandoffV1) -> FunctionV2 {
    let wrong_pointer = ValueTypeV2::Pointer {
        pointee: ScalarTypeV1::F32,
        address_space: AddressSpaceV1::Global,
    };
    FunctionV2::new(
        FunctionIdV2::new(2),
        "scale",
        FunctionKindV2::Helper,
        CallingConventionV2::C,
        ReturnTypeV2::Value(wrong_pointer),
        vec![],
        vec![FunctionAttributeV2::NoUnwind],
        BlockIdV2::new(0),
        vec![BasicBlockV2::new(
            BlockIdV2::new(0),
            vec![instruction(
                base,
                Some(TypedValueV2::new(ValueIdV2::new(1), wrong_pointer)),
                InstructionKindV2::GlobalAddress(GlobalIdV2::new(1)),
                false,
            )],
            TerminatorV2::Return(Some(ValueIdV2::new(1))),
        )],
        evidence(base, false),
    )
    .unwrap()
}
