//! Original caller demands, independent of any original or expanded target.
//! These rows describe obligations; they do not prove preservation across calls.
use super::{
    Boundaries, ComponentDemandsV42, Error, Event, InvocationPlan, Resource, Result, SourceBlock,
    SourceSlots, Terminator, Value, Variable, Writer, block, vector,
};
use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as Budget;
use std::{
    mem::{align_of_val, size_of, size_of_val},
    ops::Range,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct ComponentCut {
    pub(super) block: usize,
    pub(super) overwritten: Option<(usize, usize)>,
}

impl ComponentCut {
    pub(super) fn at(block: usize) -> Self {
        Self {
            block,
            overwritten: None,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct SourceDemand {
    pub(super) local: usize,
    pub(super) value: Value,
    pub(super) components: ComponentCut,
}

fn mismatch() -> Error {
    Error::Statement("original frame demand differs from its retained caller SSA")
}

pub(super) fn headers() -> usize {
    // Caller inputs, borrowed source/SSA rows, and event/leaf traversal state.
    18 * size_of::<&()>()
        + 18 * size_of::<usize>()
        + size_of::<Range<usize>>()
        + size_of::<Option<Range<usize>>>()
        + size_of::<ComponentCut>()
        + size_of::<SourceDemand>()
        + size_of::<Vec<SourceDemand>>()
        + 2 * size_of::<Result<Vec<SourceDemand>>>()
        + size_of::<Vec<Option<Value>>>()
        + size_of::<Result<Vec<Option<Value>>>>()
        + size_of::<Option<Value>>()
        + size_of::<Value>()
        + size_of::<Event>()
        + size_of::<Variable>()
        + 2 * size_of::<fe2o3_mir_model::SsaBlockIdV1>()
        + 2 * size_of::<SourceBlock>()
        + 2 * size_of::<std::slice::Iter<'_, Variable>>()
        + size_of::<std::slice::Iter<'_, (u32, Event)>>()
        + size_of::<&[Variable]>()
        + size_of::<&[(u32, Event)]>()
        + size_of::<&[super::super::super::invocations::CallSite]>()
        + size_of::<std::result::Result<usize, usize>>()
        + size_of::<&mut [Option<Value>]>()
        + size_of::<&mut [Option<ComponentDemandsV42<'_, '_, '_>>]>()
        + 3 * size_of::<Result<()>>()
}

fn replay_events(
    values: &mut [Option<Value>],
    events: &[(u32, Event)],
    budget: &mut Budget<'_>,
) -> Result<()> {
    for &(_, event) in events {
        budget.charge_work(2)?;
        match event {
            Event::Define { variable, value } => {
                *values
                    .get_mut(variable.get() as usize)
                    .ok_or_else(mismatch)? = Some(value);
            }
            Event::Kill { variable, .. } => {
                *values
                    .get_mut(variable.get() as usize)
                    .ok_or_else(mismatch)? = None;
            }
            Event::Use { .. } => (),
        }
    }
    Ok(())
}

fn demanded_value(values: &[Option<Value>], variable: Variable) -> Result<Value> {
    values
        .get(variable.get() as usize)
        .copied()
        .flatten()
        .ok_or_else(mismatch)
}

#[allow(clippy::too_many_arguments)]
pub(super) fn caller_demands<'slots, 'view, 'source>(
    slots: &'slots SourceSlots<'view, 'source>,
    plan: &InvocationPlan<'_, '_>,
    root: usize,
    parent: usize,
    site: SourceBlock,
    boundaries: &Boundaries<'_>,
    component_demands: &mut [Option<ComponentDemandsV42<'slots, 'view, 'source>>],
    out: &mut Writer<'_, '_>,
) -> Result<Vec<SourceDemand>> {
    let query = |out: &mut Writer<'_, '_>| {
        let relation = slots.correspondence(out)?;
        let source = relation.source(out.budget)?;
        let semantic = source.source_semantic(out.budget)?;
        out.budget.charge_work(2)?;
        if !std::ptr::eq(source, plan.source(out)?)
            || component_demands.len() != semantic.functions().len()
        {
            return Err(mismatch());
        }
        let caller = plan.instance(root, parent, out)?;
        let calls = plan.calls(root, parent, out)?;
        out.budget
            .charge_work((usize::BITS - calls.len().max(1).leading_zeros()) as usize + 4)?;
        let at = calls
            .binary_search_by_key(&site.index(), |row| row.block.index())
            .map_err(|_| mismatch())?;
        let call_site = &calls[at];
        let child = call_site.child.ok_or_else(mismatch)?;
        if !caller.active
            || !call_site.ssa_reachable
            || call_site.caller != parent
            || plan.instance(root, child, out)?.incoming != Some((parent, site))
        {
            return Err(mismatch());
        }
        let function = semantic
            .functions()
            .get(caller.function.index() as usize)
            .ok_or_else(mismatch)?;
        let ssa = source
            .source_ssa(out.budget)?
            .plan_for_function(caller.function)
            .ok_or_else(mismatch)?
            .plan();
        boundaries.check_plan_v281(ssa, out)?;
        let call = match function
            .blocks()
            .get(site.index() as usize)
            .map(|row| row.terminator().kind())
        {
            Some(Terminator::Call(call)) => call,
            _ => return Err(mismatch()),
        };
        let destination = call.destination().ok_or_else(mismatch)?;
        let next = block(destination.edge().target().index() as usize)?;
        let overwritten = if destination.place().projections().is_empty()
            || slots.has_original_object(root, parent, destination.place().local().index(), out)?
        {
            None
        } else {
            let ty = function
                .locals()
                .get(destination.place().local().index() as usize)
                .ok_or_else(mismatch)?
                .ty();
            let (range, result_type) = slots
                .aggregate_component_range(ty, destination.place().projections(), out)?
                .ok_or_else(mismatch)?;
            if result_type != destination.place().ty() {
                return Err(mismatch());
            }
            Some(range)
        };
        let live = ssa.live_in(next).ok_or_else(mismatch)?;
        let mut result = vector(live.len(), out)?;
        let mut values = vector(caller.locals.len(), out)?;
        out.budget.charge_work(caller.locals.len())?;
        values.resize(caller.locals.len(), None);
        for &variable in ssa
            .live_in(block(site.index() as usize)?)
            .ok_or_else(mismatch)?
        {
            out.budget.charge_work(1)?;
            *values
                .get_mut(variable.get() as usize)
                .ok_or_else(mismatch)? =
                Some(boundaries.value(block(site.index() as usize)?, variable, out)?);
        }
        // A call suspends the post-statement environment, not its block entry.
        replay_events(
            &mut values,
            ssa.resolved_events(block(site.index() as usize)?)
                .ok_or_else(mismatch)?,
            out.budget,
        )?;
        for &variable in live {
            out.budget.charge_work(2)?;
            let is_destination = variable.get() == destination.place().local().index();
            if is_destination {
                let Some(overwritten) = &overwritten else {
                    continue;
                };
                let cached = component_demands
                    .get_mut(caller.function.index() as usize)
                    .ok_or_else(mismatch)?;
                if cached.is_none() {
                    *cached = Some(ComponentDemandsV42::derive(slots, caller.function, out)?);
                }
                let demands = cached.as_ref().ok_or_else(mismatch)?;
                demands.check_owner_v281(slots, caller.function, out)?;
                let count = slots
                    .aggregate_leaf_count(function.locals()[variable.get() as usize].ty(), out)?
                    .ok_or_else(mismatch)?;
                let mut needed = false;
                for leaf in 0..count {
                    out.budget.charge_work(1)?;
                    if !overwritten.contains(&leaf)
                        && demands.leaf_required(
                            caller.function,
                            next.get() as usize,
                            variable.get() as usize,
                            leaf,
                            out,
                        )?
                    {
                        needed = true;
                    }
                }
                if !needed {
                    continue;
                }
            }
            result.push(SourceDemand {
                local: variable.get() as usize,
                value: demanded_value(&values, variable)?,
                components: ComponentCut {
                    block: next.get() as usize,
                    overwritten: if is_destination {
                        overwritten.as_ref().map(|range| (range.start, range.end))
                    } else {
                        None
                    },
                },
            });
        }
        Ok(result)
    };
    let frames = size_of_val(&query)
        .checked_mul(2)
        .and_then(|bytes| bytes.checked_add(align_of_val(&query)))
        .and_then(|bytes| bytes.checked_add(headers()))
        .ok_or(Resource::Arithmetic)?;
    out.budget.reserve_storage(frames)?;
    slots.with_source_query_v42(out, query)
}

#[cfg(test)]
#[path = "original_semantic_mir_source_frame_demands_v281_tests.rs"]
mod tests;
