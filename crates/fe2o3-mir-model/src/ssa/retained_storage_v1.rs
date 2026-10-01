//! Actual retained SSA-plan allocations; no planning, replay or identity work.
use super::{SsaConstructionPlanV1, SsaPlannerResourceReportV1};
use std::mem::size_of;

fn fixed<T: Copy>(_: &T) {}

fn vector<T, E>(
    values: &Vec<T>,
    visit: &mut impl FnMut(usize, usize) -> Result<(), E>,
) -> Result<(), E> {
    visit(values.capacity(), size_of::<T>())
}

fn leaves<T: Copy, E>(
    values: &Vec<T>,
    visit: &mut impl FnMut(usize, usize) -> Result<(), E>,
) -> Result<(), E> {
    vector(values, visit)
}

fn rows<T: Copy, E>(
    values: &Vec<Vec<T>>,
    visit: &mut impl FnMut(usize, usize) -> Result<(), E>,
) -> Result<(), E> {
    vector(values, visit)?;
    for row in values {
        leaves(row, visit)?;
    }
    Ok(())
}

fn edges<T: Copy, E>(
    values: &Vec<Vec<Vec<T>>>,
    visit: &mut impl FnMut(usize, usize) -> Result<(), E>,
) -> Result<(), E> {
    vector(values, visit)?;
    for block in values {
        rows(block, visit)?;
    }
    Ok(())
}

impl SsaConstructionPlanV1 {
    /// Visits every actual retained heap allocation as (element count, width).
    ///
    /// The first (0, 1) callback is this plan's root visit; every later callback
    /// is one Vec allocation, including empty vectors. Counts are actual
    /// capacities, including Vec slots containing nested Vec headers. Only
    /// initialized child Vec owners are traversed, never spare uninitialized
    /// slots. Copy-bounded leaf rows have no nested owned payload.
    ///
    /// The caller MUST checked-multiply count by width, checked-add totals,
    /// and charge an item for every callback before returning Ok. Refusal is
    /// propagated immediately, before any later child traversal. No complete
    /// preliminary scan or allocation occurs. A failed visitor may retain a
    /// partial ledger, which must not be reported as a complete observation.
    ///
    /// Excludes this plan's inline header, planner temporaries, source input,
    /// allocator metadata, peak and RSS. The enclosing owner counts its header
    /// once. The stored resource report is inline metadata, not extra bytes
    /// or an actual-capacity estimate. No admission, limits or identity change.
    pub fn visit_logical_retained_heap_v1<E>(
        &self,
        visit: &mut impl FnMut(usize, usize) -> Result<(), E>,
    ) -> Result<(), E> {
        visit(0, 1)?;
        let Self {
            identity,
            resources,
            reachable,
            reverse_postorder,
            promoted_variables,
            live_in,
            merge_variables,
            transport_variables,
            entry_definitions,
            entry_arguments,
            resolved_events,
            edge_definitions,
            edge_arguments,
        } = self;
        fixed(identity);
        let SsaPlannerResourceReportV1 {
            input_blocks,
            reachable_blocks,
            pruned_blocks,
            input_edges,
            input_events,
            input_edge_definitions,
            generated_definitions,
            output_items,
            storage_words,
            work_units,
        } = resources;
        fixed(input_blocks);
        fixed(reachable_blocks);
        fixed(pruned_blocks);
        fixed(input_edges);
        fixed(input_events);
        fixed(input_edge_definitions);
        fixed(generated_definitions);
        fixed(output_items);
        fixed(storage_words);
        fixed(work_units);
        leaves(reachable, visit)?;
        leaves(reverse_postorder, visit)?;
        leaves(promoted_variables, visit)?;
        rows(live_in, visit)?;
        rows(merge_variables, visit)?;
        rows(transport_variables, visit)?;
        leaves(entry_definitions, visit)?;
        leaves(entry_arguments, visit)?;
        rows(resolved_events, visit)?;
        edges(edge_definitions, visit)?;
        edges(edge_arguments, visit)
    }
}

#[cfg(test)]
#[path = "retained_storage_v1_tests.rs"]
mod tests;
