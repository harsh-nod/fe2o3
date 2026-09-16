#[derive(Clone, Copy, Debug)]
enum LiteralJoinHostilityV1 {
    Exact,
    Missing,
    Unknown,
    WrongType,
    WrongBytes,
    Huge,
    Overwrite,
    Escaped,
    Cycle,
    Dead,
    Store,
    Call,
    KilledUnknown,
}

fn literal_join_function_v1(hostility: LiteralJoinHostilityV1) -> SemanticFunctionDeclV1 {
    let mut entry = Vec::new();
    if matches!(hostility, LiteralJoinHostilityV1::Escaped) {
        entry.push(typed_assignment(
            3,
            U64_POINTER_TYPE,
            SemanticRvalueKindV1::Borrow {
                kind: SemanticBorrowKindV1::Mutable,
                place: typed_place(2, U64_TYPE),
            },
        ));
    }
    let mut blocks = vec![block(
        100,
        entry,
        SemanticTerminatorKindV1::SwitchInt {
            discriminant: typed_operand(1, U64_TYPE),
            targets: SemanticSwitchTargetsV1::new(
                (0..6)
                    .map(|index| {
                        SemanticSwitchTargetV1::new(
                            index,
                            cfg_edge(SemanticEdgeRoleV1::SwitchValue, index as u32 + 1),
                        )
                    })
                    .collect(),
                cfg_edge(SemanticEdgeRoleV1::SwitchOtherwise, 7),
            )
            .unwrap(),
        },
    )];
    for index in 0..7 {
        let mut statements = vec![typed_assignment(
            2,
            U64_TYPE,
            SemanticRvalueKindV1::Use(typed_constant(U64_TYPE, index * 128, 8)),
        )];
        let mut terminator = SemanticTerminatorKindV1::Goto(cfg_edge(SemanticEdgeRoleV1::Goto, 8));
        if index == 6 {
            let unknown = typed_assignment(
                2,
                U64_TYPE,
                SemanticRvalueKindV1::Use(typed_operand(1, U64_TYPE)),
            );
            match hostility {
                LiteralJoinHostilityV1::Missing => statements.clear(),
                LiteralJoinHostilityV1::Unknown => statements = vec![unknown],
                LiteralJoinHostilityV1::WrongType => {
                    statements = vec![typed_assignment(
                        2,
                        U64_TYPE,
                        SemanticRvalueKindV1::Use(typed_constant(U8_TYPE, 7, 1)),
                    )]
                }
                LiteralJoinHostilityV1::WrongBytes => {
                    statements = vec![typed_assignment(
                        2,
                        U64_TYPE,
                        SemanticRvalueKindV1::Use(typed_constant(U64_TYPE, 768, 4)),
                    )]
                }
                LiteralJoinHostilityV1::Huge => {
                    statements = vec![typed_assignment(
                        2,
                        U64_TYPE,
                        SemanticRvalueKindV1::Use(typed_constant(
                            U64_TYPE,
                            u128::from(u64::MAX),
                            8,
                        )),
                    )]
                }
                LiteralJoinHostilityV1::Overwrite => statements.push(unknown),
                LiteralJoinHostilityV1::KilledUnknown => statements.insert(0, unknown),
                LiteralJoinHostilityV1::Cycle => {
                    statements.clear();
                    terminator = zero_switch(1, U64_TYPE, 7, 8);
                }
                LiteralJoinHostilityV1::Dead => statements.push(statement(
                    SemanticStatementKindV1::StorageDead(SemanticLocalIdV1::from_index(2)),
                )),
                LiteralJoinHostilityV1::Store => statements.push(statement(
                    SemanticStatementKindV1::Store(SemanticMemoryStoreV1::new(
                        typed_place(2, U64_TYPE),
                        typed_constant(U64_TYPE, 3, 8),
                        SemanticVolatilityV1::NonVolatile,
                        None,
                    )),
                )),
                LiteralJoinHostilityV1::Call => {
                    terminator = SemanticTerminatorKindV1::Call(
                        SemanticDirectCallV1::new_callable(
                            SemanticCallableIdV1::from_index(0),
                            vec![],
                            Some(SemanticCallDestinationV1::new(
                                typed_place(2, U64_TYPE),
                                cfg_edge(SemanticEdgeRoleV1::CallReturn, 8),
                            )),
                            SemanticUnwindActionV1::Unreachable,
                        )
                        .unwrap(),
                    )
                }
                LiteralJoinHostilityV1::Exact | LiteralJoinHostilityV1::Escaped => {}
            }
        }
        blocks.push(block(101 + index as u8, statements, terminator));
    }
    blocks.push(block(
        108,
        vec![typed_assignment(
            4,
            CHECKED_U64_TYPE,
            SemanticRvalueKindV1::CheckedBinary(SemanticCheckedBinaryRvalueV1::new(
                SemanticCheckedBinaryOpV1::Add,
                typed_operand(2, U64_TYPE),
                typed_constant(U64_TYPE, 127, 8),
            )),
        )],
        checked_overflow_terminator(
            4,
            SemanticBinaryOpV1::Add,
            typed_operand(2, U64_TYPE),
            typed_constant(U64_TYPE, 127, 8),
            9,
        ),
    ));
    blocks.push(block(109, vec![], SemanticTerminatorKindV1::Return));
    projection_function_with_locals(
        blocks,
        vec![
            local(100, SCALAR_TYPE, SemanticLocalRoleV1::Return),
            local(101, U64_TYPE, SemanticLocalRoleV1::Argument(0)),
            local(102, U64_TYPE, SemanticLocalRoleV1::Temporary),
            local(103, U64_POINTER_TYPE, SemanticLocalRoleV1::Temporary),
            local(104, CHECKED_U64_TYPE, SemanticLocalRoleV1::Temporary),
        ],
    )
}

#[test]
fn literal_join_preserves_all_seven_row_bases_and_latest_definitions() {
    let types = assertion_proof_types();
    for shape in [
        LiteralJoinHostilityV1::Exact,
        LiteralJoinHostilityV1::KilledUnknown,
    ] {
        let function = literal_join_function_v1(shape);
        let mut proof = SemanticAssertProofsV1::new(&types, &function).unwrap();
        assert_eq!(
            proof
                .range_at_operand(&typed_operand(2, U64_TYPE), 8, 0)
                .unwrap(),
            Some(UnsignedRangeProofV1 {
                minimum: 0,
                maximum: 768
            })
        );
        assert!(
            SemanticAssertProofsV1::analyze(&types, &function).unwrap()[8],
            "{shape:?}"
        );
    }
}

#[test]
fn literal_join_requires_every_reaching_definition_and_exact_scalar_types() {
    let types = assertion_proof_types();
    for shape in [
        LiteralJoinHostilityV1::Missing,
        LiteralJoinHostilityV1::Unknown,
        LiteralJoinHostilityV1::WrongType,
        LiteralJoinHostilityV1::WrongBytes,
        LiteralJoinHostilityV1::Overwrite,
        LiteralJoinHostilityV1::Escaped,
        LiteralJoinHostilityV1::Cycle,
        LiteralJoinHostilityV1::Dead,
        LiteralJoinHostilityV1::Store,
        LiteralJoinHostilityV1::Call,
    ] {
        let function = literal_join_function_v1(shape);
        let mut proof = SemanticAssertProofsV1::new(&types, &function).unwrap();
        assert_eq!(
            proof
                .reaching_scalar_literal_range_v1(
                    2,
                    ScalarAssignmentSiteV1 {
                        block: 8,
                        statement: 0
                    }
                )
                .unwrap(),
            None,
            "{shape:?}"
        );
        assert!(
            !SemanticAssertProofsV1::analyze(&types, &function).unwrap()[8],
            "{shape:?}"
        );
    }
}

#[test]
fn literal_join_keeps_overflow_and_work_budget_fail_closed() {
    let types = assertion_proof_types();
    let function = literal_join_function_v1(LiteralJoinHostilityV1::Huge);
    assert!(!SemanticAssertProofsV1::analyze(&types, &function).unwrap()[8]);
    let mut proof = SemanticAssertProofsV1::new(&types, &function).unwrap();
    proof.work = MAX_PROJECTED_LOOP_GRAPH_WORK_V1;
    assert!(matches!(
        proof.reaching_scalar_literal_range_v1(
            2,
            ScalarAssignmentSiteV1 {
                block: 8,
                statement: 0
            }
        ),
        Err(ProductionRankedProjectionErrorV1::Unsupported(
            "uniform induction CFG analysis exceeds its work limit"
        ))
    ));
}

fn ordered_zero_function_v1(
    operation: SemanticBinaryOpV1,
    reversed: bool,
    ty: SemanticTypeIdV1,
    constant: u128,
    mutation: bool,
) -> SemanticFunctionDeclV1 {
    let zero = typed_constant(ty, constant, if ty == U64_TYPE { 8 } else { 4 });
    let value = typed_operand(1, ty);
    let (left, right) = if reversed {
        (zero, value)
    } else {
        (value, zero)
    };
    let overwrite = if mutation {
        vec![typed_assignment(
            1,
            ty,
            SemanticRvalueKindV1::Use(typed_constant(ty, 0, if ty == U64_TYPE { 8 } else { 4 })),
        )]
    } else {
        vec![]
    };
    projection_function_with_locals(
        vec![
            block(
                110,
                vec![typed_assignment(
                    2,
                    BOOL_TYPE,
                    SemanticRvalueKindV1::Binary {
                        operation,
                        left,
                        right,
                    },
                )],
                zero_switch(2, BOOL_TYPE, 1, 2),
            ),
            block(111, overwrite.clone(), SemanticTerminatorKindV1::Return),
            block(112, overwrite, SemanticTerminatorKindV1::Return),
        ],
        vec![
            local(110, SCALAR_TYPE, SemanticLocalRoleV1::Return),
            local(111, ty, SemanticLocalRoleV1::Argument(0)),
            local(112, BOOL_TYPE, SemanticLocalRoleV1::Temporary),
        ],
    )
}

#[test]
fn ordered_unsigned_zero_edges_have_exact_truth_and_reversed_polarities() {
    use SemanticBinaryOpV1::{Equal, GreaterOrEqual, GreaterThan, LessOrEqual, LessThan, NotEqual};
    let types = assertion_proof_types();
    for (operation, reversed, excludes) in [
        (Equal, false, Some(1)),
        (Equal, true, Some(1)),
        (NotEqual, false, Some(2)),
        (NotEqual, true, Some(2)),
        (GreaterThan, false, Some(2)),
        (LessOrEqual, false, Some(1)),
        (LessThan, true, Some(2)),
        (GreaterOrEqual, true, Some(1)),
        (GreaterThan, true, None),
        (LessOrEqual, true, None),
        (LessThan, false, None),
        (GreaterOrEqual, false, None),
    ] {
        let function = ordered_zero_function_v1(operation, reversed, U64_TYPE, 0, false);
        let mut proof = SemanticAssertProofsV1::new(&types, &function).unwrap();
        for block in [1, 2] {
            assert_eq!(
                proof.zero_excluding_edge_dominates(1, block).unwrap(),
                excludes == Some(block),
                "{operation:?} reversed={reversed} block={block}"
            );
        }
    }
}

#[test]
fn ordered_zero_edges_reject_signed_nonzero_constants_and_stale_values() {
    let types = assertion_proof_types();
    for (ty, constant, mutation) in [
        (I32_TYPE, 0, false),
        (U64_TYPE, 1, false),
        (U64_TYPE, 0, true),
    ] {
        let function = ordered_zero_function_v1(
            SemanticBinaryOpV1::GreaterThan,
            false,
            ty,
            constant,
            mutation,
        );
        let mut proof = SemanticAssertProofsV1::new(&types, &function).unwrap();
        assert!(!proof.zero_excluding_edge_dominates(1, 2).unwrap());
    }
}
