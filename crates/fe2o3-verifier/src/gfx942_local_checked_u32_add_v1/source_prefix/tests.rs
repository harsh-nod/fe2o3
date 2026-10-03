use super::*;
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1, CanonicalKernelIrWorkBudgetV1,
};
use fe2o3_lower_mir_kernel::{
    ProductionCheckedU32AddCaptureRequestV1, ProductionSemanticKirLimitsV1,
    ProductionSemanticKirOwnerV1,
};
use fe2o3_mir_model::semantic_mir_v1::*;
use fe2o3_pliron::{ProductionSemanticMirLimitsV1, ProductionSemanticMirOwnerV1};

#[path = "tests/normalization.rs"]
mod normalization;

fn ty(index: u32) -> SemanticTypeIdV1 {
    SemanticTypeIdV1::from_index(index)
}
fn place(local: u32, kind: u32) -> SemanticPlaceV1 {
    SemanticPlaceV1::new(SemanticLocalIdV1::from_index(local), vec![], ty(kind)).unwrap()
}
fn constant(value: u32) -> SemanticOperandV1 {
    SemanticOperandV1::Constant(SemanticConstantV1::new(
        ty(1),
        SemanticConstantValueV1::Scalar(SemanticScalarValueV1::new(value.into(), 4).unwrap()),
    ))
}
fn copy(local: u32) -> SemanticOperandV1 {
    SemanticOperandV1::Copy(place(local, 1))
}
fn assign(local: u32, operand: SemanticOperandV1) -> SemanticStatementV1 {
    SemanticStatementV1::new(
        SemanticSourceProvenanceV1::unavailable(),
        SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
            place(local, 1),
            SemanticRvalueV1::new(ty(1), SemanticRvalueKindV1::Use(operand)),
        )),
    )
}
fn nop() -> SemanticStatementV1 {
    SemanticStatementV1::new(
        SemanticSourceProvenanceV1::unavailable(),
        SemanticStatementKindV1::Nop,
    )
}

fn owner(
    mut prefix: Vec<SemanticStatementV1>,
    lhs: u32,
    literal: u32,
    arguments: usize,
    locals: usize,
) -> ProductionSemanticKirOwnerV1 {
    let scalar = |tag, shape, size, bits, maximum| {
        SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256([tag; 32]),
            SemanticLayoutIdentityV1::from_sha256([tag; 32]),
            SemanticTypeLayoutV1::new_with_backend_repr(
                Some(size),
                size,
                SemanticBackendReprV1::scalar(SemanticBackendScalarV1::initialized(
                    SemanticBackendPrimitiveV1::integer(false, bits, size),
                    SemanticScalarValidityRangeV1::new(0, maximum),
                )),
                false,
            )
            .unwrap(),
            SemanticTypeShapeV1::Scalar(shape),
        )
    };
    let types = vec![
        SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256([1; 32]),
            SemanticLayoutIdentityV1::from_sha256([1; 32]),
            SemanticTypeLayoutV1::with_exact_rustc_layout(
                0,
                1,
                SemanticFieldsShapeV1::arbitrary(vec![], vec![]).unwrap(),
                SemanticRustcVariantsV1::Single { index: 0 },
                SemanticBackendReprV1::memory(true),
                None,
                false,
                None,
                1,
                0,
                SemanticTypeLayoutDetailsV1::None,
            )
            .unwrap(),
            SemanticTypeShapeV1::Unit,
        ),
        scalar(
            2,
            SemanticScalarTypeV1::Integer {
                signed: false,
                bits: 32,
            },
            4,
            32,
            u32::MAX.into(),
        ),
        scalar(3, SemanticScalarTypeV1::Bool, 1, 8, 1),
        SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256([4; 32]),
            SemanticLayoutIdentityV1::from_sha256([4; 32]),
            SemanticTypeLayoutV1::aggregate(
                Some(8),
                4,
                SemanticAggregateLayoutV1::new(
                    vec![0, 4],
                    vec![SemanticPaddingV1::new(5, 3).unwrap()],
                )
                .unwrap(),
            )
            .unwrap(),
            SemanticTypeShapeV1::Tuple(SemanticAggregateTypeV1::new(vec![ty(1), ty(2)]).unwrap()),
        ),
    ];
    let statement = prefix.len() as u32;
    prefix.push(SemanticStatementV1::new(
        SemanticSourceProvenanceV1::unavailable(),
        SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
            place((locals - 1) as u32, 3),
            SemanticRvalueV1::new(
                ty(3),
                SemanticRvalueKindV1::CheckedBinary(SemanticCheckedBinaryRvalueV1::new(
                    SemanticCheckedBinaryOpV1::Add,
                    copy(lhs),
                    constant(literal),
                )),
            ),
        )),
    ));
    let block = SemanticBasicBlockV1::new(
        SemanticBlockIdentityV1::from_sha256([10; 32]),
        SemanticSourceProvenanceV1::unavailable(),
        prefix,
        SemanticTerminatorV1::new(
            SemanticSourceProvenanceV1::unavailable(),
            SemanticTerminatorKindV1::Return,
        ),
    )
    .unwrap();
    let abi = SemanticFunctionAbiV1::from_rustc(
        SemanticAbiIdentityV1::from_sha256([11; 32]),
        SemanticLayoutIdentityV1::from_sha256([250; 32]),
        SemanticCanonAbiV1::GpuKernel,
        SemanticExternAbiV1::GpuKernel,
        false,
        false,
        arguments as u32,
        (0..arguments)
            .map(|_| {
                SemanticAbiArgumentV1::source(SemanticAbiValueV1::new(
                    ty(1),
                    SemanticAbiPassModeV1::Direct(
                        SemanticAbiValueAttributesV1::new(
                            SemanticAbiRegularAttributesV1::new(
                                false, None, false, false, false, true,
                            ),
                            SemanticAbiExtensionV1::None,
                            0,
                            None,
                        )
                        .unwrap(),
                    ),
                ))
            })
            .collect(),
        SemanticAbiValueV1::new(ty(0), SemanticAbiPassModeV1::Ignore),
    )
    .unwrap();
    let locals = (0..locals)
        .map(|index| {
            SemanticLocalDeclV1::new(
                SemanticLocalIdentityV1::from_sha256({
                    let mut bytes = [20; 32];
                    bytes[..8].copy_from_slice(&(index as u64).to_be_bytes());
                    bytes
                }),
                ty(if index == 0 {
                    0
                } else if index + 1 == locals {
                    3
                } else {
                    1
                }),
                if index == 0 {
                    SemanticLocalRoleV1::Return
                } else if index <= arguments {
                    SemanticLocalRoleV1::Argument((arguments - index) as u32)
                } else {
                    SemanticLocalRoleV1::Temporary
                },
                SemanticSourceProvenanceV1::unavailable(),
            )
        })
        .collect();
    let function = SemanticFunctionDeclV1::new(
        SemanticFunctionIdentityV1::from_sha256([30; 32]),
        SemanticFunctionRoleV1::KernelRoot,
        SemanticItemDefinitionIdentityV1::from_sha256([30; 32]),
        SemanticMonomorphizationIdentityV1::from_sha256([30; 32]),
        SemanticGenericTypeArgumentsIdentityV1::from_sha256([30; 32]),
        SemanticConstGenericArgumentsIdentityV1::from_sha256([30; 32]),
        SemanticSourceProvenanceV1::unavailable(),
        abi,
        locals,
        SemanticBlockIdV1::from_index(0),
        vec![block],
    )
    .unwrap()
    .with_kernel_entry(SemanticKernelEntryV1::new(
        SemanticLinkSymbolV1::new(b"checked_u32_prefix".to_vec()).unwrap(),
        SemanticKernelBindingIdentityV1::from_sha256([31; 32]),
        SemanticKernelSourceContractV1::new(None, None, None).unwrap(),
    ));
    let source = InertSemanticMirRequestV1::new(
        SemanticTargetDataLayoutV1::gfx942(SemanticLayoutIdentityV1::from_sha256([250; 32])),
        types,
        vec![],
        vec![],
        vec![],
        vec![function],
        vec![SemanticFunctionIdV1::from_index(0)],
    )
    .unwrap()
    .admit(SemanticMirLimitsV1::default())
    .unwrap();
    let source =
        ProductionSemanticMirOwnerV1::try_new(source, ProductionSemanticMirLimitsV1::default())
            .unwrap();
    let mut work = CanonicalKernelIrWorkBudgetV1::new(16_000_000);
    let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, 16_000_000);
    ProductionSemanticKirOwnerV1::try_lower_with_checked_u32_add_capture_v1(
        source,
        ProductionSemanticKirLimitsV1::default(),
        ProductionCheckedU32AddCaptureRequestV1::new(
            SemanticFunctionIdV1::from_index(0),
            SemanticFunctionIdV1::from_index(0),
            SemanticBlockIdV1::from_index(0),
            statement,
        ),
        &mut budget,
    )
    .unwrap()
}

#[test]
fn genuine_capture_tracks_argument_roles_aliases_redefinitions_and_self_copy() {
    let owner = owner(
        vec![
            assign(3, copy(1)),
            assign(4, copy(3)),
            assign(3, constant(7)),
            nop(),
            assign(4, copy(4)),
        ],
        4,
        1,
        2,
        7,
    );
    let capture = owner.checked_u32_add_capture_v1().unwrap();
    let relation = check_captured_checked_u32_prefix_v1(capture).unwrap();
    assert!(std::ptr::eq(relation.capture().owner(), &owner));
    assert_eq!(
        relation.operand_origin(),
        CheckedU32PrefixOriginV1::Argument(1)
    );
    assert_eq!(
        relation
            .arguments()
            .iter()
            .map(|arg| (arg.argument(), arg.semantic_local()))
            .collect::<Vec<_>>(),
        [(0, 2), (1, 1)]
    );
    for a in [0, 1, 17, u32::MAX] {
        for b in [0, 1, 91, u32::MAX - 1, u32::MAX] {
            let result = relation.evaluate(&[a, b]).unwrap();
            assert_eq!((result.value, result.scc), b.overflowing_add(1));
        }
    }
    assert!(matches!(
        relation.evaluate(&[1]),
        Err(CheckedU32PrefixErrorV1::Arguments)
    ));
    assert!(matches!(
        relation.evaluate(&[1, 2, 3]),
        Err(CheckedU32PrefixErrorV1::Arguments)
    ));
}

#[test]
fn constants_preserve_old_versions_and_cover_value_carry_boundaries() {
    for source in [0, 1, 0x8000_0000, u32::MAX - 1, u32::MAX] {
        for literal in [0, 1, 2, 0x8000_0000, u32::MAX] {
            let owner = owner(
                vec![
                    assign(1, constant(source)),
                    assign(2, copy(1)),
                    assign(1, constant(43)),
                ],
                2,
                literal,
                0,
                4,
            );
            let relation =
                check_captured_checked_u32_prefix_v1(owner.checked_u32_add_capture_v1().unwrap())
                    .unwrap();
            assert_eq!(
                relation.operand_origin(),
                CheckedU32PrefixOriginV1::Constant(source)
            );
            let actual = relation.evaluate(&[]).unwrap();
            assert_eq!((actual.value, actual.scc), source.overflowing_add(literal));
        }
    }
}

#[test]
fn shared_fold_rejects_uninitialized_and_out_of_bounds_without_receipts() {
    for step in [
        PrefixStep {
            destination: 0,
            input: PrefixInput::Cell(1),
        },
        PrefixStep {
            destination: 0,
            input: PrefixInput::Cell(usize::MAX),
        },
        PrefixStep {
            destination: usize::MAX,
            input: PrefixInput::Constant(7),
        },
    ] {
        let mut state = [Origin::Constant(11), Origin::Uninitialized];
        assert!(!fold::fold(&mut state, &[step]));
        assert_eq!(state, [Origin::Constant(11), Origin::Uninitialized]);
    }
    let mut state = [Origin::Uninitialized; 2];
    let steps = [
        PrefixStep {
            destination: 0,
            input: PrefixInput::Constant(8),
        },
        PrefixStep {
            destination: 1,
            input: PrefixInput::Cell(1),
        },
    ];
    assert!(!fold::fold(&mut state, &steps));
    assert_eq!(state, [Origin::Constant(8), Origin::Uninitialized]);
}

#[test]
fn malformed_actual_kir_prefixes_and_argument_permutations_reject() {
    let owner = owner(
        vec![
            assign(3, copy(1)),
            assign(4, copy(3)),
            assign(3, constant(7)),
        ],
        4,
        1,
        2,
        7,
    );
    let capture = owner.checked_u32_add_capture_v1().unwrap();
    for mutation in 0..11 {
        let mut module = owner.module().clone();
        let function = module
            .functions
            .iter_mut()
            .find(|f| f.id.as_str() == capture.kernel_ir_function())
            .unwrap();
        let body = function.body.as_mut().unwrap();
        let ops = &mut body.blocks[0].operations;
        match mutation {
            0 => body.parameters.swap(0, 1),
            1 => function.signature.parameters[0] = Type::Scalar(ScalarType::U64),
            2 => body.parameters[0] = body.parameters[1],
            3 => ops[0].results[0].id = body.parameters[0],
            4 => ops[0].results[0].ty = Type::Scalar(ScalarType::U64),
            5 => ops[1].kind = OperationKind::Constant(Constant::U32(2)),
            6 => ops[2].results.swap(0, 1),
            7 => ops[2].results[0].ty = Type::Scalar(ScalarType::U64),
            8 => ops[2].results[0].id = ops[2].results[1].id,
            9 => {
                ops[2].kind = OperationKind::Binary {
                    op: BinaryOp::Checked(CheckedBinaryOperator::Add),
                    lhs: body.parameters[0],
                    rhs: ops[1].results[0].id,
                }
            }
            _ => {
                ops.remove(0);
            }
        }
        assert!(
            check_parts(
                capture,
                owner.semantic().semantic(),
                &module,
                owner.correspondence()
            )
            .is_err(),
            "mutation {mutation}"
        );
    }
}

#[test]
fn live_constant_corruption_rejects_but_dead_value_changes_are_not_overclaimed() {
    let owner = owner(
        vec![
            assign(1, constant(17)),
            assign(2, copy(1)),
            assign(1, constant(42)),
        ],
        2,
        1,
        0,
        4,
    );
    let capture = owner.checked_u32_add_capture_v1().unwrap();
    for (operation, accepted) in [(0, false), (1, true)] {
        let mut module = owner.module().clone();
        module.functions[0].body.as_mut().unwrap().blocks[0].operations[operation].kind =
            OperationKind::Constant(Constant::U32(99));
        assert_eq!(
            check_parts(
                capture,
                owner.semantic().semantic(),
                &module,
                owner.correspondence()
            )
            .is_ok(),
            accepted
        );
    }
}

#[test]
fn exact_statement_local_and_argument_capacity_boundaries_are_checked() {
    for count in [MAX_STATEMENTS - 1, MAX_STATEMENTS] {
        let owner = owner(vec![nop(); count], 1, 1, 1, 3);
        assert_eq!(
            check_captured_checked_u32_prefix_v1(owner.checked_u32_add_capture_v1().unwrap())
                .is_ok(),
            count < MAX_STATEMENTS
        );
    }
    for locals in [MAX_LOCALS, MAX_LOCALS + 1] {
        let owner = owner(vec![nop()], 1, 1, 1, locals);
        assert_eq!(
            check_captured_checked_u32_prefix_v1(owner.checked_u32_add_capture_v1().unwrap())
                .is_ok(),
            locals == MAX_LOCALS
        );
    }
    for arguments in [MAX_ARGUMENTS, MAX_ARGUMENTS + 1] {
        let owner = owner(vec![nop()], 1, 1, arguments, arguments + 2);
        assert_eq!(
            check_captured_checked_u32_prefix_v1(owner.checked_u32_add_capture_v1().unwrap())
                .is_ok(),
            arguments == MAX_ARGUMENTS
        );
    }
}

#[test]
fn moves_and_storage_effects_are_not_discarded_as_empty_spans() {
    for prefix in [
        vec![assign(2, SemanticOperandV1::Move(place(1, 1)))],
        vec![
            SemanticStatementV1::new(
                SemanticSourceProvenanceV1::unavailable(),
                SemanticStatementKindV1::StorageLive(SemanticLocalIdV1::from_index(2)),
            ),
            assign(2, copy(1)),
        ],
    ] {
        let owner = owner(prefix, 2, 1, 1, 4);
        assert!(matches!(
            check_captured_checked_u32_prefix_v1(owner.checked_u32_add_capture_v1().unwrap()),
            Err(CheckedU32PrefixErrorV1::Source)
        ));
    }
}
