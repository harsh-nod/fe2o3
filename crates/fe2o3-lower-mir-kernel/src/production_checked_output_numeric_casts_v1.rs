use super::*;
use fe2o3_kernel_ir::{CastKind, ScalarType};

fn integer(ty: &Type) -> bool {
    matches!(
        ty,
        Type::Scalar(
            ScalarType::I8
                | ScalarType::I16
                | ScalarType::I32
                | ScalarType::I64
                | ScalarType::U8
                | ScalarType::U16
                | ScalarType::U32
                | ScalarType::U64
        )
    )
}

// The verified inventory supplies the exact operand definition, not a caller
// type annotation. This is a total-op grammar test, never a value/range fact.
pub(super) fn native(
    inventory: &CanonicalKirInventoryV1<'_>,
    ordinal: usize,
    budget: &mut AssertOriginBudgetV1<'_>,
) -> R<bool> {
    charge(budget, 3)?;
    let row = &inventory.operations()[ordinal];
    let OperationKind::Cast { kind, to, .. } = &row.operation.kind else {
        return Ok(false);
    };
    let input = inventory.definitions()[inventory.uses()[row.operands.start].definition].ty;
    Ok(match kind {
        CastKind::IntegerToFloat => integer(input) && *to == Type::F32,
        CastKind::FloatToInteger => *input == Type::F32 && integer(to),
        _ => false,
    })
}

pub(super) fn source_integer_to_f32(
    source: &AdmittedInertSemanticMirV1,
    from: SemanticTypeIdV1,
    to: SemanticTypeIdV1,
    budget: &mut AssertOriginBudgetV1<'_>,
) -> R<bool> {
    charge(budget, 3)?;
    Ok(matches!(
        source.types()[from.index() as usize].shape(),
        SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
            bits: 8 | 16 | 32 | 64,
            ..
        })
    ) && matches!(
        source.types()[to.index() as usize].shape(),
        SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Float { bits: 32 })
    ))
}

#[cfg(test)]
#[path = "production_checked_output_numeric_casts_v1_tests.rs"]
mod tests;
