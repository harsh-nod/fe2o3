//! Unsigned index bounds from preceding path conditions. No output extent,
//! assertion under proof, wrapping overflow, or untyped scalar is assumed.

use super::*;

pub(super) fn upper_bound(
    expression: &ReferenceEffectExpressionV1,
    clause: &ReferenceGuardClauseV1,
    work: &mut ReferenceSymbolicWorkBudgetV2,
    block: u32,
    depth: usize,
) -> Result<Option<u64>, ReferenceBoundsDischargeErrorV2> {
    work.charge_v2(1).map_err(|_| bounds_work_error_v2(block))?;
    if depth >= MAX_BOUND_DEPTH_V2 {
        return Err(bounds_work_error_v2(block));
    }
    let maximum = match expression {
        ReferenceEffectExpressionV1::PointCoordinate { .. } => Some(u64::MAX),
        ReferenceEffectExpressionV1::Constant(_) => constant(expression),
        ReferenceEffectExpressionV1::Binary {
            operation,
            lhs,
            rhs,
            checked: false,
        } => {
            let Some(left) = upper_bound(lhs, clause, work, block, depth + 1)? else {
                return Ok(None);
            };
            let Some(right) = upper_bound(rhs, clause, work, block, depth + 1)? else {
                return Ok(None);
            };
            match operation {
                ReferenceBinaryOpV1::Add => left.checked_add(right),
                ReferenceBinaryOpV1::Multiply => left.checked_mul(right),
                ReferenceBinaryOpV1::Divide => constant(rhs)
                    .filter(|divisor| *divisor != 0)
                    .map(|divisor| left / divisor),
                ReferenceBinaryOpV1::Remainder => constant(rhs)
                    .filter(|divisor| *divisor != 0)
                    .map(|divisor| left.min(divisor - 1)),
                _ => None,
            }
        }
        _ => None,
    };
    let Some(mut maximum) = maximum else {
        return Ok(None);
    };
    for atom in &clause.atoms {
        work.charge_v2(1).map_err(|_| bounds_work_error_v2(block))?;
        let ReferenceGuardAtomV1::SwitchValueSet {
            discriminant:
                ReferenceEffectExpressionV1::Binary {
                    operation: ReferenceBinaryOpV1::LessThan,
                    lhs,
                    rhs,
                    checked: false,
                },
            values,
            inside_set: false,
        } = atom
        else {
            continue;
        };
        if values.as_ref() != [0] {
            continue;
        }
        work.charge_expression_v2(lhs)
            .and_then(|_| work.charge_expression_v2(expression))
            .map_err(|_| bounds_work_error_v2(block))?;
        if lhs.as_ref() != expression {
            continue;
        }
        if let Some(bound) = constant(rhs).and_then(|value| value.checked_sub(1)) {
            maximum = maximum.min(bound);
        }
    }
    Ok(Some(maximum))
}

pub(super) fn constant(expression: &ReferenceEffectExpressionV1) -> Option<u64> {
    match expression {
        ReferenceEffectExpressionV1::Constant(ReferenceConstantV1::Scalar {
            scalar: ReferenceScalarTypeV1::Usize,
            bits,
        }) => u64::try_from(*bits).ok(),
        _ => None,
    }
}

#[cfg(test)]
#[path = "guarded_index_tests.rs"]
mod tests;
