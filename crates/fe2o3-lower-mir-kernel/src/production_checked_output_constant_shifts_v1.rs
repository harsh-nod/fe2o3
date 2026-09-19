use super::*;
use fe2o3_kernel_ir::{CanonicalKirDefinitionCoordinateV1 as Definition, Constant, ScalarType};
use fe2o3_mir_model::semantic_mir_v1::{SemanticConstantValueV1, SemanticRvalueV1};

pub(super) fn width(ty: &Type) -> Option<u16> {
    match ty {
        Type::Scalar(ScalarType::I8 | ScalarType::U8) => Some(8),
        Type::Scalar(ScalarType::I16 | ScalarType::U16) => Some(16),
        Type::Scalar(ScalarType::I32 | ScalarType::U32) => Some(32),
        Type::Scalar(ScalarType::I64 | ScalarType::U64) => Some(64),
        _ => None,
    }
}

fn literal(constant: &Constant, ty: &Type) -> Option<u64> {
    Some(match (constant, ty) {
        (Constant::I8(n), Type::Scalar(ScalarType::I8)) => u64::try_from(*n).ok()?,
        (Constant::U8(n), Type::Scalar(ScalarType::U8)) => u64::from(*n),
        (Constant::I16(n), Type::Scalar(ScalarType::I16)) => u64::try_from(*n).ok()?,
        (Constant::U16(n), Type::Scalar(ScalarType::U16)) => u64::from(*n),
        (Constant::I32(n), Type::Scalar(ScalarType::I32)) => u64::try_from(*n).ok()?,
        (Constant::U32(n), Type::Scalar(ScalarType::U32)) => u64::from(*n),
        (Constant::I64(n), Type::Scalar(ScalarType::I64)) => u64::try_from(*n).ok()?,
        (Constant::U64(n), Type::Scalar(ScalarType::U64)) => *n,
        _ => return None,
    })
}

// Resolve an already-verified dense definition without a graph scan or a raw
// ValueId lookup. All its coordinates still have to describe this exact row.
pub(super) fn defining_operation(
    inventory: &CanonicalKirInventoryV1<'_>,
    definition: usize,
) -> Option<usize> {
    let row = inventory.definitions().get(definition)?;
    let Definition::Result {
        operation,
        result: 0,
    } = row.coordinate
    else {
        return None;
    };
    let function = inventory
        .functions()
        .get(operation.block.function.0 as usize)?;
    let block = function
        .blocks
        .start
        .checked_add(operation.block.block as usize)?;
    if !function.blocks.contains(&block) {
        return None;
    }
    let block = inventory.blocks().get(block)?;
    if block.coordinate != operation.block {
        return None;
    }
    let ordinal = block
        .operations
        .start
        .checked_add(operation.operation as usize)?;
    if !block.operations.contains(&ordinal) {
        return None;
    }
    let producer = inventory.operations().get(ordinal)?;
    (producer.coordinate == operation
        && producer.results.len() == 1
        && producer.results.start == definition)
        .then_some(ordinal)
}

pub(super) fn definition_literal(
    inventory: &CanonicalKirInventoryV1<'_>,
    definition: usize,
    ty: &Type,
    function: fe2o3_kernel_ir::CanonicalKirFunctionCoordinateV1,
) -> Option<u64> {
    if inventory.definitions().get(definition)?.ty != ty {
        return None;
    }
    let producer = &inventory.operations()[defining_operation(inventory, definition)?];
    if producer.coordinate.block.function != function {
        return None;
    }
    let OperationKind::Constant(value) = &producer.operation.kind else {
        return None;
    };
    literal(value, ty)
}

fn native_inner(inventory: &CanonicalKirInventoryV1<'_>, ordinal: usize) -> Option<bool> {
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
    let width = u64::from(width(ty)?);
    let lhs = inventory.uses().get(row.operands.start)?.definition;
    let rhs = inventory
        .uses()
        .get(row.operands.start.checked_add(1)?)?
        .definition;
    if inventory.definitions().get(lhs)?.ty != ty || inventory.definitions().get(rhs)?.ty != ty {
        return None;
    }
    let function = row.coordinate.block.function;
    if let Some(count) = definition_literal(inventory, rhs, ty, function) {
        return Some(count < width);
    }
    let mask = &inventory.operations()[defining_operation(inventory, rhs)?];
    if mask.coordinate.block.function != function
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
    let count = inventory.uses().get(mask.operands.start)?.definition;
    let limit = inventory
        .uses()
        .get(mask.operands.start.checked_add(1)?)?
        .definition;
    // Do not use the mask to justify an out-of-range or dynamic source count.
    Some(
        definition_literal(inventory, count, ty, function)? < width
            && definition_literal(inventory, limit, ty, function)? == width - 1,
    )
}

/// Closed operation-definedness census, never a source or value correspondence
/// certificate. The caller independently applies this to every actual endpoint.
pub(super) fn native(
    inventory: &CanonicalKirInventoryV1<'_>,
    ordinal: usize,
    budget: &mut AssertOriginBudgetV1<'_>,
) -> R<bool> {
    // At most four coordinate walks (five indexed reads each), eight other
    // indexed reads, and fixed-size scalar checks. No allocation or recursion.
    charge(budget, 64)?;
    Ok(native_inner(inventory, ordinal).unwrap_or(false))
}

pub(super) fn source(
    source: &AdmittedInertSemanticMirV1,
    value: &SemanticRvalueV1,
    budget: &mut AssertOriginBudgetV1<'_>,
) -> R<bool> {
    charge(budget, 16)?;
    let SemanticRvalueKindV1::Binary {
        operation: SemanticBinaryOpV1::ShiftLeft | SemanticBinaryOpV1::ShiftRight,
        left,
        right: SemanticOperandV1::Constant(right),
    } = value.kind()
    else {
        return Ok(false);
    };
    if left.ty() != value.result_type() {
        return Ok(false);
    }
    let Some(SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
        bits: width @ (8 | 16 | 32 | 64),
        ..
    })) = source
        .types()
        .get(left.ty().index() as usize)
        .map(|ty| ty.shape())
    else {
        return Ok(false);
    };
    let Some(SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
        signed,
        bits: rhs_width @ (8 | 16 | 32 | 64),
    })) = source
        .types()
        .get(right.ty().index() as usize)
        .map(|ty| ty.shape())
    else {
        return Ok(false);
    };
    let SemanticConstantValueV1::Scalar(count) = right.value() else {
        return Ok(false);
    };
    Ok(u16::from(count.size_bytes()) * 8 == *rhs_width
        && (!*signed || count.bits() & (1_u128 << (*rhs_width - 1)) == 0)
        && count.bits() < u128::from(*width))
}

#[cfg(test)]
#[path = "production_checked_output_constant_shifts_v1_tests.rs"]
pub(crate) mod tests;
