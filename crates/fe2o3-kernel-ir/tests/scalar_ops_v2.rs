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
            b'F', b'E', b'2', b'O', b'S', b'V', b'2', 0, 2, 0, 8, 0, 0, 0, 0, 0, 1, 1, 1, 0, 3, 3,
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
fn shifts_have_only_checked_or_masked_semantics() {
    for w in WIDTHS {
        let n = u32::from(w.bits());
        for signed in [false, true] {
            assert_eq!(
                evaluate_shift(
                    int(w, signed),
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
                    ShiftDirection::Left,
                    ShiftPolicy::Masked,
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
                    ShiftDirection::Right,
                    ShiftPolicy::Masked,
                    high,
                    n + 1
                ),
                Some(IntOutcome::Value(expected))
            );
        }
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
        for nan in [NaNSemantics::Ordered, NaNSemantics::RustPartialEq] {
            assert_eq!(
                verify(
                    Operation::FloatCompare {
                        ty: ScalarType::Float(w),
                        predicate: Predicate::Ne,
                        nan
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
    let mut state = 0x4d595df4d0f33173u64;
    for _ in 0..20_000 {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        let mut bytes = base.clone();
        let at = (state as usize) % bytes.len();
        bytes[at] ^= ((state >> 32) as u8) | 1;
        assert!(catch_unwind(AssertUnwindSafe(|| decode(&bytes, FloatCapabilities::ALL))).is_ok());
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
