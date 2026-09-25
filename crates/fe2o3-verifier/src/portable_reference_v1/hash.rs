//! Existing reference hash tags and domains, unchanged by portability.

use super::*;

pub(super) fn put_len(digest: &mut Sha256, length: usize) {
    digest.update(u64::try_from(length).unwrap_or(u64::MAX).to_le_bytes());
}

pub(super) fn scalar_tag(scalar: ReferenceScalarTypeV1) -> u8 {
    match scalar {
        ReferenceScalarTypeV1::Bool => 0,
        ReferenceScalarTypeV1::U8 => 1,
        ReferenceScalarTypeV1::U16 => 2,
        ReferenceScalarTypeV1::U32 => 3,
        ReferenceScalarTypeV1::U64 => 4,
        ReferenceScalarTypeV1::Usize => 5,
        ReferenceScalarTypeV1::I8 => 6,
        ReferenceScalarTypeV1::I16 => 7,
        ReferenceScalarTypeV1::I32 => 8,
        ReferenceScalarTypeV1::I64 => 9,
        ReferenceScalarTypeV1::Isize => 10,
        ReferenceScalarTypeV1::F32 => 11,
        ReferenceScalarTypeV1::F64 => 12,
    }
}

pub(super) fn digest_place(digest: &mut Sha256, place: &ReferencePlaceV1) {
    digest.update(place.local.to_le_bytes());
    put_len(digest, place.projection.len());
    for projection in &place.projection {
        match projection {
            ReferencePlaceProjectionV1::Dereference => digest.update([0]),
            ReferencePlaceProjectionV1::Field(field) => {
                digest.update([1]);
                digest.update(field.to_le_bytes());
            }
            ReferencePlaceProjectionV1::Index(local) => {
                digest.update([2]);
                digest.update(local.to_le_bytes());
            }
            ReferencePlaceProjectionV1::ConstantIndex {
                offset,
                minimum_length,
                from_end,
            } => {
                digest.update([3, u8::from(*from_end)]);
                digest.update(offset.to_le_bytes());
                digest.update(minimum_length.to_le_bytes());
            }
        }
    }
}

pub(super) fn digest_operand(digest: &mut Sha256, operand: &ReferenceOperandV1) {
    match operand {
        ReferenceOperandV1::Copy(place) => {
            digest.update([0]);
            digest_place(digest, place);
        }
        ReferenceOperandV1::Move(place) => {
            digest.update([1]);
            digest_place(digest, place);
        }
        ReferenceOperandV1::Constant(ReferenceConstantV1::ZeroSized) => {
            digest.update([2]);
        }
        ReferenceOperandV1::Constant(ReferenceConstantV1::Scalar { scalar, bits }) => {
            digest.update([3, scalar_tag(*scalar)]);
            digest.update(bits.to_le_bytes());
        }
    }
}

pub(super) fn digest_value(digest: &mut Sha256, value: &ReferenceValueV1) {
    match value {
        ReferenceValueV1::Use(operand) => {
            digest.update([0]);
            digest_operand(digest, operand);
        }
        ReferenceValueV1::Binary {
            operation,
            lhs,
            rhs,
            checked,
        } => {
            digest.update([1, u8::from(*checked), binary_tag(*operation)]);
            digest_operand(digest, lhs);
            digest_operand(digest, rhs);
        }
        ReferenceValueV1::Unary { operation, operand } => {
            digest.update([
                2,
                match operation {
                    ReferenceUnaryOpV1::Not => 0,
                    ReferenceUnaryOpV1::Negate => 1,
                },
            ]);
            digest_operand(digest, operand);
        }
        ReferenceValueV1::Cast {
            kind,
            source,
            target,
            operand,
        } => {
            digest.update([3, *kind as u8, scalar_tag(*source), scalar_tag(*target)]);
            digest_operand(digest, operand);
        }
        ReferenceValueV1::SafeHelperCall {
            helper,
            parameters,
            result,
            arguments,
            summary,
        } => {
            digest.update([4]);
            digest_function_identity_v2(digest, helper);
            put_len(digest, parameters.len());
            for parameter in parameters {
                digest.update([scalar_tag(*parameter)]);
            }
            digest.update([scalar_tag(*result)]);
            put_len(digest, arguments.len());
            for argument in arguments {
                digest_operand(digest, argument);
            }
            digest_effect_expression_v1(digest, summary);
        }
        ReferenceValueV1::InputLength { reference_argument } => {
            digest.update([5]);
            digest.update(reference_argument.to_le_bytes());
        }
    }
}

pub(super) fn digest_function_identity_v2(
    digest: &mut Sha256,
    identity: &ReferenceFunctionIdentityV1,
) {
    digest.update(identity.def_path_hash);
    digest.update(identity.function_sha256);
    digest.update(identity.item_definition_sha256);
    digest.update(identity.monomorphization_sha256);
    digest.update(identity.generic_type_arguments_sha256);
    digest.update(identity.const_generic_arguments_sha256);
    digest.update(identity.rustc_mir_body_sha256);
}

pub fn digest_effect_expression_v1(digest: &mut Sha256, expression: &ReferenceEffectExpressionV1) {
    match expression {
        ReferenceEffectExpressionV1::PointCoordinate { axis } => {
            digest.update([0]);
            digest.update(axis.to_le_bytes());
        }
        ReferenceEffectExpressionV1::KernelScalarArgument { argument } => {
            digest.update([1]);
            digest.update(argument.to_le_bytes());
        }
        ReferenceEffectExpressionV1::Constant(constant) => {
            digest.update([2]);
            digest_operand(digest, &ReferenceOperandV1::Constant(constant.clone()));
        }
        ReferenceEffectExpressionV1::Binary {
            operation,
            lhs,
            rhs,
            checked,
        } => {
            digest.update([3, binary_tag(*operation), u8::from(*checked)]);
            digest_effect_expression_v1(digest, lhs);
            digest_effect_expression_v1(digest, rhs);
        }
        ReferenceEffectExpressionV1::Unary { operation, operand } => {
            digest.update([
                4,
                match operation {
                    ReferenceUnaryOpV1::Not => 0,
                    ReferenceUnaryOpV1::Negate => 1,
                },
            ]);
            digest_effect_expression_v1(digest, operand);
        }
        ReferenceEffectExpressionV1::InputLoad {
            reference_argument,
            index,
        } => {
            digest.update([6]);
            digest.update(reference_argument.to_le_bytes());
            digest_effect_expression_v1(digest, index);
        }
        ReferenceEffectExpressionV1::InputLength { reference_argument } => {
            digest.update([7]);
            digest.update(reference_argument.to_le_bytes());
        }
        ReferenceEffectExpressionV1::Cast {
            kind,
            source,
            target,
            operand,
        } => {
            digest.update([5, *kind as u8, scalar_tag(*source), scalar_tag(*target)]);
            digest_effect_expression_v1(digest, operand);
        }
    }
}

pub(super) fn digest_path_predicate_v1(digest: &mut Sha256, predicate: &ReferencePathPredicateV1) {
    put_len(digest, predicate.clauses.len());
    for clause in &predicate.clauses {
        put_len(digest, clause.atoms.len());
        for atom in &clause.atoms {
            match atom {
                ReferenceGuardAtomV1::SwitchValueSet {
                    discriminant,
                    values,
                    inside_set,
                } => {
                    digest.update([0, u8::from(*inside_set)]);
                    digest_effect_expression_v1(digest, discriminant);
                    put_len(digest, values.len());
                    for value in values {
                        digest.update(value.to_le_bytes());
                    }
                }
                ReferenceGuardAtomV1::Assert {
                    condition,
                    expected,
                } => {
                    digest.update([1, u8::from(*expected)]);
                    digest_effect_expression_v1(digest, condition);
                }
            }
        }
    }
}

pub(super) fn digest_output_effect_v1(digest: &mut Sha256, effect: &ReferenceOutputWriteV1) {
    digest.update(effect.argument.to_le_bytes());
    digest.update(effect.block.to_le_bytes());
    digest.update(effect.statement.to_le_bytes());
    match &effect.coordinate {
        ReferenceOutputCoordinateV1::LogicalPoint(axes) => {
            digest.update([0]);
            put_len(digest, axes.len());
            for axis in axes {
                digest_effect_expression_v1(digest, axis);
            }
        }
        ReferenceOutputCoordinateV1::SingleCoordinate => digest.update([1]),
        ReferenceOutputCoordinateV1::Dynamic(expression) => {
            digest.update([2]);
            digest_effect_expression_v1(digest, expression);
        }
        ReferenceOutputCoordinateV1::Constant {
            offset,
            minimum_length,
            from_end,
        } => {
            digest.update([3, u8::from(*from_end)]);
            digest.update(offset.to_le_bytes());
            digest.update(minimum_length.to_le_bytes());
        }
    }
    digest_path_predicate_v1(digest, &effect.guard);
    digest_effect_expression_v1(digest, &effect.rhs);
}

pub(super) fn binary_tag(operation: ReferenceBinaryOpV1) -> u8 {
    match operation {
        ReferenceBinaryOpV1::Add => 0,
        ReferenceBinaryOpV1::Subtract => 1,
        ReferenceBinaryOpV1::Multiply => 2,
        ReferenceBinaryOpV1::Divide => 3,
        ReferenceBinaryOpV1::Remainder => 4,
        ReferenceBinaryOpV1::BitXor => 5,
        ReferenceBinaryOpV1::BitAnd => 6,
        ReferenceBinaryOpV1::BitOr => 7,
        ReferenceBinaryOpV1::ShiftLeft => 8,
        ReferenceBinaryOpV1::ShiftRight => 9,
        ReferenceBinaryOpV1::Equal => 10,
        ReferenceBinaryOpV1::LessThan => 11,
        ReferenceBinaryOpV1::LessEqual => 12,
        ReferenceBinaryOpV1::NotEqual => 13,
        ReferenceBinaryOpV1::GreaterEqual => 14,
        ReferenceBinaryOpV1::GreaterThan => 15,
    }
}

pub(super) fn digest_terminator(digest: &mut Sha256, terminator: &ReferenceTerminatorV1) {
    match terminator {
        ReferenceTerminatorV1::Return => digest.update([0]),
        ReferenceTerminatorV1::Goto { target } => {
            digest.update([1]);
            digest.update(target.to_le_bytes());
        }
        ReferenceTerminatorV1::Switch {
            discriminant,
            values,
            otherwise,
        } => {
            digest.update([2]);
            digest_operand(digest, discriminant);
            put_len(digest, values.len());
            for (value, target) in values {
                digest.update(value.to_le_bytes());
                digest.update(target.to_le_bytes());
            }
            digest.update(otherwise.to_le_bytes());
        }
        ReferenceTerminatorV1::Assert {
            condition,
            expected,
            success,
            bounds_check,
        } => {
            digest.update([3, u8::from(*expected), u8::from(bounds_check.is_some())]);
            digest_operand(digest, condition);
            digest.update(success.to_le_bytes());
            if let Some(bounds_check) = bounds_check {
                digest_operand(digest, &bounds_check.index);
                digest_operand(digest, &bounds_check.length);
            }
        }
    }
}
