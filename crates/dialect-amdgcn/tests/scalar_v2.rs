use std::collections::BTreeSet;
use std::ffi::OsString;
use std::path::PathBuf;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use dialect_amdgcn::{MAX_GFX942_SCALAR_LLVM_BYTES, lower_scalar_v2_to_gfx942_llvm};
use fe2o3_kernel_ir::ValueId;
use fe2o3_kernel_ir::scalar_ops_v2::*;

fn int(width: IntWidth, signed: bool) -> ScalarType {
    ScalarType::Int { width, signed }
}

fn carrier(operation: Operation) -> ScalarOperationV2 {
    let arity = match operation {
        Operation::IntegerUnary { .. } | Operation::FloatNeg { .. } | Operation::Cast { .. } => 1,
        Operation::IntegerBinary { .. }
        | Operation::Shift { .. }
        | Operation::IntegerCompare { .. }
        | Operation::FloatBinary { .. }
        | Operation::FloatCompare { .. }
        | Operation::FloatTotalCompare { .. } => 2,
    };
    ScalarOperationV2::new(operation, (0..arity).map(|id| ValueId(id as u32)).collect()).unwrap()
}

fn lower(operation: Operation) -> String {
    let scalar = carrier(operation);
    let first = lower_scalar_v2_to_gfx942_llvm(&scalar).unwrap();
    let second = lower_scalar_v2_to_gfx942_llvm(&scalar).unwrap();
    assert_eq!(first, second);
    assert!(first.len() <= MAX_GFX942_SCALAR_LLVM_BYTES);
    assert!(first.contains("target triple = \"amdgcn-amd-amdhsa\""));
    assert!(first.contains("\"target-cpu\"=\"gfx942\""));
    assert!(first.contains("\"unsafe-fp-math\"=\"false\""));
    assert!(!first.contains(" fast "));
    first
}

fn next_u128(state: &mut u128) -> u128 {
    *state ^= *state << 23;
    *state ^= *state >> 17;
    *state ^= *state << 26;
    *state
}

fn accepted_gfx942_operations() -> Vec<Operation> {
    let widths = [
        IntWidth::W8,
        IntWidth::W16,
        IntWidth::W32,
        IntWidth::W64,
        IntWidth::W128,
    ];
    let predicates = [
        Predicate::Eq,
        Predicate::Ne,
        Predicate::Lt,
        Predicate::Le,
        Predicate::Gt,
        Predicate::Ge,
    ];
    let mut operations = BTreeSet::new();
    for width in widths {
        for signed in [false, true] {
            let ty = int(width, signed);
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
                    if op != IntBinary::Rem || mode != IntMode::Saturating {
                        operations.insert(Operation::IntegerBinary { ty, op, mode });
                    }
                }
            }
            for op in [IntBinary::And, IntBinary::Or, IntBinary::Xor] {
                operations.insert(Operation::IntegerBinary {
                    ty,
                    op,
                    mode: IntMode::Wrapping,
                });
            }
            operations.insert(Operation::IntegerUnary {
                ty,
                op: IntUnary::Not,
                mode: IntMode::Wrapping,
            });
            if signed {
                for mode in [
                    IntMode::Checked,
                    IntMode::Wrapping,
                    IntMode::Overflowing,
                    IntMode::Saturating,
                ] {
                    operations.insert(Operation::IntegerUnary {
                        ty,
                        op: IntUnary::Neg,
                        mode,
                    });
                }
            }
            for predicate in predicates {
                operations.insert(Operation::IntegerCompare { ty, predicate });
            }
            operations.insert(Operation::Cast {
                from: ty,
                to: ScalarType::Bool,
                cast: Cast::IntToBoolChecked,
            });
            if width.bits() >= 32 {
                operations.insert(Operation::Cast {
                    from: ty,
                    to: ScalarType::Char,
                    cast: Cast::IntToCharChecked,
                });
            }
            for float in [FloatWidth::F32, FloatWidth::F64] {
                operations.insert(Operation::Cast {
                    from: ty,
                    to: ScalarType::Float(float),
                    cast: Cast::IntToFloat {
                        semantics: IntToFloatSemantics::RustAs,
                    },
                });
                if width.bits() == float.bits() {
                    operations.insert(Operation::Cast {
                        from: ty,
                        to: ScalarType::Float(float),
                        cast: Cast::Bitcast,
                    });
                }
            }
        }
    }
    for from_width in widths {
        for to_width in widths {
            for from_signed in [false, true] {
                for to_signed in [false, true] {
                    operations.insert(Operation::Cast {
                        from: int(from_width, from_signed),
                        to: int(to_width, to_signed),
                        cast: match from_width.bits().cmp(&to_width.bits()) {
                            std::cmp::Ordering::Less => Cast::IntExtend {
                                signed: from_signed,
                            },
                            std::cmp::Ordering::Equal => Cast::Bitcast,
                            std::cmp::Ordering::Greater => Cast::IntNarrow,
                        },
                    });
                }
            }
        }
    }
    for lhs_width in widths {
        for rhs_width in widths {
            for lhs_signed in [false, true] {
                for rhs_signed in [false, true] {
                    for direction in [ShiftDirection::Left, ShiftDirection::Right] {
                        for policy in [
                            ShiftPolicy::Checked,
                            ShiftPolicy::Wrapping,
                            ShiftPolicy::Overflowing,
                            ShiftPolicy::RustOperator {
                                overflow_checks: false,
                            },
                            ShiftPolicy::RustOperator {
                                overflow_checks: true,
                            },
                        ] {
                            operations.insert(Operation::Shift {
                                ty: int(lhs_width, lhs_signed),
                                rhs_ty: int(rhs_width, rhs_signed),
                                direction,
                                policy,
                            });
                        }
                    }
                }
            }
        }
    }
    for float in [FloatWidth::F32, FloatWidth::F64] {
        let ty = ScalarType::Float(float);
        for op in [
            FloatBinary::Add,
            FloatBinary::Sub,
            FloatBinary::Mul,
            FloatBinary::Rem,
        ] {
            operations.insert(Operation::FloatBinary {
                ty,
                op,
                semantics: FloatArithmeticSemantics::RustIeee754,
            });
        }
        operations.insert(Operation::FloatNeg {
            ty,
            semantics: FloatArithmeticSemantics::RustIeee754,
        });
        operations.insert(Operation::FloatTotalCompare { ty });
        for (policy, admitted) in [
            (FloatComparisonPolicy::RustPartialEq, &predicates[..2]),
            (FloatComparisonPolicy::RustPartialOrd, &predicates[2..]),
            (FloatComparisonPolicy::IeeeOrdered, &predicates[..]),
            (FloatComparisonPolicy::IeeeUnordered, &predicates[..]),
        ] {
            for &predicate in admitted {
                operations.insert(Operation::FloatCompare {
                    ty,
                    predicate,
                    policy,
                });
            }
        }
        for width in widths {
            for signed in [false, true] {
                let integer = int(width, signed);
                operations.insert(Operation::Cast {
                    from: ty,
                    to: integer,
                    cast: Cast::FloatToInt {
                        semantics: FloatToIntSemantics::RustSaturatingAs,
                    },
                });
                if width.bits() == float.bits() {
                    operations.insert(Operation::Cast {
                        from: ty,
                        to: integer,
                        cast: Cast::Bitcast,
                    });
                }
            }
        }
    }
    operations.insert(Operation::Cast {
        from: ScalarType::Float(FloatWidth::F32),
        to: ScalarType::Float(FloatWidth::F64),
        cast: Cast::FloatExtend,
    });
    operations.insert(Operation::Cast {
        from: ScalarType::Float(FloatWidth::F64),
        to: ScalarType::Float(FloatWidth::F32),
        cast: Cast::FloatNarrow,
    });
    for width in widths {
        for signed in [false, true] {
            operations.insert(Operation::Cast {
                from: ScalarType::Bool,
                to: int(width, signed),
                cast: Cast::BoolToInt,
            });
            operations.insert(Operation::Cast {
                from: ScalarType::Char,
                to: int(width, signed),
                cast: Cast::CharToInt,
            });
        }
    }
    operations.into_iter().collect()
}

fn combined_llvm_module(operations: &[Operation]) -> String {
    let mut declarations = BTreeSet::new();
    let mut definitions = Vec::with_capacity(operations.len());
    let mut attributes = None::<String>;
    for &operation in operations {
        let llvm = lower(operation);
        declarations.extend(
            llvm.lines()
                .filter(|line| line.starts_with("declare "))
                .map(str::to_owned),
        );
        let definition_start = llvm.find("define ").unwrap();
        let attributes_start = llvm.find("\nattributes #0").unwrap();
        definitions.push(llvm[definition_start..attributes_start].trim().to_owned());
        attributes = llvm
            .lines()
            .find(|line| line.starts_with("attributes #0"))
            .map(str::to_owned);
    }
    let mut module = String::from("target triple = \"amdgcn-amd-amdhsa\"\n\n");
    for declaration in declarations {
        module.push_str(&declaration);
        module.push('\n');
    }
    module.push('\n');
    for definition in definitions {
        module.push_str(&definition);
        module.push_str("\n\n");
    }
    module.push_str(attributes.as_deref().unwrap());
    module.push('\n');
    module
}

fn software_float_to_int(
    bits: u64,
    exponent_bits: u32,
    fraction_bits: u32,
    bias: i32,
    signed: bool,
) -> u128 {
    let sign = bits >> (exponent_bits + fraction_bits) != 0;
    let exponent_mask = (1_u64 << exponent_bits) - 1;
    let fraction_mask = (1_u64 << fraction_bits) - 1;
    let exponent_raw = (bits >> fraction_bits) & exponent_mask;
    let fraction = bits & fraction_mask;
    if exponent_raw == exponent_mask {
        if fraction != 0 {
            return 0;
        }
        return match (signed, sign) {
            (true, true) => i128::MIN as u128,
            (true, false) => i128::MAX as u128,
            (false, true) => 0,
            (false, false) => u128::MAX,
        };
    }
    let exponent = exponent_raw as i32 - bias;
    if exponent_raw == 0 || exponent < 0 {
        return 0;
    }
    let limit = if signed { 127 } else { 128 };
    if exponent >= limit {
        return match (signed, sign) {
            (true, true) => i128::MIN as u128,
            (true, false) => i128::MAX as u128,
            (false, true) => 0,
            (false, false) => u128::MAX,
        };
    }
    let significand = u128::from(fraction | (1_u64 << fraction_bits));
    let magnitude = if exponent >= fraction_bits as i32 {
        significand << (exponent as u32 - fraction_bits)
    } else {
        significand >> (fraction_bits - exponent as u32)
    };
    match (signed, sign) {
        (true, true) => 0_u128.wrapping_sub(magnitude),
        (true, false) | (false, false) => magnitude,
        (false, true) => 0,
    }
}

fn software_unsigned_div(dividend: u128, divisor: u128) -> (u128, u128) {
    let mut quotient = 0_u128;
    let mut remainder = 0_u128;
    for index in (0..128).rev() {
        remainder = (remainder << 1) | ((dividend >> index) & 1);
        if remainder >= divisor {
            remainder -= divisor;
            quotient |= 1_u128 << index;
        }
    }
    (quotient, remainder)
}

fn software_int_to_float_bits(value: u128, signed: bool, f32_result: bool) -> u64 {
    let negative = signed && value >> 127 != 0;
    let magnitude = if negative {
        0_u128.wrapping_sub(value)
    } else {
        value
    };
    if magnitude == 0 {
        return 0;
    }
    let (precision, fraction_bits, exponent_bits, bias) = if f32_result {
        (24_u32, 23_u32, 8_u32, 127_u32)
    } else {
        (53_u32, 52_u32, 11_u32, 1023_u32)
    };
    let bit_length = 128 - magnitude.leading_zeros();
    let mut significand = if bit_length <= precision {
        magnitude << (precision - bit_length)
    } else {
        let shift = bit_length - precision;
        let truncated = magnitude >> shift;
        let remainder = magnitude & ((1_u128 << shift) - 1);
        let half = 1_u128 << (shift - 1);
        truncated + u128::from(remainder > half || remainder == half && truncated & 1 != 0)
    };
    let carry = significand >> precision != 0;
    if carry {
        significand >>= 1;
    }
    let exponent = bit_length - 1 + bias + u32::from(carry);
    let fraction = significand & ((1_u128 << fraction_bits) - 1);
    let sign = u128::from(negative) << (exponent_bits + fraction_bits);
    (sign | (u128::from(exponent) << fraction_bits) | fraction) as u64
}

#[test]
fn every_integer_width_signedness_operation_and_mode_lowers_deterministically() {
    for width in [
        IntWidth::W8,
        IntWidth::W16,
        IntWidth::W32,
        IntWidth::W64,
        IntWidth::W128,
    ] {
        for signed in [false, true] {
            let ty = int(width, signed);
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
                    if op == IntBinary::Rem && mode == IntMode::Saturating {
                        continue;
                    }
                    let llvm = lower(Operation::IntegerBinary { ty, op, mode });
                    if mode == IntMode::Saturating
                        && matches!(op, IntBinary::Add | IntBinary::Sub | IntBinary::Mul)
                    {
                        assert!(llvm.contains(".with.overflow."));
                        assert!(!llvm.contains(".sat."));
                        assert!(llvm.contains("%result = select i1 %overflow"));
                    }
                    if matches!(op, IntBinary::Div | IntBinary::Rem) {
                        assert!(llvm.contains("%zero = icmp eq"));
                        if mode == IntMode::Checked {
                            assert!(!llvm.contains("call void @llvm.trap"));
                        } else {
                            assert!(llvm.contains("call void @llvm.trap"));
                        }
                    }
                }
            }
            for op in [IntBinary::And, IntBinary::Or, IntBinary::Xor] {
                lower(Operation::IntegerBinary {
                    ty,
                    op,
                    mode: IntMode::Wrapping,
                });
            }
            for predicate in [
                Predicate::Eq,
                Predicate::Ne,
                Predicate::Lt,
                Predicate::Le,
                Predicate::Gt,
                Predicate::Ge,
            ] {
                lower(Operation::IntegerCompare { ty, predicate });
            }
            lower(Operation::IntegerUnary {
                ty,
                op: IntUnary::Not,
                mode: IntMode::Wrapping,
            });
            if signed {
                for mode in [
                    IntMode::Checked,
                    IntMode::Wrapping,
                    IntMode::Overflowing,
                    IntMode::Saturating,
                ] {
                    lower(Operation::IntegerUnary {
                        ty,
                        op: IntUnary::Neg,
                        mode,
                    });
                }
            }
        }
    }
}

#[test]
fn shifts_preserve_the_full_typed_rhs_and_explicit_policy() {
    let widths = [
        IntWidth::W8,
        IntWidth::W16,
        IntWidth::W32,
        IntWidth::W64,
        IntWidth::W128,
    ];
    for lhs_width in widths {
        for rhs_width in widths {
            for lhs_signed in [false, true] {
                for rhs_signed in [false, true] {
                    for direction in [ShiftDirection::Left, ShiftDirection::Right] {
                        for policy in [
                            ShiftPolicy::Checked,
                            ShiftPolicy::Wrapping,
                            ShiftPolicy::Overflowing,
                            ShiftPolicy::RustOperator {
                                overflow_checks: false,
                            },
                            ShiftPolicy::RustOperator {
                                overflow_checks: true,
                            },
                        ] {
                            let llvm = lower(Operation::Shift {
                                ty: int(lhs_width, lhs_signed),
                                rhs_ty: int(rhs_width, rhs_signed),
                                direction,
                                policy,
                            });
                            assert!(llvm.contains("%rhs.raw"));
                            assert!(llvm.contains("%rhs.high"));
                            assert!(llvm.contains("%rhs.masked = and i128"));
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn floats_casts_bool_char_and_total_cmp_have_closed_surfaces() {
    for width in [FloatWidth::F32, FloatWidth::F64] {
        let ty = ScalarType::Float(width);
        for op in [
            FloatBinary::Add,
            FloatBinary::Sub,
            FloatBinary::Mul,
            FloatBinary::Rem,
        ] {
            lower(Operation::FloatBinary {
                ty,
                op,
                semantics: FloatArithmeticSemantics::RustIeee754,
            });
        }
        let division = carrier(Operation::FloatBinary {
            ty,
            op: FloatBinary::Div,
            semantics: FloatArithmeticSemantics::RustIeee754,
        });
        let error = lower_scalar_v2_to_gfx942_llvm(&division).unwrap_err();
        assert!(matches!(
            error,
            dialect_amdgcn::ScalarV2LoweringError::Unsupported(_)
        ));
        lower(Operation::FloatNeg {
            ty,
            semantics: FloatArithmeticSemantics::RustIeee754,
        });
        for (policy, predicates) in [
            (
                FloatComparisonPolicy::RustPartialEq,
                vec![Predicate::Eq, Predicate::Ne],
            ),
            (
                FloatComparisonPolicy::RustPartialOrd,
                vec![Predicate::Lt, Predicate::Le, Predicate::Gt, Predicate::Ge],
            ),
            (
                FloatComparisonPolicy::IeeeOrdered,
                vec![
                    Predicate::Eq,
                    Predicate::Ne,
                    Predicate::Lt,
                    Predicate::Le,
                    Predicate::Gt,
                    Predicate::Ge,
                ],
            ),
            (
                FloatComparisonPolicy::IeeeUnordered,
                vec![
                    Predicate::Eq,
                    Predicate::Ne,
                    Predicate::Lt,
                    Predicate::Le,
                    Predicate::Gt,
                    Predicate::Ge,
                ],
            ),
        ] {
            for predicate in predicates {
                lower(Operation::FloatCompare {
                    ty,
                    predicate,
                    policy,
                });
            }
        }
        let total = lower(Operation::FloatTotalCompare { ty });
        assert!(total.contains("%key0"));
        assert!(total.contains("ret i8 %result"));
    }

    for from in [FloatWidth::F32, FloatWidth::F64] {
        for width in [
            IntWidth::W8,
            IntWidth::W16,
            IntWidth::W32,
            IntWidth::W64,
            IntWidth::W128,
        ] {
            for signed in [false, true] {
                let llvm = lower(Operation::Cast {
                    from: ScalarType::Float(from),
                    to: int(width, signed),
                    cast: Cast::FloatToInt {
                        semantics: FloatToIntSemantics::RustSaturatingAs,
                    },
                });
                if width == IntWidth::W128 {
                    assert!(llvm.contains("%exponent.raw"));
                    assert!(!llvm.contains("fptosi i128"));
                    assert!(!llvm.contains("fptoui i128"));
                } else {
                    assert!(llvm.contains(".sat."));
                }
            }
        }
    }
    for width in [
        IntWidth::W8,
        IntWidth::W16,
        IntWidth::W32,
        IntWidth::W64,
        IntWidth::W128,
    ] {
        for signed in [false, true] {
            for to in [FloatWidth::F32, FloatWidth::F64] {
                let llvm = lower(Operation::Cast {
                    from: int(width, signed),
                    to: ScalarType::Float(to),
                    cast: Cast::IntToFloat {
                        semantics: IntToFloatSemantics::RustAs,
                    },
                });
                if width == IntWidth::W128 {
                    assert!(llvm.contains("@llvm.ctlz.i64"));
                    assert!(!llvm.contains("sitofp i128"));
                    assert!(!llvm.contains("uitofp i128"));
                }
            }
        }
    }
    lower(Operation::Cast {
        from: ScalarType::Bool,
        to: int(IntWidth::W64, false),
        cast: Cast::BoolToInt,
    });
    for signed in [false, true] {
        let same_width_char = lower(Operation::Cast {
            from: ScalarType::Char,
            to: int(IntWidth::W32, signed),
            cast: Cast::CharToInt,
        });
        assert!(same_width_char.contains("%result = add i32 %arg0, 0"));
        assert!(!same_width_char.contains("trunc i32 %arg0 to i32"));

        let char_check = lower(Operation::Cast {
            from: int(IntWidth::W32, signed),
            to: ScalarType::Char,
            cast: Cast::IntToCharChecked,
        });
        assert!(char_check.contains("%wide = add i32 %arg0, 0"));
        assert!(!char_check.contains("trunc i32 %arg0 to i32"));
        if signed {
            assert!(char_check.contains("%nonnegative = icmp sge i32 %arg0, 0"));
        }
    }
    let char_check = lower(Operation::Cast {
        from: int(IntWidth::W128, false),
        to: ScalarType::Char,
        cast: Cast::IntToCharChecked,
    });
    assert!(char_check.contains("icmp ule i128 %arg0, 1114111"));
    assert!(char_check.contains("%surrogate"));

    let signedness = lower(Operation::Cast {
        from: int(IntWidth::W64, true),
        to: int(IntWidth::W64, false),
        cast: Cast::Bitcast,
    });
    assert!(signedness.contains("%result = add i64 %arg0, 0"));
    assert!(!signedness.contains("bitcast i64 %arg0 to i64"));

    let i128_to_f64 = lower(Operation::Cast {
        from: int(IntWidth::W128, false),
        to: ScalarType::Float(FloatWidth::F64),
        cast: Cast::IntToFloat {
            semantics: IntToFloatSemantics::RustAs,
        },
    });
    assert!(i128_to_f64.contains("%exponent.storage = add i64 %exponent, 0"));
    assert!(!i128_to_f64.contains("trunc i64 %exponent to i64"));
}

#[test]
fn software_i128_boundaries_match_independent_cpu_semantics() {
    let mut state = 0xd1ff_e2e0_3a94_2d5b_1f12_7b89_00ca_11ab_u128;
    for _ in 0..120_000 {
        let random = next_u128(&mut state);
        let f32_bits = random as u32;
        let f64_bits = (random >> 32) as u64;
        let f32_value = f32::from_bits(f32_bits);
        let f64_value = f64::from_bits(f64_bits);
        assert_eq!(
            software_float_to_int(u64::from(f32_bits), 8, 23, 127, true),
            (f32_value as i128) as u128
        );
        assert_eq!(
            software_float_to_int(u64::from(f32_bits), 8, 23, 127, false),
            f32_value as u128
        );
        assert_eq!(
            software_float_to_int(f64_bits, 11, 52, 1023, true),
            (f64_value as i128) as u128
        );
        assert_eq!(
            software_float_to_int(f64_bits, 11, 52, 1023, false),
            f64_value as u128
        );

        let divisor = next_u128(&mut state) | 1;
        let (quotient, remainder) = software_unsigned_div(random, divisor);
        assert_eq!(quotient, random / divisor);
        assert_eq!(remainder, random % divisor);

        let signed_lhs = random as i128;
        let signed_rhs = divisor as i128;
        if signed_rhs != 0 && !(signed_lhs == i128::MIN && signed_rhs == -1) {
            let lhs_negative = signed_lhs < 0;
            let rhs_negative = signed_rhs < 0;
            let lhs_magnitude = signed_lhs.unsigned_abs();
            let rhs_magnitude = signed_rhs.unsigned_abs();
            let (magnitude_q, magnitude_r) = software_unsigned_div(lhs_magnitude, rhs_magnitude);
            let quotient = if lhs_negative ^ rhs_negative {
                0_u128.wrapping_sub(magnitude_q)
            } else {
                magnitude_q
            } as i128;
            let remainder = if lhs_negative {
                0_u128.wrapping_sub(magnitude_r)
            } else {
                magnitude_r
            } as i128;
            assert_eq!(quotient, signed_lhs / signed_rhs);
            assert_eq!(remainder, signed_lhs % signed_rhs);
        }

        assert_eq!(
            software_int_to_float_bits(random, true, true) as u32,
            (random as i128 as f32).to_bits()
        );
        assert_eq!(
            software_int_to_float_bits(random, true, false),
            (random as i128 as f64).to_bits()
        );
        assert_eq!(
            software_int_to_float_bits(random, false, true) as u32,
            (random as f32).to_bits()
        );
        assert_eq!(
            software_int_to_float_bits(random, false, false),
            (random as f64).to_bits()
        );
    }
}

#[test]
fn saturation_selection_matches_rust_cpu_semantics() {
    let mut state = 0xa613_5f92_89c4_7761_d0e8_4a2f_39b7_1c05_u128;
    macro_rules! check_signed {
        ($ty:ty) => {
            for _ in 0..20_000 {
                let left = next_u128(&mut state) as $ty;
                let right = next_u128(&mut state) as $ty;
                let (value, overflow) = left.overflowing_add(right);
                assert_eq!(
                    if overflow {
                        if left < 0 { <$ty>::MIN } else { <$ty>::MAX }
                    } else {
                        value
                    },
                    left.saturating_add(right)
                );
                let (value, overflow) = left.overflowing_sub(right);
                assert_eq!(
                    if overflow {
                        if left < 0 { <$ty>::MIN } else { <$ty>::MAX }
                    } else {
                        value
                    },
                    left.saturating_sub(right)
                );
                let (value, overflow) = left.overflowing_mul(right);
                assert_eq!(
                    if overflow {
                        if (left < 0) ^ (right < 0) {
                            <$ty>::MIN
                        } else {
                            <$ty>::MAX
                        }
                    } else {
                        value
                    },
                    left.saturating_mul(right)
                );
                let (value, overflow) = left.overflowing_neg();
                assert_eq!(
                    if overflow { <$ty>::MAX } else { value },
                    left.saturating_neg()
                );
            }
        };
    }
    macro_rules! check_unsigned {
        ($ty:ty) => {
            for _ in 0..20_000 {
                let left = next_u128(&mut state) as $ty;
                let right = next_u128(&mut state) as $ty;
                let (value, overflow) = left.overflowing_add(right);
                assert_eq!(
                    if overflow { <$ty>::MAX } else { value },
                    left.saturating_add(right)
                );
                let (value, overflow) = left.overflowing_sub(right);
                assert_eq!(if overflow { 0 } else { value }, left.saturating_sub(right));
                let (value, overflow) = left.overflowing_mul(right);
                assert_eq!(
                    if overflow { <$ty>::MAX } else { value },
                    left.saturating_mul(right)
                );
            }
        };
    }
    check_signed!(i8);
    check_signed!(i16);
    check_signed!(i32);
    check_signed!(i64);
    check_signed!(i128);
    check_unsigned!(u8);
    check_unsigned!(u16);
    check_unsigned!(u32);
    check_unsigned!(u64);
    check_unsigned!(u128);
}

#[test]
fn every_unicode_boundary_matches_rust_char_validity() {
    for bits in 0..=0x11_0000_u128 {
        assert_eq!(
            valid_char_bits(bits),
            u32::try_from(bits).ok().and_then(char::from_u32).is_some(),
            "char bits {bits:#x}"
        );
    }
    for bits in [u128::MAX, 1_u128 << 127, 0xffff_ffff] {
        assert!(!valid_char_bits(bits));
    }
}

#[test]
#[ignore = "requires ROCm clang with gfx942 support"]
fn rocm_clang_compiles_every_accepted_gfx942_scalar_path() {
    let operations = accepted_gfx942_operations();
    assert_eq!(operations.len(), 1_544, "incomplete operation matrix");
    let module = combined_llvm_module(&operations);
    assert!(!module.contains(".mul.sat."));
    assert!(!module.contains("constrained.fdiv"));
    assert!(!module.contains("trunc i64 %exponent to i64"));

    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "fe2o3-gfx942-scalar-matrix-{}-{nonce}",
        std::process::id()
    ));
    std::fs::create_dir(&root).unwrap();
    let llvm = root.join("all-scalar-paths.ll");
    std::fs::write(&llvm, module).unwrap();
    let clang = std::env::var_os("FE2O3_SCALAR_CLANG")
        .unwrap_or_else(|| OsString::from("/opt/rocm/llvm/bin/clang"));
    let mut failures = Vec::new();
    for optimization in ["-O0", "-O2"] {
        let object = root.join(format!("all-scalar-paths-{optimization}.o"));
        let output = Command::new(&clang)
            .args([
                "--target=amdgcn-amd-amdhsa",
                "-mcpu=gfx942",
                "-nogpulib",
                optimization,
                "-x",
                "ir",
                "-c",
            ])
            .arg(&llvm)
            .arg("-o")
            .arg(&object)
            .output()
            .expect("run gfx942 clang");
        if !output.status.success() {
            failures.push(format!(
                "{optimization}: {}",
                String::from_utf8_lossy(&output.stderr)
            ));
        }
    }
    let cleanup = std::fs::remove_dir_all(&root);
    assert!(
        failures.is_empty(),
        "{} rejected {} accepted scalar paths:\n{}",
        clang.to_string_lossy(),
        operations.len(),
        failures.join("\n")
    );
    cleanup.unwrap();
}

#[test]
fn representative_llvm_is_frozen_as_goldens() {
    let cases = [
        (
            "gfx942_scalar_checked_i128_add.ll",
            Operation::IntegerBinary {
                ty: int(IntWidth::W128, true),
                op: IntBinary::Add,
                mode: IntMode::Checked,
            },
        ),
        (
            "gfx942_scalar_wrapping_i64_div.ll",
            Operation::IntegerBinary {
                ty: int(IntWidth::W64, true),
                op: IntBinary::Div,
                mode: IntMode::Wrapping,
            },
        ),
        (
            "gfx942_scalar_f64_to_i128.ll",
            Operation::Cast {
                from: ScalarType::Float(FloatWidth::F64),
                to: int(IntWidth::W128, true),
                cast: Cast::FloatToInt {
                    semantics: FloatToIntSemantics::RustSaturatingAs,
                },
            },
        ),
        (
            "gfx942_scalar_i128_to_f32.ll",
            Operation::Cast {
                from: int(IntWidth::W128, true),
                to: ScalarType::Float(FloatWidth::F32),
                cast: Cast::IntToFloat {
                    semantics: IntToFloatSemantics::RustAs,
                },
            },
        ),
        (
            "gfx942_scalar_wrapping_i128_div.ll",
            Operation::IntegerBinary {
                ty: int(IntWidth::W128, true),
                op: IntBinary::Div,
                mode: IntMode::Wrapping,
            },
        ),
    ];
    let fixture_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    for (name, operation) in cases {
        let actual = lower(operation);
        let path = fixture_root.join(name);
        if std::env::var_os("FE2O3_BLESS_SCALAR_GOLDENS").is_some() {
            std::fs::write(&path, &actual).unwrap();
        }
        let expected = std::fs::read_to_string(&path).unwrap();
        assert_eq!(actual, expected, "golden mismatch at {}", path.display());
    }
}
