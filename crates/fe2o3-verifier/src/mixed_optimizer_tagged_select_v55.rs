//! Exact non-scalar Select transport. No arithmetic, address formation, or
//! memory/currentness authority is inferred from choosing a tagged value.
use super::*;
use fe2o3_kernel_ir::ValueId;

#[derive(Clone, Copy)]
pub(super) struct TaggedSelectV55<'a> {
    inputs: [usize; 3],
    result: usize,
    ty: &'a Type,
    width: FormalIndexWidth,
}

impl<'a> TaggedSelectV55<'a> {
    pub(super) fn same_body_v56(
        &self,
        other: &TaggedSelectV55<'_>,
        out: &mut Writer<'_, '_>,
    ) -> Result<bool> {
        out.budget.charge_work(6)?;
        Ok(self.inputs == other.inputs
            && self.result == other.result
            && self.width == other.width
            && super::super::congruence_v27::compare_type(self.ty, other.ty, out)?
                == std::cmp::Ordering::Equal)
    }

    pub(super) fn derive(
        inventory: &'a Inventory<'_>,
        operation: usize,
        width: FormalIndexWidth,
        out: &mut Writer<'_, '_>,
    ) -> Result<Option<Self>> {
        out.budget.charge_work(2)?;
        let row = inventory.operations().get(operation).ok_or_else(mismatch)?;
        let OperationKind::Select {
            condition,
            true_value,
            false_value,
        } = row.operation.kind
        else {
            return Ok(None);
        };
        let result = inventory
            .definitions()
            .get(row.results.start)
            .ok_or_else(mismatch)?;
        if matches!(result.ty, Type::Scalar(_)) {
            // Scalar and IEEE Select retain their existing, separate adapters.
            return Ok(None);
        }
        out.budget.charge_work(16)?;
        let function = inventory
            .functions()
            .get(row.coordinate.block.function.0 as usize)
            .ok_or_else(mismatch)?;
        if row.results.len() != 1
            || row.operation.results.len() != 1
            || row.operands.len() != 3
            || !row.effects.is_empty()
            || !function.operations.contains(&operation)
            || !function.definitions.contains(&row.results.start)
            || result.coordinate
                != (Definition::Result {
                    operation: row.coordinate,
                    result: 0,
                })
            || result.value != Some(row.operation.results[0].id)
        {
            return Err(mismatch());
        }
        value_type(result.ty)?;
        scalar_bytes(ScalarType::Index, width)?;
        if super::super::congruence_v27::compare_type(result.ty, &row.operation.results[0].ty, out)?
            != std::cmp::Ordering::Equal
        {
            return Err(mismatch());
        }
        let values = [condition, true_value, false_value];
        let mut inputs = [0; 3];
        let mut types = [&Type::Unit; 3];
        for (ordinal, index) in row.operands.clone().enumerate() {
            out.budget.charge_work(8)?;
            let usage = inventory.uses().get(index).ok_or_else(mismatch)?;
            let definition = inventory
                .definitions()
                .get(usage.definition)
                .ok_or_else(mismatch)?;
            if usage.coordinate
                != (Use::OperationOperand {
                    operation: row.coordinate,
                    operand: ordinal as u32,
                })
                || usage.value != values[ordinal]
                || definition.value != Some(usage.value)
                || !function.definitions.contains(&usage.definition)
            {
                return Err(mismatch());
            }
            inputs[ordinal] = usage.definition;
            types[ordinal] = definition.ty;
        }
        super::super::select_types(types[0], types[1], types[2], result.ty, out)?;
        Ok(Some(Self {
            inputs,
            result: row.results.start,
            ty: result.ty,
            width,
        }))
    }

    pub(super) fn emit_step(
        self,
        before: ByteMemoryStateNamesV30<'_>,
        after: ByteMemoryStateNamesV30<'_>,
        out: &mut Writer<'_, '_>,
    ) -> Result<()> {
        out.budget.charge_work(16)?;
        emit!(
            out,
            " let tagged_select_inputs_ok = {} && {}.len() > {}",
            before.valid,
            before.values,
            self.result
        );
        for input in self.inputs {
            emit!(out, " && {}.len() > {input}", before.values);
        }
        emit!(
            out,
            " && ({}[{}] == MemoryValueV30::Scalar(0int) || {}[{}] == MemoryValueV30::Scalar(1int));\n",
            before.values,
            self.inputs[0],
            before.values,
            self.inputs[0]
        );
        // Select is not a dereference. Keep the entire chosen pointer/slice,
        // including its nominal allocation, generation, view and length. An
        // unselected Undefined value does not make the selected value invalid.
        emit!(
            out,
            " let tagged_select_value = if !tagged_select_inputs_ok {{ MemoryValueV30::Undefined }} else if {}[{}] == MemoryValueV30::Scalar(1int) {{ {}[{}] }} else {{ {}[{}] }};\n let {} = tagged_select_inputs_ok && (",
            before.values,
            self.inputs[0],
            before.values,
            self.inputs[1],
            before.values,
            self.inputs[2],
            after.valid
        );
        emit_value_type(self.ty, self.width, "tagged_select_value", out)?;
        emit!(
            out,
            ");\n let {} = if {} {{ {}.update({}int, tagged_select_value) }} else {{ {} }};\n let {} = {};\n let {} = {};\n let {} = {};\n",
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
    size_of::<TaggedSelectV55<'static>>()
        + size_of::<Result<Option<TaggedSelectV55<'static>>>>()
        + size_of::<([ValueId; 3], [usize; 3], [&'static Type; 3])>()
        + size_of::<(
            ByteMemoryStateNamesV30<'static>,
            ByteMemoryStateNamesV30<'static>,
        )>()
        + size_of::<([usize; 8], [&(); 8], [Result<()>; 3], std::fmt::Result)>()
}
