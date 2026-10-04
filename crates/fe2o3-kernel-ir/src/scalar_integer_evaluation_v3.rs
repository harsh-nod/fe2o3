// Shared allocation-free integer evaluators. Reuse the existing width/mask,
// signed decoding and binary arithmetic; these do not alter scalar wire IDs.

/// Exact integer unary evaluation. Unsupported type/mode pairs return None
/// without constructing diagnostics or allocating.
pub fn evaluate_integer_unary(
    ty: ScalarType,
    op: IntUnary,
    mode: IntMode,
    value: u128,
) -> Option<IntOutcome> {
    let (width, signed) = int_parts(ty)?;
    match op {
        IntUnary::Neg if signed => evaluate_integer_binary(ty, IntBinary::Sub, mode, 0, value),
        IntUnary::Not if mode == IntMode::Wrapping => {
            evaluate_integer_binary(ty, IntBinary::Xor, mode, value, mask(width))
        }
        IntUnary::Neg | IntUnary::Not => None,
    }
}

/// Exact fixed-width integer comparison, including signed 128-bit values.
pub fn evaluate_integer_compare(
    ty: ScalarType,
    predicate: Predicate,
    left: u128,
    right: u128,
) -> Option<bool> {
    let (width, signed) = int_parts(ty)?;
    let left = left & mask(width);
    let right = right & mask(width);
    let ordering = if signed {
        decode_signed(left, width).cmp(&decode_signed(right, width))
    } else {
        left.cmp(&right)
    };
    Some(match predicate {
        Predicate::Eq => ordering.is_eq(),
        Predicate::Ne => !ordering.is_eq(),
        Predicate::Lt => ordering.is_lt(),
        Predicate::Le => !ordering.is_gt(),
        Predicate::Gt => ordering.is_gt(),
        Predicate::Ge => !ordering.is_lt(),
    })
}

/// Fixed-width integer/bool cast subset of the existing scalar contract.
/// Unsupported floating/pointer/char conversions return None without allocation.
pub fn evaluate_integer_cast(
    from: ScalarType,
    to: ScalarType,
    cast: Cast,
    value: u128,
) -> Option<u128> {
    if cast == Cast::BoolToInt && from == ScalarType::Bool && valid_bool_bits(value) {
        let (width, _) = int_parts(to)?;
        return Some(value & mask(width));
    }
    let (source, signed) = int_parts(from)?;
    let (target, _) = int_parts(to)?;
    let value = value & mask(source);
    match cast {
        Cast::IntExtend {
            signed: extend_signed,
        } if source.bits() < target.bits() && signed == extend_signed => Some(if signed {
            encode_signed(decode_signed(value, source), target)
        } else {
            value
        }),
        Cast::IntNarrow if source.bits() > target.bits() => Some(value & mask(target)),
        Cast::Bitcast if source.bits() == target.bits() => Some(value),
        _ => None,
    }
}

#[cfg(test)]
mod canonical_sparse_integer_evaluator_tests {
    use super::*;
    fn integer(width: IntWidth, signed: bool) -> ScalarType {
        ScalarType::Int { width, signed }
    }

    #[test]
    fn unary_compare_and_cast_reuse_exact_width_and_signed_carriers() {
        let signed = integer(IntWidth::W8, true);
        let unsigned = integer(IntWidth::W8, false);
        assert_eq!(
            evaluate_integer_unary(signed, IntUnary::Neg, IntMode::Wrapping, 128),
            Some(IntOutcome::Value(128))
        );
        assert_eq!(
            evaluate_integer_unary(unsigned, IntUnary::Not, IntMode::Wrapping, 1),
            Some(IntOutcome::Value(254))
        );
        assert_eq!(
            evaluate_integer_compare(signed, Predicate::Lt, 255, 1),
            Some(true)
        );
        assert_eq!(
            evaluate_integer_compare(unsigned, Predicate::Lt, 255, 1),
            Some(false)
        );
        assert_eq!(
            evaluate_integer_cast(
                signed,
                integer(IntWidth::W128, true),
                Cast::IntExtend { signed: true },
                255
            ),
            Some(u128::MAX)
        );
        assert_eq!(
            evaluate_integer_cast(
                integer(IntWidth::W16, false),
                unsigned,
                Cast::IntNarrow,
                0x1234
            ),
            Some(0x34)
        );
        assert_eq!(
            evaluate_integer_cast(ScalarType::Bool, unsigned, Cast::BoolToInt, 1),
            Some(1)
        );
        assert_eq!(
            evaluate_integer_cast(ScalarType::Bool, unsigned, Cast::BoolToInt, 2),
            None
        );
        assert_eq!(
            evaluate_integer_unary(unsigned, IntUnary::Neg, IntMode::Wrapping, 1),
            None
        );
        assert_eq!(
            evaluate_integer_cast(signed, unsigned, Cast::IntNarrow, 255),
            None
        );
    }
}
