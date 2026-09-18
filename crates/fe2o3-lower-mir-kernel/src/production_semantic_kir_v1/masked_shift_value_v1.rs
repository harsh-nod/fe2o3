fn masked_shift_expression_parts_v1(
    expression: &NormalizedScalarExpressionV1,
    result_scalar: ProductionSemanticScalarTypeV2,
) -> Option<(
    &NormalizedScalarExpressionV1,
    ProductionSemanticScalarTypeV2,
)> {
    let width = u64::from(fixed_shift_width_v1(result_scalar)?);
    let NormalizedScalarExpressionV1::Binary {
        operation: ProductionSemanticBinaryOpV2::BitAnd,
        scalar,
        overflow: ProductionOverflowContractV2::Wrapping,
        lhs,
        rhs,
    } = expression
    else {
        return None;
    };
    fixed_shift_width_v1(*scalar)?;
    let NormalizedScalarExpressionV1::Constant {
        scalar: literal_scalar,
        ..
    } = rhs.as_ref()
    else {
        return None;
    };
    (*literal_scalar == *scalar && fixed_shift_literal_v1(rhs)? == width - 1)
        .then_some((lhs, *scalar))
}

fn masked_shift_counts_correspond_v1(
    expected: &NormalizedScalarExpressionV1,
    actual: &NormalizedScalarExpressionV1,
    scalar: ProductionSemanticScalarTypeV2,
    depth: usize,
    budget: &mut UnsupportedIndexCorrelationBudgetV1,
) -> Option<bool> {
    // Fixed shape checks are prepaid; recursive comparison spends this same
    // caller budget and inherits its depth. Only a shift RHS uses this rule.
    for _ in 0..16 {
        budget.charge()?;
    }
    let Some((_, source_scalar)) = masked_shift_expression_parts_v1(expected, scalar) else {
        return Some(false);
    };
    // The caller already tried ordinary structural correspondence with this
    // same ledger/depth. Only the additional positional transport remains.
    let transport = if let Some((transport, native_scalar)) =
        masked_shift_expression_parts_v1(actual, scalar)
    {
        if native_scalar != scalar {
            return Some(false);
        }
        transport
    } else if matches!(actual, NormalizedScalarExpressionV1::Cast { .. }) {
        actual
    } else {
        return Some(false);
    };
    if source_scalar == scalar {
        return scalar_value_expressions_correspond_v1(expected, transport, depth, budget);
    }
    let NormalizedScalarExpressionV1::Cast {
        kind,
        source,
        target,
        operand,
    } = transport
    else {
        return Some(false);
    };
    // Normalization is downstream of verified KIR: valid_scalar_cast requires
    // the exact fixed integer cast plan, including extension signedness. The
    // normalized Integer kind does not itself carry that lower-level opcode.
    if *source != source_scalar || *target != scalar || *kind != ProductionSemanticCastV2::Integer {
        return Some(false);
    }
    scalar_value_expressions_correspond_v1(expected, operand, depth, budget)
}

fn native_masked_shift_count_v1(
    expression: &NormalizedScalarExpressionV1,
    scalar: ProductionSemanticScalarTypeV2,
) -> bool {
    if masked_shift_expression_parts_v1(expression, scalar)
        .is_some_and(|(_, actual)| actual == scalar)
    {
        return true;
    }
    let NormalizedScalarExpressionV1::Cast {
        kind: ProductionSemanticCastV2::Integer,
        source,
        target,
        operand,
    } = expression
    else {
        return false;
    };
    *target == scalar
        && fixed_shift_width_v1(scalar).is_some()
        && masked_shift_expression_parts_v1(operand, scalar)
            .is_some_and(|(_, actual)| actual == *source)
}

#[cfg(test)]
include!("masked_shift_value_v1_tests.rs");
