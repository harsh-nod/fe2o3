//! Original direct-call argument and return transfers over logical MIR locals.
//! This does not interpret the callee body, identify memory cells, or certify
//! an actual call splice. Those remain mandatory whole-program relations.

use super::{
    Error, ExpressionV30, LocalRole, NodeV30, Operand, Resource, Result, ScalarV30,
    SourceProgramV30, Writer,
    invocations::{CallKind, CallSite, Instance, InvocationPlan},
    vector,
};
use fe2o3_lower_mir_kernel::ProductionSourceOwnedViewV18 as Source;
use fe2o3_mir_model::semantic_mir_v1::{
    SemanticCallableDeclV1 as Callable, SemanticDirectCallV1 as Call,
    SemanticEdgeRoleV1 as EdgeRole, SemanticFunctionDeclV1 as Function,
    SemanticTerminatorKindV1 as Terminator, SemanticTypeDeclV1 as Type,
    SemanticUnwindActionV1 as Unwind,
};
use std::{mem::size_of, ops::Range};

pub(super) struct ReturnTransfer {
    pub continuation: usize,
    pub destination: usize,
    pub source: Option<usize>,
    pub scalar: ScalarV30,
}

pub(super) struct DirectTransfer {
    pub child: usize,
    pub site: usize,
    pub entry: usize,
    /// A fresh invocation clears this logical frame before installing inputs.
    pub child_locals: Range<usize>,
    pub caller_locals: Range<usize>,
    /// Expressions use caller-local indices, evaluated before any move kills.
    pub operands: SourceProgramV30,
    /// Original argument order, each paired with its exact callee local slot.
    pub arguments: Vec<(usize, usize)>,
    pub reads: Vec<usize>,
    pub moved: Vec<usize>,
    /// Exact original Return terminators, never selected from a result value.
    pub return_blocks: Vec<usize>,
    pub returned: ReturnTransfer,
}

pub(super) enum Transfer {
    /// Still retained in the source census. This is not an executable no-op;
    /// the complete control relation must independently exclude its block.
    Inactive,
    Direct(DirectTransfer),
}

pub(super) struct CallRow {
    pub root: usize,
    pub site: CallSite,
    pub transfer: Transfer,
}

pub(super) struct CallTransfers<'plan, 'view, 'source> {
    plan: &'plan InvocationPlan<'view, 'source>,
    rows: Vec<CallRow>,
    required: usize,
}

/// Non-constructible source-instance view for the original body interpreter.
pub(super) struct CallContext<'a, 'plan, 'view, 'source> {
    owner: &'a CallTransfers<'plan, 'view, 'source>,
    root: usize,
    instance: usize,
}

impl<'a, 'plan, 'view, 'source> CallContext<'a, 'plan, 'view, 'source> {
    pub(super) fn check_original(
        &self,
        types: &[Type],
        function: &Function,
        plan: &fe2o3_mir_model::SsaConstructionPlanV1,
        out: &mut Writer<'_, '_>,
    ) -> Result<()> {
        self.owner.rows(out)?;
        let source = self.owner.plan.source(out)?;
        let instance = self.owner.plan.instance(self.root, self.instance, out)?;
        let semantic = source.source_semantic(out.budget)?;
        let ssa = source.source_ssa(out.budget)?;
        out.budget.charge_work(4)?;
        if !instance.active
            || !std::ptr::eq(types, semantic.types())
            || !semantic
                .functions()
                .get(instance.function.index() as usize)
                .is_some_and(|original| std::ptr::eq(function, original))
            || !ssa
                .plan_for_function(instance.function)
                .is_some_and(|original| std::ptr::eq(plan, original.plan()))
        {
            return Err(mismatch());
        }
        Ok(())
    }

    pub(super) fn instance(&self, out: &Writer<'_, '_>) -> Result<&Instance> {
        self.owner.rows(out)?;
        self.owner.plan.instance(self.root, self.instance, out)
    }

    pub(super) fn call(
        &self,
        block: usize,
        out: &mut Writer<'_, '_>,
    ) -> Result<(usize, &'a DirectTransfer)> {
        let rows = self.owner.rows(out)?;
        let log = if rows.is_empty() {
            0
        } else {
            rows.len().ilog2() as usize + 1
        };
        out.budget
            .charge_work(log.checked_add(4).ok_or(Resource::Arithmetic)?)?;
        let index = rows
            .binary_search_by_key(&(self.root, self.instance, block), |row| {
                (row.root, row.site.caller, row.site.block.index() as usize)
            })
            .map_err(|_| mismatch())?;
        match &rows[index].transfer {
            Transfer::Direct(transfer) => Ok((index, transfer)),
            Transfer::Inactive => Err(mismatch()),
        }
    }

    pub(super) fn transfer(
        &self,
        index: usize,
        out: &Writer<'_, '_>,
    ) -> Result<&'a DirectTransfer> {
        let row = self.owner.rows(out)?.get(index).ok_or_else(mismatch)?;
        if row.root != self.root || row.site.caller != self.instance {
            return Err(mismatch());
        }
        match &row.transfer {
            Transfer::Direct(transfer) => Ok(transfer),
            Transfer::Inactive => Err(mismatch()),
        }
    }
}

fn mismatch() -> Error {
    Error::Statement("original MIR call transfer differs from its exact invocation")
}

impl<'plan, 'view, 'source> CallTransfers<'plan, 'view, 'source> {
    pub(super) fn derive(
        plan: &'plan InvocationPlan<'view, 'source>,
        out: &mut Writer<'_, '_>,
    ) -> Result<Self> {
        let source = plan.source(out)?;
        out.budget.reserve_storage(headers())?;
        let semantic = source.source_semantic(out.budget)?;
        let roots = source.root_count(out.budget)?;
        let mut count = 0usize;
        for root in 0..roots {
            out.budget.charge_work(1)?;
            let instances = plan.root(root, out)?.instances.len();
            for instance in 0..instances {
                out.budget.charge_work(1)?;
                count = count
                    .checked_add(plan.calls(root, instance, out)?.len())
                    .ok_or(Resource::Arithmetic)?;
            }
        }
        let mut rows = vector(count, out)?;
        for root in 0..roots {
            out.budget.charge_work(1)?;
            let instances = plan.root(root, out)?.instances.len();
            for instance in 0..instances {
                out.budget.charge_work(2)?;
                let caller = plan.instance(root, instance, out)?;
                let declaration = semantic
                    .functions()
                    .get(caller.function.index() as usize)
                    .ok_or_else(mismatch)?;
                for &site in plan.calls(root, instance, out)? {
                    out.budget.charge_work(5)?;
                    if site.caller != instance {
                        return Err(mismatch());
                    }
                    let transfer = if !caller.active || !site.ssa_reachable {
                        if let Some(child) = site.child {
                            if plan.instance(root, child, out)?.active {
                                return Err(mismatch());
                            }
                        }
                        Transfer::Inactive
                    } else {
                        let Terminator::Call(call) = declaration
                            .blocks()
                            .get(site.block.index() as usize)
                            .ok_or_else(mismatch)?
                            .terminator()
                            .kind()
                        else {
                            return Err(Error::Statement(
                                "original MIR tail/drop call transfer is not modeled",
                            ));
                        };
                        let child = site.child.ok_or(Error::Statement(
                            "original MIR intrinsic or missing call body is not modeled",
                        ))?;
                        let callee = plan.instance(root, child, out)?;
                        if site.kind != CallKind::Direct
                            || call.callee() != site.callable
                            || !callee.active
                            || callee.incoming != Some((instance, site.block))
                            || !matches!(semantic.callables().get(site.callable.index() as usize),
                                Some(Callable::Defined { function }) if *function == callee.function)
                        {
                            return Err(mismatch());
                        }
                        let body = semantic
                            .functions()
                            .get(callee.function.index() as usize)
                            .ok_or_else(mismatch)?;
                        Transfer::Direct(direct(
                            semantic.types(),
                            declaration,
                            caller,
                            body,
                            callee,
                            child,
                            site.block.index() as usize,
                            call,
                            out,
                        )?)
                    };
                    rows.push(CallRow {
                        root,
                        site,
                        transfer,
                    });
                }
            }
        }
        if rows.len() != count {
            return Err(mismatch());
        }
        Ok(Self {
            plan,
            rows,
            required: out.budget.storage(),
        })
    }

    pub(super) fn rows(&self, out: &Writer<'_, '_>) -> Result<&[CallRow]> {
        let source = self.plan.source(out)?;
        if out.budget.storage() < self.required {
            return Err(Error::Source(
                source.retain_query_resource_error_v18(Resource::Accounting),
            ));
        }
        Ok(&self.rows)
    }

    pub(super) fn emit_steps(&self, out: &mut Writer<'_, '_>) -> Result<()> {
        generate::emit_steps(self, out)
    }

    pub(super) fn plan(
        &self,
        out: &Writer<'_, '_>,
    ) -> Result<&'plan InvocationPlan<'view, 'source>> {
        self.rows(out)?;
        Ok(self.plan)
    }

    pub(super) fn context<'a>(
        &'a self,
        root: usize,
        instance: usize,
        out: &mut Writer<'_, '_>,
    ) -> Result<CallContext<'a, 'plan, 'view, 'source>> {
        self.rows(out)?;
        if !self.plan.instance(root, instance, out)?.active {
            return Err(mismatch());
        }
        out.budget.reserve_storage(context_headers())?;
        Ok(CallContext {
            owner: self,
            root,
            instance,
        })
    }
}

pub(super) fn context_headers() -> usize {
    type Frame<'a> = (
        CallContext<'a, 'a, 'a, 'a>,
        Result<CallContext<'a, 'a, 'a, 'a>>,
        Result<&'a Instance>,
        Result<(usize, &'a DirectTransfer)>,
        Result<&'a DirectTransfer>,
        Result<&'a [CallRow]>,
        Result<()>,
        &'a CallTransfers<'a, 'a, 'a>,
        &'a CallContext<'a, 'a, 'a, 'a>,
        &'a [Type],
        &'a Function,
        &'a fe2o3_mir_model::SsaConstructionPlanV1,
        &'a mut Writer<'a, 'a>,
        [usize; 8],
    );
    size_of::<Frame<'_>>() + std::mem::align_of::<Frame<'_>>()
}

fn direct(
    types: &[Type],
    caller: &Function,
    caller_instance: &Instance,
    callee: &Function,
    callee_instance: &Instance,
    child: usize,
    block: usize,
    call: &Call,
    out: &mut Writer<'_, '_>,
) -> Result<DirectTransfer> {
    out.budget.charge_work(8)?;
    if call.unwind() != Unwind::Unreachable || !call.variadic_argument_abis().is_empty() {
        return Err(Error::Statement(
            "original MIR variadic or exceptional call transfer is not modeled",
        ));
    }
    let inputs = callee.abi().source_input_types();
    if inputs.len() != call.arguments().len()
        || caller_instance.locals.len() != caller.locals().len()
        || callee_instance.locals.len() != callee.locals().len()
        || caller_instance.blocks.len() != caller.blocks().len()
        || callee_instance.blocks.len() != callee.blocks().len()
        || block >= caller.blocks().len()
        || callee.entry().index() as usize >= callee.blocks().len()
    {
        return Err(mismatch());
    }
    let destination = call.destination().ok_or(Error::Statement(
        "original MIR nonreturning call transfer is not modeled",
    ))?;
    if destination.edge().role() != EdgeRole::CallReturn
        || destination.edge().target().index() as usize >= caller.blocks().len()
        || destination.place().ty() != callee.abi().return_type()
    {
        return Err(mismatch());
    }
    let capacity = caller
        .locals()
        .len()
        .checked_add(inputs.len())
        .ok_or(Resource::Arithmetic)?;
    let mut operands = SourceProgramV30 {
        nodes: vector(capacity, out)?,
        assignments: Vec::new(),
        arguments: caller.locals().len(),
        returned: None,
        statements: 0,
        locals: vector(caller.locals().len(), out)?,
    };
    let mut initialized = vector(caller.locals().len(), out)?;
    out.budget.charge_work(caller.locals().len())?;
    operands.locals.resize(caller.locals().len(), None);
    out.budget.charge_work(caller.locals().len())?;
    initialized.resize(caller.locals().len(), false);
    let mut parameters = vector(inputs.len(), out)?;
    out.budget.charge_work(inputs.len())?;
    parameters.resize(inputs.len(), None);
    let mut returned = None;
    for (local, declaration) in callee.locals().iter().enumerate() {
        out.budget.charge_work(3)?;
        match declaration.role() {
            LocalRole::Argument(argument) => {
                let slot = parameters.get_mut(argument as usize).ok_or_else(mismatch)?;
                if inputs.get(argument as usize) != Some(&declaration.ty())
                    || slot.replace(local).is_some()
                {
                    return Err(mismatch());
                }
            }
            LocalRole::Return => {
                if declaration.ty() != callee.abi().return_type()
                    || returned.replace(local).is_some()
                {
                    return Err(mismatch());
                }
            }
            _ => {}
        }
    }
    let mut arguments = vector(inputs.len(), out)?;
    for (argument, operand) in call.arguments().iter().enumerate() {
        out.budget.charge_work(4)?;
        if inputs[argument] != operand.ty() {
            return Err(mismatch());
        }
        if let Operand::Copy(place) | Operand::Move(place) = operand {
            let local = operands.local(caller, place)?;
            if !initialized[local] {
                let scalar = ScalarV30::from_source(types, place.ty())?;
                let node = operands.push(
                    NodeV30 {
                        scalar,
                        expression: ExpressionV30::Argument(
                            u32::try_from(local).map_err(|_| Resource::Arithmetic)?,
                        ),
                    },
                    out,
                )?;
                operands.locals[local] = Some(node);
                initialized[local] = true;
            }
        }
        // Reuse the original operand interpreter, including ordered move kills.
        let expression = operands.operand(types, caller, operand, out)?;
        let local = parameters[argument].ok_or_else(mismatch)?;
        let target = callee_instance
            .locals
            .start
            .checked_add(local)
            .ok_or(Resource::Arithmetic)?;
        arguments.push((target, expression));
    }
    let mut reads = vector(caller.locals().len(), out)?;
    let mut moved = vector(caller.locals().len(), out)?;
    for (local, &read) in initialized.iter().enumerate() {
        out.budget.charge_work(2)?;
        if read {
            let global = caller_instance
                .locals
                .start
                .checked_add(local)
                .ok_or(Resource::Arithmetic)?;
            reads.push(global);
            if operands.locals[local].is_none() {
                moved.push(global);
            }
        }
    }
    let local = operands.local(caller, destination.place())?;
    let scalar = ScalarV30::from_source(types, callee.abi().return_type())?;
    let source = if scalar == ScalarV30::Unit {
        None
    } else {
        Some(
            callee_instance
                .locals
                .start
                .checked_add(returned.ok_or_else(mismatch)?)
                .ok_or(Resource::Arithmetic)?,
        )
    };
    let mut return_blocks = vector(callee.blocks().len(), out)?;
    for (ordinal, block) in callee.blocks().iter().enumerate() {
        out.budget.charge_work(1)?;
        if matches!(block.terminator().kind(), Terminator::Return) {
            return_blocks.push(
                callee_instance
                    .blocks
                    .start
                    .checked_add(ordinal)
                    .ok_or(Resource::Arithmetic)?,
            );
        }
    }
    Ok(DirectTransfer {
        child,
        site: caller_instance
            .blocks
            .start
            .checked_add(block)
            .ok_or(Resource::Arithmetic)?,
        entry: callee_instance
            .blocks
            .start
            .checked_add(callee.entry().index() as usize)
            .ok_or(Resource::Arithmetic)?,
        child_locals: callee_instance.locals.clone(),
        caller_locals: caller_instance.locals.clone(),
        operands,
        arguments,
        reads,
        moved,
        return_blocks,
        returned: ReturnTransfer {
            continuation: caller_instance
                .blocks
                .start
                .checked_add(destination.edge().target().index() as usize)
                .ok_or(Resource::Arithmetic)?,
            destination: caller_instance
                .locals
                .start
                .checked_add(local)
                .ok_or(Resource::Arithmetic)?,
            source,
            scalar,
        },
    })
}

fn headers() -> usize {
    fn vector<T>() -> usize {
        size_of::<Vec<T>>() + 2 * size_of::<Result<Vec<T>>>()
    }
    type Frame<'a> = (
        CallTransfers<'a, 'a, 'a>,
        CallRow,
        DirectTransfer,
        ReturnTransfer,
        Result<CallTransfers<'a, 'a, 'a>>,
        Result<DirectTransfer>,
        Result<&'a [CallRow]>,
        Result<&'a Source<'a>>,
        Vec<CallRow>,
        Vec<(usize, usize)>,
        [Vec<usize>; 3],
        Vec<Option<usize>>,
        Vec<bool>,
        SourceProgramV30,
        NodeV30,
        Option<usize>,
        Result<usize>,
        [usize; 16],
        [&'a Function; 2],
        [&'a Instance; 2],
        &'a Call,
        &'a [Type],
        &'a mut Writer<'a, 'a>,
    );
    size_of::<Frame<'_>>()
        + std::mem::align_of::<Frame<'_>>()
        + vector::<CallRow>()
        + vector::<NodeV30>()
        + vector::<Option<usize>>()
        + vector::<bool>()
        + vector::<(usize, usize)>()
        + 3 * vector::<usize>()
        + size_of::<Result<ScalarV30>>()
        + size_of::<std::slice::Iter<'_, CallSite>>()
        + size_of::<
            std::iter::Enumerate<
                std::slice::Iter<'_, fe2o3_mir_model::semantic_mir_v1::SemanticLocalDeclV1>,
            >,
        >()
        + size_of::<std::iter::Enumerate<std::slice::Iter<'_, Operand>>>()
        + size_of::<std::iter::Enumerate<std::slice::Iter<'_, bool>>>()
        + size_of::<
            std::iter::Enumerate<
                std::slice::Iter<'_, fe2o3_mir_model::semantic_mir_v1::SemanticBasicBlockV1>,
            >,
        >()
        + 4 * size_of::<Range<usize>>()
        + super::interpreter_headers_v31()
}

#[path = "original_semantic_mir_call_generate_v33.rs"]
mod generate;

#[cfg(test)]
#[path = "original_semantic_mir_call_transfers_v33_tests.rs"]
mod tests;
