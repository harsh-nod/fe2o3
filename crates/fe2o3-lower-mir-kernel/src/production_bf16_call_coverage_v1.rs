// Complete source/block/statement/terminator coverage, with the ordinary root
// ABI checker on the SAME ledger. Nominal helper parameters have separate
// exact positional checks; they never masquerade as aggregate projections.
fn bf16_complete_source_coverage_v1(
    source: &CheckedBf16CallInstanceV1<'_>,
    module: &Module,
    rows: &SemanticKirCorrespondenceV1,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    let semantic = source.owner().source_semantic();
    let mut block_count = 0usize;
    let mut statement_count = 0usize;
    let mut return_count = 0usize;
    for id in [source.root(), source.helper()] {
        let role = if id == source.root() {
            SemanticKirFunctionRoleV1::KernelEntry
        } else {
            SemanticKirFunctionRoleV1::InternalHelper
        };
        let function = bf16_function_v1(module, rows, source.root(), id, role, budget)?;
        let declaration = &semantic.functions()[id.index() as usize];
        let plan = source
            .owner()
            .plan_for_function(id)
            .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
        let body = function
            .body
            .as_ref()
            .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
        if body.blocks.len() != plan.plan().reverse_postorder().len() {
            return Err(bf16_emission_refusal_v1(
                "BF16 source/canonical block bijection",
            ));
        }
        for source_block in plan.plan().reverse_postorder() {
            let block_id = SemanticBlockIdV1::from_index(source_block.get());
            let original = declaration
                .blocks()
                .get(source_block.get() as usize)
                .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
            budget.charge_work(body.blocks.len() + rows.blocks.len())?;
            let actual = body
                .blocks
                .iter()
                .find(|b| b.id == BlockId(source_block.get()))
                .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
            if rows
                .blocks
                .iter()
                .filter(|b| {
                    b.correspondence_owner == source.root()
                        && b.semantic_function == id
                        && b.semantic_block == block_id
                        && b.kernel_ir_block == actual.id
                        && b.source_statement_count as usize == original.statements().len()
                })
                .count()
                != 1
            {
                return Err(bf16_emission_refusal_v1("BF16 source block identity"));
            }
            let mut end = 0usize;
            for index in 0..original.statements().len() {
                budget.charge_work(rows.statement_operation_spans.len())?;
                let mut spans = rows.statement_operation_spans.iter().filter(|s| {
                    s.correspondence_owner == source.root()
                        && s.semantic_function == id
                        && s.semantic_block == block_id
                        && s.statement_ordinal as usize == index
                });
                let span = spans
                    .next()
                    .filter(|_| spans.next().is_none())
                    .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
                if span.kernel_ir_block != actual.id || span.first_operation_ordinal as usize != end
                {
                    return Err(bf16_emission_refusal_v1("BF16 source statement partition"));
                }
                end = end
                    .checked_add(span.operation_count as usize)
                    .ok_or(ArgumentResourceV1::Arithmetic)?;
                statement_count += 1;
            }
            let span = bf16_source_span_v1(rows, source.root(), id, block_id, budget)?;
            if span.kernel_ir_block != actual.id
                || span.first_operation_ordinal as usize != end
                || end.checked_add(span.operation_count as usize) != Some(actual.operations.len())
            {
                return Err(bf16_emission_refusal_v1(
                    "BF16 complete terminator partition",
                ));
            }
            if matches!(
                original.terminator().kind(),
                SemanticTerminatorKindV1::Return
            ) {
                if !matches!(actual.terminator, Some(Terminator::Return { .. })) {
                    return Err(bf16_emission_refusal_v1(
                        "BF16 actual source Return changed",
                    ));
                }
                return_count += 1;
            }
            block_count += 1;
        }
    }
    if block_count != rows.blocks.len()
        || block_count != rows.terminator_operation_spans.len()
        || statement_count != rows.statement_operation_spans.len()
        || return_count != 2
        || !rows.generated_terminator_values.is_empty()
    {
        return Err(bf16_emission_refusal_v1(
            "BF16 extra source/canonical coverage row",
        ));
    }
    // All ordinary argument rows must belong to the actual root. The helper
    // context is not an ignored ABI scalar and fragments are not Rust tuples.
    for (owner, function) in rows
        .parameter_bindings
        .iter()
        .map(|r| (r.correspondence_owner, r.semantic_function))
        .chain(
            rows.parameter_component_bindings
                .iter()
                .map(|r| (r.correspondence_owner, r.semantic_function)),
        )
        .chain(
            rows.ignored_parameter_bindings
                .iter()
                .map(|r| (r.correspondence_owner, r.semantic_function)),
        )
    {
        budget.charge_work(1)?;
        if owner != source.root() || function != source.root() {
            return Err(bf16_emission_refusal_v1(
                "BF16 invented structural helper parameter row",
            ));
        }
    }
    let instance = rows
        .lowered_functions
        .iter()
        .find(|r| r.semantic_function == source.root())
        .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
    let root = bf16_function_v1(
        module,
        rows,
        source.root(),
        source.root(),
        SemanticKirFunctionRoleV1::KernelEntry,
        budget,
    )?;
    validate_parameter_correspondence_v1(
        semantic,
        instance,
        root,
        ArgumentTraceV1 {
            direct: &rows.parameter_bindings,
            components: &rows.parameter_component_bindings,
            ignored: &rows.ignored_parameter_bindings,
        },
        budget,
    )?;
    validate_call_component_pool_v1(&rows.call_returns, &rows.call_result_components, budget)?;
    if rows.call_returns.len() != 3 {
        return Err(bf16_emission_refusal_v1(
            "BF16 exact Call/Return anchor roster",
        ));
    }
    let mut calls = 0usize;
    let mut returns = 0usize;
    for row in &rows.call_returns {
        budget.charge_work(1)?;
        if row.correspondence_owner != source.root() {
            return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
        }
        let declaration = semantic
            .functions()
            .get(row.semantic_function.index() as usize)
            .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
        let term = declaration
            .blocks()
            .get(row.semantic_block.index() as usize)
            .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?
            .terminator()
            .kind();
        match row.kind {
            SemanticKirCallReturnKindV1::Call {
                arguments_first,
                call_operation,
                destination_end,
                destination,
                ..
            } => {
                let span = bf16_source_span_v1(
                    rows,
                    source.root(),
                    source.root(),
                    source.call_block(),
                    budget,
                )?;
                if row.semantic_function != source.root()
                    || row.semantic_block != source.call_block()
                    || !matches!(term, SemanticTerminatorKindV1::Call(_))
                    || arguments_first < span.first_operation_ordinal
                    || call_operation < arguments_first
                    || destination_end <= call_operation
                    || destination_end > span.first_operation_ordinal + span.operation_count
                    || destination != SemanticKirCallDestinationV1::Local
                {
                    return Err(bf16_emission_refusal_v1("BF16 exact Call emission anchor"));
                }
                calls += 1;
            }
            SemanticKirCallReturnKindV1::Return { .. } => {
                if !matches!(term, SemanticTerminatorKindV1::Return) {
                    return Err(bf16_emission_refusal_v1("BF16 Return source anchor"));
                }
                returns += 1;
            }
        }
    }
    if calls != 1 || returns != 2 {
        return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
    }
    Ok(())
}
