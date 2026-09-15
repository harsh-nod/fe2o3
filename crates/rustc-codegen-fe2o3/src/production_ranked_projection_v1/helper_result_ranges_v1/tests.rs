use super::{
    HelperResultRangesV1, MAX_PROJECTED_LOOP_GRAPH_WORK_V1, ProductionRankedProjectionErrorV1,
};
use fe2o3_mir_model::semantic_mir_v1::*;

#[path = "review_tests.rs"]
mod review;

#[path = "primitive_array_tests.rs"]
mod primitive_arrays;

#[path = "borrowed_edges_tests.rs"]
mod borrowed_edges;

const U32: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(0);
const U64: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(1);
const BOOL: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(2);
const I64: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(3);
const OPTION: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(4);
const PAIR: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(5);
const PTR: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(6);

fn types() -> Vec<SemanticTypeDeclV1> {
    [
        SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
            signed: false,
            bits: 32,
        }),
        SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
            signed: false,
            bits: 64,
        }),
        SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Bool),
        SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
            signed: true,
            bits: 64,
        }),
        SemanticTypeShapeV1::enum_type(
            I64,
            vec![
                SemanticEnumVariantV1::new(0, SemanticAggregateTypeV1::new(vec![]).unwrap()),
                SemanticEnumVariantV1::new(1, SemanticAggregateTypeV1::new(vec![U64]).unwrap()),
            ],
        )
        .unwrap(),
        SemanticTypeShapeV1::Tuple(SemanticAggregateTypeV1::new(vec![U64, BOOL]).unwrap()),
        SemanticTypeShapeV1::Pointer(
            SemanticPointerTypeV1::new(
                U32,
                SemanticMutabilityV1::Mutable,
                0,
                64,
                SemanticPointerMetadataV1::None,
            )
            .unwrap(),
        ),
    ]
    .into_iter()
    .enumerate()
    .map(|(i, shape)| {
        SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256([i as u8 + 1; 32]),
            SemanticLayoutIdentityV1::from_sha256([i as u8 + 1; 32]),
            SemanticTypeLayoutV1::new(Some(16), 8).unwrap(),
            shape,
        )
    })
    .collect()
}

fn place(local: u32, ty: SemanticTypeIdV1) -> SemanticPlaceV1 {
    SemanticPlaceV1::new(SemanticLocalIdV1::from_index(local), vec![], ty).unwrap()
}
fn copy(local: u32, ty: SemanticTypeIdV1) -> SemanticOperandV1 {
    SemanticOperandV1::Copy(place(local, ty))
}
fn moved(local: u32, ty: SemanticTypeIdV1) -> SemanticOperandV1 {
    SemanticOperandV1::Move(place(local, ty))
}
fn constant(ty: SemanticTypeIdV1, value: u128) -> SemanticOperandV1 {
    SemanticOperandV1::Constant(SemanticConstantV1::new(
        ty,
        SemanticConstantValueV1::Scalar(
            SemanticScalarValueV1::new(
                value,
                if ty == U32 {
                    4
                } else if ty == BOOL {
                    1
                } else {
                    8
                },
            )
            .unwrap(),
        ),
    ))
}
fn field(local: u32, index: u32, ty: SemanticTypeIdV1) -> SemanticOperandV1 {
    SemanticOperandV1::Move(
        SemanticPlaceV1::new(
            SemanticLocalIdV1::from_index(local),
            vec![SemanticProjectionV1::new(SemanticProjectionKindV1::Field(index), ty).unwrap()],
            ty,
        )
        .unwrap(),
    )
}
fn assign(local: u32, ty: SemanticTypeIdV1, kind: SemanticRvalueKindV1) -> SemanticStatementV1 {
    SemanticStatementV1::new(
        SemanticSourceProvenanceV1::unavailable(),
        SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
            place(local, ty),
            SemanticRvalueV1::new(ty, kind),
        )),
    )
}
fn statement(kind: SemanticStatementKindV1) -> SemanticStatementV1 {
    SemanticStatementV1::new(SemanticSourceProvenanceV1::unavailable(), kind)
}
fn edge(role: SemanticEdgeRoleV1, block: u32) -> SemanticControlFlowEdgeV1 {
    SemanticControlFlowEdgeV1::new(role, SemanticBlockIdV1::from_index(block))
}
fn goto(block: u32) -> SemanticTerminatorKindV1 {
    SemanticTerminatorKindV1::Goto(edge(SemanticEdgeRoleV1::Goto, block))
}
fn switch(local: u32, ty: SemanticTypeIdV1, zero: u32, other: u32) -> SemanticTerminatorKindV1 {
    SemanticTerminatorKindV1::SwitchInt {
        discriminant: moved(local, ty),
        targets: SemanticSwitchTargetsV1::new(
            vec![SemanticSwitchTargetV1::new(
                0,
                edge(SemanticEdgeRoleV1::SwitchValue, zero),
            )],
            edge(SemanticEdgeRoleV1::SwitchOtherwise, other),
        )
        .unwrap(),
    }
}
fn block(
    index: u8,
    statements: Vec<SemanticStatementV1>,
    terminator: SemanticTerminatorKindV1,
) -> SemanticBasicBlockV1 {
    SemanticBasicBlockV1::new(
        SemanticBlockIdentityV1::from_sha256([index + 1; 32]),
        SemanticSourceProvenanceV1::unavailable(),
        statements,
        SemanticTerminatorV1::new(SemanticSourceProvenanceV1::unavailable(), terminator),
    )
    .unwrap()
}

#[derive(Clone, Copy, Debug)]
enum Mutation {
    None,
    Swap,
    Signed,
    Width,
    Overwrite,
    Escape,
    Storage,
    WrongVariant,
    Bypass,
    Backedge,
    UnknownCall,
    Unwind,
    DifferentReturn,
}

fn fixture(mutation: Mutation) -> SemanticFunctionDeclV1 {
    let input = if matches!(mutation, Mutation::Signed) {
        I64
    } else {
        U32
    };
    let mut entry = vec![
        assign(2, U32, SemanticRvalueKindV1::Use(constant(U32, 256))),
        assign(
            3,
            BOOL,
            SemanticRvalueKindV1::Binary {
                operation: SemanticBinaryOpV1::Equal,
                left: copy(1, input),
                right: constant(input, 0),
            },
        ),
    ];
    if matches!(mutation, Mutation::Escape) {
        entry.insert(
            0,
            assign(
                13,
                PTR,
                SemanticRvalueKindV1::Borrow {
                    kind: SemanticBorrowKindV1::Shared,
                    place: place(1, input),
                },
            ),
        );
    }
    let (left, right) = if matches!(mutation, Mutation::Swap) {
        (constant(input, 4), copy(1, input))
    } else {
        (
            copy(1, input),
            constant(
                if matches!(mutation, Mutation::Width) {
                    U64
                } else {
                    input
                },
                4,
            ),
        )
    };
    let mut math = Vec::new();
    match mutation {
        Mutation::Overwrite => math.push(assign(
            1,
            input,
            SemanticRvalueKindV1::Use(constant(input, u32::MAX.into())),
        )),
        Mutation::Storage => math.push(statement(SemanticStatementKindV1::StorageDead(
            SemanticLocalIdV1::from_index(1),
        ))),
        _ => {}
    }
    math.extend([
        assign(
            4,
            U32,
            SemanticRvalueKindV1::Binary {
                operation: SemanticBinaryOpV1::Divide,
                left: constant(U32, 256),
                right: copy(2, U32),
            },
        ),
        assign(
            5,
            U64,
            SemanticRvalueKindV1::Cast {
                kind: SemanticCastKindV1::Integer,
                operand: copy(1, input),
            },
        ),
        assign(
            6,
            U64,
            SemanticRvalueKindV1::Cast {
                kind: SemanticCastKindV1::Integer,
                operand: moved(4, U32),
            },
        ),
    ]);
    let checked = |a, b| {
        SemanticRvalueKindV1::CheckedBinary(SemanticCheckedBinaryRvalueV1::new(
            SemanticCheckedBinaryOpV1::Multiply,
            a,
            b,
        ))
    };
    let mut success = vec![
        assign(7, PAIR, checked(copy(5, U64), copy(6, U64))),
        assign(3, BOOL, SemanticRvalueKindV1::Use(field(7, 1, BOOL))),
    ];
    if matches!(mutation, Mutation::DifferentReturn) {
        success.push(assign(
            5,
            U64,
            SemanticRvalueKindV1::Use(constant(U64, u64::MAX.into())),
        ));
    }
    let some_value = if matches!(mutation, Mutation::DifferentReturn) {
        copy(5, U64)
    } else {
        field(7, 0, U64)
    };
    let payload = SemanticOperandV1::Move(
        SemanticPlaceV1::new(
            SemanticLocalIdV1::from_index(9),
            vec![
                SemanticProjectionV1::new(
                    SemanticProjectionKindV1::Downcast(
                        if matches!(mutation, Mutation::WrongVariant) {
                            0
                        } else {
                            1
                        },
                    ),
                    OPTION,
                )
                .unwrap(),
                SemanticProjectionV1::new(SemanticProjectionKindV1::Field(0), U64).unwrap(),
            ],
            U64,
        )
        .unwrap(),
    );
    let blocks = vec![
        block(0, entry, switch(3, BOOL, 1, 5)),
        block(
            1,
            vec![assign(
                3,
                BOOL,
                SemanticRvalueKindV1::Binary {
                    operation: SemanticBinaryOpV1::GreaterThan,
                    left,
                    right,
                },
            )],
            if matches!(mutation, Mutation::Bypass) {
                goto(2)
            } else {
                switch(3, BOOL, 2, 5)
            },
        ),
        block(
            2,
            math,
            if matches!(mutation, Mutation::UnknownCall | Mutation::Unwind) {
                SemanticTerminatorKindV1::Call(
                    SemanticDirectCallV1::new_callable(
                        SemanticCallableIdV1::from_index(0),
                        vec![],
                        Some(SemanticCallDestinationV1::new(
                            place(5, U64),
                            edge(SemanticEdgeRoleV1::CallReturn, 3),
                        )),
                        if matches!(mutation, Mutation::Unwind) {
                            SemanticUnwindActionV1::Cleanup(edge(SemanticEdgeRoleV1::CallUnwind, 3))
                        } else {
                            SemanticUnwindActionV1::Unreachable
                        },
                    )
                    .unwrap(),
                )
            } else {
                goto(3)
            },
        ),
        block(3, success, switch(3, BOOL, 4, 5)),
        block(
            4,
            vec![assign(
                8,
                OPTION,
                SemanticRvalueKindV1::aggregate(
                    SemanticAggregateKindV1::EnumVariant(1),
                    vec![some_value],
                )
                .unwrap(),
            )],
            goto(6),
        ),
        block(
            5,
            vec![assign(
                8,
                OPTION,
                SemanticRvalueKindV1::aggregate(SemanticAggregateKindV1::EnumVariant(0), vec![])
                    .unwrap(),
            )],
            goto(6),
        ),
        block(
            6,
            vec![
                assign(9, OPTION, SemanticRvalueKindV1::Use(moved(8, OPTION))),
                statement(SemanticStatementKindV1::StorageDead(
                    SemanticLocalIdV1::from_index(8),
                )),
            ],
            goto(7),
        ),
        block(
            7,
            vec![assign(
                10,
                I64,
                SemanticRvalueKindV1::Discriminant(place(9, OPTION)),
            )],
            switch(10, I64, 9, 8),
        ),
        block(
            8,
            vec![
                assign(11, U64, SemanticRvalueKindV1::Use(payload)),
                assign(12, PAIR, checked(copy(11, U64), constant(U64, 256))),
            ],
            SemanticTerminatorKindV1::Assert {
                condition: field(12, 1, BOOL),
                expected: false,
                message: SemanticAssertMessageV1::Overflow {
                    operation: SemanticBinaryOpV1::Multiply,
                    left: copy(11, U64),
                    right: constant(U64, 256),
                },
                target: edge(SemanticEdgeRoleV1::AssertSuccess, 9),
                unwind: SemanticUnwindActionV1::Unreachable,
            },
        ),
        block(
            9,
            vec![],
            if matches!(mutation, Mutation::Backedge) {
                goto(2)
            } else {
                SemanticTerminatorKindV1::Return
            },
        ),
    ];
    let local_types = [
        U64, input, U32, BOOL, U32, U64, U64, PAIR, OPTION, OPTION, I64, U64, PAIR, PTR,
    ];
    let locals = local_types
        .into_iter()
        .enumerate()
        .map(|(i, ty)| {
            SemanticLocalDeclV1::new(
                SemanticLocalIdentityV1::from_sha256([i as u8 + 30; 32]),
                ty,
                if i == 0 {
                    SemanticLocalRoleV1::Return
                } else if i == 1 {
                    SemanticLocalRoleV1::Argument(0)
                } else {
                    SemanticLocalRoleV1::Temporary
                },
                SemanticSourceProvenanceV1::unavailable(),
            )
        })
        .collect();
    SemanticFunctionDeclV1::new(
        SemanticFunctionIdentityV1::from_sha256([11; 32]),
        SemanticFunctionRoleV1::KernelRoot,
        SemanticItemDefinitionIdentityV1::from_sha256([12; 32]),
        SemanticMonomorphizationIdentityV1::from_sha256([13; 32]),
        SemanticGenericTypeArgumentsIdentityV1::from_sha256([14; 32]),
        SemanticConstGenericArgumentsIdentityV1::from_sha256([15; 32]),
        SemanticSourceProvenanceV1::unavailable(),
        SemanticFunctionAbiV1::new(
            SemanticAbiIdentityV1::from_sha256([16; 32]),
            SemanticLayoutIdentityV1::from_sha256([16; 32]),
            SemanticCanonAbiV1::GpuKernel,
            false,
            false,
            vec![SemanticAbiValueV1::new(
                input,
                SemanticAbiPassModeV1::Direct(SemanticAbiValueAttributesV1::plain()),
            )],
            SemanticAbiValueV1::new(U64, SemanticAbiPassModeV1::Ignore),
        )
        .unwrap(),
        locals,
        SemanticBlockIdV1::from_index(0),
        blocks,
    )
    .unwrap()
}

fn use_operand(function: &SemanticFunctionDeclV1) -> &SemanticOperandV1 {
    let SemanticStatementKindV1::Assign(assignment) = function.blocks()[8].statements()[1].kind()
    else {
        panic!()
    };
    let SemanticRvalueKindV1::CheckedBinary(checked) = assignment.value().kind() else {
        panic!()
    };
    checked.left()
}

#[test]
fn helper_result_ranges_join_some_none_and_retain_partial_moves() {
    let types = types();
    let function = fixture(Mutation::None);
    let proof = HelperResultRangesV1::analyze(&types, &function, &mut 0).unwrap();
    let range = proof
        .at(&types, &function, use_operand(&function), 8, 1)
        .unwrap();
    assert_eq!((range.minimum, range.maximum), (1, 4));
}

#[test]
fn helper_result_ranges_reject_guard_type_move_and_mutation_substitutions() {
    let types = types();
    for mutation in [
        Mutation::Swap,
        Mutation::Signed,
        Mutation::Width,
        Mutation::Overwrite,
        Mutation::Escape,
        Mutation::Storage,
        Mutation::WrongVariant,
        Mutation::Bypass,
        Mutation::Backedge,
        Mutation::UnknownCall,
        Mutation::Unwind,
        Mutation::DifferentReturn,
    ] {
        let function = fixture(mutation);
        let proof = HelperResultRangesV1::analyze(&types, &function, &mut 0).unwrap();
        assert!(
            !proof
                .at(&types, &function, use_operand(&function), 8, 1)
                .is_some_and(|range| range.maximum <= 4),
            "{mutation:?}"
        );
    }
}

#[test]
fn helper_result_ranges_are_bound_to_borrowed_body_operand_and_site() {
    let types = types();
    let function = fixture(Mutation::None);
    let proof = HelperResultRangesV1::analyze(&types, &function, &mut 0).unwrap();
    let operand = use_operand(&function);
    assert!(
        proof
            .at(&types, &function, &operand.clone(), 8, 1)
            .is_none()
    );
    assert!(proof.at(&types.clone(), &function, operand, 8, 1).is_none());
    assert!(proof.at(&types, &function.clone(), operand, 8, 1).is_none());
    assert!(proof.at(&types, &function, operand, 8, 0).is_none());
}

#[test]
fn helper_result_ranges_share_existing_work_budget() {
    let types = types();
    let function = fixture(Mutation::None);
    let mut work = MAX_PROJECTED_LOOP_GRAPH_WORK_V1;
    assert!(matches!(
        HelperResultRangesV1::analyze(&types, &function, &mut work),
        Err(ProductionRankedProjectionErrorV1::Unsupported(_))
    ));
    assert!(work > MAX_PROJECTED_LOOP_GRAPH_WORK_V1);
}
