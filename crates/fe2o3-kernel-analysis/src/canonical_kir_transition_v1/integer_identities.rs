//! Input-derived neutral-operand facts, independent of the Pliron rewrite.
use super::{Budget, Error, Literal, Result, State};
use fe2o3_kernel_ir::{BinaryOp, CheckedBinaryOperator, OperationKind, ScalarType};

#[derive(Clone, Copy)]
struct Identity {
    result: usize,
    operand: usize,
    checked: bool,
}

impl State<'_, '_, '_, '_> {
    fn integer_identity(
        &self,
        operation: usize,
        budget: &mut Budget<'_>,
    ) -> Result<Option<Identity>> {
        let input = self.input;
        let row = &input.operations()[operation];
        let OperationKind::Binary { op, .. } = row.operation.kind else {
            return Ok(None);
        };
        if !matches!(
            op,
            BinaryOp::Add
                | BinaryOp::Subtract
                | BinaryOp::Multiply
                | BinaryOp::BitAnd
                | BinaryOp::BitOr
                | BinaryOp::BitXor
                | BinaryOp::Checked(_)
        ) {
            return Ok(None);
        }
        budget.charge_work(1)?;
        let checked = matches!(op, BinaryOp::Checked(_));
        if row.operands.len() != 2 || row.results.len() != if checked { 2 } else { 1 } {
            return Err(Error::Rule("integer identity arity"));
        }
        let left = input.uses()[row.operands.start].definition;
        let right = input.uses()[row.operands.start + 1].definition;
        budget.charge_work(3 + usize::from(checked))?;
        let ty = input.definitions()[left].ty.as_scalar();
        let width = match ty {
            Some(ScalarType::I8 | ScalarType::U8) => 8,
            Some(ScalarType::I16 | ScalarType::U16) => 16,
            Some(ScalarType::I32 | ScalarType::U32) => 32,
            Some(ScalarType::I64 | ScalarType::U64) => 64,
            _ => return Ok(None),
        };
        if input.definitions()[right].ty.as_scalar() != ty
            || input.definitions()[row.results.start].ty.as_scalar() != ty
            || checked
                && input.definitions()[row.results.start + 1].ty.as_scalar()
                    != Some(ScalarType::Bool)
        {
            return Ok(None);
        }
        let left_literal = self.literal(left, budget)?;
        let right_literal = self.literal(right, budget)?;
        budget.charge_work(1)?;
        let neutral = match op {
            BinaryOp::Multiply | BinaryOp::Checked(CheckedBinaryOperator::Multiply) => 1,
            BinaryOp::BitAnd => (1u128 << width) - 1,
            _ => 0,
        };
        let is_neutral = |value: Option<Literal>| {
            value.is_some_and(|value| Some(value.ty) == ty && value.bits == neutral)
        };
        let operand = if is_neutral(right_literal) {
            Some(left)
        } else if !matches!(
            op,
            BinaryOp::Subtract | BinaryOp::Checked(CheckedBinaryOperator::Subtract)
        ) && is_neutral(left_literal)
        {
            Some(right)
        } else {
            None
        };
        Ok(operand.map(|operand| Identity {
            result: row.results.start,
            operand,
            checked,
        }))
    }

    pub(super) fn integer_identity_alias(
        &mut self,
        operation: usize,
        budget: &mut Budget<'_>,
    ) -> Result<bool> {
        let Some(identity) = self.integer_identity(operation, budget)? else {
            return Ok(false);
        };
        let mut changed = self.union(identity.result, identity.operand, budget)?;
        if identity.checked {
            changed |= self.publish_literal(
                identity.result + 1,
                Literal {
                    ty: ScalarType::Bool,
                    bits: 0,
                },
                budget,
            )?;
        }
        Ok(changed)
    }

    pub(super) fn total_integer_identity(
        &self,
        operation: usize,
        budget: &mut Budget<'_>,
    ) -> Result<bool> {
        Ok(self.integer_identity(operation, budget)?.is_some())
    }
}
