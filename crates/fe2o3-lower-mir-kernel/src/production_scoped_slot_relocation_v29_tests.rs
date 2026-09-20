use super::*;

thread_local! {
    static EXPECTED: std::cell::Cell<(usize, u64, u32)> = const { std::cell::Cell::new((0, 0, 1)) };
}

fn inspect_relocation(
    _source: &ExecutionLifecycleSourceV29<'_>,
    instances: &ExecutionInstancesV29<'_>,
    emitted: &mut [Option<LoweredFunctionResultV1>],
    slots: &OwnedScopedSourceSlotsV29,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    OBSERVED.set(OBSERVED.get() + 1);
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
    with_canonical_call_scratch_v1(budget, |budget| {
        let prepared =
            scoped_slot_relocation_v29::prepare(instances, emitted, slots, 1024, budget)?;
        let pending = prepared.assemble(ProductionSemanticKirLimitsV1::default(), budget)?;
        let storage = pending.slot_relocation.as_ref().unwrap().storage();
        assert_eq!(
            (
                storage.allocations,
                storage.payload_bytes,
                storage.alignment
            ),
            EXPECTED.get()
        );
        assert!(emitted.iter().all(Option::is_none));
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
        for slot in &slots.slots {
            assert!(pointers.contains(&slot.origin.pointer));
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
        for (row, original) in pending.sidecars.rows.iter().zip(&sidecars) {
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
        replay_pending_instance_asserts_v1(&pending, instances, budget)?;
        drop(pending);
        Ok(())
    })?;
    Err(unsupported(0, None, None, STOP))
}

#[test]
fn relocation_preserves_nested_repeated_array_and_assertion_payloads() {
    for (fixture, branches, expected) in [
        (ScopedFixture::Plain, false, (0, 0, 1)),
        (ScopedFixture::RepeatedSlots, false, (2, 8, 4)),
        (ScopedFixture::RootAssertionSlot, false, (3, 12, 4)),
        (ScopedFixture::InitializationArray(true), false, (2, 16, 4)),
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
        let (result, _, _) = run(
            branches,
            fixture,
            inspect_relocation,
            10_000_000,
            10_000_000,
        );
        assert!(is_stopped(&result), "{fixture:?}: {result:?}");
        assert_eq!(OBSERVED.get(), 1);
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
    let body = emitted[0].as_mut().unwrap().function.body.as_mut().unwrap();
    let entry = body.blocks[0].id;
    let index = body
        .blocks
        .iter()
        .position(|block| matches!(block.terminator, Some(Terminator::Return { .. })))
        .unwrap();
    let original = body.blocks[index].terminator.replace(Terminator::Branch {
        target: entry,
        arguments: Vec::new(),
    });
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
    emitted[0]
        .as_mut()
        .unwrap()
        .function
        .body
        .as_mut()
        .unwrap()
        .blocks[index]
        .terminator = original;
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
