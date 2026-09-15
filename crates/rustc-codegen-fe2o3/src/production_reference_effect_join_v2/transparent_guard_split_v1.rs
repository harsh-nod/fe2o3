//! Unknown control is irrelevant only across exact empty branch chains.
use super::*;

pub(super) fn reconverges(
    kernel: &ProductionRankedKernelV1,
    first: usize,
    second: usize,
    work: &mut ReferenceSymbolicWorkBudgetV2,
) -> Result<bool, ()> {
    let blocks = kernel.blocks();
    if blocks.len() > MAX_REFERENCE_BLOCKS_V1 {
        return Err(());
    }
    work.charge_v2(blocks.len()).map_err(|_| ())?;
    let mut seen = vec![false; blocks.len()];
    let next = |index: usize| -> Result<Option<usize>, ()> {
        let block = blocks.get(index).ok_or(())?;
        if !block.operations().is_empty() || block.index_argument_count() != 0 {
            return Ok(None);
        }
        Ok(match block.terminator() {
            ProductionRankedTerminatorV1::Branch { target } => Some(*target as usize),
            _ => None,
        })
    };
    let mut current = first;
    loop {
        work.charge_v2(1).map_err(|_| ())?;
        let slot = seen.get_mut(current).ok_or(())?;
        if *slot {
            return Ok(false);
        }
        *slot = true;
        match next(current)? {
            Some(target) => current = target,
            None => break,
        }
    }
    current = second;
    // The existing whole-CFG topological audit also rejects cycles. This
    // explicit bound keeps the helper closed when tested independently.
    for _ in 0..=blocks.len() {
        work.charge_v2(1).map_err(|_| ())?;
        if *seen.get(current).ok_or(())? {
            return Ok(blocks[current].index_argument_count() == 0);
        }
        match next(current)? {
            Some(target) => current = target,
            None => return Ok(false),
        }
    }
    Ok(false)
}

#[cfg(test)]
#[path = "transparent_guard_split_v1/tests.rs"]
mod tests;

pub(super) fn preceding_bound(
    source: &ReferencePathPredicateV1,
    condition: &ReferenceEffectExpressionV1,
    block: usize,
    work: &mut ReferenceSymbolicWorkBudgetV2,
) -> Result<bool, ()> {
    let ReferenceEffectExpressionV1::Binary {
        operation: ReferenceBinaryOpV1::LessThan,
        lhs,
        rhs,
        checked: false,
    } = condition
    else {
        return Ok(false);
    };
    if !matches!(**rhs, ReferenceEffectExpressionV1::InputLength { .. }) {
        return Ok(false);
    }
    work.charge_expression_v2(condition).map_err(|_| ())?;
    let check = crate::reference_effect_v1::ResolvedReferenceBoundsCheckV1 {
        block: u32::try_from(block).map_err(|_| ())?,
        expected: true,
        condition: condition.clone(),
        index: (**lhs).clone(),
        length: (**rhs).clone(),
    };
    // The algebra consumes only the already derived GPU entry predicate. No
    // CPU assertion, output extent, or reference precondition becomes a fact.
    crate::production_reference_bounds_v2::cpu_path_proves_bound_v2(source, &check, work)
        .map_err(|_| ())
}

pub(super) fn canonical_point_threshold(
    mut condition: ReferenceEffectExpressionV1,
    yes: u32,
    no: u32,
    work: &mut ReferenceSymbolicWorkBudgetV2,
) -> Result<(ReferenceEffectExpressionV1, u32, u32), ()> {
    work.charge_v2(4).map_err(|_| ())?;
    let ReferenceEffectExpressionV1::Binary {
        operation: ReferenceBinaryOpV1::LessThan,
        lhs,
        rhs,
        checked: false,
    } = &mut condition
    else {
        return Ok((condition, yes, no));
    };
    let ReferenceEffectExpressionV1::Constant(ReferenceConstantV1::Scalar {
        scalar: ReferenceScalarTypeV1::Usize,
        bits,
    }) = lhs.as_ref()
    else {
        return Ok((condition, yes, no));
    };
    if !matches!(
        rhs.as_ref(),
        ReferenceEffectExpressionV1::PointCoordinate { .. }
    ) {
        return Ok((condition, yes, no));
    }
    let Some(successor) = u64::try_from(*bits)
        .ok()
        .and_then(|value| value.checked_add(1))
    else {
        return Ok((condition, yes, no));
    };
    // Ranked point indices are unsigned 64-bit values. C < point is exactly
    // the complement of point < C+1 only when that successor is representable.
    std::mem::swap(lhs, rhs);
    **rhs = ReferenceEffectExpressionV1::Constant(ReferenceConstantV1::Scalar {
        scalar: ReferenceScalarTypeV1::Usize,
        bits: u128::from(successor),
    });
    Ok((condition, no, yes))
}
