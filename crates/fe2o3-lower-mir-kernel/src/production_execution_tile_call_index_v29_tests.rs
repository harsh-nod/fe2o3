use super::*;

const INDEX_LIMIT: usize = 10_000_000;

fn run_index_observer(observer: ScopedSlotObserverV29) {
    let (result, _, _) = run(
        false,
        ScopedFixture::Plain,
        observer,
        INDEX_LIMIT,
        INDEX_LIMIT,
    );
    assert!(is_stopped(&result), "{result:?}");
    assert_eq!(OBSERVED.get(), 1);
}

fn attempt(
    instances: &ExecutionInstancesV29<'_>,
    instance: ProductionCallInstanceIdV1,
    lowered: &LoweredFunctionResultV1,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    let floor = budget.storage();
    let result = with_canonical_call_scratch_v1(budget, |budget| {
        scoped_call_index_with_deferred_parts_v29(instances, instance, lowered, budget).map(|_| ())
    });
    assert_eq!(budget.storage(), floor);
    result
}

fn assert_source_refusal(result: Result<(), ProductionSemanticKirErrorV1>) {
    assert!(
        matches!(
            result,
            Err(ProductionSemanticKirErrorV1::Unsupported {
                detail: "execution lifecycle differs from its retained source instance",
                ..
            })
        ),
        "{result:?}"
    );
}

fn assert_ledger_refusal(result: Result<(), ProductionSemanticKirErrorV1>) {
    assert!(
        matches!(
            result,
            Err(
                ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                    ArgumentResourceV1::Accounting
                )
            )
        ),
        "{result:?}"
    );
}

fn unchanged_index(
    source: &ExecutionLifecycleSourceV29<'_>,
    instances: &ExecutionInstancesV29<'_>,
    emitted: &mut [Option<LoweredFunctionResultV1>],
    receipt: &OwnedScopedSourceSlotsV29,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    for (ordinal, row) in emitted.iter().enumerate() {
        let lowered = row.as_ref().unwrap();
        let instance = instances.id_at(ordinal).unwrap();
        assert!(
            lowered
                .lifecycle_events
                .as_ref()
                .unwrap()
                .rows
                .iter()
                .all(|event| !matches!(event.kind, DeferredLifecycleKindV29::Tile(_)))
        );
        let floor = budget.storage();
        with_canonical_call_scratch_v1(budget, |budget| {
            let physical = CallFunctionIndexV1::new(&lowered.function, budget)?;
            let actual =
                scoped_call_index_with_deferred_parts_v29(instances, instance, lowered, budget)?;
            assert_eq!(actual.values, physical.values);
            assert_eq!(actual.blocks, physical.blocks);
            assert!(matches!(
                actual.ty(ValueId(lowered.next_value), budget),
                Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch)
            ));
            Ok(())
        })?;
        assert_eq!(budget.storage(), floor);
    }
    stop_after_check(source, instances, emitted, receipt, budget)
}

#[test]
fn ordinary_source_keeps_exact_physical_index_and_missing_lookup() {
    run_index_observer(unchanged_index);
}

#[derive(Clone, Copy, Debug)]
enum IndexFault {
    Source,
    Instance,
    SourceInstance,
    Placement,
    Census,
    AnchorSource,
    AnchorPlacement,
}

fn flip_fault(
    fault: IndexFault,
    lowered: &mut LoweredFunctionResultV1,
    root: ProductionCallInstanceIdV1,
    other: ProductionCallInstanceIdV1,
) {
    match fault {
        IndexFault::Source => {
            lowered.lifecycle_events.as_mut().unwrap().source.semantic[0] ^= 1;
        }
        IndexFault::Instance => {
            let value = &mut lowered.lifecycle_events.as_mut().unwrap().instance;
            *value = if *value == root { other } else { root };
        }
        IndexFault::SourceInstance => {
            lowered.source_call_instance = if lowered.source_call_instance == Some(root) {
                None
            } else {
                Some(root)
            };
        }
        IndexFault::Placement => {
            lowered
                .lifecycle_events
                .as_mut()
                .unwrap()
                .placement
                .first_value ^= 1;
        }
        IndexFault::Census => {
            lowered.lifecycle_events.as_mut().unwrap().expected_rows ^= 1;
        }
        IndexFault::AnchorSource => {
            lowered
                .scoped_memory_anchors
                .as_mut()
                .unwrap()
                .subject
                .source
                .semantic[0] ^= 1;
        }
        IndexFault::AnchorPlacement => {
            lowered
                .scoped_memory_anchors
                .as_mut()
                .unwrap()
                .placement
                .first_block ^= 1;
        }
    }
}

fn receipt_mutations(
    source: &ExecutionLifecycleSourceV29<'_>,
    instances: &ExecutionInstancesV29<'_>,
    emitted: &mut [Option<LoweredFunctionResultV1>],
    receipt: &OwnedScopedSourceSlotsV29,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    let root = instances.root();
    let other = instances.id_at(1).unwrap();
    assert_ne!(root, other);
    let lowered = emitted[root.index()].as_mut().unwrap();
    attempt(instances, root, lowered, budget)?;
    for fault in [
        IndexFault::Source,
        IndexFault::Instance,
        IndexFault::SourceInstance,
        IndexFault::Placement,
        IndexFault::Census,
        IndexFault::AnchorSource,
        IndexFault::AnchorPlacement,
    ] {
        flip_fault(fault, lowered, root, other);
        let result = attempt(instances, root, lowered, budget);
        flip_fault(fault, lowered, root, other);
        assert_source_refusal(result);
    }
    assert_source_refusal(attempt(instances, other, lowered, budget));
    let anchors = lowered.scoped_memory_anchors.take();
    let result = attempt(instances, root, lowered, budget);
    lowered.scoped_memory_anchors = anchors;
    assert_source_refusal(result);
    attempt(instances, root, lowered, budget)?;
    stop_after_check(source, instances, emitted, receipt, budget)
}

#[test]
fn ordinary_source_index_rejects_changed_receipts_and_missing_anchors() {
    run_index_observer(receipt_mutations);
}

fn ledger_and_missing_definition(
    source: &ExecutionLifecycleSourceV29<'_>,
    instances: &ExecutionInstancesV29<'_>,
    emitted: &mut [Option<LoweredFunctionResultV1>],
    receipt: &OwnedScopedSourceSlotsV29,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    let root = instances.root();
    let lowered = emitted[root.index()].as_mut().unwrap();
    let floor = budget.storage();
    assert!(floor > 0);
    let mut foreign_work = CanonicalKernelIrWorkBudgetV1::new(INDEX_LIMIT);
    let mut foreign = ArgumentBudgetV1::new(&mut foreign_work, INDEX_LIMIT);
    foreign.reserve_storage(29)?;
    assert_ledger_refusal(attempt(instances, root, lowered, &mut foreign));
    assert_eq!(foreign.storage(), 29);
    assert_eq!(budget.storage(), floor);
    let ledger = lowered
        .scoped_memory_anchors
        .as_ref()
        .unwrap()
        .subject
        .ledger;
    lowered
        .scoped_memory_anchors
        .as_mut()
        .unwrap()
        .subject
        .ledger = foreign.work_ledger_identity_v1();
    let result = attempt(instances, root, lowered, budget);
    lowered
        .scoped_memory_anchors
        .as_mut()
        .unwrap()
        .subject
        .ledger = ledger;
    assert_ledger_refusal(result);
    let missing = lowered.function.body.as_ref().unwrap().parameters[0];
    let replacement = ValueId(lowered.next_value);
    assert_ne!(missing, replacement);
    lowered.function.body.as_mut().unwrap().parameters[0] = replacement;
    let result = with_canonical_call_scratch_v1(budget, |budget| {
        let physical = CallFunctionIndexV1::new(&lowered.function, budget)?;
        let actual = scoped_call_index_with_deferred_parts_v29(instances, root, lowered, budget)?;
        assert_eq!(actual.values, physical.values);
        assert!(matches!(
            actual.ty(missing, budget),
            Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch)
        ));
        assert_eq!(
            actual.ty(replacement, budget)?,
            &Type::Scalar(ScalarType::U32)
        );
        Ok(())
    });
    lowered.function.body.as_mut().unwrap().parameters[0] = missing;
    assert_eq!(budget.storage(), floor);
    result?;
    attempt(instances, root, lowered, budget)?;
    stop_after_check(source, instances, emitted, receipt, budget)
}

#[test]
fn index_refuses_foreign_ledgers_without_reconstructing_missing_ordinary_values() {
    run_index_observer(ledger_and_missing_definition);
}

fn forged_parts(
    source: &ExecutionLifecycleSourceV29<'_>,
    instances: &ExecutionInstancesV29<'_>,
    emitted: &mut [Option<LoweredFunctionResultV1>],
    receipt: &OwnedScopedSourceSlotsV29,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    let root = instances.root();
    let other = instances.id_at(1).unwrap();
    let lowered = emitted[root.index()].as_mut().unwrap();
    let saved = lowered.lifecycle_events.as_ref().unwrap().rows[0];
    let DeferredLifecycleKindV29::Issue { result: identity } = saved.kind else {
        panic!("expected the actual root issuer");
    };
    let saved_next = lowered.next_value;
    let reserved_end = saved_next.checked_add(4).unwrap();
    let tile = DeferredTileEventV29 {
        input: DeferredTileInputV29::Parts { fragment: identity },
        producer: ProductionCallOccurrenceV1 {
            caller: root,
            block: saved.block,
        },
        result_type: identity.semantic_type,
        first_result: ValueId(saved_next),
        lanes: 64,
        elements: 2,
    };
    // Context cannot stand in for Fragment. This fixture does not qualify a
    // genuine Parts call's later source/callee checks.
    for (candidate, overflow) in [
        (tile, false),
        (
            DeferredTileEventV29 {
                producer: ProductionCallOccurrenceV1 {
                    caller: other,
                    block: saved.block,
                },
                ..tile
            },
            false,
        ),
        (
            DeferredTileEventV29 {
                producer: ProductionCallOccurrenceV1 {
                    caller: root,
                    block: SemanticBlockIdV1::from_index(u32::MAX),
                },
                ..tile
            },
            false,
        ),
        (
            DeferredTileEventV29 {
                first_result: ValueId(reserved_end),
                ..tile
            },
            false,
        ),
        (
            DeferredTileEventV29 {
                first_result: ValueId(u32::MAX),
                ..tile
            },
            true,
        ),
    ] {
        lowered.lifecycle_events.as_mut().unwrap().rows[0] = DeferredLifecycleEventV29 {
            source: DeferredLifecycleSourceV29::Intrinsic { callee: ISSUER },
            kind: DeferredLifecycleKindV29::Tile(candidate),
            ..saved
        };
        lowered.next_value = reserved_end;
        let result = attempt(instances, root, lowered, budget);
        lowered.lifecycle_events.as_mut().unwrap().rows[0] = saved;
        lowered.next_value = saved_next;
        if overflow {
            assert!(
                matches!(
                    result,
                    Err(
                        ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                            ArgumentResourceV1::Arithmetic
                        )
                    )
                ),
                "{result:?}"
            );
        } else {
            assert_source_refusal(result);
        }
    }
    attempt(instances, root, lowered, budget)?;
    stop_after_check(source, instances, emitted, receipt, budget)
}

#[test]
fn issuer_cannot_forge_parts_definitions_occurrences_or_result_ranges() {
    run_index_observer(forged_parts);
}

fn splice_view_error(error: CallInstanceEmissionErrorV1) -> ProductionSemanticKirErrorV1 {
    pending_scope_correspondence_error_v29(InstanceCorrespondenceErrorV1::Emission(error))
}

fn consume_empty_view(
    instances: &ExecutionInstancesV29<'_>,
    instance: ProductionCallInstanceIdV1,
    lowered: &LoweredFunctionResultV1,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    consume_view(instances, instance, lowered, 0, budget)
}

fn consume_view(
    instances: &ExecutionInstancesV29<'_>,
    instance: ProductionCallInstanceIdV1,
    lowered: &LoweredFunctionResultV1,
    expected: usize,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    let floor = budget.storage();
    let result = with_canonical_call_scratch_v1(budget, |budget| {
        let view = ScopedDeferredScalarViewV29::for_instance(instances, instance, lowered, budget)?;
        assert_eq!(view.definitions, expected);
        let mut scratch = 0;
        let index = call_splice_index_with_deferred_parts_v29(
            &lowered.function,
            Some(&view),
            budget,
            &mut scratch,
        )
        .map_err(splice_view_error)?;
        assert_eq!(budget.storage(), floor + scratch);
        drop(index);
        budget.release_storage(scratch)?;
        Ok(())
    });
    assert_eq!(budget.storage(), floor);
    result
}

fn ordinary_splice_views(
    source: &ExecutionLifecycleSourceV29<'_>,
    instances: &ExecutionInstancesV29<'_>,
    emitted: &mut [Option<LoweredFunctionResultV1>],
    receipt: &OwnedScopedSourceSlotsV29,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    for (ordinal, row) in emitted.iter().enumerate() {
        let lowered = row.as_ref().unwrap();
        let instance = instances.id_at(ordinal).unwrap();
        let foreign = lowered.function.clone();
        let floor = budget.storage();
        with_canonical_call_scratch_v1(budget, |budget| {
            let view =
                ScopedDeferredScalarViewV29::for_instance(instances, instance, lowered, budget)?;
            assert_eq!(view.definitions, 0);
            let mut scratch = 0;
            let ordinary = call_splice_index_with_deferred_parts_v29(
                &lowered.function,
                None,
                budget,
                &mut scratch,
            )
            .map_err(splice_view_error)?;
            let scoped = call_splice_index_with_deferred_parts_v29(
                &lowered.function,
                Some(&view),
                budget,
                &mut scratch,
            )
            .map_err(splice_view_error)?;
            assert_eq!(ordinary.values, scoped.values);
            assert_eq!(ordinary.blocks, scoped.blocks);
            for index in [&ordinary, &scoped] {
                assert!(matches!(
                    index.value(ValueId(lowered.next_value), budget),
                    Err(CallInstanceEmissionErrorV1::MissingDefinition)
                ));
            }
            assert!(matches!(
                call_splice_index_with_deferred_parts_v29(
                    &foreign,
                    Some(&view),
                    budget,
                    &mut scratch,
                ),
                Err(CallInstanceEmissionErrorV1::ExecutionTransport)
            ));
            assert_eq!(budget.storage(), floor + scratch);
            drop((ordinary, scoped));
            budget.release_storage(scratch)?;
            Ok(())
        })?;
        assert_eq!(budget.storage(), floor);
    }
    stop_after_check(source, instances, emitted, receipt, budget)
}

#[test]
fn source_bound_empty_views_preserve_ordinary_indices_and_reject_foreign_functions() {
    run_index_observer(ordinary_splice_views);
}

fn duplicate_physical_index(
    source: &ExecutionLifecycleSourceV29<'_>,
    instances: &ExecutionInstancesV29<'_>,
    emitted: &mut [Option<LoweredFunctionResultV1>],
    receipt: &OwnedScopedSourceSlotsV29,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    let instance = instances.root();
    let lowered = emitted[instance.index()].as_mut().unwrap();
    let ty = lowered.function.signature.parameters[0].clone();
    let body = lowered.function.body.as_mut().unwrap();
    let duplicate = body.parameters[0];
    let original_count = body.blocks[0].parameters.len();
    body.blocks[0].parameters.push(ValueDef::new(duplicate, ty));
    let floor = budget.storage();
    let result = with_canonical_call_scratch_v1(budget, |budget| {
        let view = ScopedDeferredScalarViewV29::for_instance(instances, instance, lowered, budget)?;
        assert_eq!(view.definitions, 0);
        for parts in [None, Some(&view)] {
            let mut scratch = 0;
            let result = call_splice_index_with_deferred_parts_v29(
                &lowered.function,
                parts,
                budget,
                &mut scratch,
            );
            assert!(matches!(
                result,
                Err(CallInstanceEmissionErrorV1::DuplicateIdentity)
            ));
            assert_eq!(budget.storage(), floor + scratch);
            budget.release_storage(scratch)?;
        }
        Ok(())
    });
    let parameters = &mut lowered.function.body.as_mut().unwrap().blocks[0].parameters;
    assert_eq!(parameters.pop().unwrap().id, duplicate);
    assert_eq!(parameters.len(), original_count);
    assert_eq!(budget.storage(), floor);
    result?;
    stop_after_check(source, instances, emitted, receipt, budget)
}

#[test]
fn source_bound_empty_views_do_not_suppress_physical_duplicate_errors() {
    run_index_observer(duplicate_physical_index);
}

fn source_and_ledger_views(
    source: &ExecutionLifecycleSourceV29<'_>,
    instances: &ExecutionInstancesV29<'_>,
    emitted: &mut [Option<LoweredFunctionResultV1>],
    receipt: &OwnedScopedSourceSlotsV29,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    let root = instances.root();
    let other = instances.id_at(1).unwrap();
    let lowered = emitted[root.index()].as_mut().unwrap();
    let floor = budget.storage();
    let original_function = lowered.function.clone();
    let original_next = lowered.next_value;
    for fault in [
        IndexFault::Source,
        IndexFault::Instance,
        IndexFault::SourceInstance,
        IndexFault::Placement,
        IndexFault::Census,
        IndexFault::AnchorSource,
        IndexFault::AnchorPlacement,
    ] {
        flip_fault(fault, lowered, root, other);
        let result =
            ScopedDeferredScalarViewV29::for_instance(instances, root, lowered, budget).map(|_| ());
        flip_fault(fault, lowered, root, other);
        assert_source_refusal(result);
        assert_eq!(budget.storage(), floor);
    }
    assert_source_refusal(
        ScopedDeferredScalarViewV29::for_instance(instances, other, lowered, budget).map(|_| ()),
    );
    let anchors = lowered.scoped_memory_anchors.take();
    let result =
        ScopedDeferredScalarViewV29::for_instance(instances, root, lowered, budget).map(|_| ());
    lowered.scoped_memory_anchors = anchors;
    assert_source_refusal(result);
    let mut foreign_work = CanonicalKernelIrWorkBudgetV1::new(INDEX_LIMIT);
    let mut foreign = ArgumentBudgetV1::new(&mut foreign_work, INDEX_LIMIT);
    foreign.reserve_storage(29)?;
    assert_ledger_refusal(
        ScopedDeferredScalarViewV29::for_instance(instances, root, lowered, &mut foreign)
            .map(|_| ()),
    );
    let ledger = lowered
        .scoped_memory_anchors
        .as_ref()
        .unwrap()
        .subject
        .ledger;
    lowered
        .scoped_memory_anchors
        .as_mut()
        .unwrap()
        .subject
        .ledger = foreign.work_ledger_identity_v1();
    let result =
        ScopedDeferredScalarViewV29::for_instance(instances, root, lowered, budget).map(|_| ());
    lowered
        .scoped_memory_anchors
        .as_mut()
        .unwrap()
        .subject
        .ledger = ledger;
    assert_ledger_refusal(result);
    {
        let view = ScopedDeferredScalarViewV29::for_instance(instances, root, lowered, budget)?;
        let mut scratch = 0;
        let result = call_splice_index_with_deferred_parts_v29(
            &lowered.function,
            Some(&view),
            &mut foreign,
            &mut scratch,
        );
        assert!(matches!(
            result,
            Err(CallInstanceEmissionErrorV1::Resource(
                ArgumentResourceV1::Accounting
            ))
        ));
        foreign.release_storage(scratch)?;
    }
    assert_eq!(foreign.storage(), 29);
    assert_eq!(budget.storage(), floor);
    assert_eq!(lowered.function, original_function);
    assert_eq!(lowered.next_value, original_next);
    consume_empty_view(instances, root, lowered, budget)?;
    stop_after_check(source, instances, emitted, receipt, budget)
}

#[test]
fn source_bound_splice_views_reject_receipt_and_ledger_mutations() {
    run_index_observer(source_and_ledger_views);
}

fn view_boundaries(
    instances: &ExecutionInstancesV29<'_>,
    emitted: &mut [Option<LoweredFunctionResultV1>],
    receipt: &OwnedScopedSourceSlotsV29,
    budget: &mut ArgumentBudgetV1<'_>,
    short_work: bool,
    instance: ProductionCallInstanceIdV1,
    expected: usize,
) -> Result<(), ProductionSemanticKirErrorV1> {
    check_receipt(instances, emitted, receipt, budget);
    let lowered = emitted[instance.index()].as_ref().unwrap();
    let original = lowered.function.clone();
    let original_next = lowered.next_value;
    let original_events = lowered.lifecycle_events.as_ref().unwrap().rows.clone();
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
    consume_view(instances, instance, lowered, expected, budget)?;
    let used_work = budget.work() - before;
    let peak = budget.peak_storage() - entry;
    assert!(used_work > 0 && peak > 0);
    assert_eq!(budget.storage(), entry);
    budget.release_storage(filler)?;
    for (allowance, success) in [(peak, true), (peak - 1, false)] {
        let filler = available - allowance;
        budget.reserve_storage(filler)?;
        let result = consume_view(instances, instance, lowered, expected, budget);
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
            (other, _) => panic!("unexpected storage boundary result: {other:?}"),
        }
        assert_eq!(budget.storage(), floor + filler);
        budget.release_storage(filler)?;
    }
    let allowance = used_work - usize::from(short_work);
    let filler_work = INDEX_LIMIT
        .checked_sub(budget.work())
        .unwrap()
        .checked_sub(allowance)
        .unwrap();
    budget.charge_work(filler_work)?;
    let result = consume_view(instances, instance, lowered, expected, budget);
    if short_work {
        match result {
            Err(ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                ArgumentResourceV1::Work(error),
            )) => {
                assert_eq!(error.limit(), INDEX_LIMIT);
                assert_eq!(error.actual(), INDEX_LIMIT + 1);
            }
            other => panic!("unexpected work boundary result: {other:?}"),
        }
    } else {
        result?;
        assert_eq!(budget.work(), INDEX_LIMIT);
    }
    assert_eq!(budget.storage(), floor);
    assert!(budget.work_ledger_identity_v1() == ledger);
    assert_eq!(lowered.function, original);
    assert_eq!(lowered.next_value, original_next);
    assert_eq!(
        lowered.lifecycle_events.as_ref().unwrap().rows,
        original_events
    );
    assert!(emitted.iter().all(Option::is_some));
    Err(unsupported(0, None, None, STOP))
}

fn exact_empty_view_boundary(
    _source: &ExecutionLifecycleSourceV29<'_>,
    instances: &ExecutionInstancesV29<'_>,
    emitted: &mut [Option<LoweredFunctionResultV1>],
    receipt: &OwnedScopedSourceSlotsV29,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    view_boundaries(
        instances,
        emitted,
        receipt,
        budget,
        false,
        instances.root(),
        0,
    )
}

fn short_empty_view_boundary(
    _source: &ExecutionLifecycleSourceV29<'_>,
    instances: &ExecutionInstancesV29<'_>,
    emitted: &mut [Option<LoweredFunctionResultV1>],
    receipt: &OwnedScopedSourceSlotsV29,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    view_boundaries(
        instances,
        emitted,
        receipt,
        budget,
        true,
        instances.root(),
        0,
    )
}

#[test]
fn source_bound_empty_views_have_exact_work_and_storage_boundaries() {
    run_index_observer(exact_empty_view_boundary);
    run_index_observer(short_empty_view_boundary);
}

fn run_tile_index_observer(observer: ScopedSlotObserverV29) {
    let (result, _, _) = run(
        false,
        ScopedFixture::TilePartsRepeated,
        observer,
        INDEX_LIMIT,
        INDEX_LIMIT,
    );
    assert!(is_stopped(&result), "{result:?}");
    assert_eq!(OBSERVED.get(), 1);
}

fn retained_parts_tile(row: &LoweredFunctionResultV1) -> Option<DeferredTileEventV29> {
    let mut parts = row
        .lifecycle_events
        .as_ref()
        .unwrap()
        .rows
        .iter()
        .filter_map(|event| match event.kind {
            DeferredLifecycleKindV29::Tile(tile)
                if matches!(tile.input, DeferredTileInputV29::Parts { .. }) =>
            {
                Some(tile)
            }
            _ => None,
        });
    let result = parts.next();
    assert!(parts.next().is_none());
    result
}

fn tile_children(
    instances: &ExecutionInstancesV29<'_>,
    emitted: &[Option<LoweredFunctionResultV1>],
) -> [ProductionCallInstanceIdV1; 2] {
    let children: Vec<_> = emitted
        .iter()
        .enumerate()
        .filter_map(|(index, row)| {
            retained_parts_tile(row.as_ref().unwrap()).map(|_| instances.id_at(index).unwrap())
        })
        .collect();
    let children: [_; 2] = children.try_into().unwrap();
    assert_ne!(children[0], children[1]);
    assert_eq!(
        instances.instance(children[0]).unwrap().function(),
        instances.instance(children[1]).unwrap().function()
    );
    let first = instances.incoming(children[0]).unwrap().occurrence();
    let second = instances.incoming(children[1]).unwrap().occurrence();
    assert_eq!(first.caller, second.caller);
    assert_ne!(first.block, second.block);
    let a = emitted[children[0].index()].as_ref().unwrap();
    let b = emitted[children[1].index()].as_ref().unwrap();
    assert_ne!(
        a.lifecycle_events.as_ref().unwrap().placement,
        b.lifecycle_events.as_ref().unwrap().placement
    );
    let a = retained_parts_tile(a).unwrap();
    let b = retained_parts_tile(b).unwrap();
    assert_eq!((a.lanes, a.elements), (64, 2));
    assert_eq!((b.lanes, b.elements), (64, 2));
    assert!(
        a.result_range()
            .unwrap()
            .all(|value| !b.result_range().unwrap().contains(&value))
    );
    children
}

const EXPECTED_PARTS_TYPES: [Type; 4] = [
    Type::Scalar(ScalarType::U32),
    Type::Scalar(ScalarType::U32),
    Type::Scalar(ScalarType::Bool),
    Type::Scalar(ScalarType::Bool),
];

fn assert_parts_lookups(
    index: &CallSpliceIndexV1<'_>,
    tiles: &[DeferredTileEventV29; 2],
    present: [bool; 2],
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), CallInstanceEmissionErrorV1> {
    for (tile, present) in tiles.iter().zip(present) {
        for (offset, value) in tile.result_range().unwrap().enumerate() {
            let result = index.value(ValueId(value), budget);
            if present {
                assert_eq!(result?, &EXPECTED_PARTS_TYPES[offset]);
            } else {
                assert!(
                    matches!(result, Err(CallInstanceEmissionErrorV1::MissingDefinition)),
                    "{result:?}"
                );
            }
        }
    }
    Ok(())
}

fn nonempty_instance_indices(
    source: &ExecutionLifecycleSourceV29<'_>,
    instances: &ExecutionInstancesV29<'_>,
    emitted: &mut [Option<LoweredFunctionResultV1>],
    receipt: &OwnedScopedSourceSlotsV29,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    let children = tile_children(instances, emitted);
    let tiles =
        children.map(|id| retained_parts_tile(emitted[id.index()].as_ref().unwrap()).unwrap());
    for (ordinal, child) in children.into_iter().enumerate() {
        let lowered = emitted[child.index()].as_ref().unwrap();
        let floor = budget.storage();
        with_canonical_call_scratch_v1(budget, |budget| {
            let view =
                ScopedDeferredScalarViewV29::for_instance(instances, child, lowered, budget)?;
            assert_eq!(view.definitions, 4);
            let mut scratch = 0;
            let ordinary = call_splice_index_with_deferred_parts_v29(
                &lowered.function,
                None,
                budget,
                &mut scratch,
            )
            .map_err(splice_view_error)?;
            assert_parts_lookups(&ordinary, &tiles, [false, false], budget)
                .map_err(splice_view_error)?;
            let scoped = call_splice_index_with_deferred_parts_v29(
                &lowered.function,
                Some(&view),
                budget,
                &mut scratch,
            )
            .map_err(splice_view_error)?;
            let mut present = [false, false];
            present[ordinal] = true;
            assert_parts_lookups(&scoped, &tiles, present, budget).map_err(splice_view_error)?;
            assert_eq!(scoped.values.len(), ordinary.values.len() + 4);
            assert_eq!(scoped.blocks, ordinary.blocks);
            let retained = (ordinary.blocks.len() + scoped.blocks.len())
                * std::mem::size_of::<BlockId>()
                + (ordinary.values.len() + scoped.values.len())
                    * std::mem::size_of::<(ValueId, &Type)>();
            assert_eq!(scratch, retained);
            assert_eq!(budget.storage(), floor + scratch);
            drop((ordinary, scoped));
            budget.release_storage(scratch)?;
            assert_eq!(budget.storage(), floor);
            let physical = CallFunctionIndexV1::new(&lowered.function, budget)?;
            let scoped =
                scoped_call_index_with_deferred_parts_v29(instances, child, lowered, budget)?;
            assert_eq!(scoped.values.len(), physical.values.len() + 4);
            for (offset, value) in tiles[ordinal].result_range()?.enumerate() {
                assert!(matches!(
                    physical.ty(ValueId(value), budget),
                    Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch)
                ));
                assert_eq!(
                    scoped.ty(ValueId(value), budget)?,
                    &EXPECTED_PARTS_TYPES[offset]
                );
            }
            let retained = (physical.values.capacity() + scoped.values.capacity())
                * std::mem::size_of::<(ValueId, &Type)>()
                + (physical.blocks.capacity() + scoped.blocks.capacity())
                    * std::mem::size_of::<&BasicBlock>();
            assert_eq!(budget.storage(), floor + retained);
            Ok(())
        })?;
        assert_eq!(budget.storage(), floor);
    }
    stop_after_check(source, instances, emitted, receipt, budget)
}

#[test]
fn repeated_source_parts_keep_distinct_typed_instance_definitions() {
    run_tile_index_observer(nonempty_instance_indices);
}

fn nonempty_view_refusals(
    source: &ExecutionLifecycleSourceV29<'_>,
    instances: &ExecutionInstancesV29<'_>,
    emitted: &mut [Option<LoweredFunctionResultV1>],
    receipt: &OwnedScopedSourceSlotsV29,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    let [child, sibling] = tile_children(instances, emitted);
    let lowered = emitted[child.index()].as_mut().unwrap();
    let original = lowered.function.clone();
    let original_next = lowered.next_value;
    let original_events = lowered.lifecycle_events.as_ref().unwrap().rows.clone();
    let floor = budget.storage();
    for fault in [
        IndexFault::Source,
        IndexFault::Instance,
        IndexFault::SourceInstance,
        IndexFault::Placement,
        IndexFault::Census,
        IndexFault::AnchorSource,
        IndexFault::AnchorPlacement,
    ] {
        flip_fault(fault, lowered, child, sibling);
        let result = ScopedDeferredScalarViewV29::for_instance(instances, child, lowered, budget)
            .map(|_| ());
        flip_fault(fault, lowered, child, sibling);
        assert_source_refusal(result);
        assert_eq!(budget.storage(), floor);
    }
    assert_source_refusal(
        ScopedDeferredScalarViewV29::for_instance(instances, sibling, lowered, budget).map(|_| ()),
    );
    let event_index = original_events
        .iter()
        .position(|event| {
            matches!(event.kind,
        DeferredLifecycleKindV29::Tile(tile)
        if matches!(tile.input, DeferredTileInputV29::Parts { .. }))
        })
        .unwrap();
    let saved = original_events[event_index];
    let DeferredLifecycleKindV29::Tile(mut tile) = saved.kind else {
        unreachable!()
    };
    tile.producer.caller = sibling;
    lowered.lifecycle_events.as_mut().unwrap().rows[event_index].kind =
        DeferredLifecycleKindV29::Tile(tile);
    let result =
        ScopedDeferredScalarViewV29::for_instance(instances, child, lowered, budget).map(|_| ());
    lowered.lifecycle_events.as_mut().unwrap().rows[event_index] = saved;
    assert_source_refusal(result);
    let mut foreign_work = CanonicalKernelIrWorkBudgetV1::new(INDEX_LIMIT);
    let mut foreign = ArgumentBudgetV1::new(&mut foreign_work, INDEX_LIMIT);
    foreign.reserve_storage(29)?;
    assert_ledger_refusal(
        ScopedDeferredScalarViewV29::for_instance(instances, child, lowered, &mut foreign)
            .map(|_| ()),
    );
    assert_eq!(foreign.storage(), 29);
    let duplicate = retained_parts_tile(lowered).unwrap().first_result;
    let parameters = &mut lowered.function.body.as_mut().unwrap().blocks[0].parameters;
    let count = parameters.len();
    parameters.push(ValueDef::new(duplicate, Type::Scalar(ScalarType::U32)));
    let result = with_canonical_call_scratch_v1(budget, |budget| {
        let view = ScopedDeferredScalarViewV29::for_instance(instances, child, lowered, budget)?;
        assert_eq!(view.definitions, 4);
        let mut scratch = 0;
        assert!(matches!(
            call_splice_index_with_deferred_parts_v29(
                &lowered.function,
                Some(&view),
                budget,
                &mut scratch
            ),
            Err(CallInstanceEmissionErrorV1::DuplicateIdentity)
        ));
        budget.release_storage(scratch)?;
        assert!(matches!(
            scoped_call_index_with_deferred_parts_v29(instances, child, lowered, budget),
            Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch)
        ));
        Ok(())
    });
    let parameters = &mut lowered.function.body.as_mut().unwrap().blocks[0].parameters;
    assert_eq!(parameters.pop().unwrap().id, duplicate);
    assert_eq!(parameters.len(), count);
    result?;
    assert_eq!(budget.storage(), floor);
    assert_eq!(lowered.function, original);
    assert_eq!(lowered.next_value, original_next);
    assert_eq!(
        lowered.lifecycle_events.as_ref().unwrap().rows,
        original_events
    );
    stop_after_check(source, instances, emitted, receipt, budget)
}

#[test]
fn genuine_parts_views_reject_sibling_receipts_foreign_ledgers_and_collisions() {
    run_tile_index_observer(nonempty_view_refusals);
}

mod live_parts_tests {
    include!("production_execution_tile_parts_index_v29_tests.rs");
}
