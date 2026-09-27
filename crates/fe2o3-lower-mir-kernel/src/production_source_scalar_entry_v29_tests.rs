thread_local! {
    static SCALAR_ENTRY_FAULT_V29: std::cell::Cell<u8> = const { std::cell::Cell::new(0) };
    static SCALAR_ENTRY_VISITS_V29: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    static SCALAR_ENTRY_MUTATIONS_V29: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    static SCALAR_ENTRY_RESTART_V29: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    static SCALAR_ENTRY_QUERY_MODE_V29: std::cell::Cell<u8> = const { std::cell::Cell::new(0) };
    static SCALAR_ENTRY_QUERY_COMPLETED_V29: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

fn scalar_entry_original_owner_v29(case: SuffixCase) -> ProductionSemanticSsaOwnerV1 {
    let original = suffix_owner(case);
    let source = original.source_semantic();
    let mut types = source.types().to_vec();
    let raw = reference(&mut types, U32, SemanticMutabilityV1::Immutable, true);
    let mut functions = source.functions().to_vec();
    // A genuine original address forces typed backing for the incoming scalar.
    // Both helper arguments remain distinct, and their unchanged stores read
    // their own original incoming values before the later helper call returns.
    for index in [0usize, 3] {
        let original_function = &functions[index];
        let mut locals = original_function.locals().to_vec();
        let mut before = Vec::new();
        for argument in if index == 0 {
            &[1u32][..]
        } else {
            &[1u32, 2][..]
        } {
            let pointer = locals.len() as u32;
            locals.push(local(
                180 + pointer as u8,
                raw,
                SemanticLocalRoleV1::Temporary,
            ));
            before.push(assign(
                place(pointer, raw),
                SemanticRvalueKindV1::AddressOf {
                    place: place(*argument, U32),
                    mutability: SemanticMutabilityV1::Immutable,
                },
            ));
        }
        let mut blocks = original_function.blocks().to_vec();
        if index == 3 && SCALAR_ENTRY_RESTART_V29.get() {
            for kind in [
                SemanticStatementKindV1::StorageDead(SemanticLocalIdV1::from_index(1)),
                SemanticStatementKindV1::StorageLive(SemanticLocalIdV1::from_index(1)),
            ] {
                before.push(SemanticStatementV1::new(
                    SemanticSourceProvenanceV1::unavailable(),
                    kind,
                ));
            }
            before.push(assign(
                place(1, U32),
                SemanticRvalueKindV1::Use(literal(71)),
            ));
        }
        before.extend_from_slice(blocks[0].statements());
        blocks[0] = SemanticBasicBlockV1::new(
            blocks[0].identity(),
            blocks[0].source(),
            before,
            blocks[0].terminator().clone(),
        )
        .unwrap();
        // Keep the admitted declaration roster and its callable coordinates.
        // Only the original body/local changes above belong to this fixture.
        let mut rebuilt = SemanticFunctionDeclV1::new(
            original_function.identity(),
            original_function.role(),
            original_function.item_definition_identity(),
            original_function.monomorphization_identity(),
            original_function.generic_type_arguments_identity(),
            original_function.const_generic_arguments_identity(),
            original_function.source(),
            original_function.abi().clone(),
            locals,
            original_function.entry(),
            blocks,
        )
        .unwrap();
        if let Some(kernel) = original_function.kernel_entry() {
            rebuilt = rebuilt.with_kernel_entry(kernel.clone());
        }
        functions[index] = rebuilt;
    }
    assert!(
        functions
            .windows(2)
            .all(|pair| { pair[0].identity().as_bytes() < pair[1].identity().as_bytes() })
    );
    for (original, rebuilt) in source.functions().iter().zip(&functions) {
        assert_eq!(original.identity(), rebuilt.identity());
        assert_eq!(original.abi(), rebuilt.abi());
        assert_eq!(original.entry(), rebuilt.entry());
        assert_eq!(original.kernel_entry(), rebuilt.kernel_entry());
        assert!(
            rebuilt
                .locals()
                .windows(2)
                .all(|pair| { pair[0].identity().as_bytes() < pair[1].identity().as_bytes() })
        );
        assert_eq!(
            &rebuilt.locals()[..original.locals().len()],
            original.locals()
        );
        assert_eq!(original.blocks().len(), rebuilt.blocks().len());
        assert!(
            rebuilt
                .blocks()
                .windows(2)
                .all(|pair| { pair[0].identity().as_bytes() < pair[1].identity().as_bytes() })
        );
        for (before, after) in original.blocks().iter().zip(rebuilt.blocks()) {
            assert_eq!(before.identity(), after.identity());
            assert_eq!(before.source(), after.source());
            assert_eq!(before.terminator(), after.terminator());
        }
    }
    scoped_root_tests::fixtures::build(types, functions, source.callables().to_vec())
}

fn inspect_scalar_entries_v29(
    _: &ExecutionLifecycleSourceV29<'_>,
    instances: &ExecutionInstancesV29<'_>,
    emitted: &mut [Option<LoweredFunctionResultV1>],
    slots: &OwnedScopedSourceSlotsV29,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    assert_eq!(
        slots.retained_storage,
        slots.instances.capacity() * std::mem::size_of::<ScopedSourceSlotInstanceV29>()
            + slots.slots.capacity() * std::mem::size_of::<ScopedSourceSlotV29>(),
        "entry ABI and source-join scratch must not be retained in the slot inventory"
    );
    let fault = SCALAR_ENTRY_FAULT_V29.get();
    let foreign_parameter = emitted
        .iter()
        .enumerate()
        .filter_map(|(index, row)| {
            let instance = instances.id_at(index)?;
            (instances.instance(instance)?.function().index() == 3).then(|| {
                row.as_ref()
                    .unwrap()
                    .function
                    .body
                    .as_ref()
                    .unwrap()
                    .parameters[0]
            })
        })
        .last()
        .unwrap();
    let mut root_count = 0;
    let mut helpers = 0;
    for owner in &slots.instances {
        let lowered = emitted[owner.instance.index()].as_mut().unwrap();
        let function = instances
            .instance(owner.instance)
            .unwrap()
            .function()
            .index();
        let body = lowered.function.body.as_ref().unwrap();
        let mut entries = Vec::new();
        visit_scoped_slot_initializers_v29(
            instances,
            owner.instance,
            body.blocks[0].id,
            &slots.slots[owner.slots.clone()],
            budget,
            |_, slot, location, _| {
                if let ScopedAllocationIdentityV29::OriginalObject { local, generation } =
                    slot.origin.identity
                {
                    assert_eq!(generation, 0);
                    let operation = &body.blocks[0].operations[location.operation];
                    let OperationKind::Storage(ScopedObjectOperationV29::WriteValue {
                        address,
                        value,
                        access,
                    }) = operation.kind
                    else {
                        panic!("typed entry initializer");
                    };
                    assert_eq!(address, slot.origin.pointer);
                    assert!(operation.results.is_empty());
                    assert_eq!(access, MemoryAccess::new(AddressSpace::Private, 4));
                    assert!(location.operation > slot.allocation.operation);
                    let parameter = if function == 0 {
                        body.parameters[0]
                    } else {
                        assert_eq!(function, 3);
                        assert_eq!(body.parameters.len(), 2);
                        body.parameters[local as usize - 1]
                    };
                    assert_eq!(value, parameter);
                    entries.push((local, location));
                }
                Ok(())
            },
        )?;
        if function == 0 {
            assert_eq!(entries.len(), 1);
            root_count += 1;
        } else if function == 3 {
            assert_eq!(entries.len(), 2);
            helpers += 1;
            if SCALAR_ENTRY_RESTART_V29.get() {
                assert!(
                    lowered
                        .scoped_memory_anchors
                        .as_ref()
                        .unwrap()
                        .objects
                        .iter()
                        .any(
                            |payload| matches!(payload.role, ScopedObjectRoleV29::WriteValue {
                        destination: ScopedObjectEndpointV29 {
                            object: ScopedObjectIdentityV29::Local { local, generation, .. },
                            source: ScopedObjectSourceV29::Place { .. }, ..
                        }, ..
                    } if local.index() == 1 && generation != 0)
                        ),
                    "the restarted value has its own original logical generation"
                );
            }
            if fault == 9 {
                let (local, location) = entries[0];
                let allocation = slots.slots[owner.slots.clone()]
                    .iter()
                    .find(|slot| {
                        slot.origin.identity
                            == ScopedAllocationIdentityV29::OriginalObject {
                                local,
                                generation: 0,
                            }
                    })
                    .unwrap()
                    .allocation;
                assert_eq!(allocation.block, location.block);
                assert!(allocation.operation < location.operation);
                lowered.function.body.as_mut().unwrap().blocks[0]
                    .operations
                    .swap(allocation.operation, location.operation);
                // Keep both operation/source coordinate pairs together. The
                // immutable original initializer schedule still forbids this
                // write before its allocation and invocation-local suffix.
                for row in &mut lowered.scoped_memory_anchors.as_mut().unwrap().rows {
                    if row.block == location.block {
                        if row.position == location.operation {
                            row.position = allocation.operation;
                        } else if row.position == allocation.operation {
                            row.position = location.operation;
                        }
                    }
                }
                SCALAR_ENTRY_MUTATIONS_V29.set(SCALAR_ENTRY_MUTATIONS_V29.get() + 1);
            } else if (1..=5).contains(&fault) || fault == 8 {
                let (local, location) = entries[0];
                let anchors = lowered.scoped_memory_anchors.as_mut().unwrap();
                let anchor = anchors
                    .rows
                    .iter()
                    .position(|row| {
                        row.block == location.block && row.position == location.operation
                    })
                    .unwrap();
                let ScopedMemoryAnchorKindV29::Object(object) = anchors.rows[anchor].kind else {
                    panic!("typed entry source row");
                };
                let payload = &mut anchors.objects[object];
                let ScopedObjectRoleV29::WriteValue {
                    destination,
                    value: original,
                } = &mut payload.role
                else {
                    panic!("entry WriteValue role");
                };
                let body = lowered.function.body.as_mut().unwrap();
                let operation = &mut body.blocks[0].operations[location.operation];
                match fault {
                    1 => {
                        let replacement = body.parameters[1];
                        let OperationKind::Storage(ScopedObjectOperationV29::WriteValue {
                            value,
                            ..
                        }) = &mut operation.kind
                        else {
                            unreachable!()
                        };
                        *value = replacement;
                        let ScopedObjectOperationV29::WriteValue { value, .. } =
                            &mut payload.operation
                        else {
                            unreachable!()
                        };
                        *value = replacement;
                    }
                    2 => {
                        let ScopedObjectIdentityV29::Local { generation, .. } =
                            &mut destination.object
                        else {
                            unreachable!()
                        };
                        *generation = 1;
                    }
                    3 => destination.root_schema = fe2o3_kernel_ir::StorageLayoutIdV1(u32::MAX),
                    4 => {
                        *original = ScopedObjectValueOriginV29::Original(
                            ScopedMemoryStoreSourceV29::EntryArgument {
                                local: SemanticLocalIdV1::from_index(local + 1),
                                ty: U32,
                            },
                        )
                    }
                    5 => {
                        let OperationKind::Storage(ScopedObjectOperationV29::WriteValue {
                            access,
                            ..
                        }) = &mut operation.kind
                        else {
                            unreachable!()
                        };
                        access.volatile = true;
                        let ScopedObjectOperationV29::WriteValue { access, .. } =
                            &mut payload.operation
                        else {
                            unreachable!()
                        };
                        access.volatile = true;
                    }
                    8 => {
                        if body.parameters[0] == foreign_parameter {
                            continue;
                        }
                        let OperationKind::Storage(ScopedObjectOperationV29::WriteValue {
                            value,
                            ..
                        }) = &mut operation.kind
                        else {
                            unreachable!()
                        };
                        *value = foreign_parameter;
                        let ScopedObjectOperationV29::WriteValue { value, .. } =
                            &mut payload.operation
                        else {
                            unreachable!()
                        };
                        *value = foreign_parameter;
                    }
                    _ => panic!("scalar entry fault"),
                }
                SCALAR_ENTRY_MUTATIONS_V29.set(SCALAR_ENTRY_MUTATIONS_V29.get() + 1);
            }
        } else {
            assert!(entries.is_empty());
        }
    }
    assert_eq!(root_count, 1);
    assert!(helpers >= 1);
    SCALAR_ENTRY_VISITS_V29.set(SCALAR_ENTRY_VISITS_V29.get() + 1);
    Ok(())
}

fn scalar_entry_census_fault_v29(
    source_index: &SourceAddressSourceIndexV29<'_>,
    sources: &mut Vec<SourceAddressAccessSourceV29>,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    let mut selected = None;
    for (index, source) in sources.iter().enumerate() {
        let sidecar = source_index.sidecar(source.instance, budget)?;
        let anchors = sidecar.scoped_memory_anchors.as_ref().unwrap();
        if let Some((endpoint, _)) =
            source_address_object_payload_v29(anchors, &anchors.rows[source.anchor], budget)?
        {
            if matches!(endpoint.source, ScopedObjectSourceV29::EntryArgument { .. }) {
                selected = Some(index);
                break;
            }
        }
    }
    let selected = selected.expect("genuine entry source row");
    match SCALAR_ENTRY_FAULT_V29.get() {
        6 => {
            sources.remove(selected);
        }
        7 => {
            sources.insert(selected, sources[selected]);
        }
        _ => panic!("entry source census fault"),
    }
    SCALAR_ENTRY_MUTATIONS_V29.set(SCALAR_ENTRY_MUTATIONS_V29.get() + 1);
    Ok(())
}

fn run_scalar_entries_v29(
    case: SuffixCase,
    fault: u8,
    work: usize,
    storage: usize,
) -> (SourceOwnedResultV18<()>, usize, usize, bool) {
    struct Restore(u8, Option<SourceObjectEffectCensusObserverV29>);
    impl Drop for Restore {
        fn drop(&mut self) {
            SCALAR_ENTRY_FAULT_V29.set(self.0);
            SOURCE_OBJECT_EFFECT_CENSUS_OBSERVER_V29.set(self.1);
        }
    }
    let _restore = Restore(
        SCALAR_ENTRY_FAULT_V29.replace(fault),
        SOURCE_OBJECT_EFFECT_CENSUS_OBSERVER_V29
            .replace(matches!(fault, 6 | 7).then_some(scalar_entry_census_fault_v29)),
    );
    SCALAR_ENTRY_VISITS_V29.set(0);
    SCALAR_ENTRY_MUTATIONS_V29.set(0);
    run_original_source_fixture_v29(
        || scalar_entry_original_owner_v29(case),
        false,
        true,
        3,
        inspect_scalar_entries_v29,
        work,
        storage,
    )
}

#[test]
fn original_scalar_entry_materialization_reaches_complete_root_and_repeated_helper_memory() {
    for case in [
        SuffixCase::Root,
        SuffixCase::Loop,
        SuffixCase::Branch,
        SuffixCase::CyclicEntry,
    ] {
        let (result, _, _, completed) = run_scalar_entries_v29(case, 0, LIMIT, LIMIT);
        assert!(result.is_ok(), "{case:?}: {result:?}");
        assert!(completed);
        assert_eq!(SCALAR_ENTRY_VISITS_V29.get(), 3);
        assert_eq!(SCALAR_ENTRY_MUTATIONS_V29.get(), 0);
    }
}

#[test]
fn original_scalar_entry_same_candidate_rejects_wrong_parameter_generation_schema_role_and_access()
{
    for fault in 1..=8 {
        let (positive, _, _, completed) = run_scalar_entries_v29(SuffixCase::Root, 0, LIMIT, LIMIT);
        positive.unwrap();
        assert!(completed);
        let (refused, _, _, completed) =
            run_scalar_entries_v29(SuffixCase::Root, fault, LIMIT, LIMIT);
        assert!(!completed);
        assert!(SCALAR_ENTRY_MUTATIONS_V29.get() >= 1);
        assert_eq!(SCALAR_ENTRY_VISITS_V29.get(), 1);
        assert!(
            matches!(
                refused,
                Err(ProductionSourceOwnedViewErrorV18::Source(
                    ProductionPendingScopedSourceErrorV29::Source(
                        ProductionSemanticKirErrorV1::Unsupported {
                            detail: "typed object source payload differs from its actual operation",
                            ..
                        }
                    )
                ))
            ),
            "fault {fault}: {refused:?}"
        );
    }
}

#[test]
fn original_scalar_entry_same_candidate_rejects_initializer_moved_into_allocation_prefix() {
    let (positive, _, _, completed) = run_scalar_entries_v29(SuffixCase::Root, 0, LIMIT, LIMIT);
    positive.unwrap();
    assert!(completed);
    let (refused, _, _, completed) = run_scalar_entries_v29(SuffixCase::Root, 9, LIMIT, LIMIT);
    assert!(!completed);
    assert_eq!(SCALAR_ENTRY_VISITS_V29.get(), 1);
    assert!(SCALAR_ENTRY_MUTATIONS_V29.get() >= 1);
    assert!(
        matches!(
            refused,
            Err(ProductionSourceOwnedViewErrorV18::Source(
                ProductionPendingScopedSourceErrorV29::Source(
                    ProductionSemanticKirErrorV1::Unsupported {
                        detail: "typed allocation identity or representation requires its exact source contract",
                        ..
                    }
                )
            ))
        ),
        "{refused:?}"
    );
}

#[test]
fn original_scalar_entry_complete_transaction_exact_and_one_short_resources() {
    let (result, work, storage, completed) =
        run_scalar_entries_v29(SuffixCase::Root, 0, LIMIT, LIMIT);
    result.unwrap();
    assert!(completed);
    let exact = run_scalar_entries_v29(SuffixCase::Root, 0, work, storage);
    exact.0.unwrap();
    assert!(exact.3);
    assert_eq!((exact.1, exact.2), (work, storage));
    for (work_limit, storage_limit, is_work) in
        [(work - 1, storage, true), (work, storage - 1, false)]
    {
        let short = run_scalar_entries_v29(SuffixCase::Root, 0, work_limit, storage_limit);
        assert!(!short.3);
        match (
            is_work,
            original_repeated_source_resource_v29(short.0.unwrap_err()),
        ) {
            (true, ArgumentResourceV1::Work(error)) => {
                assert_eq!(error.limit(), work_limit);
                assert!(error.actual() > work_limit);
            }
            (false, ArgumentResourceV1::Storage(error)) => {
                assert_eq!(error.limit(), storage_limit);
                assert!(error.actual() > storage_limit);
            }
            other => panic!("exact entry resource: {other:?}"),
        }
    }
}

#[test]
fn original_scalar_entry_restart_initializes_only_original_generation_zero() {
    struct Restore(bool);
    impl Drop for Restore {
        fn drop(&mut self) {
            SCALAR_ENTRY_RESTART_V29.set(self.0);
        }
    }
    let _restore = Restore(SCALAR_ENTRY_RESTART_V29.replace(true));
    let (result, _, _, completed) = run_scalar_entries_v29(SuffixCase::Root, 0, LIMIT, LIMIT);
    result.unwrap();
    assert!(completed);
    assert_eq!(SCALAR_ENTRY_VISITS_V29.get(), 3);
}

fn inspect_scalar_entry_query_v29(
    _: &ExecutionInstancesV29<'_>,
    emitted: &[Option<LoweredFunctionResultV1>],
    _: &mut OwnedScopedSourceSlotsV29,
    references: Option<&SourceReferenceEmissionV29<'_, '_>>,
    _: Option<&ExecutionIdentityPlanV1<'_, '_>>,
    _: usize,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    let plan = references.unwrap().plan;
    let (instance, endpoint) = emitted
        .iter()
        .flatten()
        .find_map(|lowered| {
            let anchors = lowered.scoped_memory_anchors.as_ref()?;
            anchors.objects.iter().find_map(|row| {
                let ScopedObjectRoleV29::WriteValue { destination, .. } = row.role else {
                    return None;
                };
                matches!(
                    destination.source,
                    ScopedObjectSourceV29::EntryArgument { .. }
                )
                .then_some((lowered.source_call_instance.unwrap(), destination))
            })
        })
        .unwrap();
    let ScopedObjectSourceV29::EntryArgument { local } = endpoint.source else {
        unreachable!()
    };
    let headers = std::mem::size_of::<SemanticLocalIdV1>()
        + 2 * std::mem::size_of::<Result<SemanticLocalIdV1, ProductionSemanticKirErrorV1>>();
    match SCALAR_ENTRY_QUERY_MODE_V29.get() {
        0 => {
            let before = (budget.work(), budget.storage());
            with_canonical_call_scratch_v1(budget, |budget| {
                let floor = budget.storage();
                assert_eq!(
                    source_object_entry_local_v29(plan, instance, endpoint, budget)?,
                    local
                );
                assert_eq!(budget.storage() - floor, headers);
                Ok(())
            })?;
            // Three existing five-work owner guards, a 24-work source join,
            // and the two-work unit scratch boundary.
            assert_eq!((budget.work() - before.0, budget.storage()), (41, before.1));
            SCALAR_ENTRY_QUERY_COMPLETED_V29.set(true);
            Ok(())
        }
        mode => {
            let first = match mode {
                1 => {
                    let before = (budget.work(), budget.storage());
                    let mut work = CanonicalKernelIrWorkBudgetV1::new(0);
                    let mut foreign = ArgumentBudgetV1::new(&mut work, 53);
                    foreign.reserve_storage(53)?;
                    let refused =
                        source_object_entry_local_v29(plan, instance, endpoint, &mut foreign);
                    assert!(matches!(
                        refused,
                        Err(
                            ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                                ArgumentResourceV1::Accounting
                            )
                        )
                    ));
                    assert_eq!((foreign.work(), foreign.storage()), (0, 53));
                    assert_eq!((budget.work(), budget.storage()), before);
                    refused
                }
                2 => {
                    budget.charge_work(LIMIT - budget.work() - 38)?;
                    let refused = source_object_entry_local_v29(plan, instance, endpoint, budget);
                    assert!(
                        matches!(refused, Err(ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                        ArgumentResourceV1::Work(error))) if error.limit() == LIMIT && error.actual() > LIMIT)
                    );
                    refused
                }
                3 => {
                    let padding = LIMIT - budget.storage() - (headers - 1);
                    budget.reserve_storage(padding)?;
                    let refused = source_object_entry_local_v29(plan, instance, endpoint, budget);
                    assert!(
                        matches!(refused, Err(ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                        ArgumentResourceV1::Storage(error))) if error.limit() == LIMIT && error.actual() > LIMIT)
                    );
                    budget.release_storage(padding)?;
                    budget.charge_work(LIMIT - budget.work())?;
                    refused
                }
                _ => panic!("entry query mode"),
            };
            let before = (budget.work(), budget.storage());
            let replay = source_object_entry_local_v29(plan, instance, endpoint, budget);
            assert_eq!(format!("{first:?}"), format!("{replay:?}"));
            assert_eq!((budget.work(), budget.storage()), before);
            SCALAR_ENTRY_QUERY_COMPLETED_V29.set(true);
            first.map(|_| ())
        }
    }
}

#[test]
fn original_scalar_entry_query_has_independent_headers_work_custody_and_first_resource_failure() {
    struct Restore(Option<ScopedSlotCustodyObserverV29>, u8);
    impl Drop for Restore {
        fn drop(&mut self) {
            SCOPED_SLOT_CUSTODY_OBSERVER_V29.set(self.0);
            SCALAR_ENTRY_QUERY_MODE_V29.set(self.1);
        }
    }
    let _restore = Restore(
        SCOPED_SLOT_CUSTODY_OBSERVER_V29.replace(Some(inspect_scalar_entry_query_v29)),
        SCALAR_ENTRY_QUERY_MODE_V29.get(),
    );
    for mode in 0..=3 {
        SCALAR_ENTRY_QUERY_MODE_V29.set(mode);
        SCALAR_ENTRY_QUERY_COMPLETED_V29.set(false);
        let (result, _, _, completed) = run_scalar_entries_v29(SuffixCase::Root, 0, LIMIT, LIMIT);
        assert!(
            SCALAR_ENTRY_QUERY_COMPLETED_V29.get(),
            "all inner assertions must complete"
        );
        if mode == 0 {
            result.unwrap();
            assert!(completed);
        } else {
            assert!(!completed);
            match (
                mode,
                original_repeated_source_resource_v29(result.unwrap_err()),
            ) {
                (1, ArgumentResourceV1::Accounting) => {}
                (2, ArgumentResourceV1::Work(error)) => assert_eq!(error.limit(), LIMIT),
                (3, ArgumentResourceV1::Storage(error)) => assert_eq!(error.limit(), LIMIT),
                other => panic!("entry query outer refusal: {other:?}"),
            }
        }
    }
}

thread_local! {
    static SCALAR_ENTRY_STORE_MODE_V29: std::cell::Cell<u8> = const { std::cell::Cell::new(0) };
    static SCALAR_ENTRY_STORE_VISITS_V29: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    static SCALAR_ENTRY_STORE_EXPECTED_V29: std::cell::Cell<Option<ArgumentResourceV1>> = const { std::cell::Cell::new(None) };
}

fn inspect_scalar_entry_store_scratch_v29(
    instances: &ExecutionInstancesV29<'_>,
    emitted: &[Option<LoweredFunctionResultV1>],
    slots: &mut OwnedScopedSourceSlotsV29,
    references: Option<&SourceReferenceEmissionV29<'_, '_>>,
    _: Option<&ExecutionIdentityPlanV1<'_, '_>>,
    _: usize,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    let plan = references.expect("genuine scalar entry source plan").plan;
    let mut selected = None;
    for owner in &slots.instances {
        let lowered = emitted[owner.instance.index()].as_ref().unwrap();
        let body = lowered.function.body.as_ref().unwrap();
        visit_scoped_slot_initializers_v29(
            instances,
            owner.instance,
            body.blocks[0].id,
            &slots.slots[owner.slots.clone()],
            budget,
            |index, slot, location, _| {
                if selected.is_none()
                    && matches!(
                        slot.origin.identity,
                        ScopedAllocationIdentityV29::OriginalObject { generation: 0, .. }
                    )
                {
                    selected = Some((owner.instance, owner.slots.start + index, location));
                }
                Ok(())
            },
        )?;
    }
    let (instance, index, location) = selected.expect("actual typed entry initializer required");
    let slot = &slots.slots[index];
    let lowered = emitted[instance.index()].as_ref().unwrap();
    let block = &lowered.function.body.as_ref().unwrap().blocks[location.block_ordinal];
    assert_eq!(block.id, location.block);
    let operation = &block.operations[location.operation];
    assert!(matches!(
        operation.kind,
        OperationKind::Storage(ScopedObjectOperationV29::WriteValue { .. })
    ));
    let check = |location, budget: &mut ArgumentBudgetV1<'_>| {
        source_reference_object_entry_store_v29(
            plan, instance, slot, lowered, location, operation, budget,
        )
    };
    let floor = budget.storage();
    let ledger = budget.work_ledger_identity_v1();
    assert!(floor > 0);
    assert_eq!(
        (budget.failed_work(), budget.failed_storage()),
        (None, None)
    );
    assert_eq!(plan.failure.get(), None);
    match SCALAR_ENTRY_STORE_MODE_V29.get() {
        mode @ (0 | 1) => {
            let before_work = budget.work();
            check(location, budget)?;
            let query_work = budget.work() - before_work;
            let peak = budget.peak_storage();
            assert!(query_work > 7);
            assert_eq!(budget.storage(), floor);
            let before_work = budget.work();
            check(location, budget)?;
            assert_eq!(budget.work() - before_work, query_work);
            assert_eq!((budget.storage(), budget.peak_storage()), (floor, peak));
            if mode == 1 {
                let before_work = budget.work();
                let refused = check(
                    PrivateArrayPhysicalLocationV1 {
                        operation: usize::MAX,
                        ..location
                    },
                    budget,
                );
                assert!(
                    matches!(
                        refused,
                        Err(ProductionSemanticKirErrorV1::Unsupported {
                            detail: "typed object source payload differs from its actual operation",
                            ..
                        })
                    ),
                    "the copied impossible location must reach the entry anchor refusal: {refused:?}"
                );
                assert!(budget.work() - before_work > 7);
                assert_eq!((budget.storage(), budget.peak_storage()), (floor, peak));
                assert_eq!(plan.failure.get(), None);
                assert_eq!(
                    (budget.failed_work(), budget.failed_storage()),
                    (None, None)
                );
                let before_work = budget.work();
                check(location, budget)?;
                assert_eq!(budget.work() - before_work, query_work);
                assert_eq!((budget.storage(), budget.peak_storage()), (floor, peak));
            }
            assert!(budget.work_ledger_identity_v1() == ledger);
            assert_eq!(plan.failure.get(), None);
            assert_eq!(
                (budget.failed_work(), budget.failed_storage()),
                (None, None)
            );
        }
        mode @ (2 | 3) => {
            let first = if mode == 2 {
                let limit = LIMIT;
                budget.charge_work(limit.checked_sub(budget.work() + 6).unwrap())?;
                let peak = budget.peak_storage();
                let refused = check(location, budget)
                    .expect_err("scope entry must exceed six remaining work units");
                let ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(first) = refused
                else {
                    panic!("scope entry must report resource failure: {refused:?}");
                };
                let ArgumentResourceV1::Work(error) = first else {
                    panic!("scope entry must report Work: {first:?}");
                };
                assert_eq!((error.actual(), error.limit()), (limit + 1, limit));
                assert_eq!(budget.work(), limit - 1);
                assert_eq!((budget.storage(), budget.peak_storage()), (floor, peak));
                assert_eq!(
                    (budget.failed_work(), budget.failed_storage()),
                    (Some(limit + 1), None)
                );
                first
            } else {
                let first_header = source_reference_emission_headers_v29::<()>()?;
                let second_header =
                    source_reference_emission_headers_v29::<Option<&ScopedMemoryAnchorV29>>()?;
                assert!(first_header > 0 && second_header > 0);
                let limit = budget.storage_limit();
                let padding = limit
                    .checked_sub(floor + first_header + second_header - 1)
                    .unwrap();
                budget.reserve_storage(padding)?;
                let padded_floor = budget.storage();
                let peak = budget.peak_storage();
                let refused = check(location, budget)
                    .expect_err("the second prepaid header must be one byte short");
                let ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(first) = refused
                else {
                    panic!("header denial must report resource failure: {refused:?}");
                };
                let ArgumentResourceV1::Storage(error) = first else {
                    panic!("header denial must report Storage: {first:?}");
                };
                assert_eq!((error.actual(), error.limit()), (limit + 1, limit));
                assert_eq!(budget.storage(), padded_floor);
                assert_eq!(budget.peak_storage(), peak.max(padded_floor + first_header));
                assert_eq!(
                    (budget.failed_work(), budget.failed_storage()),
                    (None, Some(limit + 1))
                );
                budget.release_storage(padding)?;
                assert_eq!(budget.storage(), floor);
                // Exhaust work only after the first Storage denial. Replay must
                // return that same error before attempting any new work debit.
                budget.charge_work(LIMIT - budget.work())?;
                assert_eq!(budget.failed_work(), None);
                first
            };
            assert_eq!(plan.failure.get(), Some(first));
            assert!(budget.work_ledger_identity_v1() == ledger);
            let before = (
                budget.work(),
                budget.storage(),
                budget.peak_storage(),
                budget.failed_work(),
                budget.failed_storage(),
            );
            let replay = check(location, budget);
            assert!(matches!(replay,
                Err(ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(error)) if error == first));
            assert_eq!(
                (
                    budget.work(),
                    budget.storage(),
                    budget.peak_storage(),
                    budget.failed_work(),
                    budget.failed_storage()
                ),
                before
            );
            assert_eq!(budget.storage(), floor);
            assert!(budget.work_ledger_identity_v1() == ledger);
            assert_eq!(plan.failure.get(), Some(first));
            SCALAR_ENTRY_STORE_EXPECTED_V29.set(Some(first));
        }
        _ => panic!("entry store scratch mode"),
    }
    // Only completed inner assertions count. Resource failures are swallowed
    // here so the genuine enclosing source owner must retain the first failure.
    SCALAR_ENTRY_STORE_VISITS_V29.set(SCALAR_ENTRY_STORE_VISITS_V29.get() + 1);
    Ok(())
}

fn run_scalar_entry_store_scratch_case_v29(mode: u8) {
    struct Restore(Option<ScopedSlotCustodyObserverV29>, u8);
    impl Drop for Restore {
        fn drop(&mut self) {
            SCOPED_SLOT_CUSTODY_OBSERVER_V29.set(self.0);
            SCALAR_ENTRY_STORE_MODE_V29.set(self.1);
        }
    }
    let _restore = Restore(
        SCOPED_SLOT_CUSTODY_OBSERVER_V29.replace(Some(inspect_scalar_entry_store_scratch_v29)),
        SCALAR_ENTRY_STORE_MODE_V29.get(),
    );
    let run = |mode| {
        SCALAR_ENTRY_STORE_MODE_V29.set(mode);
        SCALAR_ENTRY_STORE_VISITS_V29.set(0);
        SCALAR_ENTRY_STORE_EXPECTED_V29.set(None);
        let (result, _, _, completed) = run_scalar_entries_v29(SuffixCase::Root, 0, LIMIT, LIMIT);
        if mode < 2 {
            result.unwrap();
            assert!(completed);
            assert_eq!(SCALAR_ENTRY_STORE_VISITS_V29.get(), 3);
            assert_eq!(SCALAR_ENTRY_VISITS_V29.get(), 3);
            assert_eq!(SCALAR_ENTRY_STORE_EXPECTED_V29.get(), None);
        } else {
            assert!(!completed);
            assert_eq!(
                SCALAR_ENTRY_STORE_VISITS_V29.get(),
                1,
                "inner assertions must complete before outer refusal: {result:?}"
            );
            assert_eq!(SCALAR_ENTRY_VISITS_V29.get(), 1);
            let expected = SCALAR_ENTRY_STORE_EXPECTED_V29
                .get()
                .expect("actual first resource denial");
            assert_eq!(
                original_repeated_source_resource_v29(result.unwrap_err()),
                expected
            );
        }
    };
    run(0);
    if mode != 0 {
        run(mode);
    }
}

#[test]
fn original_scalar_entry_store_scratch_repeated_success_keeps_floor_and_peak() {
    run_scalar_entry_store_scratch_case_v29(0);
}

#[test]
fn original_scalar_entry_store_scratch_semantic_refusal_reclaims_headers_and_recovers() {
    run_scalar_entry_store_scratch_case_v29(1);
}

#[test]
fn original_scalar_entry_store_scratch_scope_entry_denial_keeps_first_work_error() {
    run_scalar_entry_store_scratch_case_v29(2);
}

#[test]
fn original_scalar_entry_store_scratch_partial_header_denial_keeps_first_storage_error() {
    run_scalar_entry_store_scratch_case_v29(3);
}

include!("production_source_entry_prologue_v29_tests.rs");
