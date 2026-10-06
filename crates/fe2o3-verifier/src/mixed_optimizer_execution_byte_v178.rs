//! Actual capability operations. Dynamic authority stays at each defining SSA
//! slot; a copied capability record cannot authorize a different definition.
use super::*;
use fe2o3_kernel_ir::{ExecutionOperationV15 as Execution, ExecutionRoleV15 as Role};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct ExecutionByteOperationV178 {
    site: Operation,
    code: u8,
    destination: Option<usize>,
    receiver: Option<usize>,
}

fn unsupported() -> Error {
    Error::Statement("actual execution byte operation or capability transport is not modeled")
}

pub(super) fn role_code(role: Role) -> Result<u8> {
    match role {
        Role::Context => Ok(0),
        Role::Workgroup => Ok(1),
        Role::MaskedTileU32 { .. } | Role::LaneFragmentU32 { .. } => Err(unsupported()),
    }
}

impl ExecutionByteOperationV178 {
    pub(super) fn derive(
        inventory: &Inventory<'_>,
        operation: usize,
        out: &mut Writer<'_, '_>,
    ) -> Result<Option<Self>> {
        out.budget.charge_work(2)?;
        let row = inventory.operations().get(operation).ok_or_else(mismatch)?;
        let OperationKind::Execution(execution) = &row.operation.kind else {
            return Ok(None);
        };
        out.budget.charge_work(12)?;
        let (code, result_role, receiver_role, receiver_value) = match execution {
            Execution::ContextIssue => (0, Some(Role::Context), None, None),
            Execution::WorkgroupDerive { context } => (
                1,
                Some(Role::Workgroup),
                Some(Role::Context),
                Some(*context),
            ),
            Execution::ScopeEnd {
                workgroup,
                discarded,
            } if discarded.is_empty() => (2, None, Some(Role::Workgroup), Some(*workgroup)),
            _ => return Err(unsupported()),
        };
        if !row.effects.is_empty()
            || row.results.len() != usize::from(result_role.is_some())
            || row.operands.len() != usize::from(receiver_role.is_some())
        {
            return Err(unsupported());
        }
        let destination = if let Some(role) = result_role {
            let definition = inventory
                .definitions()
                .get(row.results.start)
                .ok_or_else(mismatch)?;
            if definition.ty != &Type::Execution(role) {
                return Err(unsupported());
            }
            Some(row.results.start)
        } else {
            None
        };
        let receiver = if let Some(role) = receiver_role {
            let operand = inventory
                .uses()
                .get(row.operands.start)
                .ok_or_else(mismatch)?;
            let definition = inventory
                .definitions()
                .get(operand.definition)
                .ok_or_else(mismatch)?;
            if definition.ty != &Type::Execution(role) || definition.value != receiver_value {
                return Err(unsupported());
            }
            Some(operand.definition)
        } else {
            None
        };
        if destination.is_some() && destination == receiver {
            return Err(unsupported());
        }
        Ok(Some(Self {
            site: row.coordinate,
            code,
            destination,
            receiver,
        }))
    }

    pub(super) fn emit_step(
        &self,
        after: ByteMemoryStateNamesV30<'_>,
        out: &mut Writer<'_, '_>,
    ) -> Result<()> {
        out.budget.charge_work(8)?;
        emit!(
            out,
            " let execution_next = byte_execution_step_v178(s, operation, {}, ",
            self.code
        );
        for (index, value) in [self.destination, self.receiver].into_iter().enumerate() {
            if index != 0 {
                emit!(out, ", ");
            }
            if let Some(value) = value {
                emit!(out, "{value}");
            } else {
                emit!(out, "-1");
            }
        }
        emit!(
            out,
            ");\n let {} = execution_next.values; let {} = execution_next.memory; let {} = execution_next.generations; let {} = execution_next.frames; let {} = execution_next.valid;\n",
            after.values,
            after.memory,
            after.generations,
            after.frames,
            after.valid
        );
        Ok(())
    }
}

pub(super) fn headers() -> usize {
    2 * size_of::<ExecutionByteOperationV178>()
        + 2 * size_of::<Result<Option<ExecutionByteOperationV178>>>()
        + size_of::<(u8, [Option<Role>; 2], Option<fe2o3_kernel_ir::ValueId>)>()
        + size_of::<([usize; 6], [Option<usize>; 4], [Result<()>; 3])>()
}
