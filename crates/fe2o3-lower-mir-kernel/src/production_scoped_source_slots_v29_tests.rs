use super::*;

mod slot_use_tests {
    include!("production_scoped_slot_uses_orchestration_v29_tests.rs");
}

mod initialization_tests {
    include!("production_scoped_initialization_v29_tests.rs");
}

const STOP: &str = "test stopped after scoped source-slot validation";
thread_local! {
    static OBSERVED: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

struct ObserverGuard(Option<ScopedSlotObserverV29>);
impl ObserverGuard {
    fn install(observer: ScopedSlotObserverV29) -> Self {
        Self(SCOPED_SLOT_OBSERVER_V29.replace(Some(observer)))
    }
}
impl Drop for ObserverGuard {
    fn drop(&mut self) {
        SCOPED_SLOT_OBSERVER_V29.set(self.0);
    }
}

fn check_receipt(
    instances: &ExecutionInstancesV29<'_>,
    emitted: &[Option<LoweredFunctionResultV1>],
    receipt: &OwnedScopedSourceSlotsV29,
    budget: &mut ArgumentBudgetV1<'_>,
) {
    OBSERVED.set(OBSERVED.get() + 1);
    assert!(receipt.ledger == budget.work_ledger_identity_v1());
    assert_eq!(
        receipt.source.semantic,
        *instances.owner().source_semantic_sha256()
    );
    assert!(receipt.source.ssa == instances.owner().identity());
    assert_eq!(receipt.source.root, ROOT);
    assert_eq!(receipt.instances.len(), instances.instances().len());
    let mut physical = BTreeSet::new();
    for (index, lowered) in emitted.iter().enumerate() {
        let lowered = lowered.as_ref().unwrap();
        let instance = instances.id_at(index).unwrap();
        let source = instances.instance(instance).unwrap();
        let summary = &receipt.instances[index];
        assert_eq!(summary.instance, instance);
        assert_eq!(summary.function, source.function());
        assert_eq!(
            summary.incoming,
            instances.incoming(instance).map(|call| call.occurrence())
        );
        assert_eq!(
            summary.placement,
            lowered.lifecycle_events.as_ref().unwrap().placement
        );
        let rows = &receipt.slots[summary.slots.clone()];
        assert_eq!(
            rows.len(),
            lowered.scoped_slot_origins.as_ref().unwrap().len()
        );
        let body = lowered.function.body.as_ref().unwrap();
        let mut actual_count = 0;
        for (block_ordinal, block) in body.blocks.iter().enumerate() {
            for (operation_index, operation) in block.operations.iter().enumerate() {
                let OperationKind::Alloca {
                    element,
                    count,
                    address_space,
                    alignment,
                } = &operation.kind
                else {
                    continue;
                };
                actual_count += 1;
                let matching: Vec<_> = rows
                    .iter()
                    .filter(|row| {
                        row.allocation
                            == PrivateArrayPhysicalLocationV1 {
                                block_ordinal,
                                block: block.id,
                                operation: operation_index,
                            }
                    })
                    .collect();
                assert_eq!(matching.len(), 1);
                let row = matching[0];
                assert_eq!(row.instance, instance);
                assert_eq!(*element, Type::Scalar(ScalarType::U32));
                assert_eq!(*address_space, AddressSpace::Private);
                assert_eq!(*alignment, 4);
                assert_eq!(
                    row.element,
                    PrivateRetainedSlotFactsV1 {
                        element: PrivateRetainedElementFactsV1::Scalar(ScalarType::U32),
                        size: 4,
                        alignment: 4,
                    }
                );
                assert_eq!(row.length, 1);
                assert_eq!(row.bytes, 4);
                assert_eq!(row.element_type, U32);
                assert_eq!(
                    row.origin.semantic_type,
                    source.declaration().locals()[row.origin.local as usize].ty()
                );
                assert_eq!(
                    operation.results,
                    vec![ValueDef::new(
                        row.origin.pointer,
                        Type::pointer(
                            Type::Scalar(ScalarType::U32),
                            AddressSpace::Private,
                            AccessMode::ReadWrite
                        )
                    )]
                );
                assert!(physical.insert(row.origin.pointer));
                match row.count {
                    Some((value, location)) => {
                        assert_eq!(*count, Some(value));
                        assert_eq!(location.block, block.id);
                        assert_eq!(location.block_ordinal, block_ordinal);
                        assert_eq!(location.operation + 1, operation_index);
                        assert_eq!(
                            block.operations[location.operation],
                            Operation::effect_free(
                                ValueDef::new(value, Type::Scalar(ScalarType::Index)),
                                OperationKind::Constant(Constant::Index(1)),
                            )
                        );
                        assert_eq!(operation_index, 1);
                        assert_eq!(
                            row.origin.local,
                            if source.function() == HELPER { 6 } else { 3 }
                        );
                    }
                    None => {
                        assert_eq!(*count, None);
                        assert_eq!(operation_index, 0);
                        assert_eq!(source.function(), SemanticFunctionIdV1::from_index(3));
                        assert_eq!(row.origin.local, 2);
                    }
                }
            }
        }
        assert_eq!(actual_count, rows.len());
    }
    assert_eq!(physical.len(), receipt.slots.len());
    if let [first, second] = receipt.slots.as_slice() {
        assert_ne!(first.instance, second.instance);
        assert_ne!(first.origin.pointer, second.origin.pointer);
        assert_ne!(first.allocation.block, second.allocation.block);
        if first.count.is_none() {
            assert_eq!(first.origin.local, second.origin.local);
            assert_eq!(
                receipt.instances[first.instance.index()].function,
                receipt.instances[second.instance.index()].function
            );
        }
    } else {
        assert!(receipt.slots.is_empty());
    }
}

fn stop_after_check(
    instances: &ExecutionInstancesV29<'_>,
    emitted: &mut [Option<LoweredFunctionResultV1>],
    receipt: &OwnedScopedSourceSlotsV29,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    check_receipt(instances, emitted, receipt, budget);
    Err(unsupported(0, None, None, STOP))
}

fn run(
    branches: bool,
    fixture: ScopedFixture,
    observer: ScopedSlotObserverV29,
    work: usize,
    storage: usize,
) -> (
    Result<Vec<DeferredLifecycleEventV29>, ProductionSemanticKirErrorV1>,
    usize,
    usize,
) {
    let _guard = ObserverGuard::install(observer);
    OBSERVED.set(0);
    run_lifecycle(
        branches,
        Fault::Orchestrated {
            groups: 2,
            limits: ProductionSemanticKirLimitsV1::default(),
            fixture,
        },
        work,
        storage,
    )
}

fn is_stopped<T>(result: &Result<T, ProductionSemanticKirErrorV1>) -> bool {
    matches!(
        result,
        Err(ProductionSemanticKirErrorV1::Unsupported { detail: STOP, .. })
    )
}

#[test]
fn source_slots_cover_arrays_and_repeated_scalar_instances_before_splicing() {
    for (fixture, branches) in [
        (ScopedFixture::Plain, false),
        (ScopedFixture::Repeated, false),
        (ScopedFixture::Arrays, false),
        (ScopedFixture::Arrays, true),
        (ScopedFixture::RepeatedSlots, false),
    ] {
        let (result, _, _) = run(branches, fixture, stop_after_check, 10_000_000, 10_000_000);
        assert!(is_stopped(&result), "{fixture:?}: {result:?}");
    }
}

fn reject_mutation(
    instances: &ExecutionInstancesV29<'_>,
    emitted: &mut [Option<LoweredFunctionResultV1>],
    index: usize,
    budget: &mut ArgumentBudgetV1<'_>,
    mutate: impl FnOnce(&mut LoweredFunctionResultV1),
) {
    let lowered = emitted[index].as_ref().unwrap();
    let function = lowered.function.clone();
    let origins = lowered.scoped_slot_origins.clone();
    let arrays = lowered.private_arrays.slots.clone();
    let spans = lowered.synthetic_operation_spans.clone();
    let source_instance = lowered.source_call_instance;
    let events = lowered.lifecycle_events.as_ref().unwrap();
    let source = events.source;
    let placement = events.placement;
    let floor = budget.storage();
    mutate(emitted[index].as_mut().unwrap());
    assert!(derive_scoped_source_slots_v29(instances, emitted, 1024, budget).is_err());
    assert_eq!(budget.storage(), floor);
    assert!(emitted.iter().all(Option::is_some));
    let lowered = emitted[index].as_mut().unwrap();
    lowered.function = function;
    lowered.scoped_slot_origins = origins;
    lowered.private_arrays.slots = arrays;
    lowered.synthetic_operation_spans = spans;
    lowered.source_call_instance = source_instance;
    let events = lowered.lifecycle_events.as_mut().unwrap();
    events.source = source;
    events.placement = placement;
}

fn mutation_check(
    instances: &ExecutionInstancesV29<'_>,
    emitted: &mut [Option<LoweredFunctionResultV1>],
    receipt: &OwnedScopedSourceSlotsV29,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    check_receipt(instances, emitted, receipt, budget);
    let floor = budget.storage();
    let mut foreign_work = CanonicalKernelIrWorkBudgetV1::new(10_000_000);
    let mut foreign = ArgumentBudgetV1::new(&mut foreign_work, 10_000_000);
    foreign.reserve_storage(29).unwrap();
    assert!(matches!(
        derive_scoped_source_slots_v29(instances, emitted, 1024, &mut foreign),
        Err(
            ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                ArgumentResourceV1::Accounting
            )
        )
    ));
    assert_eq!(foreign.storage(), 29);
    assert_eq!(foreign.work(), 0);
    let saved = emitted[0].take();
    assert!(derive_scoped_source_slots_v29(instances, emitted, 1024, budget).is_err());
    emitted[0] = saved;
    assert_eq!(budget.storage(), floor);
    let slot = receipt.slots[0];
    let index = slot.instance.index();
    reject_mutation(instances, emitted, index, budget, |row| {
        row.scoped_slot_origins.as_mut().unwrap().clear();
    });
    reject_mutation(instances, emitted, index, budget, |row| {
        let origins = row.scoped_slot_origins.as_mut().unwrap();
        origins.push(origins[0]);
    });
    reject_mutation(instances, emitted, index, budget, |row| {
        row.scoped_slot_origins.as_mut().unwrap()[0].local = 0;
    });
    reject_mutation(instances, emitted, index, budget, |row| {
        row.scoped_slot_origins.as_mut().unwrap()[0].semantic_type = UNIT;
    });
    reject_mutation(instances, emitted, index, budget, |row| {
        row.scoped_slot_origins.as_mut().unwrap()[0].pointer = ValueId(u32::MAX);
    });
    reject_mutation(instances, emitted, index, budget, |row| {
        row.source_call_instance = Some(instances.root());
    });
    reject_mutation(instances, emitted, index, budget, |row| {
        row.lifecycle_events.as_mut().unwrap().source.semantic[0] ^= 1;
    });
    reject_mutation(instances, emitted, index, budget, |row| {
        row.lifecycle_events.as_mut().unwrap().placement.first_block += 1;
    });
    reject_mutation(instances, emitted, index, budget, |row| {
        let block = &mut row.function.body.as_mut().unwrap().blocks[slot.allocation.block_ordinal];
        block.operations[slot.allocation.operation].results[0].id = ValueId(u32::MAX);
    });
    reject_mutation(instances, emitted, index, budget, |row| {
        let block = &mut row.function.body.as_mut().unwrap().blocks[slot.allocation.block_ordinal];
        let OperationKind::Alloca { alignment, .. } =
            &mut block.operations[slot.allocation.operation].kind
        else {
            unreachable!()
        };
        *alignment = 8;
    });
    reject_mutation(instances, emitted, index, budget, |row| {
        let body = row.function.body.as_mut().unwrap();
        let allocation = body.blocks[slot.allocation.block_ordinal].operations
            [slot.allocation.operation]
            .clone();
        body.blocks.last_mut().unwrap().operations.push(allocation);
    });
    reject_mutation(instances, emitted, index, budget, |row| {
        row.synthetic_operation_spans
            .iter_mut()
            .find(|span| span.rule == SemanticKirSyntheticOperationRuleV1::RetainedLocalStorage)
            .unwrap()
            .operation_count -= 1;
    });
    reject_mutation(instances, emitted, index, budget, |row| {
        row.synthetic_operation_spans
            .iter_mut()
            .find(|span| span.rule == SemanticKirSyntheticOperationRuleV1::RetainedLocalStorage)
            .unwrap()
            .rule = SemanticKirSyntheticOperationRuleV1::EnumPayloadStorage;
    });
    let other = receipt.slots[1];
    reject_mutation(instances, emitted, index, budget, |row| {
        row.scoped_slot_origins.as_mut().unwrap()[0] = other.origin;
    });
    if let Some((_, count)) = slot.count {
        reject_mutation(instances, emitted, index, budget, |row| {
            row.function.body.as_mut().unwrap().blocks[count.block_ordinal].operations
                [count.operation]
                .kind = OperationKind::Constant(Constant::Index(2));
        });
        reject_mutation(instances, emitted, index, budget, |row| {
            row.private_arrays.slots[0].count_location.block_ordinal += 1;
        });
        reject_mutation(instances, emitted, index, budget, |row| {
            row.private_arrays.slots[0].length = 2;
        });
        reject_mutation(instances, emitted, index, budget, |row| {
            row.private_arrays.slots.clear();
        });
        assert!(derive_scoped_source_slots_v29(instances, emitted, 0, budget).is_err());
        assert_eq!(budget.storage(), floor);
    }
    let replay = derive_scoped_source_slots_v29(instances, emitted, 1024, budget)?;
    assert_eq!(replay.slots, receipt.slots);
    assert_eq!(replay.instances, receipt.instances);
    let retained = replay.retained_storage;
    assert_eq!(budget.storage(), floor + retained);
    drop(replay);
    budget.release_storage(retained)?;
    assert_eq!(budget.storage(), floor);
    Err(unsupported(0, None, None, STOP))
}

#[test]
fn source_slots_reject_identity_layout_census_and_placed_operation_mutations() {
    for fixture in [ScopedFixture::Arrays, ScopedFixture::RepeatedSlots] {
        let (result, _, _) = run(false, fixture, mutation_check, 10_000_000, 10_000_000);
        assert!(is_stopped(&result), "{fixture:?}: {result:?}");
    }
}

#[test]
fn source_slot_creation_has_exact_work_and_peak_storage_boundaries() {
    for fixture in [
        ScopedFixture::Plain,
        ScopedFixture::Arrays,
        ScopedFixture::RepeatedSlots,
    ] {
        let (result, work, peak) = run(false, fixture, stop_after_check, 10_000_000, 10_000_000);
        assert!(is_stopped(&result), "{fixture:?}: {result:?}");
        assert!(is_stopped(
            &run(false, fixture, stop_after_check, work, peak).0
        ));
        for (work, storage, work_failure) in [(work - 1, peak, true), (work, peak - 1, false)] {
            let error = run(false, fixture, stop_after_check, work, storage).0;
            assert_resource(error.as_ref().err().unwrap(), work_failure);
        }
    }
}

#[test]
fn source_slot_receipt_does_not_authorize_frame_splicing() {
    fn observe(
        instances: &ExecutionInstancesV29<'_>,
        emitted: &mut [Option<LoweredFunctionResultV1>],
        receipt: &OwnedScopedSourceSlotsV29,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        check_receipt(instances, emitted, receipt, budget);
        assert_eq!(receipt.slots.len(), 2);
        for instance in &receipt.instances {
            if instance.slots.is_empty() {
                continue;
            }
            let lowered = emitted[instance.instance.index()].as_ref().unwrap();
            let body = lowered.function.body.as_ref().unwrap();
            let operation = &body.blocks[0].operations
                [receipt.slots[instance.slots.start].allocation.operation];
            assert!(matches!(
                call_splice_check_callee_operation_v1(&operation.kind),
                Err(CallInstanceEmissionErrorV1::CalleeFrameAllocation)
            ));
        }
        Ok(())
    }
    for fixture in [ScopedFixture::Arrays, ScopedFixture::RepeatedSlots] {
        let (result, _, _) = run(false, fixture, observe, 10_000_000, 10_000_000);
        assert_eq!(
            OBSERVED.get(),
            1,
            "frame refusal must occur after slot validation"
        );
        assert!(matches!(
            result,
            Err(ProductionSemanticKirErrorV1::Unsupported {
                detail: "execution call parameters differ from their source instance",
                ..
            })
        ));
        assert!(!is_stopped(&result));
    }
}

#[test]
fn captured_scalar_origins_have_independent_exact_resource_limits() {
    let original: BTreeMap<_, _> = [3, 7]
        .into_iter()
        .map(|local| {
            (
                local,
                SemanticRetainedLocalSlotV1 {
                    pointer: ValueId(local + 10),
                    semantic_type: U32,
                    kernel_type: Type::Scalar(ScalarType::U32),
                    alignment: 4,
                    array: None,
                },
            )
        })
        .collect();
    let bytes = 2 * std::mem::size_of::<ScopedSlotOriginV29>();
    for (work_limit, capacity, success) in [
        (9, 37 + bytes, true),
        (8, 37 + bytes, false),
        (9, 36 + bytes, false),
    ] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(work_limit);
        let mut budget = ArgumentBudgetV1::new(&mut work, capacity);
        budget.reserve_storage(37).unwrap();
        match capture_scoped_slot_origins_v29(&original, &mut budget) {
            Ok(rows) => {
                assert!(success);
                assert_eq!(
                    rows.iter()
                        .map(|row| (row.local, row.pointer))
                        .collect::<Vec<_>>(),
                    vec![(3, ValueId(13)), (7, ValueId(17))]
                );
                let retained = rows.capacity() * std::mem::size_of::<ScopedSlotOriginV29>();
                drop(rows);
                budget.release_storage(retained).unwrap();
            }
            Err(error) => {
                assert!(!success);
                assert_resource(&error, work_limit == 8);
            }
        }
        assert_eq!(budget.storage(), 37);
    }
}

fn assert_resource(error: &ProductionSemanticKirErrorV1, work: bool) {
    assert!(
        matches!(
            (error, work),
            (
                ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                    ArgumentResourceV1::Work(_)
                ),
                true
            ) | (
                ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                    ArgumentResourceV1::Storage(_)
                ),
                false
            )
        ),
        "wrong resource refusal: {error:?}"
    );
}

#[test]
fn source_slot_rederivation_has_its_own_storage_and_work_boundaries() {
    fn observe(
        instances: &ExecutionInstancesV29<'_>,
        emitted: &mut [Option<LoweredFunctionResultV1>],
        receipt: &OwnedScopedSourceSlotsV29,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        check_receipt(instances, emitted, receipt, budget);
        let floor = budget.storage();
        let available = budget.storage_limit() - floor;
        let filler = available - 1_000_000;
        let old_peak = budget.peak_storage();
        budget.reserve_storage(filler)?;
        let entry = budget.storage();
        assert!(entry > old_peak);
        let work_before = budget.work();
        let replay = derive_scoped_source_slots_v29(instances, emitted, 1024, budget)?;
        let work = budget.work() - work_before;
        let peak = budget.peak_storage() - entry;
        let retained = replay.retained_storage;
        drop(replay);
        budget.release_storage(retained + filler)?;
        assert_eq!(budget.storage(), floor);
        for (allowance, success) in [(peak, true), (peak - 1, false)] {
            let filler = available - allowance;
            budget.reserve_storage(filler)?;
            let replay = derive_scoped_source_slots_v29(instances, emitted, 1024, budget);
            match replay {
                Ok(replay) => {
                    assert!(success);
                    assert_eq!(replay.slots, receipt.slots);
                    let retained = replay.retained_storage;
                    drop(replay);
                    budget.release_storage(retained)?;
                }
                Err(error) => {
                    assert!(!success);
                    assert_resource(&error, false);
                }
            }
            assert_eq!(budget.storage(), floor + filler);
            budget.release_storage(filler)?;
        }
        // Exhaust only this same ledger's remaining work, not a foreign meter.
        budget.charge_work(10_000_000 - budget.work() - (work - 1))?;
        let replay = derive_scoped_source_slots_v29(instances, emitted, 1024, budget);
        assert_resource(replay.err().as_ref().unwrap(), true);
        assert_eq!(budget.storage(), floor);
        assert!(emitted.iter().all(Option::is_some));
        Err(unsupported(0, None, None, STOP))
    }
    for fixture in [
        ScopedFixture::Plain,
        ScopedFixture::Arrays,
        ScopedFixture::RepeatedSlots,
    ] {
        let (result, _, _) = run(false, fixture, observe, 10_000_000, 10_000_000);
        assert!(is_stopped(&result), "{fixture:?}: {result:?}");
    }
}

#[test]
fn source_slot_attempt_restores_only_its_own_storage_on_error_and_panic() {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(100);
    let mut budget = ArgumentBudgetV1::new(&mut work, 1000);
    budget.reserve_storage(37).unwrap();
    for panic in [false, true] {
        let before = budget.work();
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            scoped_slot_attempt_v29::<()>(&mut budget, |budget| {
                let _rows = emission_vec_v1::<u8>(13, budget)?;
                if panic {
                    panic!("injected failure after slot reservation");
                }
                Err(scoped_slot_error_v29())
            })
        }));
        if panic {
            assert!(result.is_err());
        } else {
            assert!(result.unwrap().is_err());
        }
        assert_eq!(budget.storage(), 37);
        assert!(budget.work() > before);
    }
}
