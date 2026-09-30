// This structural join consumes the original source plan. The enclosing source
// replay still owns ABI derivation, assertion policy and payload equivalence.
fn invocation_checked_prefix_v1(
    instance: &production_call_instances_v1::ProductionCallInstanceV1<'_>,
    owner: SemanticFunctionIdV1,
    lowered: &LoweredFunctionResultV1,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(usize, usize), ProductionSemanticKirErrorV1> {
    let source = instance.declaration();
    let first_source = lowered
        .blocks
        .first()
        .ok_or_else(invocation_entry_error_v1)?;
    let first_block = first_source
        .kernel_ir_block
        .0
        .checked_sub(source.entry().index())
        .ok_or_else(invocation_entry_error_v1)?;
    budget.charge_work(lowered.synthetic_operation_spans.len())?;
    let has_trap = lowered
        .synthetic_operation_spans
        .iter()
        .any(|span| span.rule == SemanticKirSyntheticOperationRuleV1::RuntimeAssertFailureTrap);
    with_invocation_entry_plan_v1(
        source,
        instance.ssa(),
        SemanticEmissionPlacementV1 {
            first_block,
            first_value: 0,
        },
        has_trap,
        budget,
        |plan, budget| {
            let Some(preheader) = plan.layout.preheader else {
                if lowered.invocation_entry.is_some() {
                    return Err(invocation_entry_error_v1());
                }
                return Ok((0, 0));
            };
            let relation = lowered
                .invocation_entry
                .as_ref()
                .ok_or_else(invocation_entry_error_v1)?;
            let anchors = lowered
                .scoped_memory_anchors
                .as_ref()
                .ok_or_else(invocation_entry_error_v1)?;
            budget.charge_work(8)?;
            if relation.layout != plan.layout
                || relation.span.version != INVOCATION_ENTRY_RELATION_VERSION_V1
                || relation.span.correspondence_owner != owner
                || relation.span.semantic_function != instance.function()
                || relation.span.kernel_ir_block != preheader
                || relation.subject != Some(anchors.subject)
                || relation.subject.is_none_or(|subject| {
                    subject.source.root != owner
                        || subject.function != instance.function()
                        || Some(subject.instance) != lowered.source_call_instance
                        || subject.ledger != budget.work_ledger_identity_v1()
                })
                || anchors.placement.first_block != first_block
                || relation.arguments.len() != plan.entry_arguments().len()
            {
                return Err(invocation_entry_error_v1());
            }
            let body = lowered
                .function
                .body
                .as_ref()
                .ok_or_else(invocation_entry_error_v1)?;
            let first = body.blocks.first().ok_or_else(invocation_entry_error_v1)?;
            let header = body.blocks.get(1).ok_or_else(invocation_entry_error_v1)?;
            if first.id != preheader
                || !first.parameters.is_empty()
                || header.id != plan.layout.source_entry
                || header.parameters.len() != relation.components.len()
                || lowered.function.signature.parameters.len() != body.parameters.len()
            {
                return Err(invocation_entry_error_v1());
            }
            for (ordinal, block) in body.blocks.iter().enumerate() {
                budget.charge_work(2)?;
                if ordinal != 0 && block.id == preheader {
                    return Err(invocation_entry_error_v1());
                }
                block
                    .terminator
                    .as_ref()
                    .ok_or_else(invocation_entry_error_v1)?
                    .try_visit_edges_v1(|target, _| {
                        budget.charge_work(1)?;
                        if target == preheader {
                            return Err(invocation_entry_error_v1());
                        }
                        Ok(())
                    })?;
            }
            let Some(Terminator::Branch { target, arguments }) = &first.terminator else {
                return Err(invocation_entry_error_v1());
            };
            if *target != header.id || arguments.len() != relation.components.len() {
                return Err(invocation_entry_error_v1());
            }
            // Preserve legacy absent rosters, but replay a retained roster even
            // after all its rows have been removed from a zero-phi callee.
            if !relation.inputs_retained
                && (!plan.entry_arguments().is_empty() || !relation.inputs.is_empty())
            {
                return Err(invocation_entry_error_v1());
            }
            if relation.inputs_retained {
                invocation_check_inputs_v1(
                    source,
                    &relation.inputs,
                    body.parameters.len(),
                    budget,
                )?;
            }
            let mut operation = relation.span.first_operation_ordinal as usize;
            let mut component = 0;
            for (row, original) in relation.arguments.iter().zip(plan.entry_arguments()) {
                budget.charge_work(4)?;
                if row.original != *original || row.first_component != component {
                    return Err(invocation_entry_error_v1());
                }
                let end = argument_sum_v1(&[component, row.component_count])?;
                let rows = relation
                    .components
                    .get(component..end)
                    .ok_or_else(invocation_entry_error_v1)?;
                let local = SemanticLocalIdV1::from_index(original.variable().get());
                budget.charge_work(relation.inputs.len())?;
                let installed = relation
                    .inputs
                    .iter()
                    .find(|input| input.local == local.index());
                let declaration = source
                    .locals()
                    .get(local.index() as usize)
                    .ok_or_else(invocation_entry_error_v1)?;
                if installed.map_or(0, |input| input.parameter_count) != rows.len()
                    || installed.is_some_and(|input| input.ty != declaration.ty())
                    || installed.is_none() && declaration.role().is_entry_argument()
                {
                    return Err(invocation_entry_error_v1());
                }
                for (offset, row) in rows.iter().enumerate() {
                    budget.charge_work(argument_sum_v1(&[body.parameters.len(), 4])?)?;
                    let input = argument_sum_v1(&[
                        installed
                            .ok_or_else(invocation_entry_error_v1)?
                            .first_parameter,
                        offset,
                    ])?;
                    let expected_input = *body
                        .parameters
                        .get(input)
                        .ok_or_else(invocation_entry_error_v1)?;
                    let parameter = &header.parameters[component];
                    if body
                        .parameters
                        .iter()
                        .filter(|value| **value == expected_input)
                        .count()
                        != 1
                        || row.original != expected_input
                        || row.parameter != parameter.id
                        || arguments[component] != row.transported
                    {
                        return Err(invocation_entry_error_v1());
                    }
                    let actual = &lowered.function.signature.parameters[input];
                    let expected = &parameter.ty;
                    let equal_types = invocation_equal_types_v1(actual, expected, budget)?;
                    match row.conversion {
                        None if equal_types && row.transported == row.original => {}
                        Some(ordinal)
                            if ordinal as usize == operation
                                && index_and_u64_are_transport_equivalent(actual, expected) =>
                        {
                            let emitted = first
                                .operations
                                .get(operation)
                                .ok_or_else(invocation_entry_error_v1)?;
                            if !matches!(&emitted.kind, OperationKind::Cast { kind: CastKind::Bitcast, value, to }
                                if *value == row.original && to == expected)
                                || emitted.results.as_slice()
                                    != [ValueDef::new(row.transported, expected.clone())]
                            {
                                return Err(invocation_entry_error_v1());
                            }
                            operation = argument_sum_v1(&[operation, 1])?;
                        }
                        _ => return Err(invocation_entry_error_v1()),
                    }
                    component += 1;
                }
            }
            if component != relation.components.len()
                || operation != first.operations.len()
                || operation
                    != argument_sum_v1(&[
                        relation.span.first_operation_ordinal as usize,
                        relation.span.operation_count as usize,
                    ])?
            {
                return Err(invocation_entry_error_v1());
            }
            let mut cursor = 0;
            let mut synthetic = 0;
            for rule in [
                SemanticKirSyntheticOperationRuleV1::RetainedLocalStorage,
                SemanticKirSyntheticOperationRuleV1::EnumPayloadStorage,
            ] {
                budget.charge_work(1)?;
                if let Some(span) = lowered.synthetic_operation_spans.get(synthetic)
                    && span.kernel_ir_block == preheader
                    && span.rule == rule
                {
                    if span.correspondence_owner != owner
                        || span.semantic_function != instance.function()
                        || span.operation_count == 0
                        || span.first_operation_ordinal != cursor
                    {
                        return Err(invocation_entry_error_v1());
                    }
                    cursor = cursor
                        .checked_add(span.operation_count)
                        .ok_or(ArgumentResourceV1::Arithmetic)?;
                    synthetic += 1;
                }
            }
            if cursor != relation.span.first_operation_ordinal {
                return Err(invocation_entry_error_v1());
            }
            Ok((1, synthetic))
        },
    )
}
