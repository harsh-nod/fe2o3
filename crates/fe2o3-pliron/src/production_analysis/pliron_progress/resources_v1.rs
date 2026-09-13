fn progress_resource_error_v1(resource: &'static str) -> ProductionAnalysisResourceLimitV1 {
    ProductionAnalysisResourceLimitV1 {
        phase: ProductionAnalysisResourcePhaseV1::Progress,
        resource,
    }
}

fn checked_progress_sum_v1(
    values: &[usize],
    resource: &'static str,
) -> Result<usize, ProductionAnalysisResourceLimitV1> {
    values.iter().try_fold(0_usize, |total, value| {
        total
            .checked_add(*value)
            .ok_or_else(|| progress_resource_error_v1(resource))
    })
}

fn checked_progress_product_v1(
    lhs: usize,
    rhs: usize,
    resource: &'static str,
) -> Result<usize, ProductionAnalysisResourceLimitV1> {
    lhs.checked_mul(rhs)
        .ok_or_else(|| progress_resource_error_v1(resource))
}

/// Bounds recursive structural inventory, PLIRON structural verification,
/// classical dominator convergence, SCC construction, loop-shape
/// reconstruction, and the returned certificates before progress analysis
/// runs. PLIRON checks same-block dominance by walking from each operand's
/// defining operation toward its user, so that verifier phase needs the
/// explicit `operands * operations` term below. A dominator bit can disappear
/// at most once; the `blocks^2 + 1` factor therefore bounds even the least
/// favorable update order of the set-based fixed point.
#[cfg(test)]
pub(crate) fn preflight_progress_resource_upper_bound_v1(
    census: ProductionAnalysisInputCensusV1,
    limits: ProductionAnalysisResourceLimitsV1,
) -> Result<ProductionAnalysisResourceUpperBoundV1, ProductionAnalysisResourceLimitV1> {
    preflight_progress_execution_resource_upper_bound_v1(
        census,
        limits,
        ProgressVerifierCostV1::Standalone,
    )
}

/// Only the live callback-scoped production entry omits its redundant verifier
/// invocation. Identity capture and post-pass verification keep their existing
/// budgets; the conservative verifier workspace remains reserved here too.
pub(crate) fn preflight_scoped_progress_resource_upper_bound_v1(
    census: ProductionAnalysisInputCensusV1,
    limits: ProductionAnalysisResourceLimitsV1,
) -> Result<ProductionAnalysisResourceUpperBoundV1, ProductionAnalysisResourceLimitV1> {
    preflight_progress_execution_resource_upper_bound_v1(
        census,
        limits,
        ProgressVerifierCostV1::Scoped,
    )
}

enum ProgressVerifierCostV1 {
    #[cfg(test)]
    Standalone,
    Scoped,
}

fn preflight_progress_execution_resource_upper_bound_v1(
    census: ProductionAnalysisInputCensusV1,
    limits: ProductionAnalysisResourceLimitsV1,
    verifier: ProgressVerifierCostV1,
) -> Result<ProductionAnalysisResourceUpperBoundV1, ProductionAnalysisResourceLimitV1> {
    let phase = ProductionAnalysisResourcePhaseV1::Progress;
    if census.blocks > MAX_PLIRON_PROGRESS_BLOCKS_V1
        || census.operations > MAX_PLIRON_PROGRESS_OPERATIONS_V1
        || census.successors > MAX_PLIRON_PROGRESS_EDGES_V1
        || census.operands > MAX_PLIRON_PROGRESS_OPERANDS_V1
        || census.results > MAX_PLIRON_PROGRESS_RESULTS_V1
        || census.attributes > MAX_PLIRON_PROGRESS_ATTRIBUTES_V1
        || census.block_arguments > MAX_PLIRON_PROGRESS_BLOCK_ARGUMENTS_V1
    {
        return Err(progress_resource_error_v1("progress structural hard limit"));
    }
    let regions = census
        .operations
        .checked_add(1)
        .ok_or_else(|| progress_resource_error_v1("progress region upper bound"))?
        .min(MAX_PLIRON_PROGRESS_REGIONS_V1);
    let structural_items = checked_progress_sum_v1(
        &[
            regions,
            census.blocks,
            census.operations,
            census.operands,
            census.results,
            census.attributes,
            census.block_arguments,
            census.successors,
        ],
        "progress structural work upper bound",
    )?;
    let (verifier_dominance_work, scope_storage) = match verifier {
        #[cfg(test)]
        ProgressVerifierCostV1::Standalone => (
            checked_progress_product_v1(
                census.operands,
                census.operations,
                "progress verifier dominance work upper bound",
            )?,
            0,
        ),
        ProgressVerifierCostV1::Scoped => (
            0,
            crate::production_analysis::pliron_pass_contract::SCOPED_PROGRESS_INPUT_STORAGE_V1,
        ),
    };
    let charged_work = checked_progress_sum_v1(
        &[
            structural_items,
            verifier_dominance_work,
            checked_progress_product_v1(
                checked_progress_sum_v1(
                    &[census.blocks, census.successors],
                    "progress graph work upper bound",
                )?,
                8,
                "progress graph work upper bound",
            )?,
            checked_progress_product_v1(census.blocks, 4, "progress component work upper bound")?,
        ],
        "progress charged work upper bound",
    )?;
    if charged_work > MAX_PLIRON_PROGRESS_WORK_UNITS_V1 {
        return Err(progress_resource_error_v1("progress work hard limit"));
    }

    let block_square = checked_progress_product_v1(
        census.blocks,
        census.blocks,
        "progress dominator wave upper bound",
    )?;
    let dominator_waves = block_square
        .checked_add(1)
        .ok_or_else(|| progress_resource_error_v1("progress dominator wave upper bound"))?;
    let dominator_scan = checked_progress_sum_v1(
        &[
            block_square,
            checked_progress_product_v1(
                census.successors,
                census.blocks,
                "progress dominator scan upper bound",
            )?,
        ],
        "progress dominator scan upper bound",
    )?;
    let dominator_work = checked_progress_product_v1(
        dominator_waves,
        dominator_scan,
        "progress dominator work upper bound",
    )?;
    let loop_scan = checked_progress_sum_v1(
        &[
            census.blocks,
            census.successors,
            census.operands,
            census.operations,
        ],
        "progress loop scan upper bound",
    )?;
    let loop_candidates = census
        .blocks
        .checked_add(census.successors)
        .ok_or_else(|| progress_resource_error_v1("progress loop work upper bound"))?;
    let loop_work = checked_progress_product_v1(
        checked_progress_product_v1(
            census.blocks,
            loop_candidates,
            "progress loop work upper bound",
        )?,
        loop_scan,
        "progress loop work upper bound",
    )?;
    // At most E candidate backedges, each with two endpoint queries and
    // B propagation rounds visiting E edges. The all-edge helper adds one
    // bounded successor scan plus payload copy/equality/drop per query.
    // Each edge costs 6 visits for iteration, a two-word Ptr copy and its
    // owner/slot/generation comparison; 8 for accessor dispatch/counts and
    // fallback; and 10 for Option/Vec-header control, movement and disposal.
    // These controls are per edge, including unsupported nullary fanout.
    // Each payload Value costs four copied words, five exact equality fields
    // (UID, entity tag, owner, slot and generation), one visit and one disposal.
    let payload_queries = checked_progress_product_v1(
        census.successors,
        checked_progress_sum_v1(
            &[
                2,
                checked_progress_product_v1(
                    census.blocks,
                    census.successors,
                    "progress parallel-edge query bound",
                )?,
            ],
            "progress parallel-edge query bound",
        )?,
        "progress parallel-edge query bound",
    )?;
    let parallel_edge_work = checked_progress_product_v1(
        payload_queries,
        checked_progress_sum_v1(
            &[
                8,
                checked_progress_product_v1(census.successors, 24, "progress parallel-edge work")?,
                checked_progress_product_v1(census.operands, 11, "progress parallel-edge work")?,
            ],
            "progress parallel-edge work",
        )?,
        "progress parallel-edge work",
    )?;
    let parallel_edge_storage = if census.successors == 0 {
        0
    } else {
        // Existing workspace covers the returned payload and caller owners.
        // Only the saved first payload overlaps the next candidate: one
        // three-word header and four words per Value. Second nonempty
        // payloads use exact Range collectors; header moves do not clone them.
        checked_progress_sum_v1(
            &[
                3,
                checked_progress_product_v1(census.operands, 4, "progress parallel-edge storage")?,
            ],
            "progress parallel-edge storage",
        )?
    };
    let work = checked_progress_sum_v1(
        &[
            checked_progress_product_v1(structural_items, 4, "progress work upper bound")?,
            charged_work,
            dominator_work,
            loop_work,
            parallel_edge_work,
        ],
        "progress work upper bound",
    )?;

    let report_entries = census
        .blocks
        .checked_add(census.successors)
        .ok_or_else(|| progress_resource_error_v1("progress report upper bound"))?;
    let certificate_text = checked_progress_product_v1(
        NUMERIC_PROGRESS_LABEL_BYTES_V1,
        2,
        "progress report text upper bound",
    )?;
    let retained = checked_progress_sum_v1(
        &[
            checked_progress_product_v1(
                report_entries,
                MAX_PLIRON_PROGRESS_DIAGNOSTIC_BYTES_V1
                    .checked_add(16)
                    .ok_or_else(|| progress_resource_error_v1("progress report upper bound"))?,
                "progress report upper bound",
            )?,
            checked_progress_product_v1(
                census.successors,
                certificate_text,
                "progress report text upper bound",
            )?,
            census.blocks,
        ],
        "progress retained storage upper bound",
    )?;
    // The immutable verifier walker owns at most one item per inventoried
    // structural record. Its cached per-region dominator trees retain at most
    // the sum of the region block-count squares, bounded by `blocks^2`, while
    // def-use iteration has at most one live cursor per operand. These owners
    // are explicit here rather than being hidden in the later CFG workspace.
    let verifier_temporary = checked_progress_sum_v1(
        &[structural_items, block_square, census.operands],
        "progress verifier temporary storage upper bound",
    )?;
    let temporary = checked_progress_sum_v1(
        &[
            checked_progress_product_v1(
                structural_items,
                2,
                "progress temporary storage upper bound",
            )?,
            checked_progress_product_v1(
                census.blocks,
                14,
                "progress temporary storage upper bound",
            )?,
            checked_progress_product_v1(
                census.successors,
                8,
                "progress temporary storage upper bound",
            )?,
            verifier_temporary,
            scope_storage,
            parallel_edge_storage,
        ],
        "progress temporary storage upper bound",
    )?;
    let bound =
        ProductionAnalysisResourceUpperBoundV1::checked_phase(phase, work, retained, temporary)?;
    limits.require(phase, bound)
}
