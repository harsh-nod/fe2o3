use super::*;

type Site = (usize, u32, usize, usize);
type Insertion = (usize, usize, usize);

pub(super) fn require_site(
    rows: &[Site],
    key: (usize, u32),
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<Site> {
    let index = find(rows, key, |row| (row.0, row.1), budget)?.ok_or(
        ProductionSourceOwnedViewErrorV18::Binding("execution recipe missing original event"),
    )?;
    Ok(rows[index])
}

pub(super) fn require_insertion(
    rows: &[Insertion],
    key: (usize, usize),
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<usize> {
    let index = find(rows, key, |row| (row.0, row.1), budget)?.ok_or(
        ProductionSourceOwnedViewErrorV18::Binding("execution recipe missing original insertion"),
    )?;
    Ok(rows[index].2)
}

pub(super) fn check_event(
    expected: DeferredLifecycleSourceV29,
    block: SemanticBlockIdV1,
    event: DeferredLifecycleEventV29,
) -> SourceOwnedResultV18<()> {
    if event.source != expected || event.block != block {
        return resources::binding("execution recipe foreign original event");
    }
    Ok(())
}

pub(super) fn find<T, K: Copy + Ord>(
    rows: &[T],
    key: K,
    project: impl Fn(&T) -> K,
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<Option<usize>> {
    let (mut start, mut end) = (0, rows.len());
    while start < end {
        budget.charge_work(1)?;
        let middle = start + (end - start) / 2;
        match project(&rows[middle]).cmp(&key) {
            std::cmp::Ordering::Less => start = middle + 1,
            std::cmp::Ordering::Greater => end = middle,
            std::cmp::Ordering::Equal => return Ok(Some(middle)),
        }
    }
    budget.charge_work(1)?;
    Ok(None)
}

pub(super) fn sort_unique<T, const N: usize>(
    rows: &mut [T],
    key: impl Fn(&T) -> [usize; N] + Copy,
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<()> {
    private_array_heapsort_v1(rows, key, &mut SourceCorrespondenceWorkV18(budget), || {
        ArgumentResourceV1::Arithmetic.into()
    })?;
    for adjacent in rows.windows(2) {
        budget.charge_work(1)?;
        if key(&adjacent[0]) == key(&adjacent[1]) {
            return resources::binding("execution recipe duplicate original site or insertion");
        }
    }
    Ok(())
}

pub(super) fn build(
    view: &ProductionOptimizedSourceCorrespondenceV18<'_>,
    original: &ExecutionLifecycleSourceV29<'_>,
    capacity: usize,
    rows: &mut Vec<Recipe>,
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<()> {
    let source = view.original.source;
    for root_ordinal in 0..source.owner.inner.pending.roots.len() {
        budget.charge_work(1)?;
        let root = source.root_row(root_ordinal)?;
        let scratch = budget.storage();
        production_call_instances_v1::with_production_call_instances_v1(
            original.owner,
            root.coordinates.root,
            budget,
            |instances, budget| {
                Ok::<_, ProductionSemanticKirErrorV1>(view.retain(build_root(
                    view,
                    original,
                    root_ordinal,
                    root,
                    instances,
                    capacity,
                    rows,
                    budget,
                )))
            },
        )
        .map_err(source_error)??;
        // All borrowed plan and census indexes have dropped. Only caller-owned
        // fixed recipe backing (reserved before this floor) survives the query.
        view.check(budget)?;
        budget.release_storage(
            budget
                .storage()
                .checked_sub(scratch)
                .ok_or(ArgumentResourceV1::Accounting)?,
        )?;
    }
    budget.charge_work(1)?;
    if rows.len() != capacity {
        return resources::binding("execution recipe complete original census");
    }
    Ok(())
}

fn build_root(
    view: &ProductionOptimizedSourceCorrespondenceV18<'_>,
    original: &ExecutionLifecycleSourceV29<'_>,
    root_ordinal: usize,
    root: &ScopedModuleRootV29,
    instances: &ExecutionInstancesV29<'_>,
    capacity: usize,
    rows: &mut Vec<Recipe>,
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<()> {
    let source = view.original.source;
    budget.charge_work(argument_sum_v1(&[root.sidecars.rows.len(), 3])?)?;
    if root.coordinates.sources.rows.len() != instances.instances().len()
        || !std::ptr::eq(original.owner, instances.owner())
    {
        return resources::binding("execution recipe original instance census");
    }
    let mut count = 0;
    for sidecar in &root.sidecars.rows {
        let events =
            sidecar
                .lifecycle_events
                .as_ref()
                .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                    "execution recipe absent original events",
                ))?;
        count = argument_sum_v1(&[count, events.rows.len()])?;
    }
    if count != root.insertions.len()
        || count
            > capacity
                .checked_sub(rows.len())
                .ok_or(ArgumentResourceV1::Accounting)?
    {
        return resources::binding("execution recipe complete insertion census");
    }
    budget.reserve_storage(size_of::<Vec<Site>>() + size_of::<Vec<Insertion>>())?;
    let mut sites = resources::vector(count, budget)?;
    let mut insertions = resources::vector(count, budget)?;
    let mut issued = None;
    for (sidecar_index, sidecar) in root.sidecars.rows.iter().enumerate() {
        let events =
            sidecar
                .lifecycle_events
                .as_ref()
                .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                    "execution recipe absent original events",
                ))?;
        budget.charge_work(2)?;
        if sidecar.source_call_instance != Some(events.instance) {
            return resources::binding("execution recipe foreign original instance");
        }
        for (event_index, event) in events.rows.iter().enumerate() {
            budget.charge_work(2)?;
            if sites.len() >= count {
                return resources::binding("execution recipe original capacity");
            }
            sites.push((
                events.instance.index(),
                event.block.index(),
                sidecar_index,
                event_index,
            ));
            if let DeferredLifecycleKindV29::Issue { result } = event.kind {
                if issued.replace(result).is_some() || events.instance != instances.root() {
                    return resources::binding("execution recipe original context issuance");
                }
            }
        }
    }
    if issued.is_some() != root.requires_context_issue {
        return resources::binding("execution recipe original context issuance");
    }
    for (index, insertion) in root.insertions.iter().enumerate() {
        budget.charge_work(1)?;
        if insertions.len() >= count {
            return resources::binding("execution recipe insertion capacity");
        }
        insertions.push((insertion.instance.index(), insertion.event, index));
    }
    // Only within-block ranks affect relocation. Sparse block IDs need not be
    // in inventory order, and the locator vector itself is not authoritative.
    sort_unique(
        &mut insertions,
        |row| {
            let insertion = &root.insertions[row.2];
            [
                insertion.before.block.0 as usize,
                insertion.before.first as usize,
                row.0,
                row.1,
            ]
        },
        budget,
    )?;
    let mut previous = None;
    let mut offset = 0u32;
    for row in &insertions {
        budget.charge_work(4)?;
        let insertion = &root.insertions[row.2];
        if previous != Some(insertion.before.block) {
            offset = 0;
        }
        if insertion.before.count != 0
            || insertion.after.count != 1
            || insertion.before.block != insertion.after.block
            || insertion.before.first.checked_add(offset) != Some(insertion.after.first)
        {
            return resources::binding("execution recipe insertion relocation");
        }
        offset = offset
            .checked_add(1)
            .ok_or(ArgumentResourceV1::Arithmetic)?;
        previous = Some(insertion.before.block);
    }
    sort_unique(&mut sites, |row| [row.0, row.1 as usize], budget)?;
    sort_unique(&mut insertions, |row| [row.0, row.1], budget)?;
    let mut matched = 0;
    for (instance_index, instance) in instances.instances().iter().enumerate() {
        budget.charge_work(3)?;
        let id =
            instances
                .id_at(instance_index)
                .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                    "execution recipe original instance identity",
                ))?;
        let sidecar = source.optional_sidecar(root_ordinal, instance_index, budget)?;
        if sidecar.is_some() != (instances.instance_reachable(id) == Some(true)) {
            return resources::binding("execution recipe active original census");
        }
        let Some(sidecar) = sidecar else { continue };
        let events =
            sidecar
                .lifecycle_events
                .as_ref()
                .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                    "execution recipe absent original events",
                ))?;
        events
            .check_identity(instances, id, budget)
            .map_err(source_error)?;
        let provider = matches!(
            original
                .input
                .classes
                .get(instance.function().index() as usize),
            Some(ProductionScopeCallableCandidateV29::Provider { .. })
        );
        if events.provider != provider || events.expected_rows != events.rows.len() {
            return resources::binding("execution recipe original provider census");
        }
        let mut workgroup = None;
        for event in &events.rows {
            budget.charge_work(1)?;
            if let DeferredLifecycleKindV29::Derive { result, .. } = event.kind {
                if workgroup.replace(result).is_some() {
                    return resources::binding("execution recipe original workgroup derivation");
                }
            }
        }
        if provider != workgroup.is_some() {
            return resources::binding("execution recipe original workgroup derivation");
        }
        for block_index in 0..instance.declaration().blocks().len() {
            budget.charge_work(2)?;
            let block = SemanticBlockIdV1::from_index(
                u32::try_from(block_index).map_err(|_| ArgumentResourceV1::Arithmetic)?,
            );
            if !instance
                .ssa()
                .plan()
                .is_reachable(SsaBlockIdV1::new(block.index()))
                || !instances.block_reachable(id, block).ok_or(
                    ProductionSourceOwnedViewErrorV18::Binding(
                        "execution recipe original block reachability",
                    ),
                )?
            {
                continue;
            }
            let Some(expected) = expected_lifecycle_source_event_v29(
                original,
                instance.declaration(),
                instance.function(),
                root.coordinates.root,
                provider,
                block,
                budget,
            )
            .map_err(source_error)?
            else {
                continue;
            };
            let site = require_site(&sites, (instance_index, block.index()), budget)?;
            let event =
                events
                    .rows
                    .get(site.3)
                    .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                        "execution recipe foreign original event",
                    ))?;
            if !std::ptr::eq(&root.sidecars.rows[site.2], sidecar) {
                return resources::binding("execution recipe foreign original event");
            }
            check_event(expected, block, *event)?;
            check_identity(
                original,
                instance.declaration(),
                events,
                *event,
                issued,
                workgroup,
                budget,
            )?;
            let insertion = require_insertion(&insertions, (instance_index, site.3), budget)?;
            let insertion = &root.insertions[insertion];
            let coordinate = check_insertion(view, root, events, *event, insertion, budget)?;
            let input_index = resources::operation_index(view.checked.input(), coordinate, budget)?;
            let kind = check_payload(
                event.kind,
                view.checked.input().operations()[input_index].operation,
                budget,
            )?;
            let output = join_output(view, coordinate, budget)?;
            if rows.len() >= capacity || rows.len() >= rows.capacity() {
                return resources::binding("execution recipe result capacity");
            }
            rows.push(Recipe {
                root: root_ordinal,
                instance: instance_index,
                block,
                kind,
                input: coordinate,
                output,
            });
            matched = argument_sum_v1(&[matched, 1])?;
        }
    }
    budget.charge_work(1)?;
    if matched != count {
        return resources::binding("execution recipe unmatched original event");
    }
    Ok(())
}

fn check_identity(
    original: &ExecutionLifecycleSourceV29<'_>,
    function: &SemanticFunctionDeclV1,
    events: &PendingLifecycleEventsV29,
    event: DeferredLifecycleEventV29,
    issued: Option<SemanticExecutionIdentityV29>,
    workgroup: Option<SemanticExecutionIdentityV29>,
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<()> {
    budget.charge_work(6)?;
    let (result, role) = match (event.source, event.kind) {
        (
            DeferredLifecycleSourceV29::Issuance { .. },
            DeferredLifecycleKindV29::Issue { result },
        ) if Some(result) == issued && !events.provider => {
            (Some(result), SemanticExecutionRoleV29::KernelContext)
        }
        (
            DeferredLifecycleSourceV29::Derive { .. },
            DeferredLifecycleKindV29::Derive { context, result },
        ) if Some(context) == issued && Some(result) == workgroup && events.provider => {
            (Some(result), SemanticExecutionRoleV29::Workgroup)
        }
        (
            DeferredLifecycleSourceV29::Return { .. },
            DeferredLifecycleKindV29::End { workgroup: ended },
        ) if Some(ended) == workgroup && events.provider => {
            (None, SemanticExecutionRoleV29::Workgroup)
        }
        _ => return resources::binding("execution recipe original event role or identity"),
    };
    if let Some(result) = result {
        let block = function.blocks().get(event.block.index() as usize).ok_or(
            ProductionSourceOwnedViewErrorV18::Binding("execution recipe original result block"),
        )?;
        let SemanticTerminatorKindV1::Call(call) = block.terminator().kind() else {
            return resources::binding("execution recipe original result producer");
        };
        if result.producer
            != (ProductionCallOccurrenceV1 {
                caller: events.instance,
                block: event.block,
            })
            || call
                .destination()
                .is_none_or(|destination| destination.place().ty() != result.semantic_type)
        {
            return resources::binding("execution recipe original result producer");
        }
        result
            .check_type(original.owner.source_semantic().types(), role)
            .map_err(|_| {
                ProductionSourceOwnedViewErrorV18::Binding(
                    "execution recipe original result nominal type",
                )
            })?;
    }
    Ok(())
}

pub(super) fn check_insertion(
    view: &ProductionOptimizedSourceCorrespondenceV18<'_>,
    root: &ScopedModuleRootV29,
    events: &PendingLifecycleEventsV29,
    event: DeferredLifecycleEventV29,
    insertion: &LifecycleInsertionV29,
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<OpCoordinate> {
    budget.charge_work(8)?;
    let span = root
        .coordinates
        .spans
        .rows
        .get(insertion.source_span)
        .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
            "execution recipe original insertion span",
        ))?;
    if span.instance != events.instance
        || insertion.instance != events.instance
        || events
            .placement
            .block(event.block.index())
            .map_err(source_error)?
            != event.original_block
        || !matches!(span.source, InstanceSpanSourceV1::Terminator(source) if source.correspondence_owner == root.coordinates.root && source.semantic_function == events.function && source.semantic_block == event.block && source.kernel_ir_block == event.original_block)
    {
        return resources::binding("execution recipe foreign insertion span");
    }
    let original = span.source.coordinates().2;
    let mapped = span.segments[0].ok_or(ProductionSourceOwnedViewErrorV18::Binding(
        "execution recipe absent insertion segment",
    ))?;
    let end = original.end().map_err(|_| {
        ProductionSourceOwnedViewErrorV18::Binding("execution recipe original insertion range")
    })?;
    if span.removed_call.is_some()
        || span.segments[1].is_some()
        || original.count != mapped.count
        || event.original_gap < original.first
        || event.original_gap > end
        || (matches!(event.kind, DeferredLifecycleKindV29::End { .. }) && event.original_gap != end)
        || (matches!(event.kind, DeferredLifecycleKindV29::Issue { .. })
            && event.original_gap != original.first)
        || insertion.before.block != mapped.block
        || mapped
            .first
            .checked_add(event.original_gap - original.first)
            != Some(insertion.before.first)
    {
        return resources::binding("execution recipe original insertion gap");
    }
    let function = view
        .checked
        .input()
        .functions()
        .get(root.function_ordinal)
        .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
            "execution recipe original root function",
        ))?
        .coordinate;
    let block = view
        .checked
        .input()
        .block_for_id(function, insertion.after.block, budget)
        .map_err(source_pointer_inventory_error_v18)?
        .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
            "execution recipe original physical block",
        ))?;
    Ok(OpCoordinate {
        block: block.coordinate,
        operation: insertion.after.first,
    })
}
