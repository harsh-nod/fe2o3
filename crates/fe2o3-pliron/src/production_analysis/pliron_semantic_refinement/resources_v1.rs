fn semantic_resource_overflow_v1() -> ProductionAnalysisResourceLimitV1 {
    ProductionAnalysisResourceLimitV1 {
        phase: ProductionAnalysisResourcePhaseV1::SemanticRefinement,
        resource: "semantic refinement resource upper bound",
    }
}

fn checked_semantic_sum_v1(values: &[usize]) -> Result<usize, ProductionAnalysisResourceLimitV1> {
    values.iter().try_fold(0_usize, |total, value| {
        total
            .checked_add(*value)
            .ok_or_else(semantic_resource_overflow_v1)
    })
}

fn checked_semantic_product_v1(
    lhs: usize,
    rhs: usize,
) -> Result<usize, ProductionAnalysisResourceLimitV1> {
    lhs.checked_mul(rhs)
        .ok_or_else(semantic_resource_overflow_v1)
}

fn semantic_finding_cardinalities_v1(
    semantic_operations: usize,
) -> Result<(usize, usize), ProductionAnalysisResourceLimitV1> {
    let attempts = checked_semantic_product_v1(semantic_operations, 2)?;
    let retained = if attempts > MAX_PLIRON_SEMANTIC_FINDINGS_V1 {
        MAX_PLIRON_SEMANTIC_FINDINGS_V1
            .checked_add(1)
            .ok_or_else(semantic_resource_overflow_v1)?
    } else {
        attempts
    };
    Ok((attempts, retained))
}

/// Charges canonical expression reconstruction, proof-record joins, typed
/// commitments, numerical certificates, and bounded diagnostics. Nested
/// progress/effect analyses retain their independently admitted resources.
pub(crate) fn preflight_semantic_refinement_resource_upper_bound_v1(
    census: ProductionAnalysisInputCensusV1,
    limits: ProductionAnalysisResourceLimitsV1,
) -> Result<ProductionAnalysisResourceUpperBoundV1, ProductionAnalysisResourceLimitV1> {
    let semantic_operations = census
        .semantic_definitions
        .checked_add(census.semantic_refinement_contracts)
        .ok_or_else(semantic_resource_overflow_v1)?;
    let nodes = census
        .semantic_definitions
        .min(MAX_PLIRON_SEMANTIC_NODES_V1);
    let semantic_node_cost = MAX_SEMANTIC_TYPED_EXPRESSION_NODES_V1
        .checked_mul(2)
        .and_then(|items| items.checked_add(16))
        .ok_or_else(semantic_resource_overflow_v1)?;
    let reconstruction_work = checked_semantic_product_v1(
        checked_semantic_product_v1(
            nodes,
            nodes
                .checked_add(1)
                .ok_or_else(semantic_resource_overflow_v1)?,
        )?,
        semantic_node_cost,
    )?;
    let joins = checked_semantic_product_v1(semantic_operations, semantic_operations)?;
    let (finding_attempts, findings) = semantic_finding_cardinalities_v1(semantic_operations)?;
    let finding_payload = MAX_PLIRON_SEMANTIC_DIAGNOSTIC_BYTES_V1
        .checked_mul(2)
        .and_then(|bytes| bytes.checked_add(32))
        .and_then(|bytes| bytes.checked_add(census.identifier_bytes))
        .ok_or_else(semantic_resource_overflow_v1)?;
    let work = checked_semantic_sum_v1(&[
        reconstruction_work,
        checked_semantic_product_v1(joins, 8)?,
        // Every operation is classified once. Only the authenticated semantic
        // subset participates in the later contract and expression passes.
        census.operations,
        checked_semantic_product_v1(semantic_operations, 15)?,
        // A contract can emit one proof-correlation finding and one later
        // expression/layout finding. Attempts after the retained cap are
        // still fully constructed before `push` discards them.
        checked_semantic_product_v1(finding_attempts, finding_payload)?,
    ])?;
    let retained = checked_semantic_sum_v1(&[
        checked_semantic_product_v1(findings, finding_payload)?,
        checked_semantic_product_v1(nodes, 4)?,
        checked_semantic_product_v1(semantic_operations, 6)?,
    ])?;
    let temporary = checked_semantic_sum_v1(&[
        checked_semantic_product_v1(nodes, semantic_node_cost)?,
        checked_semantic_product_v1(
            semantic_operations,
            census
                .max_operation_arity
                .checked_mul(3)
                .and_then(|items| items.checked_add(48))
                .ok_or_else(semantic_resource_overflow_v1)?,
        )?,
        checked_semantic_product_v1(semantic_operations, 8)?,
        // Once the retained cap is full, one complete candidate finding can
        // coexist with it while `push` selects or preserves the cap marker.
        if finding_attempts == 0 {
            0
        } else {
            finding_payload
        },
    ])?;
    let bound = ProductionAnalysisResourceUpperBoundV1::checked_phase(
        ProductionAnalysisResourcePhaseV1::SemanticRefinement,
        work,
        retained,
        temporary,
    )?;
    limits.require(ProductionAnalysisResourcePhaseV1::SemanticRefinement, bound)
}
