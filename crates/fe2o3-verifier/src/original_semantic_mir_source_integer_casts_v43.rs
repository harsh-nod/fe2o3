//! Integer casts use only the original scalar types and operand state.
use super::*;
use fe2o3_mir_model::semantic_mir_v1::{SemanticAssignmentV1, SemanticCastKindV1};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct Cast {
    destination: Destination,
    value: Value,
    input: ScalarV30,
    output: ScalarV30,
}

impl Cast {
    pub(super) fn derive(
        context: &Context<'_, '_, '_>,
        assignment: &SemanticAssignmentV1,
        out: &mut Writer<'_, '_>,
    ) -> Result<Option<Self>> {
        out.budget.charge_work(2)?;
        let Rvalue::Cast { kind, operand } = assignment.value().kind() else {
            return Ok(None);
        };
        if *kind != SemanticCastKindV1::Integer {
            return Err(unsupported());
        }
        if assignment.destination().ty() != assignment.value().result_type() {
            return Err(mismatch());
        }
        let input = context.scalar(operand.ty(), out)?;
        let output = context.scalar(assignment.value().result_type(), out)?;
        if !matches!(input, ScalarV30::Bool | ScalarV30::Integer { .. })
            || !matches!(output, ScalarV30::Integer { .. })
        {
            return Err(unsupported());
        }
        Ok(Some(Self {
            destination: context.destination(assignment.destination(), out)?,
            value: context.value(operand, out)?,
            input,
            output,
        }))
    }

    pub(super) fn emit(self, out: &mut Writer<'_, '_>) -> Result<()> {
        out.budget.charge_work(1)?;
        let signed = matches!(self.input, ScalarV30::Integer { signed: true, .. });
        write!(out, "InvocationSourceByteEventV36::IntegerCast(InvocationSourceIntegerCastV43 {{ destination: ")
            .map_err(|_| out.error())?;
        emit_destination(self.destination, out)?;
        write!(out, ", value: ").map_err(|_| out.error())?;
        emit_value(self.value, out)?;
        write!(
            out,
            ", input_bits: {}int, input_signed: {signed}, output_bits: {}int }})",
            self.input.width(),
            self.output.width()
        )
        .map_err(|_| out.error())
    }
}

pub(super) fn headers() -> usize {
    size_of::<Cast>()
        + 2 * size_of::<Result<Option<Cast>>>()
        + 2 * size_of::<ScalarV30>()
        + size_of::<SemanticCastKindV1>()
        + size_of::<Value>()
        + size_of::<Destination>()
        + 4 * size_of::<&()>()
}

#[cfg(test)]
#[path = "original_semantic_mir_source_integer_casts_v43_tests.rs"]
mod tests;
