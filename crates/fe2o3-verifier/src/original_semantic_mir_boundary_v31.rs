//! Adopts the shared original SSA equations inside the bounded proof generator.
//! Actual source topology and emitted control-flow joins remain the caller's duty.

use super::{Error, Resource, Result, Writer, vector};
use fe2o3_kernel_analysis::{
    SourceSsaBlockEventsV299 as BlockEvents, SourceSsaBoundariesV31 as Checked,
    SourceSsaBoundaryErrorV31 as BoundaryError, SourceSsaBoundaryStorageV31 as Storage,
};
use fe2o3_mir_model::semantic_mir_v1::{SemanticBasicBlockV1, SemanticFunctionIdV1 as FunctionId};
use fe2o3_mir_model::{
    SsaBlockIdV1 as Block, SsaConstructionPlanV1 as Plan, SsaValueV1 as Value,
    SsaVariableIdV1 as Variable,
};
use fe2o3_pliron::{
    ProductionSemanticSsaEventOccurrenceV1,
    ProductionSemanticSsaFunctionOccurrencesV1 as Occurrences,
    ProductionSemanticSsaOccurrenceViewV1, ProductionSemanticSsaOwnerV1 as Owner,
};
use std::mem::size_of;

pub(super) struct ControlInput<'a> {
    pub entry: Block,
    pub successors: &'a [Vec<Block>],
}

pub(super) struct Boundaries<'a> {
    plan: &'a Plan,
    checked: Checked<'a>,
}

fn error(error: BoundaryError) -> Error {
    match error {
        BoundaryError::Resource(resource) => Error::Resource(resource),
        BoundaryError::Statement(detail) => Error::Statement(detail),
        BoundaryError::ForeignPlan => {
            Error::Statement("original MIR SSA boundary has a foreign plan")
        }
        BoundaryError::Panicked => {
            Error::Statement("original MIR SSA boundary derivation panicked")
        }
    }
}

fn headers() -> usize {
    size_of::<Boundaries<'_>>()
        + size_of::<Result<Boundaries<'_>>>()
        + size_of::<ControlInput<'_>>()
        + size_of::<Storage>()
        + size_of::<std::result::Result<(Checked<'_>, Storage), BoundaryError>>()
        + size_of::<std::result::Result<Value, BoundaryError>>()
        + size_of::<Result<Value>>()
        + size_of::<Result<()>>()
        + size_of::<&Plan>()
        + size_of::<&mut Writer<'_, '_>>()
        + size_of::<Block>()
        + size_of::<Variable>()
}

fn source_headers_v299() -> usize {
    size_of::<Vec<BlockEvents>>()
        + size_of::<Result<Vec<BlockEvents>>>()
        + size_of::<Occurrences<'_>>()
        + size_of::<Option<Occurrences<'_>>>()
        + size_of::<ProductionSemanticSsaOccurrenceViewV1<'_>>()
        + size_of::<Option<ProductionSemanticSsaOccurrenceViewV1<'_>>>()
        + size_of::<Option<&[ProductionSemanticSsaEventOccurrenceV1]>>()
        + size_of::<std::iter::Enumerate<std::slice::Iter<'_, SemanticBasicBlockV1>>>()
        + size_of::<(&mut usize, &mut Writer<'_, '_>, &ControlInput<'_>, &usize)>()
        + std::mem::align_of::<(&mut usize, &mut Writer<'_, '_>, &ControlInput<'_>, &usize)>()
        + size_of::<BlockEvents>()
        + size_of::<Option<BlockEvents>>()
        + size_of::<FunctionId>()
        + size_of::<&Owner>()
        + 6 * size_of::<usize>()
        + 6 * size_of::<&()>()
        + size_of::<Result<()>>()
}

impl<'a> Boundaries<'a> {
    pub(super) fn check_plan_v281(&self, plan: &Plan, out: &mut Writer<'_, '_>) -> Result<()> {
        out.budget.charge_work(1)?;
        if !std::ptr::eq(self.plan, plan) {
            return Err(error(BoundaryError::ForeignPlan));
        }
        Ok(())
    }

    pub(super) fn derive(
        plan: &'a Plan,
        input: ControlInput<'a>,
        out: &mut Writer<'_, '_>,
    ) -> Result<Self> {
        out.budget.reserve_storage(headers())?;
        let (checked, storage) =
            Checked::derive(plan, input.entry, input.successors, out.budget).map_err(error)?;
        // The shared checker restores its scratch floor and returns an
        // unreserved owner. Its actual retained capacity is adopted here.
        out.budget.reserve_storage(storage.retained_storage())?;
        Ok(Self { plan, checked })
    }

    pub(super) fn derive_source_v299(
        plan: &'a Plan,
        input: ControlInput<'a>,
        owner: &Owner,
        function: FunctionId,
        out: &mut Writer<'_, '_>,
    ) -> Result<Self> {
        out.budget
            .reserve_storage(headers() + source_headers_v299())?;
        out.budget.charge_work(4)?;
        let mismatch = || error(BoundaryError::ForeignPlan);
        if !owner
            .plan_for_function(function)
            .is_some_and(|row| std::ptr::eq(row.plan(), plan))
        {
            return Err(mismatch());
        }
        let occurrences = owner
            .occurrences_v1()
            .and_then(|rows| rows.function(function))
            .ok_or_else(mismatch)?;
        let source = owner
            .source_semantic()
            .functions()
            .get(function.index() as usize)
            .ok_or_else(mismatch)?;
        if occurrences.block_count_v299() != input.successors.len()
            || source.blocks().len() != input.successors.len()
            || source.entry().index() != input.entry.get()
        {
            return Err(mismatch());
        }
        let mut blocks = vector(input.successors.len(), out)?;
        for (index, body) in source.blocks().iter().enumerate() {
            out.budget.charge_work(3)?;
            let block = Block::new(u32::try_from(index).map_err(|_| Resource::Arithmetic)?);
            let events = occurrences.block_events_v299(block).ok_or_else(mismatch)?;
            let mut edge = 0usize;
            body.terminator().kind().try_for_each_edge(|successor| {
                out.budget.charge_work(1)?;
                if input.successors[index].get(edge).map(|target| target.get())
                    != Some(successor.target().index())
                {
                    return Err(mismatch());
                }
                edge = edge.checked_add(1).ok_or(Resource::Arithmetic)?;
                Ok::<_, Error>(())
            })?;
            if edge != input.successors[index].len() {
                return Err(mismatch());
            }
            // The private attachment already joins all source events, including
            // unpromoted rows, to this exact immutable owner and SSA plan.
            blocks.push(BlockEvents {
                events: events.len(),
                terminal_failure_start: occurrences.terminal_failure_start(block),
            });
        }
        let (checked, storage) = Checked::derive_with_terminal_failures_v299(
            plan,
            input.entry,
            input.successors,
            &blocks,
            out.budget,
        )
        .map_err(error)?;
        out.budget.reserve_storage(storage.retained_storage())?;
        let scratch = blocks
            .capacity()
            .checked_mul(size_of::<BlockEvents>())
            .ok_or(Resource::Arithmetic)?;
        drop(blocks);
        out.budget.release_storage(scratch)?;
        Ok(Self { plan, checked })
    }

    pub(super) fn value(
        &self,
        block: Block,
        variable: Variable,
        out: &mut Writer<'_, '_>,
    ) -> Result<Value> {
        self.checked
            .value(self.plan, block, variable, out.budget)
            .map_err(error)
    }
}

#[cfg(test)]
#[path = "original_semantic_mir_boundary_v31_tests.rs"]
mod tests;
