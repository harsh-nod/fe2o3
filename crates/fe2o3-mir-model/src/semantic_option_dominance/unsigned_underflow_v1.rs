//! Exact typed unsigned `lhs < rhs` is the underflow flag for `lhs - rhs`.
//! This describes the retained comparison, not a synthetic checked operation.

use crate::semantic_mir_v1::{
    SemanticAssignmentV1, SemanticBinaryOpV1, SemanticConstantValueV1, SemanticFunctionDeclV1,
    SemanticOperandV1, SemanticPlaceV1, SemanticRvalueKindV1, SemanticScalarTypeV1,
    SemanticTypeDeclV1, SemanticTypeIdV1, SemanticTypeShapeV1, SemanticUncheckedBinaryOpV1,
};

pub(super) fn comparison_operands<'a>(
    types: &[SemanticTypeDeclV1],
    function: &SemanticFunctionDeclV1,
    assignment: &'a SemanticAssignmentV1,
) -> Option<(&'a SemanticOperandV1, &'a SemanticOperandV1)> {
    let value = assignment.value();
    let SemanticRvalueKindV1::Binary {
        operation: SemanticBinaryOpV1::LessThan,
        left,
        right,
    } = value.kind()
    else {
        return None;
    };
    if !matches!(
        types.get(value.result_type().index() as usize)?.shape(),
        SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Bool)
    ) || !local_binding(function, assignment.destination(), value.result_type())
        || left.ty() != right.ty()
    {
        return None;
    }
    let bits = unsigned_bits(types, left.ty())?;
    (operand_binding(function, left, bits)
        && operand_binding(function, right, bits)
        && ordered_operands(left, right))
    .then_some((left, right))
}

pub(super) fn matches_subtraction(
    types: &[SemanticTypeDeclV1],
    function: &SemanticFunctionDeclV1,
    assignment: &SemanticAssignmentV1,
    operand_type: SemanticTypeIdV1,
) -> bool {
    let value = assignment.value();
    let SemanticRvalueKindV1::UncheckedBinary(binary) = value.kind() else {
        return false;
    };
    let Some(bits) = unsigned_bits(types, operand_type) else {
        return false;
    };
    binary.operation() == SemanticUncheckedBinaryOpV1::Subtract
        && value.result_type() == operand_type
        && local_binding(function, assignment.destination(), operand_type)
        && binary.left().ty() == operand_type
        && binary.right().ty() == operand_type
        && operand_binding(function, binary.left(), bits)
        && operand_binding(function, binary.right(), bits)
        && ordered_operands(binary.left(), binary.right())
}

fn ordered_operands(left: &SemanticOperandV1, right: &SemanticOperandV1) -> bool {
    // Operand queries observe the start of the statement. A left-hand move
    // must not authorize a later read of that same local within the rvalue.
    !matches!((left, right), (SemanticOperandV1::Move(left),
        SemanticOperandV1::Copy(right) | SemanticOperandV1::Move(right))
        if left.local() == right.local())
}

fn unsigned_bits(types: &[SemanticTypeDeclV1], ty: SemanticTypeIdV1) -> Option<u16> {
    match types.get(ty.index() as usize)?.shape() {
        SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
            signed: false,
            bits: bits @ (8 | 16 | 32 | 64 | 128),
        }) => Some(*bits),
        _ => None,
    }
}

fn local_binding(
    function: &SemanticFunctionDeclV1,
    place: &SemanticPlaceV1,
    ty: SemanticTypeIdV1,
) -> bool {
    place.projections().is_empty()
        && place.ty() == ty
        && function
            .locals()
            .get(place.local().index() as usize)
            .is_some_and(|local| local.ty() == ty)
}

fn operand_binding(
    function: &SemanticFunctionDeclV1,
    operand: &SemanticOperandV1,
    bits: u16,
) -> bool {
    match operand {
        SemanticOperandV1::Copy(place) | SemanticOperandV1::Move(place) => {
            local_binding(function, place, operand.ty())
        }
        SemanticOperandV1::Constant(constant) => matches!(
            constant.value(), SemanticConstantValueV1::Scalar(value)
                if u16::from(value.size_bytes()) * 8 == bits
        ),
    }
}
