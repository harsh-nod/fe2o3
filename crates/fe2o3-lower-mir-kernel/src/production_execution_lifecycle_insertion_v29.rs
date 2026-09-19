// Insertion custody only. Original source coordinates remain unchanged; V15
// admission and source replay still have to validate the resulting graph.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct LifecycleInsertionV29 {
    instance: ProductionCallInstanceIdV1,
    event: usize,
    source_span: usize,
    before: InstancePhysicalSpanV1,
    after: InstancePhysicalSpanV1,
}

#[cfg_attr(
    not(test),
    allow(dead_code, reason = "Scoped source admission remains gated")
)]
struct OwnedLifecycleInsertedRootV29 {
    root: OwnedPendingScopedRootV29,
    insertions: Vec<LifecycleInsertionV29>,
}

struct PreparedLifecycleEventV29 {
    block: usize,
    witness: LifecycleInsertionV29,
    operation: Option<Operation>,
}

struct PreparedLifecycleBlockV29 {
    index: usize,
    events: std::ops::Range<usize>,
    operations: Vec<Operation>,
}

fn lifecycle_source_row_v29<'a>(
    coordinates: &'a OwnedInstanceCoordinatesV1,
    events: &PendingLifecycleEventsV29,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<&'a OwnedInstanceSourceV1, ProductionSemanticKirErrorV1> {
    if events.ledger != budget.work_ledger_identity_v1() {
        return Err(ArgumentResourceV1::Accounting.into());
    }
    budget.charge_work(argument_sum_v1(&[coordinates.sources.rows.len(), 5])?)?;
    if events.source.semantic != coordinates.semantic_sha256
        || events.source.ssa != coordinates.ssa
        || events.source.root != coordinates.root
        || events.rows.len() != events.expected_rows
    {
        return Err(execution_lifecycle_error_v29());
    }
    let mut sources = coordinates
        .sources
        .rows
        .iter()
        .filter(|row| row.instance == events.instance);
    let source = sources.next().ok_or_else(execution_lifecycle_error_v29)?;
    if sources.next().is_some() || source.function != events.function {
        return Err(execution_lifecycle_error_v29());
    }
    Ok(source)
}

fn lifecycle_return_v29(
    coordinates: &OwnedInstanceCoordinatesV1,
    source: &OwnedInstanceSourceV1,
    event: &DeferredLifecycleEventV29,
    block: &BasicBlock,
    gap: u32,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    budget.charge_work(argument_sum_v1(&[
        coordinates.returns.rows.len(),
        coordinates.controls.rows.len(),
        4,
    ])?)?;
    let mut returns = coordinates.returns.rows.iter().filter(|row| {
        row.instance == source.instance
            && row.original_block == event.original_block
            && row.source.correspondence_owner == coordinates.root
            && row.source.semantic_function == source.function
            && row.source.semantic_block == event.block
            && matches!(row.source.kind, SemanticKirCallReturnKindV1::Return { .. })
    });
    if returns.next().is_none()
        || returns.next().is_some()
        || gap as usize != block.operations.len()
    {
        return Err(execution_lifecycle_error_v29());
    }
    let mut controls = coordinates.controls.rows.iter().filter(|row| {
        row.instance == source.instance
            && row.original_block == event.original_block
            && row.semantic_block == Some(event.block)
    });
    let control = controls.next().ok_or_else(execution_lifecycle_error_v29)?;
    if controls.next().is_some() || control.physical_block != block.id {
        return Err(execution_lifecycle_error_v29());
    }
    let range = control
        .return_values
        .as_ref()
        .ok_or_else(execution_lifecycle_error_v29)?;
    let values = coordinates
        .values
        .rows
        .get(range.clone())
        .ok_or_else(execution_lifecycle_error_v29)?;
    match source.incoming {
        Some(call) => {
            let (target, arguments) = control
                .expected_branch
                .as_ref()
                .ok_or_else(execution_lifecycle_error_v29)?;
            if control.origin != (InstanceControlOriginV1::ExpandedReturn { call })
                || arguments != range
            {
                return Err(execution_lifecycle_error_v29());
            }
            instance_check_branch_v1(block, *target, values, budget)
                .map_err(pending_scope_correspondence_error_v29)?;
        }
        None => {
            budget.charge_work(values.len())?;
            if control.origin != InstanceControlOriginV1::Retained
                || control.expected_branch.is_some()
                || !matches!(&block.terminator, Some(Terminator::Return { values: actual }) if actual == values)
            {
                return Err(execution_lifecycle_error_v29());
            }
        }
    }
    Ok(())
}

fn lifecycle_operation_v29(
    event: DeferredLifecycleEventV29,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<Operation, ProductionSemanticKirErrorV1> {
    use fe2o3_kernel_ir::{ExecutionOperationV15 as Op, ExecutionRoleV15 as Role};
    let (result, kind) = match event.kind {
        DeferredLifecycleKindV29::Issue { result } => {
            (Some((result.value, Role::Context)), Op::ContextIssue)
        }
        DeferredLifecycleKindV29::Derive { context, result } => (
            Some((result.value, Role::Workgroup)),
            Op::WorkgroupDerive {
                context: context.value,
            },
        ),
        DeferredLifecycleKindV29::End { workgroup } => (
            None,
            Op::ScopeEnd {
                workgroup: workgroup.value,
                discarded: Vec::new(),
            },
        ),
    };
    let mut results = emission_vec_v1(usize::from(result.is_some()), budget)?;
    if let Some((value, role)) = result {
        results.push(ValueDef::new(value, Type::Execution(role)));
    }
    Ok(Operation::new(results, OperationKind::Execution(kind)))
}

fn prepare_lifecycle_events_v29(
    root: &OwnedPendingScopedRootV29,
    limits: ProductionSemanticKirLimitsV1,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<Vec<PreparedLifecycleEventV29>, ProductionSemanticKirErrorV1> {
    let pending = &root.pending;
    let coordinates = &pending.coordinates;
    let body = pending
        .function
        .body
        .as_ref()
        .ok_or_else(execution_lifecycle_error_v29)?;
    budget.charge_work(argument_sum_v1(&[
        pending.sidecars.rows.len(),
        body.blocks.len(),
        root.kernel.entry.as_str().len(),
        pending.function.id.as_str().len(),
        3,
    ])?)?;
    if pending.sidecars.rows.len() != coordinates.sources.rows.len()
        || root.kernel.entry != pending.function.id
    {
        return Err(execution_lifecycle_error_v29());
    }
    let mut count = 0;
    let mut ordinary = 0;
    for sidecar in &pending.sidecars.rows {
        count = argument_sum_v1(&[
            count,
            sidecar
                .lifecycle_events
                .as_ref()
                .ok_or_else(execution_lifecycle_error_v29)?
                .rows
                .len(),
        ])?;
    }
    for block in &body.blocks {
        budget.charge_work(block.operations.len())?;
        ordinary = argument_sum_v1(&[ordinary, block.operations.len()])?;
        if block
            .operations
            .iter()
            .any(|op| matches!(op.kind, OperationKind::Execution(_)))
        {
            return Err(execution_lifecycle_error_v29());
        }
    }
    enforce_limit(
        ProductionSemanticKirResourceV1::Operations,
        argument_sum_v1(&[ordinary, count])?,
        limits.max_operations,
    )?;
    let mut prepared = emission_vec_v1::<PreparedLifecycleEventV29>(count, budget)?;
    let mut issued = None;
    for (index, sidecar) in pending.sidecars.rows.iter().enumerate() {
        let events = sidecar
            .lifecycle_events
            .as_ref()
            .ok_or_else(execution_lifecycle_error_v29)?;
        let source = lifecycle_source_row_v29(coordinates, events, budget)?;
        if sidecar.source_call_instance != Some(source.instance) || source.instance.index() != index
        {
            return Err(execution_lifecycle_error_v29());
        }
        budget.charge_work(events.rows.len())?;
        let mut derived = events.rows.iter().filter_map(|event| match event.kind {
            DeferredLifecycleKindV29::Derive { result, .. } => Some(result),
            _ => None,
        });
        let workgroup = derived.next();
        if workgroup.is_some() != events.provider || derived.next().is_some() {
            return Err(execution_lifecycle_error_v29());
        }
        if events.provider {
            budget.charge_work(argument_sum_v1(&[
                events.rows.len(),
                coordinates.returns.rows.len(),
            ])?)?;
            let ends = events
                .rows
                .iter()
                .filter(|row| matches!(row.kind, DeferredLifecycleKindV29::End { .. }))
                .count();
            let returns = coordinates
                .returns
                .rows
                .iter()
                .filter(|row| row.instance == source.instance)
                .count();
            if ends != returns {
                return Err(execution_lifecycle_error_v29());
            }
        }
        for (event_index, event) in events.rows.iter().enumerate() {
            budget.charge_work(argument_sum_v1(&[
                coordinates.spans.rows.len(),
                events.rows.len(),
                body.blocks.len(),
                5,
            ])?)?;
            if events.placement.block(event.block.index())? != event.original_block
                || events.rows[..event_index]
                    .iter()
                    .any(|prior| prior.block == event.block)
            {
                return Err(execution_lifecycle_error_v29());
            }
            let mut spans = coordinates.spans.rows.iter().enumerate().filter(|(_, row)| {
                row.instance == source.instance && matches!(row.source, InstanceSpanSourceV1::Terminator(span)
                    if span.correspondence_owner == coordinates.root && span.semantic_function == source.function
                        && span.semantic_block == event.block && span.kernel_ir_block == event.original_block)
            });
            let (span_index, span) = spans.next().ok_or_else(execution_lifecycle_error_v29)?;
            let original = span.source.coordinates().2;
            let mapped = span.segments[0].ok_or_else(execution_lifecycle_error_v29)?;
            if spans.next().is_some()
                || span.removed_call.is_some()
                || span.segments[1].is_some()
                || original.count != mapped.count
                || event.original_gap < original.first
                || event.original_gap
                    > original
                        .end()
                        .map_err(pending_scope_correspondence_error_v29)?
            {
                return Err(execution_lifecycle_error_v29());
            }
            let gap = mapped
                .first
                .checked_add(event.original_gap - original.first)
                .ok_or(ArgumentResourceV1::Arithmetic)?;
            let mut blocks = body
                .blocks
                .iter()
                .enumerate()
                .filter(|(_, block)| block.id == mapped.block);
            let (block_index, block) = blocks.next().ok_or_else(execution_lifecycle_error_v29)?;
            if blocks.next().is_some()
                || gap as usize > block.operations.len()
                || mapped
                    .end()
                    .map_err(pending_scope_correspondence_error_v29)? as usize
                    > block.operations.len()
            {
                return Err(execution_lifecycle_error_v29());
            }
            if !matches!(event.kind, DeferredLifecycleKindV29::End { .. }) {
                budget.charge_work(coordinates.controls.rows.len())?;
                let mut controls = coordinates.controls.rows.iter().filter(|row| {
                    row.instance == source.instance
                        && row.original_block == event.original_block
                        && row.semantic_block == Some(event.block)
                });
                let control = controls.next().ok_or_else(execution_lifecycle_error_v29)?;
                if controls.next().is_some()
                    || control.physical_block != block.id
                    || control.origin != InstanceControlOriginV1::Retained
                    || control.return_values.is_some()
                    || control.expected_branch.is_some()
                {
                    return Err(execution_lifecycle_error_v29());
                }
            }
            let result = match (event.source, event.kind) {
                (
                    DeferredLifecycleSourceV29::Issuance { .. },
                    DeferredLifecycleKindV29::Issue { result },
                ) => {
                    if source.function != coordinates.root
                        || source.incoming.is_some()
                        || issued.is_some()
                        || event.original_gap != original.first
                        || events.provider
                    {
                        return Err(execution_lifecycle_error_v29());
                    }
                    issued = Some(result);
                    Some(result)
                }
                (
                    DeferredLifecycleSourceV29::Derive { .. },
                    DeferredLifecycleKindV29::Derive { context, result },
                ) => {
                    if !events.provider || Some(context) != issued || Some(result) != workgroup {
                        return Err(execution_lifecycle_error_v29());
                    }
                    Some(result)
                }
                (
                    DeferredLifecycleSourceV29::Return { .. },
                    DeferredLifecycleKindV29::End { workgroup: ended },
                ) => {
                    if !events.provider
                        || Some(ended) != workgroup
                        || event.original_gap
                            != original
                                .end()
                                .map_err(pending_scope_correspondence_error_v29)?
                    {
                        return Err(execution_lifecycle_error_v29());
                    }
                    lifecycle_return_v29(coordinates, source, event, block, gap, budget)?;
                    None
                }
                _ => return Err(execution_lifecycle_error_v29()),
            };
            if let Some(result) = result {
                if result.producer
                    != (ProductionCallOccurrenceV1 {
                        caller: source.instance,
                        block: event.block,
                    })
                    || result.value.0 >= sidecar.next_value
                {
                    return Err(execution_lifecycle_error_v29());
                }
                budget.charge_work(argument_sum_v1(&[body.parameters.len(), prepared.len()])?)?;
                if body.parameters.contains(&result.value)
                    || prepared.iter().any(|prior| {
                        prior
                            .operation
                            .as_ref()
                            .is_some_and(|op| op.results.iter().any(|def| def.id == result.value))
                    })
                {
                    return Err(execution_lifecycle_error_v29());
                }
                budget.charge_work(body.blocks.len())?;
                for block in &body.blocks {
                    budget.charge_work(argument_sum_v1(&[
                        block.parameters.len(),
                        block.operations.len(),
                    ])?)?;
                    if block.parameters.iter().any(|def| def.id == result.value) {
                        return Err(execution_lifecycle_error_v29());
                    }
                    for op in &block.operations {
                        budget.charge_work(op.results.len())?;
                        if op.results.iter().any(|def| def.id == result.value) {
                            return Err(execution_lifecycle_error_v29());
                        }
                    }
                }
            }
            prepared.push(PreparedLifecycleEventV29 {
                block: block_index,
                witness: LifecycleInsertionV29 {
                    instance: source.instance,
                    event: event_index,
                    source_span: span_index,
                    before: InstancePhysicalSpanV1 {
                        block: mapped.block,
                        first: gap,
                        count: 0,
                    },
                    after: InstancePhysicalSpanV1 {
                        block: mapped.block,
                        first: gap,
                        count: 1,
                    },
                },
                operation: Some(lifecycle_operation_v29(*event, budget)?),
            });
        }
    }
    if issued.is_some() != root.requires_context_issue {
        return Err(execution_lifecycle_error_v29());
    }
    call_splice_sort_work_v1(prepared.len(), budget)
        .map_err(|error| pending_scope_correspondence_error_v29(error.into()))?;
    prepared.sort_unstable_by_key(|row| {
        (
            row.block,
            row.witness.before.first,
            row.witness.instance.index(),
            row.witness.event,
        )
    });
    Ok(prepared)
}

#[cfg_attr(
    not(test),
    allow(dead_code, reason = "Scoped source admission remains gated")
)]
fn insert_pending_lifecycle_v29(
    donor: &mut Option<OwnedPendingScopedRootV29>,
    limits: ProductionSemanticKirLimitsV1,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<OwnedLifecycleInsertedRootV29, ProductionSemanticKirErrorV1> {
    let owner = donor.as_ref().ok_or_else(execution_lifecycle_error_v29)?;
    if owner.ledger != budget.work_ledger_identity_v1() {
        return Err(ArgumentResourceV1::Accounting.into());
    }
    let inherited = owner.retained_emission_storage;
    let floor = budget.storage();
    let consumed_floor = floor
        .checked_sub(inherited)
        .ok_or(ArgumentResourceV1::Accounting)?;
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let owner = donor.as_ref().ok_or_else(execution_lifecycle_error_v29)?;
        let mut events = prepare_lifecycle_events_v29(owner, limits, budget)?;
        let body = owner
            .pending
            .function
            .body
            .as_ref()
            .ok_or_else(execution_lifecycle_error_v29)?;
        let mut blocks = emission_vec_v1::<PreparedLifecycleBlockV29>(body.blocks.len(), budget)?;
        let mut insertions = emission_vec_v1(events.len(), budget)?;
        let scratch = argument_sum_v1(&[
            argument_product_v1(events.capacity(), size_of::<PreparedLifecycleEventV29>())?,
            argument_product_v1(blocks.capacity(), size_of::<PreparedLifecycleBlockV29>())?,
        ])?;
        let mut first = 0;
        while first < events.len() {
            let index = events[first].block;
            let mut end = first;
            while end < events.len() && events[end].block == index {
                budget.charge_work(2)?;
                let offset =
                    u32::try_from(end - first).map_err(|_| ArgumentResourceV1::Arithmetic)?;
                events[end].witness.after.first = events[end]
                    .witness
                    .before
                    .first
                    .checked_add(offset)
                    .ok_or(ArgumentResourceV1::Arithmetic)?;
                end += 1;
            }
            let count = argument_sum_v1(&[body.blocks[index].operations.len(), end - first])?;
            u32::try_from(count).map_err(|_| ArgumentResourceV1::Arithmetic)?;
            enforce_limit(
                ProductionSemanticKirResourceV1::Operations,
                count,
                MAX_BLOCK_OPERATIONS_V1,
            )?;
            blocks.push(PreparedLifecycleBlockV29 {
                index,
                events: first..end,
                operations: emission_vec_v1(count, budget)?,
            });
            budget.charge_work(argument_sum_v1(&[count, end - first, 4])?)?;
            first = end;
        }
        let retained = argument_sum_v1(&[
            inherited,
            budget
                .storage()
                .checked_sub(floor)
                .ok_or(ArgumentResourceV1::Accounting)?,
        ])?
        .checked_sub(scratch)
        .ok_or(ArgumentResourceV1::Accounting)?;
        budget.charge_work(events.len())?;
        // Every check/allocation/move charge precedes consumption. Old operation
        // buffers have mixed accounting provenance, so their capacity is not refunded.
        let mut root = donor.take().expect("validated lifecycle donor");
        let body = root
            .pending
            .function
            .body
            .as_mut()
            .expect("validated lifecycle body");
        for mut replacement in blocks.drain(..) {
            let block = &mut body.blocks[replacement.index];
            let old = std::mem::take(&mut block.operations);
            let mut originals = old.into_iter();
            let mut copied = 0;
            for event in &mut events[replacement.events] {
                while copied < event.witness.before.first as usize {
                    replacement
                        .operations
                        .push(originals.next().expect("validated lifecycle gap"));
                    copied += 1;
                }
                replacement
                    .operations
                    .push(event.operation.take().expect("single lifecycle insertion"));
            }
            replacement.operations.extend(originals);
            block.operations = replacement.operations;
        }
        insertions.extend(events.iter().map(|event| event.witness));
        drop((events, blocks));
        budget.release_storage(scratch)?;
        root.retained_emission_storage = retained;
        Ok(OwnedLifecycleInsertedRootV29 { root, insertions })
    }));
    match result {
        Ok(Ok(inserted)) => Ok(inserted),
        other => {
            let target = if donor.is_some() {
                floor
            } else {
                consumed_floor
            };
            let remaining = budget
                .storage()
                .checked_sub(target)
                .ok_or(ArgumentResourceV1::Accounting)?;
            budget.release_storage(remaining)?;
            match other {
                Ok(Err(error)) => Err(error),
                Err(payload) => std::panic::resume_unwind(payload),
                Ok(Ok(_)) => unreachable!(),
            }
        }
    }
}
