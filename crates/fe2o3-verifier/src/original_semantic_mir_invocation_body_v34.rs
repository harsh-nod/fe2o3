//! All active original bodies share the exact source-owned invocation roster.
//! Logical local ranges are not an allocation namespace. This source model
//! does not discharge actual call splicing or any memory interpretation.

use super::{
    Error, Resource, Result, Writer,
    call_transfers::{CallTransfers, Transfer},
    control::SourceControl,
    vector,
};
use std::{mem::size_of, ops::Range};

#[path = "original_semantic_mir_invocation_generate_v34.rs"]
mod generate;

#[path = "original_semantic_mir_invocation_actual_v35.rs"]
mod actual;

pub(super) struct Body {
    root: usize,
    instance: usize,
    locals: Range<usize>,
    blocks: Range<usize>,
    returned: Option<usize>,
    control: SourceControl,
}

pub(super) struct InvocationBodies<'a, 'plan, 'view, 'source> {
    transfers: &'a CallTransfers<'plan, 'view, 'source>,
    roots: Vec<Range<usize>>,
    bodies: Vec<Option<Body>>,
    locals: usize,
    required: usize,
}

fn mismatch() -> Error {
    Error::Statement("original MIR body differs from its exact invocation transfer")
}

impl<'a, 'plan, 'view, 'source> InvocationBodies<'a, 'plan, 'view, 'source> {
    pub(super) fn derive(
        transfers: &'a CallTransfers<'plan, 'view, 'source>,
        out: &mut Writer<'_, '_>,
    ) -> Result<Self> {
        let plan = transfers.plan(out)?;
        let source = plan.source(out)?;
        out.budget.reserve_storage(headers())?;
        let semantic = source.source_semantic(out.budget)?;
        let ssa = source.source_ssa(out.budget)?;
        let root_count = source.root_count(out.budget)?;
        let mut count = 0usize;
        for root in 0..root_count {
            out.budget.charge_work(1)?;
            count = count
                .checked_add(plan.root(root, out)?.instances.len())
                .ok_or(Resource::Arithmetic)?;
        }
        let mut roots = vector(root_count, out)?;
        let mut bodies = vector(count, out)?;
        let mut locals = 0;
        for root in 0..root_count {
            let scope = plan.root(root, out)?;
            if scope.instances.start != bodies.len() {
                return Err(mismatch());
            }
            roots.push(scope.instances.clone());
            for instance in 0..scope.instances.len() {
                out.budget.charge_work(5)?;
                let row = plan.instance(root, instance, out)?;
                if row.locals.start != locals || row.locals.start > row.locals.end {
                    return Err(mismatch());
                }
                locals = row.locals.end;
                if !row.active {
                    bodies.push(None);
                    continue;
                }
                let function = semantic
                    .functions()
                    .get(row.function.index() as usize)
                    .ok_or_else(mismatch)?;
                let original_plan = ssa
                    .plan_for_function(row.function)
                    .ok_or_else(mismatch)?
                    .plan();
                let calls = transfers.context(root, instance, out)?;
                let control = SourceControl::derive_instance(
                    semantic.types(),
                    function,
                    original_plan,
                    &calls,
                    out,
                )?;
                if control.locals != row.locals.len() || control.blocks.len() != row.blocks.len() {
                    return Err(mismatch());
                }
                let returned = match row.incoming {
                    None if instance == 0 => None,
                    Some((parent, site)) if parent < instance => {
                        let parent = transfers.context(root, parent, out)?;
                        let (index, transfer) = parent.call(site.index() as usize, out)?;
                        if transfer.child != instance
                            || transfer.child_locals != row.locals
                            || transfer.entry
                                != row
                                    .blocks
                                    .start
                                    .checked_add(control.entry.get() as usize)
                                    .ok_or(Resource::Arithmetic)?
                        {
                            return Err(mismatch());
                        }
                        // The independently read callee Return roster must be
                        // exactly the one retained by the source transfer.
                        let mut cursor = 0;
                        for (block, declaration) in function.blocks().iter().enumerate() {
                            out.budget.charge_work(2)?;
                            if matches!(declaration.terminator().kind(), super::Terminator::Return)
                            {
                                let pc = row
                                    .blocks
                                    .start
                                    .checked_add(block)
                                    .ok_or(Resource::Arithmetic)?;
                                if transfer.return_blocks.get(cursor) != Some(&pc) {
                                    return Err(mismatch());
                                }
                                cursor += 1;
                            }
                        }
                        if cursor != transfer.return_blocks.len() {
                            return Err(mismatch());
                        }
                        Some(index)
                    }
                    _ => return Err(mismatch()),
                };
                bodies.push(Some(Body {
                    root,
                    instance,
                    locals: row.locals.clone(),
                    blocks: row.blocks.clone(),
                    returned,
                    control,
                }));
            }
        }
        if bodies.len() != count {
            return Err(mismatch());
        }
        Ok(Self {
            transfers,
            roots,
            bodies,
            locals,
            required: out.budget.storage(),
        })
    }

    fn check(&self, out: &Writer<'_, '_>) -> Result<()> {
        let plan = self.transfers.plan(out)?;
        if out.budget.storage() < self.required {
            return Err(plan
                .source(out)?
                .retain_query_resource_error_v18(Resource::Accounting)
                .into());
        }
        Ok(())
    }

    fn transfer(
        &self,
        index: usize,
        out: &Writer<'_, '_>,
    ) -> Result<&super::call_transfers::DirectTransfer> {
        self.check(out)?;
        match self
            .transfers
            .rows(out)?
            .get(index)
            .map(|row| &row.transfer)
        {
            Some(Transfer::Direct(transfer)) => Ok(transfer),
            _ => Err(mismatch()),
        }
    }

    pub(super) fn emit_steps(&self, out: &mut Writer<'_, '_>) -> Result<()> {
        self.check(out)?;
        generate::emit_steps(self, out)
    }
}

fn headers() -> usize {
    fn h<T>() -> usize {
        size_of::<T>() + 2 * size_of::<Result<T>>()
    }
    h::<InvocationBodies<'_, '_, '_, '_>>()
        + h::<Body>()
        + h::<Vec<Option<Body>>>()
        + h::<Vec<Range<usize>>>()
        + h::<SourceControl>()
        + h::<Range<usize>>()
        + h::<Option<usize>>()
        + h::<&super::invocations::Instance>()
        + h::<&super::invocations::Root>()
        + h::<&super::invocations::InvocationPlan<'_, '_>>()
        + h::<&super::call_transfers::DirectTransfer>()
        + h::<&[super::call_transfers::CallRow]>()
        + h::<&fe2o3_lower_mir_kernel::ProductionSourceOwnedViewV18<'_>>()
        + h::<&fe2o3_mir_model::semantic_mir_v1::AdmittedInertSemanticMirV1>()
        + h::<&fe2o3_pliron::ProductionSemanticSsaOwnerV1>()
        + h::<(&CallTransfers<'_, '_, '_>, &mut Writer<'_, '_>)>()
        + 12 * size_of::<usize>()
        + 8 * size_of::<&()>()
}

#[cfg(test)]
#[path = "original_semantic_mir_invocation_body_v34_tests.rs"]
mod tests;
