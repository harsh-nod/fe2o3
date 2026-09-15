//! Guard-only canonical values. Original checked operations/assertions remain
//! in the source IR and in the independent bounds and raw-assertion audits.
use super::*;

pub(super) fn resolve(
    resolver: &ReferenceExpressionResolverV1<'_>,
    operand: &ReferenceOperandV1,
    work: &mut ReferenceSymbolicWorkBudgetV2,
) -> Result<ReferenceEffectExpressionV1, ReferenceBindingErrorV1> {
    let mut expression = resolve_predicate_operand_v1(resolver, operand, work)?;
    fold(&mut expression, work, 0)?;
    Ok(expression)
}

fn fold(
    expression: &mut ReferenceEffectExpressionV1,
    work: &mut ReferenceSymbolicWorkBudgetV2,
    depth: usize,
) -> Result<(), ReferenceBindingErrorV1> {
    ReferenceExpressionResolverV1::require_depth_v1(depth)?;
    work.charge_v2(1)?;
    let ReferenceEffectExpressionV1::Binary {
        operation,
        lhs,
        rhs,
        ..
    } = expression
    else {
        // Do not normalize inside casts, loads or other semantic leaves.
        return Ok(());
    };
    fold(lhs, work, depth + 1)?;
    fold(rhs, work, depth + 1)?;
    if !matches!(
        operation,
        ReferenceBinaryOpV1::Add | ReferenceBinaryOpV1::Subtract | ReferenceBinaryOpV1::Multiply
    ) || !matches!(lhs.as_ref(), ReferenceEffectExpressionV1::Constant(_))
        || !matches!(rhs.as_ref(), ReferenceEffectExpressionV1::Constant(_))
    {
        return Ok(());
    }
    // Both existing evaluators receive literal leaves, never recursively folded
    // unknown expressions. Prepay evaluation and one replacement node.
    work.charge_v2(7)?;
    if reference_checked_overflow_v2(*operation, lhs, rhs) == Some(false)
        && let Some(constant) = reference_fold_constant_v2(expression)
    {
        *expression = constant;
    }
    Ok(())
}

#[cfg(test)]
mod tests;
