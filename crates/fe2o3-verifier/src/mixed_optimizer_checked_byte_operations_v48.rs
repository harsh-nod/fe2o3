//! Concrete checked arithmetic over actual typed SSA operands. Both result
//! ordinals are authenticated before emission; overflow is a value, not a trap.
use super::*;
use fe2o3_kernel_ir::{BinaryOp, CheckedBinaryOperator};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct CheckedByteOperationV48 {
    pub(super) operands: [usize; 2],
    pub(super) results: [usize; 2],
    pub(super) bits: u32,
    pub(super) signed: bool,
    pub(super) operator: CheckedBinaryOperator,
}

fn unsupported() -> Error {
    Error::Statement("actual checked byte operation is not modeled")
}

pub(super) fn width(scalar: ScalarType, index: FormalIndexWidth) -> Result<u32> {
    if scalar == ScalarType::Index {
        return match index {
            FormalIndexWidth::Bits32 => Ok(32),
            FormalIndexWidth::Bits64 => Ok(64),
            FormalIndexWidth::Unknown => Err(unsupported()),
        };
    }
    if !scalar.is_integer() {
        return Err(unsupported());
    }
    match scalar.bit_width().map(u32::from) {
        Some(bits @ (8 | 16 | 32 | 64 | 128)) => Ok(bits),
        _ => Err(unsupported()),
    }
}

impl CheckedByteOperationV48 {
    pub(super) fn derive(
        inventory: &Inventory<'_>,
        operation: usize,
        index: FormalIndexWidth,
        out: &mut Writer<'_, '_>,
    ) -> Result<Self> {
        out.budget.charge_work(48)?;
        let row = inventory.operations().get(operation).ok_or_else(mismatch)?;
        let OperationKind::Binary {
            op: BinaryOp::Checked(operator),
            lhs,
            rhs,
        } = row.operation.kind
        else {
            return Err(unsupported());
        };
        if row.operands.len() != 2
            || row.results.len() != 2
            || row.operation.results.len() != 2
            || !row.effects.is_empty()
        {
            return Err(mismatch());
        }
        let function = inventory
            .functions()
            .get(row.coordinate.block.function.0 as usize)
            .ok_or_else(mismatch)?;
        if !function.operations.contains(&operation) {
            return Err(mismatch());
        }
        let first = inventory
            .definitions()
            .get(row.results.start)
            .ok_or_else(mismatch)?;
        let Type::Scalar(scalar) = first.ty else {
            return Err(unsupported());
        };
        let bits = width(*scalar, index)?;
        let mut operands = [0; 2];
        let results = [row.results.start, row.results.start + 1];
        for ordinal in 0..2 {
            let usage = inventory
                .uses()
                .get(row.operands.start + ordinal)
                .ok_or_else(mismatch)?;
            let operand = inventory
                .definitions()
                .get(usage.definition)
                .ok_or_else(mismatch)?;
            let result = inventory
                .definitions()
                .get(results[ordinal])
                .ok_or_else(mismatch)?;
            if usage.coordinate
                != (Use::OperationOperand {
                    operation: row.coordinate,
                    operand: ordinal as u32,
                })
                || usage.value != [lhs, rhs][ordinal]
                || operand.value != Some(usage.value)
                || operand.ty != first.ty
                || !function.definitions.contains(&usage.definition)
                || !function.definitions.contains(&results[ordinal])
                || result.coordinate
                    != (Definition::Result {
                        operation: row.coordinate,
                        result: ordinal as u32,
                    })
                || result.value != Some(row.operation.results[ordinal].id)
                || result.ty != &row.operation.results[ordinal].ty
                || (ordinal == 1 && result.ty != &Type::Scalar(ScalarType::Bool))
            {
                return Err(mismatch());
            }
            operands[ordinal] = usage.definition;
        }
        Ok(Self {
            operands,
            results,
            bits,
            signed: scalar.is_signed_integer(),
            operator,
        })
    }

    pub(super) fn emit_step(
        self,
        before: ByteMemoryStateNamesV30<'_>,
        after: ByteMemoryStateNamesV30<'_>,
        out: &mut Writer<'_, '_>,
    ) -> Result<()> {
        out.budget.charge_work(16)?;
        let [left, right] = self.operands;
        let [value, overflow] = self.results;
        emit!(
            out,
            " let checked_modulus = memory_value_modulus_v30({});\n let checked_ok = {} && {}.len() > {} && {}.len() > {} && {}.len() > {} && {}.len() > {}\n && (match {}[{left}] {{ MemoryValueV30::Scalar(v) => 0 <= v < checked_modulus, _ => false }})\n && (match {}[{right}] {{ MemoryValueV30::Scalar(v) => 0 <= v < checked_modulus, _ => false }});\n",
            self.bits / 8,
            before.valid,
            before.values,
            left,
            before.values,
            right,
            before.values,
            value,
            before.values,
            overflow,
            before.values,
            before.values
        );
        for (name, definition) in [("left", left), ("right", right)] {
            emit!(
                out,
                " let checked_{name}_bits = if checked_ok {{ match {}[{definition}] {{ MemoryValueV30::Scalar(v) => v, _ => 0int }} }} else {{ 0int }};\n let checked_{name} = ",
                before.values
            );
            if self.signed {
                emit!(
                    out,
                    "if checked_{name}_bits >= checked_modulus / 2 {{ checked_{name}_bits - checked_modulus }} else {{ checked_{name}_bits }}"
                );
            } else {
                emit!(out, "checked_{name}_bits");
            }
            emit!(out, ";\n");
        }
        let symbol = match self.operator {
            CheckedBinaryOperator::Add => "+",
            CheckedBinaryOperator::Subtract => "-",
            CheckedBinaryOperator::Multiply => "*",
        };
        emit!(
            out,
            " let checked_wide = checked_left {symbol} checked_right;\n let checked_in_range = "
        );
        if self.signed {
            emit!(
                out,
                "-(checked_modulus / 2) <= checked_wide < checked_modulus / 2"
            );
        } else {
            emit!(out, "0 <= checked_wide < checked_modulus");
        }
        emit!(
            out,
            ";\n let {} = checked_ok;\n let {} = if checked_ok {{ {}.update({value}int, MemoryValueV30::Scalar(checked_wide % checked_modulus)).update({overflow}int, MemoryValueV30::Scalar(if checked_in_range {{ 0int }} else {{ 1int }})) }} else {{ {} }};\n let {} = {};\n let {} = {};\n let {} = {};\n",
            after.valid,
            after.values,
            before.values,
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
    size_of::<CheckedByteOperationV48>()
        + size_of::<Result<CheckedByteOperationV48>>()
        + size_of::<(
            CheckedBinaryOperator,
            ScalarType,
            FormalIndexWidth,
            [usize; 4],
            u32,
            bool,
        )>()
        + size_of::<(
            ByteMemoryStateNamesV30<'static>,
            ByteMemoryStateNamesV30<'static>,
        )>()
        + size_of::<([usize; 8], [&(); 8], [Result<()>; 2], std::fmt::Result)>()
}
