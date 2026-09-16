impl<'a> ProductionCanonicalCallsV1<'a> {
    fn build(
        owner: &'a ProductionPreRankedKirOwnerV1,
        inventory: &'a CanonicalKirInventoryV1<'a>,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<Self, ProductionSemanticKirErrorV1> {
        let rows = &owner.correspondence;
        let mismatch = || ProductionSemanticKirErrorV1::CorrespondenceMismatch;
        budget.charge_work(argument_sum_v1(&[
            argument_product_v1(rows.lowered_functions.len(), 6)?,
            rows.call_returns.len(),
            rows.terminator_operation_spans.len(),
            rows.parameter_bindings.len(),
            rows.parameter_component_bindings.len(),
            rows.ignored_parameter_bindings.len(),
        ])?)?;
        budget.reserve_storage(argument_sum_v1(&[
            std::mem::size_of::<Self>(),
            argument_product_v1(
                rows.lowered_functions.len(),
                std::mem::size_of::<CanonicalCallGroupV1<'_>>(),
            )?,
            argument_product_v1(
                rows.call_returns.len(),
                std::mem::size_of::<CanonicalCallBindingV1<'_>>(),
            )?,
        ])?)?;
        let mut groups = argument_vec_v1(rows.lowered_functions.len())?;
        let mut calls = argument_vec_v1(rows.call_returns.len())?;
        let index_floor = budget.storage();
        let targets =
            CallTargetIndexV1::new(owner.executable.module(), &rows.lowered_functions, budget)?;
        let index_storage = budget.storage() - index_floor;
        let mut remaining_calls = rows.call_returns.as_ref();
        let mut remaining_spans = rows.terminator_operation_spans.as_ref();
        let mut remaining_direct = rows.parameter_bindings.as_ref();
        let mut remaining_components = rows.parameter_component_bindings.as_ref();
        let mut remaining_ignored = rows.ignored_parameter_bindings.as_ref();
        // The sealed correspondence stores each association contiguously in
        // lowering order. Partition once; queries never search these rosters.
        macro_rules! group {
            ($remaining:ident, $source:ident) => {{
                let count = $remaining
                    .iter()
                    .take_while(|row| {
                        row.correspondence_owner == $source.correspondence_owner
                            && row.semantic_function == $source.semantic_function
                    })
                    .count();
                let (selected, rest) = $remaining.split_at(count);
                $remaining = rest;
                selected
            }};
        }
        for source in &rows.lowered_functions {
            let canonical = inventory
                .function_for_name(source.kernel_ir_function.as_str(), budget)
                .map_err(canonical_call_inventory_error_v1)?
                .ok_or_else(mismatch)?;
            let (_, target) = targets.source(
                source.correspondence_owner,
                source.semantic_function,
                budget,
            )?;
            if !std::ptr::eq(canonical.function, target) {
                return Err(mismatch());
            }
            let group = CanonicalCallGroupV1 {
                function: ProductionCanonicalCallFunctionV1 { source, canonical },
                calls: group!(remaining_calls, source),
                spans: group!(remaining_spans, source),
                direct: group!(remaining_direct, source),
                components: group!(remaining_components, source),
                ignored: group!(remaining_ignored, source),
            };
            validate_call_correspondence_v1(
                &owner.semantic_ssa,
                source,
                target,
                &targets,
                group.calls,
                &rows.call_result_components,
                group.spans,
                budget,
            )?;
            groups.push(group);
        }
        if !remaining_calls.is_empty()
            || !remaining_spans.is_empty()
            || !remaining_direct.is_empty()
            || !remaining_components.is_empty()
            || !remaining_ignored.is_empty()
        {
            return Err(mismatch());
        }
        for group in &groups {
            let first = calls.len();
            for index in group.function.canonical.calls.clone() {
                budget.charge_work(1)?;
                let call = inventory.calls().get(index).ok_or_else(mismatch)?;
                if call
                    .operation
                    .has_complete_effect_summary_with_budget_v1(budget)?
                {
                    continue;
                }
                if calls.len() == rows.call_returns.len() {
                    return Err(mismatch());
                }
                calls.push(bind_canonical_call_v1(
                    owner, inventory, &groups, &targets, group, index, budget,
                )?);
            }
            budget.charge_work(group.calls.len())?;
            let expected = group
                .calls
                .iter()
                .filter(|row| matches!(row.kind, SemanticKirCallReturnKindV1::Call { .. }))
                .count();
            if calls.len() - first != expected {
                return Err(mismatch());
            }
        }
        assert_origin_sort_v1(&mut calls, budget, |a, b, budget| {
            budget.charge_work(1)?;
            Ok(a.key().cmp(&b.key()))
        })
        .map_err(call_index_error_v1)?;
        budget.charge_work(calls.len())?;
        if calls.windows(2).any(|pair| pair[0].key() == pair[1].key()) {
            return Err(mismatch());
        }
        drop(targets);
        budget.release_storage(index_storage)?;
        Ok(Self {
            owner,
            inventory,
            groups,
            calls,
        })
    }
}

#[allow(clippy::too_many_arguments)]
fn bind_canonical_call_v1<'a>(
    owner: &'a ProductionPreRankedKirOwnerV1,
    inventory: &'a CanonicalKirInventoryV1<'a>,
    groups: &[CanonicalCallGroupV1<'a>],
    targets: &CallTargetIndexV1<'a>,
    group: &CanonicalCallGroupV1<'a>,
    index: usize,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<CanonicalCallBindingV1<'a>, ProductionSemanticKirErrorV1> {
    let mismatch = || ProductionSemanticKirErrorV1::CorrespondenceMismatch;
    let call = &inventory.calls()[index];
    let function = group.function.canonical;
    let caller = group.function.source;
    let block_index =
        argument_sum_v1(&[function.blocks.start, call.coordinate.block.block as usize])?;
    if call.coordinate.block.function != function.coordinate
        || !function.blocks.contains(&block_index)
    {
        return Err(mismatch());
    }
    let block = inventory
        .blocks()
        .get(block_index)
        .ok_or_else(mismatch)?
        .block;
    let semantic_block = SemanticBlockIdV1::from_index(block.id.0);
    // Anchors are ID-sorted; spans follow the physical block's stored order.
    budget.charge_work(41)?;
    let anchor = &group.calls[group
        .calls
        .binary_search_by_key(&semantic_block, |row| row.semantic_block)
        .map_err(|_| mismatch())?];
    let span = group
        .spans
        .get(call.coordinate.block.block as usize)
        .ok_or_else(mismatch)?;
    let SemanticKirCallReturnKindV1::Call { call_operation, .. } = anchor.kind else {
        return Err(mismatch());
    };
    if span.semantic_block != semantic_block
        || call_operation != call.coordinate.operation
        || span.kernel_ir_block != block.id
        || !std::ptr::eq(
            block
                .operations
                .get(call_operation as usize)
                .ok_or_else(mismatch)?,
            call.operation,
        )
    {
        return Err(mismatch());
    }
    let semantic = owner.semantic_ssa.source_semantic();
    let source_block = semantic
        .functions()
        .get(caller.semantic_function.index() as usize)
        .and_then(|function| function.blocks().get(semantic_block.index() as usize))
        .ok_or_else(mismatch)?;
    let SemanticTerminatorKindV1::Call(source) = source_block.terminator().kind() else {
        return Err(mismatch());
    };
    let Some(SemanticCallableDeclV1::Defined { function: callee }) =
        semantic.callables().get(source.callee().index() as usize)
    else {
        return Err(mismatch());
    };
    let (callee, association) = targets.source_row(caller.correspondence_owner, *callee, budget)?;
    let callee_group = groups.get(callee).ok_or_else(mismatch)?;
    if !std::ptr::eq(association, callee_group.function.source)
        || call.target != Some(callee_group.function.canonical.coordinate)
    {
        return Err(mismatch());
    }
    Ok(CanonicalCallBindingV1 {
        call: index,
        callee,
        site: CheckedCallSiteV1 {
            caller,
            source,
            callee: association,
            callee_target: callee_group.function.canonical.function,
            block,
            span,
            anchor,
            returns: callee_group.calls,
        },
    })
}
