use super::*;
use crate::reference_effect_v1::CompactRowsUsize1D;

pub(super) fn require_exclusive(
    ir: &ReferenceEffectIrV1,
    argument: u32,
) -> Result<(), ProductionReferenceEffectJoinErrorV2> {
    if !ir.relations.iter().any(|relation| {
        matches!(relation,
            ReferenceArgumentRelationV1::ExclusivePrimitiveOutputSlice1D { argument: actual, .. }
                if *actual == argument
        )
    }) {
        return Err(ProductionReferenceEffectJoinErrorV2::UnsupportedReference(
            "compact row requires an authenticated exclusive primitive output relation",
        ));
    }
    Ok(())
}

pub(super) fn gpu_coordinate(
    kernel: &ProductionRankedKernelV1,
    ir: &ReferenceEffectIrV1,
    argument: u32,
    write: &RankedGpuWriteV2,
    scope: &mut scalar_guard_v1::ScalarWriteGuardV1<'_>,
) -> Result<ReferenceOutputCoordinateV1, ProductionReferenceEffectJoinErrorV2> {
    if !scope.is_write(write) || !std::ptr::eq(scope.expressions.effect_ir, ir) {
        return Err(gpu_guard_error_v2(
            write,
            "compact row guard belongs to another write",
        ));
    }
    if write.allocation_origin.checked_sub(1) != Some(u64::from(argument)) {
        return Err(gpu_guard_error_v2(
            write,
            "compact row output argument disagrees with its source view",
        ));
    }
    if scope.exclusive_output && write.indices.len() == 1 {
        let expression = scope
            .expressions
            .index(write.indices[0], 0, &mut scope.work)?;
        if let Some(mapping) =
            CompactRowsUsize1D::from_expression(&expression, &scope.predicate, &mut scope.work)
                .map_err(|_| gpu_guard_error_v2(write, "compact row GPU work limit exceeded"))?
        {
            return Ok(ReferenceOutputCoordinateV1::CompactRowsUsize1D(mapping));
        }
        return Ok(ReferenceOutputCoordinateV1::LogicalPoint(
            vec![expression].into_boxed_slice(),
        ));
    }
    Ok(ReferenceOutputCoordinateV1::LogicalPoint(
        write
            .indices
            .iter()
            .copied()
            .map(|value| gpu_index_expression_v2(kernel, value, 0))
            .collect::<Result<Vec<_>, _>>()?
            .into_boxed_slice(),
    ))
}

pub(super) fn place_coordinate(
    operations: &mut [ProductionRankedOperationV1],
    identity: ProductionRankedValueIdV1,
    expected_symbol: u32,
    coordinate: &ReferenceOutputCoordinateV1,
    ir: &ReferenceEffectIrV1,
    work: &mut ReferenceSymbolicWorkBudgetV2,
) -> Result<(), ProductionReferenceEffectJoinErrorV2> {
    let ReferenceOutputCoordinateV1::CompactRowsUsize1D(mapping) = coordinate else {
        return replace_reserved_semantic_symbol_v2(operations, identity, expected_symbol);
    };
    if expected_symbol != mapping.axis() {
        return Err(ProductionReferenceEffectJoinErrorV2::InvalidReservedValue(
            identity.get(),
        ));
    }
    let budget_error = |_| {
        ProductionReferenceEffectJoinErrorV2::UnsupportedReference(
            "compact row coordinate materialization exceeds the shared bounds work budget",
        )
    };
    let expression = mapping.expression(work).map_err(budget_error)?;
    work.charge_expression_v2(&expression)
        .map_err(budget_error)?;
    let expression =
        reference_expression_inner_checked_v2(ir, &expression, ReferenceScalarTypeV1::Usize, None)?;
    // Verify the original reserved symbol identity and uniqueness before any
    // mutation. Never replace an arbitrary existing semantic expression.
    work.charge_v2(operations.len()).map_err(budget_error)?;
    let mut found = None;
    for (index, operation) in operations.iter().enumerate() {
        if operation_result_v2(operation) != Some(identity) {
            continue;
        }
        if !matches!(operation, ProductionRankedOperationV1::SemanticSymbol { symbol, .. }
            if *symbol == expected_symbol)
            || found.replace(index).is_some()
        {
            return Err(ProductionReferenceEffectJoinErrorV2::InvalidReservedValue(
                identity.get(),
            ));
        }
    }
    let index = found.ok_or(ProductionReferenceEffectJoinErrorV2::InvalidReservedValue(
        identity.get(),
    ))?;
    operations[index] = ProductionRankedOperationV1::SemanticExpression {
        result: identity,
        expression,
        numerical_contract: ProductionNumericalContractV2::ExactBitVectorOperatorCongruence,
    };
    Ok(())
}

/// Exact complement for the two unsigned quotient/remainder leaves in the
/// closed map. These ASTs come from the same ranked index inventory as guards.
pub(super) fn canonical_row_threshold(
    mut condition: ReferenceEffectExpressionV1,
    yes: u32,
    no: u32,
    work: &mut ReferenceSymbolicWorkBudgetV2,
) -> Result<(ReferenceEffectExpressionV1, u32, u32), ()> {
    work.charge_v2(6).map_err(|_| ())?;
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
    let ReferenceEffectExpressionV1::Binary {
        operation: ReferenceBinaryOpV1::Divide | ReferenceBinaryOpV1::Remainder,
        lhs: point,
        rhs: divisor,
        checked: false,
    } = rhs.as_ref()
    else {
        return Ok((condition, yes, no));
    };
    if !matches!(
        point.as_ref(),
        ReferenceEffectExpressionV1::PointCoordinate { axis: 0 }
    ) || !matches!(divisor.as_ref(), ReferenceEffectExpressionV1::Constant(ReferenceConstantV1::Scalar {
            scalar: ReferenceScalarTypeV1::Usize, bits,
        }) if *bits > 0 && *bits <= u64::MAX as u128)
    {
        return Ok((condition, yes, no));
    }
    let Some(next) = u64::try_from(*bits).ok().and_then(|x| x.checked_add(1)) else {
        return Ok((condition, yes, no));
    };
    std::mem::swap(lhs, rhs);
    **rhs = ReferenceEffectExpressionV1::Constant(ReferenceConstantV1::Scalar {
        scalar: ReferenceScalarTypeV1::Usize,
        bits: next as u128,
    });
    Ok((condition, no, yes))
}
