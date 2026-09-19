use super::*;

#[path = "semantic_masked_shift_meter_v1_tests.rs"]
mod metered;

const VALUE: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(0);
const COUNT: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(1);
const BOOL: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(2);
const CAST: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(3);
const POINTER: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(4);
const FUNCTION: SemanticFunctionIdV1 = SemanticFunctionIdV1::from_index(0);

#[derive(Clone, Copy, Debug, Default)]
enum Mutation {
    #[default]
    None,
    WrongMask,
    WrongComparison,
    WrongComparisonOperator,
    WrongMaskOperator,
    ReversedMask,
    WrongMessageDirection,
    WrongMessageCount,
    WrongConsumerCount,
    WrongConsumerLeft,
    FalseExpected,
    MoveComparisonCount,
    MoveMessageCount,
    MoveCountBeforeComparison,
    DeadBeforeMask,
    LiveAfterMask,
    DeadAfterMask,
    DeinitAfterMask,
    DeadInSuccessor,
    DeinitInSuccessor,
    AssumeAfterMask,
    EscapeBeforeMask,
    Bypass,
    DuplicateIncoming,
    Cycle,
    Cleanup,
    WrongCastWidth,
    WrongCastSource,
    WrongConstantSize,
}

#[derive(Clone, Copy)]
struct Shape {
    value_signed: bool,
    value_width: u16,
    count_signed: bool,
    count_width: u16,
    direction: SemanticBinaryOpV1,
    mutation: Mutation,
    seed: u8,
}

impl Default for Shape {
    fn default() -> Self {
        Self {
            value_signed: false,
            value_width: 32,
            count_signed: false,
            count_width: 32,
            direction: SemanticBinaryOpV1::ShiftLeft,
            mutation: Mutation::None,
            seed: 0,
        }
    }
}

fn id(seed: u8, tag: u8) -> [u8; 32] {
    [seed + tag; 32]
}
fn local(index: u32) -> SemanticLocalIdV1 {
    SemanticLocalIdV1::from_index(index)
}
fn block_id(index: u32) -> SemanticBlockIdV1 {
    SemanticBlockIdV1::from_index(index)
}
fn place(index: u32, ty: SemanticTypeIdV1) -> SemanticPlaceV1 {
    SemanticPlaceV1::new(local(index), vec![], ty).unwrap()
}
fn copy(index: u32, ty: SemanticTypeIdV1) -> SemanticOperandV1 {
    SemanticOperandV1::Copy(place(index, ty))
}
fn moved(index: u32, ty: SemanticTypeIdV1) -> SemanticOperandV1 {
    SemanticOperandV1::Move(place(index, ty))
}
fn constant(ty: SemanticTypeIdV1, width: u16, value: u128) -> SemanticOperandV1 {
    SemanticOperandV1::Constant(SemanticConstantV1::new(
        ty,
        SemanticConstantValueV1::Scalar(
            SemanticScalarValueV1::new(value, (width / 8) as u8).unwrap(),
        ),
    ))
}
fn statement(kind: SemanticStatementKindV1) -> SemanticStatementV1 {
    SemanticStatementV1::new(SemanticSourceProvenanceV1::unavailable(), kind)
}
fn assign(index: u32, ty: SemanticTypeIdV1, value: SemanticRvalueKindV1) -> SemanticStatementV1 {
    statement(SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
        place(index, ty),
        SemanticRvalueV1::new(ty, value),
    )))
}
fn edge(role: SemanticEdgeRoleV1, target: u32) -> SemanticControlFlowEdgeV1 {
    SemanticControlFlowEdgeV1::new(role, block_id(target))
}
fn block(
    seed: u8,
    tag: u8,
    statements: Vec<SemanticStatementV1>,
    terminator: SemanticTerminatorKindV1,
) -> SemanticBasicBlockV1 {
    SemanticBasicBlockV1::new(
        SemanticBlockIdentityV1::from_sha256(id(seed, tag)),
        SemanticSourceProvenanceV1::unavailable(),
        statements,
        SemanticTerminatorV1::new(SemanticSourceProvenanceV1::unavailable(), terminator),
    )
    .unwrap()
}
fn layout(
    width: u16,
    primitive: SemanticBackendPrimitiveV1,
    maximum: u128,
) -> SemanticTypeLayoutV1 {
    SemanticTypeLayoutV1::new_with_backend_repr(
        Some(u64::from(width / 8)),
        u64::from(width / 8),
        SemanticBackendReprV1::scalar(SemanticBackendScalarV1::initialized(
            primitive,
            SemanticScalarValidityRangeV1::new(0, maximum),
        )),
        false,
    )
    .unwrap()
}
fn integer_type(seed: u8, tag: u8, signed: bool, width: u16) -> SemanticTypeDeclV1 {
    SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256(id(seed, tag)),
        SemanticLayoutIdentityV1::from_sha256(id(seed, tag + 1)),
        layout(
            width,
            SemanticBackendPrimitiveV1::integer(signed, width, u64::from(width / 8)),
            (1_u128 << width) - 1,
        ),
        SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
            signed,
            bits: width,
        }),
    )
}
fn direct(ty: SemanticTypeIdV1) -> SemanticAbiValueV1 {
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

fn fixture(shape: Shape) -> Result<AdmittedInertSemanticMirV1, SemanticMirErrorV1> {
    let cast_width = if matches!(shape.mutation, Mutation::WrongCastWidth) {
        16
    } else {
        shape.count_width
    };
    let types = vec![
        integer_type(shape.seed, 1, shape.value_signed, shape.value_width),
        integer_type(shape.seed, 3, shape.count_signed, shape.count_width),
        SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256(id(shape.seed, 5)),
            SemanticLayoutIdentityV1::from_sha256(id(shape.seed, 6)),
            layout(8, SemanticBackendPrimitiveV1::integer(false, 8, 1), 1),
            SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Bool),
        ),
        integer_type(shape.seed, 7, false, cast_width),
        SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256(id(shape.seed, 9)),
            SemanticLayoutIdentityV1::from_sha256(id(shape.seed, 10)),
            layout(
                64,
                SemanticBackendPrimitiveV1::pointer(0, 8, 8),
                u128::from(u64::MAX),
            ),
            SemanticTypeShapeV1::Pointer(
                SemanticPointerTypeV1::new(
                    COUNT,
                    SemanticMutabilityV1::Mutable,
                    0,
                    64,
                    SemanticPointerMetadataV1::None,
                )
                .unwrap(),
            ),
        ),
    ];
    let locals = [VALUE, VALUE, COUNT, COUNT, BOOL, CAST, COUNT, POINTER]
        .into_iter()
        .enumerate()
        .map(|(index, ty)| {
            SemanticLocalDeclV1::new(
                SemanticLocalIdentityV1::from_sha256(id(shape.seed, 20 + index as u8)),
                ty,
                match index {
                    0 => SemanticLocalRoleV1::Return,
                    1 => SemanticLocalRoleV1::Argument(0),
                    2 => SemanticLocalRoleV1::Argument(1),
                    _ => SemanticLocalRoleV1::Temporary,
                },
                SemanticSourceProvenanceV1::unavailable(),
            )
        })
        .collect();
    let mut statements = vec![statement(SemanticStatementKindV1::StorageLive(local(3)))];
    if matches!(shape.mutation, Mutation::DeadBeforeMask) {
        statements.push(statement(SemanticStatementKindV1::StorageDead(local(3))));
    }
    if matches!(shape.mutation, Mutation::EscapeBeforeMask) {
        statements.push(assign(
            3,
            COUNT,
            SemanticRvalueKindV1::Use(constant(COUNT, shape.count_width, 0)),
        ));
        statements.push(assign(
            7,
            POINTER,
            SemanticRvalueKindV1::AddressOf {
                mutability: SemanticMutabilityV1::Mutable,
                place: place(3, COUNT),
            },
        ));
    }
    let mask = u128::from(
        shape.value_width
            - if matches!(shape.mutation, Mutation::WrongMask) {
                2
            } else {
                1
            },
    );
    let mask_size = if matches!(shape.mutation, Mutation::WrongConstantSize) {
        64
    } else {
        shape.count_width
    };
    let (input, limit) = if matches!(shape.mutation, Mutation::ReversedMask) {
        (constant(COUNT, mask_size, mask), copy(2, COUNT))
    } else {
        (copy(2, COUNT), constant(COUNT, mask_size, mask))
    };
    statements.push(assign(
        3,
        COUNT,
        SemanticRvalueKindV1::Binary {
            operation: if matches!(shape.mutation, Mutation::WrongMaskOperator) {
                SemanticBinaryOpV1::BitOr
            } else {
                SemanticBinaryOpV1::BitAnd
            },
            left: input,
            right: limit,
        },
    ));
    match shape.mutation {
        Mutation::LiveAfterMask => {
            statements.push(statement(SemanticStatementKindV1::StorageLive(local(3))))
        }
        Mutation::DeadAfterMask => {
            statements.push(statement(SemanticStatementKindV1::StorageDead(local(3))))
        }
        Mutation::DeinitAfterMask => statements.push(statement(
            SemanticStatementKindV1::Deinitialize(place(3, COUNT)),
        )),
        Mutation::AssumeAfterMask => statements.push(statement(SemanticStatementKindV1::Assume(
            constant(BOOL, 8, 1),
        ))),
        Mutation::MoveCountBeforeComparison => {
            statements.push(assign(6, COUNT, SemanticRvalueKindV1::Use(moved(3, COUNT))))
        }
        _ => {}
    }
    let (compared_local, compared_ty, compared_width) = if shape.count_signed {
        statements.push(assign(
            5,
            CAST,
            SemanticRvalueKindV1::Cast {
                kind: SemanticCastKindV1::Integer,
                operand: copy(
                    if matches!(shape.mutation, Mutation::WrongCastSource) {
                        2
                    } else {
                        3
                    },
                    COUNT,
                ),
            },
        ));
        (5, CAST, cast_width)
    } else {
        (3, COUNT, shape.count_width)
    };
    statements.push(assign(
        4,
        BOOL,
        SemanticRvalueKindV1::Binary {
            operation: if matches!(shape.mutation, Mutation::WrongComparisonOperator) {
                SemanticBinaryOpV1::LessOrEqual
            } else {
                SemanticBinaryOpV1::LessThan
            },
            left: if matches!(shape.mutation, Mutation::MoveComparisonCount) {
                moved(compared_local, compared_ty)
            } else {
                copy(compared_local, compared_ty)
            },
            right: constant(
                compared_ty,
                compared_width,
                u128::from(
                    shape.value_width
                        + u16::from(matches!(shape.mutation, Mutation::WrongComparison)),
                ),
            ),
        },
    ));
    let assertion = SemanticTerminatorKindV1::Assert {
        condition: moved(4, BOOL),
        expected: !matches!(shape.mutation, Mutation::FalseExpected),
        message: SemanticAssertMessageV1::Overflow {
            operation: if matches!(shape.mutation, Mutation::WrongMessageDirection) {
                SemanticBinaryOpV1::ShiftRight
            } else {
                shape.direction
            },
            left: copy(1, VALUE),
            right: if matches!(shape.mutation, Mutation::MoveMessageCount) {
                moved(3, COUNT)
            } else {
                copy(
                    if matches!(shape.mutation, Mutation::WrongMessageCount) {
                        2
                    } else {
                        3
                    },
                    COUNT,
                )
            },
        },
        target: edge(SemanticEdgeRoleV1::AssertSuccess, 1),
        unwind: if matches!(shape.mutation, Mutation::Cleanup) {
            SemanticUnwindActionV1::Terminate
        } else {
            SemanticUnwindActionV1::Unreachable
        },
    };
    let mut successor = vec![];
    match shape.mutation {
        Mutation::DeadInSuccessor => {
            successor.push(statement(SemanticStatementKindV1::StorageDead(local(3))))
        }
        Mutation::DeinitInSuccessor => successor.push(statement(
            SemanticStatementKindV1::Deinitialize(place(3, COUNT)),
        )),
        _ => {}
    }
    successor.push(assign(
        0,
        VALUE,
        SemanticRvalueKindV1::Binary {
            operation: shape.direction,
            left: if matches!(shape.mutation, Mutation::WrongConsumerLeft) {
                copy(0, VALUE)
            } else {
                copy(1, VALUE)
            },
            right: moved(
                if matches!(shape.mutation, Mutation::WrongConsumerCount) {
                    2
                } else {
                    3
                },
                COUNT,
            ),
        },
    ));
    successor.push(statement(SemanticStatementKindV1::StorageDead(local(3))));
    let mut blocks = vec![
        block(shape.seed, 40, statements, assertion),
        block(
            shape.seed,
            41,
            successor,
            if matches!(shape.mutation, Mutation::Cycle) {
                SemanticTerminatorKindV1::Goto(edge(SemanticEdgeRoleV1::Goto, 0))
            } else {
                SemanticTerminatorKindV1::Return
            },
        ),
    ];
    if matches!(shape.mutation, Mutation::Bypass) {
        blocks.push(block(
            shape.seed,
            42,
            vec![],
            SemanticTerminatorKindV1::Goto(edge(SemanticEdgeRoleV1::Goto, 1)),
        ));
    }
    if matches!(shape.mutation, Mutation::DuplicateIncoming) {
        blocks.push(block(
            shape.seed,
            42,
            vec![],
            SemanticTerminatorKindV1::SwitchInt {
                discriminant: copy(2, COUNT),
                targets: SemanticSwitchTargetsV1::new(
                    vec![SemanticSwitchTargetV1::new(
                        0,
                        edge(SemanticEdgeRoleV1::SwitchValue, 1),
                    )],
                    edge(SemanticEdgeRoleV1::SwitchOtherwise, 1),
                )
                .unwrap(),
            },
        ));
    }
    let function = SemanticFunctionDeclV1::new(
        SemanticFunctionIdentityV1::from_sha256(id(shape.seed, 50)),
        SemanticFunctionRoleV1::KernelRoot,
        SemanticItemDefinitionIdentityV1::from_sha256(id(shape.seed, 51)),
        SemanticMonomorphizationIdentityV1::from_sha256(id(shape.seed, 52)),
        SemanticGenericTypeArgumentsIdentityV1::from_sha256(id(shape.seed, 53)),
        SemanticConstGenericArgumentsIdentityV1::from_sha256(id(shape.seed, 54)),
        SemanticSourceProvenanceV1::unavailable(),
        SemanticFunctionAbiV1::new(
            SemanticAbiIdentityV1::from_sha256(id(shape.seed, 55)),
            SemanticLayoutIdentityV1::from_sha256(id(shape.seed, 56)),
            SemanticCanonAbiV1::Rust,
            false,
            false,
            vec![direct(VALUE), direct(COUNT)],
            direct(VALUE),
        )
        .unwrap(),
        locals,
        block_id(0),
        blocks,
    )
    .unwrap();
    InertSemanticMirRequestV1::new(
        SemanticTargetDataLayoutV1::gfx942(SemanticLayoutIdentityV1::from_sha256(id(
            shape.seed, 57,
        ))),
        types,
        vec![],
        vec![],
        vec![],
        vec![function],
        vec![FUNCTION],
    )?
    .admit_current_production(SemanticMirLimitsV1::default())
}

fn index(owner: &AdmittedInertSemanticMirV1) -> SemanticMaskedShiftIndexV1<'_> {
    SemanticMaskedShiftIndexV1::analyze(owner, FUNCTION, SemanticMaskedShiftLimitsV1::default())
        .unwrap()
}

#[test]
fn all_fixed_widths_and_signed_count_transport_bind_the_actual_owner() {
    for value_width in [8, 16, 32, 64] {
        for count_width in [8, 16, 32, 64] {
            for value_signed in [false, true] {
                for count_signed in [false, true] {
                    for direction in [
                        SemanticBinaryOpV1::ShiftLeft,
                        SemanticBinaryOpV1::ShiftRight,
                    ] {
                        let owner = fixture(Shape {
                            value_width,
                            count_width,
                            value_signed,
                            count_signed,
                            direction,
                            ..Shape::default()
                        })
                        .unwrap();
                        let mut query = index(&owner);
                        let fact = query.assertion(block_id(0)).unwrap().unwrap();
                        assert!(std::ptr::eq(fact.owner(), &owner));
                        assert_eq!(fact.function(), FUNCTION);
                        assert_eq!(fact.mask_statement(), 1);
                        assert_eq!(fact.cast_statement(), count_signed.then_some(2));
                        assert_eq!(
                            fact.comparison_statement(),
                            if count_signed { 3 } else { 2 }
                        );
                        assert_eq!(fact.successor_block(), block_id(1));
                        assert_eq!(fact.shift_statement(), 0);
                        assert_eq!(
                            query
                                .shift(block_id(1), 0)
                                .unwrap()
                                .unwrap()
                                .assertion_block(),
                            block_id(0)
                        );
                        assert!(query.shift(block_id(1), 1).unwrap().is_none());
                    }
                }
            }
        }
    }
}

#[test]
fn hostile_source_occurrences_do_not_produce_facts() {
    for mutation in [
        Mutation::WrongMask,
        Mutation::WrongComparison,
        Mutation::WrongComparisonOperator,
        Mutation::WrongMaskOperator,
        Mutation::ReversedMask,
        Mutation::WrongMessageDirection,
        Mutation::WrongMessageCount,
        Mutation::WrongConsumerCount,
        Mutation::WrongConsumerLeft,
        Mutation::FalseExpected,
        Mutation::MoveComparisonCount,
        Mutation::MoveMessageCount,
        Mutation::MoveCountBeforeComparison,
        Mutation::DeadBeforeMask,
        Mutation::LiveAfterMask,
        Mutation::DeadAfterMask,
        Mutation::DeinitAfterMask,
        Mutation::DeadInSuccessor,
        Mutation::DeinitInSuccessor,
        Mutation::AssumeAfterMask,
        Mutation::EscapeBeforeMask,
        Mutation::Bypass,
        Mutation::DuplicateIncoming,
        Mutation::Cycle,
        Mutation::Cleanup,
    ] {
        let owner = fixture(Shape {
            mutation,
            ..Shape::default()
        })
        .unwrap();
        let mut query = index(&owner);
        assert!(
            query.assertion(block_id(0)).unwrap().is_none(),
            "{mutation:?}"
        );
        assert!(
            query.shift(block_id(1), 0).unwrap().is_none(),
            "{mutation:?}"
        );
    }
}

#[test]
fn cast_width_and_source_must_be_exact() {
    for mutation in [Mutation::WrongCastWidth, Mutation::WrongCastSource] {
        let owner = fixture(Shape {
            count_signed: true,
            mutation,
            ..Shape::default()
        })
        .unwrap();
        assert!(
            index(&owner).assertion(block_id(0)).unwrap().is_none(),
            "{mutation:?}"
        );
    }
}

#[test]
fn malformed_scalar_width_never_enters_the_admitted_source_query() {
    assert!(
        fixture(Shape {
            mutation: Mutation::WrongConstantSize,
            ..Shape::default()
        })
        .is_err()
    );
}

#[test]
fn constructor_and_queries_share_exact_work_and_storage_limits() {
    let owner = fixture(Shape::default()).unwrap();
    let query = index(&owner);
    let (work, storage) = (query.work_units(), query.storage_bytes());
    drop(query);
    let mut exact = SemanticMaskedShiftIndexV1::analyze(
        &owner,
        FUNCTION,
        SemanticMaskedShiftLimitsV1::new(work + 3, storage),
    )
    .unwrap();
    assert!(exact.assertion(block_id(0)).unwrap().is_some());
    assert!(exact.shift(block_id(1), 0).unwrap().is_some());
    assert_eq!(exact.work_units(), work + 3);
    assert!(
        matches!(exact.assertion(block_id(0)), Err(SemanticMaskedShiftErrorV1::WorkLimit { actual, limit }) if actual == work + 4 && limit == work + 3)
    );
    assert!(matches!(
        SemanticMaskedShiftIndexV1::analyze(
            &owner,
            FUNCTION,
            SemanticMaskedShiftLimitsV1::new(work - 1, storage)
        ),
        Err(SemanticMaskedShiftErrorV1::WorkLimit { .. })
    ));
    assert!(matches!(
        SemanticMaskedShiftIndexV1::analyze(
            &owner,
            FUNCTION,
            SemanticMaskedShiftLimitsV1::new(work, storage - 1)
        ),
        Err(SemanticMaskedShiftErrorV1::StorageLimit { .. })
    ));
    assert!(
        SemanticMaskedShiftIndexV1::analyze(
            &owner,
            FUNCTION,
            SemanticMaskedShiftLimitsV1::new(work, storage)
        )
        .is_ok()
    );
}

#[test]
fn invalid_limits_and_foreign_coordinates_remain_refused() {
    let owner = fixture(Shape::default()).unwrap();
    assert!(matches!(
        SemanticMaskedShiftIndexV1::analyze(
            &owner,
            FUNCTION,
            SemanticMaskedShiftLimitsV1::new(MAX_SEMANTIC_MASKED_SHIFT_WORK_V1 + 1, 0)
        ),
        Err(SemanticMaskedShiftErrorV1::InvalidLimits)
    ));
    assert!(matches!(
        SemanticMaskedShiftIndexV1::analyze(
            &owner,
            SemanticFunctionIdV1::from_index(1),
            SemanticMaskedShiftLimitsV1::default()
        ),
        Err(SemanticMaskedShiftErrorV1::InvalidModel(_))
    ));
    let foreign = fixture(Shape {
        seed: 100,
        ..Shape::default()
    })
    .unwrap();
    assert_ne!(owner.semantic_sha256(), foreign.semantic_sha256());
    let mut query = index(&owner);
    assert!(query.assertion(block_id(2)).unwrap().is_none());
    assert!(query.shift(block_id(0), 0).unwrap().is_none());
    assert!(std::ptr::eq(
        query.assertion(block_id(0)).unwrap().unwrap().owner(),
        &owner
    ));
    assert!(!std::ptr::eq(query.owner(), &foreign));
}

#[test]
fn actual_capacity_excess_is_checked_without_changing_rejected_accounting() {
    let mut budget = Budget {
        limits: SemanticMaskedShiftLimitsV1::new(100, 32),
        work: 0,
        storage: 16,
    };
    budget.account_capacity::<u64>(2, 4).unwrap();
    assert_eq!(budget.storage, 32);
    assert_eq!(budget.work, 0);
    budget.account_capacity::<u64>(4, 4).unwrap();
    assert_eq!(budget.storage, 32);
    assert!(matches!(
        budget.account_capacity::<u64>(2, 3),
        Err(SemanticMaskedShiftErrorV1::StorageLimit {
            actual: 40,
            limit: 32
        })
    ));
    assert_eq!(budget.storage, 32);
    assert_eq!(
        budget.account_capacity::<u64>(2, 1),
        Err(SemanticMaskedShiftErrorV1::Allocation)
    );
    assert_eq!(
        budget.account_capacity::<u64>(0, usize::MAX),
        Err(SemanticMaskedShiftErrorV1::Allocation)
    );
    assert_eq!(budget.storage, 32);
}
