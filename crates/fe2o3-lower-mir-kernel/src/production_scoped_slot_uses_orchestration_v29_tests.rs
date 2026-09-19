use super::*;
use crate::production_semantic_kir_v1::scoped_slot_uses_v29;

fn stop_after_uses(
    instances: &ExecutionInstancesV29<'_>,
    emitted: &mut [Option<LoweredFunctionResultV1>],
    receipt: &OwnedScopedSourceSlotsV29,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    check_receipt(instances, emitted, receipt, budget);
    let floor = budget.storage();
    scoped_slot_uses_v29::check_scoped_source_slot_uses_v29(
        instances, emitted, receipt, 1024, budget,
    )?;
    assert_eq!(budget.storage(), floor);
    Err(unsupported(0, None, None, STOP))
}

#[test]
fn real_source_scalar_array_diamond_and_loop_slot_uses_pass() {
    for fixture in [
        ScopedFixture::Arrays,
        ScopedFixture::RepeatedSlots,
        ScopedFixture::Initialization(InitializationFixtureV29::default()),
        ScopedFixture::Initialization(InitializationFixtureV29 {
            looping: true,
            ..InitializationFixtureV29::default()
        }),
        ScopedFixture::InitializationArray(true),
    ] {
        let (result, _, _) = run(false, fixture, stop_after_uses, 10_000_000, 10_000_000);
        assert!(is_stopped(&result), "{fixture:?}: {result:?}");
        assert_eq!(OBSERVED.get(), 1);
    }
}

fn inject_observation(
    instances: &ExecutionInstancesV29<'_>,
    emitted: &mut [Option<LoweredFunctionResultV1>],
    receipt: &OwnedScopedSourceSlotsV29,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    check_receipt(instances, emitted, receipt, budget);
    let slot = receipt.slots[0];
    let function = &mut emitted[slot.instance.index()].as_mut().unwrap().function;
    assert!(
        !function
            .body
            .as_ref()
            .unwrap()
            .blocks
            .iter()
            .flat_map(|block| &block.operations)
            .flat_map(|op| &op.results)
            .any(|result| result.id == ValueId(u32::MAX))
    );
    function.body.as_mut().unwrap().blocks[0]
        .operations
        .push(Operation::effect_free(
            ValueDef::new(ValueId(u32::MAX), Type::BOOL),
            OperationKind::Compare {
                predicate: ComparePredicate::Equal,
                lhs: slot.origin.pointer,
                rhs: slot.origin.pointer,
            },
        ));
    Ok(())
}

#[test]
fn production_pre_splice_hook_rejects_slot_identity_observation_after_capture() {
    let (result, _, _) = run(
        false,
        ScopedFixture::RepeatedSlots,
        inject_observation,
        10_000_000,
        10_000_000,
    );
    assert_eq!(OBSERVED.get(), 1);
    assert!(
        matches!(
            result,
            Err(ProductionSemanticKirErrorV1::Unsupported {
                function: 3,
                detail: "scoped source-slot address escapes through an unsupported operand",
                ..
            })
        ),
        "{result:?}"
    );
}

fn mutate_allocation(
    instances: &ExecutionInstancesV29<'_>,
    emitted: &mut [Option<LoweredFunctionResultV1>],
    receipt: &OwnedScopedSourceSlotsV29,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    check_receipt(instances, emitted, receipt, budget);
    let slot = receipt.slots[0];
    let body = emitted[slot.instance.index()]
        .as_mut()
        .unwrap()
        .function
        .body
        .as_mut()
        .unwrap();
    let operation =
        &mut body.blocks[slot.allocation.block_ordinal].operations[slot.allocation.operation];
    let OperationKind::Alloca { alignment, .. } = &mut operation.kind else {
        unreachable!()
    };
    *alignment = 0;
    Ok(())
}

#[test]
fn slot_use_check_revalidates_live_allocations_after_the_original_census() {
    let (result, _, _) = run(
        false,
        ScopedFixture::RepeatedSlots,
        mutate_allocation,
        10_000_000,
        10_000_000,
    );
    assert_eq!(OBSERVED.get(), 1);
    assert!(
        matches!(
            result,
            Err(ProductionSemanticKirErrorV1::Unsupported {
                detail: "scoped source-slot allocation census is incomplete or mismatched",
                ..
            })
        ),
        "{result:?}"
    );
}

fn check_foreign_ledger(
    instances: &ExecutionInstancesV29<'_>,
    emitted: &mut [Option<LoweredFunctionResultV1>],
    receipt: &OwnedScopedSourceSlotsV29,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    check_receipt(instances, emitted, receipt, budget);
    let floor = budget.storage();
    let mut work = CanonicalKernelIrWorkBudgetV1::new(10_000_000);
    let mut foreign = ArgumentBudgetV1::new(&mut work, 10_000_000);
    foreign.reserve_storage(43)?;
    assert!(matches!(
        scoped_slot_uses_v29::check_scoped_source_slot_uses_v29(
            instances,
            emitted,
            receipt,
            1024,
            &mut foreign
        ),
        Err(
            ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                ArgumentResourceV1::Accounting
            )
        )
    ));
    assert_eq!(foreign.storage(), 43);
    assert_eq!(foreign.work(), 0);
    assert_eq!(budget.storage(), floor);
    Err(unsupported(0, None, None, STOP))
}

#[test]
fn source_slot_use_checks_reject_foreign_ledgers_and_obey_full_orchestration_limits() {
    assert!(is_stopped(
        &run(
            false,
            ScopedFixture::RepeatedSlots,
            check_foreign_ledger,
            10_000_000,
            10_000_000
        )
        .0
    ));
    let (result, work, storage) = run(
        false,
        ScopedFixture::RepeatedSlots,
        stop_after_uses,
        10_000_000,
        10_000_000,
    );
    assert!(is_stopped(&result));
    assert!(is_stopped(
        &run(
            false,
            ScopedFixture::RepeatedSlots,
            stop_after_uses,
            work,
            storage
        )
        .0
    ));
    for (work, storage, work_failure) in [(work - 1, storage, true), (work, storage - 1, false)] {
        let result = run(
            false,
            ScopedFixture::RepeatedSlots,
            stop_after_uses,
            work,
            storage,
        )
        .0;
        assert_resource(result.as_ref().err().unwrap(), work_failure);
    }
}
