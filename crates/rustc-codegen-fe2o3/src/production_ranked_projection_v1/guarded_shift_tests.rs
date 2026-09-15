const GUARDED_SHIFT_ASSERT_BLOCK_V1: usize = 4;

fn guarded_shift_function_v1(operation: SemanticBinaryOpV1) -> SemanticFunctionDeclV1 {
    let copy = |destination, source| {
        typed_assignment(
            destination,
            U64_TYPE,
            SemanticRvalueKindV1::Use(typed_operand(source, U64_TYPE)),
        )
    };
    // Pinned rustc MIR for `if lane >= 64 { trap() }; 1_u64 << lane`
    // checks Lt(lane, 64_usize) directly, without a signed-literal cast.
    // Retain separate source aliases and a diamond between guard and use.
    projection_function_with_locals(
        vec![
            block(
                200,
                vec![],
                SemanticTerminatorKindV1::Goto(cfg_edge(SemanticEdgeRoleV1::Goto, 1)),
            ),
            block(
                201,
                vec![
                    copy(4, 1),
                    typed_assignment(
                        5,
                        BOOL_TYPE,
                        SemanticRvalueKindV1::Binary {
                            operation: SemanticBinaryOpV1::GreaterOrEqual,
                            left: typed_operand(4, U64_TYPE),
                            right: typed_constant(U64_TYPE, 64, 8),
                        },
                    ),
                ],
                zero_switch(5, BOOL_TYPE, 2, 6),
            ),
            block(202, vec![copy(6, 1)], zero_switch(3, BOOL_TYPE, 3, 4)),
            block(
                203,
                vec![],
                SemanticTerminatorKindV1::Goto(cfg_edge(SemanticEdgeRoleV1::Goto, 4)),
            ),
            block(
                204,
                vec![
                    copy(7, 6),
                    typed_assignment(
                        8,
                        BOOL_TYPE,
                        SemanticRvalueKindV1::Binary {
                            operation: SemanticBinaryOpV1::LessThan,
                            left: typed_operand(7, U64_TYPE),
                            right: typed_constant(U64_TYPE, 64, 8),
                        },
                    ),
                ],
                SemanticTerminatorKindV1::Assert {
                    condition: SemanticOperandV1::Move(typed_place(8, BOOL_TYPE)),
                    expected: true,
                    message: SemanticAssertMessageV1::Overflow {
                        operation,
                        left: typed_constant(U64_TYPE, 1, 8),
                        right: typed_operand(6, U64_TYPE),
                    },
                    target: cfg_edge(SemanticEdgeRoleV1::AssertSuccess, 5),
                    unwind: SemanticUnwindActionV1::Unreachable,
                },
            ),
            block(
                205,
                vec![typed_assignment(
                    0,
                    U64_TYPE,
                    SemanticRvalueKindV1::Binary {
                        operation,
                        left: typed_constant(U64_TYPE, 1, 8),
                        right: typed_operand(6, U64_TYPE),
                    },
                )],
                SemanticTerminatorKindV1::Return,
            ),
            block(206, vec![], SemanticTerminatorKindV1::Unreachable),
        ],
        vec![
            local(200, U64_TYPE, SemanticLocalRoleV1::Return),
            local(201, U64_TYPE, SemanticLocalRoleV1::Argument(0)),
            local(202, U64_TYPE, SemanticLocalRoleV1::Argument(1)),
            local(203, BOOL_TYPE, SemanticLocalRoleV1::Argument(2)),
            local(204, U64_TYPE, SemanticLocalRoleV1::Temporary),
            local(205, BOOL_TYPE, SemanticLocalRoleV1::Temporary),
            local(206, U64_TYPE, SemanticLocalRoleV1::Temporary),
            local(207, U64_TYPE, SemanticLocalRoleV1::Temporary),
            local(208, BOOL_TYPE, SemanticLocalRoleV1::Temporary),
        ],
    )
}

fn replace_guarded_shift_block_v1(
    function: &SemanticFunctionDeclV1,
    index: usize,
    statements: Vec<SemanticStatementV1>,
    terminator: SemanticTerminatorKindV1,
) -> SemanticFunctionDeclV1 {
    let mut blocks = function.blocks().to_vec();
    blocks[index] = block(200 + index as u8, statements, terminator);
    projection_function_with_locals(blocks, function.locals().to_vec())
}

fn proves_guarded_shift_fixture_v1(
    proof: &mut SemanticAssertProofsV1<'_>,
) -> Result<bool, ProductionRankedProjectionErrorV1> {
    let SemanticTerminatorKindV1::Assert {
        condition,
        expected,
        message,
        ..
    } = proof.function.blocks()[GUARDED_SHIFT_ASSERT_BLOCK_V1]
        .terminator()
        .kind()
    else {
        unreachable!("guarded shift fixture retains its assertion")
    };
    proof.proves_guarded_shift_assert_v1(
        condition,
        *expected,
        message,
        GUARDED_SHIFT_ASSERT_BLOCK_V1,
    )
}

fn check_guarded_shift_fixture_v1(function: &SemanticFunctionDeclV1, expected: bool) {
    let types = assertion_proof_types();
    let mut proof = SemanticAssertProofsV1::new(&types, function).unwrap();
    assert_eq!(
        proves_guarded_shift_fixture_v1(&mut proof).unwrap(),
        expected
    );
    assert_eq!(
        SemanticAssertProofsV1::analyze(&types, function).unwrap()[GUARDED_SHIFT_ASSERT_BLOCK_V1],
        expected
    );
    assert_eq!(
        SemanticAssertProofsV1::analyze_defined_callable_asserts_v1(&types, function).unwrap()
            [GUARDED_SHIFT_ASSERT_BLOCK_V1],
        expected
    );
}

#[test]
fn guarded_shift_accepts_exact_source_aliases_on_every_guarded_path() {
    for operation in [
        SemanticBinaryOpV1::ShiftLeft,
        SemanticBinaryOpV1::ShiftRight,
    ] {
        let function = guarded_shift_function_v1(operation);
        check_guarded_shift_fixture_v1(&function, true);
        // No failure-side trap recognition is needed to prove the success edge.
        let returning_failure =
            replace_guarded_shift_block_v1(&function, 6, vec![], SemanticTerminatorKindV1::Return);
        check_guarded_shift_fixture_v1(&returning_failure, true);
    }
}

#[test]
fn guarded_shift_unsigned_boundaries_accept_zero_and_63_but_reject_64() {
    for operation in [
        SemanticBinaryOpV1::ShiftLeft,
        SemanticBinaryOpV1::ShiftRight,
    ] {
        for lane in [0, 63, 64, u64::MAX] {
            let function = guarded_shift_function_v1(operation);
            let function = replace_guarded_shift_block_v1(
                &function,
                0,
                vec![typed_assignment(
                    1,
                    U64_TYPE,
                    SemanticRvalueKindV1::Use(typed_constant(U64_TYPE, u128::from(lane), 8)),
                )],
                SemanticTerminatorKindV1::Goto(cfg_edge(SemanticEdgeRoleV1::Goto, 2)),
            );
            check_guarded_shift_fixture_v1(&function, lane < 64);
        }
    }
}

#[test]
fn guarded_shift_rejects_wrong_guard_value_width_orientation_and_bypass() {
    for (operation, tested, width, zero, otherwise) in [
        (SemanticBinaryOpV1::GreaterOrEqual, 2, 64, 2, 6),
        (SemanticBinaryOpV1::GreaterOrEqual, 4, 65, 2, 6),
        (SemanticBinaryOpV1::GreaterThan, 4, 64, 2, 6),
        (SemanticBinaryOpV1::LessThan, 4, 64, 2, 6),
        (SemanticBinaryOpV1::GreaterOrEqual, 4, 64, 6, 2),
    ] {
        let function = guarded_shift_function_v1(SemanticBinaryOpV1::ShiftLeft);
        let mut statements = function.blocks()[1].statements().to_vec();
        statements[1] = typed_assignment(
            5,
            BOOL_TYPE,
            SemanticRvalueKindV1::Binary {
                operation,
                left: typed_operand(tested, U64_TYPE),
                right: typed_constant(U64_TYPE, width, 8),
            },
        );
        let function = replace_guarded_shift_block_v1(
            &function,
            1,
            statements,
            zero_switch(5, BOOL_TYPE, zero, otherwise),
        );
        check_guarded_shift_fixture_v1(&function, false);
    }
    let function = guarded_shift_function_v1(SemanticBinaryOpV1::ShiftLeft);
    let bypass =
        replace_guarded_shift_block_v1(&function, 0, vec![], zero_switch(3, BOOL_TYPE, 1, 2));
    check_guarded_shift_fixture_v1(&bypass, false);
    let rejoining_failure = replace_guarded_shift_block_v1(
        &function,
        6,
        vec![],
        SemanticTerminatorKindV1::Goto(cfg_edge(SemanticEdgeRoleV1::Goto, 2)),
    );
    check_guarded_shift_fixture_v1(&rejoining_failure, false);
}

#[test]
fn guarded_shift_rejects_path_mutation_and_stale_alias_captures() {
    for destination in [1, 6] {
        let function = guarded_shift_function_v1(SemanticBinaryOpV1::ShiftLeft);
        let function = replace_guarded_shift_block_v1(
            &function,
            3,
            vec![typed_assignment(
                destination,
                U64_TYPE,
                SemanticRvalueKindV1::Use(typed_operand(2, U64_TYPE)),
            )],
            SemanticTerminatorKindV1::Goto(cfg_edge(SemanticEdgeRoleV1::Goto, 4)),
        );
        check_guarded_shift_fixture_v1(&function, false);
    }
    // The guard observes a captured value, not a later assignment to its source.
    for insertion in [1, 2] {
        let function = guarded_shift_function_v1(SemanticBinaryOpV1::ShiftLeft);
        let mut statements = function.blocks()[1].statements().to_vec();
        statements.insert(
            insertion,
            typed_assignment(
                1,
                U64_TYPE,
                SemanticRvalueKindV1::Use(typed_operand(2, U64_TYPE)),
            ),
        );
        let function = replace_guarded_shift_block_v1(
            &function,
            1,
            statements,
            function.blocks()[1].terminator().kind().clone(),
        );
        check_guarded_shift_fixture_v1(&function, false);
    }
}

#[test]
fn guarded_shift_rejects_source_substitution_and_mutation_after_the_check() {
    let function = guarded_shift_function_v1(SemanticBinaryOpV1::ShiftLeft);
    for destination in [1, 6] {
        let mut statements = function.blocks()[4].statements().to_vec();
        statements.push(typed_assignment(
            destination,
            U64_TYPE,
            SemanticRvalueKindV1::Use(typed_constant(U64_TYPE, 64, 8)),
        ));
        let hostile = replace_guarded_shift_block_v1(
            &function,
            4,
            statements,
            function.blocks()[4].terminator().kind().clone(),
        );
        let types = assertion_proof_types();
        let mut proof = SemanticAssertProofsV1::new(&types, &hostile).unwrap();
        // The generic assertion evaluator may independently prove an old
        // Boolean snapshot; it cannot supply this exact shift-source proof.
        assert!(!proves_guarded_shift_fixture_v1(&mut proof).unwrap());
    }
    for (right, width, expected) in [(2, 64, true), (6, 65, true), (6, 64, false)] {
        let mut statements = function.blocks()[4].statements().to_vec();
        statements[1] = typed_assignment(
            8,
            BOOL_TYPE,
            SemanticRvalueKindV1::Binary {
                operation: SemanticBinaryOpV1::LessThan,
                left: typed_operand(7, U64_TYPE),
                right: typed_constant(U64_TYPE, width, 8),
            },
        );
        let hostile = replace_guarded_shift_block_v1(
            &function,
            4,
            statements,
            SemanticTerminatorKindV1::Assert {
                condition: typed_operand(8, BOOL_TYPE),
                expected,
                message: SemanticAssertMessageV1::Overflow {
                    operation: SemanticBinaryOpV1::ShiftLeft,
                    left: typed_constant(U64_TYPE, 1, 8),
                    right: typed_operand(right, U64_TYPE),
                },
                target: cfg_edge(SemanticEdgeRoleV1::AssertSuccess, 5),
                unwind: SemanticUnwindActionV1::Unreachable,
            },
        );
        let types = assertion_proof_types();
        let mut proof = SemanticAssertProofsV1::new(&types, &hostile).unwrap();
        assert!(!proves_guarded_shift_fixture_v1(&mut proof).unwrap());
    }
}

#[test]
fn guarded_shift_authenticates_a_call_result_but_not_a_post_guard_call_write() {
    let call = |destination, target| {
        SemanticTerminatorKindV1::Call(
            SemanticDirectCallV1::new_callable(
                SemanticCallableIdV1::from_index(0),
                vec![typed_operand(2, U64_TYPE)],
                Some(SemanticCallDestinationV1::new(
                    typed_place(destination, U64_TYPE),
                    cfg_edge(SemanticEdgeRoleV1::CallReturn, target),
                )),
                SemanticUnwindActionV1::Unreachable,
            )
            .unwrap(),
        )
    };
    let function = guarded_shift_function_v1(SemanticBinaryOpV1::ShiftLeft);
    let function = replace_guarded_shift_block_v1(&function, 0, vec![], call(1, 1));
    let mut locals = function.locals().to_vec();
    locals[1] = local(201, U64_TYPE, SemanticLocalRoleV1::Temporary);
    locals[2] = local(202, U64_TYPE, SemanticLocalRoleV1::Argument(0));
    locals[3] = local(203, BOOL_TYPE, SemanticLocalRoleV1::Argument(1));
    let function = projection_function_with_locals(function.blocks().to_vec(), locals);
    check_guarded_shift_fixture_v1(&function, true);
    for destination in [1, 6] {
        let hostile = replace_guarded_shift_block_v1(&function, 3, vec![], call(destination, 4));
        check_guarded_shift_fixture_v1(&hostile, false);
    }
}

#[test]
fn guarded_shift_rejects_address_escape_and_value_erasing_cast_aliases() {
    for source in [1, 6] {
        let function = guarded_shift_function_v1(SemanticBinaryOpV1::ShiftLeft);
        let function = replace_guarded_shift_block_v1(
            &function,
            3,
            vec![typed_assignment(
                9,
                U64_POINTER_TYPE,
                SemanticRvalueKindV1::Borrow {
                    kind: SemanticBorrowKindV1::Mutable,
                    place: typed_place(source, U64_TYPE),
                },
            )],
            SemanticTerminatorKindV1::Goto(cfg_edge(SemanticEdgeRoleV1::Goto, 4)),
        );
        let mut locals = function.locals().to_vec();
        locals.push(local(209, U64_POINTER_TYPE, SemanticLocalRoleV1::Temporary));
        let function = projection_function_with_locals(function.blocks().to_vec(), locals);
        check_guarded_shift_fixture_v1(&function, false);
    }
    let function = guarded_shift_function_v1(SemanticBinaryOpV1::ShiftLeft);
    let function = replace_guarded_shift_block_v1(
        &function,
        2,
        vec![
            typed_assignment(
                9,
                SCALAR_TYPE,
                SemanticRvalueKindV1::Cast {
                    kind: SemanticCastKindV1::Integer,
                    operand: typed_operand(1, U64_TYPE),
                },
            ),
            typed_assignment(
                6,
                U64_TYPE,
                SemanticRvalueKindV1::Cast {
                    kind: SemanticCastKindV1::Integer,
                    operand: typed_operand(9, SCALAR_TYPE),
                },
            ),
        ],
        function.blocks()[2].terminator().kind().clone(),
    );
    let mut locals = function.locals().to_vec();
    locals.push(local(209, SCALAR_TYPE, SemanticLocalRoleV1::Temporary));
    let function = projection_function_with_locals(function.blocks().to_vec(), locals);
    check_guarded_shift_fixture_v1(&function, false);
}

#[test]
fn guarded_shift_rejects_a_mutating_backedge_that_bypasses_revalidation() {
    let function = guarded_shift_function_v1(SemanticBinaryOpV1::ShiftLeft);
    let function = replace_guarded_shift_block_v1(
        &function,
        3,
        vec![typed_assignment(
            6,
            U64_TYPE,
            SemanticRvalueKindV1::Use(typed_operand(2, U64_TYPE)),
        )],
        SemanticTerminatorKindV1::Goto(cfg_edge(SemanticEdgeRoleV1::Goto, 4)),
    );
    let function = replace_guarded_shift_block_v1(
        &function,
        5,
        function.blocks()[5].statements().to_vec(),
        zero_switch(3, BOOL_TYPE, 3, 6),
    );
    check_guarded_shift_fixture_v1(&function, false);
}

#[test]
fn guarded_shift_does_not_infer_a_nonnegative_signed_rhs() {
    let function = guarded_shift_function_v1(SemanticBinaryOpV1::ShiftLeft);
    let mut types = assertion_proof_types();
    let original = &types[U64_TYPE.index() as usize];
    types[U64_TYPE.index() as usize] = SemanticTypeDeclV1::new(
        original.identity(),
        original.layout_identity(),
        original.layout().clone(),
        SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
            signed: true,
            bits: 64,
        }),
    );
    let mut proof = SemanticAssertProofsV1::new(&types, &function).unwrap();
    assert!(!proves_guarded_shift_fixture_v1(&mut proof).unwrap());
    assert!(
        !SemanticAssertProofsV1::analyze(&types, &function).unwrap()[GUARDED_SHIFT_ASSERT_BLOCK_V1]
    );
}

#[test]
fn guarded_shift_obeys_the_existing_graph_work_limit() {
    let function = guarded_shift_function_v1(SemanticBinaryOpV1::ShiftLeft);
    let types = assertion_proof_types();
    let mut proof = SemanticAssertProofsV1::new(&types, &function).unwrap();
    proof.work = MAX_PROJECTED_LOOP_GRAPH_WORK_V1;
    assert!(proves_guarded_shift_fixture_v1(&mut proof).is_err());
}
