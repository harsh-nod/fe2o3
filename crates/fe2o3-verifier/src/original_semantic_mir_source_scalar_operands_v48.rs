//! Scalar expressions retain ordered byte reads and moves before their write.
use super::super::super::OperatorV30;
use super::*;
use fe2o3_mir_model::semantic_mir_v1::{SemanticAssignmentV1, SemanticUnaryOpV1};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Operator {
    Not,
    Binary(OperatorV30),
}

impl Operator {
    fn code(self) -> u8 {
        match self {
            Self::Not => 0,
            Self::Binary(operator) => match operator {
                OperatorV30::And => 1,
                OperatorV30::Or => 2,
                OperatorV30::Xor => 3,
                OperatorV30::Equal => 4,
                OperatorV30::NotEqual => 5,
                OperatorV30::Less => 6,
                OperatorV30::LessEqual => 7,
                OperatorV30::Greater => 8,
                OperatorV30::GreaterEqual => 9,
            },
        }
    }

    fn result(self, input: ScalarV30) -> Result<ScalarV30> {
        if input == ScalarV30::Unit {
            return Err(unsupported());
        }
        Ok(match self {
            Self::Binary(operator) if operator.comparison() => ScalarV30::Bool,
            _ => input,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct Operation {
    destination: Destination,
    left: Value,
    right: Option<Value>,
    operator: Operator,
    input: ScalarV30,
    output: ScalarV30,
}

impl Operation {
    pub(super) fn derive(
        context: &Context<'_, '_, '_>,
        assignment: &SemanticAssignmentV1,
        destination: Destination,
        out: &mut Writer<'_, '_>,
    ) -> Result<Option<Self>> {
        out.budget.charge_work(4)?;
        if assignment.destination().ty() != assignment.value().result_type() {
            return Err(mismatch());
        }
        let (operator, left, right) = match assignment.value().kind() {
            Rvalue::Unary {
                operation: SemanticUnaryOpV1::Not,
                operand,
            } => (Operator::Not, operand, None),
            Rvalue::Binary {
                operation,
                left,
                right,
            } => (
                Operator::Binary(OperatorV30::from_source(*operation)?),
                left,
                Some(right),
            ),
            _ => return Err(unsupported()),
        };
        let input = context.scalar(left.ty(), out)?;
        let output = context.scalar(assignment.value().result_type(), out)?;
        if operator.result(input)? != output {
            return Err(mismatch());
        }
        let left = context.value(left, out)?;
        let right = right
            .map(|operand| {
                if context.scalar(operand.ty(), out)? != input {
                    return Err(mismatch());
                }
                context.value(operand, out)
            })
            .transpose()?;
        let is_local_value = |value| matches!(value, Value::Constant(_) | Value::Local { .. });
        if matches!(destination, Destination::Local(_))
            && is_local_value(left)
            && right.is_none_or(is_local_value)
        {
            // Retain the existing current-local primitive graph for this case.
            return Ok(None);
        }
        Ok(Some(Self {
            destination,
            left,
            right,
            operator,
            input,
            output,
        }))
    }

    pub(super) fn emit(self, out: &mut Writer<'_, '_>) -> Result<()> {
        out.budget.charge_work(1)?;
        write!(out, "InvocationSourceByteEventV36::ScalarOperands(InvocationSourceScalarOperandsV48 {{ destination: ")
            .map_err(|_| out.error())?;
        emit_destination(self.destination, out)?;
        write!(out, ", left: ").map_err(|_| out.error())?;
        emit_value(self.left, out)?;
        write!(out, ", right: ").map_err(|_| out.error())?;
        if let Some(right) = self.right {
            write!(out, "Some(").map_err(|_| out.error())?;
            emit_value(right, out)?;
            write!(out, ")").map_err(|_| out.error())?;
        } else {
            write!(out, "None").map_err(|_| out.error())?;
        }
        let signed = matches!(self.input, ScalarV30::Integer { signed: true, .. });
        write!(
            out,
            ", operation: {}int, input_bits: {}int, input_signed: {signed}, output_bits: {}int }})",
            self.operator.code(),
            self.input.width(),
            self.output.width()
        )
        .map_err(|_| out.error())
    }
}

pub(super) fn headers() -> usize {
    size_of::<Operation>()
        + 2 * size_of::<Result<Option<Operation>>>()
        + size_of::<Operator>()
        + size_of::<Value>()
        + size_of::<Option<Value>>()
        + 2 * size_of::<ScalarV30>()
        + size_of::<Destination>()
        + 6 * size_of::<&()>()
}

#[cfg(test)]
#[path = "original_semantic_mir_source_scalar_operands_v48_tests.rs"]
mod tests;
