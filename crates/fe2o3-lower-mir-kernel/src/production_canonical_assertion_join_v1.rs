fn block_at<'i, 'g>(
    inventory: &'i CanonicalKirInventoryV1<'g>,
    coordinate: fe2o3_kernel_ir::CanonicalKirBlockCoordinateV1,
) -> R<&'i fe2o3_kernel_analysis::CanonicalKirBlockRefV1<'g>> {
    let function = inventory
        .functions()
        .get(coordinate.function.0 as usize)
        .filter(|row| row.coordinate == coordinate.function)
        .ok_or_else(|| binding(None, "block function coordinate"))?;
    let index = function
        .blocks
        .start
        .checked_add(coordinate.block as usize)
        .ok_or(ArgumentResourceV1::Arithmetic)?;
    inventory
        .blocks()
        .get(index)
        .filter(|row| index < function.blocks.end && row.coordinate == coordinate)
        .ok_or_else(|| binding(None, "block coordinate"))
}
fn edge_at<'i, 'g>(
    inventory: &'i CanonicalKirInventoryV1<'g>,
    coordinate: fe2o3_kernel_ir::CanonicalKirEdgeCoordinateV1,
) -> R<&'i fe2o3_kernel_analysis::CanonicalKirEdgeRefV1<'g>> {
    let block = block_at(inventory, coordinate.source)?;
    let index = block
        .edges
        .start
        .checked_add(coordinate.successor as usize)
        .ok_or(ArgumentResourceV1::Arithmetic)?;
    inventory
        .edges()
        .get(index)
        .filter(|row| index < block.edges.end && row.coordinate == coordinate)
        .ok_or_else(|| binding(None, "edge coordinate"))
}
fn same_edge(
    a: &fe2o3_kernel_analysis::CanonicalKirEdgeRefV1<'_>,
    b: &fe2o3_kernel_analysis::CanonicalKirEdgeRefV1<'_>,
    budget: &mut ArgumentBudgetV1<'_>,
) -> R<bool> {
    budget.charge_work(argument_sum_v1(&[a.arguments.len(), b.arguments.len(), 4])?)?;
    Ok(a.coordinate == b.coordinate
        && a.target == b.target
        && a.target_id == b.target_id
        && a.arguments == b.arguments)
}
fn sparse_veto(value: CanonicalKirSparseValueV1, expected: bool, span: usize) -> R<()> {
    match value {
        CanonicalKirSparseValueV1::Constant(value) => {
            if value.ty() != ScalarType::Bool || value.bits() > 1 {
                return Err(binding(Some(span), "actual assertion value is not Boolean"));
            }
            if (value.bits() == 1) != expected {
                return Err(binding(
                    Some(span),
                    "actual Boolean contradicts proved source polarity",
                ));
            }
        }
        // These are not proofs. derive() has already required a real source Fact.
        CanonicalKirSparseValueV1::Unknown
        | CanonicalKirSparseValueV1::Dynamic
        | CanonicalKirSparseValueV1::Unreachable => {}
    }
    Ok(())
}
#[allow(clippy::too_many_arguments)]
fn join<'s, 'g: 's>(
    source: &ProductionCanonicalRankedMetadataV1<'_>,
    row: &ProductionCanonicalRankedAssertionV1,
    graph: &AssertGraphIndexV1<'_>,
    sparse: &CanonicalKirSparseV1<'_, '_>,
    policies: &impl AssertionTrapReaderV1<'s, 'g>,
    incoming: &mut [usize],
    budget: &mut ArgumentBudgetV1<'_>,
) -> R<()> {
    budget.charge_work(8)?;
    let source_row = &source.source.spans[row.span];
    let ProductionCanonicalRankedSourceSiteV1::Terminator { span, source: term } = source_row.site
    else {
        return Err(binding(Some(row.span), "assertion terminator source"));
    };
    let SemanticTerminatorKindV1::Assert {
        expected,
        target,
        unwind,
        ..
    } = term.kind()
    else {
        return Err(binding(Some(row.span), "actual Assert source"));
    };
    if target.role() != SemanticEdgeRoleV1::AssertSuccess
        || matches!(unwind, SemanticUnwindActionV1::Cleanup(_))
    {
        return Err(binding(Some(row.span), "unsupported assertion source edge"));
    }
    let fresh = source
        .owner
        .assert_origins()
        .assert_condition(
            span.correspondence_owner,
            span.semantic_function,
            span.semantic_block,
            budget,
        )
        .map_err(query_origin)?;
    if fresh != row.binding
        || fresh.expected() != *expected
        || fresh.semantic_success() != target.target()
        || fresh.block() != source_row.block
        || !sparse.belongs_to(source.inventory)
    {
        return Err(binding(Some(row.span), "original assertion attachment"));
    }
    let mut success_id = None;
    for candidate in &source.source.spans {
        budget.charge_work(1)?;
        if candidate.association != source_row.association {
            continue;
        }
        if let ProductionCanonicalRankedSourceSiteV1::Terminator { span: other, .. } =
            candidate.site
        {
            if other.semantic_block == target.target() {
                if success_id.replace(other.kernel_ir_block).is_some() {
                    return Err(binding(Some(row.span), "duplicate success source block"));
                }
            }
        }
    }
    let success_target = graph
        .block(
            source_row.block.function,
            success_id.ok_or_else(|| binding(Some(row.span), "missing success source block"))?,
            budget,
        )
        .map_err(query_origin)?;
    match fresh.outcome() {
        SemanticKirAssertConditionOutcomeV1::ElidedByExistingRule { success_edge } => {
            budget.charge_work(5)?;
            let success = edge_at(source.inventory, success_edge)?;
            let block = block_at(source.inventory, source_row.block)?;
            let Terminator::Branch { target, arguments } = block.terminator else {
                return Err(binding(
                    Some(row.span),
                    "elided assertion is not unconditional success",
                ));
            };
            budget.charge_work(arguments.len())?;
            if block.edges.len() != 1
                || success_edge.successor != 0
                || success.target != success_target
                || success.target_id != *target
                || success.arguments != arguments.as_slice()
            {
                return Err(binding(Some(row.span), "elided assertion success payload"));
            }
        }
        SemanticKirAssertConditionOutcomeV1::Emitted {
            condition_use,
            definition,
            success_edge,
            failure_edge,
        } => {
            budget.charge_work(9)?;
            let block = block_at(source.inventory, source_row.block)?;
            let Terminator::ConditionalBranch { condition, .. } = block.terminator else {
                return Err(binding(
                    Some(row.span),
                    "retained assertion is not conditional",
                ));
            };
            let actual = graph
                .definition(source_row.block.function, *condition, budget)
                .map_err(query_origin)?;
            let success = edge_at(source.inventory, success_edge)?;
            let failure = edge_at(source.inventory, failure_edge)?;
            if !actual.boolean
                || actual.coordinate != definition
                || success.target != success_target
                || success_edge.successor != u32::from(!*expected)
                || failure_edge.successor != u32::from(*expected)
                || !failure.arguments.is_empty()
                || condition_use
                    != (fe2o3_kernel_ir::CanonicalKirUseCoordinateV1::TerminatorOperand {
                        block: source_row.block,
                        operand: 0,
                    })
            {
                return Err(binding(
                    Some(row.span),
                    "actual assertion condition or ordered edges",
                ));
            }
            sparse_veto(
                sparse
                    .value_at_use(condition_use, budget)
                    .map_err(Failure::Sparse)?,
                *expected,
                row.span,
            )?;
            let mut found = None;
            for ordinal in 0..incoming.len() {
                let candidate = policies.incoming_edge(ordinal, budget)?;
                budget.charge_work(6)?;
                if candidate.failure().coordinate != failure_edge {
                    continue;
                }
                if found.replace(ordinal).is_some()
                    || candidate.condition().coordinate != condition_use
                    || candidate.condition().value != *condition
                    || candidate.definition().coordinate != definition
                    || candidate.definition().value != Some(*condition)
                    || *candidate.definition().ty != Type::BOOL
                    || candidate.success_when() != *expected
                    || !same_edge(candidate.success(), success, budget)?
                    || !same_edge(candidate.failure(), failure, budget)?
                {
                    return Err(binding(
                        Some(row.span),
                        "trap pair differs from source assertion",
                    ));
                }
            }
            let ordinal =
                found.ok_or_else(|| binding(Some(row.span), "source failure has no trap pair"))?;
            incoming[ordinal] = incoming[ordinal]
                .checked_add(1)
                .ok_or(ArgumentResourceV1::Arithmetic)?;
        }
    }
    Ok(())
}

fn join_synthetic<'s, 'g: 's>(
    source: &ProductionCanonicalRankedMetadataV1<'_>,
    policies: &impl AssertionTrapReaderV1<'s, 'g>,
    coverage: &mut Coverage<'_>,
    budget: &mut ArgumentBudgetV1<'_>,
) -> R<()> {
    let pairs = policies.pair_count(budget)?;
    for (span, source_row) in source.source.spans.iter().enumerate() {
        budget.charge_work(1)?;
        let ProductionCanonicalRankedSourceSiteV1::Synthetic(synthetic) = source_row.site else {
            continue;
        };
        if synthetic.rule() != SemanticKirSyntheticOperationRuleV1::RuntimeAssertFailureTrap {
            continue;
        }
        let mut found = None;
        for pair in 0..pairs {
            let pair = policies.pair(pair, budget)?;
            budget.charge_work(4)?;
            if pair.terminal_block() != source_row.block {
                continue;
            }
            if found.replace(pair).is_some()
                || source_row.operations.len() != 1
                || source.inventory.operations()[source_row.operations.start].coordinate
                    != pair.call()
                || synthetic.operation_count() != 1
                || synthetic.first_operation_ordinal() != 0
            {
                return Err(binding(Some(span), "synthetic trap source span"));
            }
        }
        let pair =
            found.ok_or_else(|| binding(Some(span), "synthetic trap has no complete pair"))?;
        // Every source association of a shared sink must cover all its incoming edges.
        for incoming in pair.incoming_edges() {
            let edge = policies
                .incoming_edge(incoming, budget)?
                .failure()
                .coordinate;
            let mut count = 0usize;
            for alias in &source.contracts.assertions {
                budget.charge_work(2)?;
                if source.source.spans[alias.span].association == source_row.association
                    && matches!(alias.binding.outcome(), SemanticKirAssertConditionOutcomeV1::Emitted { failure_edge, .. } if failure_edge == edge)
                    && coverage.rows[alias.span].is_some()
                {
                    count += 1;
                }
            }
            if count != 1 {
                return Err(binding(Some(span), "shared trap sink alias coverage"));
            }
        }
        coverage.synthetic[span] = true;
    }
    for ordinal in 0..pairs {
        let pair = policies.pair(ordinal, budget)?;
        for (association, group) in source.calls.groups.iter().enumerate() {
            budget.charge_work(1)?;
            if group.function.canonical.coordinate != pair.call().block.function {
                continue;
            }
            let mut count = 0usize;
            for (span, row) in source.source.spans.iter().enumerate() {
                budget.charge_work(1)?;
                if row.association == association
                    && row.block == pair.terminal_block()
                    && coverage.synthetic[span]
                {
                    count += 1;
                }
            }
            if count != 1 {
                return Err(binding(None, "graph trap is missing its source alias"));
            }
        }
    }
    Ok(())
}
