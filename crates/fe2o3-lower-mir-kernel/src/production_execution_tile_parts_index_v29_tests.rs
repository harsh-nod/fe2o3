use super::*;

fn check_container_parts(
    map: &ProductionInstanceCorrespondenceV1<'_, '_>,
    sidecars: &[PendingInstanceSidecarsV29],
    active: &PendingActiveInstanceIndexV1,
    container: ProductionCallInstanceIdV1,
    function: &Function,
    tiles: &[DeferredTileEventV29; 2],
    present: [bool; 2],
    budget: &mut ArgumentBudgetV1<'_>,
) {
    let floor = budget.storage();
    let view =
        ScopedDeferredScalarViewV29::for_container(map, sidecars, active, container, function, budget)
            .unwrap();
    assert_eq!(
        view.definitions,
        present.into_iter().filter(|value| *value).count() * 4
    );
    let mut scratch = 0;
    let ordinary =
        call_splice_index_with_deferred_parts_v29(function, None, budget, &mut scratch).unwrap();
    assert_parts_lookups(&ordinary, tiles, [false, false], budget).unwrap();
    let scoped =
        call_splice_index_with_deferred_parts_v29(function, Some(&view), budget, &mut scratch)
            .unwrap();
    assert_parts_lookups(&scoped, tiles, present, budget).unwrap();
    assert_eq!(
        scoped.values.len(),
        ordinary.values.len() + view.definitions
    );
    assert_eq!(scoped.blocks, ordinary.blocks);
    let retained = (ordinary.blocks.len() + scoped.blocks.len()) * std::mem::size_of::<BlockId>()
        + (ordinary.values.len() + scoped.values.len()) * std::mem::size_of::<(ValueId, &Type)>();
    assert_eq!(scratch, retained);
    assert_eq!(budget.storage(), floor + scratch);
    drop((ordinary, scoped));
    budget.release_storage(scratch).unwrap();
    assert_eq!(budget.storage(), floor);
}

fn live_container_parts(
    _source: &ExecutionLifecycleSourceV29<'_>,
    instances: &ExecutionInstancesV29<'_>,
    emitted: &mut [Option<LoweredFunctionResultV1>],
    receipt: &OwnedScopedSourceSlotsV29,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    check_receipt(instances, emitted, receipt, budget);
    let children = tile_children(instances, emitted);
    let tiles =
        children.map(|id| retained_parts_tile(emitted[id.index()].as_ref().unwrap()).unwrap());
    let callback = instances.incoming(children[0]).unwrap().occurrence().caller;
    let root = instances.root();
    assert_ne!(callback, root);
    let mut next_block = pending_scope_preflight_v29(
        instances,
        emitted,
        ProductionSemanticKirLimitsV1::default(),
        budget,
    )?;
    let floor = budget.storage();
    with_canonical_call_scratch_v1(budget, |budget| {
        with_production_instance_correspondence_v1(instances, budget, |map, budget| {
            for (index, row) in emitted.iter().enumerate() {
                map.append_lowered(
                    instances.id_at(index).unwrap(),
                    row.as_ref().unwrap(),
                    budget,
                )?;
            }
            let (mut functions, sidecars): (Vec<_>, Vec<_>) = emitted
                .iter_mut()
                .map(|row| {
                    let (function, sidecar) =
                        PendingInstanceSidecarsV29::split(row.take().unwrap());
                    (Some(function), sidecar)
                })
                .unzip();
            let active = pending_active_instance_index_v1(instances, &sidecars, budget)
                .map_err(instance_anchor_error_v1)?;
            check_container_parts(
                map,
                &sidecars,
                &active,
                callback,
                functions[callback.index()].as_ref().unwrap(),
                &tiles,
                [false, false],
                budget,
            );
            let mut present = [false, false];
            // Match production's reverse child order; the other sibling remains independent.
            for ordinal in [1, 0] {
                let child = children[ordinal];
                let call = instances.incoming(child).unwrap();
                let caller = functions[callback.index()].take().unwrap();
                let callee = functions[child.index()].take().unwrap();
                let expanded = map.splice_with_scoped_parts_v29(
                    call,
                    caller,
                    callee,
                    BlockId(next_block),
                    BlockId(next_block + 1),
                    None,
                    &sidecars,
                    &active,
                    budget,
                )?;
                next_block += 2;
                functions[callback.index()] = Some(expanded.caller);
                present[ordinal] = true;
                check_container_parts(
                    map,
                    &sidecars,
                    &active,
                    callback,
                    functions[callback.index()].as_ref().unwrap(),
                    &tiles,
                    present,
                    budget,
                );
                check_container_parts(
                    map,
                    &sidecars,
                    &active,
                    root,
                    functions[root.index()].as_ref().unwrap(),
                    &tiles,
                    [false, false],
                    budget,
                );
                for sibling in 0..2 {
                    if !present[sibling] {
                        let mut only_sibling = [false, false];
                        only_sibling[sibling] = true;
                        check_container_parts(
                            map,
                            &sidecars,
                            &active,
                            children[sibling],
                            functions[children[sibling].index()].as_ref().unwrap(),
                            &tiles,
                            only_sibling,
                            budget,
                        );
                    }
                }
            }
            assert_eq!(present, [true, true]);
            check_mapped_parts_refusals(
                map,
                &sidecars,
                &active,
                callback,
                functions[callback.index()].as_ref().unwrap(),
                children,
                budget,
            );
            Ok::<_, InstanceCorrespondenceErrorV1>(())
        })
        .map_err(pending_scope_correspondence_error_v29)
    })?;
    assert_eq!(budget.storage(), floor);
    assert!(emitted.iter().all(Option::is_none));
    Err(unsupported(0, None, None, STOP))
}

#[test]
fn deferred_parts_follow_only_live_spliced_container_membership() {
    run_tile_index_observer(live_container_parts);
}

#[derive(Clone, Copy, Debug)]
enum MappedPartsFault {
    SourceSubstitution,
    InstanceSubstitution,
    SegmentSubstitution,
    MissingSegment,
    SecondSegment,
    RemovedCall,
    DuplicateSourceRow,
    ControlOrigin,
    ControlReturn,
    Owner,
}

fn mapped_parts_rows(
    map: &ProductionInstanceCorrespondenceV1<'_, '_>,
    sidecars: &[PendingInstanceSidecarsV29],
    instance: ProductionCallInstanceIdV1,
) -> (usize, usize) {
    let sidecar = &sidecars[instance.index()];
    let mut events = sidecar
        .lifecycle_events
        .as_ref()
        .unwrap()
        .rows
        .iter()
        .filter(|event| {
            matches!(event.kind, DeferredLifecycleKindV29::Tile(tile)
            if matches!(tile.input, DeferredTileInputV29::Parts { .. }))
        });
    let event = events.next().unwrap();
    assert!(events.next().is_none());
    let mut sources = sidecar
        .terminator_operation_spans
        .iter()
        .filter(|span| span.semantic_block == event.block);
    let source = *sources.next().unwrap();
    assert!(sources.next().is_none());
    let mut spans = map.spans.rows.iter().enumerate().filter(|(_, row)| {
        row.instance == instance && row.source == InstanceSpanSourceV1::Terminator(source)
    });
    let span = spans.next().unwrap().0;
    assert!(spans.next().is_none());
    let mut controls = map.controls.rows.iter().enumerate().filter(|(_, row)| {
        row.instance == instance
            && row.original_block == event.original_block
            && row.semantic_block == Some(event.block)
    });
    let control = controls.next().unwrap().0;
    assert!(controls.next().is_none());
    (span, control)
}

fn check_mapped_parts_refusals(
    map: &mut ProductionInstanceCorrespondenceV1<'_, '_>,
    sidecars: &[PendingInstanceSidecarsV29],
    active: &PendingActiveInstanceIndexV1,
    container: ProductionCallInstanceIdV1,
    function: &Function,
    children: [ProductionCallInstanceIdV1; 2],
    budget: &mut ArgumentBudgetV1<'_>,
) {
    let (target, control) = mapped_parts_rows(map, sidecars, children[0]);
    let (sibling, _) = mapped_parts_rows(map, sidecars, children[1]);
    assert_ne!(target, sibling);
    let spans = map.spans.rows.clone();
    let controls = map.controls.rows.clone();
    let owner = map.owner;
    let ledger = map.ledger;
    let seed_coordinates: Vec<_> = map
        .seeds
        .rows
        .iter()
        .map(|row| (row.instance, row.container, row.parameters.clone()))
        .collect();
    let floor = budget.storage();
    let source_identity = *map.plan.owner().source_semantic_sha256();
    let incoming = map.plan.incoming(children[0]).unwrap().occurrence();
    let original_segment = spans[target].segments[0].unwrap();
    let sibling_segment = spans[sibling].segments[0].unwrap();
    assert_ne!(original_segment.block, sibling_segment.block);
    assert_ne!(spans[target].source, spans[sibling].source);
    assert_eq!(original_segment.count, sibling_segment.count);
    assert_eq!(
        map.seeds
            .rows
            .iter()
            .find(|row| row.instance == children[0])
            .unwrap()
            .container,
        container
    );
    assert_eq!(
        map.seeds
            .rows
            .iter()
            .find(|row| row.instance == children[1])
            .unwrap()
            .container,
        container
    );
    assert_eq!(
        ScopedDeferredScalarViewV29::for_container(map, sidecars, active, container, function, budget)
            .unwrap()
            .definitions,
        8
    );
    for fault in [
        MappedPartsFault::SourceSubstitution,
        MappedPartsFault::InstanceSubstitution,
        MappedPartsFault::SegmentSubstitution,
        MappedPartsFault::MissingSegment,
        MappedPartsFault::SecondSegment,
        MappedPartsFault::RemovedCall,
        MappedPartsFault::DuplicateSourceRow,
        MappedPartsFault::ControlOrigin,
        MappedPartsFault::ControlReturn,
        MappedPartsFault::Owner,
    ] {
        match fault {
            MappedPartsFault::SourceSubstitution => {
                map.spans.rows[target].source = spans[sibling].source;
            }
            MappedPartsFault::InstanceSubstitution => {
                map.spans.rows[target].instance = children[1];
            }
            MappedPartsFault::SegmentSubstitution => {
                map.spans.rows[target].segments = spans[sibling].segments;
            }
            MappedPartsFault::MissingSegment => {
                map.spans.rows[target].segments = [None, None];
            }
            MappedPartsFault::SecondSegment => {
                map.spans.rows[target].segments[1] = Some(original_segment);
            }
            MappedPartsFault::RemovedCall => {
                map.spans.rows[target].removed_call = Some(incoming);
            }
            MappedPartsFault::DuplicateSourceRow => {
                map.spans.rows[sibling] = spans[target];
            }
            MappedPartsFault::ControlOrigin => {
                map.controls.rows[control].origin =
                    InstanceControlOriginV1::ExpandedReturn { call: incoming };
            }
            MappedPartsFault::ControlReturn => {
                map.controls.rows[control].return_values = Some(0..0);
            }
            MappedPartsFault::Owner => {
                let other = map.plan.instance(children[0]).unwrap().function();
                assert_ne!(owner, Some(other));
                map.owner = Some(other);
            }
        }
        map.check_coordinates(container, function, budget)
            .unwrap_or_else(|error| panic!("coordinate guard masked {fault:?}: {error:?}"));
        let result =
            ScopedDeferredScalarViewV29::for_container(map, sidecars, active, container, function, budget)
                .map(|_| ());
        map.spans.rows[target] = spans[target];
        map.spans.rows[sibling] = spans[sibling];
        map.controls.rows[control] = controls[control].clone();
        map.owner = owner;
        assert_source_refusal(result);
        assert_eq!(map.spans.rows, spans);
        assert_eq!(map.controls.rows, controls);
        assert_eq!(budget.storage(), floor);
        assert_eq!(
            ScopedDeferredScalarViewV29::for_container(map, sidecars, active, container, function, budget)
                .unwrap()
                .definitions,
            8
        );
    }
    let mut foreign_work = CanonicalKernelIrWorkBudgetV1::new(INDEX_LIMIT);
    let mut foreign = ArgumentBudgetV1::new(&mut foreign_work, INDEX_LIMIT);
    foreign.reserve_storage(29).unwrap();
    assert_ledger_refusal(
        ScopedDeferredScalarViewV29::for_container(
            map,
            sidecars,
            active,
            container,
            function,
            &mut foreign,
        )
        .map(|_| ()),
    );
    assert_eq!(foreign.storage(), 29);
    assert_eq!(budget.storage(), floor);
    assert!(map.ledger == ledger);
    assert!(budget.work_ledger_identity_v1() == ledger);
    assert_eq!(*map.plan.owner().source_semantic_sha256(), source_identity);
    assert_eq!(
        map.seeds
            .rows
            .iter()
            .map(|row| (row.instance, row.container, row.parameters.clone()))
            .collect::<Vec<_>>(),
        seed_coordinates
    );
}

fn exact_nonempty_view_boundary(
    _source: &ExecutionLifecycleSourceV29<'_>,
    instances: &ExecutionInstancesV29<'_>,
    emitted: &mut [Option<LoweredFunctionResultV1>],
    receipt: &OwnedScopedSourceSlotsV29,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    let child = tile_children(instances, emitted)[0];
    view_boundaries(instances, emitted, receipt, budget, false, child, 4)
}

fn short_nonempty_view_boundary(
    _source: &ExecutionLifecycleSourceV29<'_>,
    instances: &ExecutionInstancesV29<'_>,
    emitted: &mut [Option<LoweredFunctionResultV1>],
    receipt: &OwnedScopedSourceSlotsV29,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    let child = tile_children(instances, emitted)[0];
    view_boundaries(instances, emitted, receipt, budget, true, child, 4)
}

#[test]
fn source_bound_nonempty_views_have_exact_work_and_storage_boundaries() {
    run_tile_index_observer(exact_nonempty_view_boundary);
    run_tile_index_observer(short_nonempty_view_boundary);
}

thread_local! {
    static PENDING_PARTS_STORAGE: std::cell::Cell<(usize, u64, u32)> =
        const { std::cell::Cell::new((0, 0, 1)) };
    static PENDING_PARTS_SOURCE: std::cell::Cell<(u32, u32)> =
        const { std::cell::Cell::new((7, 0)) };
}

fn run_pending_parts_observer(observer: ScopedSlotObserverV29) {
    for (fixture, storage, source) in [
        (ScopedFixture::TilePartsRepeated, (0, 0, 1), (7, 0)),
        (ScopedFixture::TilePartsRepeatedSlots, (2, 8, 4), (7, 0)),
        (ScopedFixture::TilePartsEntrySlots, (2, 8, 4), (4, 1)),
    ] {
        PENDING_PARTS_STORAGE.set(storage);
        PENDING_PARTS_SOURCE.set(source);
        let (result, _, _) = run(false, fixture, observer, INDEX_LIMIT, INDEX_LIMIT);
        assert!(is_stopped(&result), "{fixture:?}: {result:?}");
        assert_eq!(OBSERVED.get(), 1);
    }
}

fn with_pending_parts(
    instances: &ExecutionInstancesV29<'_>,
    emitted: &mut [Option<LoweredFunctionResultV1>],
    receipt: &OwnedScopedSourceSlotsV29,
    budget: &mut ArgumentBudgetV1<'_>,
    consume: impl FnOnce(
        &ExecutionInstancesV29<'_>,
        &mut PendingScopedRootEmissionV29,
        [ProductionCallInstanceIdV1; 2],
        &[DeferredTileEventV29; 2],
        &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), ProductionSemanticKirErrorV1>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    let (scalar_local, span_shift) = PENDING_PARTS_SOURCE.get();
    check_receipt_with_scalar_local(instances, emitted, receipt, budget, scalar_local);
    let children = tile_children(instances, emitted);
    let tiles =
        children.map(|id| retained_parts_tile(emitted[id.index()].as_ref().unwrap()).unwrap());
    let expected = PENDING_PARTS_STORAGE.get();
    let source_spans = children.map(|child| {
        let lowered = emitted[child.index()].as_ref().unwrap();
        let tile = retained_parts_tile(lowered).unwrap();
        let mut spans = lowered
            .terminator_operation_spans
            .iter()
            .filter(|span| span.semantic_block == tile.producer.block);
        let origin = *spans.next().unwrap();
        assert!(spans.next().is_none());
        (
            child,
            origin,
            lowered.function.body.as_ref().unwrap().blocks[0].id,
        )
    });
    assert!(receipt.instances[0].slots.is_empty());
    assert_eq!(receipt.slots.len(), expected.0);
    let mut owners = BTreeSet::new();
    for slot in &receipt.slots {
        assert!(children.contains(&slot.instance));
        assert!(owners.insert(slot.instance.index()));
        assert_eq!(slot.legacy_local().unwrap(), scalar_local);
        let scalar = slot.scalar_array().unwrap();
        assert_eq!((scalar.length, scalar.bytes), (1, 4));
        assert!(scalar.count.is_none());
    }
    let floor = budget.storage();
    with_canonical_call_scratch_v1(budget, |budget| {
        let prepared =
            scoped_slot_relocation_v29::prepare(instances, emitted, receipt, 1024, budget)?;
        let mut pending = prepared.assemble(ProductionSemanticKirLimitsV1::default(), budget)?;
        assert!(emitted.iter().all(Option::is_none));
        let storage = pending.slot_relocation.as_ref().unwrap().storage();
        let mut shifted = 0;
        for (child, origin, entry) in source_spans {
            let source = InstanceSpanSourceV1::Terminator(origin);
            let before = source.coordinates().2;
            if span_shift != 0 {
                assert_eq!(span_shift, 1);
                assert_eq!(before.block, entry);
                shifted += 1;
            }
            let after = pending
                .slot_relocation
                .as_ref()
                .unwrap()
                .assertion_span(before, budget)?;
            assert_eq!(after.block, before.block);
            assert_eq!(after.count, before.count);
            assert_eq!(after.first.checked_add(span_shift), Some(before.first));
            let mut rows = pending
                .coordinates
                .spans
                .rows
                .iter()
                .filter(|row| row.instance == child && row.source == source);
            let mapped = rows.next().unwrap();
            assert!(rows.next().is_none());
            assert_eq!(mapped.segments, [Some(after), None]);
            assert!(mapped.removed_call.is_none());
        }
        assert_eq!(shifted, if span_shift == 0 { 0 } else { 2 });
        assert_eq!(
            (
                storage.allocations,
                storage.payload_bytes,
                storage.alignment
            ),
            expected
        );
        let body = pending.function.body.as_ref().unwrap();
        let root = body.blocks[0].id;
        let mut pointers = BTreeSet::new();
        for block in &body.blocks {
            for operation in &block.operations {
                if matches!(operation.kind, OperationKind::Alloca { .. }) {
                    assert_eq!(block.id, root);
                    assert!(pointers.insert(operation.results[0].id));
                }
            }
        }
        assert_eq!(pointers.len(), expected.0);
        for slot in &receipt.slots {
            assert!(pointers.contains(&slot.origin.pointer));
        }
        consume(instances, &mut pending, children, &tiles, budget)?;
        drop(pending);
        Ok(())
    })?;
    assert_eq!(budget.storage(), floor);
    Err(unsupported(0, None, None, STOP))
}

fn inspect_pending_parts(
    instances: &ExecutionInstancesV29<'_>,
    pending: &mut PendingScopedRootEmissionV29,
    _children: [ProductionCallInstanceIdV1; 2],
    tiles: &[DeferredTileEventV29; 2],
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    let floor = budget.storage();
    with_canonical_call_scratch_v1(budget, |budget| {
        let view = ScopedDeferredScalarViewV29::for_pending_root(
            instances,
            pending,
            pending.slot_relocation.as_ref().unwrap(),
            budget,
        )?;
        assert_eq!(view.definitions, 8);
        let mut scratch = 0;
        let ordinary = call_splice_index_with_deferred_parts_v29(
            &pending.function,
            None,
            budget,
            &mut scratch,
        )
        .map_err(splice_view_error)?;
        assert_parts_lookups(&ordinary, tiles, [false, false], budget)
            .map_err(splice_view_error)?;
        assert!(matches!(
            call_splice_check_body_v1(&pending.function, &ordinary, false, budget),
            Err(CallInstanceEmissionErrorV1::MissingDefinition)
        ));
        let scoped = call_splice_index_with_deferred_parts_v29(
            &pending.function,
            Some(&view),
            budget,
            &mut scratch,
        )
        .map_err(splice_view_error)?;
        assert_parts_lookups(&scoped, tiles, [true, true], budget).map_err(splice_view_error)?;
        assert_eq!(scoped.values.len(), ordinary.values.len() + 8);
        assert_eq!(scoped.blocks, ordinary.blocks);
        call_splice_check_body_v1(&pending.function, &scoped, false, budget)
            .map_err(splice_view_error)?;
        let retained = (ordinary.blocks.len() + scoped.blocks.len())
            * std::mem::size_of::<BlockId>()
            + (ordinary.values.len() + scoped.values.len())
                * std::mem::size_of::<(ValueId, &Type)>();
        assert_eq!(scratch, retained);
        assert_eq!(budget.storage(), floor + scratch);
        drop((ordinary, scoped));
        budget.release_storage(scratch)?;
        assert_eq!(budget.storage(), floor);
        Ok(())
    })?;
    assert_eq!(budget.storage(), floor);
    Ok(())
}

fn pending_parts_observer(
    _source: &ExecutionLifecycleSourceV29<'_>,
    instances: &ExecutionInstancesV29<'_>,
    emitted: &mut [Option<LoweredFunctionResultV1>],
    receipt: &OwnedScopedSourceSlotsV29,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    with_pending_parts(instances, emitted, receipt, budget, inspect_pending_parts)
}

#[test]
fn pending_root_parts_survive_zero_and_real_child_allocation_relocation() {
    run_pending_parts_observer(pending_parts_observer);
}

fn consume_pending_parts_view(
    instances: &ExecutionInstancesV29<'_>,
    pending: &PendingScopedRootEmissionV29,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    let floor = budget.storage();
    let result = with_canonical_call_scratch_v1(budget, |budget| {
        let view = ScopedDeferredScalarViewV29::for_pending_root(
            instances,
            pending,
            pending.slot_relocation.as_ref().unwrap(),
            budget,
        )?;
        assert_eq!(view.definitions, 8);
        let mut scratch = 0;
        let index = call_splice_index_with_deferred_parts_v29(
            &pending.function,
            Some(&view),
            budget,
            &mut scratch,
        )
        .map_err(splice_view_error)?;
        let retained = index.blocks.len() * std::mem::size_of::<BlockId>()
            + index.values.len() * std::mem::size_of::<(ValueId, &Type)>();
        assert_eq!(scratch, retained);
        assert_eq!(budget.storage(), floor + scratch);
        let checked = call_splice_check_body_v1(&pending.function, &index, false, budget)
            .map(|_| ())
            .map_err(splice_view_error);
        drop(index);
        budget.release_storage(scratch)?;
        assert_eq!(budget.storage(), floor);
        checked
    });
    assert_eq!(budget.storage(), floor);
    result
}

fn pending_parts_boundaries(
    instances: &ExecutionInstancesV29<'_>,
    pending: &PendingScopedRootEmissionV29,
    budget: &mut ArgumentBudgetV1<'_>,
    short_work: bool,
) -> Result<(), ProductionSemanticKirErrorV1> {
    let original = pending.function.clone();
    let spans = pending.coordinates.spans.rows.clone();
    let events: Vec<_> = pending
        .sidecars
        .rows
        .iter()
        .map(|row| {
            (
                row.next_value,
                row.source_call_instance,
                row.lifecycle_events.as_ref().unwrap().rows.clone(),
            )
        })
        .collect();
    let ledger = budget.work_ledger_identity_v1();
    let floor = budget.storage();
    assert!(floor > 0);
    let available = budget.storage_limit() - floor;
    let filler = available.checked_sub(1_000_000).unwrap();
    let old_peak = budget.peak_storage();
    budget.reserve_storage(filler)?;
    let entry = budget.storage();
    assert!(entry > old_peak);
    let before = budget.work();
    consume_pending_parts_view(instances, pending, budget)?;
    let work = budget.work() - before;
    let peak = budget.peak_storage() - entry;
    assert!(work > 0 && peak > 0);
    assert_eq!(budget.storage(), entry);
    budget.release_storage(filler)?;
    for (allowance, success) in [(peak, true), (peak - 1, false)] {
        let filler = available - allowance;
        budget.reserve_storage(filler)?;
        let result = consume_pending_parts_view(instances, pending, budget);
        match (result, success) {
            (Ok(()), true) => {}
            (
                Err(ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                    ArgumentResourceV1::Storage(error),
                )),
                false,
            ) => {
                assert_eq!(error.limit(), budget.storage_limit());
                assert_eq!(error.actual(), budget.storage_limit() + 1);
                assert_eq!(budget.failed_storage(), Some(error.actual()));
            }
            (other, _) => panic!("unexpected pending storage boundary: {other:?}"),
        }
        assert_eq!(budget.storage(), floor + filler);
        budget.release_storage(filler)?;
    }
    let allowance = work - usize::from(short_work);
    let filler = INDEX_LIMIT
        .checked_sub(budget.work())
        .unwrap()
        .checked_sub(allowance)
        .unwrap();
    budget.charge_work(filler)?;
    let result = consume_pending_parts_view(instances, pending, budget);
    if short_work {
        match result {
            Err(ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                ArgumentResourceV1::Work(error),
            )) => {
                assert_eq!(error.limit(), INDEX_LIMIT);
                assert_eq!(error.actual(), INDEX_LIMIT + 1);
            }
            other => panic!("unexpected pending work boundary: {other:?}"),
        }
    } else {
        result?;
        assert_eq!(budget.work(), INDEX_LIMIT);
    }
    assert_eq!(budget.storage(), floor);
    assert!(budget.work_ledger_identity_v1() == ledger);
    assert_eq!(pending.function, original);
    assert_eq!(pending.coordinates.spans.rows, spans);
    for (row, saved) in pending.sidecars.rows.iter().zip(events) {
        assert_eq!(row.next_value, saved.0);
        assert_eq!(row.source_call_instance, saved.1);
        assert_eq!(row.lifecycle_events.as_ref().unwrap().rows, saved.2);
    }
    Ok(())
}

thread_local! {
    static PENDING_PARTS_SHORT_WORK: std::cell::Cell<bool> =
        const { std::cell::Cell::new(false) };
}

fn pending_parts_boundary_observer(
    _source: &ExecutionLifecycleSourceV29<'_>,
    instances: &ExecutionInstancesV29<'_>,
    emitted: &mut [Option<LoweredFunctionResultV1>],
    receipt: &OwnedScopedSourceSlotsV29,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    with_pending_parts(
        instances,
        emitted,
        receipt,
        budget,
        |instances, pending, _, _, budget| {
            pending_parts_boundaries(instances, pending, budget, PENDING_PARTS_SHORT_WORK.get())
        },
    )
}

#[test]
fn pending_root_parts_have_same_ledger_exact_and_one_short_quotas() {
    for short in [false, true] {
        PENDING_PARTS_SHORT_WORK.set(short);
        run_pending_parts_observer(pending_parts_boundary_observer);
    }
}

fn pending_view_attempt(
    instances: &ExecutionInstancesV29<'_>,
    pending: &PendingScopedRootEmissionV29,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    ScopedDeferredScalarViewV29::for_pending_root(
        instances,
        pending,
        pending.slot_relocation.as_ref().unwrap(),
        budget,
    )
    .map(|_| ())
}

fn pending_parts_refusals(
    instances: &ExecutionInstancesV29<'_>,
    pending: &mut PendingScopedRootEmissionV29,
    children: [ProductionCallInstanceIdV1; 2],
    tiles: &[DeferredTileEventV29; 2],
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    consume_pending_parts_view(instances, pending, budget)?;
    let floor = budget.storage();
    let original = pending.function.clone();
    let original_spans = pending.coordinates.spans.rows.clone();
    let original_controls = pending.coordinates.controls.rows.clone();
    let source_identity = pending.coordinates.semantic_sha256;
    let child = children[0].index();

    let seed = pending
        .coordinates
        .seeds
        .rows
        .iter()
        .position(|row| row.instance == children[0])
        .unwrap();
    let container = pending.coordinates.seeds.rows[seed].container;
    assert_ne!(container, children[0]);
    pending.coordinates.seeds.rows[seed].container = children[0];
    let result = pending_view_attempt(instances, pending, budget);
    pending.coordinates.seeds.rows[seed].container = container;
    assert_source_refusal(result);

    pending.coordinates.semantic_sha256[0] ^= 1;
    let result = pending_view_attempt(instances, pending, budget);
    pending.coordinates.semantic_sha256[0] ^= 1;
    assert!(
        matches!(
            result,
            Err(ProductionSemanticKirErrorV1::Unsupported {
                detail: "execution call parameters differ from their source instance",
                ..
            })
        ),
        "{result:?}"
    );

    pending.sidecars.rows[child]
        .lifecycle_events
        .as_mut()
        .unwrap()
        .source
        .semantic[0] ^= 1;
    let result = pending_view_attempt(instances, pending, budget);
    pending.sidecars.rows[child]
        .lifecycle_events
        .as_mut()
        .unwrap()
        .source
        .semantic[0] ^= 1;
    assert_source_refusal(result);
    let instance = std::mem::replace(
        &mut pending.sidecars.rows[child].source_call_instance,
        Some(children[1]),
    );
    let active = pending.active_instances.check_source_plan(
        instances,
        &pending.sidecars.rows,
        budget,
    );
    assert!(matches!(active, Err(ProductionSemanticKirErrorV1::Unsupported {
        detail: "execution call parameters differ from their source instance", ..
    })), "{active:?}");
    let result = pending_view_attempt(instances, pending, budget);
    pending.sidecars.rows[child].source_call_instance = instance;
    assert!(matches!(result, Err(ProductionSemanticKirErrorV1::Unsupported {
        detail: "execution call parameters differ from their source instance", ..
    })), "{result:?}");
    let events = pending.sidecars.rows[child].lifecycle_events.take();
    let result = pending_view_attempt(instances, pending, budget);
    pending.sidecars.rows[child].lifecycle_events = events;
    assert_source_refusal(result);
    let next = std::mem::replace(
        &mut pending.sidecars.rows[child].next_value,
        tiles[0].first_result.0,
    );
    let result = pending_view_attempt(instances, pending, budget);
    pending.sidecars.rows[child].next_value = next;
    assert_source_refusal(result);

    let mut foreign_work = CanonicalKernelIrWorkBudgetV1::new(INDEX_LIMIT);
    let mut foreign = ArgumentBudgetV1::new(&mut foreign_work, INDEX_LIMIT);
    foreign.reserve_storage(29)?;
    let before = foreign.work();
    assert_ledger_refusal(pending_view_attempt(instances, pending, &mut foreign));
    assert_eq!(foreign.work(), before);
    assert_eq!(foreign.storage(), 29);
    let ledger = pending.sidecars.rows[child]
        .lifecycle_events
        .as_ref()
        .unwrap()
        .ledger;
    pending.sidecars.rows[child]
        .lifecycle_events
        .as_mut()
        .unwrap()
        .ledger = foreign.work_ledger_identity_v1();
    let result = pending_view_attempt(instances, pending, budget);
    pending.sidecars.rows[child]
        .lifecycle_events
        .as_mut()
        .unwrap()
        .ledger = ledger;
    assert_ledger_refusal(result);

    with_canonical_call_scratch_v1(budget, |budget| {
        let view = ScopedDeferredScalarViewV29::for_pending_root(
            instances,
            pending,
            pending.slot_relocation.as_ref().unwrap(),
            budget,
        )?;
        let mut scratch = 0;
        let result =
            call_splice_index_with_deferred_parts_v29(&original, Some(&view), budget, &mut scratch);
        assert!(matches!(
            result,
            Err(CallInstanceEmissionErrorV1::ExecutionTransport)
        ));
        drop(result);
        budget.release_storage(scratch)?;
        assert_eq!(budget.storage(), floor);
        Ok(())
    })?;

    let indices = (*tiles).map(|tile| {
        let origin = pending.sidecars.rows[tile.producer.caller.index()]
            .terminator_operation_spans
            .iter()
            .find(|span| span.semantic_block == tile.producer.block)
            .copied()
            .unwrap();
        pending
            .coordinates
            .spans
            .rows
            .iter()
            .position(|row| {
                row.instance == tile.producer.caller
                    && row.source == InstanceSpanSourceV1::Terminator(origin)
            })
            .unwrap()
    });
    let target = indices[0];
    let sibling = indices[1];
    assert_ne!(target, sibling);
    let saved = pending.coordinates.spans.rows[target];
    let other = pending.coordinates.spans.rows[sibling];
    let InstanceSpanSourceV1::Terminator(origin) = saved.source else {
        unreachable!()
    };
    let event = *pending.sidecars.rows[child]
        .lifecycle_events
        .as_ref()
        .unwrap()
        .rows
        .iter()
        .find(|event| event.block == tiles[0].producer.block)
        .unwrap();
    let control = pending
        .coordinates
        .controls
        .rows
        .iter()
        .position(|row| {
            row.instance == children[0]
                && row.original_block == event.original_block
                && row.semantic_block == Some(event.block)
        })
        .unwrap();
    let saved_control = pending.coordinates.controls.rows[control].clone();
    let other_location = other.segments[0].unwrap();
    assert_ne!(saved.segments[0].unwrap().block, other_location.block);
    assert_eq!(saved.segments[0].unwrap().count, other_location.count);

    // A coherent sibling substitution passes the generic mapping check but
    // must not replace the original instance's exact relocation witness.
    pending.coordinates.spans.rows[target].segments = other.segments;
    pending.coordinates.controls.rows[control].physical_block = other_location.block;
    let mapped = check_mapped_parts_location_v29(
        DeferredPartsCoordinatesV29::owned(&pending.coordinates),
        children[0],
        &event,
        &origin,
        &pending.function,
        budget,
    )?;
    assert_eq!(mapped, other_location);
    let expected = pending
        .slot_relocation
        .as_ref()
        .unwrap()
        .assertion_span(saved.source.coordinates().2, budget)?;
    assert_ne!(mapped, expected);
    let result = pending_view_attempt(instances, pending, budget);
    pending.coordinates.spans.rows[target] = saved;
    pending.coordinates.controls.rows[control] = saved_control;
    assert_source_refusal(result);

    assert_eq!(pending.function, original);
    assert_eq!(pending.coordinates.spans.rows, original_spans);
    assert_eq!(pending.coordinates.controls.rows, original_controls);
    assert_eq!(pending.coordinates.semantic_sha256, source_identity);
    assert_eq!(budget.storage(), floor);
    consume_pending_parts_view(instances, pending, budget)?;
    assert_eq!(budget.storage(), floor);
    Ok(())
}

fn pending_parts_refusal_observer(
    _source: &ExecutionLifecycleSourceV29<'_>,
    instances: &ExecutionInstancesV29<'_>,
    emitted: &mut [Option<LoweredFunctionResultV1>],
    receipt: &OwnedScopedSourceSlotsV29,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    with_pending_parts(instances, emitted, receipt, budget, pending_parts_refusals)
}

#[test]
fn pending_root_parts_reject_foreign_custody_and_substituted_relocated_spans() {
    run_pending_parts_observer(pending_parts_refusal_observer);
}
