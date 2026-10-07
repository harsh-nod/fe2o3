//! Partial statement-prefix demands, not a complete source-frame relation.
//! Scalar liveness is available only for promoted locals. Component demands
//! reuse their independent source fixed point, including unpromoted locals.
use super::super::component_demands::ComponentDomainV283;
use super::*;
use fe2o3_mir_model::{SsaResolvedEventV1 as Event, SsaValueV1 as Value};
use fe2o3_pliron::ProductionSemanticSsaOccurrenceSiteV1 as Site;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in super::super) enum ScalarDemand {
    Needed(Value),
    NotNeeded,
    /// No all-local liveness solution is retained for this local. Not dead.
    Unavailable,
}

pub(in super::super) struct Local {
    pub local: usize,
    pub ty: fe2o3_mir_model::semantic_mir_v1::SemanticTypeIdV1,
    pub scalar: ScalarDemand,
    pub current: Option<Value>,
    pub domain: ComponentDomainV283,
    pub components: Range<usize>,
}

pub(in super::super) struct Prefix<'frames, 'plan, 'slots, 'view, 'source> {
    owner: &'frames FramePlan<'plan, 'slots, 'view, 'source>,
    pub frame: usize,
    pub block: usize,
    pub statement: usize,
    pub pc: usize,
    pub locals: Vec<Local>,
    components: Vec<bool>,
    required: usize,
    charged: usize,
}

fn mismatch() -> Error {
    Error::Statement("partial source prefix differs from its retained statement or SSA owner")
}

fn zeros<T: Copy + Default>(count: usize, out: &mut Writer<'_, '_>) -> Result<Vec<T>> {
    let mut result = vector(count, out)?;
    out.budget.charge_work(count)?;
    result.resize(count, T::default());
    Ok(result)
}

fn headers() -> usize {
    2 * size_of::<Prefix<'_, '_, '_, '_, '_>>()
        + 2 * size_of::<Result<Prefix<'_, '_, '_, '_, '_>>>()
        + 2 * size_of::<Local>()
        + 2 * size_of::<ScalarDemand>()
        + size_of::<Vec<Local>>()
        + size_of::<Vec<Option<Value>>>()
        + 6 * size_of::<Vec<bool>>()
        + size_of::<Vec<u64>>()
        + 3 * size_of::<Event>()
        + 3 * size_of::<Option<Event>>()
        + 3 * size_of::<Option<Value>>()
        + 3 * size_of::<Range<usize>>()
        + size_of::<fe2o3_pliron::ProductionSemanticSsaFunctionOccurrencesV1<'_>>()
        + 2 * size_of::<std::slice::Iter<'_, fe2o3_pliron::ProductionSemanticSsaEventOccurrenceV1>>(
        )
        + size_of::<std::slice::Iter<'_, fe2o3_pliron::ProductionSemanticSsaSuccessorOccurrenceV1>>(
        )
        + 2 * size_of::<std::slice::Iter<'_, fe2o3_mir_model::SsaArgumentV1>>()
        + size_of::<std::slice::Iter<'_, fe2o3_mir_model::SsaVariableIdV1>>()
        + size_of::<std::slice::Iter<'_, Demand>>()
        + size_of::<std::slice::IterMut<'_, Local>>()
        + size_of::<std::slice::Iter<'_, Local>>()
        + 5 * size_of::<Result<()>>()
        + 4 * size_of::<Result<usize>>()
        + 40 * size_of::<usize>()
        + 30 * size_of::<&()>()
}

impl<'plan, 'slots, 'view, 'source> FramePlan<'plan, 'slots, 'view, 'source> {
    /// k == statement count is before the terminator, not an outgoing edge.
    /// All per-query source/suffix scans and vector capacities are charged.
    pub(in super::super) fn partial_prefix_v296(
        &self,
        root: usize,
        instance: usize,
        block_index: usize,
        statement: usize,
        out: &mut Writer<'_, '_>,
    ) -> Result<Prefix<'_, 'plan, 'slots, 'view, 'source>> {
        let floor = out.budget.storage();
        let derive = |out: &mut Writer<'_, '_>| {
            self.check(self.plan, self.slots, out)?;
            out.budget.reserve_storage(headers())?;
            out.budget.charge_work(8)?;
            let root_frames = self.roots.get(root).ok_or_else(mismatch)?;
            if instance >= root_frames.len() {
                return Err(mismatch());
            }
            let frame_index = add(root_frames.start, instance)?;
            let frame = self.frames.get(frame_index).ok_or_else(mismatch)?;
            if frame.root != root
                || frame.instance != instance
                || !frame.active
                || block_index >= frame.cuts.len()
            {
                return Err(mismatch());
            }
            let cut = &self.cuts[add(frame.cuts.start, block_index)?];
            if !cut.reachable
                || cut.root != root
                || cut.instance != instance
                || cut.block != block_index
            {
                return Err(mismatch());
            }
            let source = self.plan.source(out)?;
            let semantic = source.source_semantic(out.budget)?;
            let function = semantic
                .functions()
                .get(frame.function.index() as usize)
                .ok_or_else(mismatch)?;
            let body = function.blocks().get(block_index).ok_or_else(mismatch)?;
            if statement > body.statements().len() || frame.locals.len() != function.locals().len()
            {
                return Err(mismatch());
            }
            let archive = source.source_ssa(out.budget)?;
            let ssa = archive
                .plan_for_function(frame.function)
                .ok_or_else(mismatch)?
                .plan();
            let occurrences = archive
                .occurrences_v1()
                .and_then(|all| all.function(frame.function))
                .ok_or_else(mismatch)?;
            if !std::ptr::eq(occurrences.owner(), archive) {
                return Err(mismatch());
            }
            let block = block(block_index)?;
            let cache = self
                .components
                .get(frame.function.index() as usize)
                .and_then(Option::as_ref)
                .ok_or_else(mismatch)?;
            cache.check_owner_v281(self.slots, frame.function, out)?;
            let count = frame.locals.len();
            let mut locals = vector(count, out)?;
            let mut leaves = 0;
            for local in 0..count {
                out.budget.charge_work(1)?;
                let (ty, domain, length) =
                    cache.local_domain_v283(self.slots, frame.function, local, out)?;
                let end = add(leaves, length)?;
                locals.push(Local {
                    local,
                    ty,
                    scalar: ScalarDemand::Unavailable,
                    current: None,
                    domain,
                    components: leaves..end,
                });
                leaves = end;
            }
            let mut components = zeros::<bool>(leaves, out)?;
            let retained = out.budget.storage();
            let bits = cache.prefix_bits_v296(block_index, statement, out)?;
            let mut promoted = zeros::<bool>(count, out)?;
            let mut current = zeros::<Option<Value>>(count, out)?;
            let mut generated = zeros::<bool>(count, out)?;
            let mut killed = zeros::<bool>(count, out)?;
            let mut success_killed = zeros::<bool>(count, out)?;
            let mut edge_defined = zeros::<bool>(count, out)?;
            for variable in ssa.promoted_variables() {
                out.budget.charge_work(1)?;
                *promoted
                    .get_mut(variable.get() as usize)
                    .ok_or_else(mismatch)? = true;
            }
            for demand in &self.demands[cut.demands.clone()] {
                out.budget.charge_work(2)?;
                if demand.frame != frame_index {
                    return Err(mismatch());
                }
                let at = demand.source.local;
                if promoted.get(at) != Some(&true) || current.get(at).is_none() {
                    return Err(mismatch());
                }
                current[at] = Some(demand.source.value);
            }
            let failure_start = occurrences.terminal_failure_start(block);
            let resolved_count = ssa.resolved_events(block).ok_or_else(mismatch)?.len();
            let lookup_work = (usize::BITS - resolved_count.max(1).leading_zeros()) as usize + 2;
            let mut in_failure = false;
            let mut ordinal = 0usize;
            for row in occurrences.events() {
                // This is deliberately a bounded full captured-function scan.
                out.budget.charge_work(7)?;
                let (site_block, before) = match row.site() {
                    Site::Statement {
                        block,
                        statement: at,
                    } => (block, (at as usize) < statement),
                    Site::Terminator { block } => (block, false),
                };
                if site_block != block {
                    continue;
                }
                out.budget.charge_work(lookup_work)?;
                let local = row.event().variable().get() as usize;
                if row.ordinal() as usize != ordinal
                    || !row.is_reachable()
                    || promoted.get(local).copied() != Some(row.is_promoted())
                    || row.resolved() != ssa.resolved_event(block, row.ordinal()).copied()
                {
                    return Err(mismatch());
                }
                if !before && !in_failure && failure_start.is_some_and(|start| ordinal >= start) {
                    out.budget.charge_work(count)?;
                    success_killed.copy_from_slice(&killed);
                    in_failure = true;
                }
                ordinal = add(ordinal, 1)?;
                let Some(event) = row.resolved() else {
                    if row.is_promoted() {
                        return Err(mismatch());
                    }
                    continue;
                };
                if before {
                    match event {
                        Event::Define { value, .. } => current[local] = Some(value),
                        Event::Kill { .. } => current[local] = None,
                        Event::Use { value, .. } if current[local] == Some(value) => (),
                        Event::Use { .. } => return Err(mismatch()),
                    }
                } else {
                    match event {
                        Event::Use { .. } => generated[local] |= !killed[local],
                        Event::Define { .. } | Event::Kill { .. } => killed[local] = true,
                    }
                }
            }
            if in_failure {
                out.budget.charge_work(count)?;
                killed.copy_from_slice(&success_killed);
            }
            for edge in occurrences.successors() {
                out.budget.charge_work(1)?;
                if edge.id().source() != block {
                    continue;
                }
                let definitions = ssa.edge_definitions(edge.id()).ok_or_else(mismatch)?;
                for definition in definitions {
                    out.budget.charge_work(1)?;
                    *edge_defined
                        .get_mut(definition.variable().get() as usize)
                        .ok_or_else(mismatch)? = true;
                }
                // Edge arguments carry phi transport, not all live-through
                // locals. The successor's full live-in set is the demand source.
                let target = super::block(edge.edge().target().index() as usize)?;
                for variable in ssa.live_in(target).ok_or_else(mismatch)? {
                    out.budget.charge_work(3)?;
                    let local = variable.get() as usize;
                    if promoted.get(local) != Some(&true) {
                        return Err(mismatch());
                    }
                    if !edge_defined[local] && !killed[local] {
                        generated[local] = true;
                    }
                }
                for definition in definitions {
                    out.budget.charge_work(1)?;
                    edge_defined[definition.variable().get() as usize] = false;
                }
            }
            for row in &mut locals {
                out.budget.charge_work(3)?;
                row.current = current[row.local];
                row.scalar = if !promoted[row.local] {
                    ScalarDemand::Unavailable
                } else if generated[row.local] {
                    ScalarDemand::Needed(current[row.local].ok_or_else(mismatch)?)
                } else {
                    ScalarDemand::NotNeeded
                };
                for leaf in 0..row.components.len() {
                    out.budget.charge_work(1)?;
                    components[row.components.start + leaf] =
                        cache.prefix_leaf_v296(&bits, row.local, leaf, out)?;
                }
            }
            drop((
                bits,
                promoted,
                current,
                generated,
                killed,
                success_killed,
                edge_defined,
            ));
            let released = out
                .budget
                .storage()
                .checked_sub(retained)
                .ok_or(Resource::Accounting)?;
            out.budget.release_storage(released)?;
            Ok(Prefix {
                owner: self,
                frame: frame_index,
                block: block_index,
                statement,
                pc: cut.pc,
                locals,
                components,
                required: retained,
                charged: retained.checked_sub(floor).ok_or(Resource::Accounting)?,
            })
        };
        out.budget.reserve_storage(
            2 * std::mem::size_of_val(&derive) + std::mem::align_of_val(&derive),
        )?;
        self.slots.with_source_query_v42(out, derive)
    }
}

impl Prefix<'_, '_, '_, '_, '_> {
    pub(in super::super) fn check(
        &self,
        frames: &FramePlan<'_, '_, '_, '_>,
        out: &mut Writer<'_, '_>,
    ) -> Result<()> {
        self.owner
            .slots
            .check_query_storage_floor(self.required, out.budget)?;
        self.owner.check(self.owner.plan, self.owner.slots, out)?;
        out.budget.charge_work(1)?;
        if !std::ptr::eq(self.owner, frames) {
            return Err(mismatch());
        }
        Ok(())
    }

    /// This admits only complete local-demand coverage, not a frame invariant.
    pub(in super::super) fn require_complete_local_coverage(
        &self,
        out: &mut Writer<'_, '_>,
    ) -> Result<()> {
        self.check(self.owner, out)?;
        for row in &self.locals {
            out.budget.charge_work(1)?;
            if row.scalar == ScalarDemand::Unavailable {
                return Err(Error::Statement(
                    "partial prefix has unavailable all-local liveness; complete frame admission refused",
                ));
            }
        }
        Ok(())
    }

    pub(in super::super) fn component_required(
        &self,
        local: usize,
        leaf: usize,
        out: &mut Writer<'_, '_>,
    ) -> Result<bool> {
        self.check(self.owner, out)?;
        out.budget.charge_work(2)?;
        let row = self.locals.get(local).ok_or_else(mismatch)?;
        if leaf >= row.components.len() {
            return Err(mismatch());
        }
        Ok(self.components[row.components.start + leaf])
    }

    pub(in super::super) fn discard(self, out: &mut Writer<'_, '_>) -> Result<()> {
        self.check(self.owner, out)?;
        let charged = self.charged;
        drop(self);
        out.budget.release_storage(charged)?;
        Ok(())
    }
}

#[cfg(test)]
#[path = "original_semantic_mir_source_prefix_demands_v296_tests.rs"]
mod tests;
