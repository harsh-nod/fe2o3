use crate::production_analysis::pliron_pipeline::invocation_receipt_v1::{
    InvocationObserverV1, observe_resource_preflight_v1, require_observed_v1,
};

fn sparse_index_resource_error_v1(resource: &'static str) -> ProductionAnalysisResourceLimitV1 {
    ProductionAnalysisResourceLimitV1 {
        phase: ProductionAnalysisResourcePhaseV1::SparseIndex,
        resource,
    }
}

fn checked_sparse_index_sum_v1(
    values: &[usize],
    resource: &'static str,
) -> Result<usize, ProductionAnalysisResourceLimitV1> {
    values.iter().try_fold(0_usize, |total, value| {
        total
            .checked_add(*value)
            .ok_or_else(|| sparse_index_resource_error_v1(resource))
    })
}

fn checked_sparse_index_product_v1(
    lhs: usize,
    rhs: usize,
    resource: &'static str,
) -> Result<usize, ProductionAnalysisResourceLimitV1> {
    lhs.checked_mul(rhs)
        .ok_or_else(|| sparse_index_resource_error_v1(resource))
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct SparseIndexMergeCensusV1 {
    inputs: usize,
    squared_inputs: usize,
    argument_type_work: usize,
}

fn sparse_index_merge_census_work_v1(
    census: ProductionAnalysisInputCensusV1,
) -> Result<usize, ProductionAnalysisResourceLimitV1> {
    census
        .blocks
        .checked_add(1)
        .ok_or_else(|| sparse_index_resource_error_v1("sparse-index merge census work upper bound"))
}

// The structural-identity owner has already verified this exact immutable
// function. num_preds reads the existing successor-use count without copying
// a predecessor roster; duplicate and unreachable edges conservatively count.
#[cfg(test)]
fn collect_sparse_index_merge_census_v1(
    context: &Context,
    function: &FuncOp,
    census: ProductionAnalysisInputCensusV1,
    limits: ProductionAnalysisResourceLimitsV1,
) -> Result<SparseIndexMergeCensusV1, ProductionAnalysisResourceLimitV1> {
    collect_sparse_index_merge_census_observed_v1(context, function, census, limits, None)
}

fn collect_sparse_index_merge_census_observed_v1(
    context: &Context,
    function: &FuncOp,
    census: ProductionAnalysisInputCensusV1,
    limits: ProductionAnalysisResourceLimitsV1,
    observer: Option<&InvocationObserverV1<'_, '_>>,
) -> Result<SparseIndexMergeCensusV1, ProductionAnalysisResourceLimitV1> {
    let phase = ProductionAnalysisResourcePhaseV1::SparseIndex;
    let scan_work = sparse_index_merge_census_work_v1(census)?;
    require_observed_v1(
        limits,
        phase,
        ProductionAnalysisResourceUpperBoundV1::checked_phase(phase, scan_work, 0, 0),
        observer,
    )?;
    let mut result = SparseIndexMergeCensusV1::default();
    let mut blocks = 0_usize;
    let mut arguments = 0_usize;
    for block in function.get_region(context).deref(context).iter(context) {
        if blocks == census.blocks {
            return Err(sparse_index_resource_error_v1(
                "sparse-index merge census mismatch",
            ));
        }
        blocks += 1;
        let arity = block.deref(context).get_num_arguments();
        arguments = checked_sparse_index_sum_v1(
            &[arguments, arity],
            "sparse-index merge census arguments",
        )?;
        // Pinned Pliron finds the type of argument i by scanning i+1 entries.
        // Divide before multiplying so the triangular bound does not overflow
        // when the mathematically representable result still fits usize.
        let next = arity.checked_add(1).ok_or_else(|| {
            sparse_index_resource_error_v1("sparse-index argument type work upper bound")
        })?;
        let (lhs, rhs) = if arity.is_multiple_of(2) {
            (arity / 2, next)
        } else {
            (arity, next / 2)
        };
        result.argument_type_work = checked_sparse_index_sum_v1(
            &[
                result.argument_type_work,
                checked_sparse_index_product_v1(
                    lhs,
                    rhs,
                    "sparse-index argument type work upper bound",
                )?,
            ],
            "sparse-index argument type work upper bound",
        )?;
        if arity == 0 {
            continue;
        }
        let predecessors = block.num_preds(context);
        let inputs = checked_sparse_index_product_v1(
            arity,
            predecessors,
            "sparse-index merge input upper bound",
        )?;
        result.inputs = checked_sparse_index_sum_v1(
            &[result.inputs, inputs],
            "sparse-index merge input upper bound",
        )?;
        let squared_inputs = checked_sparse_index_product_v1(
            inputs,
            predecessors,
            "sparse-index merge rescan upper bound",
        )?;
        result.squared_inputs = checked_sparse_index_sum_v1(
            &[result.squared_inputs, squared_inputs],
            "sparse-index merge rescan upper bound",
        )?;
    }
    if blocks != census.blocks || arguments != census.block_arguments {
        return Err(sparse_index_resource_error_v1(
            "sparse-index merge census mismatch",
        ));
    }
    Ok(result)
}

/// Bounds the dense lattice and dependency graph before allocating them. The
/// immutable function is the one authenticated by the caller's identity census.
pub(crate) fn preflight_sparse_index_resource_upper_bound_v1(
    context: &Context,
    function: &FuncOp,
    census: ProductionAnalysisInputCensusV1,
    limits: ProductionAnalysisResourceLimitsV1,
) -> Result<ProductionAnalysisResourceUpperBoundV1, ProductionAnalysisResourceLimitV1> {
    preflight_sparse_index_resource_upper_bound_with_observation_v1(
        context, function, census, limits, None,
    )
}

pub(crate) fn preflight_sparse_index_resource_upper_bound_with_observation_v1(
    context: &Context,
    function: &FuncOp,
    census: ProductionAnalysisInputCensusV1,
    limits: ProductionAnalysisResourceLimitsV1,
    observer: Option<&InvocationObserverV1<'_, '_>>,
) -> Result<ProductionAnalysisResourceUpperBoundV1, ProductionAnalysisResourceLimitV1> {
    observe_resource_preflight_v1(observer, |observer| {
        checked_sparse_index_population_v1(census)?;
        let merges = collect_sparse_index_merge_census_observed_v1(
            context, function, census, limits, observer,
        )?;
        sparse_index_resource_upper_bound_observed_v1(census, merges, limits, observer)
    })
}

fn checked_sparse_index_population_v1(
    census: ProductionAnalysisInputCensusV1,
) -> Result<(usize, usize), ProductionAnalysisResourceLimitV1> {
    let values = census
        .results
        .checked_add(census.block_arguments)
        .ok_or_else(|| sparse_index_resource_error_v1("sparse-index value upper bound"))?;
    let input_uses = census
        .operands
        .checked_add(census.successors)
        .ok_or_else(|| sparse_index_resource_error_v1("sparse-index input-use upper bound"))?;
    let dependency_uses = checked_sparse_index_sum_v1(
        &[
            census.operands,
            census.operands,
            census.operations,
            census.block_arguments,
        ],
        "sparse-index dependency-use upper bound",
    )?;
    if values > MAX_SPARSE_INDEX_VALUES_V1 {
        return Err(sparse_index_resource_error_v1(
            "sparse-index value hard limit",
        ));
    }
    if input_uses > MAX_SPARSE_INDEX_USES_V1 || dependency_uses > MAX_SPARSE_INDEX_USES_V1 {
        return Err(sparse_index_resource_error_v1(
            "sparse-index use hard limit",
        ));
    }
    Ok((values, dependency_uses))
}

#[cfg(test)]
fn sparse_index_resource_upper_bound_v1(
    census: ProductionAnalysisInputCensusV1,
    merges: SparseIndexMergeCensusV1,
    limits: ProductionAnalysisResourceLimitsV1,
) -> Result<ProductionAnalysisResourceUpperBoundV1, ProductionAnalysisResourceLimitV1> {
    sparse_index_resource_upper_bound_observed_v1(census, merges, limits, None)
}

fn sparse_index_resource_upper_bound_observed_v1(
    census: ProductionAnalysisInputCensusV1,
    merges: SparseIndexMergeCensusV1,
    limits: ProductionAnalysisResourceLimitsV1,
    observer: Option<&InvocationObserverV1<'_, '_>>,
) -> Result<ProductionAnalysisResourceUpperBoundV1, ProductionAnalysisResourceLimitV1> {
    let phase = ProductionAnalysisResourcePhaseV1::SparseIndex;
    let (values, dependency_uses) = checked_sparse_index_population_v1(census)?;

    let structural_work = checked_sparse_index_sum_v1(
        &[
            census.blocks,
            census.operations,
            census.operands,
            census.results,
            census.successors,
            census.block_arguments,
        ],
        "sparse-index structural work upper bound",
    )?;
    // Numeric publication remains capped at two; copy roots have height two.
    // Together they schedule <=4D consumer visits and <=V+4D pops. Each pop
    // charges four dispatch/publication units and each merge input two units.
    // Root setup/export/drop costs <=56V+16; final Pending
    // closure costs V. Including numeric counters gives 62V+20D+2M+8M2.
    let propagation_work = checked_sparse_index_sum_v1(
        &[
            merges.argument_type_work,
            census.blocks,
            census.successors,
            checked_sparse_index_product_v1(
                values,
                62,
                "sparse-index propagation work upper bound",
            )?,
            checked_sparse_index_product_v1(
                dependency_uses,
                20,
                "sparse-index propagation work upper bound",
            )?,
            checked_sparse_index_product_v1(
                merges.inputs,
                2,
                "sparse-index merge work upper bound",
            )?,
            checked_sparse_index_product_v1(
                merges.squared_inputs,
                8,
                "sparse-index merge rescan work upper bound",
            )?,
            16,
        ],
        "sparse-index propagation work upper bound",
    )?;
    if propagation_work > MAX_SPARSE_INDEX_WORK_UNITS_V1 {
        return Err(sparse_index_resource_error_v1(
            "sparse-index propagation hard limit",
        ));
    }
    let work = checked_sparse_index_sum_v1(
        &[
            checked_sparse_index_product_v1(structural_work, 8, "sparse-index work upper bound")?,
            sparse_index_merge_census_work_v1(census)?,
            propagation_work,
        ],
        "sparse-index work upper bound",
    )?;

    // Each returned fact owns at most a fixed-rank affine/checked-index
    // payload. All CFG, dependency, lattice, and worklist storage is temporary.
    let retained = checked_sparse_index_sum_v1(
        &[
            checked_sparse_index_product_v1(
                values,
                MAX_RANKED_MEMORY_RANK + 20,
                "sparse-index retained storage upper bound",
            )?,
            MAX_RANKED_MEMORY_RANK * 2 + 8,
        ],
        "sparse-index retained storage upper bound",
    )?;
    let temporary = checked_sparse_index_sum_v1(
        &[
            checked_sparse_index_product_v1(
                values,
                57,
                "sparse-index temporary storage upper bound",
            )?,
            // Exact-capacity dense TypeHandle sidecar, coexisting with all facts.
            checked_sparse_index_product_v1(
                values,
                std::mem::size_of::<pliron::r#type::TypeHandle>()
                    .div_ceil(std::mem::size_of::<usize>()),
                "sparse-index type storage upper bound",
            )?,
            checked_sparse_index_product_v1(
                census.blocks,
                8,
                "sparse-index temporary storage upper bound",
            )?,
            checked_sparse_index_product_v1(
                census.successors,
                4,
                "sparse-index temporary storage upper bound",
            )?,
            checked_sparse_index_product_v1(
                census.operands,
                4,
                "sparse-index temporary storage upper bound",
            )?,
            checked_sparse_index_product_v1(
                census.operations,
                2,
                "sparse-index temporary storage upper bound",
            )?,
            6,
        ],
        "sparse-index temporary storage upper bound",
    )?;
    let bound =
        ProductionAnalysisResourceUpperBoundV1::checked_phase(phase, work, retained, temporary)?;
    require_observed_v1(limits, phase, Ok(bound), observer)
}
