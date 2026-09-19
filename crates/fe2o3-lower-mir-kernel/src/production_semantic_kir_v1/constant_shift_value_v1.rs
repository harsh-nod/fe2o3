// Only the count operand of a fixed-width shift has this relation. It is not
// general expression folding, a dynamic range proof, or permission to mask.
fn fixed_shift_width_v1(scalar: ProductionSemanticScalarTypeV2) -> Option<u16> {
    match scalar {
        ProductionSemanticScalarTypeV2::Integer {
            bits: bits @ (8 | 16 | 32 | 64),
            ..
        } => Some(bits),
        _ => None,
    }
}

fn fixed_shift_literal_v1(expression: &NormalizedScalarExpressionV1) -> Option<u64> {
    let NormalizedScalarExpressionV1::Constant { scalar, bits } = expression else {
        return None;
    };
    let ProductionSemanticScalarTypeV2::Integer {
        signed,
        bits: width,
    } = *scalar
    else {
        return None;
    };
    fixed_shift_width_v1(*scalar)?;
    if (width < 64 && *bits >= (1_u64 << width)) || (signed && *bits & (1_u64 << (width - 1)) != 0)
    {
        return None;
    }
    Some(*bits)
}

fn fixed_native_shift_count_v1(
    expression: &NormalizedScalarExpressionV1,
    scalar: ProductionSemanticScalarTypeV2,
) -> Option<u64> {
    let width = u64::from(fixed_shift_width_v1(scalar)?);
    let literal = |expression: &NormalizedScalarExpressionV1| {
        let NormalizedScalarExpressionV1::Constant { scalar: other, .. } = expression else {
            return None;
        };
        (*other == scalar)
            .then(|| fixed_shift_literal_v1(expression))
            .flatten()
    };
    let count = if let Some(count) = literal(expression) {
        count
    } else {
        let NormalizedScalarExpressionV1::Binary {
            operation: ProductionSemanticBinaryOpV2::BitAnd,
            scalar: other,
            overflow: ProductionOverflowContractV2::Wrapping,
            lhs,
            rhs,
        } = expression
        else {
            return None;
        };
        if *other != scalar || literal(rhs)? != width - 1 {
            return None;
        }
        literal(lhs)?
    };
    (count < width).then_some(count)
}

fn constant_shift_counts_correspond_v1(
    expected: &NormalizedScalarExpressionV1,
    actual: &NormalizedScalarExpressionV1,
    scalar: ProductionSemanticScalarTypeV2,
    budget: &mut UnsupportedIndexCorrelationBudgetV1,
) -> Option<bool> {
    // At most one source literal and three native expression nodes. Charge
    // before reading any of them; retain the caller's shared correlation cap.
    for _ in 0..8 {
        budget.charge()?;
    }
    let Some(width) = fixed_shift_width_v1(scalar) else {
        return Some(false);
    };
    let Some(expected) = fixed_shift_literal_v1(expected) else {
        return Some(false);
    };
    Some(
        expected < u64::from(width)
            && fixed_native_shift_count_v1(actual, scalar) == Some(expected),
    )
}
