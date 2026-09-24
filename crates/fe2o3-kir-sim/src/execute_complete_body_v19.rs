//! One actual SSA instruction per ordinary simulator step. The surrounding
//! engine owns real CFG branches, edge arguments, debug checkpoints and memory.
//! No packed-program interpreter or physical register execution is introduced.

use super::*;
use fe2o3_kernel_ir::{
    Gfx942ProgramBinaryOpcodeV1 as Opcode, Gfx942ProgramInstructionV1 as Instruction,
};

#[inline(never)]
pub(super) fn execute(
    engine: &mut Engine<'_, impl SimulationEventSinkV1>,
    values: &HashMap<ValueId, RuntimeValue>,
    operation: &Operation,
    site: &CompactSite,
) -> Result<SmallResults<RuntimeValue>, SimulationExecutionErrorV1> {
    let invariant = |message| {
        engine.at(
            *site,
            SimulationExecutionErrorKindV1::InternalInvariant(message),
        )
    };
    let step = match &operation.kind {
        OperationKind::Gfx942CompleteBodyDeclaration(declaration)
            if operation.results.is_empty() && declaration.validate_shape().is_ok() =>
        {
            // Retained authoring metadata has no runtime result or memory effect.
            return Ok(SmallResults::None);
        }
        OperationKind::Gfx942CompleteBodyStep(step)
            if step.validate_shape().is_ok()
                && matches!(operation.results.as_slice(),
                    [result] if result.ty == Type::Scalar(ScalarType::U32)) =>
        {
            step
        }
        _ => return Err(invariant("complete-body shape changed after preflight")),
    };
    let scalar = |value| {
        let value = scalar_value(engine, values, value, site)?;
        if value.ty() != ScalarType::U32 {
            return Err(invariant("complete-body non-U32 operand"));
        }
        Ok(value)
    };
    let left =
        scalar(step.operands[0].ok_or_else(|| invariant("complete-body missing first operand"))?)?;
    let result = match step.instruction {
        Instruction::Move { .. } => left,
        Instruction::Binary { opcode, .. } => {
            let right = scalar(
                step.operands[1]
                    .ok_or_else(|| invariant("complete-body missing second operand"))?,
            )?;
            // Ordinary unchecked KIR Add/Subtract reject unsigned overflow.
            // E32 authoring explicitly wraps: use the existing checked scalar
            // engine and discard only its overflow flag, never duplicate math.
            let binary = match opcode {
                Opcode::Add => BinaryOp::Checked(CheckedBinaryOperator::Add),
                Opcode::Subtract => BinaryOp::Checked(CheckedBinaryOperator::Subtract),
                Opcode::And => BinaryOp::BitAnd,
                Opcode::Or => BinaryOp::BitOr,
                Opcode::Xor => BinaryOp::BitXor,
            };
            let result = execute_binary(binary, left, right, engine.target)
                .map_err(|kind| engine.at(*site, kind))?;
            match (binary, result) {
                (BinaryOp::Checked(_), SmallResults::Two(value, overflow))
                    if overflow.ty() == ScalarType::Bool =>
                {
                    value
                }
                (
                    BinaryOp::BitAnd | BinaryOp::BitOr | BinaryOp::BitXor,
                    SmallResults::One(value),
                ) => value,
                _ => return Err(invariant("complete-body scalar engine result arity")),
            }
        }
    };
    Ok(SmallResults::One(RuntimeValue::Scalar(result)))
}
