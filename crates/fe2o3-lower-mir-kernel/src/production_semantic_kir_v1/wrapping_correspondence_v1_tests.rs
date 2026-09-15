mod wrapping_correspondence_v1_tests {
    use super::*;

    fn integer(signed: bool, bits: u16) -> ProductionSemanticScalarTypeV2 {
        ProductionSemanticScalarTypeV2::Integer { signed, bits }
    }

    fn binary(
        operation: ProductionSemanticBinaryOpV2,
        scalar: ProductionSemanticScalarTypeV2,
        overflow: ProductionOverflowContractV2,
    ) -> NormalizedScalarExpressionV1 {
        NormalizedScalarExpressionV1::Binary {
            operation,
            scalar,
            overflow,
            lhs: Box::new(NormalizedScalarExpressionV1::Symbol { symbol: 1, scalar }),
            rhs: Box::new(NormalizedScalarExpressionV1::Symbol { symbol: 2, scalar }),
        }
    }

    fn corresponds(
        expected: &NormalizedScalarExpressionV1,
        actual: &NormalizedScalarExpressionV1,
    ) -> Option<bool> {
        scalar_value_expressions_correspond_v1(
            expected,
            actual,
            0,
            &mut UnsupportedIndexCorrelationBudgetV1 { remaining: 256 },
        )
    }

    fn nested(overflow: ProductionOverflowContractV2) -> NormalizedScalarExpressionV1 {
        let u32_type = integer(false, 32);
        let u64_type = integer(false, 64);
        NormalizedScalarExpressionV1::Select {
            scalar: u32_type,
            condition: Box::new(NormalizedScalarExpressionV1::Compare {
                operation: ProductionSemanticComparisonV2::LessThan,
                operand_scalar: u32_type,
                lhs: Box::new(binary(
                    ProductionSemanticBinaryOpV2::Add,
                    u32_type,
                    overflow,
                )),
                rhs: Box::new(NormalizedScalarExpressionV1::Constant {
                    scalar: u32_type,
                    bits: 7,
                }),
            }),
            when_true: Box::new(NormalizedScalarExpressionV1::Cast {
                kind: ProductionSemanticCastV2::Integer,
                source: u64_type,
                target: u32_type,
                operand: Box::new(NormalizedScalarExpressionV1::Unary {
                    operation: ProductionSemanticUnaryOpV2::Not,
                    scalar: u64_type,
                    operand: Box::new(NormalizedScalarExpressionV1::Cast {
                        kind: ProductionSemanticCastV2::Integer,
                        source: u32_type,
                        target: u64_type,
                        operand: Box::new(binary(
                            ProductionSemanticBinaryOpV2::Multiply,
                            u32_type,
                            overflow,
                        )),
                    }),
                }),
            }),
            when_false: Box::new(binary(
                ProductionSemanticBinaryOpV2::Subtract,
                u32_type,
                overflow,
            )),
        }
    }

    #[test]
    fn wrapping_value_correspondence_is_directional_and_width_exact() {
        use ProductionOverflowContractV2::{Checked, Wrapping};
        for operation in [
            ProductionSemanticBinaryOpV2::Add,
            ProductionSemanticBinaryOpV2::Subtract,
            ProductionSemanticBinaryOpV2::Multiply,
        ] {
            for signed in [false, true] {
                for bits in [8, 16, 32, 64] {
                    let scalar = integer(signed, bits);
                    let wrapping = binary(operation, scalar, Wrapping);
                    let checked = binary(operation, scalar, Checked);
                    assert_eq!(corresponds(&wrapping, &checked), Some(true));
                    assert_eq!(corresponds(&wrapping, &wrapping), Some(true));
                    assert_eq!(corresponds(&checked, &checked), Some(true));
                    assert_eq!(corresponds(&checked, &wrapping), Some(false));
                    assert_eq!(
                        corresponds(
                            &wrapping,
                            &binary(operation, integer(!signed, bits), Checked)
                        ),
                        Some(false),
                    );
                    assert_eq!(
                        corresponds(
                            &wrapping,
                            &binary(
                                operation,
                                integer(signed, if bits == 64 { 32 } else { 64 }),
                                Checked
                            )
                        ),
                        Some(false),
                    );
                }
            }
        }
    }

    #[test]
    fn wrapping_value_correspondence_rejects_other_operations_and_operand_substitution() {
        use ProductionOverflowContractV2::{Checked, Wrapping};
        let scalar = integer(false, 32);
        let expected = binary(ProductionSemanticBinaryOpV2::Add, scalar, Wrapping);
        let actual = binary(ProductionSemanticBinaryOpV2::Add, scalar, Checked);
        for mutation in 0..4 {
            let mut changed = actual.clone();
            let NormalizedScalarExpressionV1::Binary {
                operation,
                lhs,
                rhs,
                ..
            } = &mut changed
            else {
                unreachable!();
            };
            match mutation {
                0 => *operation = ProductionSemanticBinaryOpV2::Subtract,
                1 => std::mem::swap(lhs, rhs),
                2 => **lhs = NormalizedScalarExpressionV1::Symbol { symbol: 3, scalar },
                3 => **rhs = NormalizedScalarExpressionV1::Constant { bits: 2, scalar },
                _ => unreachable!(),
            }
            assert_eq!(
                corresponds(&expected, &changed),
                Some(false),
                "mutation {mutation}"
            );
        }
        for operation in [
            ProductionSemanticBinaryOpV2::Divide,
            ProductionSemanticBinaryOpV2::Remainder,
            ProductionSemanticBinaryOpV2::BitAnd,
            ProductionSemanticBinaryOpV2::ShiftLeft,
        ] {
            assert_eq!(
                corresponds(
                    &binary(operation, scalar, Wrapping),
                    &binary(operation, scalar, Checked)
                ),
                Some(false),
            );
        }
        for scalar in [
            ProductionSemanticScalarTypeV2::Float { bits: 32 },
            ProductionSemanticScalarTypeV2::Bool,
        ] {
            assert_eq!(
                corresponds(
                    &binary(ProductionSemanticBinaryOpV2::Add, scalar, Wrapping),
                    &binary(ProductionSemanticBinaryOpV2::Add, scalar, Checked),
                ),
                Some(false),
            );
        }
    }

    #[test]
    fn nested_value_correspondence_preserves_non_arithmetic_structure() {
        let expected = nested(ProductionOverflowContractV2::Wrapping);
        let actual = nested(ProductionOverflowContractV2::Checked);
        assert_eq!(corresponds(&expected, &actual), Some(true));
        assert_eq!(corresponds(&actual, &expected), Some(false));
        for mutation in 0..5 {
            let mut changed = actual.clone();
            let NormalizedScalarExpressionV1::Select {
                scalar,
                condition,
                when_true,
                when_false,
            } = &mut changed
            else {
                unreachable!();
            };
            match mutation {
                0 => *scalar = integer(true, 32),
                1 => std::mem::swap(when_true, when_false),
                2 => {
                    let NormalizedScalarExpressionV1::Compare { operation, .. } =
                        condition.as_mut()
                    else {
                        unreachable!()
                    };
                    *operation = ProductionSemanticComparisonV2::Equal;
                }
                3 => {
                    let NormalizedScalarExpressionV1::Cast { target, .. } = when_true.as_mut()
                    else {
                        unreachable!()
                    };
                    *target = integer(false, 64);
                }
                4 => {
                    let NormalizedScalarExpressionV1::Cast { operand, .. } = when_true.as_mut()
                    else {
                        unreachable!()
                    };
                    let NormalizedScalarExpressionV1::Unary { operation, .. } = operand.as_mut()
                    else {
                        unreachable!()
                    };
                    *operation = ProductionSemanticUnaryOpV2::Negate;
                }
                _ => unreachable!(),
            }
            assert_eq!(
                corresponds(&expected, &changed),
                Some(false),
                "mutation {mutation}"
            );
        }
        let load = NormalizedScalarExpressionV1::Load {
            site: SemanticAccessSiteV1 {
                block: 1,
                statement: Some(2),
                ordinal: 0,
            },
            scalar: integer(false, 32),
        };
        assert_eq!(corresponds(&load, &load), Some(true));
        let mut changed = load.clone();
        let NormalizedScalarExpressionV1::Load { site, .. } = &mut changed else {
            unreachable!()
        };
        site.ordinal = 1;
        assert_eq!(corresponds(&load, &changed), Some(false));
    }

    #[test]
    fn value_correspondence_charges_every_pair_even_for_identical_expressions() {
        let expected = nested(ProductionOverflowContractV2::Wrapping);
        let checked = nested(ProductionOverflowContractV2::Checked);
        for actual in [&expected, &checked] {
            // Select(1) + comparison(5) + cast/not/cast/product(6) + difference(3).
            for (remaining, result, leftover) in [
                (15, Some(true), 0),
                (14, None, 0),
                (16, Some(true), 1),
                (0, None, 0),
            ] {
                let mut budget = UnsupportedIndexCorrelationBudgetV1 { remaining };
                assert_eq!(
                    scalar_value_expressions_correspond_v1(&expected, actual, 0, &mut budget),
                    result
                );
                assert_eq!(budget.remaining, leftover);
            }
        }
    }

    #[test]
    fn value_correspondence_enforces_the_expression_depth_boundary() {
        let leaf = NormalizedScalarExpressionV1::Constant {
            scalar: integer(false, 32),
            bits: 1,
        };
        let limit = MAX_PRODUCTION_SEMANTIC_EXPRESSION_DEPTH_V2;
        assert_eq!(
            scalar_value_expressions_correspond_v1(
                &leaf,
                &leaf,
                limit,
                &mut UnsupportedIndexCorrelationBudgetV1 { remaining: 1 }
            ),
            Some(true),
        );
        assert_eq!(
            scalar_value_expressions_correspond_v1(
                &leaf,
                &leaf,
                limit + 1,
                &mut UnsupportedIndexCorrelationBudgetV1 { remaining: 1 }
            ),
            None,
        );
        let nested = binary(
            ProductionSemanticBinaryOpV2::Add,
            integer(false, 32),
            ProductionOverflowContractV2::Wrapping,
        );
        assert_eq!(
            scalar_value_expressions_correspond_v1(
                &nested,
                &nested,
                limit,
                &mut UnsupportedIndexCorrelationBudgetV1 { remaining: 3 }
            ),
            None,
        );
    }

    #[test]
    fn checked_arithmetic_normalization_accepts_only_its_value_result() {
        for (operator, expected) in [
            (
                CheckedBinaryOperator::Add,
                ProductionSemanticBinaryOpV2::Add,
            ),
            (
                CheckedBinaryOperator::Subtract,
                ProductionSemanticBinaryOpV2::Subtract,
            ),
            (
                CheckedBinaryOperator::Multiply,
                ProductionSemanticBinaryOpV2::Multiply,
            ),
        ] {
            let operation = Operation::checked_binary(
                ValueDef::new(ValueId(3), Type::Scalar(ScalarType::U32)),
                ValueDef::new(ValueId(4), Type::BOOL),
                operator,
                ValueId(1),
                ValueId(2),
            );
            assert_eq!(
                normalize_kir_binary_v1(BinaryOp::Checked(operator), &operation, ValueId(3)),
                Some((expected, ProductionOverflowContractV2::Checked))
            );
            for rejected in [ValueId(4), ValueId(99)] {
                assert_eq!(
                    normalize_kir_binary_v1(BinaryOp::Checked(operator), &operation, rejected),
                    None
                );
            }
        }
    }

    #[test]
    fn partial_plain_integer_arithmetic_cannot_claim_wrapping_value_correspondence() {
        for (operator, expected) in [
            (BinaryOp::Add, ProductionSemanticBinaryOpV2::Add),
            (BinaryOp::Subtract, ProductionSemanticBinaryOpV2::Subtract),
            (BinaryOp::Multiply, ProductionSemanticBinaryOpV2::Multiply),
        ] {
            for scalar in [
                ScalarType::I8,
                ScalarType::U8,
                ScalarType::I16,
                ScalarType::U16,
                ScalarType::I32,
                ScalarType::U32,
                ScalarType::I64,
                ScalarType::U64,
                ScalarType::I128,
                ScalarType::U128,
                ScalarType::Index,
                ScalarType::F32,
                ScalarType::F64,
            ] {
                let operation = Operation::new(
                    vec![ValueDef::new(ValueId(3), Type::Scalar(scalar))],
                    OperationKind::Binary {
                        op: operator,
                        lhs: ValueId(1),
                        rhs: ValueId(2),
                    },
                );
                let normalized = normalize_kir_binary_v1(operator, &operation, ValueId(3));
                if scalar.is_integer() {
                    assert_eq!(normalized, None, "{operator:?} {scalar:?}");
                } else {
                    assert_eq!(
                        normalized,
                        Some((expected, ProductionOverflowContractV2::Wrapping))
                    );
                }
            }
        }
    }
}
