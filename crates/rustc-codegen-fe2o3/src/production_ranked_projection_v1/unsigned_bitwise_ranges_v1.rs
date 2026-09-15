//! Typed scalar interval transfers; these do not alter or discharge source MIR.

use super::*;

impl SemanticAssertProofsV1<'_> {
    pub(super) fn unsigned_bitwise_maximum_v1(&self, value: &SemanticRvalueV1) -> Option<u128> {
        let ty = value.result_type();
        let declaration = self.types.get(ty.index() as usize)?;
        let SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
            signed: false,
            bits,
        }) = declaration.shape()
        else {
            return None;
        };
        if !matches!(*bits, 8 | 16 | 32 | 64 | 128)
            || declaration.layout().size_bytes() != Some(u64::from(*bits / 8))
        {
            return None;
        }
        let maximum = self.scalar_unsigned_maximum(ty)?;
        let exact_operand = |operand: &SemanticOperandV1| {
            if operand.ty() != ty {
                return false;
            }
            match operand {
                SemanticOperandV1::Constant(constant) => matches!(
                    constant.value(), SemanticConstantValueV1::Scalar(scalar)
                    if u16::from(scalar.size_bytes()) * 8 == *bits && scalar.bits() <= maximum
                ),
                SemanticOperandV1::Copy(place) | SemanticOperandV1::Move(place) => {
                    place.projections().is_empty()
                        && self
                            .function
                            .locals()
                            .get(place.local().index() as usize)
                            .is_some_and(|local| local.ty() == ty)
                }
            }
        };
        match value.kind() {
            SemanticRvalueKindV1::Unary {
                operation: SemanticUnaryOpV1::Not,
                operand,
            } if exact_operand(operand) => Some(maximum),
            SemanticRvalueKindV1::Binary {
                operation: SemanticBinaryOpV1::BitAnd,
                left,
                right,
            } if exact_operand(left) && exact_operand(right) => Some(maximum),
            _ => None,
        }
    }
}

pub(super) fn unsigned_not_range_v1(
    input: UnsignedRangeProofV1,
    maximum: u128,
) -> Option<UnsignedRangeProofV1> {
    if input.minimum > input.maximum || input.maximum > maximum {
        return None;
    }
    // Complement reverses the order within the same unsigned bit width.
    Some(UnsignedRangeProofV1 {
        minimum: maximum - input.maximum,
        maximum: maximum - input.minimum,
    })
}

pub(super) fn unsigned_and_range_v1(
    left: UnsignedRangeProofV1,
    right: UnsignedRangeProofV1,
    maximum: u128,
) -> Option<UnsignedRangeProofV1> {
    if left.minimum > left.maximum
        || right.minimum > right.maximum
        || left.maximum > maximum
        || right.maximum > maximum
    {
        return None;
    }
    if left.minimum == left.maximum && right.minimum == right.maximum {
        return Some(UnsignedRangeProofV1::exact(left.minimum & right.minimum));
    }
    // For unsigned operands, x & y is no greater than either operand.
    Some(UnsignedRangeProofV1 {
        minimum: 0,
        maximum: left.maximum.min(right.maximum),
    })
}

#[cfg(test)]
#[path = "unsigned_bitwise_ranges_v1/tests.rs"]
mod tests;
