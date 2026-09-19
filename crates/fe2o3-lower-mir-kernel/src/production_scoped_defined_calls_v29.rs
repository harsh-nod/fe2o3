// Checks live call attachment and memory phases, not source value equivalence.
fn check_scoped_defined_call_phases_v29(
    instances: &ProductionCallInstancePlanV1<'_>,
    emitted: &[Option<LoweredFunctionResultV1>],
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    with_canonical_call_scratch_v1(budget, |budget| {
        let mismatch = || ProductionSemanticKirErrorV1::CorrespondenceMismatch;
        budget.charge_work(1)?;
        if emitted.len() != instances.instances().len() {
            return Err(mismatch());
        }
        let source_identity = ExecutionCallSourceV29::from_instances(instances, budget)?;
        for (index, lowered) in emitted.iter().enumerate() {
            with_canonical_call_scratch_v1(budget, |budget| {
                budget.charge_work(12)?;
                let id = instances.id_at(index).ok_or_else(mismatch)?;
                let source = instances.instance(id).ok_or_else(mismatch)?;
                let lowered = lowered.as_ref().ok_or_else(mismatch)?;
                if lowered.source_call_instance != Some(id) {
                    return Err(mismatch());
                }
                if let Some(events) = &lowered.lifecycle_events {
                    events.check_identity(instances, id, budget)?;
                }
                instance_check_source_rows_v1(source, source_identity.root, lowered, budget)
                    .map_err(pending_scope_correspondence_error_v29)?;
                let anchors = lowered
                    .scoped_memory_anchors
                    .as_ref()
                    .ok_or_else(mismatch)?;
                if anchors.subject.ledger != budget.work_ledger_identity_v1() {
                    return Err(ArgumentResourceV1::Accounting.into());
                }
                if anchors.subject.source != source_identity
                    || anchors.subject.instance != id
                    || anchors.subject.function != source.function()
                    || lowered
                        .lifecycle_events
                        .as_ref()
                        .is_some_and(|events| events.placement != anchors.placement)
                {
                    return Err(mismatch());
                }
                let rows = &lowered.call_returns.sites.rows;
                let components = &lowered.call_returns.components.rows;
                validate_call_component_pool_v1(rows, components, budget)?;
                budget.charge_work(argument_product_v1(rows.len(), 4)?)?;
                if rows
                    .windows(2)
                    .any(|pair| pair[0].semantic_block >= pair[1].semantic_block)
                    || rows.iter().any(|row| {
                        row.correspondence_owner != source_identity.root
                            || row.semantic_function != source.function()
                    })
                {
                    return Err(mismatch());
                }
                let calls = instances.calls(id).ok_or_else(mismatch)?;
                budget.charge_work(calls.len())?;
                let expected = calls.iter().filter(|call| call.child().is_some()).count();
                if rows
                    .iter()
                    .filter(|row| matches!(row.kind, SemanticKirCallReturnKindV1::Call { .. }))
                    .count()
                    != expected
                {
                    return Err(mismatch());
                }
                let values = CallFunctionIndexV1::new(&lowered.function, budget)?;
                check_call_signature_v1(
                    instances.owner().source_semantic(),
                    source.declaration(),
                    source.function(),
                    if id == instances.root() {
                        SemanticKirFunctionRoleV1::KernelEntry
                    } else {
                        SemanticKirFunctionRoleV1::InternalHelper
                    },
                    &lowered.function,
                    budget,
                )?;
                let exits = instances.exits(id).ok_or_else(mismatch)?;
                budget.charge_work(exits.len())?;
                let returns = instances.returns(id).ok_or_else(mismatch)?.count();
                if rows
                    .iter()
                    .filter(|row| matches!(row.kind, SemanticKirCallReturnKindV1::Return { .. }))
                    .count()
                    != returns
                {
                    return Err(mismatch());
                }
                for row in rows {
                    let SemanticKirCallReturnKindV1::Return {
                        components: returned,
                    } = row.kind
                    else {
                        continue;
                    };
                    budget.charge_work(lowered.terminator_operation_spans.len())?;
                    let span = lowered
                        .terminator_operation_spans
                        .iter()
                        .find(|span| span.semantic_block == row.semantic_block)
                        .ok_or_else(mismatch)?;
                    if !source
                        .declaration()
                        .blocks()
                        .get(row.semantic_block.index() as usize)
                        .is_some_and(|block| {
                            matches!(block.terminator().kind(), SemanticTerminatorKindV1::Return)
                        })
                    {
                        return Err(mismatch());
                    }
                    let block = values.block(
                        scoped_call_block_v29(lowered, row.semantic_block, budget)?,
                        budget,
                    )?;
                    let first = span.first_operation_ordinal as usize;
                    let end = argument_sum_v1(&[first, span.operation_count as usize])?;
                    check_call_return_v1(
                        block,
                        first,
                        block.operations.get(first..end).ok_or_else(mismatch)?,
                        call_components_v1(components, returned)?,
                        &lowered.function.signature.results,
                        &values,
                        budget,
                    )?;
                }
                for call in calls {
                    let Some(child) = call.child() else { continue };
                    budget.charge_work(8)?;
                    let occurrence = call.occurrence();
                    let callee = instances.instance(child).ok_or_else(mismatch)?;
                    let target = emitted
                        .get(child.index())
                        .and_then(Option::as_ref)
                        .ok_or_else(mismatch)?;
                    if occurrence.caller != id
                        || target.source_call_instance != Some(child)
                        || !matches!(call.callable(), SemanticCallableDeclV1::Defined { function }
                            if *function == callee.function())
                        || !instances
                            .incoming(child)
                            .is_some_and(|incoming| std::ptr::eq(incoming, call))
                    {
                        return Err(mismatch());
                    }
                    budget.charge_work(scoped_initialization_search_work_v29(rows.len()))?;
                    let row = rows
                        .binary_search_by_key(&occurrence.block, |row| row.semantic_block)
                        .ok()
                        .map(|index| &rows[index])
                        .ok_or_else(mismatch)?;
                    let SemanticKirCallReturnKindV1::Call {
                        arguments_first,
                        call_operation,
                        destination_end,
                        destination,
                        transport,
                    } = row.kind
                    else {
                        return Err(mismatch());
                    };
                    budget.charge_work(lowered.terminator_operation_spans.len())?;
                    let span = lowered
                        .terminator_operation_spans
                        .iter()
                        .find(|span| span.semantic_block == occurrence.block)
                        .ok_or_else(mismatch)?;
                    let block_id = scoped_call_block_v29(lowered, occurrence.block, budget)?;
                    if span.kernel_ir_block != block_id {
                        return Err(mismatch());
                    }
                    let block = values.block(block_id, budget)?;
                    let source_call = call.source();
                    let next = source_call
                        .destination()
                        .ok_or_else(mismatch)?
                        .edge()
                        .target();
                    let successor = scoped_call_block_v29(lowered, next, budget)?;
                    let first = span.first_operation_ordinal as usize;
                    let end = argument_sum_v1(&[first, span.operation_count as usize])?;
                    check_resolved_defined_call_v1(
                        instances.owner().source_semantic(),
                        callee.declaration(),
                        &target.function.id,
                        &target.function,
                        successor,
                        block,
                        source_call,
                        first,
                        end,
                        arguments_first as usize,
                        call_operation as usize,
                        destination_end as usize,
                        destination,
                        &values,
                        budget,
                    )?;
                    check_resolved_call_transport_v1(
                        source.ssa(),
                        occurrence.block,
                        block,
                        source_call,
                        call_operation as usize,
                        destination_end as usize,
                        end,
                        destination,
                        call_components_v1(components, transport)?,
                        &values,
                        budget,
                    )?;
                    check_scoped_call_access_phases_v29(
                        anchors,
                        occurrence.block,
                        block,
                        source_call,
                        first,
                        end,
                        arguments_first as usize,
                        call_operation as usize,
                        destination_end as usize,
                        budget,
                    )?;
                }
                Ok(())
            })?;
        }
        Ok(())
    })
}

fn scoped_call_block_v29(
    lowered: &LoweredFunctionResultV1,
    source: SemanticBlockIdV1,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<BlockId, ProductionSemanticKirErrorV1> {
    let mismatch = || ProductionSemanticKirErrorV1::CorrespondenceMismatch;
    budget.charge_work(lowered.blocks.len())?;
    let mut matches = lowered
        .blocks
        .iter()
        .filter(|row| row.semantic_block == source);
    let block = matches.next().ok_or_else(mismatch)?.kernel_ir_block;
    let placement = lowered
        .scoped_memory_anchors
        .as_ref()
        .ok_or_else(mismatch)?
        .placement;
    if matches.next().is_some() || block != placement.block(source.index())? {
        return Err(mismatch());
    }
    Ok(block)
}

#[allow(clippy::too_many_arguments)]
fn check_scoped_call_access_phases_v29(
    anchors: &ScopedMemoryAnchorsV29,
    source_block: SemanticBlockIdV1,
    block: &BasicBlock,
    call: &SemanticDirectCallV1,
    first: usize,
    end: usize,
    arguments_first: usize,
    call_operation: usize,
    destination_end: usize,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    let operations = block
        .operations
        .get(first..end)
        .ok_or_else(scoped_memory_error_v29)?;
    budget.charge_work(argument_sum_v1(&[operations.len(), anchors.rows.len()])?)?;
    let expected = operations
        .iter()
        .filter(|operation| scoped_memory_pointer_v29(&operation.kind).is_some())
        .count();
    let site = execution_site_v29(source_block, None);
    let mut previous = None;
    let mut checked = 0;
    for row in &anchors.rows {
        let ScopedMemoryAnchorKindV29::Access { pointer } = row.kind else {
            continue;
        };
        if row.block != block.id || !(first..end).contains(&row.position) {
            continue;
        }
        let frame = row.source.ok_or_else(scoped_memory_error_v29)?;
        let operation = &block.operations[row.position].kind;
        if frame.site != site
            || previous.is_some_and(|position| position >= row.position)
            || scoped_memory_pointer_v29(operation) != Some(pointer)
        {
            return Err(scoped_memory_error_v29());
        }
        let valid = if row.position < arguments_first {
            frame.role
                == Some(ScopedMemoryRoleV29::Operand(
                    ExecutionOperandV29::CallDestinationAddress,
                ))
                && matches!(
                    operation,
                    OperationKind::Load { .. } | OperationKind::GuardedLoad { .. }
                )
        } else if row.position < call_operation {
            matches!(frame.role,
                Some(ScopedMemoryRoleV29::Operand(ExecutionOperandV29::CallArgument(index)))
                    if (index as usize) < call.arguments().len())
        } else if row.position > call_operation && row.position < destination_end {
            frame.role == Some(ScopedMemoryRoleV29::CallResult)
                && matches!(
                    operation,
                    OperationKind::Store { .. } | OperationKind::GuardedStore { .. }
                )
        } else {
            false
        };
        if !valid {
            return Err(scoped_memory_error_v29());
        }
        previous = Some(row.position);
        checked += 1;
    }
    if checked != expected {
        return Err(scoped_memory_error_v29());
    }
    Ok(())
}
