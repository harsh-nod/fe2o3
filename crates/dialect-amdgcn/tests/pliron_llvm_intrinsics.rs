use dialect_amdgcn::{
    AmdgcnPlironLlvmProfileV1, AmdgcnPlironLlvmRejectionV1, UnsupportedInstructionV1,
    admit_amdgcn_pliron_llvm_v1,
};
use fe2o3_llvm_handoff::{
    AxisV2, BasicBlockV2, BlockIdV2, CallTargetV2, CallingConventionV2, EvidenceV2,
    ExecutableModuleV2, FunctionAttributeV1, FunctionAttributeV2, FunctionIdV2, FunctionKindV2,
    FunctionV2, Gfx942HandoffInputV1, Gfx942HandoffV1, Gfx942HandoffV2, Gfx942TargetPolicyV1,
    HandoffDiagnosticV2, IdentityV1, InstructionKindV2, InstructionV2, IntrinsicReferenceV2,
    IntrinsicV2, KernelEntryV1, ModuleFlagV1, ModuleMetadataV1, ObligationKindV1, ObligationV1,
    OriginKindV1, OriginV1, ReturnTypeV2, ScalarConstantV2, ScalarTypeV1, StageIdentitiesV1,
    TerminatorV2, TypedValueV2, ValueIdV2, ValueTypeV2, WorkgroupSizeRangeV1,
};

const ALL_INTRINSICS: [IntrinsicV2; 11] = [
    IntrinsicV2::AmdGpuWorkitemId(AxisV2::X),
    IntrinsicV2::AmdGpuWorkitemId(AxisV2::Y),
    IntrinsicV2::AmdGpuWorkitemId(AxisV2::Z),
    IntrinsicV2::AmdGpuWorkgroupId(AxisV2::X),
    IntrinsicV2::AmdGpuWorkgroupId(AxisV2::Y),
    IntrinsicV2::AmdGpuWorkgroupId(AxisV2::Z),
    IntrinsicV2::AmdGpuBarrier,
    IntrinsicV2::FmaF32,
    IntrinsicV2::SqrtF32,
    IntrinsicV2::Trap,
    IntrinsicV2::AmdGpuMfmaF32_16x16x16Bf16_1k,
];

struct Fixture {
    base: Gfx942HandoffV1,
    flags: Vec<ModuleFlagV1>,
    evidence: EvidenceV2,
}

#[test]
fn admits_exact_intrinsic_declaration_and_use_closure() {
    let fixture = fixture();
    let function = intrinsic_kernel(&fixture, None);
    let declarations = ALL_INTRINSICS
        .into_iter()
        .map(|intrinsic| IntrinsicReferenceV2::new(intrinsic, fixture.evidence.clone()))
        .collect();
    let module =
        ExecutableModuleV2::new(fixture.flags, vec![], vec![], declarations, vec![function])
            .unwrap();
    let handoff = Gfx942HandoffV2::new(fixture.base, module).unwrap();

    let admitted = admit_amdgcn_pliron_llvm_v1(&handoff).unwrap();
    assert_eq!(
        admitted.profile(),
        AmdgcnPlironLlvmProfileV1::VectorAndLocalMemory
    );
    assert_eq!(admitted.handoff().module().intrinsics().len(), 11);
}

#[test]
fn undeclared_intrinsic_use_is_rejected_by_canonical_handoff_validation() {
    let fixture = fixture();
    let function = intrinsic_kernel(
        &fixture,
        Some(vec![call(
            &fixture.evidence,
            Some(value(1, ValueTypeV2::Scalar(ScalarTypeV1::I32))),
            IntrinsicV2::AmdGpuWorkitemId(AxisV2::X),
            vec![],
        )]),
    );

    assert_eq!(
        ExecutableModuleV2::new(fixture.flags, vec![], vec![], vec![], vec![function],),
        Err(HandoffDiagnosticV2::MissingIntrinsicReference)
    );
}

#[test]
fn malformed_intrinsic_signature_is_rejected_by_canonical_handoff_validation() {
    let fixture = fixture();
    let function = intrinsic_kernel(
        &fixture,
        Some(vec![call(
            &fixture.evidence,
            Some(value(1, ValueTypeV2::Scalar(ScalarTypeV1::F32))),
            IntrinsicV2::FmaF32,
            vec![],
        )]),
    );
    let declarations = vec![IntrinsicReferenceV2::new(
        IntrinsicV2::FmaF32,
        fixture.evidence.clone(),
    )];

    assert_eq!(
        ExecutableModuleV2::new(fixture.flags, vec![], vec![], declarations, vec![function],),
        Err(HandoffDiagnosticV2::UnsupportedInstruction)
    );
}

#[test]
fn direct_function_calls_remain_outside_the_bounded_lane() {
    let fixture = fixture();
    let kernel = intrinsic_kernel(
        &fixture,
        Some(vec![
            InstructionV2::new(
                None,
                InstructionKindV2::Call {
                    target: CallTargetV2::Function(FunctionIdV2::new(2)),
                    arguments: vec![],
                },
                fixture.evidence.clone(),
            )
            .unwrap(),
        ]),
    );
    let helper = FunctionV2::new(
        FunctionIdV2::new(2),
        "helper",
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
        fixture.evidence.clone(),
    )
    .unwrap();
    let module =
        ExecutableModuleV2::new(fixture.flags, vec![], vec![], vec![], vec![kernel, helper])
            .unwrap();
    let handoff = Gfx942HandoffV2::new(fixture.base, module).unwrap();

    assert_eq!(
        admit_amdgcn_pliron_llvm_v1(&handoff).unwrap_err(),
        AmdgcnPlironLlvmRejectionV1::UnsupportedInstruction(UnsupportedInstructionV1::Call)
    );
}

fn intrinsic_kernel(fixture: &Fixture, replacement: Option<Vec<InstructionV2>>) -> FunctionV2 {
    let instructions = replacement.unwrap_or_else(|| all_intrinsic_calls(&fixture.evidence));
    FunctionV2::new(
        FunctionIdV2::new(1),
        "intrinsic_kernel",
        FunctionKindV2::Kernel,
        CallingConventionV2::AmdGpuKernel,
        ReturnTypeV2::Void,
        vec![],
        kernel_attributes_v1()
            .into_iter()
            .map(FunctionAttributeV2::from)
            .collect(),
        BlockIdV2::new(1),
        vec![BasicBlockV2::new(
            BlockIdV2::new(1),
            instructions,
            TerminatorV2::Return(None),
        )],
        fixture.evidence.clone(),
    )
    .unwrap()
}

fn all_intrinsic_calls(evidence: &EvidenceV2) -> Vec<InstructionV2> {
    let i32_type = ValueTypeV2::Scalar(ScalarTypeV1::I32);
    let f32_type = ValueTypeV2::Scalar(ScalarTypeV1::F32);
    let i16x4 = ValueTypeV2::fixed_vector(ScalarTypeV1::I16, 4).unwrap();
    let f32x4 = ValueTypeV2::fixed_vector(ScalarTypeV1::F32, 4).unwrap();
    let mut instructions = vec![
        instruction(
            evidence,
            Some(value(1, i32_type)),
            InstructionKindV2::Constant(ScalarConstantV2::new(ScalarTypeV1::I32, 0).unwrap()),
        ),
        instruction(
            evidence,
            Some(value(2, f32_type)),
            InstructionKindV2::Constant(ScalarConstantV2::new(ScalarTypeV1::F32, 0).unwrap()),
        ),
        instruction(
            evidence,
            Some(value(3, i16x4)),
            InstructionKindV2::VectorZero {
                element_type: ScalarTypeV1::I16,
            },
        ),
        instruction(
            evidence,
            Some(value(4, i16x4)),
            InstructionKindV2::VectorZero {
                element_type: ScalarTypeV1::I16,
            },
        ),
        instruction(
            evidence,
            Some(value(5, f32x4)),
            InstructionKindV2::VectorZero {
                element_type: ScalarTypeV1::F32,
            },
        ),
    ];
    for (result, intrinsic) in (6..12).zip(ALL_INTRINSICS[..6].iter().copied()) {
        instructions.push(call(
            evidence,
            Some(value(result, i32_type)),
            intrinsic,
            vec![],
        ));
    }
    instructions.extend([
        call(evidence, None, IntrinsicV2::AmdGpuBarrier, vec![]),
        call(
            evidence,
            Some(value(12, f32_type)),
            IntrinsicV2::FmaF32,
            vec![ValueIdV2::new(2), ValueIdV2::new(2), ValueIdV2::new(2)],
        ),
        call(
            evidence,
            Some(value(13, f32_type)),
            IntrinsicV2::SqrtF32,
            vec![ValueIdV2::new(12)],
        ),
        call(evidence, None, IntrinsicV2::Trap, vec![]),
        call(
            evidence,
            Some(value(14, f32x4)),
            IntrinsicV2::AmdGpuMfmaF32_16x16x16Bf16_1k,
            vec![
                ValueIdV2::new(3),
                ValueIdV2::new(4),
                ValueIdV2::new(5),
                ValueIdV2::new(1),
                ValueIdV2::new(1),
                ValueIdV2::new(1),
            ],
        ),
    ]);
    instructions
}

fn call(
    evidence: &EvidenceV2,
    result: Option<TypedValueV2>,
    intrinsic: IntrinsicV2,
    arguments: Vec<ValueIdV2>,
) -> InstructionV2 {
    instruction(
        evidence,
        result,
        InstructionKindV2::Call {
            target: CallTargetV2::Intrinsic(intrinsic),
            arguments,
        },
    )
}

fn instruction(
    evidence: &EvidenceV2,
    result: Option<TypedValueV2>,
    kind: InstructionKindV2,
) -> InstructionV2 {
    InstructionV2::new(result, kind, evidence.clone()).unwrap()
}

fn value(id: u32, value_type: ValueTypeV2) -> TypedValueV2 {
    TypedValueV2::new(ValueIdV2::new(id), value_type)
}

fn fixture() -> Fixture {
    let origin = OriginV1::new(OriginKindV1::AmdgcnIr, identity(31), None);
    let obligations = [
        ObligationKindV1::PreserveKernelAbi,
        ObligationKindV1::PreserveAddressSpaces,
        ObligationKindV1::PreserveTargetFeatures,
        ObligationKindV1::PreserveCallingConvention,
        ObligationKindV1::PreserveFunctionAttributes,
        ObligationKindV1::PreserveModuleMetadata,
        ObligationKindV1::MaintainOriginCoverage,
    ]
    .into_iter()
    .enumerate()
    .map(|(index, kind)| ObligationV1::new(kind, identity(index as u8 + 41), origin.identity()))
    .collect::<Vec<_>>();
    let evidence = EvidenceV2::new(
        origin.identity(),
        obligations
            .iter()
            .map(|obligation| obligation.identity())
            .collect(),
    )
    .unwrap();
    let flags = vec![ModuleFlagV1::CodeObjectVersion6, ModuleFlagV1::PicLevel2];
    let kernel = KernelEntryV1::new(
        "intrinsic_kernel",
        vec![],
        kernel_attributes_v1(),
        origin.identity(),
    )
    .unwrap();
    let base = Gfx942HandoffV1::new(Gfx942HandoffInputV1 {
        stage_identities: StageIdentitiesV1::new([11; 32], [12; 32], [13; 32]).unwrap(),
        target: Gfx942TargetPolicyV1::canonical(),
        kernels: vec![kernel],
        module: ModuleMetadataV1::new(flags.clone(), vec![], vec![]).unwrap(),
        origins: vec![origin],
        obligations,
    })
    .unwrap();
    Fixture {
        base,
        flags,
        evidence,
    }
}

fn kernel_attributes_v1() -> Vec<FunctionAttributeV1> {
    FunctionAttributeV1::gfx942_kernel_defaults(WorkgroupSizeRangeV1::new(1, 64).unwrap())
}

fn identity(byte: u8) -> IdentityV1 {
    IdentityV1::new([byte; 32]).unwrap()
}
