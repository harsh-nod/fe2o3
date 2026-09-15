fn derived_exclusive_index_fixture(
    divisor: u64,
    stride: u64,
    mask: u64,
    invocation: bool,
    reassign: bool,
) -> (
    Vec<SemanticTypeDeclV1>,
    Vec<SemanticCallableDeclV1>,
    SemanticFunctionDeclV1,
) {
    let (types, callables, function) = typed_global_projection_fixture_v1();
    let (types, callables, function) =
        typed_global_exclusive_fixture_v1(types, callables, function, false);
    let mut locals = function.locals().to_vec();
    let first = locals.len() as u32;
    for index in 0..4 {
        locals.push(local(225 + index, U64_TYPE, SemanticLocalRoleV1::Temporary));
    }
    let seed = || {
        if invocation {
            typed_operand(6, U64_TYPE)
        } else {
            typed_constant(U64_TYPE, 0, 8)
        }
    };
    let mut blocks = function.blocks().to_vec();
    let original = &blocks[7];
    let SemanticTerminatorKindV1::Call(call) = original.terminator().kind() else {
        unreachable!()
    };
    let mut statements = original.statements().to_vec();
    for (index, operation, left, right) in [
        (
            0,
            SemanticBinaryOpV1::Divide,
            seed(),
            typed_constant(U64_TYPE, u128::from(divisor), 8),
        ),
        (
            1,
            SemanticBinaryOpV1::Multiply,
            typed_operand(first, U64_TYPE),
            typed_constant(U64_TYPE, u128::from(stride), 8),
        ),
        (
            2,
            SemanticBinaryOpV1::BitAnd,
            seed(),
            typed_constant(U64_TYPE, u128::from(mask), 8),
        ),
        (
            3,
            SemanticBinaryOpV1::Add,
            typed_operand(first + 1, U64_TYPE),
            typed_operand(first + 2, U64_TYPE),
        ),
    ] {
        statements.push(typed_assignment(
            first + index,
            U64_TYPE,
            SemanticRvalueKindV1::Binary {
                operation,
                left,
                right,
            },
        ));
    }
    if reassign {
        statements.push(typed_assignment(
            first + 3,
            U64_TYPE,
            SemanticRvalueKindV1::Use(typed_constant(U64_TYPE, 0, 8)),
        ));
    }
    let mut arguments = call.arguments().to_vec();
    arguments[1] = typed_operand(first + 3, U64_TYPE);
    blocks[7] = SemanticBasicBlockV1::new(
        original.identity(),
        original.source(),
        statements,
        SemanticTerminatorV1::new(
            original.terminator().source(),
            SemanticTerminatorKindV1::Call(
                SemanticDirectCallV1::new_callable(
                    call.callee(),
                    arguments,
                    call.destination().cloned(),
                    call.unwind(),
                )
                .unwrap(),
            ),
        ),
    )
    .unwrap();
    (
        types,
        callables,
        typed_global_fixture_with_body_v1(&function, locals, blocks),
    )
}

fn evaluate_projected_index(
    operations: &[ProductionRankedOperationV1],
    output: ProductionRankedValueV1,
    invocation: u64,
) -> u64 {
    let mut values = HashMap::new();
    for operation in operations {
        let (result, value) = match operation {
            ProductionRankedOperationV1::InvocationIndex {
                result,
                dimension: 0,
                ..
            } => (*result, invocation),
            ProductionRankedOperationV1::IndexConstant { result, value } => (*result, *value),
            ProductionRankedOperationV1::IndexBinary {
                result,
                kind,
                lhs,
                rhs,
            } => {
                let left = values[lhs];
                let right = values[rhs];
                (
                    *result,
                    match kind {
                        IndexBinaryKindAttr::Add => left + right,
                        IndexBinaryKindAttr::Multiply => left * right,
                        IndexBinaryKindAttr::Divide => left / right,
                        IndexBinaryKindAttr::Remainder => left % right,
                    },
                )
            }
            _ => continue,
        };
        values.insert(ProductionRankedValueV1::Local(result), value);
    }
    values[&output]
}

#[test]
fn exclusive_index_projection_preserves_compacted_invocation_arithmetic() {
    for stride in [4, 16, 32] {
        let (types, callables, function) =
            derived_exclusive_index_fixture(64, stride, 63, true, false);
        let (projection, operations) =
            project_capability_index_fixture_with_launch(&types, &callables, &function, Some(1024))
                .unwrap();
        let write = projection.direct_write_effects[7].as_ref().unwrap();
        assert_eq!(write.indices.len(), 1);
        for index in 0..1024 {
            assert_eq!(
                evaluate_projected_index(&operations, write.indices[0], index),
                (index / 64) * stride + (index & 63)
            );
        }
        // Projection preserves the expression; active-lane guards and race
        // freedom still require independent ranked verification.
        assert_eq!(write.comparisons.len(), 1);
    }
}

#[test]
fn exclusive_index_projection_rejects_non_total_or_fabricated_expressions() {
    for (divisor, stride, mask, invocation, reassign) in [
        (0, 16, 63, true, false),
        (64, u64::MAX, 63, true, false),
        (64, 16, 10, true, false),
        (64, 16, 63, false, false),
        (64, 16, 63, true, true),
    ] {
        let (types, callables, function) =
            derived_exclusive_index_fixture(divisor, stride, mask, invocation, reassign);
        assert_incomplete(
            project_capability_index_fixture_with_launch(&types, &callables, &function, Some(1024)),
            "a typed global exclusive store requires an exact invocation-derived index",
        );
    }
}

#[test]
fn exclusive_index_projection_requires_a_bounded_launch() {
    let (types, callables, function) = derived_exclusive_index_fixture(64, 16, 63, true, false);
    assert_incomplete(
        project_capability_index_fixture_with_launch(&types, &callables, &function, None),
        "a typed global exclusive store requires an exact invocation-derived index",
    );
}

#[test]
fn invocation_index_roots_cannot_be_used_as_uniform_loop_bounds() {
    let (types, _, function) = derived_exclusive_index_fixture(64, 16, 63, true, false);
    let inventory = assertion_definition_inventory(&function).unwrap();
    let constants = constant_locals(&function).unwrap();
    let mut proof = SemanticAssertProofsV1::new(&types, &function).unwrap();
    let mut arguments = vec![None; function.locals().len()];
    let mut next_argument = 1;
    let mut operations = Vec::new();
    let mut next_value = 0;
    let operand = typed_operand(function.locals().len() as u32 - 1, U64_TYPE);
    let result = TotalUnsignedIndexProjectorV1::new(
        &types,
        &function,
        &constants,
        &inventory.counts,
        &inventory.address_escaped,
        &inventory.assignments,
        &mut proof,
        &mut arguments,
        &mut next_argument,
        &mut operations,
        &mut next_value,
    )
    .unwrap()
    .resolve_operand(&operand, 7, function.blocks()[7].statements().len())
    .unwrap();
    assert!(result.is_none());
    assert!(arguments.iter().all(Option::is_none));
}

#[test]
fn exclusive_index_projection_requires_the_producing_edge_and_alias_chain() {
    for derived in [false, true] {
        for bypass in [false, true] {
            let (types, callables, function) = if derived {
                derived_exclusive_index_fixture(64, 16, 63, true, false)
            } else {
                let (types, callables, function) = typed_global_projection_fixture_v1();
                typed_global_exclusive_fixture_v1(types, callables, function, false)
            };
            let mut blocks = function.blocks().to_vec();
            let original = &blocks[2];
            let mut statements = original.statements().to_vec();
            let terminator = if bypass {
                let SemanticTerminatorKindV1::Call(call) = original.terminator().kind() else {
                    unreachable!()
                };
                let destination = call.destination().unwrap();
                SemanticTerminatorKindV1::Call(
                    SemanticDirectCallV1::new_callable(
                        call.callee(),
                        call.arguments().to_vec(),
                        Some(SemanticCallDestinationV1::new(
                            destination.place().clone(),
                            cfg_edge(SemanticEdgeRoleV1::CallReturn, 9),
                        )),
                        call.unwind(),
                    )
                    .unwrap(),
                )
            } else {
                // The final alias dominates its use, but its source does not
                // exist until this block's call returns.
                statements.extend_from_slice(blocks[3].statements());
                original.terminator().kind().clone()
            };
            blocks[2] = SemanticBasicBlockV1::new(
                original.identity(),
                original.source(),
                statements,
                SemanticTerminatorV1::new(original.terminator().source(), terminator),
            )
            .unwrap();
            if bypass {
                // bb4 dominates the store, but the bb3 -> bb4 producer edge
                // does not: this additional predecessor bypasses ThreadIndexGet.
                blocks.push(block(
                    239,
                    vec![],
                    SemanticTerminatorKindV1::SwitchInt {
                        discriminant: typed_constant(U64_TYPE, 0, 8),
                        targets: SemanticSwitchTargetsV1::new(
                            vec![SemanticSwitchTargetV1::new(
                                0,
                                cfg_edge(SemanticEdgeRoleV1::SwitchValue, 4),
                            )],
                            cfg_edge(SemanticEdgeRoleV1::SwitchOtherwise, 3),
                        )
                        .unwrap(),
                    },
                ));
            } else {
                let original = &blocks[3];
                blocks[3] = SemanticBasicBlockV1::new(
                    original.identity(),
                    original.source(),
                    vec![],
                    original.terminator().clone(),
                )
                .unwrap();
            }
            let changed =
                typed_global_fixture_with_body_v1(&function, function.locals().to_vec(), blocks);
            assert_incomplete(
                project_capability_index_fixture_with_launch(
                    &types,
                    &callables,
                    &changed,
                    Some(1024),
                ),
                "a typed global exclusive store requires an exact invocation-derived index",
            );
        }
    }
}

#[test]
fn checked_exclusive_index_uses_launch_bounds_and_exact_overflow_assertion() {
    for (checked, operation, constant, wrong_assertion) in [
        (
            SemanticCheckedBinaryOpV1::Add,
            SemanticBinaryOpV1::Add,
            1,
            false,
        ),
        (
            SemanticCheckedBinaryOpV1::Add,
            SemanticBinaryOpV1::Add,
            u64::MAX,
            false,
        ),
        (
            SemanticCheckedBinaryOpV1::Add,
            SemanticBinaryOpV1::Add,
            1,
            true,
        ),
        (
            SemanticCheckedBinaryOpV1::Multiply,
            SemanticBinaryOpV1::Multiply,
            2,
            false,
        ),
        (
            SemanticCheckedBinaryOpV1::Multiply,
            SemanticBinaryOpV1::Multiply,
            u64::MAX,
            false,
        ),
        (
            SemanticCheckedBinaryOpV1::Multiply,
            SemanticBinaryOpV1::Multiply,
            2,
            true,
        ),
    ] {
        let (types, callables, function) = derived_exclusive_index_fixture(64, 16, 63, true, false);
        let mut locals = function.locals().to_vec();
        let result = locals.len() as u32;
        locals.push(local(238, CHECKED_U64_TYPE, SemanticLocalRoleV1::Temporary));
        let field = |index, ty| {
            SemanticOperandV1::Copy(
                SemanticPlaceV1::new(
                    SemanticLocalIdV1::from_index(result),
                    vec![
                        SemanticProjectionV1::new(SemanticProjectionKindV1::Field(index), ty)
                            .unwrap(),
                    ],
                    ty,
                )
                .unwrap(),
            )
        };
        let left = typed_operand(result - 1, U64_TYPE);
        let right = typed_constant(U64_TYPE, u128::from(constant), 8);
        let mut blocks = function.blocks().to_vec();
        let original = &blocks[7];
        let SemanticTerminatorKindV1::Call(call) = original.terminator().kind() else {
            unreachable!()
        };
        let mut arguments = call.arguments().to_vec();
        arguments[1] = field(0, U64_TYPE);
        let continuation = block(
            240,
            vec![],
            SemanticTerminatorKindV1::Call(
                SemanticDirectCallV1::new_callable(
                    call.callee(),
                    arguments,
                    call.destination().cloned(),
                    call.unwind(),
                )
                .unwrap(),
            ),
        );
        let mut statements = original.statements().to_vec();
        statements.push(typed_assignment(
            result,
            CHECKED_U64_TYPE,
            SemanticRvalueKindV1::CheckedBinary(SemanticCheckedBinaryRvalueV1::new(
                checked,
                left.clone(),
                right.clone(),
            )),
        ));
        blocks[7] = block(
            197,
            statements,
            SemanticTerminatorKindV1::Assert {
                condition: field(1, BOOL_TYPE),
                expected: false,
                message: SemanticAssertMessageV1::Overflow {
                    operation: if wrong_assertion {
                        SemanticBinaryOpV1::Subtract
                    } else {
                        operation
                    },
                    left,
                    right: right.clone(),
                },
                target: cfg_edge(SemanticEdgeRoleV1::AssertSuccess, 9),
                unwind: SemanticUnwindActionV1::Unreachable,
            },
        );
        blocks.push(continuation);
        let changed = typed_global_fixture_with_body_v1(&function, locals, blocks);
        let projected =
            project_capability_index_fixture_with_launch(&types, &callables, &changed, Some(1024));
        if wrong_assertion || constant == u64::MAX {
            assert_incomplete(
                projected,
                "a typed global exclusive store requires an exact invocation-derived index",
            );
        } else {
            let (projection, operations) = projected.unwrap();
            let write = projection.direct_write_effects[9].as_ref().unwrap();
            for index in 0..1024 {
                let base = (index / 64) * 16 + (index & 63);
                let expected = if checked == SemanticCheckedBinaryOpV1::Add {
                    base + constant
                } else {
                    base * constant
                };
                assert_eq!(
                    evaluate_projected_index(&operations, write.indices[0], index),
                    expected
                );
            }
        }
    }
}
