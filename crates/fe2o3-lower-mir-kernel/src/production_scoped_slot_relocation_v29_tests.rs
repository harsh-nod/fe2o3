use super::*;

mod initializer_relocation_tests {
    include!("production_scoped_slot_initializer_relocation_v29_tests.rs");
}

include!("production_source_partial_array_consuming_v29_tests.rs");
include!("production_source_index_fold_consuming_v29_tests.rs");
include!("production_source_mixed_memory_consuming_v29_tests.rs");

thread_local! {
    static EXPECTED: std::cell::Cell<(usize, u64, u32)> = const { std::cell::Cell::new((0, 0, 1)) };
    static ORIGINAL_RELOCATION_V29: std::cell::RefCell<Option<RelocationSnapshotV29>> = const { std::cell::RefCell::new(None) };
    static ORIGINAL_RELOCATION_CHECKED_V29: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

struct RelocationSnapshotV29 {
    originals: Vec<Function>,
    sidecars: Vec<(
        Vec<SemanticKirStatementOperationSpanV1>,
        Vec<SemanticKirTerminatorOperationSpanV1>,
        Vec<SemanticKirSyntheticOperationSpanV1>,
        Option<Vec<ScopedSlotOriginV29>>,
        Vec<PrivateArraySlotV1>,
        Vec<PrivateArrayEffectV1>,
        SemanticEmissionPlacementV1,
        PrivateArrayPayloadV1,
    )>,
    calls: Vec<Vec<SemanticKirCallReturnV1>>,
    pointers: Vec<ValueId>,
}

fn capture_relocation_v29(
    emitted: &[Option<LoweredFunctionResultV1>],
    slots: &OwnedScopedSourceSlotsV29,
) -> RelocationSnapshotV29 {
    let originals: Vec<_> = emitted
        .iter()
        .map(|row| row.as_ref().unwrap().function.clone())
        .collect();
    let sidecars: Vec<_> = emitted
        .iter()
        .map(|row| {
            let row = row.as_ref().unwrap();
            (
                row.statement_operation_spans.clone(),
                row.terminator_operation_spans.clone(),
                row.synthetic_operation_spans.clone(),
                row.scoped_slot_origins.clone(),
                row.private_arrays.slots.clone(),
                row.private_arrays.effects.clone(),
                row.private_arrays.placement,
                row.private_arrays.payload,
            )
        })
        .collect();
    let calls: Vec<_> = emitted
        .iter()
        .map(|row| row.as_ref().unwrap().call_returns.sites.rows.clone())
        .collect();
    RelocationSnapshotV29 { originals, sidecars, calls,
        pointers: slots.slots.iter().map(|slot| slot.origin.pointer).collect() }
}

fn check_relocation_v29(
    pending: &PendingScopedRootEmissionV29,
    instances: &ExecutionInstancesV29<'_>,
    snapshot: &RelocationSnapshotV29,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
        let RelocationSnapshotV29 { originals, sidecars, calls, pointers: original_pointers } = snapshot;
        let storage = pending.slot_relocation.as_ref().unwrap().storage();
        assert_eq!(
            (
                storage.allocations,
                storage.payload_bytes,
                storage.alignment
            ),
            EXPECTED.get()
        );
        let body = pending.function.body.as_ref().unwrap();
        let entry = &body.blocks[0];
        let mut pointers = BTreeSet::new();
        for block in &body.blocks {
            for operation in &block.operations {
                if matches!(operation.kind, OperationKind::Alloca { .. }) {
                    assert_eq!(block.id, entry.id);
                    assert!(pointers.insert(operation.results[0].id));
                }
            }
        }
        assert_eq!(pointers.len(), EXPECTED.get().0);
        for pointer in original_pointers {
            assert!(pointers.contains(pointer));
        }
        // Compare every original span's exact payload, omitting only its one
        // already-authenticated removed Call. Source coordinates never change.
        for row in &pending.coordinates.spans.rows {
            let original = row.source.coordinates().2;
            let function = &originals[row.instance.index()];
            let block = function
                .body
                .as_ref()
                .unwrap()
                .blocks
                .iter()
                .find(|block| block.id == original.block)
                .unwrap();
            let removed = row.removed_call.map(|call| {
                let witness = calls[row.instance.index()]
                    .iter()
                    .find(|witness| witness.semantic_block == call.block)
                    .unwrap();
                match witness.kind {
                    SemanticKirCallReturnKindV1::Call { call_operation, .. } => {
                        call_operation as usize
                    }
                    _ => panic!("removed call requires a Call witness"),
                }
            });
            let expected: Vec<_> = block
                .operations
                .iter()
                .enumerate()
                .skip(original.first as usize)
                .take(original.count as usize)
                .filter(|(index, _)| Some(*index) != removed)
                .map(|(_, operation)| operation)
                .collect();
            let actual: Vec<_> = row
                .segments
                .iter()
                .flatten()
                .flat_map(|span| {
                    let block = body
                        .blocks
                        .iter()
                        .find(|block| block.id == span.block)
                        .unwrap();
                    &block.operations[span.first as usize..span.end().unwrap() as usize]
                })
                .collect();
            assert_eq!(
                actual, expected,
                "instance {:?}, source {:?}",
                row.instance, row.source
            );
        }
        assert_eq!(pending.sidecars.rows.len(), sidecars.len());
        for (row, original) in pending.sidecars.rows.iter().zip(sidecars) {
            assert_eq!(row.statement_operation_spans, original.0);
            assert_eq!(row.terminator_operation_spans, original.1);
            assert_eq!(row.synthetic_operation_spans, original.2);
            assert_eq!(row.scoped_slot_origins, original.3);
            assert_eq!(row.private_arrays.slots, original.4);
            assert_eq!(row.private_arrays.effects, original.5);
            assert_eq!(row.private_arrays.placement, original.6);
            assert_eq!(row.private_arrays.payload.occupied, original.7.occupied);
            assert_eq!(row.private_arrays.payload.capacity, original.7.capacity);
        }
        replay_pending_instance_asserts_v1(pending, instances, budget)
}

fn inspect_relocation(
    _source: &ExecutionLifecycleSourceV29<'_>,
    instances: &ExecutionInstancesV29<'_>,
    emitted: &mut [Option<LoweredFunctionResultV1>],
    slots: &OwnedScopedSourceSlotsV29,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    OBSERVED.set(OBSERVED.get() + 1);
    let snapshot = capture_relocation_v29(emitted, slots);
    with_canonical_call_scratch_v1(budget, |budget| {
        let prepared = scoped_slot_relocation_v29::prepare(instances, emitted, slots, 1024, budget)?;
        let pending = prepared.assemble(ProductionSemanticKirLimitsV1::default(), budget)?;
        assert!(emitted.iter().all(Option::is_none));
        check_relocation_v29(&pending, instances, &snapshot, budget)
    })?;
    Err(unsupported(0, None, None, STOP))
}

fn capture_original_relocation_v29(
    _source: &ExecutionLifecycleSourceV29<'_>,
    _instances: &ExecutionInstancesV29<'_>,
    emitted: &mut [Option<LoweredFunctionResultV1>],
    slots: &OwnedScopedSourceSlotsV29,
    _budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    OBSERVED.set(OBSERVED.get() + 1);
    assert!(ORIGINAL_RELOCATION_V29.replace(Some(capture_relocation_v29(emitted, slots))).is_none());
    Ok(())
}

fn inspect_original_relocation_v29(
    pending: &mut PendingScopedRootEmissionV29,
    instances: &ExecutionInstancesV29<'_>,
    _plan: &SourceReferencePlanV29<'_, '_>,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    let snapshot = ORIGINAL_RELOCATION_V29.take().expect("this exact candidate's pre-relocation snapshot");
    check_relocation_v29(pending, instances, &snapshot, budget)?;
    ORIGINAL_RELOCATION_CHECKED_V29.set(ORIGINAL_RELOCATION_CHECKED_V29.get() + 1);
    Ok(())
}

fn original_relocation_owner_v29(fixture: ScopedFixture, branches: bool) -> ProductionSemanticSsaOwnerV1 {
    match fixture {
        ScopedFixture::Plain => lifecycle_owner(branches),
        ScopedFixture::RepeatedSlots => super::super::fixtures::repeated_slot_owner(),
        ScopedFixture::RootAssertionSlot => super::super::fixtures::root_assertion_slot_owner(),
        ScopedFixture::InitializationArray(whole) => initialized_literal_array_owner_v29(whole),
        ScopedFixture::Arrays => super::super::fixtures::array_owner(branches),
        ScopedFixture::AssertionSlots { move_condition, move_message, reinitialize } =>
            super::super::fixtures::assertion_slots_owner(move_condition, move_message, reinitialize),
        _ => panic!("exact relocation corpus fixture required"),
    }
}

#[test]
fn relocation_preserves_nested_repeated_array_and_assertion_payloads() {
    struct Restore(Option<RootExecutionArchiveObserverV29>);
    impl Drop for Restore {
        fn drop(&mut self) {
            ROOT_EXECUTION_ARCHIVE_OBSERVER_V29.set(self.0);
            ORIGINAL_RELOCATION_V29.take();
        }
    }
    let _restore = Restore(ROOT_EXECUTION_ARCHIVE_OBSERVER_V29.replace(Some(inspect_original_relocation_v29)));
    for (fixture, branches, expected) in [
        (ScopedFixture::Plain, false, (0, 0, 1)),
        (ScopedFixture::RepeatedSlots, false, (2, 8, 4)),
        (ScopedFixture::RootAssertionSlot, false, (3, 12, 4)),
        (ScopedFixture::InitializationArray(true), false, (2, 16, 4)),
        (ScopedFixture::InitializationArray(false), false, (2, 16, 4)),
        (ScopedFixture::Arrays, false, (2, 8, 4)),
        (ScopedFixture::Arrays, true, (2, 8, 4)),
        (
            ScopedFixture::AssertionSlots {
                move_condition: false,
                move_message: false,
                reinitialize: false,
            },
            false,
            (6, 18, 4),
        ),
    ] {
        EXPECTED.set(expected);
        ORIGINAL_RELOCATION_CHECKED_V29.set(0);
        assert!(ORIGINAL_RELOCATION_V29.take().is_none());
        let repeated = matches!(fixture, ScopedFixture::RepeatedSlots | ScopedFixture::RootAssertionSlot
            | ScopedFixture::InitializationArray(_) | ScopedFixture::AssertionSlots { .. });
        let (result, _, _, completed) = run_original_source_fixture_v29(
            || original_relocation_owner_v29(fixture, branches), branches, repeated,
            usize::from(expected.0 != 0), capture_original_relocation_v29,
            10_000_000,
            10_000_000,
        );
        assert!(result.is_ok(), "{fixture:?}/{branches}: {result:?}");
        assert!(completed, "the same relocated candidate must finish physical admission");
        assert_eq!(OBSERVED.get(), 3);
        assert_eq!(ORIGINAL_RELOCATION_CHECKED_V29.get(), 3);
        assert!(ORIGINAL_RELOCATION_V29.take().is_none());
    }
}

#[test]
fn relocation_obeys_exact_work_and_storage_limits() {
    for fixture in [ScopedFixture::Arrays, ScopedFixture::RepeatedSlots] {
        EXPECTED.set((2, 8, 4));
        let (result, work, peak) = run(false, fixture, inspect_relocation, 10_000_000, 10_000_000);
        assert!(is_stopped(&result), "{result:?}");
        assert!(is_stopped(
            &run(false, fixture, inspect_relocation, work, peak).0
        ));
        for (work, storage, work_failure) in [(work - 1, peak, true), (work, peak - 1, false)] {
            let error = run(false, fixture, inspect_relocation, work, storage).0;
            let error = error.as_ref().err().unwrap();
            match error {
                ProductionSemanticKirErrorV1::AssertOrigin(
                    SemanticKirAssertOriginErrorV1::Resource(ArgumentResourceV1::Work(_)),
                ) => assert!(work_failure),
                ProductionSemanticKirErrorV1::AssertOrigin(
                    SemanticKirAssertOriginErrorV1::Resource(ArgumentResourceV1::Storage(_)),
                ) => assert!(!work_failure),
                _ => assert_resource(error, work_failure),
            }
        }
    }
}

fn inspect_original_literal_array_v29(
    _source: &ExecutionLifecycleSourceV29<'_>,
    _instances: &ExecutionInstancesV29<'_>,
    _emitted: &mut [Option<LoweredFunctionResultV1>],
    slots: &OwnedScopedSourceSlotsV29,
    _budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    assert_eq!(slots.slots.len(), 2, "both genuine repeated helper arrays");
    for slot in &slots.slots {
        assert!(matches!(slot.origin.source, ScopedAllocationSourceV29::OriginalArray { .. }));
        let scalar = slot.scalar_array()?;
        assert_eq!((scalar.length, scalar.bytes, scalar.element.size, scalar.element.alignment), (2, 8, 4, 4));
    }
    OBSERVED.set(OBSERVED.get() + 1);
    Ok(())
}

fn initialized_literal_array_owner_v29(whole: bool) -> ProductionSemanticSsaOwnerV1 {
    let original = super::super::fixtures::initialization_array_owner(whole);
    if whole {
        return original;
    }
    // The shared false fixture intentionally reads untouched element 1. This
    // separate positive reads element 0, the only element its source writes.
    let semantic = original.source_semantic();
    let mut functions = semantic.functions().to_vec();
    let helper = &functions[3];
    let SemanticStatementKindV1::Assign(write) = helper.blocks()[0].statements()[0].kind() else {
        panic!("original single-element initializer");
    };
    let SemanticStatementKindV1::Assign(read) = helper.blocks()[1].statements()[0].kind() else {
        panic!("original array read");
    };
    let SemanticRvalueKindV1::Load(load) = read.value().kind() else {
        panic!("original scalar array load");
    };
    assert!(matches!(write.destination().projections()[0].kind(),
        SemanticProjectionKindV1::ConstantIndex { offset: 0, minimum_length: 2, from_end: false }));
    assert!(matches!(load.source().projections()[0].kind(),
        SemanticProjectionKindV1::ConstantIndex { offset: 1, minimum_length: 2, from_end: false }));
    assert_eq!(helper.blocks()[0].statements().len(), 1);
    let mut blocks = helper.blocks().to_vec();
    blocks[1] = block(141, vec![assign(read.destination().clone(),
        SemanticRvalueKindV1::Load(fe2o3_mir_model::semantic_mir_v1::SemanticMemoryLoadV1::new(
            write.destination().clone(), load.volatility(), load.atomic(),
        )))], SemanticTerminatorKindV1::Return);
    functions[3] = function(130, helper.role(), helper.abi().clone(), helper.locals().to_vec(), blocks);
    super::super::fixtures::build(semantic.types().to_vec(), functions, semantic.callables().to_vec())
}

#[test]
fn original_literal_arrays_finish_whole_and_element_history_with_exact_limits() {
    for whole in [true, false] {
        let run = |work, storage| run_original_repeated_source_v29(
            || initialized_literal_array_owner_v29(whole),
            inspect_original_literal_array_v29, work, storage,
        );
        let (positive, work, peak, completed) = run(10_000_000, 10_000_000);
        assert!(positive.is_ok() && completed, "whole={whole}: {positive:?}");
        assert_eq!(OBSERVED.get(), 3);
        let (exact, _, _, completed) = run(work, peak);
        assert!(exact.is_ok() && completed, "whole={whole}: {exact:?}");
        for (work_limit, storage_limit, work_failure) in [(work - 1, peak, true), (work, peak - 1, false)] {
            let (refused, _, _, completed) = run(work_limit, storage_limit);
            assert!(!completed);
            let error = original_repeated_source_resource_v29(refused.unwrap_err());
            assert!(matches!(error, ArgumentResourceV1::Work(_)) && work_failure
                || matches!(error, ArgumentResourceV1::Storage(_)) && !work_failure,
                "whole={whole}: {error:?}");
        }
    }
}

#[test]
fn original_literal_array_untouched_element_keeps_its_source_initialization_refusal() {
    let (positive, _, _, completed) = run_original_repeated_source_v29(
        || initialized_literal_array_owner_v29(false),
        inspect_original_literal_array_v29, 10_000_000, 10_000_000,
    );
    assert!(positive.is_ok() && completed, "{positive:?}");
    assert_eq!(OBSERVED.get(), 3);
    let (refused, _, _, completed) = run_original_repeated_source_v29(
        || super::super::fixtures::initialization_array_owner(false),
        inspect_original_literal_array_v29, 10_000_000, 10_000_000,
    );
    assert!(!completed);
    assert_eq!(OBSERVED.get(), 0, "original source refuses before physical admission");
    assert!(matches!(refused,
        Err(ProductionSourceOwnedViewErrorV18::Source(ProductionPendingScopedSourceErrorV29::Source(
            ProductionSemanticKirErrorV1::Unsupported {
                function: 0, block: None, statement: None,
                detail: "source reference reads an uninitialized partial holder",
            })))), "{refused:?}");
}

fn reject_root_reentry(
    _source: &ExecutionLifecycleSourceV29<'_>,
    instances: &ExecutionInstancesV29<'_>,
    emitted: &mut [Option<LoweredFunctionResultV1>],
    slots: &OwnedScopedSourceSlotsV29,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    OBSERVED.set(OBSERVED.get() + 1);
    assert!(slots.instances[0].slots.is_empty());
    scoped_slot_uses_v29::check_scoped_source_slot_uses_v29(
        instances, emitted, slots, 1024, budget,
    )?;
    let issuance = instances
        .calls(instances.root())
        .unwrap()
        .iter()
        .find(|call| call.occurrence().block.index() == 0)
        .unwrap();
    assert!(issuance.child().is_none());
    let lowered = emitted[0].as_mut().unwrap();
    let issuance_block = lowered
        .blocks
        .iter()
        .find(|row| row.semantic_block == issuance.occurrence().block)
        .unwrap()
        .kernel_ir_block;
    let body = lowered.function.body.as_mut().unwrap();
    let entry = body.blocks[0].id;
    let index = body
        .blocks
        .iter()
        .position(|block| block.id == issuance_block)
        .unwrap();
    // Preserve the source Return so the relocation guard remains the rejecting boundary.
    let Some(Terminator::Branch { target, .. }) = body.blocks[index].terminator.as_mut() else {
        panic!("context issuance has a normal edge")
    };
    let original = std::mem::replace(target, entry);
    scoped_slot_uses_v29::check_scoped_source_slot_uses_v29(
        instances, emitted, slots, 1024, budget,
    )?;
    let floor = budget.storage();
    let result = scoped_slot_relocation_v29::prepare(instances, emitted, slots, 1024, budget);
    assert!(matches!(
        result,
        Err(ProductionSemanticKirErrorV1::Unsupported {
            detail: "scoped allocation relocation is incomplete or mismatched",
            ..
        })
    ));
    drop(result);
    assert_eq!(budget.storage(), floor);
    assert!(emitted.iter().all(Option::is_some));
    let terminator = emitted[0]
        .as_mut()
        .unwrap()
        .function
        .body
        .as_mut()
        .unwrap()
        .blocks[index]
        .terminator
        .as_mut()
        .unwrap();
    let Terminator::Branch { target, .. } = terminator else {
        unreachable!()
    };
    *target = original;
    scoped_slot_uses_v29::check_scoped_source_slot_uses_v29(
        instances, emitted, slots, 1024, budget,
    )?;
    Err(unsupported(0, None, None, STOP))
}

#[test]
fn relocation_rejects_new_storage_in_a_slot_free_reentered_root() {
    let (result, _, _) = run(
        false,
        ScopedFixture::RepeatedSlots,
        reject_root_reentry,
        10_000_000,
        10_000_000,
    );
    assert!(is_stopped(&result), "{result:?}");
    assert_eq!(OBSERVED.get(), 1);
}

fn reject_foreign_ledger(
    _source: &ExecutionLifecycleSourceV29<'_>,
    instances: &ExecutionInstancesV29<'_>,
    emitted: &mut [Option<LoweredFunctionResultV1>],
    slots: &OwnedScopedSourceSlotsV29,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    OBSERVED.set(OBSERVED.get() + 1);
    with_canonical_call_scratch_v1(budget, |budget| {
        let prepared =
            scoped_slot_relocation_v29::prepare(instances, emitted, slots, 1024, budget)?;
        let charged = budget.storage();
        let mut work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(10_000_000);
        let mut foreign = ArgumentBudgetV1::new(&mut work, 10_000_000);
        let result = prepared.assemble(ProductionSemanticKirLimitsV1::default(), &mut foreign);
        assert!(matches!(
            result,
            Err(
                ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                    ArgumentResourceV1::Accounting
                )
            )
        ));
        drop(result);
        assert_eq!(foreign.work(), 0);
        assert_eq!(foreign.storage(), 0);
        assert_eq!(budget.storage(), charged);
        assert!(emitted.iter().all(Option::is_some));
        Ok(())
    })?;
    Err(unsupported(0, None, None, STOP))
}

fn reject_refunded_preparation(
    _source: &ExecutionLifecycleSourceV29<'_>,
    instances: &ExecutionInstancesV29<'_>,
    emitted: &mut [Option<LoweredFunctionResultV1>],
    slots: &OwnedScopedSourceSlotsV29,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    OBSERVED.set(OBSERVED.get() + 1);
    with_canonical_call_scratch_v1(budget, |budget| {
        let floor = budget.storage();
        let prepared =
            scoped_slot_relocation_v29::prepare(instances, emitted, slots, 1024, budget)?;
        assert!(budget.storage() > floor);
        budget.release_storage(1)?;
        let charged = budget.storage();
        let work = budget.work();
        let result = prepared.assemble(ProductionSemanticKirLimitsV1::default(), budget);
        assert!(matches!(
            result,
            Err(
                ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                    ArgumentResourceV1::Accounting
                )
            )
        ));
        drop(result);
        assert_eq!(budget.storage(), charged);
        assert_eq!(budget.work(), work);
        assert!(emitted.iter().all(Option::is_some));
        Ok(())
    })?;
    Err(unsupported(0, None, None, STOP))
}

#[test]
fn relocation_preparation_cannot_cross_ledgers_or_use_refunded_storage() {
    for fixture in [ScopedFixture::Plain, ScopedFixture::Arrays] {
        for observer in [
            reject_foreign_ledger as ScopedSlotObserverV29,
            reject_refunded_preparation,
        ] {
            let (result, _, _) = run(false, fixture, observer, 10_000_000, 10_000_000);
            assert!(is_stopped(&result), "{fixture:?}: {result:?}");
            assert_eq!(OBSERVED.get(), 1);
        }
    }
}
