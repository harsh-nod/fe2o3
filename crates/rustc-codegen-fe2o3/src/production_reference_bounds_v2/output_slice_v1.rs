//! Slice bounds from exact length substitution and preceding path conditions,
//! never from the assertion being discharged or an inferred output domain.

use super::*;
use crate::reference_effect_v1::{ReferenceGuardAtomV1, ReferenceGuardClauseV1};

mod guarded_index;

pub(super) fn equal_length_bound(
    clause: &ReferenceGuardClauseV1,
    check: &ResolvedReferenceBoundsCheckV1,
    work: &mut ReferenceSymbolicWorkBudgetV2,
) -> Result<bool, ReferenceBoundsDischargeErrorV2> {
    let mut exact_length = None;
    for atom in &clause.atoms {
        work.charge_v2(1)
            .map_err(|_| bounds_work_error_v2(check.block))?;
        let ReferenceGuardAtomV1::SwitchValueSet {
            discriminant:
                equality @ ReferenceEffectExpressionV1::Binary {
                    operation: ReferenceBinaryOpV1::Equal,
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
        work.charge_expression_v2(equality)
            .map_err(|_| bounds_work_error_v2(check.block))?;
        let constant = if **lhs == check.length {
            rhs.as_ref()
        } else if **rhs == check.length {
            lhs.as_ref()
        } else {
            continue;
        };
        if !matches!(constant, ReferenceEffectExpressionV1::Constant(ReferenceConstantV1::Scalar {
            scalar: ReferenceScalarTypeV1::Usize, bits,
        }) if *bits <= u64::MAX as u128)
        {
            continue;
        }
        if exact_length.is_some_and(|previous| previous != constant) {
            return Ok(false);
        }
        exact_length = Some(constant);
    }
    let Some(constant) = exact_length else {
        return Ok(false);
    };
    work.charge_expression_v2(&check.index)
        .and_then(|_| work.charge_v2(clause.atoms.len()))
        .map_err(|_| bounds_work_error_v2(check.block))?;
    let bound = ReferenceEffectExpressionV1::Binary {
        operation: ReferenceBinaryOpV1::LessThan,
        lhs: Box::new(check.index.clone()),
        rhs: Box::new(constant.clone()),
        checked: false,
    };
    if clause
        .atoms
        .contains(&reference_boolean_guard_atom_v1(bound, true))
    {
        return Ok(true);
    }
    let length = guarded_index::constant(constant).expect("exact usize length checked above");
    Ok(guarded_index::upper_bound(&check.index, clause, work, check.block, 0)?
        .is_some_and(|maximum| maximum < length))
}

#[cfg(test)]
#[path = "output_slice_v1/tests.rs"]
mod tests;
