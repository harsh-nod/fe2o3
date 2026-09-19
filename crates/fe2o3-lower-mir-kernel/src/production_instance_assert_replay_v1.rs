// Replays assertion attachments through the owned instance coordinates.
// It does not prove source-value equivalence, elision validity or scope lifetime.
fn replay_pending_instance_asserts_v1(
    pending: &PendingScopedRootEmissionV29,
    instances: &ProductionCallInstancePlanV1<'_>,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    let floor = budget.storage();
    let result = (|| {
        pending
            .coordinates
            .check_source_plan(instances, budget)
            .map_err(pending_scope_correspondence_error_v29)?;
        budget.charge_work(2 + pending.coordinates.seeds.rows.len())?;
        if pending.sidecars.rows.len() != instances.instances().len() {
            return Err(execution_call_error_v29());
        }
        let root_seed = pending
            .coordinates
            .seeds
            .rows
            .iter()
            .find(|row| row.instance == instances.root())
            .ok_or_else(execution_call_error_v29)?;
        assert_origin_string_work_v1(
            &root_seed.function_name,
            pending.function.id.as_str(),
            budget,
        )?;
        if root_seed.function_name != pending.function.id.as_str() {
            return Err(execution_call_error_v29());
        }
        let functions = std::slice::from_ref(&pending.function);
        let graph = AssertGraphIndexV1::build_functions(functions, true, budget)?;
        let root_coordinate = graph.function(pending.function.id.as_str(), budget)?;
        for (index, sidecar) in pending.sidecars.rows.iter().enumerate() {
            budget.charge_work(2)?;
            let instance = instances
                .id_at(index)
                .ok_or_else(execution_call_error_v29)?;
            let capture = sidecar
                .instance_assert_origins
                .as_ref()
                .ok_or_else(execution_call_error_v29)?;
            capture.check_identity(instances, instance, budget)?;
            if sidecar.source_call_instance != Some(instance) {
                return Err(execution_call_error_v29());
            }
            let row = instances
                .instance(instance)
                .ok_or_else(execution_call_error_v29)?;
            let source = row.declaration();
            budget.charge_work(pending.coordinates.seeds.rows.len())?;
            let seed = pending
                .coordinates
                .seeds
                .rows
                .iter()
                .find(|seed| seed.instance == instance)
                .ok_or_else(execution_call_error_v29)?;
            let mut count = 0;
            let mut argument_end = 0_usize;
            for block in row.ssa().plan().reverse_postorder() {
                budget.charge_work(2)?;
                let semantic_block = SemanticBlockIdV1::from_index(block.get());
                let source_block = source
                    .blocks()
                    .get(block.get() as usize)
                    .ok_or_else(execution_call_error_v29)?;
                let SemanticTerminatorKindV1::Assert {
                    expected, target, ..
                } = source_block.terminator().kind()
                else {
                    continue;
                };
                let site = SemanticKirAssertSiteV1::new(
                    capture.source.root,
                    capture.function,
                    semantic_block,
                );
                let bad = || {
                    assert_origin_invalid_v1(Some(site), "instance assertion attachment differs")
                };
                let recorded = capture
                    .records
                    .get(count)
                    .ok_or(SemanticKirAssertOriginErrorV1::MissingBinding { site })?;
                count += 1;
                let success = capture.placement.block(target.target().index())?;
                if recorded.site != site
                    || recorded.expected != *expected
                    || recorded.semantic_success != target.target()
                    || recorded.physical_success != success
                    || recorded.block != capture.placement.block(block.get())?
                    || recorded.argument_start != argument_end
                {
                    return Err(bad().into());
                }
                argument_end = argument_end
                    .checked_add(recorded.argument_count)
                    .ok_or(ArgumentResourceV1::Arithmetic)?;
                assert_origin_string_work_v1(
                    &recorded.emitted_function,
                    &seed.function_name,
                    budget,
                )?;
                if recorded.emitted_function != seed.function_name {
                    return Err(bad().into());
                }
                let coordinates = &pending.coordinates;
                budget.charge_work(coordinates.spans.rows.len())?;
                let mut spans = coordinates.spans.rows.iter().filter(|span| {
                    span.instance == instance
                        && matches!(span.source,
                        InstanceSpanSourceV1::Terminator(origin)
                        if origin.correspondence_owner == site.correspondence_owner
                            && origin.semantic_function == site.semantic_function
                            && origin.semantic_block == site.semantic_block)
                });
                let mapped = spans.next().ok_or_else(bad)?;
                let InstanceSpanSourceV1::Terminator(origin) = mapped.source else {
                    return Err(bad().into());
                };
                let physical = InstancePhysicalSpanV1 {
                    block: recorded.block,
                    first: recorded.first_operation,
                    count: recorded.operation_count,
                };
                let relocated = match &pending.slot_relocation {
                    Some(relocation) => relocation.assertion_span(physical, budget)?,
                    None => physical,
                };
                if spans.next().is_some()
                    || mapped.removed_call.is_some()
                    || origin.kernel_ir_block != physical.block
                    || origin.first_operation_ordinal != physical.first
                    || origin.operation_count != physical.count
                    || mapped.segments != [Some(relocated), None]
                {
                    return Err(bad().into());
                }
                budget.charge_work(coordinates.controls.rows.len())?;
                let controls = coordinates
                    .controls
                    .rows
                    .iter()
                    .filter(|control| {
                        control.instance == instance
                            && control.semantic_block == Some(semantic_block)
                            && control.original_block == recorded.block
                            && control.physical_block == recorded.block
                            && control.origin == InstanceControlOriginV1::Retained
                    })
                    .count();
                if controls != 1 {
                    return Err(bad().into());
                }

                // A called success block keeps its original entry; its Retained
                // terminator moves to a continuation and is not the assertion target.
                budget.charge_work(sidecar.blocks.len() + coordinates.controls.rows.len())?;
                let mut entries = sidecar.blocks.iter().filter(|entry| {
                    entry.correspondence_owner == site.correspondence_owner
                        && entry.semantic_function == site.semantic_function
                        && entry.semantic_block == target.target()
                });
                if entries.next().map(|entry| entry.kernel_ir_block) != Some(success)
                    || entries.next().is_some()
                    || coordinates
                        .controls
                        .rows
                        .iter()
                        .filter(|control| {
                            control.instance == instance
                                && control.semantic_block == Some(target.target())
                                && control.original_block == success
                                && control.physical_block == success
                                && !matches!(
                                    control.origin,
                                    InstanceControlOriginV1::ParameterPreheader { .. }
                                )
                        })
                        .count()
                        != 1
                {
                    return Err(bad().into());
                }
                let failure = capture.placement.block(
                    u32::try_from(source.blocks().len())
                        .map_err(|_| ArgumentResourceV1::Arithmetic)?,
                )?;
                if matches!(recorded.outcome, PendingAssertOutcomeV1::Emitted { .. }) {
                    budget.charge_work(
                        coordinates.spans.rows.len() + coordinates.controls.rows.len(),
                    )?;
                    if coordinates.spans.rows.iter().filter(|span| {
                        span.instance == instance && matches!(span.source,
                            InstanceSpanSourceV1::Synthetic(origin)
                            if origin.correspondence_owner == site.correspondence_owner
                                && origin.semantic_function == site.semantic_function
                                && origin.rule == SemanticKirSyntheticOperationRuleV1::RuntimeAssertFailureTrap
                                && origin.kernel_ir_block == failure
                                && origin.first_operation_ordinal == 0 && origin.operation_count == 1)
                            && span.removed_call.is_none()
                            && span.segments == [Some(InstancePhysicalSpanV1 { block: failure, first: 0, count: 1 }), None]
                    }).count() != 1 || coordinates.controls.rows.iter().filter(|control| {
                        control.instance == instance && control.semantic_block.is_none()
                            && control.original_block == failure && control.physical_block == failure
                            && control.origin == InstanceControlOriginV1::Retained
                    }).count() != 1 {
                        return Err(bad().into());
                    }
                }
                let coordinate = graph.block(root_coordinate, recorded.block, budget)?;
                // Returned coordinates are local scratch, never sealed bindings.
                seal_assert_occurrence_in_functions_at_v1(
                    recorded,
                    relocated.first,
                    &capture.arguments,
                    coordinate,
                    &graph,
                    functions,
                    failure,
                    budget,
                )?;
            }
            if count != capture.records.len() || argument_end != capture.arguments.len() {
                return Err(execution_call_error_v29());
            }
        }
        graph.release(budget)?;
        Ok(())
    })();
    // Every temporary graph index is dropped before its outstanding scratch is
    // released, including partially built indices on budget or attachment failure.
    let extra = budget
        .storage()
        .checked_sub(floor)
        .ok_or(ArgumentResourceV1::Accounting)?;
    budget.release_storage(extra)?;
    result
}
