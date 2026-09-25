//! Test-owned inert semantic graphs. No rustc/HIR or numerical authority.
use super::*;

const UNIT: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(0);
const CONTEXT: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(1);
const CONTEXT_REF: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(2);
const LANE: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(3);
const LANE_REF: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(4);
const A: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(5);
const B: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(6);
const ACC: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(7);
const F32: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(8);
const VALUES: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(9);
const VIEW_A: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(10);
const VIEW_A_REF: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(11);
const VIEW_B: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(12);
const VIEW_B_REF: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(13);
const U64: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(14);

#[derive(Clone, Copy, Debug)]
pub(super) enum Case {
    Identity,
    Swap01,
    CopyRootFragment,
    DuplicateReturnedComponent,
    ConditionalHelper,
    UnreachableHelperBlock,
    CyclicHelper,
}
impl Case {
    pub(super) fn permutation(self) -> [u8; 4] {
        match self {
            Self::Swap01 => [1, 0, 2, 3],
            Self::DuplicateReturnedComponent => [0, 0, 2, 3],
            _ => [0, 1, 2, 3],
        }
    }
}
fn provenance() -> SemanticSourceProvenanceV1 {
    SemanticSourceProvenanceV1::unavailable()
}
fn opaque(tag: u8, size: u64, align: u64) -> SemanticTypeDeclV1 {
    SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256([tag; 32]),
        SemanticLayoutIdentityV1::from_sha256([tag; 32]),
        SemanticTypeLayoutV1::new(Some(size), align).unwrap(),
        SemanticTypeShapeV1::Opaque,
    )
}
fn reference(tag: u8, pointee: SemanticTypeIdV1, size: u64, align: u64) -> SemanticTypeDeclV1 {
    SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256([tag; 32]),
        SemanticLayoutIdentityV1::from_sha256([tag; 32]),
        SemanticTypeLayoutV1::new_with_backend_repr(
            Some(8),
            8,
            SemanticBackendReprV1::scalar(SemanticBackendScalarV1::initialized(
                SemanticBackendPrimitiveV1::pointer(0, 8, 8),
                SemanticScalarValidityRangeV1::new(1, u64::MAX.into()),
            )),
            false,
        )
        .unwrap(),
        SemanticTypeShapeV1::Pointer(
            SemanticPointerTypeV1::new_with_kind(
                pointee,
                SemanticPointerKindV1::Reference,
                SemanticMutabilityV1::Immutable,
                0,
                64,
                SemanticPointerMetadataV1::None,
            )
            .unwrap(),
        ),
    )
    .with_rustc_abi_properties(
        SemanticTypeAbiPropertiesV1::new(false, false).with_scalar_pointee_info(
            Some(
                SemanticAbiPointeeInfoV1::new(
                    SemanticAbiPointeeKindV1::SharedReference { frozen: true },
                    size,
                    align,
                )
                .unwrap(),
            ),
            None,
        ),
    )
}
fn types() -> Vec<SemanticTypeDeclV1> {
    vec![
        unit_type(),
        opaque(21, 0, 1),
        reference(22, CONTEXT, 0, 1),
        opaque(23, 0, 1),
        reference(24, LANE, 0, 1),
        opaque(25, 8, 4),
        opaque(26, 8, 4),
        opaque(27, 16, 4),
        scalar_type(28, SemanticScalarTypeV1::Float { bits: 32 }),
        SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256([29; 32]),
            SemanticLayoutIdentityV1::from_sha256([29; 32]),
            SemanticTypeLayoutV1::with_exact_rustc_layout(
                16,
                4,
                SemanticFieldsShapeV1::array(4, 4),
                SemanticRustcVariantsV1::Single { index: 0 },
                SemanticBackendReprV1::memory(true),
                None,
                false,
                None,
                4,
                0,
                SemanticTypeLayoutDetailsV1::None,
            )
            .unwrap(),
            SemanticTypeShapeV1::Array {
                element: F32,
                length: 4,
            },
        ),
        opaque(30, 48, 8),
        reference(31, VIEW_A, 48, 8),
        opaque(32, 48, 8),
        reference(33, VIEW_B, 48, 8),
        scalar_type(
            34,
            SemanticScalarTypeV1::Integer {
                signed: false,
                bits: 64,
            },
        ),
    ]
}
fn abi_value(ty: SemanticTypeIdV1) -> SemanticAbiValueV1 {
    if [UNIT, CONTEXT, LANE].contains(&ty) {
        return SemanticAbiValueV1::new(ty, SemanticAbiPassModeV1::Ignore);
    }
    if [CONTEXT_REF, LANE_REF, VIEW_A_REF, VIEW_B_REF].contains(&ty) {
        let (size, align) = if [VIEW_A_REF, VIEW_B_REF].contains(&ty) {
            (48, Some(8))
        } else {
            (0, None)
        };
        return SemanticAbiValueV1::new(
            ty,
            SemanticAbiPassModeV1::Direct(
                SemanticAbiValueAttributesV1::new(
                    SemanticAbiRegularAttributesV1::new(
                        true,
                        Some(SemanticAbiPointerCaptureV1::CapturesReadOnly),
                        true,
                        true,
                        false,
                        true,
                    ),
                    SemanticAbiExtensionV1::None,
                    size,
                    align,
                )
                .unwrap(),
            ),
        );
    }
    if [A, B].contains(&ty) {
        return SemanticAbiValueV1::new(
            ty,
            SemanticAbiPassModeV1::Cast {
                pad_i32: false,
                cast: SemanticAbiCastV1::new(
                    [None; 8],
                    None,
                    SemanticAbiUniformV1::new(
                        SemanticAbiRegisterV1::new(SemanticAbiRegisterKindV1::Integer, 8).unwrap(),
                        8,
                    )
                    .unwrap(),
                    SemanticAbiValueAttributesV1::plain(),
                ),
            },
        );
    }
    if [ACC, VALUES].contains(&ty) {
        return SemanticAbiValueV1::new(
            ty,
            SemanticAbiPassModeV1::Indirect {
                attributes: SemanticAbiValueAttributesV1::new(
                    SemanticAbiRegularAttributesV1::new(
                        true,
                        Some(SemanticAbiPointerCaptureV1::CapturesNone),
                        true,
                        false,
                        false,
                        true,
                    ),
                    SemanticAbiExtensionV1::None,
                    16,
                    Some(4),
                )
                .unwrap(),
                metadata_attributes: None,
                on_stack: false,
            },
        );
    }
    direct_abi_value(ty)
}
fn abi(
    tag: u8,
    inputs: &[SemanticTypeIdV1],
    output: SemanticTypeIdV1,
    kernel: bool,
) -> SemanticFunctionAbiV1 {
    SemanticFunctionAbiV1::from_rustc(
        SemanticAbiIdentityV1::from_sha256([tag; 32]),
        SemanticLayoutIdentityV1::from_sha256([250; 32]),
        if kernel {
            SemanticCanonAbiV1::GpuKernel
        } else {
            SemanticCanonAbiV1::Rust
        },
        if kernel {
            SemanticExternAbiV1::GpuKernel
        } else {
            SemanticExternAbiV1::Rust
        },
        false,
        false,
        inputs.len() as u32,
        inputs
            .iter()
            .map(|ty| SemanticAbiArgumentV1::source(abi_value(*ty)))
            .collect(),
        abi_value(output),
    )
    .unwrap()
    .with_source_argument_ownership(
        inputs
            .iter()
            .map(|ty| {
                if [CONTEXT_REF, LANE_REF, VIEW_A_REF, VIEW_B_REF].contains(ty) {
                    SemanticSourceArgumentOwnershipV1::SharedBorrow
                } else {
                    SemanticSourceArgumentOwnershipV1::ByValue
                }
            })
            .collect(),
    )
    .unwrap()
}
fn intrinsic(
    tag: u8,
    inputs: &[SemanticTypeIdV1],
    output: SemanticTypeIdV1,
    operation: SemanticCompilerIntrinsicOperationV1,
) -> SemanticCallableDeclV1 {
    SemanticCallableDeclV1::CompilerIntrinsic {
        binding: SemanticNonBodyCallableBindingV1::new(
            SemanticFunctionIdentityV1::from_sha256([tag; 32]),
            SemanticItemDefinitionIdentityV1::from_sha256([tag; 32]),
            SemanticMonomorphizationIdentityV1::from_sha256([tag; 32]),
            SemanticGenericTypeArgumentsIdentityV1::from_sha256([tag; 32]),
            SemanticConstGenericArgumentsIdentityV1::from_sha256([tag; 32]),
            provenance(),
            abi(tag, inputs, output, false),
        ),
        operation,
        operation_identity: SemanticCompilerIntrinsicIdentityV1::from_sha256([tag; 32]),
    }
}
fn contract(role: SemanticMfmaOperandRoleV1) -> SemanticMfmaOperandContractV1 {
    SemanticMfmaOperandContractV1 {
        role,
        profile: SemanticMfmaProfileV1::Bf16F32M16N16K16,
        register_distribution: SemanticMfmaRegisterDistributionV1::Tile16x16,
        wave_width: 64,
    }
}
fn accumulator() -> SemanticMfmaAccumulatorContractV1 {
    SemanticMfmaAccumulatorContractV1 {
        profile: SemanticMfmaProfileV1::Bf16F32M16N16K16,
        wave_width: 64,
        distribution: SemanticMfmaAccumulatorDistributionV1::RowMajor,
    }
}
fn callables() -> Vec<SemanticCallableDeclV1> {
    vec![
        SemanticCallableDeclV1::defined(SemanticFunctionIdV1::from_index(0)),
        SemanticCallableDeclV1::defined(SemanticFunctionIdV1::from_index(1)),
        intrinsic(
            102,
            &[],
            CONTEXT,
            SemanticCompilerIntrinsicOperationV1::MatrixContextCurrent { context: CONTEXT },
        ),
        intrinsic(
            103,
            &[],
            LANE,
            SemanticCompilerIntrinsicOperationV1::WaveLaneCurrent {
                lane: LANE,
                wave_width: 64,
            },
        ),
        intrinsic(
            104,
            &[VIEW_A_REF, LANE_REF, U64, U64],
            A,
            SemanticCompilerIntrinsicOperationV1::Bf16MatrixLoadZeroFilledV2 {
                fragment: A,
                view: VIEW_A,
                lane: LANE,
                contract: contract(SemanticMfmaOperandRoleV1::A),
                storage_layout: SemanticMfmaStorageLayoutV1::RowMajor,
            },
        ),
        intrinsic(
            105,
            &[VIEW_B_REF, LANE_REF, U64, U64],
            B,
            SemanticCompilerIntrinsicOperationV1::Bf16MatrixLoadZeroFilledV2 {
                fragment: B,
                view: VIEW_B,
                lane: LANE,
                contract: contract(SemanticMfmaOperandRoleV1::B),
                storage_layout: SemanticMfmaStorageLayoutV1::RowMajor,
            },
        ),
        intrinsic(
            106,
            &[LANE_REF],
            ACC,
            SemanticCompilerIntrinsicOperationV1::F32MatrixAccumulatorZero {
                lane: LANE,
                fragment: ACC,
                contract: accumulator(),
            },
        ),
        intrinsic(
            107,
            &[CONTEXT_REF, A, B, ACC],
            ACC,
            SemanticCompilerIntrinsicOperationV1::MatrixMultiplyAccumulate {
                context: CONTEXT,
                lhs_fragment: A,
                rhs_fragment: B,
                accumulator_fragment: ACC,
                lhs: contract(SemanticMfmaOperandRoleV1::A),
                rhs: contract(SemanticMfmaOperandRoleV1::B),
                accumulator: accumulator(),
            },
        ),
        intrinsic(
            108,
            &[ACC],
            VALUES,
            SemanticCompilerIntrinsicOperationV1::F32MatrixAccumulatorIntoValues {
                fragment: ACC,
                values: VALUES,
            },
        ),
    ]
}
fn operand(local: u32, ty: SemanticTypeIdV1) -> SemanticOperandV1 {
    SemanticOperandV1::Move(local_place(local, ty))
}
fn copied(local: u32, ty: SemanticTypeIdV1) -> SemanticOperandV1 {
    SemanticOperandV1::Copy(local_place(local, ty))
}
fn edge(role: SemanticEdgeRoleV1, to: u32) -> SemanticControlFlowEdgeV1 {
    SemanticControlFlowEdgeV1::new(role, SemanticBlockIdV1::from_index(to))
}
fn call(
    callee: u32,
    args: Vec<SemanticOperandV1>,
    destination: u32,
    ty: SemanticTypeIdV1,
    next: u32,
) -> SemanticTerminatorKindV1 {
    SemanticTerminatorKindV1::Call(
        SemanticDirectCallV1::new_callable(
            SemanticCallableIdV1::from_index(callee),
            args,
            Some(SemanticCallDestinationV1::new(
                local_place(destination, ty),
                edge(SemanticEdgeRoleV1::CallReturn, next),
            )),
            SemanticUnwindActionV1::Unreachable,
        )
        .unwrap(),
    )
}
fn assign(local: u32, ty: SemanticTypeIdV1, value: SemanticRvalueKindV1) -> SemanticStatementV1 {
    SemanticStatementV1::new(
        provenance(),
        SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
            local_place(local, ty),
            SemanticRvalueV1::new(ty, value),
        )),
    )
}
fn borrow(
    destination: u32,
    reference: SemanticTypeIdV1,
    input: u32,
    ty: SemanticTypeIdV1,
) -> SemanticStatementV1 {
    assign(
        destination,
        reference,
        SemanticRvalueKindV1::Borrow {
            kind: SemanticBorrowKindV1::Shared,
            place: local_place(input, ty),
        },
    )
}
fn function(
    tag: u8,
    abi: SemanticFunctionAbiV1,
    local_types: &[SemanticTypeIdV1],
    blocks: Vec<SemanticBasicBlockV1>,
    kernel: bool,
) -> SemanticFunctionDeclV1 {
    let arity = abi.source_input_types().len();
    SemanticFunctionDeclV1::new(
        SemanticFunctionIdentityV1::from_sha256([tag; 32]),
        if kernel {
            SemanticFunctionRoleV1::KernelRoot
        } else {
            SemanticFunctionRoleV1::InternalHelper
        },
        SemanticItemDefinitionIdentityV1::from_sha256([tag; 32]),
        SemanticMonomorphizationIdentityV1::from_sha256([tag; 32]),
        SemanticGenericTypeArgumentsIdentityV1::from_sha256([tag; 32]),
        SemanticConstGenericArgumentsIdentityV1::from_sha256([tag; 32]),
        provenance(),
        abi,
        local_types
            .iter()
            .enumerate()
            .map(|(i, ty)| {
                SemanticLocalDeclV1::new(
                    SemanticLocalIdentityV1::from_sha256([tag.wrapping_add(i as u8 + 1); 32]),
                    *ty,
                    if i == 0 {
                        SemanticLocalRoleV1::Return
                    } else if i <= arity {
                        SemanticLocalRoleV1::Argument(i as u32 - 1)
                    } else {
                        SemanticLocalRoleV1::Temporary
                    },
                    provenance(),
                )
            })
            .collect(),
        SemanticBlockIdV1::from_index(0),
        blocks,
    )
    .unwrap()
}
fn root(case: Case) -> SemanticFunctionDeclV1 {
    let zero = || scalar_constant(U64, 0, 8);
    let root_a = if matches!(case, Case::CopyRootFragment) {
        copied(7, A)
    } else {
        operand(7, A)
    };
    let blocks = vec![
        block(150, vec![], call(2, vec![], 3, CONTEXT, 1)),
        block(151, vec![], call(3, vec![], 5, LANE, 2)),
        block(
            152,
            vec![borrow(6, LANE_REF, 5, LANE)],
            call(
                4,
                vec![copied(1, VIEW_A_REF), copied(6, LANE_REF), zero(), zero()],
                7,
                A,
                3,
            ),
        ),
        block(
            153,
            vec![borrow(11, LANE_REF, 5, LANE)],
            call(
                5,
                vec![copied(2, VIEW_B_REF), copied(11, LANE_REF), zero(), zero()],
                8,
                B,
                4,
            ),
        ),
        block(
            154,
            vec![borrow(12, LANE_REF, 5, LANE)],
            call(6, vec![copied(12, LANE_REF)], 9, ACC, 5),
        ),
        block(
            155,
            vec![borrow(4, CONTEXT_REF, 3, CONTEXT)],
            call(
                1,
                vec![
                    copied(4, CONTEXT_REF),
                    root_a,
                    operand(8, B),
                    operand(9, ACC),
                ],
                10,
                VALUES,
                6,
            ),
        ),
        block(156, vec![], SemanticTerminatorKindV1::Return),
    ];
    let dims = SemanticWorkgroupDimensionsV1::new([64, 1, 1]).unwrap();
    function(
        50,
        abi(50, &[VIEW_A_REF, VIEW_B_REF], UNIT, true),
        &[
            UNIT,
            VIEW_A_REF,
            VIEW_B_REF,
            CONTEXT,
            CONTEXT_REF,
            LANE,
            LANE_REF,
            A,
            B,
            ACC,
            VALUES,
            LANE_REF,
            LANE_REF,
        ],
        blocks,
        true,
    )
    .with_kernel_entry(SemanticKernelEntryV1::new(
        SemanticLinkSymbolV1::new(b"inert_bf16_call_transport".to_vec()).unwrap(),
        SemanticKernelBindingIdentityV1::from_sha256([51; 32]),
        SemanticKernelSourceContractV1::new(
            Some(SemanticKernelLaunchBoundsV1::new(Some(dims), Some(dims), None).unwrap()),
            None,
            None,
        )
        .unwrap(),
    ))
}
fn component(index: u8) -> SemanticOperandV1 {
    SemanticOperandV1::Copy(
        SemanticPlaceV1::new(
            SemanticLocalIdV1::from_index(6),
            vec![
                SemanticProjectionV1::new(
                    SemanticProjectionKindV1::ConstantIndex {
                        offset: u64::from(index),
                        minimum_length: 4,
                        from_end: false,
                    },
                    F32,
                )
                .unwrap(),
            ],
            F32,
        )
        .unwrap(),
    )
}
fn helper(case: Case) -> SemanticFunctionDeclV1 {
    let return_value = if matches!(case, Case::Identity | Case::CopyRootFragment) {
        SemanticRvalueKindV1::Use(operand(6, VALUES))
    } else {
        SemanticRvalueKindV1::Aggregate(
            SemanticAggregateRvalueV1::new(
                SemanticAggregateKindV1::Array,
                case.permutation().into_iter().map(component).collect(),
            )
            .unwrap(),
        )
    };
    let tail = match case {
        Case::ConditionalHelper => SemanticTerminatorKindV1::SwitchInt {
            discriminant: scalar_constant(U64, 0, 8),
            targets: SemanticSwitchTargetsV1::new(
                vec![SemanticSwitchTargetV1::new(
                    0,
                    edge(SemanticEdgeRoleV1::SwitchValue, 3),
                )],
                edge(SemanticEdgeRoleV1::SwitchOtherwise, 4),
            )
            .unwrap(),
        },
        Case::CyclicHelper => SemanticTerminatorKindV1::Goto(edge(SemanticEdgeRoleV1::Goto, 2)),
        _ => SemanticTerminatorKindV1::Return,
    };
    let mut blocks = vec![
        block(
            170,
            vec![],
            call(
                7,
                vec![
                    copied(1, CONTEXT_REF),
                    operand(2, A),
                    operand(3, B),
                    operand(4, ACC),
                ],
                5,
                ACC,
                1,
            ),
        ),
        block(171, vec![], call(8, vec![operand(5, ACC)], 6, VALUES, 2)),
        block(172, vec![assign(0, VALUES, return_value)], tail),
    ];
    if matches!(case, Case::ConditionalHelper) {
        blocks.push(block(173, vec![], SemanticTerminatorKindV1::Return));
        blocks.push(block(174, vec![], SemanticTerminatorKindV1::Return));
    } else if matches!(case, Case::UnreachableHelperBlock) {
        blocks.push(block(173, vec![], SemanticTerminatorKindV1::Return));
    }
    function(
        70,
        abi(70, &[CONTEXT_REF, A, B, ACC], VALUES, false),
        &[VALUES, CONTEXT_REF, A, B, ACC, ACC, VALUES],
        blocks,
        false,
    )
}
pub(super) fn owner(case: Case) -> ProductionSemanticSsaOwnerV1 {
    let request = InertSemanticMirRequestV1::new_with_callables(
        SemanticTargetDataLayoutV1::gfx942(SemanticLayoutIdentityV1::from_sha256([250; 32])),
        types(),
        vec![],
        vec![],
        vec![],
        vec![root(case), helper(case)],
        callables(),
        vec![SemanticFunctionIdV1::from_index(0)],
    )
    .expect("test request construction");
    let admitted = request
        .admit(SemanticMirLimitsV1::default())
        .expect("test semantic admission; earlier refusal is NOT a checked-relation result");
    let mir =
        ProductionSemanticMirOwnerV1::try_new(admitted, ProductionSemanticMirLimitsV1::default())
            .expect("test semantic owner; earlier refusal is NOT a checked-relation result");
    let owner =
        ProductionSemanticSsaOwnerV1::try_new(mir, ProductionSemanticSsaLimitsV1::default())
            .expect("test real SSA construction; earlier refusal is NOT a checked-relation result");
    owner.verify_replay().expect("test real SSA replay");
    owner
}
