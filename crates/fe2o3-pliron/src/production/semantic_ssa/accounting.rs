use super::*;

pub(super) fn retained_cross_edge_variables_v1(
    input: &SsaConstructionInputV1,
    plan: &SsaConstructionPlanV1,
) -> Vec<SsaVariableIdV1> {
    let variable_count = input.variable_count() as usize;
    let block_count = input.blocks().len();
    let mut first_mention = vec![None::<usize>; variable_count];
    let mut multiple_blocks = vec![false; variable_count];
    {
        let mut mention = |variable: SsaVariableIdV1, block: usize| {
            let variable = variable.get() as usize;
            if !input.promotable()[variable] {
                match first_mention[variable] {
                    Some(first) if first != block => multiple_blocks[variable] = true,
                    Some(_) => {}
                    None => first_mention[variable] = Some(block),
                }
            }
        };

        let entry = input.entry().get() as usize;
        for variable in input.entry_definitions().iter().copied() {
            mention(variable, entry);
        }
        for (block_index, block) in input.blocks().iter().enumerate() {
            let block_id = SsaBlockIdV1::new(block_index as u32);
            if !plan.is_reachable(block_id) {
                continue;
            }
            for event in block.events().iter().copied() {
                mention(event.variable(), block_index);
            }
            for edge in block.edges() {
                for variable in edge.definitions().iter().copied() {
                    mention(variable, block_index);
                }
            }
        }
    }

    // Kahn elimination leaves cyclic blocks and blocks whose execution depends
    // on a cycle. A retained local mentioned there may carry state across a
    // backedge even when it is mentioned in only one source block.
    let mut indegree = vec![0_usize; block_count];
    for (source, block) in input.blocks().iter().enumerate() {
        if !plan.is_reachable(SsaBlockIdV1::new(source as u32)) {
            continue;
        }
        for edge in block.edges() {
            let target = edge.target().get() as usize;
            if plan.is_reachable(SsaBlockIdV1::new(target as u32)) {
                indegree[target] = indegree[target].saturating_add(1);
            }
        }
    }
    let mut pending = indegree
        .iter()
        .enumerate()
        .filter_map(|(block, incoming)| {
            (plan.is_reachable(SsaBlockIdV1::new(block as u32)) && *incoming == 0).then_some(block)
        })
        .collect::<VecDeque<_>>();
    while let Some(source) = pending.pop_front() {
        for edge in input.blocks()[source].edges() {
            let target = edge.target().get() as usize;
            if !plan.is_reachable(SsaBlockIdV1::new(target as u32)) {
                continue;
            }
            indegree[target] -= 1;
            if indegree[target] == 0 {
                pending.push_back(target);
            }
        }
    }

    input
        .promotable()
        .iter()
        .copied()
        .enumerate()
        .filter_map(|(variable, promotable)| {
            if promotable {
                return None;
            }
            let first = first_mention[variable]?;
            (multiple_blocks[variable] || indegree[first] != 0)
                .then_some(SsaVariableIdV1::new(variable as u32))
        })
        .collect()
}

pub(super) fn accumulate_summary_v1(
    summary: &mut ProductionSemanticSsaSummaryV1,
    function_plan: &ProductionSemanticSsaFunctionPlanV1,
    variable_count: usize,
    limits: ProductionSemanticSsaLimitsV1,
) -> Result<(), ProductionSemanticSsaErrorV1> {
    let resources = function_plan.plan.resources();
    let promotable = function_plan.plan.promoted_variables().len();
    let memory = variable_count - promotable;
    let partial_moves = function_plan.partial_moves;
    macro_rules! add {
        ($field:ident, $value:expr) => {
            summary.$field = summary
                .$field
                .checked_add($value)
                .ok_or(ProductionSemanticSsaErrorV1::ResourceOverflow)?;
        };
    }
    add!(promotable_variables, promotable);
    add!(memory_variables, memory);
    add!(input_blocks, resources.input_blocks());
    add!(reachable_blocks, resources.reachable_blocks());
    add!(pruned_blocks, resources.pruned_blocks());
    add!(input_edges, resources.input_edges());
    add!(input_events, resources.input_events());
    add!(input_edge_definitions, resources.input_edge_definitions());
    add!(generated_definitions, resources.generated_definitions());
    add!(output_items, resources.output_items());
    add!(storage_words, resources.storage_words());
    add!(work_units, resources.work_units());
    add!(
        storage_words,
        function_plan.auxiliary_resources.storage_words
    );
    add!(work_units, function_plan.auxiliary_resources.work_units);
    add!(storage_words, partial_moves.state_entries());
    add!(work_units, partial_moves.work_units());
    enforce_module_resource_limits_v1(*summary, limits)
}

pub(super) fn enforce_module_resource_limits_v1(
    summary: ProductionSemanticSsaSummaryV1,
    limits: ProductionSemanticSsaLimitsV1,
) -> Result<(), ProductionSemanticSsaErrorV1> {
    let module = limits.module();
    let variables = summary
        .promotable_variables
        .checked_add(summary.memory_variables)
        .ok_or(ProductionSemanticSsaErrorV1::ResourceOverflow)?;
    for (resource, required, limit) in [
        (
            SsaPlannerResourceV1::Variables,
            variables,
            module.max_variables(),
        ),
        (
            SsaPlannerResourceV1::Blocks,
            summary.input_blocks,
            module.max_blocks(),
        ),
        (
            SsaPlannerResourceV1::Edges,
            summary.input_edges,
            module.max_edges(),
        ),
        (
            SsaPlannerResourceV1::Events,
            summary.input_events,
            module.max_events(),
        ),
        (
            SsaPlannerResourceV1::EdgeDefinitions,
            summary.input_edge_definitions,
            module.max_edge_definitions(),
        ),
        (
            SsaPlannerResourceV1::OutputItems,
            summary.output_items,
            module.max_output_items(),
        ),
        (
            SsaPlannerResourceV1::StorageWords,
            summary.storage_words,
            module.max_storage_words(),
        ),
        (
            SsaPlannerResourceV1::WorkUnits,
            summary.work_units,
            module.max_work_units(),
        ),
    ] {
        if required > limit {
            return Err(ProductionSemanticSsaErrorV1::AggregateResourceLimit {
                resource,
                required,
                limit,
            });
        }
    }
    Ok(())
}

pub(super) fn derive_semantic_ssa_identity_v1(
    source_semantic_sha256: &[u8; 32],
    plans: &[ProductionSemanticSsaFunctionPlanV1],
    summary: ProductionSemanticSsaSummaryV1,
) -> ProductionSemanticSsaIdentityV1 {
    let mut digest = Sha256::new();
    digest.update(PRODUCTION_SEMANTIC_SSA_IDENTITY_DOMAIN_V1);
    digest.update(source_semantic_sha256);
    hash_usize_v1(&mut digest, plans.len());
    for function in plans {
        digest.update(function.function.index().to_le_bytes());
        digest.update(function.function_identity.as_bytes());
        digest.update(function.plan.identity().as_bytes());
        hash_resource_report_v1(&mut digest, function.plan.resources());
        hash_usize_v1(&mut digest, function.partial_moves.projected_moves());
        hash_usize_v1(&mut digest, function.partial_moves.state_entries());
        hash_usize_v1(&mut digest, function.partial_moves.work_units());
        hash_usize_v1(&mut digest, function.auxiliary_resources.storage_words);
        hash_usize_v1(&mut digest, function.auxiliary_resources.work_units);
        hash_usize_v1(&mut digest, function.implicit_entry_variables.len());
        for variable in &function.implicit_entry_variables {
            digest.update(variable.get().to_le_bytes());
        }
        hash_usize_v1(&mut digest, function.retained_cross_edge_variables.len());
        for variable in &function.retained_cross_edge_variables {
            digest.update(variable.get().to_le_bytes());
        }
    }
    hash_summary_v1(&mut digest, summary);
    ProductionSemanticSsaIdentityV1(digest.finalize().into())
}

fn hash_resource_report_v1(digest: &mut Sha256, report: &SsaPlannerResourceReportV1) {
    for value in [
        report.input_blocks(),
        report.reachable_blocks(),
        report.pruned_blocks(),
        report.input_edges(),
        report.input_events(),
        report.input_edge_definitions(),
        report.generated_definitions(),
        report.output_items(),
        report.storage_words(),
        report.work_units(),
    ] {
        hash_usize_v1(digest, value);
    }
}

fn hash_summary_v1(digest: &mut Sha256, summary: ProductionSemanticSsaSummaryV1) {
    for value in [
        summary.function_count,
        summary.promotable_variables,
        summary.memory_variables,
        summary.input_blocks,
        summary.reachable_blocks,
        summary.pruned_blocks,
        summary.input_edges,
        summary.input_events,
        summary.input_edge_definitions,
        summary.generated_definitions,
        summary.output_items,
        summary.storage_words,
        summary.work_units,
    ] {
        hash_usize_v1(digest, value);
    }
}

fn hash_usize_v1(digest: &mut Sha256, value: usize) {
    digest.update((value as u64).to_le_bytes());
}
