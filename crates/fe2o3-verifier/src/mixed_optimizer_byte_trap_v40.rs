//! A registered terminal trap is an observable outcome, not model refusal.
use super::*;
use fe2o3_kernel_ir::AmdGpuDiagnosticOperation as Diagnostic;

pub(super) struct TrapByteOperationV40;

impl TrapByteOperationV40 {
    pub(super) fn derive(
        inventory: &Inventory<'_>,
        operation: usize,
        out: &mut Writer<'_, '_>,
    ) -> Result<Option<Self>> {
        let row = inventory.operations().get(operation).ok_or_else(mismatch)?;
        let OperationKind::Call { callee, arguments } = &row.operation.kind else {
            return Ok(None);
        };
        // Empty arguments reject allocating Print descriptors before
        // materialization. The canonical API owns the priced roster census.
        out.budget.charge_work(
            Diagnostic::intrinsic_descriptor_lookup_work_v1(callee)
                .and_then(|n| n.checked_add(9))
                .ok_or(Resource::Arithmetic)?,
        )?;
        if !arguments.is_empty()
            || !matches!(
                Diagnostic::from_intrinsic_call(callee, arguments),
                Some(Diagnostic::Trap)
            )
        {
            return Ok(None);
        }
        let block = block_index(inventory, row.coordinate.block)?;
        let block = &inventory.blocks()[block];
        if block.operations.end.checked_sub(1) != Some(operation)
            || !matches!(block.terminator, Terminator::Unreachable)
            || !row.operation.results.is_empty()
            || !row.operands.is_empty()
            || !row.effects.is_empty()
        {
            return Err(Error::Statement(
                "actual byte trap must end an unreachable block",
            ));
        }
        Ok(Some(Self))
    }

    pub(super) fn emit_step(
        &self,
        before: ByteMemoryStateNamesV30<'_>,
        after: ByteMemoryStateNamesV30<'_>,
        out: &mut Writer<'_, '_>,
    ) -> Result<()> {
        out.budget.charge_work(1)?;
        emit!(
            out,
            " let {} = {}; let {} = {}; let {} = {}; let {} = {}; let {} = {};\n",
            after.values,
            before.values,
            after.memory,
            before.memory,
            after.generations,
            before.generations,
            after.frames,
            before.frames,
            after.valid,
            before.valid
        );
        Ok(())
    }
}

pub(super) fn headers() -> usize {
    2 * size_of::<TrapByteOperationV40>()
        + 2 * size_of::<Result<Option<TrapByteOperationV40>>>()
        + 2 * size_of::<ByteMemoryStateNamesV30<'_>>()
        + size_of::<(Option<Diagnostic>, [&(); 5], [usize; 5], Result<usize>)>()
        + size_of::<([&str; 3], [usize; 4], Option<usize>, Result<()>)>()
}
