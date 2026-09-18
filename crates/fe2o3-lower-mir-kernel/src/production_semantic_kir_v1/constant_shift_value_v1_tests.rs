#[cfg(test)]
mod constant_shift_value_v1_tests {
    use super::*;
    type E = NormalizedScalarExpressionV1;
    type S = ProductionSemanticScalarTypeV2;

    fn constant(scalar: S, bits: u64) -> E {
        E::Constant { scalar, bits }
    }
    fn masked(scalar: S, count: u64, mask: u64) -> E {
        E::Binary {
            operation: ProductionSemanticBinaryOpV2::BitAnd,
            scalar,
            overflow: ProductionOverflowContractV2::Wrapping,
            lhs: Box::new(constant(scalar, count)),
            rhs: Box::new(constant(scalar, mask)),
        }
    }
    fn agrees(expected: &E, actual: &E, scalar: S) -> bool {
        constant_shift_counts_correspond_v1(
            expected,
            actual,
            scalar,
            &mut UnsupportedIndexCorrelationBudgetV1 { remaining: 8 },
        )
        .unwrap()
    }

    fn shift(operation: ProductionSemanticBinaryOpV2, scalar: S, rhs: E) -> E {
        E::Binary {
            operation,
            scalar,
            overflow: ProductionOverflowContractV2::Wrapping,
            lhs: Box::new(E::Symbol { symbol: 41, scalar }),
            rhs: Box::new(rhs),
        }
    }

    fn expressions_agree(expected: &E, actual: &E, remaining: usize) -> Option<bool> {
        scalar_value_expressions_correspond_v1(
            expected,
            actual,
            0,
            &mut UnsupportedIndexCorrelationBudgetV1 { remaining },
        )
    }

    #[test]
    fn unchanged_shift_values_remain_structurally_reflexive_without_definedness_admission() {
        for operation in [
            ProductionSemanticBinaryOpV2::ShiftLeft,
            ProductionSemanticBinaryOpV2::ShiftRight,
        ] {
            for signed in [false, true] {
                // The 128-bit cases test the value relation only, not production admission.
                for bits in [8, 16, 32, 64, 128] {
                    let scalar = S::Integer { signed, bits };
                    for count in [0, 3, u64::from(bits) - 1, u64::from(bits), u64::MAX] {
                        let value = shift(operation, scalar, constant(scalar, count));
                        assert_eq!(expressions_agree(&value, &value, 3), Some(true));
                        assert_eq!(expressions_agree(&value, &value, 2), None);
                    }
                }
            }
        }
        let index = kir_semantic_scalar_v1(&Type::Scalar(ScalarType::Index)).unwrap();
        assert_eq!(
            index,
            S::Integer {
                signed: false,
                bits: 64
            }
        );
        let value = shift(
            ProductionSemanticBinaryOpV2::ShiftLeft,
            index,
            constant(index, 64),
        );
        assert_eq!(expressions_agree(&value, &value, 3), Some(true));
        assert!(kir_semantic_scalar_v1(&Type::Scalar(ScalarType::I128)).is_none());
        assert!(kir_semantic_scalar_v1(&Type::Scalar(ScalarType::U128)).is_none());
        let expression_count = shift(
            ProductionSemanticBinaryOpV2::ShiftLeft,
            index,
            masked(index, 3, 63),
        );
        assert_eq!(
            expressions_agree(&expression_count, &expression_count, 5),
            Some(true)
        );
        assert_eq!(
            expressions_agree(&expression_count, &expression_count, 4),
            None
        );
    }

    #[test]
    fn literal_shift_transport_is_additional_and_uses_the_same_exact_work_budget() {
        for operation in [
            ProductionSemanticBinaryOpV2::ShiftLeft,
            ProductionSemanticBinaryOpV2::ShiftRight,
        ] {
            for signed in [false, true] {
                for bits in [8, 16, 32, 64] {
                    let scalar = S::Integer { signed, bits };
                    let count_scalar = S::Integer {
                        signed: !signed,
                        bits: 32,
                    };
                    for count in [0, 3, u64::from(bits) - 1] {
                        let expected = shift(operation, scalar, constant(count_scalar, count));
                        for rhs in [
                            constant(scalar, count),
                            masked(scalar, count, u64::from(bits) - 1),
                        ] {
                            let actual = shift(operation, scalar, rhs);
                            assert_eq!(expressions_agree(&expected, &actual, 11), Some(true));
                            assert_eq!(expressions_agree(&expected, &actual, 10), None);
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn whole_shift_relation_does_not_repair_counts_or_relax_outer_obligations() {
        for signed in [false, true] {
            for bits in [8, 16, 32, 64] {
                let scalar = S::Integer { signed, bits };
                let operation = ProductionSemanticBinaryOpV2::ShiftLeft;
                let mask = u64::from(bits) - 1;
                for count in [u64::from(bits), 1_u64 << (bits - 1), u64::MAX] {
                    let expected = shift(operation, scalar, constant(scalar, count));
                    for native_count in [count, count & mask] {
                        let actual = shift(operation, scalar, masked(scalar, native_count, mask));
                        assert_eq!(expressions_agree(&expected, &actual, 11), Some(false));
                    }
                }
                let expected = shift(operation, scalar, constant(scalar, 3));
                for rhs in [
                    constant(scalar, 4),
                    masked(scalar, 4, mask),
                    masked(scalar, 3, mask - 1),
                ] {
                    assert_eq!(
                        expressions_agree(&expected, &shift(operation, scalar, rhs), 11),
                        Some(false)
                    );
                }
                let changed_direction = shift(
                    ProductionSemanticBinaryOpV2::ShiftRight,
                    scalar,
                    masked(scalar, 3, mask),
                );
                assert_eq!(
                    expressions_agree(&expected, &changed_direction, 11),
                    Some(false)
                );
                let other = S::Integer {
                    signed: !signed,
                    bits,
                };
                let changed_scalar = shift(operation, other, masked(other, 3, mask));
                assert_eq!(
                    expressions_agree(&expected, &changed_scalar, 11),
                    Some(false)
                );
                let changed_overflow = E::Binary {
                    operation,
                    scalar,
                    overflow: ProductionOverflowContractV2::Checked,
                    lhs: Box::new(E::Symbol { symbol: 41, scalar }),
                    rhs: Box::new(masked(scalar, 3, mask)),
                };
                assert_eq!(
                    expressions_agree(&expected, &changed_overflow, 11),
                    Some(false)
                );
                let changed_lhs = E::Binary {
                    operation,
                    scalar,
                    overflow: ProductionOverflowContractV2::Wrapping,
                    lhs: Box::new(E::Symbol { symbol: 42, scalar }),
                    rhs: Box::new(masked(scalar, 3, mask)),
                };
                assert_eq!(expressions_agree(&expected, &changed_lhs, 11), Some(false));
            }
        }
    }

    #[test]
    fn heterogeneous_exact_literal_counts_match_only_the_exact_native_width_mask() {
        for signed in [false, true] {
            for bits in [8, 16, 32, 64] {
                let scalar = S::Integer { signed, bits };
                for rhs_signed in [false, true] {
                    for rhs_bits in [8, 16, 32, 64] {
                        let rhs = S::Integer {
                            signed: rhs_signed,
                            bits: rhs_bits,
                        };
                        for count in [0, 3, u64::from(bits) - 1] {
                            let expected = constant(rhs, count);
                            assert!(agrees(&expected, &constant(scalar, count), scalar));
                            assert!(agrees(
                                &expected,
                                &masked(scalar, count, u64::from(bits) - 1),
                                scalar
                            ));
                            assert!(!agrees(
                                &expected,
                                &masked(scalar, count, u64::from(bits) - 2),
                                scalar
                            ));
                            assert!(!agrees(&expected, &constant(scalar, count + 1), scalar));
                        }
                    }
                }
                for count in [u64::from(bits), u64::MAX] {
                    assert!(!agrees(
                        &constant(scalar, count),
                        &masked(scalar, count, u64::from(bits) - 1),
                        scalar
                    ));
                }
                let dynamic = E::Symbol { symbol: 1, scalar };
                assert!(!agrees(&dynamic, &dynamic, scalar));
                let mut budget = UnsupportedIndexCorrelationBudgetV1 { remaining: 7 };
                assert_eq!(
                    constant_shift_counts_correspond_v1(
                        &constant(scalar, 3),
                        &masked(scalar, 3, u64::from(bits) - 1),
                        scalar,
                        &mut budget
                    ),
                    None
                );
            }
        }
    }
}
