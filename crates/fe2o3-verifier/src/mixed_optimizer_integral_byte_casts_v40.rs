//! Exact integral casts over raw scalar bits, independent of pointer storage.
//! The containing byte function retains inventory ownership and budget custody.
use super::*;
use fe2o3_kernel_ir::{CastKind, plan_integer_cast_v1};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct IntegralByteCastV40 {
    pub(super) input: usize,
    pub(super) result: usize,
    pub(super) input_bits: u32,
    pub(super) output_bits: u32,
    pub(super) kind: CastKind,
}

fn unsupported() -> Error {
    Error::Statement("actual integral byte cast is not modeled")
}

fn width(scalar: ScalarType, index: FormalIndexWidth) -> Result<u32> {
    if scalar == ScalarType::Bool {
        return Ok(1);
    }
    if !scalar.is_integer() {
        return Err(unsupported());
    }
    if scalar == ScalarType::Index {
        return match index {
            FormalIndexWidth::Bits32 => Ok(32),
            FormalIndexWidth::Bits64 => Ok(64),
            FormalIndexWidth::Unknown => Err(unsupported()),
        };
    }
    scalar.bit_width().map(u32::from).ok_or_else(unsupported)
}

impl IntegralByteCastV40 {
    pub(super) fn derive(
        inventory: &Inventory<'_>,
        operation: usize,
        index: FormalIndexWidth,
        out: &mut Writer<'_, '_>,
    ) -> Result<Self> {
        out.budget.charge_work(32)?;
        let row = inventory.operations().get(operation).ok_or_else(mismatch)?;
        let OperationKind::Cast { kind, to, .. } = &row.operation.kind else {
            return Err(mismatch());
        };
        if row.operands.len() != 1 || row.results.len() != 1 || !row.effects.is_empty() {
            return Err(mismatch());
        }
        let operand = inventory
            .uses()
            .get(row.operands.start)
            .ok_or_else(mismatch)?;
        let input = inventory
            .definitions()
            .get(operand.definition)
            .ok_or_else(mismatch)?;
        let result = inventory
            .definitions()
            .get(row.results.start)
            .ok_or_else(mismatch)?;
        let function = inventory
            .functions()
            .get(row.coordinate.block.function.0 as usize)
            .ok_or_else(mismatch)?;
        if operand.coordinate
            != (Use::OperationOperand {
                operation: row.coordinate,
                operand: 0,
            })
            || result.coordinate
                != (Definition::Result {
                    operation: row.coordinate,
                    result: 0,
                })
            || !function.operations.contains(&operation)
            || !function.definitions.contains(&operand.definition)
            || !function.definitions.contains(&row.results.start)
            || result.ty != to
        {
            return Err(mismatch());
        }
        let (Type::Scalar(from), Type::Scalar(to)) = (input.ty, result.ty) else {
            return Err(unsupported());
        };
        // Reuse the canonical admitted one-operation contract. In particular,
        // this never admits integer/pointer or float/integer reinterpretation.
        if plan_integer_cast_v1(*from, *to) != Some([Some((*kind, *to)), None]) {
            return Err(unsupported());
        }
        let input_bits = width(*from, index)?;
        let output_bits = width(*to, index)?;
        let valid = match kind {
            CastKind::Truncate => input_bits > output_bits,
            CastKind::SignExtend => from.is_signed_integer() && input_bits < output_bits,
            CastKind::ZeroExtend => !from.is_signed_integer() && input_bits <= output_bits,
            // The target-neutral U64/INDEX bridge requires actual equal widths.
            // A 32-bit target must not silently truncate its nominal bitcast.
            CastKind::Bitcast => input_bits == output_bits,
            _ => false,
        };
        if !valid {
            return Err(unsupported());
        }
        Ok(Self {
            input: operand.definition,
            result: row.results.start,
            input_bits,
            output_bits,
            kind: *kind,
        })
    }

    pub(super) fn emit_step(
        self,
        before: ByteMemoryStateNamesV30<'_>,
        after: ByteMemoryStateNamesV30<'_>,
        out: &mut Writer<'_, '_>,
    ) -> Result<()> {
        out.budget.charge_work(12)?;
        emit!(out, " let integral_source_modulus = ");
        if self.input_bits == 1 {
            emit!(out, "2int");
        } else {
            emit!(out, "memory_value_modulus_v30({})", self.input_bits / 8);
        }
        emit!(
            out,
            ";\n let integral_target_modulus = memory_value_modulus_v30({});\n let integral_ok = {} && {}.len() > {} && {}.len() > {} && match {}[{}] {{ MemoryValueV30::Scalar(v) => 0 <= v < integral_source_modulus, _ => false }};\n let integral_input = if integral_ok {{ match {}[{}] {{ MemoryValueV30::Scalar(v) => v, _ => 0int }} }} else {{ 0int }};\n let integral_result = ",
            self.output_bits / 8,
            before.valid,
            before.values,
            self.input,
            before.values,
            self.result,
            before.values,
            self.input,
            before.values,
            self.input
        );
        match self.kind {
            CastKind::Truncate => emit!(out, "integral_input % integral_target_modulus"),
            CastKind::SignExtend => emit!(
                out,
                "if integral_input >= integral_source_modulus / 2 {{ integral_input + integral_target_modulus - integral_source_modulus }} else {{ integral_input }}"
            ),
            CastKind::ZeroExtend | CastKind::Bitcast => emit!(out, "integral_input"),
            _ => return Err(mismatch()),
        }
        emit!(
            out,
            ";\n let {} = integral_ok && 0 <= integral_result < integral_target_modulus;\n let {} = if {} {{ {}.update({}int, MemoryValueV30::Scalar(integral_result)) }} else {{ {} }};\n let {} = {};\n let {} = {};\n let {} = {};\n",
            after.valid,
            after.values,
            after.valid,
            before.values,
            self.result,
            before.values,
            after.memory,
            before.memory,
            after.generations,
            before.generations,
            after.frames,
            before.frames
        );
        Ok(())
    }
}

pub(super) fn headers() -> usize {
    size_of::<IntegralByteCastV40>()
        + size_of::<Result<IntegralByteCastV40>>()
        + size_of::<(CastKind, ScalarType, ScalarType, FormalIndexWidth, [u32; 2])>()
        + size_of::<(
            ByteMemoryStateNamesV30<'static>,
            ByteMemoryStateNamesV30<'static>,
        )>()
        + size_of::<([usize; 8], [&(); 8], [Result<()>; 2], std::fmt::Result)>()
}
