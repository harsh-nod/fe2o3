//! Original assertions evaluate the condition once and failure operands only
//! on the failure path. A modeled trap is not an invalid/refused execution.
use super::*;

pub(super) struct SourceAssertControlV40 {
    condition: TypedOperand,
    failure: [Option<TypedOperand>; 2],
    expected: bool,
    success: usize,
}

impl SourceAssertControlV40 {
    pub(super) fn derive(
        body: &SourceByteBody<'_, '_, '_>,
        block: usize,
        original: &Terminator,
        success: usize,
        out: &mut Writer<'_, '_>,
    ) -> Result<Self> {
        out.budget.charge_work(4)?;
        let Terminator::Assert {
            expected,
            target,
            unwind,
            ..
        } = original
        else {
            return Err(mismatch());
        };
        if *unwind != Unwind::Unreachable || target.role() != EdgeRole::AssertSuccess {
            return Err(Error::Statement(
                "original assertion unwinding is not modeled",
            ));
        }
        let (condition, failure) = body.assertion_operands(block, original, out)?;
        Ok(Self {
            condition,
            failure,
            expected: *expected,
            success,
        })
    }

    pub(super) fn emit(
        &self,
        root: usize,
        instance: usize,
        block: usize,
        out: &mut Writer<'_, '_>,
    ) -> Result<()> {
        out.budget.charge_work(1)?;
        write!(
            out,
            " let assertion_condition = invocation_source_operand_evaluate_v36(cursor.source, "
        )
        .map_err(|_| out.error())?;
        self.condition.emit(out)?;
        write!(out, ", {root}, {instance}, little_endian);\n let assertion_operand = InvocationSourceOperandObservationV36 {{ root: {root}, instance: {instance}, block: {block}, role: InvocationSourceOperandRoleV36::AssertCondition, operand: ").map_err(|_| out.error())?;
        self.condition.emit(out)?;
        write!(out, ", before: cursor.source, after: assertion_condition.source, value: assertion_condition.value }};\n if !assertion_condition.source.machine.valid || !matches!(assertion_condition.value, MemoryValueV30::Scalar(0) | MemoryValueV30::Scalar(1)) {{\n InvocationSourceBlockResultV36 {{ source: invocation_source_byte_refused_v36(assertion_condition.source), before_control: cursor.source, observations: cursor.observations, operands: seq![assertion_operand], returned: None }}\n }} else if assertion_condition.value == MemoryValueV30::Scalar({}) {{\n InvocationSourceBlockResultV36 {{ source: invocation_source_byte_pc_v36(assertion_condition.source, {}), before_control: cursor.source, observations: cursor.observations, operands: seq![assertion_operand], returned: None }}\n }} else {{\n let source = assertion_condition.source;\n", u8::from(self.expected), self.success).map_err(|_| out.error())?;
        for (ordinal, operand) in self.failure.iter().flatten().enumerate() {
            out.budget.charge_work(1)?;
            write!(out, " let assertion_before_{ordinal} = source;\n let assertion_failure_{ordinal} = invocation_source_operand_evaluate_v36(source, ").map_err(|_| out.error())?;
            operand.emit(out)?;
            write!(out, ", {root}, {instance}, little_endian);\n let source = assertion_failure_{ordinal}.source;\n").map_err(|_| out.error())?;
        }
        write!(out, " let operands = seq![assertion_operand").map_err(|_| out.error())?;
        for (ordinal, operand) in self.failure.iter().flatten().enumerate() {
            out.budget.charge_work(1)?;
            write!(out, ", InvocationSourceOperandObservationV36 {{ root: {root}, instance: {instance}, block: {block}, role: InvocationSourceOperandRoleV36::AssertMessage({ordinal}), operand: ").map_err(|_| out.error())?;
            operand.emit(out)?;
            write!(out, ", before: assertion_before_{ordinal}, after: assertion_failure_{ordinal}.source, value: assertion_failure_{ordinal}.value }}").map_err(|_| out.error())?;
        }
        write!(out, "];\n InvocationSourceBlockResultV36 {{ source: invocation_source_byte_trap_v40(source), before_control: cursor.source, observations: cursor.observations, operands, returned: None }}\n }}\n").map_err(|_| out.error())
    }
}

pub(super) fn headers() -> usize {
    size_of::<SourceAssertControlV40>()
        + 2 * size_of::<Result<SourceAssertControlV40>>()
        + size_of::<(TypedOperand, [Option<TypedOperand>; 2])>()
        + 2 * size_of::<Result<(TypedOperand, [Option<TypedOperand>; 2])>>()
        + size_of::<
            std::iter::Enumerate<
                std::iter::Flatten<std::slice::Iter<'static, Option<TypedOperand>>>,
            >,
        >()
        + size_of::<(
            &SourceByteBody<'_, '_, '_>,
            &Terminator,
            &mut Writer<'_, '_>,
            [usize; 7],
            bool,
        )>()
}

#[cfg(test)]
#[path = "original_semantic_mir_source_assert_control_v40_tests.rs"]
mod tests;
