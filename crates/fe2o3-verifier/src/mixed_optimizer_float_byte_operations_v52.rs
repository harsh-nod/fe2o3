//! Exact F32/F64 value transport and strict operator congruence. Integer
//! operators and arbitrary callback opcodes cannot enter this adapter.
use super::*;
use fe2o3_kernel_ir::{BinaryOp, ComparePredicate, Constant, UnaryOp, ValueId};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Action {
    Constant(u64),
    Operator(u8),
    Select,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct FloatByteOperationV52 {
    action: Action,
    inputs: [usize; 3],
    arity: usize,
    result: usize,
    input_bits: u32,
    output_bits: u32,
}

fn float_width(ty: &Type) -> Option<u32> {
    match ty {
        Type::Scalar(ScalarType::F32) => Some(32),
        Type::Scalar(ScalarType::F64) => Some(64),
        _ => None,
    }
}

fn unsupported() -> Error {
    Error::Statement("actual floating byte operation is not modeled")
}

impl FloatByteOperationV52 {
    pub(super) fn derive(
        inventory: &Inventory<'_>,
        operation: usize,
        out: &mut Writer<'_, '_>,
    ) -> Result<Option<Self>> {
        out.budget.charge_work(4)?;
        let row = inventory.operations().get(operation).ok_or_else(mismatch)?;
        let result_type = row.operation.results.first().map(|value| &value.ty);
        let uses = inventory
            .uses()
            .get(row.operands.clone())
            .ok_or_else(mismatch)?;
        let first_type = uses
            .first()
            .and_then(|usage| inventory.definitions().get(usage.definition))
            .map(|definition| definition.ty);
        let Some(input_bits) = first_type
            .and_then(float_width)
            .or_else(|| result_type.and_then(float_width))
        else {
            return Ok(None);
        };
        let (action, values, arity): (Action, [Option<ValueId>; 3], usize) =
            match &row.operation.kind {
                OperationKind::Constant(Constant::F32Bits(bits)) if input_bits == 32 => {
                    (Action::Constant(u64::from(*bits)), [None; 3], 0)
                }
                OperationKind::Constant(Constant::F64Bits(bits)) if input_bits == 64 => {
                    (Action::Constant(*bits), [None; 3], 0)
                }
                OperationKind::Unary {
                    op: UnaryOp::Negate,
                    operand,
                } => (Action::Operator(10), [Some(*operand), None, None], 1),
                OperationKind::Binary { op, lhs, rhs } => {
                    let code = match op {
                        BinaryOp::Add => 11,
                        BinaryOp::Subtract => 12,
                        BinaryOp::Multiply => 13,
                        BinaryOp::Divide => 14,
                        BinaryOp::Remainder => 15,
                        _ => return Err(unsupported()),
                    };
                    (Action::Operator(code), [Some(*lhs), Some(*rhs), None], 2)
                }
                OperationKind::Compare {
                    predicate,
                    lhs,
                    rhs,
                } => {
                    let code = match predicate {
                        ComparePredicate::Equal => 16,
                        ComparePredicate::NotEqual => 17,
                        ComparePredicate::LessThan => 18,
                        ComparePredicate::LessThanOrEqual => 19,
                        ComparePredicate::GreaterThan => 20,
                        ComparePredicate::GreaterThanOrEqual => 21,
                    };
                    (Action::Operator(code), [Some(*lhs), Some(*rhs), None], 2)
                }
                OperationKind::Select {
                    condition,
                    true_value,
                    false_value,
                } => (
                    Action::Select,
                    [Some(*condition), Some(*true_value), Some(*false_value)],
                    3,
                ),
                _ => return Ok(None),
            };
        out.budget.charge_work(
            16usize
                .checked_add(arity.checked_mul(14).ok_or(Resource::Arithmetic)?)
                .ok_or(Resource::Arithmetic)?,
        )?;
        if uses.len() != arity
            || row.results.len() != 1
            || row.operation.results.len() != 1
            || !row.effects.is_empty()
        {
            return Err(mismatch());
        }
        let function = inventory
            .functions()
            .get(row.coordinate.block.function.0 as usize)
            .ok_or_else(mismatch)?;
        let result = inventory
            .definitions()
            .get(row.results.start)
            .ok_or_else(mismatch)?;
        let output_bits = if matches!(action, Action::Operator(16..=21)) {
            1
        } else {
            input_bits
        };
        if !function.operations.contains(&operation)
            || !function.definitions.contains(&row.results.start)
            || result.coordinate
                != (Definition::Result {
                    operation: row.coordinate,
                    result: 0,
                })
            || result.value != Some(row.operation.results[0].id)
            || result.ty != &row.operation.results[0].ty
            || if output_bits == 1 {
                result.ty != &Type::Scalar(ScalarType::Bool)
            } else {
                float_width(result.ty) != Some(output_bits)
            }
        {
            return Err(mismatch());
        }
        let mut inputs = [0; 3];
        for (ordinal, usage) in uses.iter().enumerate() {
            let definition = inventory
                .definitions()
                .get(usage.definition)
                .ok_or_else(mismatch)?;
            if usage.coordinate
                != (Use::OperationOperand {
                    operation: row.coordinate,
                    operand: ordinal as u32,
                })
                || Some(usage.value) != values[ordinal]
                || definition.value != Some(usage.value)
                || !function.definitions.contains(&usage.definition)
                || if action == Action::Select && ordinal == 0 {
                    definition.ty != &Type::Scalar(ScalarType::Bool)
                } else {
                    float_width(definition.ty) != Some(input_bits)
                }
            {
                return Err(mismatch());
            }
            inputs[ordinal] = usage.definition;
        }
        Ok(Some(Self {
            action,
            inputs,
            arity,
            result: row.results.start,
            input_bits,
            output_bits,
        }))
    }

    pub(super) fn emit_step(
        self,
        before: ByteMemoryStateNamesV30<'_>,
        after: ByteMemoryStateNamesV30<'_>,
        out: &mut Writer<'_, '_>,
    ) -> Result<()> {
        out.budget.charge_work(12 + self.arity)?;
        emit!(
            out,
            " let float_inputs_ok = {} && {}.len() > {}",
            before.valid,
            before.values,
            self.result
        );
        for (ordinal, input) in self.inputs[..self.arity].iter().enumerate() {
            emit!(out, " && {}.len() > {input}", before.values);
            if self.action == Action::Select && ordinal == 0 {
                emit!(
                    out,
                    " && (match {}[{input}] {{ MemoryValueV30::Scalar(v) => v == 0int || v == 1int, _ => false }})",
                    before.values
                );
            } else {
                emit!(
                    out,
                    " && byte_float_operand_v52({}[{input}], {})",
                    before.values,
                    self.input_bits
                );
            }
        }
        emit!(
            out,
            ";\n let float_value = if !float_inputs_ok {{ MemoryValueV30::Undefined }} else {{ "
        );
        match self.action {
            Action::Constant(bits) => emit!(out, "MemoryValueV30::Scalar({bits}int)"),
            Action::Select => emit!(
                out,
                "if {}[{}] == MemoryValueV30::Scalar(1int) {{ {}[{}] }} else {{ {}[{}] }}",
                before.values,
                self.inputs[0],
                before.values,
                self.inputs[1],
                before.values,
                self.inputs[2]
            ),
            Action::Operator(code) => {
                emit!(
                    out,
                    "byte_float_value_v52({}, {code}int, {}int, {}int, {}[{}], ",
                    before.frames,
                    self.input_bits,
                    self.output_bits,
                    before.values,
                    self.inputs[0]
                );
                if self.arity == 1 {
                    emit!(out, "MemoryValueV30::Undefined");
                } else {
                    emit!(out, "{}[{}]", before.values, self.inputs[1]);
                }
                emit!(out, ")");
            }
        }
        emit!(
            out,
            " }};\n let {} = float_inputs_ok && !matches!(float_value, MemoryValueV30::Undefined);\n let {} = if {} {{ {}.update({}int, float_value) }} else {{ {} }};\n let {} = {};\n let {} = {};\n let {} = {};\n",
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
    size_of::<FloatByteOperationV52>()
        + size_of::<Result<Option<FloatByteOperationV52>>>()
        + size_of::<(Action, [Option<ValueId>; 3], [usize; 3], u32, u32, usize)>()
        + size_of::<(
            ByteMemoryStateNamesV30<'static>,
            ByteMemoryStateNamesV30<'static>,
        )>()
        + size_of::<([usize; 8], [&(); 8], [Result<()>; 2], std::fmt::Result)>()
}
