use fe2o3_mir_model::semantic_mir_v1::*;
use fe2o3_mir_model::{
    MAX_SEMANTIC_U32_INDUCTION_CERTIFICATES_V1, MAX_SEMANTIC_U32_INDUCTION_WORK_V1,
    SemanticU32InductionAnalysisErrorV1, SemanticU32InductionAnalysisLimitsV1,
    analyze_semantic_u32_induction_no_overflow_v1,
    analyze_semantic_u32_induction_no_overflow_with_limits_v1,
};

const UNIT: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(0);
const U32: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(1);
const BOOL: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(2);
const CHECKED_U32: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(3);
const U64: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(4);
const CHECKED_U64: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(5);
const I32: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(6);
const CHECKED_I32: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(7);

const INDUCTION: SemanticLocalIdV1 = SemanticLocalIdV1::from_index(1);
const BOUND: SemanticLocalIdV1 = SemanticLocalIdV1::from_index(2);
const PREDICATE: SemanticLocalIdV1 = SemanticLocalIdV1::from_index(3);
const CHECKED_RESULT: SemanticLocalIdV1 = SemanticLocalIdV1::from_index(4);
const ALIAS: SemanticLocalIdV1 = SemanticLocalIdV1::from_index(5);
const FUNCTION: SemanticFunctionIdV1 = SemanticFunctionIdV1::from_index(0);

#[derive(Clone, Copy, Debug, Default)]
enum Mutation {
    #[default]
    None,
    AliasGuardInduction,
    AliasGuardBound,
    ProjectedCheckedInduction,
    U64Induction,
    SignedI32Induction,
    CheckedResultDeclarationMismatch,
    PredicateDeclarationMismatch,
    TemporaryBound,
    LessOrEqualGuard,
    ReversedGuard,
    StepTwo,
    InitialOne,
    TrueValueSwitch,
    ExpectedOverflow,
    WrongOverflowMessage,
    WrongOverflowCondition,
    ReachableUnwind,
    WrongAssertionTarget,
    AliasedUpdate,
    ResetDefinition,
    DuplicatePredicateDefinition,
    DuplicateCheckedDefinition,
    AlternateBodyEntry,
    WrongBodyEdgeRole,
    UnrelatedExitNop,
    UnreachableBlock,
    UnknownEdge,
}

fn bytes(seed: u8, tag: u8) -> [u8; 32] {
    [seed.wrapping_add(tag); 32]
}

fn scalar_type(seed: u8, tag: u8, size: u64, shape: SemanticScalarTypeV1) -> SemanticTypeDeclV1 {
    let (primitive, maximum) = match shape {
        SemanticScalarTypeV1::Bool => (SemanticBackendPrimitiveV1::integer(false, 8, 1), 1),
        SemanticScalarTypeV1::Integer { signed, bits } => (
            SemanticBackendPrimitiveV1::integer(signed, bits, size),
            if bits == 128 {
                u128::MAX
            } else {
                (1_u128 << bits) - 1
            },
        ),
        SemanticScalarTypeV1::Char | SemanticScalarTypeV1::Float { .. } => {
            unreachable!("the induction fixture catalog contains only bool and integer scalars")
        }
    };
    SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256(bytes(seed, tag)),
        SemanticLayoutIdentityV1::from_sha256(bytes(seed, tag)),
        SemanticTypeLayoutV1::new_with_backend_repr(
            Some(size),
            size.max(1),
            SemanticBackendReprV1::scalar(SemanticBackendScalarV1::initialized(
                primitive,
                SemanticScalarValidityRangeV1::new(0, maximum),
            )),
            false,
        )
        .unwrap(),
        SemanticTypeShapeV1::Scalar(shape),
    )
}

fn checked_tuple_type(seed: u8, tag: u8, value: SemanticTypeIdV1, size: u64) -> SemanticTypeDeclV1 {
    let value_size = size / 2;
    SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256(bytes(seed, tag)),
        SemanticLayoutIdentityV1::from_sha256(bytes(seed, tag)),
        SemanticTypeLayoutV1::aggregate(
            Some(size),
            value_size,
            SemanticAggregateLayoutV1::new(
                vec![0, value_size],
                vec![SemanticPaddingV1::new(value_size + 1, value_size - 1).unwrap()],
            )
            .unwrap(),
        )
        .unwrap(),
        SemanticTypeShapeV1::Tuple(SemanticAggregateTypeV1::new(vec![value, BOOL]).unwrap()),
    )
}

fn types(seed: u8) -> Vec<SemanticTypeDeclV1> {
    vec![
        SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256(bytes(seed, 1)),
            SemanticLayoutIdentityV1::from_sha256(bytes(seed, 1)),
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
        scalar_type(
            seed,
            2,
            4,
            SemanticScalarTypeV1::Integer {
                signed: false,
                bits: 32,
            },
        ),
        scalar_type(seed, 3, 1, SemanticScalarTypeV1::Bool),
        checked_tuple_type(seed, 4, U32, 8),
        scalar_type(
            seed,
            5,
            8,
            SemanticScalarTypeV1::Integer {
                signed: false,
                bits: 64,
            },
        ),
        checked_tuple_type(seed, 6, U64, 16),
        scalar_type(
            seed,
            7,
            4,
            SemanticScalarTypeV1::Integer {
                signed: true,
                bits: 32,
            },
        ),
        checked_tuple_type(seed, 8, I32, 8),
    ]
}

fn direct_value(ty: SemanticTypeIdV1) -> SemanticAbiValueV1 {
    SemanticAbiValueV1::new(
        ty,
        SemanticAbiPassModeV1::Direct(
            SemanticAbiValueAttributesV1::new(
                SemanticAbiRegularAttributesV1::new(false, None, false, false, false, true),
                SemanticAbiExtensionV1::None,
                0,
                None,
            )
            .unwrap(),
        ),
    )
}

fn local(
    seed: u8,
    tag: u8,
    ty: SemanticTypeIdV1,
    role: SemanticLocalRoleV1,
) -> SemanticLocalDeclV1 {
    SemanticLocalDeclV1::new(
        SemanticLocalIdentityV1::from_sha256(bytes(seed, tag)),
        ty,
        role,
        SemanticSourceProvenanceV1::unavailable(),
    )
}

fn place(local: SemanticLocalIdV1, ty: SemanticTypeIdV1) -> SemanticPlaceV1 {
    SemanticPlaceV1::new(local, vec![], ty).unwrap()
}

fn field_place(local: SemanticLocalIdV1, field: u32, ty: SemanticTypeIdV1) -> SemanticPlaceV1 {
    SemanticPlaceV1::new(
        local,
        vec![SemanticProjectionV1::new(SemanticProjectionKindV1::Field(field), ty).unwrap()],
        ty,
    )
    .unwrap()
}

fn projected_place(local: SemanticLocalIdV1, ty: SemanticTypeIdV1) -> SemanticPlaceV1 {
    SemanticPlaceV1::new(
        local,
        vec![SemanticProjectionV1::new(SemanticProjectionKindV1::OpaqueCast, ty).unwrap()],
        ty,
    )
    .unwrap()
}

fn copy(local: SemanticLocalIdV1, ty: SemanticTypeIdV1) -> SemanticOperandV1 {
    SemanticOperandV1::Copy(place(local, ty))
}

fn scalar_constant(ty: SemanticTypeIdV1, value: u128, size: u8) -> SemanticOperandV1 {
    SemanticOperandV1::Constant(SemanticConstantV1::new(
        ty,
        SemanticConstantValueV1::Scalar(SemanticScalarValueV1::new(value, size).unwrap()),
    ))
}

fn assign(
    destination: SemanticPlaceV1,
    result_type: SemanticTypeIdV1,
    value: SemanticRvalueKindV1,
) -> SemanticStatementV1 {
    SemanticStatementV1::new(
        SemanticSourceProvenanceV1::unavailable(),
        SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
            destination,
            SemanticRvalueV1::new(result_type, value),
        )),
    )
}

fn edge(role: SemanticEdgeRoleV1, target: u32) -> SemanticControlFlowEdgeV1 {
    SemanticControlFlowEdgeV1::new(role, SemanticBlockIdV1::from_index(target))
}

fn block(
    seed: u8,
    tag: u8,
    statements: Vec<SemanticStatementV1>,
    terminator: SemanticTerminatorKindV1,
) -> SemanticBasicBlockV1 {
    SemanticBasicBlockV1::new(
        SemanticBlockIdentityV1::from_sha256(bytes(seed, tag)),
        SemanticSourceProvenanceV1::unavailable(),
        statements,
        SemanticTerminatorV1::new(SemanticSourceProvenanceV1::unavailable(), terminator),
    )
    .unwrap()
}

fn function(seed: u8, mutation: Mutation) -> (Vec<SemanticTypeDeclV1>, SemanticFunctionDeclV1) {
    let types = types(seed);
    let (integer_ty, checked_ty, integer_size) = match mutation {
        Mutation::U64Induction => (U64, CHECKED_U64, 8),
        Mutation::SignedI32Induction => (I32, CHECKED_I32, 4),
        _ => (U32, CHECKED_U32, 4),
    };
    let bound_role = if matches!(mutation, Mutation::TemporaryBound) {
        SemanticLocalRoleV1::Temporary
    } else {
        SemanticLocalRoleV1::Argument(0)
    };
    let locals = vec![
        local(seed, 20, integer_ty, SemanticLocalRoleV1::Return),
        local(seed, 21, integer_ty, SemanticLocalRoleV1::Temporary),
        local(seed, 22, integer_ty, bound_role),
        local(
            seed,
            23,
            if matches!(mutation, Mutation::PredicateDeclarationMismatch) {
                integer_ty
            } else {
                BOOL
            },
            SemanticLocalRoleV1::Temporary,
        ),
        local(
            seed,
            24,
            if matches!(mutation, Mutation::CheckedResultDeclarationMismatch) {
                integer_ty
            } else {
                checked_ty
            },
            SemanticLocalRoleV1::Temporary,
        ),
        local(seed, 25, integer_ty, SemanticLocalRoleV1::Temporary),
        local(seed, 26, UNIT, SemanticLocalRoleV1::Temporary),
        local(seed, 27, U32, SemanticLocalRoleV1::Temporary),
        local(seed, 28, BOOL, SemanticLocalRoleV1::Temporary),
        local(seed, 29, CHECKED_U32, SemanticLocalRoleV1::Temporary),
        local(seed, 30, U64, SemanticLocalRoleV1::Temporary),
        local(seed, 31, CHECKED_U64, SemanticLocalRoleV1::Temporary),
        local(seed, 32, I32, SemanticLocalRoleV1::Temporary),
        local(seed, 33, CHECKED_I32, SemanticLocalRoleV1::Temporary),
    ];

    let initial = if matches!(mutation, Mutation::InitialOne) {
        1
    } else {
        0
    };
    let initialization = assign(
        place(INDUCTION, integer_ty),
        integer_ty,
        SemanticRvalueKindV1::Use(scalar_constant(integer_ty, initial, integer_size)),
    );
    let preheader_terminator = if matches!(mutation, Mutation::AlternateBodyEntry) {
        SemanticTerminatorKindV1::FalseEdge {
            real_target: edge(SemanticEdgeRoleV1::FalseEdgeReal, 1),
            imaginary_target: edge(SemanticEdgeRoleV1::FalseEdgeImaginary, 2),
        }
    } else {
        SemanticTerminatorKindV1::Goto(edge(SemanticEdgeRoleV1::Goto, 1))
    };

    let mut guard_statements = Vec::new();
    let guard_left = if matches!(mutation, Mutation::AliasGuardInduction) {
        guard_statements.push(assign(
            place(ALIAS, integer_ty),
            integer_ty,
            SemanticRvalueKindV1::Use(copy(INDUCTION, integer_ty)),
        ));
        copy(ALIAS, integer_ty)
    } else {
        copy(INDUCTION, integer_ty)
    };
    let guard_right = if matches!(mutation, Mutation::AliasGuardBound) {
        guard_statements.push(assign(
            place(ALIAS, integer_ty),
            integer_ty,
            SemanticRvalueKindV1::Use(copy(BOUND, integer_ty)),
        ));
        copy(ALIAS, integer_ty)
    } else {
        copy(BOUND, integer_ty)
    };
    let (guard_left, guard_right) = if matches!(mutation, Mutation::ReversedGuard) {
        (guard_right, guard_left)
    } else {
        (guard_left, guard_right)
    };
    let guard_operation = if matches!(mutation, Mutation::LessOrEqualGuard) {
        SemanticBinaryOpV1::LessOrEqual
    } else {
        SemanticBinaryOpV1::LessThan
    };
    let guard = assign(
        place(PREDICATE, BOOL),
        BOOL,
        SemanticRvalueKindV1::Binary {
            operation: guard_operation,
            left: guard_left,
            right: guard_right,
        },
    );
    guard_statements.push(guard.clone());
    if matches!(mutation, Mutation::DuplicatePredicateDefinition) {
        guard_statements.push(guard);
    }
    let header_terminator = if matches!(mutation, Mutation::TrueValueSwitch) {
        SemanticTerminatorKindV1::SwitchInt {
            discriminant: copy(PREDICATE, BOOL),
            targets: SemanticSwitchTargetsV1::new(
                vec![SemanticSwitchTargetV1::new(
                    1,
                    edge(SemanticEdgeRoleV1::SwitchValue, 2),
                )],
                edge(SemanticEdgeRoleV1::SwitchOtherwise, 4),
            )
            .unwrap(),
        }
    } else {
        SemanticTerminatorKindV1::SwitchInt {
            discriminant: copy(PREDICATE, BOOL),
            targets: SemanticSwitchTargetsV1::new(
                vec![SemanticSwitchTargetV1::new(
                    0,
                    edge(SemanticEdgeRoleV1::SwitchValue, 4),
                )],
                edge(
                    if matches!(mutation, Mutation::WrongBodyEdgeRole) {
                        SemanticEdgeRoleV1::SwitchValue
                    } else {
                        SemanticEdgeRoleV1::SwitchOtherwise
                    },
                    2,
                ),
            )
            .unwrap(),
        }
    };

    let checked_left = if matches!(mutation, Mutation::ProjectedCheckedInduction) {
        SemanticOperandV1::Copy(projected_place(INDUCTION, integer_ty))
    } else {
        copy(INDUCTION, integer_ty)
    };
    let step = if matches!(mutation, Mutation::StepTwo) {
        2
    } else {
        1
    };
    let checked_right = scalar_constant(integer_ty, step, integer_size);
    let checked_statement = assign(
        place(CHECKED_RESULT, checked_ty),
        checked_ty,
        SemanticRvalueKindV1::CheckedBinary(SemanticCheckedBinaryRvalueV1::new(
            SemanticCheckedBinaryOpV1::Add,
            checked_left.clone(),
            checked_right.clone(),
        )),
    );
    let mut checked_statements = Vec::new();
    if matches!(mutation, Mutation::ResetDefinition) {
        checked_statements.push(assign(
            place(INDUCTION, integer_ty),
            integer_ty,
            SemanticRvalueKindV1::Use(scalar_constant(integer_ty, 0, integer_size)),
        ));
    }
    checked_statements.push(checked_statement.clone());
    if matches!(mutation, Mutation::DuplicateCheckedDefinition) {
        checked_statements.push(checked_statement);
    }
    let condition = if matches!(mutation, Mutation::WrongOverflowCondition) {
        SemanticOperandV1::Copy(field_place(CHECKED_RESULT, 0, integer_ty))
    } else {
        SemanticOperandV1::Copy(field_place(CHECKED_RESULT, 1, BOOL))
    };
    let checked_terminator = SemanticTerminatorKindV1::Assert {
        condition,
        expected: matches!(mutation, Mutation::ExpectedOverflow),
        message: SemanticAssertMessageV1::Overflow {
            operation: if matches!(mutation, Mutation::WrongOverflowMessage) {
                SemanticBinaryOpV1::Subtract
            } else {
                SemanticBinaryOpV1::Add
            },
            left: checked_left,
            right: checked_right,
        },
        target: edge(
            if matches!(mutation, Mutation::WrongAssertionTarget) {
                SemanticEdgeRoleV1::Goto
            } else {
                SemanticEdgeRoleV1::AssertSuccess
            },
            3,
        ),
        unwind: if matches!(mutation, Mutation::ReachableUnwind) {
            SemanticUnwindActionV1::Continue
        } else {
            SemanticUnwindActionV1::Unreachable
        },
    };

    let mut update_statements = Vec::new();
    let update_value = if matches!(mutation, Mutation::AliasedUpdate) {
        update_statements.push(assign(
            place(ALIAS, integer_ty),
            integer_ty,
            SemanticRvalueKindV1::Use(SemanticOperandV1::Move(field_place(
                CHECKED_RESULT,
                0,
                integer_ty,
            ))),
        ));
        copy(ALIAS, integer_ty)
    } else {
        SemanticOperandV1::Move(field_place(CHECKED_RESULT, 0, integer_ty))
    };
    update_statements.push(assign(
        place(INDUCTION, integer_ty),
        integer_ty,
        SemanticRvalueKindV1::Use(update_value),
    ));

    let exit_statements = if matches!(mutation, Mutation::UnrelatedExitNop) {
        vec![SemanticStatementV1::new(
            SemanticSourceProvenanceV1::unavailable(),
            SemanticStatementKindV1::Nop,
        )]
    } else {
        vec![]
    };
    let mut blocks = vec![
        block(seed, 40, vec![initialization], preheader_terminator),
        block(seed, 41, guard_statements, header_terminator),
        block(seed, 42, checked_statements, checked_terminator),
        block(
            seed,
            43,
            update_statements,
            SemanticTerminatorKindV1::Goto(edge(SemanticEdgeRoleV1::Goto, 1)),
        ),
        block(seed, 44, exit_statements, SemanticTerminatorKindV1::Return),
    ];
    if matches!(mutation, Mutation::UnreachableBlock) {
        blocks.push(block(seed, 45, vec![], SemanticTerminatorKindV1::Return));
    }
    if matches!(mutation, Mutation::UnknownEdge) {
        blocks[4] = block(
            seed,
            44,
            vec![],
            SemanticTerminatorKindV1::Goto(edge(SemanticEdgeRoleV1::Goto, 99)),
        );
    }

    let inputs = if matches!(mutation, Mutation::TemporaryBound) {
        vec![]
    } else {
        vec![direct_value(integer_ty)]
    };
    let abi = SemanticFunctionAbiV1::new(
        SemanticAbiIdentityV1::from_sha256(bytes(seed, 60)),
        SemanticLayoutIdentityV1::from_sha256(bytes(seed, 61)),
        SemanticCanonAbiV1::Rust,
        false,
        false,
        inputs,
        direct_value(integer_ty),
    )
    .unwrap();
    let function = SemanticFunctionDeclV1::new(
        SemanticFunctionIdentityV1::from_sha256(bytes(seed, 70)),
        SemanticFunctionRoleV1::KernelRoot,
        SemanticItemDefinitionIdentityV1::from_sha256(bytes(seed, 71)),
        SemanticMonomorphizationIdentityV1::from_sha256(bytes(seed, 72)),
        SemanticGenericTypeArgumentsIdentityV1::from_sha256(bytes(seed, 73)),
        SemanticConstGenericArgumentsIdentityV1::from_sha256(bytes(seed, 74)),
        SemanticSourceProvenanceV1::unavailable(),
        abi,
        locals,
        SemanticBlockIdV1::from_index(0),
        blocks,
    )
    .unwrap();
    (types, function)
}

fn try_admitted(
    seed: u8,
    mutation: Mutation,
) -> Result<AdmittedInertSemanticMirV1, SemanticMirErrorV1> {
    let (types, function) = function(seed, mutation);
    InertSemanticMirRequestV1::new(
        SemanticTargetDataLayoutV1::gfx942(SemanticLayoutIdentityV1::from_sha256(bytes(seed, 80))),
        types,
        vec![],
        vec![],
        vec![],
        vec![function],
        vec![FUNCTION],
    )?
    .admit_current_production(SemanticMirLimitsV1::default())
}

fn admitted(seed: u8, mutation: Mutation) -> AdmittedInertSemanticMirV1 {
    try_admitted(seed, mutation).unwrap_or_else(|error| panic!("{mutation:?}: {error}"))
}

fn certificate_count(mutation: Mutation) -> usize {
    let admitted = admitted(1, mutation);
    analyze_semantic_u32_induction_no_overflow_v1(&admitted, FUNCTION)
        .unwrap()
        .certificates()
        .len()
}

#[test]
fn canonical_guarded_u32_increment_binds_every_exact_identity() {
    let seed = 9;
    let admitted = admitted(seed, Mutation::None);
    let function = &admitted.functions()[0];
    let report = analyze_semantic_u32_induction_no_overflow_v1(&admitted, FUNCTION).unwrap();
    assert_eq!(report.semantic_mir_sha256(), admitted.semantic_sha256());
    assert_eq!(report.function(), FUNCTION);
    assert_eq!(report.function_identity(), function.identity());
    assert_eq!(report.checked_additions_examined(), 1);
    assert!(report.work_units() > 0);
    assert!(!report.grants_authority());
    assert!(!report.authorizes_compiler_transform());

    let [certificate] = report.certificates() else {
        panic!("the canonical loop must produce one certificate");
    };
    assert_eq!(certificate.function_identity(), function.identity());
    assert_eq!(
        certificate.semantic_mir_sha256(),
        admitted.semantic_sha256()
    );
    assert_eq!(certificate.function(), FUNCTION);
    assert_eq!(certificate.induction().local(), INDUCTION);
    assert_eq!(
        certificate.induction().local_identity(),
        function.locals()[INDUCTION.index() as usize].identity()
    );
    assert_eq!(certificate.induction().ty(), U32);
    assert_eq!(
        certificate.induction().type_identity(),
        admitted.types()[U32.index() as usize].identity()
    );
    assert_eq!(certificate.bound().local(), BOUND);
    assert_eq!(certificate.predicate().local(), PREDICATE);
    assert_eq!(certificate.checked_result().local(), CHECKED_RESULT);
    assert_eq!(certificate.preheader().block().index(), 0);
    assert_eq!(certificate.header().block().index(), 1);
    assert_eq!(certificate.body_entry().block().index(), 2);
    assert_eq!(certificate.exit().block().index(), 4);
    assert_eq!(
        certificate.preheader().identity(),
        function.blocks()[0].identity()
    );
    assert_eq!(
        certificate.header().identity(),
        function.blocks()[1].identity()
    );
    assert_eq!(certificate.initialization().statement(), 0);
    assert_eq!(certificate.guard().statement(), 0);
    assert_eq!(certificate.checked_addition().statement(), 0);
    assert_eq!(certificate.update().statement(), 0);
    assert!(certificate.establishes_semantic_no_overflow());
    assert!(!certificate.grants_authority());
    assert!(!certificate.authorizes_compiler_transform());
}

#[test]
fn identities_not_names_select_the_workload() {
    let first = admitted(1, Mutation::None);
    let second = admitted(101, Mutation::None);
    let first_report = analyze_semantic_u32_induction_no_overflow_v1(&first, FUNCTION).unwrap();
    let second_report = analyze_semantic_u32_induction_no_overflow_v1(&second, FUNCTION).unwrap();
    assert_eq!(
        first.functions()[0].role(),
        SemanticFunctionRoleV1::KernelRoot
    );
    assert_eq!(
        second.functions()[0].role(),
        SemanticFunctionRoleV1::KernelRoot
    );
    assert_ne!(
        first_report.function_identity(),
        second_report.function_identity()
    );
    assert_ne!(
        first_report.certificates()[0].induction().local_identity(),
        second_report.certificates()[0].induction().local_identity()
    );
    assert_eq!(first_report.certificates().len(), 1);
    assert_eq!(second_report.certificates().len(), 1);
}

#[test]
fn canonical_digest_distinguishes_same_identity_body_substitution() {
    let first = admitted(17, Mutation::None);
    let second = admitted(17, Mutation::UnrelatedExitNop);
    assert_eq!(
        first.functions()[0].identity(),
        second.functions()[0].identity()
    );
    assert!(
        first.functions()[0]
            .locals()
            .iter()
            .zip(second.functions()[0].locals())
            .all(|(left, right)| left.identity() == right.identity())
    );
    assert!(
        first.functions()[0]
            .blocks()
            .iter()
            .zip(second.functions()[0].blocks())
            .all(|(left, right)| left.identity() == right.identity())
    );
    assert_ne!(first.semantic_sha256(), second.semantic_sha256());

    let first_report = analyze_semantic_u32_induction_no_overflow_v1(&first, FUNCTION).unwrap();
    let second_report = analyze_semantic_u32_induction_no_overflow_v1(&second, FUNCTION).unwrap();
    assert_eq!(first_report.certificates().len(), 1);
    assert_eq!(second_report.certificates().len(), 1);
    assert_ne!(
        first_report.semantic_mir_sha256(),
        second_report.semantic_mir_sha256()
    );
    assert_ne!(
        first_report.certificates()[0].semantic_mir_sha256(),
        second_report.certificates()[0].semantic_mir_sha256()
    );
}

#[test]
fn function_selection_is_exact_and_bounded() {
    let admitted = admitted(1, Mutation::None);
    assert!(matches!(
        analyze_semantic_u32_induction_no_overflow_v1(
            &admitted,
            SemanticFunctionIdV1::from_index(1),
        ),
        Err(SemanticU32InductionAnalysisErrorV1::InvalidModel(_))
    ));
}

#[test]
fn aliases_projections_and_non_u32_places_fail_closed() {
    for mutation in [
        Mutation::AliasGuardInduction,
        Mutation::AliasGuardBound,
        Mutation::ProjectedCheckedInduction,
        Mutation::U64Induction,
        Mutation::SignedI32Induction,
        Mutation::TemporaryBound,
    ] {
        assert_eq!(certificate_count(mutation), 0, "{mutation:?}");
    }
}

#[test]
fn alternate_guards_steps_and_initialization_fail_closed() {
    for mutation in [
        Mutation::LessOrEqualGuard,
        Mutation::ReversedGuard,
        Mutation::StepTwo,
        Mutation::InitialOne,
        Mutation::TrueValueSwitch,
    ] {
        assert_eq!(certificate_count(mutation), 0, "{mutation:?}");
    }
}

#[test]
fn inexact_overflow_assertions_and_writeback_fail_closed() {
    for mutation in [
        Mutation::ExpectedOverflow,
        Mutation::WrongOverflowMessage,
        Mutation::ReachableUnwind,
        Mutation::AliasedUpdate,
    ] {
        assert_eq!(certificate_count(mutation), 0, "{mutation:?}");
    }
}

#[test]
fn reset_forks_ambiguous_definitions_and_alternate_edges_fail_closed() {
    for mutation in [
        Mutation::ResetDefinition,
        Mutation::DuplicatePredicateDefinition,
        Mutation::DuplicateCheckedDefinition,
        Mutation::AlternateBodyEntry,
    ] {
        assert_eq!(certificate_count(mutation), 0, "{mutation:?}");
    }
    let duplicate = admitted(1, Mutation::DuplicateCheckedDefinition);
    let report = analyze_semantic_u32_induction_no_overflow_v1(&duplicate, FUNCTION).unwrap();
    assert_eq!(report.checked_additions_examined(), 2);
    assert!(report.certificates().is_empty());
}

#[test]
fn ill_typed_aliases_and_illegal_edges_fail_at_admission() {
    for mutation in [
        Mutation::CheckedResultDeclarationMismatch,
        Mutation::PredicateDeclarationMismatch,
        Mutation::WrongOverflowCondition,
        Mutation::WrongAssertionTarget,
        Mutation::WrongBodyEdgeRole,
    ] {
        assert!(try_admitted(1, mutation).is_err(), "{mutation:?}");
    }
}

#[test]
fn malformed_cfg_is_rejected_before_candidate_analysis() {
    let unreachable = admitted(1, Mutation::UnreachableBlock);
    let error = analyze_semantic_u32_induction_no_overflow_v1(&unreachable, FUNCTION).unwrap_err();
    assert!(matches!(
        error,
        SemanticU32InductionAnalysisErrorV1::InvalidControlFlow(_)
    ));
    assert!(error.to_string().contains("unreachable block"));

    assert!(try_admitted(1, Mutation::UnknownEdge).is_err());
}

#[test]
fn work_certificate_and_hard_caps_are_enforced() {
    let admitted = admitted(1, Mutation::None);
    assert!(matches!(
        analyze_semantic_u32_induction_no_overflow_with_limits_v1(
            &admitted,
            FUNCTION,
            SemanticU32InductionAnalysisLimitsV1::new(0, 1),
        ),
        Err(SemanticU32InductionAnalysisErrorV1::WorkLimit {
            actual: 5,
            limit: 0
        })
    ));
    assert_eq!(
        analyze_semantic_u32_induction_no_overflow_with_limits_v1(
            &admitted,
            FUNCTION,
            SemanticU32InductionAnalysisLimitsV1::new(MAX_SEMANTIC_U32_INDUCTION_WORK_V1, 0,),
        ),
        Err(SemanticU32InductionAnalysisErrorV1::CertificateLimit {
            actual: 1,
            limit: 0,
        })
    );
    assert!(matches!(
        analyze_semantic_u32_induction_no_overflow_with_limits_v1(
            &admitted,
            FUNCTION,
            SemanticU32InductionAnalysisLimitsV1::new(
                MAX_SEMANTIC_U32_INDUCTION_WORK_V1 + 1,
                MAX_SEMANTIC_U32_INDUCTION_CERTIFICATES_V1,
            ),
        ),
        Err(SemanticU32InductionAnalysisErrorV1::InvalidLimits { .. })
    ));
}

#[test]
fn analysis_is_deterministic_under_the_same_bounded_input() {
    let admitted = admitted(33, Mutation::None);
    let first = analyze_semantic_u32_induction_no_overflow_v1(&admitted, FUNCTION).unwrap();
    let second = analyze_semantic_u32_induction_no_overflow_v1(&admitted, FUNCTION).unwrap();
    assert_eq!(first, second);
}
