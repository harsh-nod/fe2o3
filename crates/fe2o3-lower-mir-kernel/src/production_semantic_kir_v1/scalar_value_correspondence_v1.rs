include!("constant_shift_value_v1.rs");
include!("masked_shift_value_v1.rs");

// This compares scalar values, not operation-definedness or overflow flags.
#[cfg(test)]
include!("constant_shift_value_v1_tests.rs");

// A checked KIR pair's first result implements modular source arithmetic;
// the reverse implication would discard the source's no-overflow obligation.
fn scalar_value_expressions_correspond_v1(
    expected: &NormalizedScalarExpressionV1,
    actual: &NormalizedScalarExpressionV1,
    depth: usize,
    budget: &mut UnsupportedIndexCorrelationBudgetV1,
) -> Option<bool> {
    use NormalizedScalarExpressionV1 as E;
    budget.charge()?;
    if depth > MAX_PRODUCTION_SEMANTIC_EXPRESSION_DEPTH_V2 {
        return None;
    }
    let next = depth.checked_add(1)?;
    let compare = |expected, actual, budget: &mut UnsupportedIndexCorrelationBudgetV1| {
        scalar_value_expressions_correspond_v1(expected, actual, next, budget)
    };
    Some(match (expected, actual) {
        (
            E::Symbol {
                symbol: left,
                scalar,
            },
            E::Symbol {
                symbol: right,
                scalar: other,
            },
        ) => left == right && scalar == other,
        (
            E::Constant { scalar, bits },
            E::Constant {
                scalar: other,
                bits: other_bits,
            },
        ) => scalar == other && bits == other_bits,
        (
            E::Load { site, scalar },
            E::Load {
                site: other_site,
                scalar: other,
            },
        ) => site == other_site && scalar == other,
        (
            E::Unary {
                operation,
                scalar,
                operand,
            },
            E::Unary {
                operation: other_op,
                scalar: other,
                operand: other_operand,
            },
        ) => operation == other_op && scalar == other && compare(operand, other_operand, budget)?,
        (
            E::Binary {
                operation,
                scalar,
                overflow,
                lhs,
                rhs,
            },
            E::Binary {
                operation: other_op,
                scalar: other,
                overflow: other_overflow,
                lhs: other_lhs,
                rhs: other_rhs,
            },
        ) => {
            let modular_value = *overflow == ProductionOverflowContractV2::Wrapping
                && *other_overflow == ProductionOverflowContractV2::Checked
                && scalar.is_integer()
                && matches!(
                    operation,
                    ProductionSemanticBinaryOpV2::Add
                        | ProductionSemanticBinaryOpV2::Subtract
                        | ProductionSemanticBinaryOpV2::Multiply
                );
            operation == other_op
                && scalar == other
                && (overflow == other_overflow || modular_value)
                && compare(lhs, other_lhs, budget)?
                && if matches!(
                    operation,
                    ProductionSemanticBinaryOpV2::ShiftLeft
                        | ProductionSemanticBinaryOpV2::ShiftRight
                ) {
                    if matches!(rhs.as_ref(), E::Constant { .. }) {
                        compare(rhs, other_rhs, budget)?
                            || constant_shift_counts_correspond_v1(rhs, other_rhs, *scalar, budget)?
                    } else {
                        compare(rhs, other_rhs, budget)?
                            || masked_shift_counts_correspond_v1(
                                rhs, other_rhs, *scalar, next, budget,
                            )?
                    }
                } else {
                    compare(rhs, other_rhs, budget)?
                }
        }
        (
            E::Compare {
                operation,
                operand_scalar,
                lhs,
                rhs,
            },
            E::Compare {
                operation: other_op,
                operand_scalar: other_scalar,
                lhs: other_lhs,
                rhs: other_rhs,
            },
        ) => {
            operation == other_op
                && operand_scalar == other_scalar
                && compare(lhs, other_lhs, budget)?
                && compare(rhs, other_rhs, budget)?
        }
        (
            E::Select {
                scalar,
                condition,
                when_true,
                when_false,
            },
            E::Select {
                scalar: other,
                condition: other_condition,
                when_true: other_true,
                when_false: other_false,
            },
        ) => {
            scalar == other
                && compare(condition, other_condition, budget)?
                && compare(when_true, other_true, budget)?
                && compare(when_false, other_false, budget)?
        }
        (
            E::Cast {
                kind,
                source,
                target,
                operand,
            },
            E::Cast {
                kind: other_kind,
                source: other_source,
                target: other_target,
                operand: other_operand,
            },
        ) => {
            kind == other_kind
                && source == other_source
                && target == other_target
                && compare(operand, other_operand, budget)?
        }
        _ => false,
    })
}
