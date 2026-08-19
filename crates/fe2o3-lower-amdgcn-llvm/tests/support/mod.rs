#![allow(dead_code)]

use fe2o3_llvm_handoff::*;

pub fn scalar_handoff() -> Gfx942HandoffV2 {
    build_handoff(Fixture::Scalar, OriginKindV1::AmdgcnIr, None)
}

pub fn scalar_handoff_permuted() -> Gfx942HandoffV2 {
    build_handoff(Fixture::ScalarPermuted, OriginKindV1::AmdgcnIr, None)
}

pub fn gemm_control_flow_handoff() -> Gfx942HandoffV2 {
    build_handoff(Fixture::GemmControlFlow, OriginKindV1::KernelIr, None)
}

pub fn handoff_with_named_metadata(metadata: NamedMetadataV1) -> Gfx942HandoffV2 {
    build_handoff(Fixture::Scalar, OriginKindV1::AmdgcnIr, Some(metadata))
}

pub fn handoff_with_origin(kind: OriginKindV1) -> Gfx942HandoffV2 {
    build_handoff(Fixture::Scalar, kind, None)
}

pub fn handoff_missing_obligation(missing: ObligationKindV1) -> Gfx942HandoffV2 {
    build_handoff_with(Fixture::Scalar, OriginKindV1::AmdgcnIr, None, Some(missing))
}

pub fn handoff_with_f64_parameter() -> Gfx942HandoffV2 {
    let origin = OriginV1::new(OriginKindV1::AmdgcnIr, identity(41), None);
    let obligations = obligations(&origin, None);
    let evidence = evidence(&origin, &obligations);
    let flags = canonical_flags(false);
    let base = base_handoff(
        &origin,
        &obligations,
        &flags,
        &[],
        vec![kernel_parameter(
            "value",
            KernelValueTypeV1::Scalar(ScalarTypeV1::F64),
        )],
        "unsupported_f64",
    );
    let parameter = FunctionParameterV2::new(
        TypedValueV2::new(ValueIdV2::new(1), ValueTypeV2::Scalar(ScalarTypeV1::F64)),
        "value",
        vec![],
    )
    .unwrap();
    let function = FunctionV2::new(
        FunctionIdV2::new(1),
        "unsupported_f64",
        FunctionKindV2::Kernel,
        CallingConventionV2::AmdGpuKernel,
        ReturnTypeV2::Void,
        vec![parameter],
        function_attributes(),
        BlockIdV2::new(1),
        vec![BasicBlockV2::new(
            BlockIdV2::new(1),
            vec![],
            TerminatorV2::Return(None),
        )],
        evidence,
    )
    .unwrap();
    finish_handoff(base, flags, vec![], vec![function])
}

pub fn handoff_with_helper_c_calling_convention() -> Gfx942HandoffV2 {
    let canonical = scalar_parts(Fixture::Scalar, OriginKindV1::AmdgcnIr, None, None);
    let helper = FunctionV2::new(
        FunctionIdV2::new(2),
        "helper_c",
        FunctionKindV2::Helper,
        CallingConventionV2::C,
        ReturnTypeV2::Void,
        vec![],
        vec![FunctionAttributeV2::NoUnwind],
        BlockIdV2::new(1),
        vec![BasicBlockV2::new(
            BlockIdV2::new(1),
            vec![],
            TerminatorV2::Return(None),
        )],
        canonical.evidence.clone(),
    )
    .unwrap();
    finish_handoff(
        canonical.base,
        canonical.flags,
        canonical.named_metadata,
        vec![canonical.function, helper],
    )
}

pub fn handoff_with_global() -> Gfx942HandoffV2 {
    let canonical = scalar_parts(Fixture::Scalar, OriginKindV1::AmdgcnIr, None, None);
    let global = GlobalV2::new_lds_bf16_array_256(
        GlobalIdV2::new(1),
        "tile_lds",
        canonical.evidence.clone(),
    )
    .unwrap();
    let module = ExecutableModuleV2::new(
        canonical.flags,
        canonical.named_metadata,
        vec![global],
        vec![],
        vec![canonical.function],
    )
    .unwrap();
    Gfx942HandoffV2::new(canonical.base, module).unwrap()
}

pub fn tiled_data_handoff() -> Gfx942HandoffV2 {
    let canonical = scalar_parts(Fixture::Scalar, OriginKindV1::AmdgcnIr, None, None);
    let source = &canonical.function;
    let mut instructions = source.blocks()[0].instructions().to_vec();
    let i64_zero = instruction(
        Some(TypedValueV2::new(
            ValueIdV2::new(6),
            ValueTypeV2::Scalar(ScalarTypeV1::I64),
        )),
        InstructionKindV2::Constant(ScalarConstantV2::new(ScalarTypeV1::I64, 0).unwrap()),
        &canonical.evidence,
    );
    let i32_zero = instruction(
        Some(TypedValueV2::new(
            ValueIdV2::new(7),
            ValueTypeV2::Scalar(ScalarTypeV1::I32),
        )),
        InstructionKindV2::Constant(ScalarConstantV2::new(ScalarTypeV1::I32, 0).unwrap()),
        &canonical.evidence,
    );
    let i16_zero = instruction(
        Some(TypedValueV2::new(
            ValueIdV2::new(8),
            ValueTypeV2::Scalar(ScalarTypeV1::I16),
        )),
        InstructionKindV2::Constant(ScalarConstantV2::new(ScalarTypeV1::I16, 0).unwrap()),
        &canonical.evidence,
    );
    let lds_array = ValueTypeV2::ArrayPointer {
        element: ScalarTypeV1::I16,
        elements: 256,
        address_space: AddressSpaceV1::Local,
    };
    let local_i16 = ValueTypeV2::Pointer {
        pointee: ScalarTypeV1::I16,
        address_space: AddressSpaceV1::Local,
    };
    let i16x4 = ValueTypeV2::Vector {
        element: ScalarTypeV1::I16,
        lanes: 4,
    };
    instructions.extend([
        i64_zero,
        i32_zero,
        i16_zero,
        instruction(
            Some(TypedValueV2::new(ValueIdV2::new(9), lds_array)),
            InstructionKindV2::GlobalAddress(GlobalIdV2::new(1)),
            &canonical.evidence,
        ),
        instruction(
            Some(TypedValueV2::new(ValueIdV2::new(10), local_i16)),
            InstructionKindV2::GetElementPtr {
                base: ValueIdV2::new(9),
                indices: vec![ValueIdV2::new(6), ValueIdV2::new(6)],
            },
            &canonical.evidence,
        ),
        instruction(
            Some(TypedValueV2::new(ValueIdV2::new(11), i16x4)),
            InstructionKindV2::VectorZero {
                element_type: ScalarTypeV1::I16,
            },
            &canonical.evidence,
        ),
        instruction(
            Some(TypedValueV2::new(ValueIdV2::new(12), i16x4)),
            InstructionKindV2::InsertElement {
                vector: ValueIdV2::new(11),
                element: ValueIdV2::new(8),
                index: ValueIdV2::new(7),
            },
            &canonical.evidence,
        ),
        instruction(
            Some(TypedValueV2::new(ValueIdV2::new(13), i16x4)),
            InstructionKindV2::VectorLoad4 {
                pointer: ValueIdV2::new(10),
                element_type: ScalarTypeV1::I16,
                alignment: 8,
            },
            &canonical.evidence,
        ),
        instruction(
            Some(TypedValueV2::new(
                ValueIdV2::new(14),
                ValueTypeV2::Scalar(ScalarTypeV1::I16),
            )),
            InstructionKindV2::ExtractElement {
                vector: ValueIdV2::new(13),
                index: ValueIdV2::new(7),
            },
            &canonical.evidence,
        ),
        instruction(
            None,
            InstructionKindV2::Store {
                pointer: ValueIdV2::new(10),
                value: ValueIdV2::new(14),
                value_type: ScalarTypeV1::I16,
                alignment: 2,
            },
            &canonical.evidence,
        ),
    ]);
    let function = FunctionV2::new(
        source.id(),
        source.symbol(),
        source.kind(),
        source.calling_convention(),
        source.return_type(),
        source.parameters().to_vec(),
        source.attributes().to_vec(),
        source.entry(),
        vec![BasicBlockV2::new(
            source.entry(),
            instructions,
            TerminatorV2::Return(None),
        )],
        canonical.evidence.clone(),
    )
    .unwrap();
    let globals = vec![
        GlobalV2::new_lds_bf16_array_256(
            GlobalIdV2::new(1),
            "tile_lds",
            canonical.evidence.clone(),
        )
        .unwrap(),
        GlobalV2::new_private_constant_bytes(
            GlobalIdV2::new(2),
            "kernel_descriptor",
            KERNEL_DESCRIPTOR_SECTION_V2,
            vec![1, 2, 3, 4],
            4,
            canonical.evidence.clone(),
        )
        .unwrap(),
    ];
    let module = ExecutableModuleV2::new(
        canonical.flags,
        canonical.named_metadata,
        globals,
        vec![],
        vec![function],
    )
    .unwrap();
    Gfx942HandoffV2::new(canonical.base, module).unwrap()
}

pub fn handoff_with_scalar_global() -> Gfx942HandoffV2 {
    let canonical = scalar_parts(Fixture::Scalar, OriginKindV1::AmdgcnIr, None, None);
    let global = GlobalV2::new(
        GlobalIdV2::new(1),
        "unsupported_scalar",
        GlobalLinkageV2::Internal,
        AddressSpaceV1::Constant,
        false,
        ScalarTypeV1::I32,
        Some(ScalarConstantV2::new(ScalarTypeV1::I32, 0).unwrap()),
        canonical.evidence.clone(),
    )
    .unwrap();
    let module = ExecutableModuleV2::new(
        canonical.flags,
        canonical.named_metadata,
        vec![global],
        vec![],
        vec![canonical.function],
    )
    .unwrap();
    Gfx942HandoffV2::new(canonical.base, module).unwrap()
}

pub fn handoff_with_required_workgroup_size(shape: [u16; 3]) -> Gfx942HandoffV2 {
    let canonical = scalar_parts(Fixture::Scalar, OriginKindV1::AmdgcnIr, None, None);
    let source = &canonical.function;
    let mut attributes = source.attributes().to_vec();
    attributes.push(FunctionAttributeV2::RequiredWorkgroupSize(shape));
    let function = FunctionV2::new(
        source.id(),
        source.symbol(),
        source.kind(),
        source.calling_convention(),
        source.return_type(),
        source.parameters().to_vec(),
        attributes,
        source.entry(),
        source.blocks().to_vec(),
        source.evidence().clone(),
    )
    .unwrap();
    finish_handoff(
        canonical.base,
        canonical.flags,
        canonical.named_metadata,
        vec![function],
    )
}

#[derive(Clone, Copy)]
enum Fixture {
    Scalar,
    ScalarPermuted,
    GemmControlFlow,
}

fn build_handoff(
    fixture: Fixture,
    origin_kind: OriginKindV1,
    metadata: Option<NamedMetadataV1>,
) -> Gfx942HandoffV2 {
    build_handoff_with(fixture, origin_kind, metadata, None)
}

fn build_handoff_with(
    fixture: Fixture,
    origin_kind: OriginKindV1,
    metadata: Option<NamedMetadataV1>,
    missing: Option<ObligationKindV1>,
) -> Gfx942HandoffV2 {
    match fixture {
        Fixture::Scalar | Fixture::ScalarPermuted => {
            let parts = scalar_parts(fixture, origin_kind, metadata, missing);
            finish_handoff(
                parts.base,
                parts.flags,
                parts.named_metadata,
                vec![parts.function],
            )
        }
        Fixture::GemmControlFlow => gemm_parts(origin_kind, metadata, missing),
    }
}

struct ScalarParts {
    base: Gfx942HandoffV1,
    flags: Vec<ModuleFlagV1>,
    named_metadata: Vec<NamedMetadataV1>,
    function: FunctionV2,
    evidence: EvidenceV2,
}

fn scalar_parts(
    fixture: Fixture,
    origin_kind: OriginKindV1,
    metadata: Option<NamedMetadataV1>,
    missing: Option<ObligationKindV1>,
) -> ScalarParts {
    let origin = OriginV1::new(origin_kind, identity(31), None);
    let obligations = obligations(&origin, missing);
    let evidence = evidence(&origin, &obligations);
    let permuted = matches!(fixture, Fixture::ScalarPermuted);
    let flags = canonical_flags(permuted);
    let mut named_metadata = metadata.into_iter().collect::<Vec<_>>();
    if permuted {
        named_metadata.reverse();
    }
    let parameters_v1 = vec![
        kernel_parameter(
            "input",
            KernelValueTypeV1::Pointer {
                pointee: ScalarTypeV1::F32,
                address_space: AddressSpaceV1::Global,
            },
        ),
        kernel_parameter(
            "output",
            KernelValueTypeV1::Pointer {
                pointee: ScalarTypeV1::F32,
                address_space: AddressSpaceV1::Global,
            },
        ),
        kernel_parameter("addend", KernelValueTypeV1::Scalar(ScalarTypeV1::F32)),
    ];
    let base = base_handoff(
        &origin,
        &obligations,
        &flags,
        &named_metadata,
        parameters_v1,
        "scalar_add_v1",
    );
    let parameters = vec![
        function_parameter(
            1,
            "input",
            ValueTypeV2::Pointer {
                pointee: ScalarTypeV1::F32,
                address_space: AddressSpaceV1::Global,
            },
        ),
        function_parameter(
            2,
            "output",
            ValueTypeV2::Pointer {
                pointee: ScalarTypeV1::F32,
                address_space: AddressSpaceV1::Global,
            },
        ),
        function_parameter(3, "addend", ValueTypeV2::Scalar(ScalarTypeV1::F32)),
    ];
    let load = instruction(
        Some(TypedValueV2::new(
            ValueIdV2::new(4),
            ValueTypeV2::Scalar(ScalarTypeV1::F32),
        )),
        InstructionKindV2::Load {
            pointer: ValueIdV2::new(1),
            value_type: ScalarTypeV1::F32,
            alignment: 4,
        },
        &evidence,
    );
    let add = instruction(
        Some(TypedValueV2::new(
            ValueIdV2::new(5),
            ValueTypeV2::Scalar(ScalarTypeV1::F32),
        )),
        InstructionKindV2::Binary {
            operation: BinaryOperationV2::Float(FloatBinaryOperationV2::Add),
            left: ValueIdV2::new(4),
            right: ValueIdV2::new(3),
        },
        &evidence,
    );
    let store = instruction(
        None,
        InstructionKindV2::Store {
            pointer: ValueIdV2::new(2),
            value: ValueIdV2::new(5),
            value_type: ScalarTypeV1::F32,
            alignment: 4,
        },
        &evidence,
    );
    let function = FunctionV2::new(
        FunctionIdV2::new(1),
        "scalar_add_v1",
        FunctionKindV2::Kernel,
        CallingConventionV2::AmdGpuKernel,
        ReturnTypeV2::Void,
        parameters,
        function_attributes(),
        BlockIdV2::new(1),
        vec![BasicBlockV2::new(
            BlockIdV2::new(1),
            vec![load, add, store],
            TerminatorV2::Return(None),
        )],
        evidence.clone(),
    )
    .unwrap();
    ScalarParts {
        base,
        flags,
        named_metadata,
        function,
        evidence,
    }
}

fn gemm_parts(
    origin_kind: OriginKindV1,
    metadata: Option<NamedMetadataV1>,
    missing: Option<ObligationKindV1>,
) -> Gfx942HandoffV2 {
    let origin = OriginV1::new(origin_kind, identity(51), None);
    let obligations = obligations(&origin, missing);
    let evidence = evidence(&origin, &obligations);
    let flags = canonical_flags(false);
    let named_metadata = metadata.into_iter().collect::<Vec<_>>();
    let base = base_handoff(
        &origin,
        &obligations,
        &flags,
        &named_metadata,
        vec![
            kernel_parameter(
                "output",
                KernelValueTypeV1::Pointer {
                    pointee: ScalarTypeV1::F32,
                    address_space: AddressSpaceV1::Global,
                },
            ),
            kernel_parameter("active", KernelValueTypeV1::Scalar(ScalarTypeV1::I1)),
            kernel_parameter("lhs", KernelValueTypeV1::Scalar(ScalarTypeV1::F32)),
            kernel_parameter("rhs", KernelValueTypeV1::Scalar(ScalarTypeV1::F32)),
        ],
        "scalar_gemm_core_v1",
    );
    let parameters = vec![
        function_parameter(
            1,
            "output",
            ValueTypeV2::Pointer {
                pointee: ScalarTypeV1::F32,
                address_space: AddressSpaceV1::Global,
            },
        ),
        function_parameter(2, "active", ValueTypeV2::Scalar(ScalarTypeV1::I1)),
        function_parameter(3, "lhs", ValueTypeV2::Scalar(ScalarTypeV1::F32)),
        function_parameter(4, "rhs", ValueTypeV2::Scalar(ScalarTypeV1::F32)),
    ];
    let multiply = instruction(
        Some(TypedValueV2::new(
            ValueIdV2::new(5),
            ValueTypeV2::Scalar(ScalarTypeV1::F32),
        )),
        InstructionKindV2::Binary {
            operation: BinaryOperationV2::Float(FloatBinaryOperationV2::Multiply),
            left: ValueIdV2::new(3),
            right: ValueIdV2::new(4),
        },
        &evidence,
    );
    let add = instruction(
        Some(TypedValueV2::new(
            ValueIdV2::new(6),
            ValueTypeV2::Scalar(ScalarTypeV1::F32),
        )),
        InstructionKindV2::Binary {
            operation: BinaryOperationV2::Float(FloatBinaryOperationV2::Add),
            left: ValueIdV2::new(3),
            right: ValueIdV2::new(4),
        },
        &evidence,
    );
    let phi = instruction(
        Some(TypedValueV2::new(
            ValueIdV2::new(7),
            ValueTypeV2::Scalar(ScalarTypeV1::F32),
        )),
        InstructionKindV2::Phi {
            incoming: vec![
                (ValueIdV2::new(5), BlockIdV2::new(2)),
                (ValueIdV2::new(6), BlockIdV2::new(3)),
            ],
        },
        &evidence,
    );
    let store = instruction(
        None,
        InstructionKindV2::Store {
            pointer: ValueIdV2::new(1),
            value: ValueIdV2::new(7),
            value_type: ScalarTypeV1::F32,
            alignment: 4,
        },
        &evidence,
    );
    let blocks = vec![
        BasicBlockV2::new(
            BlockIdV2::new(4),
            vec![phi, store],
            TerminatorV2::Return(None),
        ),
        BasicBlockV2::new(
            BlockIdV2::new(2),
            vec![multiply],
            TerminatorV2::Branch(BlockIdV2::new(4)),
        ),
        BasicBlockV2::new(
            BlockIdV2::new(1),
            vec![],
            TerminatorV2::ConditionalBranch {
                condition: ValueIdV2::new(2),
                then_block: BlockIdV2::new(2),
                else_block: BlockIdV2::new(3),
            },
        ),
        BasicBlockV2::new(
            BlockIdV2::new(3),
            vec![add],
            TerminatorV2::Branch(BlockIdV2::new(4)),
        ),
    ];
    let function = FunctionV2::new(
        FunctionIdV2::new(1),
        "scalar_gemm_core_v1",
        FunctionKindV2::Kernel,
        CallingConventionV2::AmdGpuKernel,
        ReturnTypeV2::Void,
        parameters,
        function_attributes(),
        BlockIdV2::new(1),
        blocks,
        evidence,
    )
    .unwrap();
    finish_handoff(base, flags, named_metadata, vec![function])
}

fn finish_handoff(
    base: Gfx942HandoffV1,
    flags: Vec<ModuleFlagV1>,
    named_metadata: Vec<NamedMetadataV1>,
    functions: Vec<FunctionV2>,
) -> Gfx942HandoffV2 {
    let module = ExecutableModuleV2::new(flags, named_metadata, vec![], vec![], functions).unwrap();
    Gfx942HandoffV2::new(base, module).unwrap()
}

fn base_handoff(
    origin: &OriginV1,
    obligations: &[ObligationV1],
    flags: &[ModuleFlagV1],
    named_metadata: &[NamedMetadataV1],
    parameters: Vec<KernelParameterV1>,
    symbol: &str,
) -> Gfx942HandoffV1 {
    let kernel = KernelEntryV1::new(
        symbol,
        parameters,
        function_attributes_v1(),
        origin.identity(),
    )
    .unwrap();
    Gfx942HandoffV1::new(Gfx942HandoffInputV1 {
        stage_identities: StageIdentitiesV1::new([11; 32], [12; 32], [13; 32]).unwrap(),
        target: Gfx942TargetPolicyV1::canonical(),
        kernels: vec![kernel],
        module: ModuleMetadataV1::new(flags.to_vec(), named_metadata.to_vec(), vec![]).unwrap(),
        origins: vec![origin.clone()],
        obligations: obligations.to_vec(),
    })
    .unwrap()
}

fn obligations(origin: &OriginV1, missing: Option<ObligationKindV1>) -> Vec<ObligationV1> {
    [
        ObligationKindV1::PreserveKernelAbi,
        ObligationKindV1::PreserveAddressSpaces,
        ObligationKindV1::PreserveTargetFeatures,
        ObligationKindV1::PreserveCallingConvention,
        ObligationKindV1::PreserveFunctionAttributes,
        ObligationKindV1::PreserveModuleMetadata,
        ObligationKindV1::MaintainOriginCoverage,
    ]
    .into_iter()
    .filter(|kind| Some(*kind) != missing)
    .map(|kind| ObligationV1::new(kind, identity(kind as u8 + 61), origin.identity()))
    .collect()
}

fn evidence(origin: &OriginV1, obligations: &[ObligationV1]) -> EvidenceV2 {
    EvidenceV2::new(
        origin.identity(),
        obligations.iter().map(|value| value.identity()).collect(),
    )
    .unwrap()
}

fn instruction(
    result: Option<TypedValueV2>,
    kind: InstructionKindV2,
    evidence: &EvidenceV2,
) -> InstructionV2 {
    InstructionV2::new(result, kind, evidence.clone()).unwrap()
}

fn kernel_parameter(name: &str, value_type: KernelValueTypeV1) -> KernelParameterV1 {
    KernelParameterV1::new(name, value_type, vec![]).unwrap()
}

fn function_parameter(id: u32, name: &str, value_type: ValueTypeV2) -> FunctionParameterV2 {
    FunctionParameterV2::new(
        TypedValueV2::new(ValueIdV2::new(id), value_type),
        name,
        vec![],
    )
    .unwrap()
}

fn function_attributes_v1() -> Vec<FunctionAttributeV1> {
    FunctionAttributeV1::gfx942_kernel_defaults(WorkgroupSizeRangeV1::new(1, 64).unwrap())
}

fn function_attributes() -> Vec<FunctionAttributeV2> {
    function_attributes_v1()
        .into_iter()
        .map(FunctionAttributeV2::from)
        .collect()
}

fn canonical_flags(permuted: bool) -> Vec<ModuleFlagV1> {
    let mut flags = vec![ModuleFlagV1::CodeObjectVersion6, ModuleFlagV1::PicLevel2];
    if permuted {
        flags.reverse();
    }
    flags
}

fn identity(byte: u8) -> IdentityV1 {
    IdentityV1::new([byte; 32]).unwrap()
}
