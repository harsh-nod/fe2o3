//! Complete retained original invocation coordinates, not call semantics.
//! Local/block ranges address logical source state, never physical memory cells.
use super::{Error, Resource, Result, Writer, vector};
use fe2o3_kernel_ir::CanonicalKernelIrWorkLedgerIdentityV1 as Ledger;
use fe2o3_lower_mir_kernel::ProductionSourceOwnedViewV18 as Source;
use fe2o3_mir_model::semantic_mir_v1::{
    SemanticBlockIdV1 as Block, SemanticCallableDeclV1 as Callable,
    SemanticCallableIdV1 as CallableId, SemanticFunctionIdV1 as FunctionId,
    SemanticTerminatorKindV1 as Terminator,
};
use std::{mem::size_of, ops::Range};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum CallKind {
    Direct,
    Tail,
    Drop,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct CallSite {
    pub caller: usize,
    pub block: Block,
    pub callable: CallableId,
    pub kind: CallKind,
    pub ssa_reachable: bool,
    /// Root-relative retained instance, never chosen from equal function types.
    /// None remains an unresolved interpretation obligation for a defined call.
    pub child: Option<usize>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct Instance {
    pub function: FunctionId,
    pub incoming: Option<(usize, Block)>,
    pub active: bool,
    pub locals: Range<usize>,
    pub blocks: Range<usize>,
    pub calls: Range<usize>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct Root {
    pub function: FunctionId,
    pub physical: usize,
    pub instances: Range<usize>,
}

pub(super) struct InvocationPlan<'view, 'source> {
    source: &'view Source<'source>,
    roots: Vec<Root>,
    instances: Vec<Instance>,
    calls: Vec<CallSite>,
    ledger: Ledger,
    slot: usize,
    required: usize,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct Incoming {
    caller: usize,
    block: Block,
    child: usize,
}

fn mismatch() -> Error {
    Error::Statement("original MIR invocation coordinates differ from their source calls")
}

impl<'view, 'source> InvocationPlan<'view, 'source> {
    pub(super) fn derive(source: &'view Source<'source>, out: &mut Writer<'_, '_>) -> Result<Self> {
        source.check_query_v18(out.budget)?;
        out.budget.reserve_storage(headers())?;
        let semantic = source.source_semantic(out.budget)?;
        let ssa = source.source_ssa(out.budget)?;
        let count = source.root_count(out.budget)?;
        if count == 0 || count != semantic.roots().len() {
            return Err(mismatch());
        }
        let mut counts = vector(count, out)?;
        let mut instance_count = 0usize;
        let mut call_capacity = 0usize;
        // Count exact original terminator slots before allocating any call rows.
        for root in 0..count {
            let length = source.instance_count(root, out.budget)?;
            if length == 0 {
                return Err(mismatch());
            }
            instance_count = instance_count
                .checked_add(length)
                .ok_or(Resource::Arithmetic)?;
            counts.push(length);
            for instance in 0..length {
                let (function, _) = source.instance(root, instance, out.budget)?;
                let declaration = semantic
                    .functions()
                    .get(function.index() as usize)
                    .ok_or_else(mismatch)?;
                out.budget.charge_work(declaration.blocks().len())?;
                for block in declaration.blocks() {
                    if call(block.terminator().kind()).is_some() {
                        call_capacity = call_capacity.checked_add(1).ok_or(Resource::Arithmetic)?;
                    }
                }
            }
        }
        let mut roots = vector(count, out)?;
        let mut instances = vector(instance_count, out)?;
        let mut calls = vector(call_capacity, out)?;
        let mut local_end = 0usize;
        let mut block_end = 0usize;
        for (root, &length) in counts.iter().enumerate() {
            let (function, physical) = source.root(root, out.budget)?;
            if semantic.roots().get(root) != Some(&function) {
                return Err(mismatch());
            }
            let first = instances.len();
            let mut incoming = vector(length - 1, out)?;
            for instance in 0..length {
                let (original, parent) = source.instance(root, instance, out.budget)?;
                let active = source.instance_active(root, instance, out.budget)?;
                out.budget.charge_work(4)?;
                if instance == 0 {
                    if original != function || parent.is_some() || !active {
                        return Err(mismatch());
                    }
                } else {
                    let (caller, block) = parent.ok_or_else(mismatch)?;
                    if caller >= instance {
                        return Err(mismatch());
                    }
                    incoming.push(Incoming {
                        caller,
                        block,
                        child: instance,
                    });
                }
                let declaration = semantic
                    .functions()
                    .get(original.index() as usize)
                    .ok_or_else(mismatch)?;
                let plan = ssa.plan_for_function(original).ok_or_else(mismatch)?.plan();
                if plan.resources().input_blocks() != declaration.blocks().len() {
                    return Err(mismatch());
                }
                let locals = local_end
                    ..local_end
                        .checked_add(declaration.locals().len())
                        .ok_or(Resource::Arithmetic)?;
                let blocks = block_end
                    ..block_end
                        .checked_add(declaration.blocks().len())
                        .ok_or(Resource::Arithmetic)?;
                local_end = locals.end;
                block_end = blocks.end;
                let call_start = calls.len();
                for (ordinal, block) in declaration.blocks().iter().enumerate() {
                    out.budget.charge_work(2)?;
                    let Some((kind, callable)) = call(block.terminator().kind()) else {
                        continue;
                    };
                    if semantic
                        .callables()
                        .get(callable.index() as usize)
                        .is_none()
                    {
                        return Err(mismatch());
                    }
                    let block = Block::from_index(
                        u32::try_from(ordinal).map_err(|_| Resource::Arithmetic)?,
                    );
                    calls.push(CallSite {
                        caller: instance,
                        block,
                        callable,
                        kind,
                        ssa_reachable: plan
                            .live_in(fe2o3_mir_model::SsaBlockIdV1::new(block.index()))
                            .is_some(),
                        child: None,
                    });
                }
                instances.push(Instance {
                    function: original,
                    incoming: parent,
                    active,
                    locals,
                    blocks,
                    calls: call_start..calls.len(),
                });
            }
            link_root(
                semantic.callables(),
                &instances[first..],
                &mut calls,
                &mut incoming,
                out,
            )?;
            roots.push(Root {
                function,
                physical,
                instances: first..instances.len(),
            });
        }
        if instances.len() != instance_count || calls.len() != call_capacity {
            return Err(mismatch());
        }
        Ok(Self {
            source,
            roots,
            instances,
            calls,
            ledger: out.budget.work_ledger_identity_v1(),
            slot: std::ptr::from_ref(&*out.budget) as usize,
            required: out.budget.storage(),
        })
    }

    fn check(&self, out: &Writer<'_, '_>) -> Result<()> {
        self.source.check_query_v18(out.budget)?;
        if self.ledger != out.budget.work_ledger_identity_v1()
            || self.slot != std::ptr::from_ref(&*out.budget) as usize
            || out.budget.storage() < self.required
        {
            return Err(self
                .source
                .retain_query_resource_error_v18(Resource::Accounting)
                .into());
        }
        Ok(())
    }

    pub(super) fn root(&self, root: usize, out: &Writer<'_, '_>) -> Result<&Root> {
        self.check(out)?;
        self.roots.get(root).ok_or_else(mismatch)
    }

    pub(super) fn source(&self, out: &Writer<'_, '_>) -> Result<&'view Source<'source>> {
        self.check(out)?;
        Ok(self.source)
    }

    pub(super) fn instance(
        &self,
        root: usize,
        instance: usize,
        out: &Writer<'_, '_>,
    ) -> Result<&Instance> {
        let range = &self.root(root, out)?.instances;
        if instance >= range.len() {
            return Err(mismatch());
        }
        self.instances
            .get(range.start + instance)
            .ok_or_else(mismatch)
    }

    pub(super) fn calls(
        &self,
        root: usize,
        instance: usize,
        out: &Writer<'_, '_>,
    ) -> Result<&[CallSite]> {
        let row = self.instance(root, instance, out)?;
        self.calls.get(row.calls.clone()).ok_or_else(mismatch)
    }
}

fn link_root(
    callables: &[Callable],
    instances: &[Instance],
    calls: &mut [CallSite],
    incoming: &mut [Incoming],
    out: &mut Writer<'_, '_>,
) -> Result<()> {
    if incoming.len().checked_add(1) != Some(instances.len()) {
        return Err(mismatch());
    }
    for row in incoming.iter() {
        out.budget.charge_work(3)?;
        let child = instances.get(row.child).ok_or_else(mismatch)?;
        if row.caller >= row.child || child.incoming != Some((row.caller, row.block)) {
            return Err(mismatch());
        }
    }
    let log = logarithm(incoming.len());
    out.budget.charge_work(
        incoming
            .len()
            .checked_mul(log.checked_add(2).ok_or(Resource::Arithmetic)?)
            .ok_or(Resource::Arithmetic)?,
    )?;
    incoming.sort_unstable();
    if incoming
        .windows(2)
        .any(|pair| (pair[0].caller, pair[0].block) == (pair[1].caller, pair[1].block))
    {
        return Err(mismatch());
    }
    let mut matched = 0usize;
    for (instance, row) in instances.iter().enumerate() {
        for call in calls.get_mut(row.calls.clone()).ok_or_else(mismatch)? {
            out.budget
                .charge_work(log.checked_add(4).ok_or(Resource::Arithmetic)?)?;
            if call.caller != instance || call.child.is_some() {
                return Err(mismatch());
            }
            let Ok(found) = incoming
                .binary_search_by_key(&(instance, call.block), |row| (row.caller, row.block))
            else {
                continue;
            };
            let child = incoming[found].child;
            let child_row = &instances[child];
            let Some(Callable::Defined { function }) =
                callables.get(call.callable.index() as usize)
            else {
                return Err(mismatch());
            };
            if *function != child_row.function
                || (child_row.active && (!row.active || !call.ssa_reachable))
            {
                return Err(mismatch());
            }
            call.child = Some(child);
            matched = matched.checked_add(1).ok_or(Resource::Arithmetic)?;
        }
    }
    if matched != incoming.len() {
        return Err(mismatch());
    }
    Ok(())
}

fn call(terminator: &Terminator) -> Option<(CallKind, CallableId)> {
    match terminator {
        Terminator::Call(call) => Some((CallKind::Direct, call.callee())),
        Terminator::TailCall(call) => Some((CallKind::Tail, call.callee())),
        Terminator::Drop { drop_glue, .. } => {
            Some((CallKind::Drop, CallableId::from_index(drop_glue.index())))
        }
        _ => None,
    }
}

fn logarithm(count: usize) -> usize {
    usize::BITS as usize - count.max(1).leading_zeros() as usize
}

fn headers() -> usize {
    fn vector<T>() -> usize {
        size_of::<Vec<T>>() + 2 * size_of::<Result<Vec<T>>>()
    }
    fn query<T>() -> usize {
        size_of::<T>()
            + 2 * size_of::<Result<T>>()
            + 2 * size_of::<
                std::result::Result<T, fe2o3_lower_mir_kernel::ProductionSourceOwnedViewErrorV18>,
            >()
    }
    size_of::<InvocationPlan<'_, '_>>()
        + 2 * size_of::<Result<InvocationPlan<'_, '_>>>()
        + vector::<Root>()
        + vector::<Instance>()
        + vector::<CallSite>()
        + vector::<Incoming>()
        + vector::<usize>()
        + size_of::<Root>()
        + size_of::<Instance>()
        + size_of::<CallSite>()
        + size_of::<Incoming>()
        + size_of::<(&Source<'_>, &mut Writer<'_, '_>, Ledger)>()
        + size_of::<Result<&Root>>()
        + size_of::<Result<&Instance>>()
        + size_of::<Result<&[CallSite]>>()
        + size_of::<Result<&Source<'_>>>()
        + size_of::<(&InvocationPlan<'_, '_>, &Writer<'_, '_>)>()
        + query::<()>()
        + query::<usize>()
        + query::<bool>()
        + query::<(FunctionId, usize)>()
        + query::<(FunctionId, Option<(usize, Block)>)>()
        + query::<&fe2o3_mir_model::semantic_mir_v1::AdmittedInertSemanticMirV1>()
        + query::<&fe2o3_pliron::ProductionSemanticSsaOwnerV1>()
        + size_of::<Option<(CallKind, CallableId)>>()
        + size_of::<(&[Callable], &[Instance], &mut [CallSite], &mut [Incoming])>()
        + 18 * size_of::<usize>()
}

#[cfg(test)]
#[path = "original_semantic_mir_invocations_v32_tests.rs"]
pub(super) mod tests;
