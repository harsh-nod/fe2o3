//! Shared original frame obligations. A parent link is static call provenance,
//! not evidence that a frame is currently suspended or that a step preserves it.
use super::super::{
    Terminator,
    boundary::{Boundaries, ControlInput},
    invocations::{CallKind, InvocationPlan},
};
use super::{
    Error, Resource, Result, Writer,
    component_demands::ComponentDemandsV42,
    slots::SourceSlots,
    source_frame_demands::{self, ComponentCut, SourceDemand},
    vector,
};
use fe2o3_mir_model::{SsaBlockIdV1 as Block, semantic_mir_v1::SemanticFunctionIdV1 as Function};
use std::{
    mem::{align_of_val, size_of, size_of_val},
    ops::Range,
};

#[cfg(test)]
#[path = "original_semantic_mir_source_frame_plan_v281_tests.rs"]
mod tests;

pub(super) struct Frame {
    pub root: usize,
    pub instance: usize,
    pub function: Function,
    pub active: bool,
    pub locals: Range<usize>,
    pub cuts: Range<usize>,
    pub calls: Range<usize>,
    pub parent_call: Option<usize>,
    pub depth: usize,
}

pub(super) struct Cut {
    pub root: usize,
    pub instance: usize,
    pub block: usize,
    pub pc: usize,
    pub reachable: bool,
    pub demands: Range<usize>,
}

pub(super) struct Call {
    pub root: usize,
    pub caller: usize,
    pub block: usize,
    pub child: Option<usize>,
    // Authenticated instance activity, not merely the caller's SSA reachability.
    pub child_active: bool,
    pub kind: CallKind,
    pub reachable: bool,
    pub continuation: Option<usize>,
    pub demands: Range<usize>,
}

pub(super) struct Demand {
    pub frame: usize,
    pub source: SourceDemand,
}

pub(super) struct FramePlan<'plan, 'slots, 'view, 'source> {
    plan: &'plan InvocationPlan<'view, 'source>,
    slots: &'slots SourceSlots<'view, 'source>,
    pub roots: Vec<Range<usize>>,
    pub frames: Vec<Frame>,
    pub cuts: Vec<Cut>,
    pub calls: Vec<Call>,
    pub demands: Vec<Demand>,
    components: Vec<Option<ComponentDemandsV42<'slots, 'view, 'source>>>,
    required: usize,
}

fn mismatch() -> Error {
    Error::Statement("original frame plan differs from its retained owner or call ancestry")
}

fn add(a: usize, b: usize) -> Result<usize> {
    a.checked_add(b).ok_or_else(|| Resource::Arithmetic.into())
}

fn block(index: usize) -> Result<Block> {
    Ok(Block::new(
        u32::try_from(index).map_err(|_| Resource::Arithmetic)?,
    ))
}

#[allow(clippy::too_many_arguments)]
fn parent_link(
    root: usize,
    instance: usize,
    scope: &super::super::invocations::Root,
    row: &super::super::invocations::Instance,
    frames: &[Frame],
    calls: &[Call],
    budget: &mut fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1<'_>,
) -> Result<(Option<usize>, usize)> {
    budget.charge_work(5)?;
    if instance >= scope.instances.len() {
        return Err(mismatch());
    }
    match row.incoming {
        None if instance == 0 && row.function == scope.function && row.active => Ok((None, 0)),
        Some((parent, site)) if parent < instance => {
            let parent_row = frames
                .get(add(scope.instances.start, parent)?)
                .ok_or_else(mismatch)?;
            budget.charge_work(
                (usize::BITS - parent_row.calls.len().max(1).leading_zeros()) as usize + 7,
            )?;
            if parent_row.root != root || parent_row.instance != parent {
                return Err(mismatch());
            }
            let slice = calls.get(parent_row.calls.clone()).ok_or_else(mismatch)?;
            let at = slice
                .binary_search_by_key(&(site.index() as usize), |call| call.block)
                .map_err(|_| mismatch())?;
            let call = &slice[at];
            if call.root != root
                || call.caller != parent
                || call.child != Some(instance)
                || call.child_active != row.active
                || (row.active
                    && (!parent_row.active
                        || !call.reachable
                        || call.kind != CallKind::Direct
                        || call.continuation.is_none()))
            {
                return Err(mismatch());
            }
            Ok((
                Some(add(parent_row.calls.start, at)?),
                add(parent_row.depth, 1)?,
            ))
        }
        _ => Err(mismatch()),
    }
}

impl<'plan, 'slots, 'view, 'source> FramePlan<'plan, 'slots, 'view, 'source> {
    fn headers() -> usize {
        // All retained carriers plus fixed graph-query, branch and loop frames.
        3 * size_of::<Self>()
            + 3 * size_of::<Result<Self>>()
            + 3 * size_of::<Frame>()
            + 3 * size_of::<Call>()
            + 3 * size_of::<Cut>()
            + 4 * size_of::<SourceDemand>()
            + 3 * size_of::<Demand>()
            + 4 * size_of::<Range<usize>>()
            + size_of::<Vec<Vec<Block>>>()
            + 3 * size_of::<Vec<Block>>()
            + size_of::<Vec<SourceDemand>>()
            + size_of::<Result<Vec<SourceDemand>>>()
            + size_of::<Boundaries<'_>>()
            + size_of::<Option<Boundaries<'_>>>()
            + size_of::<Result<Boundaries<'_>>>()
            + size_of::<ControlInput<'_>>()
            + 4 * size_of::<std::slice::Iter<'_, SourceDemand>>()
            + size_of::<std::vec::IntoIter<SourceDemand>>()
            + size_of::<&[Frame]>()
            + size_of::<&[Call]>()
            + 2 * size_of::<Result<(Option<usize>, usize)>>()
            + 3 * size_of::<std::slice::Iter<'_, super::super::invocations::CallSite>>()
            + 3 * size_of::<std::slice::Iter<'_, fe2o3_mir_model::SsaVariableIdV1>>()
            + 3 * size_of::<
                std::slice::Iter<'_, fe2o3_mir_model::semantic_mir_v1::SemanticBasicBlockV1>,
            >()
            + 3 * size_of::<Result<()>>()
            + 2 * size_of::<Result<usize>>()
            + 40 * size_of::<usize>()
            + 32 * size_of::<&()>()
    }

    pub(super) fn derive(
        plan: &'plan InvocationPlan<'view, 'source>,
        slots: &'slots SourceSlots<'view, 'source>,
        out: &mut Writer<'_, '_>,
    ) -> Result<Self> {
        let derive = |out: &mut Writer<'_, '_>| {
            let source = plan.source(out)?;
            out.budget.charge_work(1)?;
            if !std::ptr::eq(source, slots.correspondence(out)?.source(out.budget)?) {
                return Err(mismatch());
            }
            let semantic = source.source_semantic(out.budget)?;
            let archive = source.source_ssa(out.budget)?;
            let roots = source.root_count(out.budget)?;
            let (mut frames, mut cuts, mut calls, mut demands) = (0, 0, 0, 0);
            for root in 0..roots {
                let scope = plan.root(root, out)?;
                frames = add(frames, scope.instances.len())?;
                for instance in 0..scope.instances.len() {
                    let row = plan.instance(root, instance, out)?;
                    calls = add(calls, plan.calls(root, instance, out)?.len())?;
                    cuts = add(cuts, row.blocks.len())?;
                    if !row.active {
                        continue;
                    }
                    let function = &semantic.functions()[row.function.index() as usize];
                    let ssa = archive
                        .plan_for_function(row.function)
                        .ok_or_else(mismatch)?
                        .plan();
                    for index in 0..function.blocks().len() {
                        out.budget.charge_work(1)?;
                        demands = add(demands, ssa.live_in(block(index)?).map_or(0, <[_]>::len))?;
                    }
                    for call in plan.calls(root, instance, out)? {
                        out.budget.charge_work(1)?;
                        if call.ssa_reachable
                            && match call.child {
                                Some(child) => plan.instance(root, child, out)?.active,
                                None => false,
                            }
                        {
                            // Continuation demands cannot exceed the original local roster.
                            demands = add(demands, row.locals.len())?;
                        }
                    }
                }
            }
            let mut result = Self {
                plan,
                slots,
                roots: vector(roots, out)?,
                frames: vector(frames, out)?,
                cuts: vector(cuts, out)?,
                calls: vector(calls, out)?,
                demands: vector(demands, out)?,
                components: vector(semantic.functions().len(), out)?,
                required: 0,
            };
            out.budget.charge_work(semantic.functions().len())?;
            result
                .components
                .resize_with(semantic.functions().len(), || None);
            for root in 0..roots {
                let scope = plan.root(root, out)?;
                if scope.instances.start != result.frames.len() {
                    return Err(mismatch());
                }
                result.roots.push(scope.instances.clone());
                for instance in 0..scope.instances.len() {
                    let row = plan.instance(root, instance, out)?;
                    let function = &semantic.functions()[row.function.index() as usize];
                    let ssa = archive
                        .plan_for_function(row.function)
                        .ok_or_else(mismatch)?
                        .plan();
                    let (first_cut, first_call) = (result.cuts.len(), result.calls.len());
                    let mut successors = vector(
                        if row.active {
                            function.blocks().len()
                        } else {
                            0
                        },
                        out,
                    )?;
                    if row.active {
                        for body in function.blocks() {
                            out.budget.charge_work(1)?;
                            let mut edges = vector(body.terminator().kind().edge_count(), out)?;
                            body.terminator().kind().try_for_each_edge(|edge| {
                                out.budget.charge_work(1)?;
                                edges.push(block(edge.target().index() as usize)?);
                                Ok::<_, Error>(())
                            })?;
                            successors.push(edges);
                        }
                    }
                    let boundaries = if row.active {
                        Some(Boundaries::derive_source_v299(
                            ssa,
                            ControlInput {
                                entry: block(function.entry().index() as usize)?,
                                successors: &successors,
                            },
                            archive,
                            row.function,
                            out,
                        )?)
                    } else {
                        None
                    };
                    if row.active && result.components[row.function.index() as usize].is_none() {
                        result.components[row.function.index() as usize] =
                            Some(ComponentDemandsV42::derive(slots, row.function, out)?);
                    }
                    for index in 0..function.blocks().len() {
                        out.budget.charge_work(2)?;
                        let first = result.demands.len();
                        let live = ssa.live_in(block(index)?);
                        if let (Some(boundaries), Some(live)) = (&boundaries, live) {
                            for &variable in live {
                                out.budget.charge_work(1)?;
                                result.demands.push(Demand {
                                    frame: result.frames.len(),
                                    source: SourceDemand {
                                        local: variable.get() as usize,
                                        value: boundaries.value(block(index)?, variable, out)?,
                                        components: ComponentCut::at(index),
                                    },
                                });
                            }
                        }
                        result.cuts.push(Cut {
                            root,
                            instance,
                            block: index,
                            pc: add(row.blocks.start, index)?,
                            reachable: row.active && live.is_some(),
                            demands: first..result.demands.len(),
                        });
                    }
                    for site in plan.calls(root, instance, out)? {
                        out.budget.charge_work(2)?;
                        let first = result.demands.len();
                        let child_active = match site.child {
                            Some(child) => plan.instance(root, child, out)?.active,
                            None => false,
                        };
                        let continuation = match function.blocks()[site.block.index() as usize]
                            .terminator()
                            .kind()
                        {
                            Terminator::Call(call) => call
                                .destination()
                                .map(|dest| dest.edge().target().index() as usize),
                            _ => None,
                        };
                        if let Some(boundaries) = &boundaries {
                            if site.ssa_reachable && child_active {
                                if site.kind != CallKind::Direct {
                                    return Err(Error::Statement(
                                        "expanded frame demands require an ordinary retained call continuation",
                                    ));
                                }
                                let carry = source_frame_demands::caller_demands(
                                    slots,
                                    plan,
                                    root,
                                    instance,
                                    site.block,
                                    boundaries,
                                    &mut result.components,
                                    out,
                                )?;
                                out.budget.charge_work(carry.len())?;
                                for source in carry {
                                    result.demands.push(Demand {
                                        frame: result.frames.len(),
                                        source,
                                    });
                                }
                            }
                        }
                        result.calls.push(Call {
                            root,
                            caller: instance,
                            block: site.block.index() as usize,
                            child: site.child,
                            child_active,
                            kind: site.kind,
                            reachable: site.ssa_reachable,
                            continuation,
                            demands: first..result.demands.len(),
                        });
                    }
                    let (parent_call, depth) = parent_link(
                        root,
                        instance,
                        scope,
                        row,
                        &result.frames,
                        &result.calls,
                        out.budget,
                    )?;
                    result.frames.push(Frame {
                        root,
                        instance,
                        function: row.function,
                        active: row.active,
                        locals: row.locals.clone(),
                        cuts: first_cut..result.cuts.len(),
                        calls: first_call..result.calls.len(),
                        parent_call,
                        depth,
                    });
                }
            }
            // Conservative generation scratch stays charged with retained cache floors.
            // Do not refund dropped per-call temporaries below any cached required floor.
            result.required = out.budget.storage();
            result.check(plan, slots, out)?;
            Ok(result)
        };
        let headers = Self::headers()
            .checked_add(2 * size_of_val(&derive))
            .and_then(|n| n.checked_add(align_of_val(&derive)))
            .ok_or(Resource::Arithmetic)?;
        out.budget.reserve_storage(headers)?;
        slots.with_source_query_v42(out, derive)
    }

    pub(super) fn check(
        &self,
        plan: &InvocationPlan<'_, '_>,
        slots: &SourceSlots<'_, '_>,
        out: &mut Writer<'_, '_>,
    ) -> Result<()> {
        self.slots
            .check_query_storage_floor(self.required, out.budget)?;
        out.budget.charge_work(3)?;
        if !std::ptr::eq(self.plan, plan)
            || !std::ptr::eq(self.slots, slots)
            || !std::ptr::eq(
                plan.source(out)?,
                slots.correspondence(out)?.source(out.budget)?,
            )
        {
            return Err(mismatch());
        }
        Ok(())
    }

    pub(super) fn leaf_required(
        &self,
        frame: usize,
        demand: usize,
        leaf: usize,
        out: &mut Writer<'_, '_>,
    ) -> Result<bool> {
        self.check(self.plan, self.slots, out)?;
        let owner = self.frames.get(frame).ok_or_else(mismatch)?;
        let demand = self.demands.get(demand).ok_or_else(mismatch)?;
        if demand.frame != frame {
            return Err(mismatch());
        }
        let demand = &demand.source;
        let components = self
            .components
            .get(owner.function.index() as usize)
            .and_then(Option::as_ref)
            .ok_or_else(mismatch)?;
        components.check_owner_v281(self.slots, owner.function, out)?;
        out.budget.charge_work(2)?;
        Ok(!demand
            .components
            .overwritten
            .is_some_and(|(begin, end)| begin <= leaf && leaf < end)
            && components.leaf_required(
                owner.function,
                demand.components.block,
                demand.local,
                leaf,
                out,
            )?)
    }
}
