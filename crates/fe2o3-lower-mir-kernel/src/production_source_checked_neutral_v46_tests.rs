#[test]
fn private_checked_neutral_values_preserve_separate_false_overflow_at_integer_boundaries() {
    use ProductionSemanticBinaryOpV2 as Op;
    use fe2o3_kernel_ir::scalar_ops_v2 as eval;
    for (bits, width) in [
        (8, eval::IntWidth::W8),
        (16, eval::IntWidth::W16),
        (32, eval::IntWidth::W32),
        (64, eval::IntWidth::W64),
    ] {
        let mask = u64::MAX >> (64 - bits);
        let sign = 1_u64 << (bits - 1);
        for signed in [false, true] {
            let scalar = ProductionSemanticScalarTypeV2::Integer { bits, signed };
            for (operation, evaluator, neutral) in [
                (Op::Add, eval::IntBinary::Add, 0),
                (Op::Subtract, eval::IntBinary::Sub, 0),
                (Op::Multiply, eval::IntBinary::Mul, 1),
            ] {
                for left in [false, true] {
                    if left && operation == Op::Subtract {
                        continue;
                    }
                    let mut expression = normalized_private_binary_v22(
                        operation,
                        scalar,
                        ProductionOverflowContractV2::Checked,
                        neutral,
                        left,
                    );
                    let mut work = IdentityWorkV22 { remaining: 24 };
                    assert_eq!(
                        source_private_integer_identity_v22(&mut expression, 0, &mut work),
                        Some(())
                    );
                    assert_eq!(work.remaining, 0);
                    assert!(matches!(expression,
                        NormalizedScalarExpressionV1::Symbol { symbol: 7, scalar: ty } if ty == scalar));
                    let check = |value: u64| {
                        let (lhs, rhs) = if left {
                            (neutral, value)
                        } else {
                            (value, neutral)
                        };
                        let ty = eval::ScalarType::Int { width, signed };
                        assert_eq!(
                            eval::evaluate_integer_binary(
                                ty,
                                evaluator,
                                eval::IntMode::Checked,
                                u128::from(lhs),
                                u128::from(rhs)
                            ),
                            Some(eval::IntOutcome::Value(u128::from(value)))
                        );
                        assert_eq!(
                            eval::evaluate_integer_binary(
                                ty,
                                evaluator,
                                eval::IntMode::Overflowing,
                                u128::from(lhs),
                                u128::from(rhs)
                            ),
                            Some(eval::IntOutcome::Overflowing {
                                value: u128::from(value),
                                overflowed: false,
                            })
                        );
                    };
                    if bits == 8 {
                        for value in 0..=mask {
                            check(value);
                        }
                    }
                    // Includes signed MIN/MAX and unsigned MAX, using bits.
                    for value in [0, 1, sign - 1, sign, mask - 1, mask] {
                        check(value);
                    }
                    let mut short = normalized_private_binary_v22(
                        operation,
                        scalar,
                        ProductionOverflowContractV2::Checked,
                        neutral,
                        left,
                    );
                    assert_eq!(
                        source_private_integer_identity_v22(
                            &mut short,
                            0,
                            &mut IdentityWorkV22 { remaining: 23 }
                        ),
                        None
                    );
                    assert!(matches!(short, NormalizedScalarExpressionV1::Binary { .. }));
                }
            }
        }
    }
}

#[test]
fn private_checked_neutral_rules_retain_partial_operations_and_distinct_operand_types() {
    use ProductionSemanticBinaryOpV2 as Op;
    for bits in [8, 16, 32, 64] {
        for signed in [false, true] {
            let scalar = ProductionSemanticScalarTypeV2::Integer { bits, signed };
            for (operation, constant, left) in [
                (Op::Add, 1, false),
                (Op::Subtract, 1, false),
                (Op::Subtract, 0, true),
                (Op::Multiply, 2, false),
                (Op::Multiply, u64::MAX >> (64 - bits), false),
                (Op::Divide, 1, false),
                (Op::Remainder, 1, false),
                (Op::ShiftLeft, 0, false),
                (Op::ShiftRight, 0, false),
                (Op::BitAnd, u64::MAX >> (64 - bits), false),
                (Op::BitOr, 0, false),
                (Op::BitXor, 0, false),
            ] {
                let mut expression = normalized_private_binary_v22(
                    operation,
                    scalar,
                    ProductionOverflowContractV2::Checked,
                    constant,
                    left,
                );
                source_private_integer_identity_v22(
                    &mut expression,
                    0,
                    &mut IdentityWorkV22 { remaining: 24 },
                )
                .unwrap();
                assert!(matches!(expression, NormalizedScalarExpressionV1::Binary {
                    operation: op, overflow: ProductionOverflowContractV2::Checked, ..
                } if op == operation));
            }
            let mut wrong_type = normalized_private_binary_v22(
                Op::Add,
                scalar,
                ProductionOverflowContractV2::Checked,
                0,
                false,
            );
            let NormalizedScalarExpressionV1::Binary { rhs, .. } = &mut wrong_type else {
                unreachable!()
            };
            rhs.0[0] = NormalizedScalarExpressionV1::Constant {
                scalar: ProductionSemanticScalarTypeV2::Integer {
                    bits,
                    signed: !signed,
                },
                bits: 0,
            };
            source_private_integer_identity_v22(
                &mut wrong_type,
                0,
                &mut IdentityWorkV22 { remaining: 24 },
            )
            .unwrap();
            assert!(matches!(
                wrong_type,
                NormalizedScalarExpressionV1::Binary { .. }
            ));
        }
    }
    for scalar in [
        ProductionSemanticScalarTypeV2::Float { bits: 32 },
        ProductionSemanticScalarTypeV2::Float { bits: 64 },
        ProductionSemanticScalarTypeV2::Integer {
            bits: 128,
            signed: false,
        },
    ] {
        let mut expression = normalized_private_binary_v22(
            Op::Multiply,
            scalar,
            ProductionOverflowContractV2::Checked,
            1,
            false,
        );
        source_private_integer_identity_v22(
            &mut expression,
            0,
            &mut IdentityWorkV22 { remaining: 24 },
        )
        .unwrap();
        assert!(matches!(
            expression,
            NormalizedScalarExpressionV1::Binary { .. }
        ));
    }
}

#[test]
fn private_checked_overflowing_values_remain_nonconstant_and_nonidentity() {
    use ProductionSemanticBinaryOpV2 as Op;
    for bits in [8, 16, 32, 64] {
        let sign = 1_u64 << (bits - 1);
        let mask = u64::MAX >> (64 - bits);
        for (signed, operation, lhs, rhs) in [
            (false, Op::Add, mask, 1),
            (false, Op::Subtract, 0, 1),
            (false, Op::Multiply, mask, 2),
            (true, Op::Add, sign - 1, 1),
            (true, Op::Subtract, sign, 1),
            (true, Op::Multiply, sign, mask),
        ] {
            let scalar = ProductionSemanticScalarTypeV2::Integer { bits, signed };
            let mut expression = NormalizedScalarExpressionV1::Binary {
                operation,
                scalar,
                overflow: ProductionOverflowContractV2::Checked,
                lhs: NormalizedScalarNodeV18::legacy(NormalizedScalarExpressionV1::Constant {
                    scalar,
                    bits: lhs,
                }),
                rhs: NormalizedScalarNodeV18::legacy(NormalizedScalarExpressionV1::Constant {
                    scalar,
                    bits: rhs,
                }),
            };
            source_scalar_constant_fold_v18(
                &mut expression,
                0,
                &mut IdentityWorkV22 { remaining: 3 },
            )
            .unwrap();
            source_private_integer_identity_v22(
                &mut expression,
                0,
                &mut IdentityWorkV22 { remaining: 24 },
            )
            .unwrap();
            assert!(matches!(
                expression,
                NormalizedScalarExpressionV1::Binary {
                    overflow: ProductionOverflowContractV2::Checked,
                    ..
                }
            ));
        }
    }
}
