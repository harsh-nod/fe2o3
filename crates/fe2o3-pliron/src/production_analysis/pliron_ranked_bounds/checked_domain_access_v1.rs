// This fallback consumes only the already-verified immutable graph. Neither
// source terminal receipts nor the Checked capability enter its proof state.
fn checked_domain_bound_is_proven_v1(
    graph: &BoundsEdgeTransportV1<'_>,
    block: usize,
    index: Value,
    access_extent: (Value, usize, IndexExpr),
    budget: &mut RankedBoundsBudget,
) -> Result<bool, RankedBoundsFindingV1> {
    budget.work(32)?;
    let (view, dimension, extent) = access_extent;
    let Some(definition) = index.defining_op() else {
        return Ok(false);
    };
    let (geometry, operands, result, physical, parent) = {
        let raw = definition
            .try_deref(graph.context)
            .map_err(|_| RankedBoundsFindingV1::StructuralVerificationFailed)?;
        let (geometry, operands, result, physical) = if let Some(checked) =
            Operation::get_op::<CheckedRowStripedIndex2DOp>(definition, graph.context)
        {
            let geometry = checked
                .geometry(graph.context)
                .ok_or(RankedBoundsFindingV1::StructuralVerificationFailed)?;
            (
                CheckedDomainGeometryV1::RowStriped(geometry),
                checked.operands(graph.context),
                checked.result(graph.context),
                checked.physical_extent(graph.context),
            )
        } else if let Some(checked) =
            Operation::get_op::<CheckedTiledIndex2DOp>(definition, graph.context)
        {
            let geometry = checked
                .geometry(graph.context)
                .ok_or(RankedBoundsFindingV1::StructuralVerificationFailed)?;
            (
                CheckedDomainGeometryV1::Tiled(geometry),
                checked.operands(graph.context),
                checked.result(graph.context),
                checked.physical_extent(graph.context),
            )
        } else {
            return Ok(false);
        };
        (geometry, operands, result, physical, raw.get_parent_block())
    };
    if block >= graph.blocks.len()
        || graph.predecessors.len() != graph.blocks.len()
        || parent != Some(graph.blocks[block])
        || result != index
    {
        return Ok(false);
    }
    let Some(dag) = CheckedDomainDagV1::build(geometry, budget)? else {
        return Ok(false);
    };
    let mut proof = CheckedDomainProofV1::new(graph, dag, budget)?;
    proof.validate_view_extent(view, dimension, budget)?;
    let mut environment = [IndexExpr::Constant(0); 6];
    for (role, value) in operands.into_iter().enumerate() {
        budget.work(2)?;
        environment[role] = proof.canonical(value, budget)?;
    }
    if let IndexExpr::Value(value) | IndexExpr::Dimension { view: value, .. } = extent {
        proof.owned_value(value, budget)?;
    }
    environment[5] = extent;
    if let Some(physical) = physical {
        budget.work(8)?;
        if proof.canonical(physical, budget)? != extent {
            return Ok(false);
        }
    }
    for formula in 0..proof.dag.formula_count {
        budget.work(2)?;
        if proof.dag.required & (1 << formula) != 0 {
            let residual = CheckedDomainResidualV1::new(formula as u8, &proof.dag);
            if !proof.predicate(
                CheckedDomainStateV1 {
                    block,
                    environment,
                    goal: CheckedDomainGoalV1::Predicate(residual),
                },
                budget,
            )? {
                return Ok(false);
            }
        }
    }
    Ok(true)
}

fn checked_domain_resource_bound_v1(
    census: ProductionAnalysisInputCensusV1,
) -> Result<(usize, usize), ProductionAnalysisResourceLimitV1> {
    let error = "memory-bounds checked-domain upper bound";
    let queries = census.operands.min(checked_ranked_bounds_product_v1(
        census.ranked_accesses,
        MAX_RANKED_MEMORY_RANK,
        error,
    )?);
    if queries == 0 {
        return Ok((0, 0));
    }
    // Roster charges may include both argument/result rosters in an exact
    // edge substitution. Fixed-state scan, relocation, and setup are smaller.
    let roster = checked_ranked_bounds_sum_v1(&[census.block_arguments, census.results], error)?;
    let denied = checked_ranked_bounds_sum_v1(
        &[
            census.operands,
            checked_ranked_bounds_product_v1(roster, 2, error)?,
            checked_ranked_bounds_product_v1(
                MAX_RANKED_BOUNDS_FACTS,
                CHECKED_DOMAIN_STATE_ITEMS_V1 * 2,
                error,
            )?,
            CHECKED_DOMAIN_DAG_ITEMS_V1 * 32,
            CHECKED_DOMAIN_QUERY_ITEMS_V1 * 2,
            checked_ranked_bounds_product_v1(
                census.blocks,
                CHECKED_DOMAIN_TERMINATOR_ROW_ITEMS_V1 * 2,
                error,
            )?,
            5,
            CHECKED_DOMAIN_NODES_V1 * CHECKED_DOMAIN_FRAME_ITEMS_V1 * 4,
            BOUNDS_TRANSPORT_CANONICAL_WORK_V1 * 4 + 256,
        ],
        error,
    )?;
    let work = checked_ranked_bounds_sum_v1(&[MAX_RANKED_BOUNDS_WORK_UNITS, denied], error)?;
    // The runtime's shared cap admits each current/grown allocation before
    // it occurs and charges scratch cumulatively even after it is dropped.
    // This is a rejecting-prefix envelope, not a claim every query succeeds.
    Ok((work, MAX_RANKED_BOUNDS_STORAGE_ITEMS))
}
