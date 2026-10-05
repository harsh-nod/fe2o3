thread_local! {
    static PAYLOAD_BASE_USE_ROWS_V29: std::cell::RefCell<Vec<(SourceObjectOccurrenceKeyV29, usize)>> = const { std::cell::RefCell::new(Vec::new()) };
    static PAYLOAD_BASE_USE_EVENTS_V29: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    static PAYLOAD_BASE_USE_OBSERVATIONS_V29: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    static PAYLOAD_BASE_USE_FAULT_V29: std::cell::Cell<u8> = const { std::cell::Cell::new(0) };
    static PAYLOAD_BASE_USE_MUTATED_V29: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    static PAYLOAD_BASE_USE_EXPECTED_BYTES_V29: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    static PAYLOAD_READ_ROWS_V29: std::cell::RefCell<Vec<(SourceObjectReadKeyV29, usize)>> = const { std::cell::RefCell::new(Vec::new()) };
    static PAYLOAD_READ_OBJECTS_V29: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    static PAYLOAD_READ_OLD_CAPACITY_V29: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    static PAYLOAD_READ_EXPECTED_BYTES_V29: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

fn observe_payload_base_use_roster_v29(
    instances: &ExecutionInstancesV29<'_>,
    emitted: &[Option<LoweredFunctionResultV1>],
    slots: &mut OwnedScopedSourceSlotsV29,
    references: Option<&SourceReferenceEmissionV29<'_, '_>>,
    identity: Option<&ExecutionIdentityPlanV1<'_, '_>>,
    floor: usize,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    // This existing fixture observer independently proves which actual
    // invocations participate, including the absence of static-holder outsiders.
    observe_aggregate_project_scale_roster_v29(
        instances, emitted, slots, references, identity, floor, budget,
    )?;
    let mut rows = Vec::new();
    let mut read_rows = Vec::new();
    let mut object_rows = 0;
    let mut old_read_capacity = 0;
    let mut events = 0;
    for lowered in emitted.iter().flatten() {
        if lowered
            .scoped_memory_anchors
            .as_ref()
            .unwrap()
            .objects
            .is_empty()
        {
            continue;
        }
        let instance = lowered.source_call_instance.unwrap();
        let original = instances.occurrences(instance).unwrap();
        events += original.events().len();
        for (position, row) in original.events().iter().enumerate() {
            if row.role() == ExecutionEventV29::BaseUse {
                rows.push((
                    (
                        instance.index(),
                        unit_local_source_key_v1(row.site(), row.operand(), Some(row.role())),
                        row.event().variable().get(),
                    ),
                    position,
                ));
            }
        }
        let anchors = lowered.scoped_memory_anchors.as_ref().unwrap();
        for (anchor, row) in anchors.rows.iter().enumerate() {
            let read = match row.kind {
                ScopedMemoryAnchorKindV29::Access {
                    payload: Some(ScopedMemoryPayloadV29::Load { result, read }),
                    ..
                } => {
                    old_read_capacity += 1;
                    Some((result, read))
                }
                ScopedMemoryAnchorKindV29::Object(index) => {
                    object_rows += 1;
                    old_read_capacity += 1;
                    let payload = &anchors.objects[index];
                    let operation = &lowered
                        .function
                        .body
                        .as_ref()
                        .unwrap()
                        .blocks
                        .iter()
                        .find(|block| block.id == row.block)
                        .unwrap()
                        .operations[row.position];
                    assert!(
                        matches!(&operation.kind, OperationKind::Storage(actual) if *actual == payload.operation)
                    );
                    match payload.role {
                        ScopedObjectRoleV29::ReadValue {
                            read: ScopedObjectReadOriginV29::Original(read),
                            ..
                        } => {
                            assert!(matches!(
                                operation.kind,
                                OperationKind::Storage(ScopedObjectOperationV29::ReadValue { .. })
                            ));
                            Some((payload.result.unwrap(), read))
                        }
                        _ => None,
                    }
                }
                _ => None,
            };
            if let Some((value, read)) = read {
                let operation = &lowered
                    .function
                    .body
                    .as_ref()
                    .unwrap()
                    .blocks
                    .iter()
                    .find(|block| block.id == row.block)
                    .unwrap()
                    .operations[row.position];
                assert!(matches!(operation.results.as_slice(), [result] if result.id == value));
                let event = match read.occurrence {
                    ScopedMemoryOccurrenceV29::Promoted { event, .. }
                    | ScopedMemoryOccurrenceV29::Retained { event } => event,
                };
                read_rows.push((
                    (
                        instance.index(),
                        unit_local_source_key_v1(
                            read.site,
                            read.role,
                            Some(ExecutionEventV29::BaseUse),
                        ),
                        read.prefix,
                        event,
                    ),
                    anchor,
                ));
            }
        }
    }
    rows.sort_unstable();
    assert!(rows.len() > 1 && rows.len() < events);
    assert!(rows.windows(2).all(|pair| pair[0].0 < pair[1].0));
    PAYLOAD_BASE_USE_EVENTS_V29.set(events);
    PAYLOAD_BASE_USE_ROWS_V29.with_borrow_mut(|expected| *expected = rows);
    read_rows.sort_unstable();
    assert!(!read_rows.is_empty() && read_rows.len() < old_read_capacity);
    assert!(read_rows.windows(2).all(|pair| pair[0].0 < pair[1].0));
    PAYLOAD_READ_ROWS_V29.with_borrow_mut(|expected| *expected = read_rows);
    PAYLOAD_READ_OBJECTS_V29.set(object_rows);
    PAYLOAD_READ_OLD_CAPACITY_V29.set(old_read_capacity);
    Ok(())
}

fn observe_payload_base_use_index_v29(
    index: &mut SourceObjectPayloadIndexV29,
    _: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    PAYLOAD_BASE_USE_ROWS_V29.with_borrow(|expected| {
        assert_eq!(&index.occurrences, expected);
        let mut allocation = Vec::<(SourceObjectOccurrenceKeyV29, usize)>::new();
        allocation.try_reserve_exact(expected.len()).unwrap();
        assert_eq!(index.occurrences.capacity(), allocation.capacity());
        assert!(index.occurrences.capacity() < PAYLOAD_BASE_USE_EVENTS_V29.get());
        type IndependentRow = ((usize, [u64; 7], u32), usize);
        PAYLOAD_BASE_USE_EXPECTED_BYTES_V29.set(
            PAYLOAD_BASE_USE_EXPECTED_BYTES_V29.get()
                + allocation.capacity() * std::mem::size_of::<IndependentRow>(),
        );
    });
    PAYLOAD_BASE_USE_OBSERVATIONS_V29.set(PAYLOAD_BASE_USE_OBSERVATIONS_V29.get() + 1);
    PAYLOAD_READ_ROWS_V29.with_borrow(|expected| {
        assert_eq!(&index.reads, expected);
        type IndependentReadRow = ((usize, [u64; 7], u32, usize), usize);
        let mut allocation = Vec::<IndependentReadRow>::new();
        allocation.try_reserve_exact(expected.len()).unwrap();
        assert_eq!(index.reads.capacity(), allocation.capacity());
        assert!(index.reads.capacity() < PAYLOAD_READ_OLD_CAPACITY_V29.get());
        PAYLOAD_READ_EXPECTED_BYTES_V29.set(
            PAYLOAD_READ_EXPECTED_BYTES_V29.get()
                + allocation.capacity() * std::mem::size_of::<IndependentReadRow>(),
        );
    });
    match PAYLOAD_BASE_USE_FAULT_V29.get() {
        0 => {}
        1 => {
            index.occurrences.pop().unwrap();
            PAYLOAD_BASE_USE_MUTATED_V29.set(true);
        }
        2 => {
            index.occurrences[1] = index.occurrences[0];
            PAYLOAD_BASE_USE_MUTATED_V29.set(true);
        }
        3 => {
            index.reads.pop().unwrap();
            PAYLOAD_BASE_USE_MUTATED_V29.set(true);
        }
        4 => {
            index.reads[1] = index.reads[0];
            PAYLOAD_BASE_USE_MUTATED_V29.set(true);
        }
        _ => unreachable!(),
    }
    Ok(())
}

fn run_base_use_index_v29(
    count: usize,
    fault: u8,
    work: usize,
    storage: usize,
) -> (SourceOwnedResultV18<()>, usize, usize, bool) {
    struct Restore(
        usize,
        Option<ScopedSlotCustodyObserverV29>,
        Option<SourceObjectPayloadIndexObserverV29>,
        u8,
    );
    impl Drop for Restore {
        fn drop(&mut self) {
            AGGREGATE_OBJECT_READ_COUNT_V29.set(self.0);
            SCOPED_SLOT_CUSTODY_OBSERVER_V29.set(self.1);
            SOURCE_OBJECT_PAYLOAD_INDEX_OBSERVER_V29.set(self.2);
            PAYLOAD_BASE_USE_FAULT_V29.set(self.3);
            PAYLOAD_BASE_USE_ROWS_V29.with_borrow_mut(Vec::clear);
            PAYLOAD_READ_ROWS_V29.with_borrow_mut(Vec::clear);
        }
    }
    let _restore = Restore(
        AGGREGATE_OBJECT_READ_COUNT_V29.replace(count),
        SCOPED_SLOT_CUSTODY_OBSERVER_V29.replace(Some(observe_payload_base_use_roster_v29)),
        SOURCE_OBJECT_PAYLOAD_INDEX_OBSERVER_V29.replace(Some(observe_payload_base_use_index_v29)),
        PAYLOAD_BASE_USE_FAULT_V29.replace(fault),
    );
    AGGREGATE_OBJECT_SCALE_ROSTER_V29.set(None);
    PAYLOAD_BASE_USE_OBSERVATIONS_V29.set(0);
    PAYLOAD_BASE_USE_MUTATED_V29.set(false);
    PAYLOAD_BASE_USE_EXPECTED_BYTES_V29.set(0);
    PAYLOAD_READ_EXPECTED_BYTES_V29.set(0);
    SOURCE_OBJECT_READ_INDEX_STORAGE_V29.set((0, 0));
    SOURCE_OBJECT_READ_CENSUS_WORK_V29.set((0, 0));
    SOURCE_OBJECT_READ_FIRST_CLASSIFICATION_V29.set(None);
    SOURCE_OBJECT_BASE_USE_CENSUS_WORK_V29.set((0, 0));
    SOURCE_OBJECT_BASE_USE_FIRST_SCAN_V29.set(None);
    SOURCE_OBJECT_BASE_USE_STORAGE_V29.set((0, 0));
    SOURCE_OBJECT_PAYLOAD_SCAN_COUNTS_V29.set((0, 0, 0));
    run_original_source_fixture_v29(
        aggregate_reborrow_original_owner_v29,
        false,
        true,
        3,
        |_, _, _, _, _| Ok(()),
        work,
        storage,
    )
}

#[test]
fn payload_occurrence_capacity_matches_actual_base_use_rows_and_positions() {
    for count in [1, 4, 16] {
        let result = run_base_use_index_v29(count, 0, LIMIT, LIMIT);
        assert!(
            result.0.is_ok() && result.3,
            "count={count}: {:?}",
            result.0
        );
        assert!(PAYLOAD_BASE_USE_OBSERVATIONS_V29.get() >= 3);
        assert!(!PAYLOAD_BASE_USE_MUTATED_V29.get());
        let calls = PAYLOAD_BASE_USE_OBSERVATIONS_V29.get();
        let events = PAYLOAD_BASE_USE_EVENTS_V29.get();
        assert_eq!(
            SOURCE_OBJECT_BASE_USE_CENSUS_WORK_V29.get(),
            (calls * events, calls)
        );
        assert_eq!(
            SOURCE_OBJECT_PAYLOAD_SCAN_COUNTS_V29.get().2,
            2 * calls * events
        );
        assert_eq!(
            SOURCE_OBJECT_BASE_USE_STORAGE_V29.get(),
            (calls, PAYLOAD_BASE_USE_EXPECTED_BYTES_V29.get())
        );
    }
}

#[test]
fn payload_base_use_census_refuses_unpaid_scan_before_visiting_events() {
    let positive = run_base_use_index_v29(4, 0, LIMIT, LIMIT);
    assert!(positive.0.is_ok() && positive.3, "{:?}", positive.0);
    let (before, events) = SOURCE_OBJECT_BASE_USE_FIRST_SCAN_V29.get().unwrap();
    assert!(events > 1);
    let limit = before + events - 1;
    let refused = run_base_use_index_v29(4, 0, limit, LIMIT);
    assert!(!refused.3);
    assert_eq!(
        SOURCE_OBJECT_BASE_USE_FIRST_SCAN_V29.get(),
        Some((before, events))
    );
    assert_eq!(SOURCE_OBJECT_BASE_USE_CENSUS_WORK_V29.get(), (0, 0));
    assert_eq!(SOURCE_OBJECT_PAYLOAD_SCAN_COUNTS_V29.get(), (0, 0, 0));
    assert_eq!(SOURCE_OBJECT_BASE_USE_STORAGE_V29.get(), (0, 0));
    assert_eq!(PAYLOAD_BASE_USE_OBSERVATIONS_V29.get(), 0);
    assert_eq!(refused.1, before);
    assert!(
        matches!(original_repeated_source_resource_v29(refused.0.unwrap_err()),
        ArgumentResourceV1::Work(error) if error.actual() == before + events && error.limit() == limit)
    );
}

#[test]
fn payload_base_use_census_keeps_exact_and_one_short_transaction_resources() {
    let measured = run_base_use_index_v29(4, 0, LIMIT, LIMIT);
    assert!(measured.0.is_ok() && measured.3, "{:?}", measured.0);
    let (work, peak) = (measured.1, measured.2);
    let exact = run_base_use_index_v29(4, 0, work, peak);
    assert!(exact.0.is_ok() && exact.3, "{:?}", exact.0);
    assert_eq!((exact.1, exact.2), (work, peak));
    let short = run_base_use_index_v29(4, 0, work - 1, peak);
    assert!(!short.3);
    assert!(
        matches!(original_repeated_source_resource_v29(short.0.unwrap_err()),
        ArgumentResourceV1::Work(error) if error.actual() == work && error.limit() == work - 1)
    );
    let short = run_base_use_index_v29(4, 0, work, peak - 1);
    assert!(!short.3);
    assert!(
        matches!(original_repeated_source_resource_v29(short.0.unwrap_err()),
        ArgumentResourceV1::Storage(error) if error.actual() == peak && error.limit() == peak - 1)
    );
}

#[test]
fn payload_base_use_census_rejects_missing_and_duplicate_actual_rows() {
    let positive = run_base_use_index_v29(4, 0, LIMIT, LIMIT);
    assert!(positive.0.is_ok() && positive.3, "{:?}", positive.0);
    for fault in [1, 2] {
        let refused = run_base_use_index_v29(4, fault, LIMIT, LIMIT);
        assert!(PAYLOAD_BASE_USE_MUTATED_V29.get());
        assert!(!refused.3);
        assert!(
            matches!(refused.0, Err(ProductionSourceOwnedViewErrorV18::Source(
            ProductionPendingScopedSourceErrorV29::Source(ProductionSemanticKirErrorV1::Unsupported { detail, .. })))
            if detail == "typed object source payload differs from its actual operation"),
            "fault={fault}: {:?}",
            refused.0
        );
    }
}

#[test]
fn payload_read_capacity_matches_actual_operations_coordinates_and_paid_storage() {
    for count in [1, 4, 16] {
        let result = run_base_use_index_v29(count, 0, LIMIT, LIMIT);
        assert!(
            result.0.is_ok() && result.3,
            "count={count}: {:?}",
            result.0
        );
        let calls = PAYLOAD_BASE_USE_OBSERVATIONS_V29.get();
        assert!(calls >= 3);
        assert_eq!(
            SOURCE_OBJECT_READ_CENSUS_WORK_V29.get(),
            (calls * 2 * PAYLOAD_READ_OBJECTS_V29.get(), calls)
        );
        assert_eq!(
            SOURCE_OBJECT_READ_INDEX_STORAGE_V29.get(),
            (calls, PAYLOAD_READ_EXPECTED_BYTES_V29.get())
        );
    }
}

#[test]
fn payload_read_classification_refuses_one_short_work_before_capacity_or_publication() {
    let positive = run_base_use_index_v29(4, 0, LIMIT, LIMIT);
    assert!(positive.0.is_ok() && positive.3, "{:?}", positive.0);
    let before = SOURCE_OBJECT_READ_FIRST_CLASSIFICATION_V29.get().unwrap();
    let limit = before + 1;
    let refused = run_base_use_index_v29(4, 0, limit, LIMIT);
    assert!(!refused.3);
    assert_eq!(
        SOURCE_OBJECT_READ_FIRST_CLASSIFICATION_V29.get(),
        Some(before)
    );
    assert_eq!(SOURCE_OBJECT_READ_CENSUS_WORK_V29.get(), (0, 0));
    assert_eq!(SOURCE_OBJECT_READ_INDEX_STORAGE_V29.get(), (0, 0));
    assert_eq!(PAYLOAD_BASE_USE_OBSERVATIONS_V29.get(), 0);
    assert_eq!(refused.1, before);
    assert!(
        matches!(original_repeated_source_resource_v29(refused.0.unwrap_err()),
        ArgumentResourceV1::Work(error) if error.actual() == before + 2 && error.limit() == limit)
    );
}

#[test]
fn payload_read_census_rejects_missing_and_duplicate_actual_rows() {
    let positive = run_base_use_index_v29(4, 0, LIMIT, LIMIT);
    assert!(positive.0.is_ok() && positive.3, "{:?}", positive.0);
    for fault in [3, 4] {
        let refused = run_base_use_index_v29(4, fault, LIMIT, LIMIT);
        assert!(PAYLOAD_BASE_USE_MUTATED_V29.get());
        assert!(!refused.3);
        assert!(
            matches!(refused.0, Err(ProductionSourceOwnedViewErrorV18::Source(
            ProductionPendingScopedSourceErrorV29::Source(ProductionSemanticKirErrorV1::Unsupported { detail, .. })))
            if detail == "typed object source payload differs from its actual operation"),
            "fault={fault}: {:?}",
            refused.0
        );
    }
}
