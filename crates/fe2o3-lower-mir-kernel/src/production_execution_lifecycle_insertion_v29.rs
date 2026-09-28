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
    terminal_failures: Option<TerminalFailureRelationV18>,
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
    terminal: Option<usize>,
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
        DeferredLifecycleKindV29::Tile(tile) => return tile.operation(budget),
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
    prepare_lifecycle_events_with_failures_v18(root, limits, None, budget)
}

// Join the existing compact locator to the original roster and expansion seeds.
// Reachability is still established by the independent original source plan.
fn check_lifecycle_instance_roster_v29(
    pending: &PendingScopedRootEmissionV29,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    let coordinates = &pending.coordinates;
    let sidecars = &pending.sidecars.rows;
    let count = coordinates.sources.rows.len();
    budget.charge_work(7)?;
    if count == 0
        || pending.active_instances.rows.len() != count
        || coordinates.seeds.rows.len() != sidecars.len()
        || sidecars.is_empty()
        || pending.active_instances.rows[0] != Some(0)
    {
        return Err(execution_lifecycle_error_v29());
    }
    let root = coordinates.sources.rows[0].instance;
    let mut active = 0;
    for (original, source) in coordinates.sources.rows.iter().enumerate() {
        budget.charge_work(3)?;
        if source.instance.index() != original {
            return Err(execution_lifecycle_error_v29());
        }
        let ordinal = pending
            .active_instances
            .sidecar_ordinal(original, count, sidecars, budget)?;
        let Some(ordinal) = ordinal else { continue };
        budget.charge_work(5)?;
        let seed = coordinates
            .seeds
            .rows
            .get(ordinal)
            .ok_or_else(execution_lifecycle_error_v29)?;
        if ordinal != active
            || sidecars[ordinal].source_call_instance != Some(source.instance)
            || seed.instance != source.instance
            || seed.container != root
        {
            return Err(execution_lifecycle_error_v29());
        }
        active = argument_sum_v1(&[active, 1])?;
    }
    budget.charge_work(1)?;
    if active != sidecars.len() {
        return Err(execution_lifecycle_error_v29());
    }
    Ok(())
}

fn prepare_lifecycle_events_with_failures_v18(
    root: &OwnedPendingScopedRootV29,
    limits: ProductionSemanticKirLimitsV1,
    failures: Option<&mut PreparedTerminalFailuresV18>,
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
    if root.kernel.entry != pending.function.id {
        return Err(execution_lifecycle_error_v29());
    }
    check_lifecycle_instance_roster_v29(pending, budget)?;
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
    let mut has_tiles = false;
    for sidecar in &pending.sidecars.rows {
        let events = sidecar
            .lifecycle_events
            .as_ref()
            .ok_or_else(execution_lifecycle_error_v29)?;
        let source = lifecycle_source_row_v29(coordinates, events, budget)?;
        if sidecar.source_call_instance != Some(source.instance) {
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
                    DeferredLifecycleSourceV29::Intrinsic { .. },
                    DeferredLifecycleKindV29::Tile(tile),
                ) => {
                    let range = tile.result_range()?;
                    if tile.producer
                        != (ProductionCallOccurrenceV1 {
                            caller: source.instance,
                            block: event.block,
                        })
                        || range.start < events.placement.first_value
                        || range.end > sidecar.next_value
                    {
                        return Err(execution_lifecycle_error_v29());
                    }
                    check_lifecycle_result_range_v29(range, body, &prepared, budget)?;
                    has_tiles = true;
                    None
                }
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
                check_lifecycle_result_range_v29(
                    result.value.0
                        ..result
                            .value
                            .0
                            .checked_add(1)
                            .ok_or(ArgumentResourceV1::Arithmetic)?,
                    body,
                    &prepared,
                    budget,
                )?;
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
    if let Some(failures) = failures {
        prepare_lifecycle_ownership_v18(body, &mut prepared, Some(failures), budget)?;
    } else if has_tiles {
        prepare_tile_discards_v29(body, &mut prepared, budget)?;
    }
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
    insert_pending_lifecycle_inner_v18(donor, limits, false, budget)
}

fn insert_pending_lifecycle_with_failures_v18(
    donor: &mut Option<OwnedPendingScopedRootV29>,
    limits: ProductionSemanticKirLimitsV1,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<OwnedLifecycleInsertedRootV29, ProductionSemanticKirErrorV1> {
    insert_pending_lifecycle_inner_v18(donor, limits, true, budget)
}

fn insert_pending_lifecycle_inner_v18(
    donor: &mut Option<OwnedPendingScopedRootV29>,
    limits: ProductionSemanticKirLimitsV1,
    terminal: bool,
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
        let terminal_headers = if terminal {
            argument_sum_v1(&[
                size_of::<Option<PreparedTerminalFailuresV18>>(),
                size_of::<Vec<TerminalFailureClosureV18>>(),
                2 * size_of::<Vec<BasicBlock>>(),
            ])?
        } else {
            0
        };
        budget.reserve_storage(terminal_headers)?;
        let mut failures = if terminal {
            Some(PreparedTerminalFailuresV18::prepare(owner, limits, budget)?)
        } else {
            None
        };
        let mut events =
            prepare_lifecycle_events_with_failures_v18(owner, limits, failures.as_mut(), budget)?;
        let body = owner
            .pending
            .function
            .body
            .as_ref()
            .ok_or_else(execution_lifecycle_error_v29)?;
        let mut blocks = emission_vec_v1::<PreparedLifecycleBlockV29>(body.blocks.len(), budget)?;
        let mut insertions = emission_vec_v1(events.len(), budget)?;
        let mut scratch = argument_sum_v1(&[
            terminal_headers,
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
                terminal: None,
            });
            budget.charge_work(argument_sum_v1(&[count, end - first, 4])?)?;
            first = end;
        }
        let mut terminal_closures = Vec::new();
        let mut generated_blocks = Vec::new();
        let mut joined_blocks = Vec::new();
        if let Some(failures) = &mut failures {
            terminal_closures = emission_vec_v1(failures.rows.len(), budget)?;
            generated_blocks = emission_vec_v1(failures.generated.len(), budget)?;
            joined_blocks = emission_vec_v1(failures.block_count(body.blocks.len())?, budget)?;
            prepare_terminal_replacements_v18(
                owner,
                &events,
                failures,
                &mut blocks,
                &mut generated_blocks,
                &mut terminal_closures,
                limits,
                budget,
            )?;
            scratch = argument_sum_v1(&[
                scratch,
                failures.scratch_storage()?,
                terminal_failure_operation_storage_v18(&failures.rows)?,
                argument_product_v1(generated_blocks.capacity(), size_of::<BasicBlock>())?,
            ])?;
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
            if let Some(row) = replacement.terminal {
                let failure = &mut failures.as_mut().expect("validated terminal plan").rows[row];
                while copied < failure.witness.original_gap as usize {
                    replacement
                        .operations
                        .push(originals.next().expect("validated terminal gap"));
                    copied += 1;
                }
                replacement.operations.append(&mut failure.operations);
            }
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
        let terminal_failures = if let Some(failures) = &mut failures {
            install_terminal_edges_v18(body, &root.terminal_failures, failures);
            joined_blocks.append(&mut body.blocks);
            joined_blocks.append(&mut generated_blocks);
            body.blocks = joined_blocks;
            let source = root.terminal_failures.source;
            let ledger = root.terminal_failures.ledger;
            let origins = std::mem::replace(
                &mut root.terminal_failures,
                TerminalFailureOriginsV18 {
                    source,
                    ledger,
                    rows: Vec::new(),
                },
            );
            Some(TerminalFailureRelationV18 {
                origins,
                closures: terminal_closures,
            })
        } else {
            None
        };
        insertions.extend(events.iter().map(|event| event.witness));
        drop((events, blocks, failures, generated_blocks));
        budget.release_storage(scratch)?;
        root.retained_emission_storage = retained;
        Ok(OwnedLifecycleInsertedRootV29 {
            root,
            insertions,
            terminal_failures,
        })
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
