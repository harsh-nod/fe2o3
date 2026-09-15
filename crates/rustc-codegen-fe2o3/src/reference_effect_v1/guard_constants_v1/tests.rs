use super::*;

fn constant(scalar: ReferenceScalarTypeV1, bits: u128) -> ReferenceEffectExpressionV1 {
    ReferenceEffectExpressionV1::Constant(ReferenceConstantV1::Scalar { scalar, bits })
}

fn binary(
    operation: ReferenceBinaryOpV1,
    lhs: ReferenceEffectExpressionV1,
    rhs: ReferenceEffectExpressionV1,
    checked: bool,
) -> ReferenceEffectExpressionV1 {
    ReferenceEffectExpressionV1::Binary {
        operation,
        lhs: Box::new(lhs),
        rhs: Box::new(rhs),
        checked,
    }
}

fn normalized(mut expression: ReferenceEffectExpressionV1) -> ReferenceEffectExpressionV1 {
    fold(
        &mut expression,
        &mut ReferenceSymbolicWorkBudgetV2::default(),
        0,
    )
    .unwrap();
    expression
}

#[test]
fn guard_constants_fold_only_nonoverflowing_unsigned_arithmetic() {
    for scalar in [
        ReferenceScalarTypeV1::U8,
        ReferenceScalarTypeV1::U16,
        ReferenceScalarTypeV1::U32,
        ReferenceScalarTypeV1::U64,
        ReferenceScalarTypeV1::Usize,
    ] {
        let maximum = reference_scalar_mask_v2(scalar).unwrap();
        for checked in [false, true] {
            for (operation, lhs, rhs, value) in [
                (ReferenceBinaryOpV1::Add, maximum - 1, 1, maximum),
                (ReferenceBinaryOpV1::Subtract, maximum, maximum, 0),
                (ReferenceBinaryOpV1::Multiply, maximum, 1, maximum),
            ] {
                assert_eq!(
                    normalized(binary(
                        operation,
                        constant(scalar, lhs),
                        constant(scalar, rhs),
                        checked
                    )),
                    constant(scalar, value)
                );
            }
            for (operation, lhs, rhs) in [
                (ReferenceBinaryOpV1::Add, maximum, 1),
                (ReferenceBinaryOpV1::Subtract, 0, 1),
                (ReferenceBinaryOpV1::Multiply, maximum, 2),
            ] {
                let original = binary(
                    operation,
                    constant(scalar, lhs),
                    constant(scalar, rhs),
                    checked,
                );
                assert_eq!(normalized(original.clone()), original);
            }
        }
    }
}

#[test]
fn guard_constants_preserve_malformed_types_signed_bool_and_other_operators() {
    let c = constant;
    for (lhs, rhs) in [
        (
            c(ReferenceScalarTypeV1::U8, 256),
            c(ReferenceScalarTypeV1::U8, 1),
        ),
        (
            c(ReferenceScalarTypeV1::Usize, 16),
            c(ReferenceScalarTypeV1::U64, 16),
        ),
        (
            c(ReferenceScalarTypeV1::Bool, 1),
            c(ReferenceScalarTypeV1::Bool, 1),
        ),
        (
            c(ReferenceScalarTypeV1::I32, 16),
            c(ReferenceScalarTypeV1::I32, 16),
        ),
        (
            c(ReferenceScalarTypeV1::F32, 16),
            c(ReferenceScalarTypeV1::F32, 16),
        ),
        (
            ReferenceEffectExpressionV1::PointCoordinate { axis: 0 },
            c(ReferenceScalarTypeV1::Usize, 16),
        ),
    ] {
        let original = binary(ReferenceBinaryOpV1::Multiply, lhs, rhs, true);
        assert_eq!(normalized(original.clone()), original);
    }
    for operation in [
        ReferenceBinaryOpV1::Divide,
        ReferenceBinaryOpV1::Remainder,
        ReferenceBinaryOpV1::ShiftLeft,
        ReferenceBinaryOpV1::BitAnd,
        ReferenceBinaryOpV1::Equal,
    ] {
        let original = binary(
            operation,
            c(ReferenceScalarTypeV1::Usize, 16),
            c(ReferenceScalarTypeV1::Usize, 0),
            false,
        );
        assert_eq!(normalized(original.clone()), original);
    }
}

#[test]
fn guard_constants_nested_literals_cannot_hide_an_inner_overflow() {
    let c = |bits| constant(ReferenceScalarTypeV1::U8, bits);
    for (lhs, expected) in [(4, Some(17)), (128, None)] {
        let original = binary(
            ReferenceBinaryOpV1::Add,
            binary(ReferenceBinaryOpV1::Multiply, c(lhs), c(4), true),
            c(1),
            true,
        );
        assert_eq!(
            normalized(original.clone()),
            expected.map(c).unwrap_or(original)
        );
    }
}

#[test]
fn guard_constants_differential_u8_checked_boundary() {
    let c = |bits| constant(ReferenceScalarTypeV1::U8, bits);
    for operation in [
        ReferenceBinaryOpV1::Add,
        ReferenceBinaryOpV1::Subtract,
        ReferenceBinaryOpV1::Multiply,
    ] {
        for lhs in 0_u16..=255 {
            for rhs in 0_u16..=255 {
                let expected = match operation {
                    ReferenceBinaryOpV1::Add => lhs.checked_add(rhs).filter(|v| *v <= 255),
                    ReferenceBinaryOpV1::Subtract => lhs.checked_sub(rhs),
                    ReferenceBinaryOpV1::Multiply => lhs.checked_mul(rhs).filter(|v| *v <= 255),
                    _ => unreachable!(),
                };
                let original = binary(operation, c(lhs.into()), c(rhs.into()), true);
                assert_eq!(
                    normalized(original.clone()),
                    expected.map(|v| c(v.into())).unwrap_or(original)
                );
            }
        }
    }
}

fn place(local: u32) -> ReferencePlaceV1 {
    ReferencePlaceV1 {
        local,
        projection: Box::default(),
    }
}
fn copy(local: u32) -> ReferenceOperandV1 {
    ReferenceOperandV1::Copy(place(local))
}
fn field(index: u32) -> ReferenceOperandV1 {
    ReferenceOperandV1::Copy(ReferencePlaceV1 {
        local: 2,
        projection: vec![ReferencePlaceProjectionV1::Field(index)].into_boxed_slice(),
    })
}
fn operand(bits: u128) -> ReferenceOperandV1 {
    ReferenceOperandV1::Constant(ReferenceConstantV1::Scalar {
        scalar: ReferenceScalarTypeV1::Usize,
        bits,
    })
}
fn assignment(statement: u32, local: u32, value: ReferenceValueV1) -> ReferenceAssignmentV1 {
    ReferenceAssignmentV1 {
        statement,
        destination: place(local),
        value,
    }
}
fn fixture() -> ReferenceEffectIrV1 {
    ReferenceEffectIrV1 {
        argument_count: 1,
        local_count: 5,
        relations: vec![ReferenceArgumentRelationV1::SharedSliceInput {
            argument: 0,
            element: ReferenceScalarTypeV1::F32,
        }]
        .into_boxed_slice(),
        blocks: vec![
            ReferenceBlockV1 {
                block: 0,
                assignments: vec![assignment(
                    0,
                    2,
                    ReferenceValueV1::Binary {
                        operation: ReferenceBinaryOpV1::Multiply,
                        lhs: operand(16),
                        rhs: operand(16),
                        checked: true,
                    },
                )]
                .into_boxed_slice(),
                terminator: ReferenceTerminatorV1::Assert {
                    condition: field(1),
                    expected: false,
                    success: 1,
                    bounds_check: None,
                },
            },
            ReferenceBlockV1 {
                block: 1,
                assignments: vec![
                    assignment(
                        0,
                        3,
                        ReferenceValueV1::InputLength {
                            reference_argument: 0,
                        },
                    ),
                    assignment(
                        1,
                        4,
                        ReferenceValueV1::Binary {
                            operation: ReferenceBinaryOpV1::Equal,
                            lhs: copy(3),
                            rhs: field(0),
                            checked: false,
                        },
                    ),
                ]
                .into_boxed_slice(),
                terminator: ReferenceTerminatorV1::Switch {
                    discriminant: copy(4),
                    values: vec![(0, 3)].into_boxed_slice(),
                    otherwise: 2,
                },
            },
            ReferenceBlockV1 {
                block: 2,
                assignments: Box::default(),
                terminator: ReferenceTerminatorV1::Return,
            },
            ReferenceBlockV1 {
                block: 3,
                assignments: Box::default(),
                terminator: ReferenceTerminatorV1::Return,
            },
        ]
        .into_boxed_slice(),
        loop_summaries: Box::default(),
        observable_output_effects: Box::default(),
    }
}

#[test]
fn guard_constants_source_path_canonicalizes_product_without_rewriting_pair_or_assert() {
    let ir = fixture();
    let original = ir.clone();
    let hash = ir.canonical_sha256_v1();
    let resolver = ReferenceExpressionResolverV1::new(&ir).unwrap();
    let raw = resolve_predicate_operand_v1(
        &resolver,
        &field(0),
        &mut ReferenceSymbolicWorkBudgetV2::default(),
    )
    .unwrap();
    assert!(matches!(
        raw,
        ReferenceEffectExpressionV1::Binary { checked: true, .. }
    ));
    let paths = reference_block_path_predicates_with_budget_v1(
        &ir,
        &mut ReferenceSymbolicWorkBudgetV2::default(),
    )
    .unwrap();
    let condition = binary(
        ReferenceBinaryOpV1::Equal,
        ReferenceEffectExpressionV1::InputLength {
            reference_argument: 0,
        },
        constant(ReferenceScalarTypeV1::Usize, 256),
        false,
    );
    for (block, expected) in [(2, true), (3, false)] {
        let canonical = reference_predicate_and_atom_v1(
            &ReferencePathPredicateV1::unconditional_v1(),
            reference_boolean_guard_atom_v1(condition.clone(), expected),
        )
        .unwrap();
        assert_eq!(paths[block], canonical);
    }
    assert_eq!(ir, original);
    assert_eq!(ir.canonical_sha256_v1(), hash);
}

#[test]
fn guard_constants_path_folding_does_not_fold_or_discharge_retained_bounds() {
    let mut ir = fixture();
    ir.local_count += 1;
    ir.blocks[2].assignments = vec![assignment(
        0,
        5,
        ReferenceValueV1::Binary {
            operation: ReferenceBinaryOpV1::LessThan,
            lhs: field(0),
            rhs: copy(3),
            checked: false,
        },
    )]
    .into_boxed_slice();
    ir.blocks[2].terminator = ReferenceTerminatorV1::Assert {
        condition: copy(5),
        expected: true,
        success: 3,
        bounds_check: Some(ReferenceBoundsCheckV1 {
            index: field(0),
            length: copy(3),
        }),
    };
    let source = ir.clone();
    let index = binary(
        ReferenceBinaryOpV1::Multiply,
        constant(ReferenceScalarTypeV1::Usize, 16),
        constant(ReferenceScalarTypeV1::Usize, 16),
        true,
    );
    let length = ReferenceEffectExpressionV1::InputLength {
        reference_argument: 0,
    };
    let expected = vec![ResolvedReferenceBoundsCheckV1 {
        block: 2,
        expected: true,
        condition: binary(
            ReferenceBinaryOpV1::LessThan,
            index.clone(),
            length.clone(),
            false,
        ),
        index,
        length: length.clone(),
    }];
    let mut work = ReferenceSymbolicWorkBudgetV2::default();
    assert_eq!(
        ir.resolved_bounds_checks_with_budget_v1(&mut work).unwrap(),
        expected
    );
    let guards = reference_block_path_predicates_with_budget_v1(&ir, &mut work).unwrap();
    assert_eq!(
        guards[2],
        reference_predicate_and_atom_v1(
            &ReferencePathPredicateV1::unconditional_v1(),
            reference_boolean_guard_atom_v1(
                binary(
                    ReferenceBinaryOpV1::Equal,
                    length,
                    constant(ReferenceScalarTypeV1::Usize, 256),
                    false,
                ),
                true,
            ),
        )
        .unwrap(),
    );
    assert_eq!(
        ir.resolved_bounds_checks_with_budget_v1(&mut work).unwrap(),
        expected
    );
    assert_eq!(ir, source);
}

#[test]
fn guard_constants_source_overflow_and_wrong_assert_polarity_still_remove_success() {
    for overflow in [false, true] {
        let mut ir = fixture();
        if overflow {
            let ReferenceValueV1::Binary { lhs, .. } = &mut ir.blocks[0].assignments[0].value
            else {
                panic!()
            };
            *lhs = operand(u64::MAX.into());
        } else {
            let ReferenceTerminatorV1::Assert { expected, .. } = &mut ir.blocks[0].terminator
            else {
                panic!()
            };
            *expected = true;
        }
        let before = ir.clone();
        let paths = reference_block_path_predicates_v1(&ir).unwrap();
        for path in &paths[1..] {
            assert!(path.clauses.is_empty());
        }
        assert_eq!(ir, before);
    }
}

#[test]
fn guard_constants_source_ambiguous_pair_remains_rejected() {
    let mut ir = fixture();
    let mut repeated = ir.blocks[0].assignments[0].clone();
    repeated.statement = 1;
    ir.blocks[0].assignments =
        vec![ir.blocks[0].assignments[0].clone(), repeated].into_boxed_slice();
    let error = reference_block_path_predicates_v1(&ir).unwrap_err();
    assert!(error.to_string().contains("has no unique definition"));
}

#[test]
fn guard_constants_use_existing_budget_exactly_and_do_not_refund_failure() {
    let original = binary(
        ReferenceBinaryOpV1::Multiply,
        constant(ReferenceScalarTypeV1::Usize, 16),
        constant(ReferenceScalarTypeV1::Usize, 16),
        true,
    );
    let mut work = ReferenceSymbolicWorkBudgetV2::default();
    fold(&mut original.clone(), &mut work, 0).unwrap();
    assert_eq!(work.charged_nodes, 10);
    let spent = work.charged_nodes;
    for missing in 0..=spent {
        let mut work = ReferenceSymbolicWorkBudgetV2 {
            charged_nodes: MAX_REFERENCE_SYMBOLIC_WORK_NODES_V2 - spent + missing,
        };
        let result = fold(&mut original.clone(), &mut work, 0);
        if missing == 0 {
            assert!(result.is_ok());
            assert_eq!(work.charged_nodes, MAX_REFERENCE_SYMBOLIC_WORK_NODES_V2);
        } else {
            assert!(
                result
                    .unwrap_err()
                    .to_string()
                    .contains("cumulative expression work nodes")
            );
            assert!(work.charged_nodes > MAX_REFERENCE_SYMBOLIC_WORK_NODES_V2);
        }
    }
}
