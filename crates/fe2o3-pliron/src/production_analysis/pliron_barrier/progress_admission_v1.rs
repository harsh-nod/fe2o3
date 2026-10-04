// This probe predicts a possible dependency, never termination or convergence.
// Its bool is retained in the enclosing pipeline; no graph or report is cached.
// Borrowed census and 64 fixed cells cover both suspended probe frames,
// including raw-operation/successor iterators and the returned bound owner.
pub(crate) fn barrier_progress_probe_bound_v1(
    census: &ProductionAnalysisInputCensusV1,
) -> Result<ProductionAnalysisResourceUpperBoundV1, ProductionAnalysisResourceLimitV1> {
    let work = checked_barrier_sum_v1(&[
        96,
        checked_barrier_product_v1(census.operations, 6)?,
        checked_barrier_product_v1(census.blocks, 12)?,
        checked_barrier_product_v1(
            census.successors,
            checked_barrier_sum_v1(&[checked_barrier_product_v1(census.blocks, 2)?, 12])?,
        )?,
    ])?;
    ProductionAnalysisResourceUpperBoundV1::checked_phase(
        ProductionAnalysisResourcePhaseV1::BarrierConvergence,
        work,
        1,
        64,
    )
}

pub(crate) fn admit_barrier_progress_probe_v1(
    context: &Context,
    analyses: &mut PlironAnalysisManagerV1,
    census: &ProductionAnalysisInputCensusV1,
) -> Result<bool, ProductionAnalysisResourceLimitV1> {
    let bound = barrier_progress_probe_bound_v1(census)?;
    analyses.admit_retained_resource_upper_bound(
        ProductionAnalysisResourcePhaseV1::BarrierConvergence,
        bound,
    )?;
    // Borrow the trace discriminant. exact_trace() would clone its error.
    let exact_trace_succeeded = analyses.has_successful_exact_trace_v1();
    let inventory =
        analyses
            .function_inventory()
            .map_err(|_| ProductionAnalysisResourceLimitV1 {
                phase: ProductionAnalysisResourcePhaseV1::BarrierConvergence,
                resource: "barrier dependency inventory",
            })?;
    barrier_progress_may_be_needed_v1(context, inventory, exact_trace_succeeded, census)
}

fn barrier_progress_may_be_needed_v1(
    context: &Context,
    inventory: &crate::production_analysis::pliron_function_inventory::BoundedPlironFunctionInventoryV1,
    exact_trace_succeeded: bool,
    census: &ProductionAnalysisInputCensusV1,
) -> Result<bool, ProductionAnalysisResourceLimitV1> {
    let mismatch = || ProductionAnalysisResourceLimitV1 {
        phase: ProductionAnalysisResourcePhaseV1::BarrierConvergence,
        resource: "barrier dependency census mismatch",
    };
    let blocks = inventory.blocks();
    if blocks.len() > census.blocks || inventory.operations().len() > census.operations {
        return Err(mismatch());
    }
    if exact_trace_succeeded {
        return Ok(false);
    }
    if !inventory.operations().iter().any(|site| {
        Operation::get_op_dyn(site.pointer(), context)
            .downcast_ref::<BarrierOp>()
            .is_some()
    }) {
        return Ok(false);
    }
    let mut remaining_edges = census.successors;
    for (source, block) in blocks.iter().enumerate() {
        let Some(terminator) = block.deref(context).get_terminator(context) else {
            return Ok(true);
        };
        let raw = terminator.deref(context);
        let count = raw.get_num_successors();
        remaining_edges = remaining_edges.checked_sub(count).ok_or_else(mismatch)?;
        for successor in raw.successors() {
            // Order is the exact inventory row order, never labels or IDs.
            // Every directed cycle contains a non-forward edge in this order.
            match blocks.iter().position(|block| *block == successor) {
                Some(target) if target > source => {}
                _ => return Ok(true),
            }
        }
    }
    Ok(false)
}

pub(crate) fn compose_barrier_dependencies_v1(
    local: ProductionAnalysisResourceUpperBoundV1,
    pipeline: ProductionAnalysisResourceUpperBoundV1,
    progress: Option<ProductionAnalysisResourceUpperBoundV1>,
) -> Result<ProductionAnalysisResourceUpperBoundV1, ProductionAnalysisResourceLimitV1> {
    let phase = ProductionAnalysisResourcePhaseV1::BarrierConvergence;
    match progress {
        Some(progress) => local.checked_with_nested_sequence_discard(&[progress, pipeline], phase),
        None => local.checked_with_nested_sequence_discard(&[pipeline], phase),
    }
}
