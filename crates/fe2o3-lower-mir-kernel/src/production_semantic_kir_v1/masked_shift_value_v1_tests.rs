mod masked_shift_value_v1_tests {
    use super::*;
    type E = NormalizedScalarExpressionV1;
    type S = ProductionSemanticScalarTypeV2;

    fn mask(scalar: S, input: E, bits: u64) -> E {
        E::Binary {
            operation: ProductionSemanticBinaryOpV2::BitAnd,
            scalar,
            overflow: ProductionOverflowContractV2::Wrapping,
            lhs: Box::new(input),
            rhs: Box::new(E::Constant { scalar, bits }),
        }
    }
    fn transported(source: S, target: S, value: E) -> E {
        if source == target {
            value
        } else {
            E::Cast {
                kind: ProductionSemanticCastV2::Integer,
                source,
                target,
                operand: Box::new(value),
            }
        }
    }
    fn shift(scalar: S, count: E) -> E {
        E::Binary {
            operation: ProductionSemanticBinaryOpV2::ShiftRight,
            scalar,
            overflow: ProductionOverflowContractV2::Wrapping,
            lhs: Box::new(E::Symbol { symbol: 1, scalar }),
            rhs: Box::new(count),
        }
    }
    fn agrees(expected: &E, actual: &E, cap: usize) -> Option<bool> {
        scalar_value_expressions_correspond_v1(
            expected,
            actual,
            0,
            &mut UnsupportedIndexCorrelationBudgetV1 { remaining: cap },
        )
    }

    #[test]
    fn actual_source_mask_binds_all_dynamic_bits_and_exact_fixed_integer_cast() {
        for signed in [false, true] {
            for bits in [8, 16, 32, 64] {
                let result = S::Integer { signed, bits };
                for count_signed in [false, true] {
                    for count_bits in [8, 16, 32, 64] {
                        let count = S::Integer {
                            signed: count_signed,
                            bits: count_bits,
                        };
                        let limit = u64::from(bits) - 1;
                        let raw = E::Symbol {
                            symbol: 2,
                            scalar: count,
                        };
                        let source_count = mask(count, raw.clone(), limit);
                        let expected = shift(result, source_count.clone());
                        let native = mask(result, transported(count, result, source_count), limit);
                        assert_eq!(agrees(&expected, &shift(result, native), 1000), Some(true));
                        let changed = mask(
                            count,
                            E::Symbol {
                                symbol: 3,
                                scalar: count,
                            },
                            limit,
                        );
                        let changed = mask(result, transported(count, result, changed), limit);
                        assert_eq!(
                            agrees(&expected, &shift(result, changed), 1000),
                            Some(false)
                        );
                        let repaired_raw =
                            mask(result, transported(count, result, raw.clone()), limit);
                        assert_eq!(
                            agrees(&shift(result, raw), &shift(result, repaired_raw), 1000),
                            Some(false)
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn ordinary_structural_shift_equality_is_not_count_admission_authority() {
        let scalar = S::Integer {
            signed: false,
            bits: 32,
        };
        let raw = E::Symbol { symbol: 2, scalar };
        let unchanged = shift(scalar, raw.clone());
        let repaired = shift(scalar, mask(scalar, raw, 31));
        assert_eq!(agrees(&unchanged, &unchanged, 1000), Some(true));
        assert_eq!(agrees(&unchanged, &repaired, 1000), Some(false));
        // Equal raw counts do not prove definedness. The source census still
        // rejects them, independently of this shared value relation.
        let source_mask = mask(scalar, E::Symbol { symbol: 2, scalar }, 31);
        let source = shift(scalar, source_mask.clone());
        let transport = shift(scalar, mask(scalar, source_mask, 31));
        for (expected, actual) in [(&unchanged, &unchanged), (&source, &transport)] {
            let exact = (1..1000)
                .find(|cap| agrees(expected, actual, *cap) == Some(true))
                .unwrap();
            assert_eq!(agrees(expected, actual, exact - 1), None);
            assert_eq!(agrees(expected, actual, exact), Some(true));
        }
    }

    #[test]
    fn source_mask_relation_is_rhs_only_and_rejects_wrong_mask_cast_and_overflow() {
        let scalar = S::Integer {
            signed: false,
            bits: 8,
        };
        let count = S::Integer {
            signed: false,
            bits: 32,
        };
        let source = mask(
            count,
            E::Symbol {
                symbol: 2,
                scalar: count,
            },
            7,
        );
        let expected = shift(scalar, source.clone());
        let actual = shift(
            scalar,
            mask(scalar, transported(count, scalar, source.clone()), 7),
        );
        assert_eq!(agrees(&expected, &actual, 1000), Some(true));
        assert_eq!(
            agrees(
                &source,
                &mask(scalar, transported(count, scalar, source.clone()), 7),
                1000
            ),
            Some(false)
        );
        for changed in [6, 15] {
            assert_eq!(
                agrees(
                    &expected,
                    &shift(
                        scalar,
                        mask(scalar, transported(count, scalar, source.clone()), changed)
                    ),
                    1000
                ),
                Some(false)
            );
        }
        let bad = E::Cast {
            kind: ProductionSemanticCastV2::FloatToIntegerSaturating,
            source: count,
            target: scalar,
            operand: Box::new(source.clone()),
        };
        assert_eq!(
            agrees(&expected, &shift(scalar, mask(scalar, bad, 7)), 1000),
            Some(false)
        );
        let mut changed = actual.clone();
        if let E::Binary { operation, .. } = &mut changed {
            *operation = ProductionSemanticBinaryOpV2::ShiftLeft;
        }
        assert_eq!(agrees(&expected, &changed, 1000), Some(false));
        let mut changed = actual.clone();
        if let E::Binary { overflow, .. } = &mut changed {
            *overflow = ProductionOverflowContractV2::Checked;
        }
        assert_eq!(agrees(&expected, &changed, 1000), Some(false));
        let exact = (1..1000)
            .find(|cap| agrees(&expected, &actual, *cap) == Some(true))
            .unwrap();
        assert_eq!(agrees(&expected, &actual, exact - 1), None);
    }
}
