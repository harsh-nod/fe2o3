use super::*;
use fe2o3_mir_model::semantic_mir_v1::SemanticConstantValueV1;

fn integer(source: &AdmittedInertSemanticMirV1, ty: SemanticTypeIdV1) -> Option<(bool, u16)> {
    match source.types().get(ty.index() as usize)?.shape() {
        SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
            signed,
            bits: bits @ (8 | 16 | 32 | 64),
        }) => Some((*signed, *bits)),
        _ => None,
    }
}

fn inner_source(
    source: &AdmittedInertSemanticMirV1,
    function: usize,
    block: usize,
    statement: usize,
) -> Option<bool> {
    let function = source.functions().get(function)?;
    let block = function.blocks().get(block)?;
    let SemanticStatementKindV1::Assign(shift) = block.statements().get(statement)?.kind() else {
        return None;
    };
    let SemanticRvalueKindV1::Binary {
        operation: SemanticBinaryOpV1::ShiftLeft | SemanticBinaryOpV1::ShiftRight,
        left,
        right,
    } = shift.value().kind()
    else {
        return None;
    };
    let result = shift.value().result_type();
    let (_, width) = integer(source, result)?;
    if left.ty() != result || shift.destination().ty() != result {
        return None;
    }
    let (SemanticOperandV1::Copy(count) | SemanticOperandV1::Move(count)) = right else {
        return None;
    };
    if !count.projections().is_empty()
        || function.locals().get(count.local().index() as usize)?.ty() != count.ty()
    {
        return None;
    }
    let (signed, count_width) = integer(source, count.ty())?;
    let previous = statement.checked_sub(1)?;
    let SemanticStatementKindV1::Assign(mask) = block.statements().get(previous)?.kind() else {
        return None;
    };
    // Resolve the actual adjacent occurrence, never a same-numbered local or a
    // matching expression found elsewhere in the function. The admitted SSA
    // owner separately checks initialization, lifetime, and all source uses.
    if mask.destination() != count || mask.value().result_type() != count.ty() {
        return None;
    }
    let SemanticRvalueKindV1::Binary {
        operation: SemanticBinaryOpV1::BitAnd,
        left: input,
        right: SemanticOperandV1::Constant(limit),
    } = mask.value().kind()
    else {
        return None;
    };
    if input.ty() != count.ty() || limit.ty() != count.ty() {
        return None;
    }
    let SemanticConstantValueV1::Scalar(limit) = limit.value() else {
        return None;
    };
    Some(
        u16::from(limit.size_bytes()) * 8 == count_width
            && (!signed || limit.bits() & (1_u128 << (count_width - 1)) == 0)
            && limit.bits() == u128::from(width - 1),
    )
}

/// A source occurrence predicate, not a value-correspondence or proof receipt.
/// All coordinates are resolved through the exact admitted owner supplied by
/// the whole-source census. No search, allocation, recursion, or local cache.
pub(super) fn source(
    source: &AdmittedInertSemanticMirV1,
    function: usize,
    block: usize,
    statement: usize,
    budget: &mut AssertOriginBudgetV1<'_>,
) -> R<bool> {
    charge(budget, 64)?;
    Ok(inner_source(source, function, block, statement).unwrap_or(false))
}

fn inner_native(inventory: &CanonicalKirInventoryV1<'_>, ordinal: usize) -> Option<bool> {
    let row = inventory.operations().get(ordinal)?;
    if !matches!(
        row.operation.kind,
        OperationKind::Binary {
            op: BinaryOp::ShiftLeft | BinaryOp::ShiftRight,
            ..
        }
    ) || row.operands.len() != 2
        || row.results.len() != 1
    {
        return None;
    }
    let ty = inventory.definitions().get(row.results.start)?.ty;
    let width = u64::from(constant_shifts::width(ty)?);
    let lhs = inventory.uses().get(row.operands.start)?.definition;
    let rhs = inventory
        .uses()
        .get(row.operands.start.checked_add(1)?)?
        .definition;
    if inventory.definitions().get(lhs)?.ty != ty || inventory.definitions().get(rhs)?.ty != ty {
        return None;
    }
    let producer = constant_shifts::defining_operation(inventory, rhs)?;
    let mut mask = inventory.operations().get(producer)?;
    let mut count_ty = ty;
    let mut before = row.coordinate.operation;
    if let OperationKind::Cast { kind, to, .. } = &mask.operation.kind {
        if mask.coordinate.block != row.coordinate.block
            || mask.coordinate.operation >= before
            || mask.operands.len() != 1
            || to != ty
        {
            return None;
        }
        let input = inventory.uses().get(mask.operands.start)?.definition;
        count_ty = inventory.definitions().get(input)?.ty;
        constant_shifts::width(count_ty)?;
        if fe2o3_kernel_ir::plan_integer_cast_v1(count_ty.as_scalar()?, ty.as_scalar()?)
            != Some([Some((*kind, ty.as_scalar()?)), None])
        {
            return None;
        }
        before = mask.coordinate.operation;
        mask = inventory
            .operations()
            .get(constant_shifts::defining_operation(inventory, input)?)?;
    }
    if mask.coordinate.block != row.coordinate.block
        || mask.coordinate.operation >= before
        || !matches!(
            mask.operation.kind,
            OperationKind::Binary {
                op: BinaryOp::BitAnd,
                ..
            }
        )
        || mask.operands.len() != 2
    {
        return None;
    }
    let input = inventory.uses().get(mask.operands.start)?.definition;
    let limit = inventory
        .uses()
        .get(mask.operands.start.checked_add(1)?)?
        .definition;
    if inventory.definitions().get(input)?.ty != count_ty {
        return None;
    }
    Some(
        constant_shifts::definition_literal(
            inventory,
            limit,
            count_ty,
            row.coordinate.block.function,
        )? == width - 1,
    )
}

/// Checks only actual native operation definedness. In particular, the native
/// lowerer's unconditional mask cannot establish the independent source rule.
/// Complete source/N value correspondence still binds the dynamic expression.
pub(super) fn native(
    inventory: &CanonicalKirInventoryV1<'_>,
    ordinal: usize,
    budget: &mut AssertOriginBudgetV1<'_>,
) -> R<bool> {
    charge(budget, 96)?;
    Ok(inner_native(inventory, ordinal).unwrap_or(false))
}

#[cfg(test)]
#[path = "production_checked_output_masked_shifts_v1_tests.rs"]
mod tests;
