//! Independent CPU expressions in the existing scalar/byte interpretation.
use super::{Error, Resource, Result, Writer};
use crate::portable_reference_v1::signature::{
    ReferenceLogicalSignaturePreimageV1 as Signature, ReferenceSignatureInputV1 as Input,
};
use crate::portable_reference_v1::{
    ReferenceArgumentRelationV1 as Relation, ReferenceBinaryOpV1 as Binary,
    ReferenceCastKindV1 as Cast, ReferenceConstantV1 as Constant,
    ReferenceEffectExpressionV1 as Expression, ReferenceGuardAtomV1 as Atom,
    ReferencePathPredicateV1 as Predicate, ReferenceScalarTypeV1 as Scalar,
};
use std::{fmt::Write as _, mem::size_of};

pub(super) struct Context<'a> {
    pub signature: &'a Signature,
    pub relations: &'a [Relation],
    pub index_bits: u32,
    pub rank: u32,
}

#[cfg(test)]
#[path = "original_semantic_mir_reference_expressions_v69_tests.rs"]
mod tests;

fn unsupported() -> Error {
    Error::Statement("original/reference scalar expression is outside the closed byte model")
}

pub(super) fn bits(scalar: Scalar, index_bits: u32) -> Result<u32> {
    Ok(match scalar {
        Scalar::Bool => 1,
        Scalar::U8 | Scalar::I8 => 8,
        Scalar::U16 | Scalar::I16 => 16,
        Scalar::U32 | Scalar::I32 | Scalar::F32 => 32,
        Scalar::U64 | Scalar::I64 | Scalar::F64 => 64,
        Scalar::Usize | Scalar::Isize if matches!(index_bits, 32 | 64) => index_bits,
        _ => return Err(unsupported()),
    })
}

fn signed(scalar: Scalar) -> bool {
    matches!(
        scalar,
        Scalar::I8 | Scalar::I16 | Scalar::I32 | Scalar::I64 | Scalar::Isize
    )
}

fn float(scalar: Scalar) -> bool {
    matches!(scalar, Scalar::F32 | Scalar::F64)
}

fn binary(operation: Binary, scalar: Scalar) -> Result<(u32, Scalar)> {
    let comparison = matches!(
        operation,
        Binary::Equal
            | Binary::NotEqual
            | Binary::LessThan
            | Binary::LessEqual
            | Binary::GreaterThan
            | Binary::GreaterEqual
    );
    let code = if float(scalar) {
        match operation {
            Binary::Add => 11,
            Binary::Subtract => 12,
            Binary::Multiply => 13,
            Binary::Divide => 14,
            Binary::Remainder => 15,
            Binary::Equal => 16,
            Binary::NotEqual => 17,
            Binary::LessThan => 18,
            Binary::LessEqual => 19,
            Binary::GreaterThan => 20,
            Binary::GreaterEqual => 21,
            _ => return Err(unsupported()),
        }
    } else {
        match operation {
            Binary::BitAnd => 1,
            Binary::BitOr => 2,
            Binary::BitXor => 3,
            Binary::Equal => 4,
            Binary::NotEqual => 5,
            Binary::LessThan => 6,
            Binary::LessEqual => 7,
            Binary::GreaterThan => 8,
            Binary::GreaterEqual => 9,
            _ => return Err(unsupported()),
        }
    };
    Ok((code, if comparison { Scalar::Bool } else { scalar }))
}

impl Context<'_> {
    fn input(&self, raw: u32, out: &mut Writer<'_, '_>) -> Result<(u32, Scalar)> {
        out.budget.charge_work(2)?;
        let Some(Relation::SharedSliceInput { argument, element }) =
            self.relations.get(raw as usize)
        else {
            return Err(unsupported());
        };
        if *argument as usize >= self.signature.kernel_inputs().len() {
            return Err(unsupported());
        }
        Ok((*argument, *element))
    }

    pub(super) fn expression(
        &self,
        value: &Expression,
        out: &mut Writer<'_, '_>,
    ) -> Result<Scalar> {
        self.expression_at(value, 1, out)
    }

    fn expression_at(
        &self,
        value: &Expression,
        depth: usize,
        out: &mut Writer<'_, '_>,
    ) -> Result<Scalar> {
        out.budget.charge_work(8)?;
        if depth > fe2o3_pliron::MAX_PRODUCTION_SEMANTIC_EXPRESSION_DEPTH_V2 {
            return Err(unsupported());
        }
        // Conservative cumulative recursion scratch, retired with generation.
        out.budget.reserve_storage(size_of::<(
            &Self,
            &Expression,
            usize,
            &mut Writer<'_, '_>,
            Scalar,
            Scalar,
            u32,
        )>())?;
        match value {
            Expression::PointCoordinate { axis } => {
                if *axis >= self.rank || self.rank == 0 || self.rank > 3 {
                    return Err(unsupported());
                }
                write!(
                    out,
                    "MemoryValueV30::Scalar(byte_execution_index_v37(execution, 0int, {axis}int))"
                )
                .map_err(|_| out.error())?;
                Ok(Scalar::Usize)
            }
            Expression::KernelScalarArgument { argument } => {
                let Some(Input::Scalar(scalar)) =
                    self.signature.kernel_inputs().get(*argument as usize)
                else {
                    return Err(unsupported());
                };
                write!(out, "arguments[{argument}int]").map_err(|_| out.error())?;
                Ok(*scalar)
            }
            Expression::Constant(Constant::Scalar {
                scalar,
                bits: value,
            }) => {
                let width = bits(*scalar, self.index_bits)?;
                if *value >= (1u128 << width) {
                    return Err(unsupported());
                }
                write!(out, "MemoryValueV30::Scalar({value}int)").map_err(|_| out.error())?;
                Ok(*scalar)
            }
            Expression::Constant(Constant::ZeroSized) => Err(unsupported()),
            Expression::InputLength { reference_argument } => {
                let (argument, _) = self.input(*reference_argument, out)?;
                write!(out, "(match arguments[{argument}int] {{ MemoryValueV30::Slice(slice) => MemoryValueV30::Scalar(slice.length), _ => MemoryValueV30::Undefined }})").map_err(|_| out.error())?;
                Ok(Scalar::Usize)
            }
            // A path-sensitive CPU bounds/read theorem is a separate remaining
            // obligation. Never borrow a GPU load value as the CPU result.
            Expression::InputLoad { .. } => Err(unsupported()),
            Expression::Binary {
                operation,
                lhs,
                rhs,
                checked,
            } => {
                if *checked {
                    return Err(unsupported());
                }
                write!(out, "{{ let left = ").map_err(|_| out.error())?;
                let left = self.expression_at(lhs, depth + 1, out)?;
                write!(out, "; let right = ").map_err(|_| out.error())?;
                if self.expression_at(rhs, depth + 1, out)? != left {
                    return Err(unsupported());
                }
                let (operation, result) = binary(*operation, left)?;
                self.operation(operation, left, result, false, out)?;
                Ok(result)
            }
            Expression::Unary { operation, operand } => {
                write!(out, "{{ let left = ").map_err(|_| out.error())?;
                let scalar = self.expression_at(operand, depth + 1, out)?;
                let operation = match operation {
                    crate::portable_reference_v1::ReferenceUnaryOpV1::Not if !float(scalar) => 0,
                    crate::portable_reference_v1::ReferenceUnaryOpV1::Negate if float(scalar) => 10,
                    _ => return Err(unsupported()),
                };
                write!(out, "; let right = MemoryValueV30::Undefined").map_err(|_| out.error())?;
                self.operation(operation, scalar, scalar, true, out)?;
                Ok(scalar)
            }
            Expression::Cast {
                kind,
                source,
                target,
                operand,
            } => {
                if *kind != Cast::Integer
                    || float(*source)
                    || float(*target)
                    || *target == Scalar::Bool
                {
                    return Err(unsupported());
                }
                write!(out, "{{ let value = ").map_err(|_| out.error())?;
                if self.expression_at(operand, depth + 1, out)? != *source {
                    return Err(unsupported());
                }
                let from = bits(*source, self.index_bits)?;
                let to = bits(*target, self.index_bits)?;
                write!(
                    out,
                    "; invocation_source_integer_cast_value_v43(value, {from}int, {}, {to}int) }}",
                    signed(*source)
                )
                .map_err(|_| out.error())?;
                Ok(*target)
            }
        }
    }

    fn operation(
        &self,
        operation: u32,
        input: Scalar,
        output: Scalar,
        unary: bool,
        out: &mut Writer<'_, '_>,
    ) -> Result<()> {
        let input_bits = bits(input, self.index_bits)?;
        let output_bits = bits(output, self.index_bits)?;
        let right = if unary {
            "None"
        } else {
            "Some(InvocationSourceByteValueV36::Constant(0int))"
        };
        write!(out, "; invocation_source_scalar_operands_value_v48(byte_root_frame_with_execution_v37(0int, execution), InvocationSourceScalarOperandsV48 {{ destination: InvocationSourceByteDestinationV36::Local(0int), left: InvocationSourceByteValueV36::Constant(0int), right: {right}, operation: {operation}int, input_bits: {input_bits}int, input_signed: {}, output_bits: {output_bits}int }}, left, right) }}", signed(input)).map_err(|_| out.error())
    }

    pub(super) fn predicate(&self, predicate: &Predicate, out: &mut Writer<'_, '_>) -> Result<()> {
        write!(out, "(false").map_err(|_| out.error())?;
        for clause in &predicate.clauses {
            out.budget.charge_work(1)?;
            write!(out, " || (true").map_err(|_| out.error())?;
            for atom in &clause.atoms {
                out.budget.charge_work(1)?;
                write!(out, " && ({{ let condition = ").map_err(|_| out.error())?;
                match atom {
                    Atom::Assert {
                        condition,
                        expected,
                    } => {
                        if self.expression(condition, out)? != Scalar::Bool {
                            return Err(unsupported());
                        }
                        write!(
                            out,
                            "; condition == MemoryValueV30::Scalar({}int) }})",
                            u8::from(*expected)
                        )
                        .map_err(|_| out.error())?;
                    }
                    Atom::SwitchValueSet {
                        discriminant,
                        values,
                        inside_set,
                    } => {
                        let scalar = self.expression(discriminant, out)?;
                        if float(scalar) {
                            return Err(unsupported());
                        }
                        let width = bits(scalar, self.index_bits)?;
                        write!(out, "; invocation_source_byte_value_typed_v36(condition, {width}int) && ({} (false", if *inside_set { "" } else { "!" }).map_err(|_| out.error())?;
                        for value in values {
                            out.budget.charge_work(1)?;
                            if *value >= (1u128 << width) {
                                return Err(unsupported());
                            }
                            write!(out, " || condition == MemoryValueV30::Scalar({value}int)")
                                .map_err(|_| out.error())?;
                        }
                        write!(out, ")) }})").map_err(|_| out.error())?;
                    }
                }
            }
            write!(out, ")").map_err(|_| out.error())?;
        }
        write!(out, ")").map_err(|_| out.error())
    }
}
