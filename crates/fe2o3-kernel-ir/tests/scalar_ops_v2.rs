#[path = "../src/scalar_ops_v2.rs"]
mod scalar_ops_v2;
use scalar_ops_v2::*;
use std::collections::BTreeSet;
use std::panic::{AssertUnwindSafe, catch_unwind};

const WIDTHS: [IntWidth; 5] = [
    IntWidth::W8,
    IntWidth::W16,
    IntWidth::W32,
    IntWidth::W64,
    IntWidth::W128,
];
fn int(w: IntWidth, signed: bool) -> ScalarType {
    ScalarType::Int { width: w, signed }
}
fn binary(ty: ScalarType, op: IntBinary, mode: IntMode) -> Operation {
    Operation::IntegerBinary { ty, op, mode }
}

#[test]
fn canonical_encoding_is_frozen_and_is_the_identity() {
    let op = binary(int(IntWidth::W32, true), IntBinary::Add, IntMode::Checked);
    let bytes = encode(op, FloatCapabilities::NONE).unwrap();
    assert_eq!(
        bytes,
        vec![
            b'F', b'E', b'2', b'O', b'S', b'V', b'2', 0, 3, 0, 8, 0, 0, 0, 0, 0, 1, 1, 1, 0, 3, 3,
            1, 0
        ]
    );
    assert_eq!(decode(&bytes, FloatCapabilities::NONE), Ok(op));
    assert_eq!(
        identity(op, FloatCapabilities::NONE)
            .unwrap()
            .canonical_bytes(),
        bytes
    );
}

#[test]
fn v3_shift_and_float_comparison_encodings_are_frozen() {
    let shift = Operation::Shift {
        ty: int(IntWidth::W32, false),
        rhs_ty: int(IntWidth::W128, true),
        direction: ShiftDirection::Left,
        policy: ShiftPolicy::RustOperator {
            overflow_checks: false,
        },
    };
    assert_eq!(
        encode(shift, FloatCapabilities::NONE).unwrap(),
        vec![
            b'F', b'E', b'2', b'O', b'S', b'V', b'2', 0, 3, 0, 12, 0, 0, 0, 0, 0, 3, 1, 5, 0, 3, 3,
            0, 0, 3, 5, 1, 0,
        ]
    );
    let compare = Operation::FloatCompare {
        ty: ScalarType::Float(FloatWidth::F64),
        predicate: Predicate::Ne,
        policy: FloatComparisonPolicy::IeeeUnordered,
    };
    assert_eq!(
        encode(compare, FloatCapabilities::ALL).unwrap(),
        vec![
            b'F', b'E', b'2', b'O', b'S', b'V', b'2', 0, 3, 0, 8, 0, 0, 0, 0, 0, 7, 2, 4, 0, 4, 3,
            0, 0,
        ]
    );
}

#[test]
fn every_width_signedness_operation_and_mode_has_distinct_identity() {
    let mut ids = BTreeSet::new();
    for w in WIDTHS {
        for signed in [false, true] {
            for op in [
                IntBinary::Add,
                IntBinary::Sub,
                IntBinary::Mul,
                IntBinary::Div,
                IntBinary::Rem,
            ] {
                for mode in [
                    IntMode::Checked,
                    IntMode::Wrapping,
                    IntMode::Overflowing,
                    IntMode::Saturating,
                ] {
                    if verify(binary(int(w, signed), op, mode), FloatCapabilities::NONE).is_ok() {
                        assert!(
                            ids.insert(
                                identity(binary(int(w, signed), op, mode), FloatCapabilities::NONE)
                                    .unwrap()
                            )
                        );
                    }
                }
            }
        }
    }
    assert_eq!(ids.len(), 190);
}

#[test]
fn exhaustive_u8_arithmetic_matches_rust_reference() {
    for a in 0u8..=u8::MAX {
        for b in 0u8..=u8::MAX {
            for (op, checked, wrapping, overflowing, saturating) in [
                (
                    IntBinary::Add,
                    a.checked_add(b),
                    a.wrapping_add(b),
                    a.overflowing_add(b),
                    a.saturating_add(b),
                ),
                (
                    IntBinary::Sub,
                    a.checked_sub(b),
                    a.wrapping_sub(b),
                    a.overflowing_sub(b),
                    a.saturating_sub(b),
                ),
                (
                    IntBinary::Mul,
                    a.checked_mul(b),
                    a.wrapping_mul(b),
                    a.overflowing_mul(b),
                    a.saturating_mul(b),
                ),
            ] {
                let ty = int(IntWidth::W8, false);
                assert_eq!(
                    evaluate_integer_binary(ty, op, IntMode::Checked, a.into(), b.into()).unwrap(),
                    checked.map_or(IntOutcome::CheckedNone, |v| IntOutcome::Value(v.into()))
                );
                assert_eq!(
                    evaluate_integer_binary(ty, op, IntMode::Wrapping, a.into(), b.into()),
                    Some(IntOutcome::Value(wrapping.into()))
                );
                assert_eq!(
                    evaluate_integer_binary(ty, op, IntMode::Overflowing, a.into(), b.into()),
                    Some(IntOutcome::Overflowing {
                        value: overflowing.0.into(),
                        overflowed: overflowing.1
                    })
                );
                assert_eq!(
                    evaluate_integer_binary(ty, op, IntMode::Saturating, a.into(), b.into()),
                    Some(IntOutcome::Value(saturating.into()))
                );
            }
        }
    }
}

#[test]
fn exhaustive_i8_arithmetic_matches_rust_reference() {
    for raw_a in 0u16..=255 {
        for raw_b in 0u16..=255 {
            let a = raw_a as u8 as i8;
            let b = raw_b as u8 as i8;
            for (op, checked, wrapping, overflowing, saturating) in [
                (
                    IntBinary::Add,
                    a.checked_add(b),
                    a.wrapping_add(b),
                    a.overflowing_add(b),
                    a.saturating_add(b),
                ),
                (
                    IntBinary::Sub,
                    a.checked_sub(b),
                    a.wrapping_sub(b),
                    a.overflowing_sub(b),
                    a.saturating_sub(b),
                ),
                (
                    IntBinary::Mul,
                    a.checked_mul(b),
                    a.wrapping_mul(b),
                    a.overflowing_mul(b),
                    a.saturating_mul(b),
                ),
            ] {
                let ty = int(IntWidth::W8, true);
                let enc = |v: i8| u128::from(v as u8);
                assert_eq!(
                    evaluate_integer_binary(ty, op, IntMode::Checked, raw_a.into(), raw_b.into())
                        .unwrap(),
                    checked.map_or(IntOutcome::CheckedNone, |v| IntOutcome::Value(enc(v)))
                );
                assert_eq!(
                    evaluate_integer_binary(ty, op, IntMode::Wrapping, raw_a.into(), raw_b.into()),
                    Some(IntOutcome::Value(enc(wrapping)))
                );
                assert_eq!(
                    evaluate_integer_binary(
                        ty,
                        op,
                        IntMode::Overflowing,
                        raw_a.into(),
                        raw_b.into()
                    ),
                    Some(IntOutcome::Overflowing {
                        value: enc(overflowing.0),
                        overflowed: overflowing.1
                    })
                );
                assert_eq!(
                    evaluate_integer_binary(
                        ty,
                        op,
                        IntMode::Saturating,
                        raw_a.into(),
                        raw_b.into()
                    ),
                    Some(IntOutcome::Value(enc(saturating)))
                );
            }
        }
    }
}

#[test]
fn division_remainder_exception_table_covers_all_widths() {
    for w in WIDTHS {
        let bits = w.bits();
        let min = 1u128 << (bits - 1);
        let neg_one = if bits == 128 {
            u128::MAX
        } else {
            (1u128 << bits) - 1
        };
        for op in [IntBinary::Div, IntBinary::Rem] {
            for mode in [
                IntMode::Checked,
                IntMode::Wrapping,
                IntMode::Overflowing,
                IntMode::Saturating,
            ] {
                if op == IntBinary::Rem && mode == IntMode::Saturating {
                    assert!(
                        verify(binary(int(w, true), op, mode), FloatCapabilities::NONE).is_err()
                    );
                    continue;
                }
                let zero = evaluate_integer_binary(int(w, true), op, mode, min, 0).unwrap();
                assert_eq!(
                    zero,
                    if mode == IntMode::Checked {
                        IntOutcome::CheckedNone
                    } else {
                        IntOutcome::Trap
                    }
                );
                let edge = evaluate_integer_binary(int(w, true), op, mode, min, neg_one).unwrap();
                match mode {
                    IntMode::Checked => assert_eq!(edge, IntOutcome::CheckedNone),
                    IntMode::Wrapping => assert_eq!(
                        edge,
                        IntOutcome::Value(if op == IntBinary::Div { min } else { 0 })
                    ),
                    IntMode::Overflowing => assert_eq!(
                        edge,
                        IntOutcome::Overflowing {
                            value: if op == IntBinary::Div { min } else { 0 },
                            overflowed: true
                        }
                    ),
                    IntMode::Saturating => {
                        assert_eq!(edge, IntOutcome::Value((1u128 << (bits - 1)) - 1))
                    }
                }
            }
        }
    }
}

#[test]
fn shifts_preserve_rhs_type_and_full_value() {
    for w in WIDTHS {
        let n = u128::from(w.bits());
        for signed in [false, true] {
            assert_eq!(
                evaluate_shift(
                    int(w, signed),
                    int(IntWidth::W128, false),
                    ShiftDirection::Left,
                    ShiftPolicy::Checked,
                    1,
                    n
                ),
                Some(IntOutcome::CheckedNone)
            );
            assert_eq!(
                evaluate_shift(
                    int(w, signed),
                    int(IntWidth::W128, false),
                    ShiftDirection::Left,
                    ShiftPolicy::Wrapping,
                    1,
                    n
                ),
                Some(IntOutcome::Value(1))
            );
            let high = 1u128 << (n - 1);
            let width_mask = if n == 128 {
                u128::MAX
            } else {
                (1u128 << n) - 1
            };
            let expected = width_mask ^ ((1u128 << (n - 2)) - 1);
            assert_eq!(
                evaluate_shift(
                    int(w, true),
                    int(IntWidth::W64, false),
                    ShiftDirection::Right,
                    ShiftPolicy::Wrapping,
                    high,
                    n + 1
                ),
                Some(IntOutcome::Value(expected))
            );
        }
    }
    for (rhs_ty, raw, wrapped) in [
        (int(IntWidth::W64, false), 1u128 << 32, 1),
        (int(IntWidth::W128, false), 1u128 << 96, 1),
        (int(IntWidth::W64, true), u64::MAX.into(), 1u128 << 31),
        (int(IntWidth::W128, true), u128::MAX, 1u128 << 31),
    ] {
        assert_eq!(
            evaluate_shift(
                int(IntWidth::W32, false),
                rhs_ty,
                ShiftDirection::Left,
                ShiftPolicy::Checked,
                1,
                raw,
            ),
            Some(IntOutcome::CheckedNone)
        );
        assert_eq!(
            evaluate_shift(
                int(IntWidth::W32, false),
                rhs_ty,
                ShiftDirection::Left,
                ShiftPolicy::Overflowing,
                1,
                raw,
            ),
            Some(IntOutcome::Overflowing {
                value: wrapped,
                overflowed: true,
            })
        );
        assert_eq!(
            evaluate_shift(
                int(IntWidth::W32, false),
                rhs_ty,
                ShiftDirection::Left,
                ShiftPolicy::RustOperator {
                    overflow_checks: true,
                },
                1,
                raw,
            ),
            Some(IntOutcome::Trap)
        );
        assert_eq!(
            evaluate_shift(
                int(IntWidth::W32, false),
                rhs_ty,
                ShiftDirection::Left,
                ShiftPolicy::RustOperator {
                    overflow_checks: false,
                },
                1,
                raw,
            ),
            Some(IntOutcome::Value(wrapped))
        );
    }
}

#[test]
fn randomized_shift_oracle_covers_all_rhs_widths() {
    let mut state = 0xa863_91c4_7e2d_5b0fu64;
    for iteration in 0..100_000 {
        state = xorshift(state);
        let lhs_width = WIDTHS[iteration % WIDTHS.len()];
        let rhs_width = WIDTHS[(iteration / WIDTHS.len()) % WIDTHS.len()];
        let lhs_signed = state & 1 != 0;
        let rhs_signed = state & 2 != 0;
        let value = u128::from(state) << 64 | u128::from(xorshift(state));
        let amount = u128::from(xorshift(xorshift(state))) << 64
            | u128::from(xorshift(xorshift(xorshift(state))));
        let lhs_ty = int(lhs_width, lhs_signed);
        let rhs_ty = int(rhs_width, rhs_signed);
        let normalized = amount & width_mask(rhs_width);
        let bits = u128::from(lhs_width.bits());
        let invalid = if rhs_signed {
            signed_value(normalized, rhs_width).is_negative()
                || signed_value(normalized, rhs_width) as u128 >= bits
        } else {
            normalized >= bits
        };
        let shift = (normalized % bits) as u32;
        let base = value & width_mask(lhs_width);
        let expected = if iteration & 1 == 0 {
            (base << shift) & width_mask(lhs_width)
        } else if lhs_signed {
            encode_signed_reference(signed_value(base, lhs_width) >> shift, lhs_width)
        } else {
            base >> shift
        };
        let direction = if iteration & 1 == 0 {
            ShiftDirection::Left
        } else {
            ShiftDirection::Right
        };
        assert_eq!(
            evaluate_shift(
                lhs_ty,
                rhs_ty,
                direction,
                ShiftPolicy::Checked,
                value,
                amount,
            ),
            Some(if invalid {
                IntOutcome::CheckedNone
            } else {
                IntOutcome::Value(expected)
            })
        );
        assert_eq!(
            evaluate_shift(
                lhs_ty,
                rhs_ty,
                direction,
                ShiftPolicy::Overflowing,
                value,
                amount,
            ),
            Some(IntOutcome::Overflowing {
                value: expected,
                overflowed: invalid,
            })
        );
    }
}

#[test]
fn bitwise_compare_and_unary_contracts_are_typed() {
    for w in WIDTHS {
        for signed in [false, true] {
            for op in [IntBinary::And, IntBinary::Or, IntBinary::Xor] {
                assert!(
                    verify(
                        binary(int(w, signed), op, IntMode::Wrapping),
                        FloatCapabilities::NONE
                    )
                    .is_ok()
                );
                assert!(
                    verify(
                        binary(int(w, signed), op, IntMode::Checked),
                        FloatCapabilities::NONE
                    )
                    .is_err()
                )
            }
            for p in [
                Predicate::Eq,
                Predicate::Ne,
                Predicate::Lt,
                Predicate::Le,
                Predicate::Gt,
                Predicate::Ge,
            ] {
                assert!(
                    verify(
                        Operation::IntegerCompare {
                            ty: int(w, signed),
                            predicate: p
                        },
                        FloatCapabilities::NONE
                    )
                    .is_ok()
                )
            }
            assert_eq!(
                verify(
                    Operation::IntegerUnary {
                        ty: int(w, signed),
                        op: IntUnary::Neg,
                        mode: IntMode::Checked
                    },
                    FloatCapabilities::NONE
                )
                .is_ok(),
                signed
            );
        }
    }
}

#[test]
fn float_arithmetic_comparisons_and_nan_are_capability_gated() {
    let widths = [
        FloatWidth::F16,
        FloatWidth::F32,
        FloatWidth::F64,
        FloatWidth::F128,
    ];
    let caps = FloatCapabilities::new(false, true, true, false);
    for w in widths {
        for op in [
            FloatBinary::Add,
            FloatBinary::Sub,
            FloatBinary::Mul,
            FloatBinary::Div,
            FloatBinary::Rem,
        ] {
            assert_eq!(
                verify(
                    Operation::FloatBinary {
                        ty: ScalarType::Float(w),
                        op,
                        semantics: FloatArithmeticSemantics::RustIeee754,
                    },
                    caps
                )
                .is_ok(),
                matches!(w, FloatWidth::F32 | FloatWidth::F64)
            )
        }
        for policy in [
            FloatComparisonPolicy::RustPartialEq,
            FloatComparisonPolicy::IeeeOrdered,
            FloatComparisonPolicy::IeeeUnordered,
        ] {
            assert_eq!(
                verify(
                    Operation::FloatCompare {
                        ty: ScalarType::Float(w),
                        predicate: Predicate::Eq,
                        policy,
                    },
                    caps
                )
                .is_ok(),
                matches!(w, FloatWidth::F32 | FloatWidth::F64)
            )
        }
    }
}

#[test]
fn float_comparison_policies_define_nan_and_result_surfaces() {
    let predicates = [
        Predicate::Eq,
        Predicate::Ne,
        Predicate::Lt,
        Predicate::Le,
        Predicate::Gt,
        Predicate::Ge,
    ];
    for predicate in predicates {
        let rust_policy = if matches!(predicate, Predicate::Eq | Predicate::Ne) {
            FloatComparisonPolicy::RustPartialEq
        } else {
            FloatComparisonPolicy::RustPartialOrd
        };
        let op = Operation::FloatCompare {
            ty: ScalarType::Float(FloatWidth::F64),
            predicate,
            policy: rust_policy,
        };
        assert!(verify(op, FloatCapabilities::ALL).is_ok());
        for (left, right) in [
            (f64::NAN, 1.0),
            (1.0, f64::NAN),
            (-0.0, 0.0),
            (f64::NEG_INFINITY, f64::INFINITY),
            (3.0, 3.0),
            (4.0, 3.0),
        ] {
            let expected = match predicate {
                Predicate::Eq => left == right,
                Predicate::Ne => left != right,
                Predicate::Lt => left < right,
                Predicate::Le => left <= right,
                Predicate::Gt => left > right,
                Predicate::Ge => left >= right,
            };
            assert_eq!(
                evaluate_float_compare_f64(rust_policy, predicate, left, right),
                Some(expected)
            );
            let unordered = left.is_nan() || right.is_nan();
            assert_eq!(
                evaluate_float_compare_f64(
                    FloatComparisonPolicy::IeeeOrdered,
                    predicate,
                    left,
                    right,
                ),
                Some(!unordered && expected)
            );
            assert_eq!(
                evaluate_float_compare_f64(
                    FloatComparisonPolicy::IeeeUnordered,
                    predicate,
                    left,
                    right,
                ),
                Some(unordered || expected)
            );
        }
    }
    for (policy, predicate) in [
        (FloatComparisonPolicy::RustPartialEq, Predicate::Lt),
        (FloatComparisonPolicy::RustPartialOrd, Predicate::Eq),
    ] {
        assert_eq!(
            evaluate_float_compare_f32(policy, predicate, f32::NAN, 0.0),
            None
        );
        assert!(matches!(
            verify(
                Operation::FloatCompare {
                    ty: ScalarType::Float(FloatWidth::F32),
                    predicate,
                    policy,
                },
                FloatCapabilities::ALL,
            ),
            Err(ds) if ds.contains(&Diagnostic::InvalidFloatComparison)
        ));
    }

    let total = Operation::FloatTotalCompare {
        ty: ScalarType::Float(FloatWidth::F64),
    };
    assert_eq!(
        decode(
            &encode(total, FloatCapabilities::ALL).unwrap(),
            FloatCapabilities::ALL
        ),
        Ok(total)
    );
    assert_eq!(
        evaluate_float_total_cmp_f64(-0.0, 0.0),
        (-0.0f64).total_cmp(&0.0)
    );
    assert_eq!(
        evaluate_float_total_cmp_f32(f32::from_bits(0xffc0_0001), f32::NEG_INFINITY),
        f32::from_bits(0xffc0_0001).total_cmp(&f32::NEG_INFINITY)
    );
}

#[test]
fn cast_matrix_enforces_width_validity_and_provenance() {
    let p64 = ScalarType::Pointer {
        address_space: 1,
        width: IntWidth::W64,
    };
    let p32 = ScalarType::Pointer {
        address_space: 3,
        width: IntWidth::W32,
    };
    let ok = [
        Operation::Cast {
            from: int(IntWidth::W8, true),
            to: int(IntWidth::W64, true),
            cast: Cast::IntExtend { signed: true },
        },
        Operation::Cast {
            from: int(IntWidth::W64, false),
            to: int(IntWidth::W8, true),
            cast: Cast::IntNarrow,
        },
        Operation::Cast {
            from: int(IntWidth::W8, false),
            to: int(IntWidth::W64, true),
            cast: Cast::IntExtend { signed: false },
        },
        Operation::Cast {
            from: ScalarType::Char,
            to: int(IntWidth::W8, true),
            cast: Cast::CharToInt,
        },
        Operation::Cast {
            from: p64,
            to: ScalarType::Pointer {
                address_space: 3,
                width: IntWidth::W64,
            },
            cast: Cast::PointerAddressSpace,
        },
        Operation::Cast {
            from: p64,
            to: int(IntWidth::W64, false),
            cast: Cast::PointerToInt {
                unsafe_policy: UnsafeProvenancePolicy::ExplicitProvenanceLoss,
            },
        },
        Operation::Cast {
            from: int(IntWidth::W64, false),
            to: p64,
            cast: Cast::IntToPointer {
                unsafe_policy: UnsafeProvenancePolicy::ExplicitProvenanceLoss,
            },
        },
    ];
    for op in ok {
        assert!(verify(op, FloatCapabilities::ALL).is_ok(), "{op:?}")
    }
    assert_eq!(
        evaluate_integer_binary(
            int(IntWidth::W8, false),
            IntBinary::And,
            IntMode::Checked,
            1,
            1,
        ),
        None
    );
    for op in [
        Operation::Cast {
            from: int(IntWidth::W64, true),
            to: int(IntWidth::W8, true),
            cast: Cast::IntExtend { signed: true },
        },
        Operation::Cast {
            from: p64,
            to: p32,
            cast: Cast::PointerAddressSpace,
        },
        Operation::Cast {
            from: p64,
            to: int(IntWidth::W32, false),
            cast: Cast::PointerToInt {
                unsafe_policy: UnsafeProvenancePolicy::ExplicitProvenanceLoss,
            },
        },
        Operation::Cast {
            from: int(IntWidth::W32, false),
            to: ScalarType::Float(FloatWidth::F64),
            cast: Cast::Bitcast,
        },
    ] {
        assert!(verify(op, FloatCapabilities::ALL).is_err(), "{op:?}")
    }
}

#[test]
fn bool_char_and_float_cast_boundaries_follow_rust() {
    for bits in 0..=3 {
        assert_eq!(valid_bool_bits(bits), bits <= 1)
    }
    for bits in [0, 0x41, 0xd7ff, 0xd800, 0xdfff, 0xe000, 0x10ffff, 0x110000] {
        assert_eq!(valid_char_bits(bits), char::from_u32(bits as u32).is_some())
    }
    assert_eq!(
        rust_saturating_float_to_int(f64::NAN, IntWidth::W32, true),
        0
    );
    assert_eq!(
        rust_saturating_float_to_int(f64::NEG_INFINITY, IntWidth::W8, true),
        128
    );
    assert_eq!(
        rust_saturating_float_to_int(f64::INFINITY, IntWidth::W8, true),
        127
    );
    assert_eq!(rust_saturating_float_to_int(-1.9, IntWidth::W8, false), 0);
    assert_eq!(rust_saturating_float_to_int(42.9, IntWidth::W8, false), 42);
}

#[test]
fn float_cast_thresholds_and_random_campaign_match_rust_as() {
    for width in WIDTHS {
        let signed_limit = 2.0f64.powi(i32::from(width.bits() - 1));
        let unsigned_limit = 2.0f64.powi(i32::from(width.bits()));
        let values = [
            f64::NAN,
            f64::from_bits(0xfff8_0000_0000_0001),
            f64::NEG_INFINITY,
            f64::INFINITY,
            -0.0,
            0.0,
            f64::from_bits(1),
            -f64::from_bits(1),
            -signed_limit,
            (-signed_limit).next_down(),
            (-signed_limit).next_up(),
            signed_limit,
            signed_limit.next_down(),
            signed_limit.next_up(),
            unsigned_limit,
            unsigned_limit.next_down(),
            unsigned_limit.next_up(),
        ];
        for value in values {
            assert_eq!(
                rust_saturating_float_to_int(value, width, true),
                rust_cast_f64(value, width, true),
                "signed {width:?} {value:?}"
            );
            assert_eq!(
                rust_saturating_float_to_int(value, width, false),
                rust_cast_f64(value, width, false),
                "unsigned {width:?} {value:?}"
            );
        }

        let signed_limit = signed_limit as f32;
        let unsigned_limit = unsigned_limit as f32;
        for value in [
            f32::NAN,
            f32::from_bits(0xffc0_0001),
            f32::NEG_INFINITY,
            f32::INFINITY,
            -0.0,
            0.0,
            f32::from_bits(1),
            -f32::from_bits(1),
            -signed_limit,
            (-signed_limit).next_down(),
            (-signed_limit).next_up(),
            signed_limit,
            signed_limit.next_down(),
            signed_limit.next_up(),
            unsigned_limit,
            unsigned_limit.next_down(),
            unsigned_limit.next_up(),
        ] {
            assert_eq!(
                rust_saturating_f32_to_int(value, width, true),
                rust_cast_f32(value, width, true),
                "signed {width:?} {value:?}"
            );
            assert_eq!(
                rust_saturating_f32_to_int(value, width, false),
                rust_cast_f32(value, width, false),
                "unsigned {width:?} {value:?}"
            );
        }
    }

    let mut state = 0x8f4d_3a29_61c7_b5e1u64;
    for iteration in 0..110_000 {
        state = xorshift(state);
        let value = f64::from_bits(state);
        let width = WIDTHS[iteration % WIDTHS.len()];
        for signed in [false, true] {
            assert_eq!(
                rust_saturating_float_to_int(value, width, signed),
                rust_cast_f64(value, width, signed)
            );
        }
    }
    for iteration in 0..110_000 {
        state = xorshift(state);
        let value = f32::from_bits(state as u32);
        let width = WIDTHS[iteration % WIDTHS.len()];
        for signed in [false, true] {
            assert_eq!(
                rust_saturating_f32_to_int(value, width, signed),
                rust_cast_f32(value, width, signed)
            );
        }
    }
}

#[test]
fn malformed_encoding_is_bounded_fail_closed_and_never_panics() {
    let base = encode(
        binary(
            int(IntWidth::W128, false),
            IntBinary::Mul,
            IntMode::Overflowing,
        ),
        FloatCapabilities::NONE,
    )
    .unwrap();
    for n in 0..base.len() {
        assert_eq!(
            decode(&base[..n], FloatCapabilities::NONE),
            Err(DecodeError::Truncated)
        )
    }
    let mut long = base.clone();
    long.resize(MAX_ENCODED_BYTES + 1, 0);
    assert!(matches!(
        decode(&long, FloatCapabilities::NONE),
        Err(DecodeError::ResourceLimit { .. })
    ));
    let mut trailing = base.clone();
    trailing.push(0);
    assert_eq!(
        decode(&trailing, FloatCapabilities::NONE),
        Err(DecodeError::TrailingBytes)
    );
    let corpus = [
        base,
        encode(
            Operation::Shift {
                ty: int(IntWidth::W8, true),
                rhs_ty: int(IntWidth::W128, true),
                direction: ShiftDirection::Right,
                policy: ShiftPolicy::Overflowing,
            },
            FloatCapabilities::NONE,
        )
        .unwrap(),
        encode(
            Operation::FloatCompare {
                ty: ScalarType::Float(FloatWidth::F64),
                predicate: Predicate::Ge,
                policy: FloatComparisonPolicy::IeeeUnordered,
            },
            FloatCapabilities::ALL,
        )
        .unwrap(),
        encode(
            Operation::FloatTotalCompare {
                ty: ScalarType::Float(FloatWidth::F32),
            },
            FloatCapabilities::ALL,
        )
        .unwrap(),
        encode(
            Operation::Cast {
                from: ScalarType::Float(FloatWidth::F64),
                to: int(IntWidth::W128, true),
                cast: Cast::FloatToInt {
                    semantics: FloatToIntSemantics::RustSaturatingAs,
                },
            },
            FloatCapabilities::ALL,
        )
        .unwrap(),
    ];
    let mut state = 0x4d595df4d0f33173u64;
    for iteration in 0..160_000 {
        state = xorshift(state);
        let mut bytes = corpus[iteration % corpus.len()].clone();
        let at = (state as usize) % bytes.len();
        bytes[at] ^= ((state >> 32) as u8) | 1;
        let decoded =
            catch_unwind(AssertUnwindSafe(|| decode(&bytes, FloatCapabilities::ALL))).unwrap();
        if let Ok(operation) = decoded {
            assert_eq!(
                encode(operation, FloatCapabilities::ALL).unwrap(),
                bytes,
                "successful decodes must already be canonical"
            );
        }
    }
}

fn xorshift(mut state: u64) -> u64 {
    state ^= state << 13;
    state ^= state >> 7;
    state ^ (state << 17)
}

fn width_mask(width: IntWidth) -> u128 {
    if width == IntWidth::W128 {
        u128::MAX
    } else {
        (1u128 << width.bits()) - 1
    }
}

fn signed_value(value: u128, width: IntWidth) -> i128 {
    if width == IntWidth::W128 {
        value as i128
    } else {
        let sign = 1u128 << (width.bits() - 1);
        if value & sign == 0 {
            value as i128
        } else {
            value as i128 - (1i128 << width.bits())
        }
    }
}

fn encode_signed_reference(value: i128, width: IntWidth) -> u128 {
    value as u128 & width_mask(width)
}

fn rust_cast_f64(value: f64, width: IntWidth, signed: bool) -> u128 {
    match (width, signed) {
        (IntWidth::W8, true) => (value as i8) as u8 as u128,
        (IntWidth::W16, true) => (value as i16) as u16 as u128,
        (IntWidth::W32, true) => (value as i32) as u32 as u128,
        (IntWidth::W64, true) => (value as i64) as u64 as u128,
        (IntWidth::W128, true) => (value as i128) as u128,
        (IntWidth::W8, false) => (value as u8) as u128,
        (IntWidth::W16, false) => (value as u16) as u128,
        (IntWidth::W32, false) => (value as u32) as u128,
        (IntWidth::W64, false) => (value as u64) as u128,
        (IntWidth::W128, false) => value as u128,
    }
}

fn rust_cast_f32(value: f32, width: IntWidth, signed: bool) -> u128 {
    match (width, signed) {
        (IntWidth::W8, true) => (value as i8) as u8 as u128,
        (IntWidth::W16, true) => (value as i16) as u16 as u128,
        (IntWidth::W32, true) => (value as i32) as u32 as u128,
        (IntWidth::W64, true) => (value as i64) as u64 as u128,
        (IntWidth::W128, true) => (value as i128) as u128,
        (IntWidth::W8, false) => (value as u8) as u128,
        (IntWidth::W16, false) => (value as u16) as u128,
        (IntWidth::W32, false) => (value as u32) as u128,
        (IntWidth::W64, false) => (value as u64) as u128,
        (IntWidth::W128, false) => value as u128,
    }
}

#[test]
fn reserved_unknown_and_semantically_invalid_encodings_are_distinct() {
    let base = encode(
        binary(int(IntWidth::W32, false), IntBinary::Add, IntMode::Checked),
        FloatCapabilities::NONE,
    )
    .unwrap();
    let mut x = base.clone();
    x[12] = 1;
    assert_eq!(
        decode(&x, FloatCapabilities::NONE),
        Err(DecodeError::ReservedNonZero)
    );
    x = base.clone();
    x[16] = 255;
    assert!(matches!(
        decode(&x, FloatCapabilities::NONE),
        Err(DecodeError::UnknownTag {
            field: "operation",
            tag: 255
        })
    ));
    x = base.clone();
    x[17] = 6;
    x[18] = 1;
    assert!(matches!(
        decode(&x, FloatCapabilities::NONE),
        Err(DecodeError::Invalid(_))
    ));
}

#[test]
fn boundary_table_roundtrips_all_widths_and_cast_families() {
    let mut operations = Vec::new();
    for w in WIDTHS {
        for signed in [false, true] {
            operations.push(Operation::Shift {
                ty: int(w, signed),
                rhs_ty: int(IntWidth::W128, !signed),
                direction: ShiftDirection::Right,
                policy: ShiftPolicy::Checked,
            });
            operations.push(Operation::IntegerCompare {
                ty: int(w, signed),
                predicate: Predicate::Ge,
            })
        }
    }
    operations.extend([
        Operation::Cast {
            from: ScalarType::Float(FloatWidth::F16),
            to: ScalarType::Float(FloatWidth::F128),
            cast: Cast::FloatExtend,
        },
        Operation::Cast {
            from: ScalarType::Float(FloatWidth::F128),
            to: ScalarType::Float(FloatWidth::F16),
            cast: Cast::FloatNarrow,
        },
        Operation::Cast {
            from: ScalarType::Float(FloatWidth::F64),
            to: int(IntWidth::W128, true),
            cast: Cast::FloatToInt {
                semantics: FloatToIntSemantics::RustSaturatingAs,
            },
        },
        Operation::Cast {
            from: ScalarType::Bool,
            to: int(IntWidth::W8, false),
            cast: Cast::BoolToInt,
        },
        Operation::Cast {
            from: int(IntWidth::W32, false),
            to: ScalarType::Char,
            cast: Cast::IntToCharChecked,
        },
    ]);
    for op in operations {
        let bytes = encode(op, FloatCapabilities::ALL).unwrap();
        assert!(bytes.len() <= MAX_ENCODED_BYTES);
        assert_eq!(decode(&bytes, FloatCapabilities::ALL), Ok(op));
        assert_eq!(
            encode(
                decode(&bytes, FloatCapabilities::ALL).unwrap(),
                FloatCapabilities::ALL
            )
            .unwrap(),
            bytes
        )
    }
}

#[test]
fn exceptional_mode_table_is_explicit() {
    for mode in [
        IntMode::Checked,
        IntMode::Wrapping,
        IntMode::Overflowing,
        IntMode::Saturating,
    ] {
        let s = exceptional_semantics(mode);
        assert_eq!(s.zero_returns_none, mode == IntMode::Checked);
        assert_eq!(s.zero_traps, mode != IntMode::Checked);
        assert_eq!(
            [
                s.min_neg_one_returns_none,
                s.min_neg_one_wraps,
                s.min_neg_one_overflows,
                s.min_neg_one_saturates
            ]
            .into_iter()
            .filter(|x| *x)
            .count(),
            1
        )
    }
}
