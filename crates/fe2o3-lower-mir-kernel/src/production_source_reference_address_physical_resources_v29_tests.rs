#[derive(Clone, Copy, Debug)]
enum PhysicalAddressCase {
    Stored,
    FreshRestart,
    SavedRestart,
    IndexedSeries(usize),
}

fn stored_physical_owner_v29() -> ProductionSemanticSsaOwnerV1 {
    physical_address_owner(PhysicalAddressCase::Stored)
}

fn physical_prepared_result_v29(
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<ProductionPreparedSourceV18> {
    let projection = stored_physical_owner_v29();
    let owner = stored_physical_owner_v29();
    let (_, launch) = with_module_fixture_view(&owner, ModuleFixture::Ordinary, budget, |_, _| ())?;
    with_module_fixture_view(
        &projection,
        ModuleFixture::Ordinary,
        budget,
        |source, budget| {
            ProductionPendingScopedSourceOwnerV29::prepare_source_with_budget_v18(
                owner,
                launch,
                source.input,
                ProductionSemanticKirLimitsV1::default(),
                budget,
            )
        },
    )?
    .0
}

fn final_physical_resource_run_v29(
    work_limit: usize,
    storage_limit: usize,
) -> (SourceOwnedResultV18<()>, usize, usize, bool) {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(work_limit);
    let mut budget = ArgumentBudgetV1::new(&mut work, storage_limit);
    budget.reserve_storage(MODULE_FLOOR).unwrap();
    let mut completed = false;
    let result = (|| {
        let prepared = physical_prepared_result_v29(&mut budget)?;
        prepared.with_source_consumer_v18(
            &mut budget,
            |source, budget| -> SourceOwnedResultV18<()> {
                source.with_analysis_v18(budget, |scope| {
                    scope.with_inventory_v1(|inventory, budget| {
                        source.with_ranked_correspondence_v18(
                            inventory,
                            budget,
                            |relation, budget| {
                                scoped_raw_admission_v29::with_checked_source_memory_v29(
                                    relation,
                                    0,
                                    None,
                                    budget,
                                    |physical, budget| -> SourceOwnedResultV18<()> {
                                        let mut effects = 0;
                                        physical.visit_effects(budget, |_, _| {
                                            effects += 1;
                                            Ok(())
                                        })?;
                                        assert!(effects > 0);
                                        completed = true;
                                        Ok(())
                                    },
                                )
                            },
                        )
                    })
                })
            },
        )
    })();
    assert_eq!(budget.storage(), MODULE_FLOOR, "{result:?}");
    (result, budget.work(), budget.peak_storage(), completed)
}

#[test]
fn actual_final_physical_census_has_exact_and_one_short_work_storage_boundaries() {
    let (result, work, storage, completed) =
        final_physical_resource_run_v29(MODULE_LIMIT, MODULE_LIMIT);
    result.unwrap();
    assert!(completed);
    let (result, _, _, completed) = final_physical_resource_run_v29(work, storage);
    result.unwrap();
    assert!(completed);
    for (work_limit, storage_limit, expect_work) in
        [(work - 1, storage, true), (work, storage - 1, false)]
    {
        // The helper returns after every inner assertion. A caught assertion
        // panic cannot qualify as an expected resource denial.
        let (result, _, _, _) = final_physical_resource_run_v29(work_limit, storage_limit);
        let error = match result {
            Err(ProductionSourceOwnedViewErrorV18::Resource(error)) => error,
            Err(ProductionSourceOwnedViewErrorV18::Source(
                ProductionPendingScopedSourceErrorV29::Source(
                    ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(error),
                ),
            )) => error,
            other => panic!("exact original resource denial required: {other:?}"),
        };
        if expect_work {
            assert!(matches!(error, ArgumentResourceV1::Work(_)));
        } else {
            assert!(matches!(error, ArgumentResourceV1::Storage(_)));
        }
    }
}

#[test]
fn final_physical_census_borrows_the_actual_immutable_source_graph_and_exact_accesses() {
    for fault in 0..5 {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(MODULE_LIMIT);
        let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
        budget.reserve_storage(MODULE_FLOOR).unwrap();
        let prepared = scalar_payload_prepared_from_v18(stored_physical_owner_v29, &mut budget);
        let completed = std::cell::Cell::new(false);
        let result = prepared.with_source_consumer_v18(&mut budget,
            |source, budget| -> SourceOwnedResultV18<()> {
            source.with_analysis_v18(budget, |scope| scope.with_inventory_v1(|inventory, budget| {
                source.with_ranked_correspondence_v18(inventory, budget, |relation, budget| {
                    scoped_raw_admission_v29::with_checked_source_memory_v29(relation, 0, None, budget,
                        |physical, budget| -> SourceOwnedResultV18<()> {
                        let root = source.root(0, budget)?.1;
                        let body = inventory.functions()[root].function.body.as_ref().unwrap();
                        let mut raw = 0;
                        let mut invocation = 0;
                        let mut ordinary = 0;
                        for (block, body) in body.blocks.iter().enumerate() {
                            for (operation, value) in body.operations.iter().enumerate() {
                                let pointer = match &value.kind {
                                    OperationKind::Storage(ScopedObjectOperationV29::ReadValue { address, .. }
                                        | ScopedObjectOperationV29::WriteValue { address, .. }) => *address,
                                    _ => continue,
                                };
                                let operation = fe2o3_kernel_ir::CanonicalKirOperationCoordinateV1 {
                                    block: fe2o3_kernel_ir::CanonicalKirBlockCoordinateV1 {
                                        function: fe2o3_kernel_ir::CanonicalKirFunctionCoordinateV1(root as u32),
                                        block: block as u32,
                                    }, operation: operation as u32,
                                };
                                let original = relation.retained_object_payload_v29(0, operation, budget)?
                                    .expect("every actual typed memory effect has one original Object anchor");
                                let endpoint = match original.source.role {
                                    ScopedObjectRoleV29::ReadValue { source, .. } => source,
                                    ScopedObjectRoleV29::WriteValue { destination, .. } => destination,
                                    _ => panic!("typed memory endpoint"),
                                };
                                let Some(access) = physical.access(original.instance, original.row, operation, pointer, budget)? else {
                                    assert!(matches!(endpoint.object, ScopedObjectIdentityV29::Local { instance, local, generation }
                                        if instance.index() == original.instance && matches!((local.index(), generation), (2, 1) | (6, 3))),
                                        "only exact direct later-generation locals stay pending: {:?} at {operation:?}", endpoint.object);
                                    ordinary += 1;
                                    continue;
                                };
                                assert_eq!(access.operation_pointer(budget)?, (operation, pointer));
                                let direct = matches!(endpoint.object, ScopedObjectIdentityV29::Local { .. });
                                if direct {
                                    assert!(matches!(endpoint.object, ScopedObjectIdentityV29::Local { instance, local, generation: 0 }
                                        if instance.index() == original.instance && local.index() == 3));
                                    invocation += 1;
                                } else {
                                    assert!(matches!(endpoint.object, ScopedObjectIdentityV29::Reference { instance, .. }
                                        if instance.index() == original.instance));
                                    raw += 1;
                                }
                                let mut alternatives = 0;
                                access.visit_alternatives(budget, |instance, local, slot, activation, budget| {
                                    let row = source.root_row(0)?.source_slots.slots.get(slot).unwrap();
                                    assert_eq!(row.instance.index(), instance);
                                    assert_eq!(instance, original.instance);
                                    assert_eq!(local.index(), if direct { 3 } else { 2 });
                                    assert_eq!(row.origin.identity,
                                        ScopedAllocationIdentityV29::OriginalObject { local: local.index(), generation: if direct { 0 } else { 1 } });
                                    assert_eq!(activation, if direct { None } else { Some((SemanticBlockIdV1::from_index(0), 0)) });
                                    source.instance(0, instance, budget)?;
                                    alternatives += 1;
                                    Ok(())
                                })?;
                                assert_eq!(alternatives, 1);
                                // Keep hostile queries on the original raw-reference
                                // witness; newly admitted direct rows are not a substitute.
                                if direct { continue; }
                                if fault == 4 {
                                    // Typed objects require the complete correspondence query;
                                    // the legacy scalar query must refuse and poison this scope.
                                    let attempted = relation.retained_memory_access(0, operation, pointer, budget);
                                    assert!(matches!(attempted, Err(ProductionSourceOwnedViewErrorV18::Binding(
                                        "typed object requires the complete object correspondence query"
                                    ))));
                                    completed.set(true);
                                    return Ok(());
                                }
                                if fault != 0 {
                                    let attempted = match fault {
                                        1 => physical.access(usize::MAX, original.row, operation, pointer, budget),
                                        2 => physical.access(original.instance, usize::MAX, operation, pointer, budget),
                                        3 => {
                                            let candidate = inventory.functions()[root].function.body.as_ref().unwrap()
                                                .blocks.iter().flat_map(|block| &block.operations)
                                                .filter(|operation| matches!(operation.kind, OperationKind::Select { .. }))
                                                .flat_map(|operation| &operation.results)
                                                .find(|result| result.id != pointer).unwrap();
                                            assert!(matches!(candidate.ty, Type::Pointer(_)));
                                            physical.access(original.instance, original.row, operation, candidate.id, budget)
                                        }
                                        _ => unreachable!(),
                                    };
                                    assert!(matches!(attempted, Err(ProductionSourceOwnedViewErrorV18::Binding(_))));
                                    completed.set(true);
                                    // Ignoring the query refusal cannot create a completed scope.
                                    return Ok(());
                                }
                            }
                        }
                        assert_eq!((raw, invocation, ordinary), (1, 4, 2));
                        let mut effects = 0;
                        physical.visit_effects(budget, |_, _| { effects += 1; Ok(()) })?;
                        assert!(effects > raw, "non-access lifetime/formation/return rows are retained");
                        completed.set(true);
                        Ok(())
                    })
                })
            }))
        });
        assert!(
            completed.get(),
            "the actual final physical gate and inner assertions must execute: {result:?}"
        );
        if fault == 0 {
            result.unwrap();
        } else {
            assert!(
                matches!(result, Err(ProductionSourceOwnedViewErrorV18::Binding(_))),
                "{result:?}"
            );
        }
        assert_eq!(budget.storage(), MODULE_FLOOR);
    }
}

thread_local! {
    static PHYSICAL_ADDRESS_PRODUCERS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    static PHYSICAL_ADDRESS_ASSERTIONS_STARTED: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    static PHYSICAL_ADDRESS_ASSERTIONS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    static PHYSICAL_ADDRESS_RUN_FINISHED: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    static PHYSICAL_ADDRESS_FAULT: std::cell::Cell<u8> = const { std::cell::Cell::new(0) };
    static PHYSICAL_ADDRESS_MUTATED: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    static PHYSICAL_ADDRESS_FRESH: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

fn physical_address_owner(case: PhysicalAddressCase) -> ProductionSemanticSsaOwnerV1 {
    let original = module_fixture_owner(ModuleFixture::Ordinary);
    let semantic = original.source_semantic();
    let original_root = &semantic.functions()[0];
    let mut types = semantic.types().to_vec();
    let raw = reference(&mut types, U32, SemanticMutabilityV1::Mutable, true);
    let mut locals = original_root.locals().to_vec();
    assert_eq!(locals.len(), 2);
    locals.extend([
        local(70, U32, SemanticLocalRoleV1::Temporary),
        local(71, raw, SemanticLocalRoleV1::Temporary),
        local(72, raw, SemanticLocalRoleV1::Temporary),
        local(73, U32, SemanticLocalRoleV1::Temporary),
        local(74, U32, SemanticLocalRoleV1::Temporary),
    ]);
    let scalar = |value| {
        SemanticOperandV1::Constant(SemanticConstantV1::new(
            U32,
            SemanticConstantValueV1::Scalar(SemanticScalarValueV1::new(value, 4).unwrap()),
        ))
    };
    let initialize = |id, value| assign(place(id, U32), SemanticRvalueKindV1::Use(scalar(value)));
    let live = |id| {
        SemanticStatementV1::new(
            source(),
            SemanticStatementKindV1::StorageLive(SemanticLocalIdV1::from_index(id)),
        )
    };
    let address = |id, object| {
        assign(
            place(id, raw),
            SemanticRvalueKindV1::AddressOf {
                mutability: SemanticMutabilityV1::Mutable,
                place: place(object, U32),
            },
        )
    };
    let stored = || {
        SemanticStatementV1::new(
            source(),
            SemanticStatementKindV1::Store(SemanticMemoryStoreV1::new(
                place(3, raw),
                SemanticOperandV1::Copy(place(3, raw)),
                SemanticVolatilityV1::NonVolatile,
                None,
            )),
        )
    };
    let mut statements = vec![
        live(2),
        initialize(2, 7),
        live(6),
        initialize(6, 9),
        address(3, 2),
        address(4, 6),
        stored(),
    ];
    if matches!(
        case,
        PhysicalAddressCase::FreshRestart | PhysicalAddressCase::SavedRestart
    ) {
        statements.extend([
            SemanticStatementV1::new(
                source(),
                SemanticStatementKindV1::StorageDead(SemanticLocalIdV1::from_index(2)),
            ),
            live(2),
            initialize(2, 11),
        ]);
        if matches!(case, PhysicalAddressCase::FreshRestart) {
            statements.extend([address(3, 2), stored()]);
        }
    }
    let dereference = || {
        assign(
            place(5, U32),
            SemanticRvalueKindV1::Load(SemanticMemoryLoadV1::new(
                SemanticPlaceV1::new(
                    SemanticLocalIdV1::from_index(3),
                    vec![
                        SemanticProjectionV1::new(SemanticProjectionKindV1::Dereference, U32)
                            .unwrap(),
                    ],
                    U32,
                )
                .unwrap(),
                SemanticVolatilityV1::NonVolatile,
                None,
            )),
        )
    };
    if let PhysicalAddressCase::IndexedSeries(count) = case {
        for _ in 0..count {
            statements.push(stored());
            statements.push(dereference());
            statements.push(assign(
                place(6, U32),
                SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(place(2, U32))),
            ));
        }
    }
    statements.push(dereference());
    let root = function(
        60,
        SemanticFunctionRoleV1::KernelRoot,
        original_root.abi().clone(),
        locals,
        vec![
            block(
                64,
                statements,
                original_root.blocks()[0].terminator().kind().clone(),
            ),
            block(65, vec![], SemanticTerminatorKindV1::Return),
        ],
    )
    .with_kernel_entry(original_root.kernel_entry().unwrap().clone());
    let admitted = InertSemanticMirRequestV1::new_with_callables(
        semantic.target(),
        types,
        vec![],
        vec![],
        vec![],
        vec![root],
        vec![SemanticCallableDeclV1::defined(ROOT)],
        vec![ROOT],
    )
    .unwrap()
    .admit_exact_v29(SemanticMirLimitsV1::default())
    .unwrap();
    ProductionSemanticSsaOwnerV1::try_new(
        ProductionSemanticMirOwnerV1::try_new(admitted, ProductionSemanticMirLimitsV1::default())
            .unwrap(),
        ProductionSemanticSsaLimitsV1::default(),
    )
    .unwrap()
}

thread_local! {
    static OBJECT_PAYLOAD_INDEX_FAULT_V29: std::cell::Cell<u8> = const { std::cell::Cell::new(0) };
    static OBJECT_PAYLOAD_INDEX_MUTATED_V29: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

fn object_payload_index_observer_v29(
    index: &mut SourceObjectPayloadIndexV29,
    _: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    let fault = OBJECT_PAYLOAD_INDEX_FAULT_V29.get();
    if fault == 0 {
        return Ok(());
    }
    assert!(index.occurrences.len() >= 2 && index.reads.len() >= 2);
    // The growing original fixture dereferences a retained pointer. Its exact
    // RvaluePlace/prefix-zero read is consumed by the holder-value join.
    let holder = index
        .reads
        .iter()
        .position(|row| row.0.1[3] == 1 && row.0.2 == 0)
        .expect("an actual retained pointer-holder read");
    match fault {
        1 => index.occurrences[1].0 = index.occurrences[0].0,
        2 => {
            index.reads.remove(holder);
        }
        3 => index.reads[holder].1 = usize::MAX,
        4 => {
            let mut row = index.reads.remove(holder);
            row.0.0 = usize::MAX;
            index.reads.push(row);
        }
        5 => {
            index.reads[holder].0.3 = usize::MAX;
            index.reads.sort_unstable_by_key(|row| row.0);
        }
        6 => index.reads[1].0 = index.reads[0].0,
        7 => index.pending = usize::MAX,
        8 => {
            let mut other_work = CanonicalKernelIrWorkBudgetV1::new(MODULE_LIMIT);
            let other = ArgumentBudgetV1::new(&mut other_work, MODULE_LIMIT);
            index.ledger = other.work_ledger_identity_v1();
        }
        _ => unreachable!(),
    }
    OBJECT_PAYLOAD_INDEX_MUTATED_V29.set(true);
    Ok(())
}

fn run_indexed_object_payloads_v29(
    count: usize,
    fault: u8,
    work_limit: usize,
    storage_limit: usize,
) -> (
    Result<(), ScopedModuleErrorV29>,
    usize,
    usize,
    (usize, usize),
    (usize, usize),
    bool,
) {
    struct Reset(Option<SourceObjectPayloadIndexObserverV29>, u8);
    impl Drop for Reset {
        fn drop(&mut self) {
            SOURCE_OBJECT_PAYLOAD_INDEX_OBSERVER_V29.set(self.0);
            OBJECT_PAYLOAD_INDEX_FAULT_V29.set(self.1);
        }
    }
    let _reset = Reset(
        SOURCE_OBJECT_PAYLOAD_INDEX_OBSERVER_V29.replace(Some(object_payload_index_observer_v29)),
        OBJECT_PAYLOAD_INDEX_FAULT_V29.replace(fault),
    );
    SOURCE_OBJECT_PAYLOAD_INDEX_WORK_V29.set((0, 0));
    SOURCE_OBJECT_PAYLOAD_PASS_WORK_V29.set((0, 0));
    SOURCE_ADDRESS_ACCESS_FIRST_CENSUS_V29.set(None);
    SOURCE_ADDRESS_ACCESS_ALLOCATIONS_V29.set(0);
    scoped_raw_admission_v29::SOURCE_OBJECT_PAYLOAD_QUERY_SCRATCH_V29.set((0, 0));
    scoped_raw_admission_v29::SOURCE_OBJECT_PAYLOAD_ROW_SCRATCH_V29.set((0, 0, 0));
    scoped_raw_admission_v29::PENDING_ALTERNATIVE_CAPACITY_V29.set((0, 0, 0));
    OBJECT_PAYLOAD_INDEX_MUTATED_V29.set(false);
    let mut work = CanonicalKernelIrWorkBudgetV1::new(work_limit);
    let mut budget = ArgumentBudgetV1::new(&mut work, storage_limit);
    budget.reserve_storage(MODULE_FLOOR).unwrap();
    let mut completed = false;
    let result = (|| {
        let original = physical_address_owner(PhysicalAddressCase::IndexedSeries(count));
        let (input, launch) = with_module_fixture_view(
            &original,
            ModuleFixture::Ordinary,
            &mut budget,
            |source, budget| OwnedExecutionInputV29::capture(source, budget),
        )?;
        let roots = original.source_semantic().roots().len();
        let before_construction = SOURCE_OBJECT_PAYLOAD_PASS_WORK_V29.get().0;
        let mut donor = Some(ScopedSourceInputsV29 {
            owner: original,
            launch,
            input: input?,
        });
        let owner = SourceOwnedScopedModuleV29::try_new(
            &mut donor,
            ProductionSemanticKirLimitsV1::default(),
            &mut budget,
        )?;
        assert!(donor.is_none());
        assert!(owner.retained_storage > 0);
        assert_eq!(budget.storage(), MODULE_FLOOR + owner.retained_storage);
        // Construction checks the emitted candidate, then reconstructs it once.
        let after_construction = SOURCE_OBJECT_PAYLOAD_PASS_WORK_V29.get().0;
        assert_eq!(
            after_construction - before_construction,
            2 * roots,
            "one admission and one reconstruction payload pass per root"
        );
        let floor = budget.storage();
        let identity = *owner.pending.graph.identity();
        let replay = owner.replay(&mut budget);
        assert_eq!(owner.pending.graph.identity(), &identity);
        assert_eq!(budget.storage(), floor);
        let retained = owner.retained_storage;
        drop(owner);
        budget.release_storage(retained)?;
        replay?;
        assert_eq!(
            SOURCE_OBJECT_PAYLOAD_PASS_WORK_V29.get().0 - after_construction,
            roots,
            "one explicit immutable replay payload pass per root"
        );
        let (queries, reclaimed) =
            scoped_raw_admission_v29::SOURCE_OBJECT_PAYLOAD_QUERY_SCRATCH_V29.get();
        assert_eq!(queries, 3 * roots);
        assert!(reclaimed > 0, "real payload queries must reserve scratch");
        completed = true;
        Ok(())
    })();
    assert_eq!(budget.storage(), MODULE_FLOOR, "{result:?}");
    (
        result,
        budget.work(),
        budget.peak_storage(),
        SOURCE_OBJECT_PAYLOAD_INDEX_WORK_V29.get(),
        SOURCE_OBJECT_PAYLOAD_PASS_WORK_V29.get(),
        completed,
    )
}

#[test]
fn original_object_payload_unit_query_scratch_ends_before_pending_retention() {
    let mut previous = 0;
    for count in [1, 4, 16] {
        let (result, _, _, index, payloads, completed) =
            run_indexed_object_payloads_v29(count, 0, MODULE_LIMIT, MODULE_LIMIT);
        result.unwrap();
        assert!(completed);
        assert!(index.0 > count);
        let (queries, reclaimed) =
            scoped_raw_admission_v29::SOURCE_OBJECT_PAYLOAD_QUERY_SCRATCH_V29.get();
        assert_eq!((queries, payloads.0), (3, 3));
        assert!(
            reclaimed > previous,
            "growing query frames must be released"
        );
        previous = reclaimed;
    }
}

#[test]
fn original_object_payload_rows_reclaim_scratch_before_the_next_source_row() {
    let mut previous = (0, 0);
    let mut largest_row = None;
    for count in [1, 4, 16] {
        let (result, _, _, _, payloads, completed) =
            run_indexed_object_payloads_v29(count, 0, MODULE_LIMIT, MODULE_LIMIT);
        result.unwrap();
        assert!(completed);
        assert_eq!(payloads.0, 3);
        let (rows, reclaimed, largest) =
            scoped_raw_admission_v29::SOURCE_OBJECT_PAYLOAD_ROW_SCRATCH_V29.get();
        let (queries, query_reclaimed) =
            scoped_raw_admission_v29::SOURCE_OBJECT_PAYLOAD_QUERY_SCRATCH_V29.get();
        assert_eq!(queries, 3);
        assert!(rows > previous.0 && reclaimed > previous.1);
        assert!(largest > 0 && reclaimed > largest);
        assert!(
            query_reclaimed > reclaimed,
            "whole-query envelopes are also paid"
        );
        assert_eq!(
            *largest_row.get_or_insert(largest),
            largest,
            "repeated identical source rows must not accumulate temporary envelopes"
        );
        previous = (rows, reclaimed);
    }
}

#[test]
fn growing_original_object_payload_joins_have_subquadratic_work() {
    let mut measured = Vec::new();
    for count in [32, 64, 128] {
        let (result, work, peak, index, payloads, completed) =
            run_indexed_object_payloads_v29(count, 0, MODULE_LIMIT, MODULE_LIMIT);
        assert!(
            result.is_ok(),
            "count={count}, work={work}, peak={peak}, index={index:?}, payloads={payloads:?}, access_allocations={}: {result:?}",
            SOURCE_ADDRESS_ACCESS_ALLOCATIONS_V29.get()
        );
        assert!(
            completed,
            "actual module construction and immutable replay must succeed"
        );
        assert!(
            index.0 > count,
            "the growing source must exercise indexed lookups"
        );
        assert_eq!(
            payloads.0,
            2 + 1,
            "construction checks and explicit replay of one root"
        );
        measured.push((index.1, payloads.1));
    }
    for scope in 0..2 {
        let work: Vec<_> = measured
            .iter()
            .map(|pair| if scope == 0 { pair.0 } else { pair.1 })
            .collect();
        let first = work[1].checked_sub(work[0]).expect("growing work");
        let second = work[2].checked_sub(work[1]).expect("growing work");
        assert!(
            first > 0 && second < 3 * first,
            "indexed/payload work must grow subquadratically: {work:?}"
        );
    }
}

thread_local! {
    static PHYSICAL_ACCESS_CAPACITY_OBSERVATIONS_V29: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

fn observe_physical_access_capacity_v29(
    index: &SourceAddressSourceIndexV29<'_>,
    actual: &[SourceAddressAccessSourceV29],
    bound: usize,
    capacity: usize,
    paid: usize,
) {
    let mut expected = Vec::new();
    let mut total = 0;
    for sidecar in &index.pending.sidecars.rows {
        let instance = sidecar.source_call_instance.unwrap();
        let anchors = sidecar.scoped_memory_anchors.as_ref().unwrap();
        total += anchors.rows.len();
        for (anchor, row) in anchors.rows.iter().enumerate() {
            let selected = match row.kind {
                ScopedMemoryAnchorKindV29::Object(object) => {
                    // Independently select by typed source role, then require
                    // its corresponding actual operation form.
                    let payload = &anchors.objects[object];
                    match payload.role {
                        ScopedObjectRoleV29::ReadValue { .. } => {
                            assert!(matches!(
                                payload.operation,
                                ScopedObjectOperationV29::ReadValue { .. }
                            ));
                            true
                        }
                        ScopedObjectRoleV29::WriteValue { .. } => {
                            assert!(matches!(
                                payload.operation,
                                ScopedObjectOperationV29::WriteValue { .. }
                            ));
                            true
                        }
                        _ => false,
                    }
                }
                ScopedMemoryAnchorKindV29::Access { payload, .. } => payload.is_some(),
                _ => false,
            };
            if selected {
                expected.push((instance.index(), anchor));
            }
        }
    }
    let mut observed = actual
        .iter()
        .map(|row| (row.instance.index(), row.anchor))
        .collect::<Vec<_>>();
    expected.sort_unstable();
    observed.sort_unstable();
    // This original-object fixture has no descriptor or issued exclusions.
    assert_eq!(observed, expected);
    assert_eq!(bound, expected.len());
    assert!(bound > 0 && bound < total);
    let mut allocation = Vec::<SourceAddressAccessSourceV29>::new();
    allocation.try_reserve_exact(expected.len()).unwrap();
    assert_eq!(capacity, allocation.capacity());
    assert!(capacity < total);
    assert_eq!(
        paid,
        capacity * std::mem::size_of::<SourceAddressAccessSourceV29>()
    );
    for row in actual {
        let operation = &index
            .pending
            .function
            .body
            .as_ref()
            .unwrap()
            .blocks
            .iter()
            .find(|block| block.id == row.physical.block)
            .unwrap()
            .operations[row.physical.operation];
        assert!(matches!(
            operation.kind,
            OperationKind::Load { .. }
                | OperationKind::Store { .. }
                | OperationKind::Storage(
                    ScopedObjectOperationV29::ReadValue { .. }
                        | ScopedObjectOperationV29::WriteValue { .. }
                )
        ));
    }
    PHYSICAL_ACCESS_CAPACITY_OBSERVATIONS_V29
        .set(PHYSICAL_ACCESS_CAPACITY_OBSERVATIONS_V29.get() + 1);
}

#[test]
fn physical_access_capacity_tracks_actual_value_rows_and_paid_allocation() {
    struct Restore(Option<SourceAddressAccessCapacityObserverV29>);
    impl Drop for Restore {
        fn drop(&mut self) {
            SOURCE_ADDRESS_ACCESS_CAPACITY_OBSERVER_V29.set(self.0);
        }
    }
    let _restore = Restore(
        SOURCE_ADDRESS_ACCESS_CAPACITY_OBSERVER_V29
            .replace(Some(observe_physical_access_capacity_v29)),
    );
    for count in [1, 4, 16] {
        PHYSICAL_ACCESS_CAPACITY_OBSERVATIONS_V29.set(0);
        let (result, _, _, _, payloads, completed) =
            run_indexed_object_payloads_v29(count, 0, MODULE_LIMIT, MODULE_LIMIT);
        result.unwrap();
        assert!(completed);
        assert_eq!(payloads.0, 3);
        assert_eq!(PHYSICAL_ACCESS_CAPACITY_OBSERVATIONS_V29.get(), 3);
        assert_eq!(SOURCE_ADDRESS_ACCESS_ALLOCATIONS_V29.get(), 3);
    }
}

#[test]
fn physical_access_candidate_census_refuses_one_short_before_allocation() {
    let (result, _, _, _, _, completed) =
        run_indexed_object_payloads_v29(4, 0, MODULE_LIMIT, MODULE_LIMIT);
    result.unwrap();
    assert!(completed);
    let before = SOURCE_ADDRESS_ACCESS_FIRST_CENSUS_V29.get().unwrap();
    let (result, _, _, _, _, completed) =
        run_indexed_object_payloads_v29(4, 0, before, MODULE_LIMIT);
    assert!(!completed);
    assert_eq!(SOURCE_ADDRESS_ACCESS_FIRST_CENSUS_V29.get(), Some(before));
    assert_eq!(SOURCE_ADDRESS_ACCESS_ALLOCATIONS_V29.get(), 0);
    assert!(
        matches!(&result,
        Err(ScopedModuleErrorV29::Source(
            ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                ArgumentResourceV1::Work(error))))
        if error.actual() == before + 1 && error.limit() == before),
        "{result:?}"
    );
}

#[test]
fn original_object_pending_alternatives_preflight_bounds_actual_retained_rows() {
    let mut previous = 0;
    for count in [0, 1, 4, 16] {
        let (result, _, _, _, payloads, completed) =
            run_indexed_object_payloads_v29(count, 0, MODULE_LIMIT, MODULE_LIMIT);
        result.unwrap();
        assert!(completed);
        assert_eq!(payloads.0, 3);
        let (bound, capacity, actual) =
            scoped_raw_admission_v29::PENDING_ALTERNATIVE_CAPACITY_V29.get();
        assert!(
            actual > previous && actual <= bound && bound <= capacity,
            "count={count} bound={bound} capacity={capacity} actual={actual}"
        );
        previous = actual;
    }
}

#[test]
fn original_object_payload_index_rejects_duplicate_missing_stale_and_foreign_keys() {
    let (result, _, _, _, _, completed) =
        run_indexed_object_payloads_v29(4, 0, MODULE_LIMIT, MODULE_LIMIT);
    result.unwrap();
    assert!(completed);
    for fault in 1..=8 {
        let (result, _, _, _, _, completed) =
            run_indexed_object_payloads_v29(4, fault, MODULE_LIMIT, MODULE_LIMIT);
        assert!(
            OBJECT_PAYLOAD_INDEX_MUTATED_V29.get(),
            "actual indexed source must reach sabotage"
        );
        assert!(!completed);
        if fault <= 6 {
            assert!(
                matches!(
                    result,
                    Err(ScopedModuleErrorV29::Source(
                        ProductionSemanticKirErrorV1::Unsupported { .. }
                    ))
                ),
                "semantic key refusal, not a swallowed assertion or exhausted budget: {result:?}"
            );
        } else {
            assert!(
                matches!(
                    result,
                    Err(ScopedModuleErrorV29::Source(
                        ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                            ArgumentResourceV1::Accounting
                        )
                    ))
                ),
                "exact foreign custody refusal required: {result:?}"
            );
        }
    }
}

#[test]
fn indexed_object_payload_module_preserves_exact_resource_boundaries() {
    let (result, work, storage, _, _, completed) =
        run_indexed_object_payloads_v29(4, 0, MODULE_LIMIT, MODULE_LIMIT);
    result.unwrap();
    assert!(completed);
    let (result, _, _, _, _, completed) = run_indexed_object_payloads_v29(4, 0, work, storage);
    result.unwrap();
    assert!(completed);
    for (work_limit, storage_limit, is_work) in
        [(work - 1, storage, true), (work, storage - 1, false)]
    {
        let (result, _, _, _, _, completed) =
            run_indexed_object_payloads_v29(4, 0, work_limit, storage_limit);
        assert!(!completed);
        descriptor_resource_error(result.unwrap_err(), is_work);
    }
}

fn physical_address_observer(
    _: &ExecutionLifecycleSourceV29<'_>,
    _: &ExecutionInstancesV29<'_>,
    emitted: &mut [Option<LoweredFunctionResultV1>],
    _: &OwnedScopedSourceSlotsV29,
    _: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    PHYSICAL_ADDRESS_ASSERTIONS_STARTED.set(PHYSICAL_ADDRESS_ASSERTIONS_STARTED.get() + 1);
    let mut aliases = 0;
    let mut pointer_loads = 0;
    let mut stores = 0;
    let mut scalar_loads = 0;
    let mut object_rows = 0;
    for row in emitted.iter().flatten() {
        let body = row.function.body.as_ref().unwrap();
        for operation in body.blocks.iter().flat_map(|block| &block.operations) {
            match &operation.kind {
                OperationKind::Select {
                    true_value,
                    false_value,
                    ..
                } => {
                    assert_eq!(true_value, false_value);
                    assert!(matches!(operation.results.as_slice(), [result]
                        if matches!(&result.ty, Type::Pointer(pointer)
                            if pointer.address_space == AddressSpace::Private
                                && pointer.access == AccessMode::ReadWrite
                                && matches!(*pointer.pointee, Type::StorageObject(_)))));
                    aliases += 1;
                }
                OperationKind::Storage(ScopedObjectOperationV29::ReadValue { .. })
                    if operation
                        .results
                        .iter()
                        .any(|result| matches!(result.ty, Type::Pointer(_))) =>
                {
                    pointer_loads += 1
                }
                OperationKind::Storage(ScopedObjectOperationV29::ReadValue { .. }) => {
                    assert!(
                        matches!(operation.results.as_slice(), [result] if result.ty == Type::Scalar(ScalarType::U32))
                    );
                    scalar_loads += 1;
                }
                OperationKind::Storage(ScopedObjectOperationV29::WriteValue { .. }) => stores += 1,
                OperationKind::Load { .. }
                | OperationKind::Store { .. }
                | OperationKind::Cast {
                    kind: CastKind::PointerToGeneric,
                    ..
                }
                | OperationKind::Storage(_) => {
                    panic!("whole typed cells must use their selected operation family")
                }
                _ => {}
            }
        }
        let anchors = row.scoped_memory_anchors.as_ref().unwrap();
        object_rows += anchors
            .rows
            .iter()
            .filter(|anchor| matches!(anchor.kind, ScopedMemoryAnchorKindV29::Object(_)))
            .count();
    }
    let fresh = usize::from(PHYSICAL_ADDRESS_FRESH.get());
    assert_eq!(
        aliases,
        2 + fresh,
        "one fresh identity per original AddressOf"
    );
    assert_eq!(
        pointer_loads,
        2 + fresh,
        "stored RHS reads plus final dereference holder read"
    );
    assert_eq!(scalar_loads, 1, "the original final scalar read");
    assert_eq!(
        stores,
        4 + 3 * fresh,
        "scalar initialization, pointer assignment and explicit stores"
    );
    assert_eq!(object_rows, pointer_loads + scalar_loads + stores);
    let fault = PHYSICAL_ADDRESS_FAULT.get();
    if fault != 0 {
        let row = emitted.iter_mut().flatten().next().unwrap();
        let body = row.function.body.as_mut().unwrap();
        let formations: Vec<_> = body
            .blocks
            .iter()
            .flat_map(|block| &block.operations)
            .filter_map(|operation| match &operation.kind {
                OperationKind::Select { true_value, .. } => Some((
                    *true_value,
                    operation.results[0].id,
                    operation.results[0].ty.clone(),
                )),
                _ => None,
            })
            .collect();
        assert!(formations.len() >= 2);
        assert_eq!(formations[0].2, formations[1].2);
        assert_ne!(formations[0].0, formations[1].0);
        assert_ne!(formations[0].1, formations[1].1);
        let mut changed = false;
        for operation in body
            .blocks
            .iter_mut()
            .flat_map(|block| &mut block.operations)
        {
            match (&mut operation.kind, fault) {
                (
                    OperationKind::Select {
                        true_value,
                        false_value,
                        ..
                    },
                    1,
                ) if *true_value == formations[0].0 => {
                    *true_value = formations[1].0;
                    *false_value = formations[1].0;
                    changed = true;
                }
                (OperationKind::Storage(ScopedObjectOperationV29::WriteValue { value, .. }), 2)
                    if *value == formations[0].1 =>
                {
                    *value = formations[1].1;
                    changed = true;
                }
                (
                    OperationKind::Storage(ScopedObjectOperationV29::ReadValue {
                        address: pointer,
                        ..
                    }),
                    3,
                ) if operation
                    .results
                    .iter()
                    .any(|result| result.ty == Type::Scalar(ScalarType::U32)) =>
                {
                    *pointer = formations[1].1;
                    changed = true;
                }
                (OperationKind::Storage(ScopedObjectOperationV29::ReadValue { .. }), 5) if matches!(operation.results.as_slice(), [result] if result.ty == Type::Scalar(ScalarType::U32)) =>
                {
                    operation.results[0].ty = Type::Scalar(ScalarType::I32);
                    changed = true;
                }
                (
                    OperationKind::Storage(ScopedObjectOperationV29::WriteValue { access, .. }),
                    6,
                ) => {
                    access.volatile = true;
                    changed = true;
                }
                (
                    OperationKind::Storage(ScopedObjectOperationV29::ReadValue { address, .. }),
                    8,
                ) if matches!(operation.results.as_slice(), [result] if result.ty == Type::Scalar(ScalarType::U32)) =>
                {
                    assert_eq!(
                        formations.len(),
                        3,
                        "fresh restart must form a new generation's address"
                    );
                    assert_ne!(formations[0].0, formations[2].0);
                    *address = formations[0].1;
                    changed = true;
                }
                _ => {}
            }
            if changed {
                break;
            }
        }
        if fault == 4 {
            for object in &mut row.scoped_memory_anchors.as_mut().unwrap().objects {
                let endpoint = match &mut object.role {
                    ScopedObjectRoleV29::WriteValue { destination, .. } => destination,
                    _ => continue,
                };
                if let ScopedObjectIdentityV29::Local { generation, .. } = &mut endpoint.object {
                    assert_ne!(*generation, u32::MAX);
                    *generation = u32::MAX;
                    changed = true;
                    break;
                }
            }
        }
        if fault == 7 {
            let anchors = row.scoped_memory_anchors.as_mut().unwrap();
            let writes: Vec<_> = anchors
                .rows
                .iter()
                .enumerate()
                .filter_map(|(index, anchor)| {
                    let ScopedMemoryAnchorKindV29::Object(object) = anchor.kind else {
                        return None;
                    };
                    matches!(
                        anchors.objects[object].role,
                        ScopedObjectRoleV29::WriteValue { .. }
                    )
                    .then_some(index)
                })
                .collect();
            assert!(writes.len() >= 2);
            // Duplicate an existing original role rather than removing an IR
            // operation: the complete source census must reject the omission.
            let replacement = anchors.rows[writes[1]].kind;
            anchors.rows[writes[0]].kind = replacement;
            changed = true;
        }
        assert!(
            changed,
            "the sabotage must change a real original operation"
        );
        PHYSICAL_ADDRESS_MUTATED.set(true);
    }
    PHYSICAL_ADDRESS_PRODUCERS.set(PHYSICAL_ADDRESS_PRODUCERS.get() + 1);
    PHYSICAL_ADDRESS_ASSERTIONS.set(PHYSICAL_ADDRESS_ASSERTIONS.get() + 1);
    Ok(())
}

fn run_physical_address_module(
    case: PhysicalAddressCase,
    work_limit: usize,
    storage_limit: usize,
) -> (Result<(), ScopedModuleErrorV29>, usize, usize) {
    struct Reset(Option<ScopedSlotObserverV29>);
    impl Drop for Reset {
        fn drop(&mut self) {
            SCOPED_SLOT_OBSERVER_V29.set(self.0);
        }
    }
    let reset = Reset(SCOPED_SLOT_OBSERVER_V29.replace(Some(physical_address_observer)));
    PHYSICAL_ADDRESS_PRODUCERS.set(0);
    PHYSICAL_ADDRESS_ASSERTIONS_STARTED.set(0);
    PHYSICAL_ADDRESS_ASSERTIONS.set(0);
    PHYSICAL_ADDRESS_RUN_FINISHED.set(false);
    PHYSICAL_ADDRESS_FRESH.set(matches!(case, PhysicalAddressCase::FreshRestart));
    let mut work = CanonicalKernelIrWorkBudgetV1::new(work_limit);
    let mut budget = ArgumentBudgetV1::new(&mut work, storage_limit);
    budget.reserve_storage(MODULE_FLOOR).unwrap();
    let result = (|| {
        let original = physical_address_owner(case);
        let (input, launch) = with_module_fixture_view(
            &original,
            ModuleFixture::Ordinary,
            &mut budget,
            |source, budget| OwnedExecutionInputV29::capture(source, budget),
        )?;
        let mut donor = Some(ScopedSourceInputsV29 {
            owner: original,
            launch,
            input: input?,
        });
        let owner = SourceOwnedScopedModuleV29::try_new(
            &mut donor,
            ProductionSemanticKirLimitsV1::default(),
            &mut budget,
        )?;
        assert!(donor.is_none());
        assert!(PHYSICAL_ADDRESS_PRODUCERS.get() > 0);
        let floor = budget.storage();
        let identity = *owner.pending.graph.identity();
        let replay = owner.replay(&mut budget);
        assert_eq!(owner.pending.graph.identity(), &identity);
        assert_eq!(budget.storage(), floor);
        let retained = owner.retained_storage;
        drop(owner);
        budget.release_storage(retained)?;
        replay
    })();
    assert_eq!(budget.storage(), MODULE_FLOOR, "{case:?}: {result:?}");
    PHYSICAL_ADDRESS_RUN_FINISHED.set(true);
    drop(reset);
    (result, budget.work(), budget.peak_storage())
}

#[test]
fn actual_source_stored_raw_pointer_and_fresh_restart_reach_module_and_replay() {
    for case in [
        PhysicalAddressCase::Stored,
        PhysicalAddressCase::FreshRestart,
    ] {
        let original = physical_address_owner(case);
        let promoted = original.plans()[0].plan().promoted_variables();
        assert!(
            !promoted.iter().any(|variable| variable.get() == 3),
            "the stored raw holder must exercise retained memory, not a fabricated SSA definition"
        );
        assert!(
            promoted.iter().any(|variable| variable.get() == 4),
            "the independent raw formation must also exercise promoted pointer admission"
        );
        let result = run_physical_address_module(case, MODULE_LIMIT, MODULE_LIMIT).0;
        assert!(PHYSICAL_ADDRESS_RUN_FINISHED.get());
        assert_eq!(
            PHYSICAL_ADDRESS_ASSERTIONS_STARTED.get(),
            PHYSICAL_ADDRESS_ASSERTIONS.get()
        );
        assert!(
            PHYSICAL_ADDRESS_ASSERTIONS.get() > 0,
            "{case:?}: {result:?}"
        );
        result.unwrap_or_else(|error| panic!("{case:?}: {error:?}"));
    }
}

#[test]
fn actual_source_saved_raw_pointer_cannot_revive_after_storage_restart() {
    let result = run_physical_address_module(
        PhysicalAddressCase::SavedRestart,
        MODULE_LIMIT,
        MODULE_LIMIT,
    )
    .0;
    assert!(PHYSICAL_ADDRESS_RUN_FINISHED.get());
    assert_eq!(
        PHYSICAL_ADDRESS_ASSERTIONS_STARTED.get(),
        PHYSICAL_ADDRESS_ASSERTIONS.get()
    );
    assert_eq!(PHYSICAL_ADDRESS_PRODUCERS.get(), 0);
    assert!(
        matches!(
            result,
            Err(ScopedModuleErrorV29::Source(
                ProductionSemanticKirErrorV1::Unsupported {
                    detail: "source raw pointer outlived its storage activation",
                    ..
                }
            ))
        ),
        "{result:?}"
    );
}

#[test]
fn actual_source_raw_module_and_replay_obey_exact_and_one_short_resource_limits() {
    let case = PhysicalAddressCase::Stored;
    let (result, work, storage) = run_physical_address_module(case, MODULE_LIMIT, MODULE_LIMIT);
    result.unwrap();
    assert!(PHYSICAL_ADDRESS_RUN_FINISHED.get());
    assert_eq!(
        PHYSICAL_ADDRESS_ASSERTIONS_STARTED.get(),
        PHYSICAL_ADDRESS_ASSERTIONS.get()
    );
    assert!(PHYSICAL_ADDRESS_ASSERTIONS.get() > 0);
    run_physical_address_module(case, work, storage).0.unwrap();
    assert!(PHYSICAL_ADDRESS_RUN_FINISHED.get());
    assert_eq!(
        PHYSICAL_ADDRESS_ASSERTIONS_STARTED.get(),
        PHYSICAL_ADDRESS_ASSERTIONS.get()
    );
    assert!(PHYSICAL_ADDRESS_ASSERTIONS.get() > 0);
    for (work_limit, storage_limit, is_work) in
        [(work - 1, storage, true), (work, storage - 1, false)]
    {
        let (result, _, _) = run_physical_address_module(case, work_limit, storage_limit);
        assert!(PHYSICAL_ADDRESS_RUN_FINISHED.get());
        assert_eq!(
            PHYSICAL_ADDRESS_ASSERTIONS_STARTED.get(),
            PHYSICAL_ADDRESS_ASSERTIONS.get()
        );
        descriptor_resource_error(result.unwrap_err(), is_work);
    }
}

#[test]
fn actual_source_raw_census_rejects_changed_formation_payload_and_access() {
    struct Reset(u8);
    impl Drop for Reset {
        fn drop(&mut self) {
            PHYSICAL_ADDRESS_FAULT.set(self.0);
        }
    }
    for fault in 1..=8 {
        let reset = Reset(PHYSICAL_ADDRESS_FAULT.replace(fault));
        PHYSICAL_ADDRESS_MUTATED.set(false);
        let case = if fault == 8 {
            PhysicalAddressCase::FreshRestart
        } else {
            PhysicalAddressCase::Stored
        };
        let result = run_physical_address_module(case, MODULE_LIMIT, MODULE_LIMIT).0;
        assert!(
            PHYSICAL_ADDRESS_RUN_FINISHED.get(),
            "the outer helper must return normally"
        );
        assert!(
            PHYSICAL_ADDRESS_MUTATED.get(),
            "the actual source emitter must reach sabotage"
        );
        assert_eq!(
            PHYSICAL_ADDRESS_ASSERTIONS_STARTED.get(),
            PHYSICAL_ADDRESS_ASSERTIONS.get()
        );
        assert!(
            matches!(
                result,
                Err(ScopedModuleErrorV29::Source(
                    ProductionSemanticKirErrorV1::Unsupported { .. }
                ))
            ),
            "a semantic census refusal, not a caught assertion/resource failure: {result:?}"
        );
        drop(reset);
    }
}

#[test]
fn final_physical_scope_preserves_error_panic_and_denied_refund_chronology() {
    const ERROR: &str = "original final physical callback error";
    const PANIC: &str = "original final physical callback panic";
    for fault in 0..5 {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(MODULE_LIMIT);
        let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
        budget.reserve_storage(MODULE_FLOOR).unwrap();
        let prepared = scalar_payload_prepared_from_v18(stored_physical_owner_v29, &mut budget);
        let completed = std::cell::Cell::new(false);
        let required = std::cell::Cell::new(0);
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            prepared.with_source_consumer_v18(&mut budget,
                |source, budget| -> SourceOwnedResultV18<()> {
                source.with_analysis_v18(budget, |scope| scope.with_inventory_v1(|inventory, budget| {
                    source.with_ranked_correspondence_v18(inventory, budget, |relation, budget| {
                        scoped_raw_admission_v29::with_checked_source_memory_v29(relation, 0, None, budget,
                            |physical, budget| -> SourceOwnedResultV18<()> {
                            required.set(budget.storage());
                            match fault {
                                0 => {
                                    // The lexical physical constructor must not refund
                                    // an independently owned callback allocation.
                                    budget.reserve_storage(17)?;
                                    completed.set(true);
                                    Ok(())
                                }
                                1 => { completed.set(true); Err(ProductionSourceOwnedViewErrorV18::Binding(ERROR)) }
                                2 => { completed.set(true); std::panic::panic_any(PANIC) }
                                3 => {
                                    let mut foreign_work = CanonicalKernelIrWorkBudgetV1::new(MODULE_LIMIT);
                                    let mut foreign = ArgumentBudgetV1::new(&mut foreign_work, MODULE_LIMIT);
                                    foreign.reserve_storage(required.get()).unwrap();
                                    let refused = physical.visit_effects(&mut foreign, |_, _| panic!("foreign callback"));
                                    assert!(matches!(refused, Err(ProductionSourceOwnedViewErrorV18::Resource(ArgumentResourceV1::Accounting))));
                                    completed.set(true);
                                    refused
                                }
                                4 => {
                                    budget.release_storage(required.get() - 1).unwrap();
                                    let refused = physical.visit_effects(budget, |_, _| panic!("lost-custody callback"));
                                    assert!(matches!(refused, Err(ProductionSourceOwnedViewErrorV18::Resource(ArgumentResourceV1::Accounting))));
                                    completed.set(true);
                                    refused
                                }
                                _ => unreachable!(),
                            }
                        })?;
                        if fault == 0 {
                            assert!(budget.storage() >= 17);
                            budget.release_storage(17)?;
                        }
                        Ok(())
                    })
                }))
            })
        }));
        assert!(
            completed.get(),
            "the actual physical callback and all inner assertions must execute"
        );
        match fault {
            0 => {
                result.unwrap().unwrap();
                assert_eq!(budget.storage(), MODULE_FLOOR);
            }
            1 => {
                assert!(matches!(
                    result,
                    Ok(Err(ProductionSourceOwnedViewErrorV18::Binding(ERROR)))
                ));
                assert_eq!(budget.storage(), MODULE_FLOOR);
            }
            2 => {
                let payload = result.err().expect("original raw panic must unwind");
                assert_eq!(payload.downcast_ref::<&'static str>(), Some(&PANIC));
                assert_eq!(budget.storage(), MODULE_FLOOR);
            }
            3 | 4 => {
                assert!(matches!(
                    result,
                    Ok(Err(ProductionSourceOwnedViewErrorV18::Resource(
                        ArgumentResourceV1::Accounting
                    )))
                ));
                if fault == 3 {
                    assert!(budget.storage() >= required.get());
                } else {
                    assert_eq!(
                        budget.storage(),
                        1,
                        "no enclosing refund may hide lost custody"
                    );
                }
            }
            _ => unreachable!(),
        }
    }
}

#[derive(Debug)]
struct PhysicalRejectedOutputV29(std::rc::Rc<std::cell::Cell<usize>>);

impl Drop for PhysicalRejectedOutputV29 {
    fn drop(&mut self) {
        self.0.set(self.0.get() + 1);
        std::panic::panic_any("physical rejected output destructor");
    }
}

#[derive(Debug)]
enum PhysicalCallbackErrorV29 {
    Source(ProductionSourceOwnedViewErrorV18),
    Hostile(std::rc::Rc<std::cell::Cell<usize>>),
}

impl From<ProductionSourceOwnedViewErrorV18> for PhysicalCallbackErrorV29 {
    fn from(error: ProductionSourceOwnedViewErrorV18) -> Self {
        Self::Source(error)
    }
}

impl From<fe2o3_pliron::CanonicalAnalysisScopeErrorV1> for PhysicalCallbackErrorV29 {
    fn from(error: fe2o3_pliron::CanonicalAnalysisScopeErrorV1) -> Self {
        Self::Source(error.into())
    }
}

impl From<ArgumentResourceV1> for PhysicalCallbackErrorV29 {
    fn from(error: ArgumentResourceV1) -> Self {
        Self::Source(error.into())
    }
}

impl Drop for PhysicalCallbackErrorV29 {
    fn drop(&mut self) {
        if let Self::Hostile(count) = self {
            count.set(count.get() + 1);
            std::panic::panic_any("physical superseded callback error destructor");
        }
    }
}

fn physical_refund_lost_credit_v29(budget: &mut ArgumentBudgetV1<'_>) {
    budget.release_storage(budget.storage() - 1).unwrap();
}

#[test]
fn actual_physical_scope_bounds_rejected_output_and_error_drop_after_selecting_failure() {
    struct Reset(Option<fn(&mut ArgumentBudgetV1<'_>)>);
    impl Drop for Reset {
        fn drop(&mut self) {
            scoped_raw_admission_v29::PHYSICAL_REFUND_OBSERVER_V29.set(self.0);
        }
    }
    for fault in 0..3 {
        let reset = Reset(
            scoped_raw_admission_v29::PHYSICAL_REFUND_OBSERVER_V29.replace(if fault == 1 {
                Some(physical_refund_lost_credit_v29)
            } else {
                None
            }),
        );
        let mut work = CanonicalKernelIrWorkBudgetV1::new(MODULE_LIMIT);
        let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
        budget.reserve_storage(MODULE_FLOOR).unwrap();
        let prepared = scalar_payload_prepared_from_v18(stored_physical_owner_v29, &mut budget);
        let completed = std::cell::Cell::new(false);
        let output_drops = std::rc::Rc::new(std::cell::Cell::new(0));
        let error_drops = std::rc::Rc::new(std::cell::Cell::new(0));
        let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            prepared.with_source_consumer_v18(&mut budget, |source, budget|
                -> Result<PhysicalRejectedOutputV29, PhysicalCallbackErrorV29> {
                source.with_analysis_v18(budget, |scope| scope.with_inventory_v1(|inventory, budget| {
                    source.with_ranked_correspondence_v18(inventory, budget, |relation, budget| {
                        scoped_raw_admission_v29::with_checked_source_memory_v29(relation, 0, None, budget,
                            |physical, budget| -> Result<PhysicalRejectedOutputV29, PhysicalCallbackErrorV29> {
                            if fault != 1 {
                                let coordinate = fe2o3_kernel_ir::CanonicalKirOperationCoordinateV1 {
                                    block: fe2o3_kernel_ir::CanonicalKirBlockCoordinateV1 {
                                        function: fe2o3_kernel_ir::CanonicalKirFunctionCoordinateV1(0), block: 0,
                                    }, operation: 0,
                                };
                                let refused = physical.access(usize::MAX, usize::MAX, coordinate, ValueId(0), budget);
                                assert!(matches!(refused, Err(ProductionSourceOwnedViewErrorV18::Binding("missing physical access identity"))));
                            }
                            completed.set(true);
                            if fault == 2 { Err(PhysicalCallbackErrorV29::Hostile(error_drops.clone())) }
                            else { Ok(PhysicalRejectedOutputV29(output_drops.clone())) }
                        })
                    })
                }))
            })
        }));
        assert!(
            completed.get(),
            "genuine final physical scope and inner assertions must execute"
        );
        assert!(
            caught.is_ok(),
            "a rejected destructor must not replace the selected refusal"
        );
        let result = caught.unwrap();
        match (&result, fault) {
            (
                Err(PhysicalCallbackErrorV29::Source(ProductionSourceOwnedViewErrorV18::Resource(
                    ArgumentResourceV1::Accounting,
                ))),
                1,
            ) => {}
            (
                Err(PhysicalCallbackErrorV29::Source(ProductionSourceOwnedViewErrorV18::Binding(
                    "missing physical access identity",
                ))),
                0 | 2,
            ) => {}
            _ => panic!("selected original physical refusal required: {result:?}"),
        }
        assert_eq!(output_drops.get(), usize::from(fault != 2));
        assert_eq!(error_drops.get(), usize::from(fault == 2));
        if fault == 1 {
            assert_eq!(budget.storage(), 1);
        } else {
            assert_eq!(budget.storage(), MODULE_FLOOR);
        }
        drop(reset);
    }
}
