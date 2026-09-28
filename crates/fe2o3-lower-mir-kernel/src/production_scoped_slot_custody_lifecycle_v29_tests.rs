use super::*;

thread_local! {
    static CASE: std::cell::Cell<(bool, Option<u8>)> = const { std::cell::Cell::new((false, None)) };
    static COMPLETED: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    static EXPECTED: std::cell::Cell<Option<ArgumentResourceV1>> = const { std::cell::Cell::new(None) };
}

struct Restore(Option<ScopedSlotCustodyObserverV29>);
impl Drop for Restore {
    fn drop(&mut self) {
        SCOPED_SLOT_CUSTODY_OBSERVER_V29.set(self.0);
    }
}

fn observe(
    instances: &ExecutionInstancesV29<'_>,
    emitted: &[Option<LoweredFunctionResultV1>],
    slots: &mut OwnedScopedSourceSlotsV29,
    references: Option<&SourceReferenceEmissionV29<'_, '_>>,
    identities: Option<&ExecutionIdentityPlanV1<'_, '_>>,
    max_elements: usize,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    assert!(
        !COMPLETED.get(),
        "one original root must reach the observer"
    );
    let references = references.expect("the original cell source owns its actual reference claims");
    let plan = references.plan;
    assert!(!plan.cells.rows.is_empty());
    assert!(!slots.slots.is_empty());
    assert_eq!(slots.instances.len(), instances.instances().len());
    assert!(
        emitted
            .iter()
            .flatten()
            .any(
                |lowered| lowered.function.body.as_ref().is_some_and(|body| body
                    .blocks
                    .iter()
                    .flat_map(|block| &block.operations)
                    .any(|operation| matches!(operation.kind, OperationKind::Alloca { .. })))
            )
    );
    let (pending, fault) = CASE.get();
    let check = |slots: &OwnedScopedSourceSlotsV29, budget: &mut ArgumentBudgetV1<'_>| {
        if pending {
            scoped_slot_uses_v29::check_pending_raw_source_inputs_v29(
                instances,
                emitted,
                slots,
                max_elements,
                references,
                identities,
                budget,
            )
        } else {
            scoped_slot_uses_v29::check_scoped_source_slot_uses_with_identities_v1(
                instances,
                emitted,
                slots,
                max_elements,
                Some(references),
                identities,
                budget,
            )
        }
    };
    let floor = budget.storage();
    check(slots, budget)?;
    plan.check_owner(instances, budget)?;
    references.check(budget)?;
    assert_eq!(plan.failure.get(), None);
    assert_eq!(budget.storage(), floor);
    let Some(fault) = fault else {
        COMPLETED.set(true);
        return Ok(());
    };
    let mut foreign_work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
    let mut foreign = ArgumentBudgetV1::new(&mut foreign_work, usize::MAX);
    foreign.reserve_storage(43)?;
    let first = if fault == 1 {
        budget.charge_work(usize::MAX - budget.work())?;
        let error = plan
            .charge(1, budget)
            .expect_err("actual original-ledger work denial required");
        let ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(first) = error else {
            panic!("original source work denial must be retained");
        };
        assert!(matches!(first, ArgumentResourceV1::Work(_)));
        first
    } else {
        ArgumentResourceV1::Accounting
    };
    EXPECTED.set(Some(first));
    if fault != 2 {
        slots.ledger = foreign.work_ledger_identity_v1();
    }
    let before = (budget.work(), budget.storage());
    let foreign_before = (foreign.work(), foreign.storage());
    let refused = if fault == 2 {
        check(slots, &mut foreign)
    } else {
        check(slots, budget)
    };
    assert!(matches!(refused,
        Err(ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(error)) if error == first));
    assert_eq!((budget.work(), budget.storage()), before);
    assert_eq!((foreign.work(), foreign.storage()), foreign_before);
    assert_eq!(plan.failure.get(), Some(first));
    assert!(matches!(references.check(budget),
        Err(ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(error)) if error == first));
    assert_eq!((budget.work(), budget.storage()), before);
    // Set only after every assertion: enclosing scoped panic conversion and
    // resource postflight must never satisfy this negative oracle by accident.
    COMPLETED.set(true);
    Ok(()) // Swallowing the refusal must still fail whole-root postflight.
}

#[test]
fn source_slot_entries_reject_foreign_custody_before_debits_and_keep_first_failure() {
    let _restore = Restore(SCOPED_SLOT_CUSTODY_OBSERVER_V29.replace(Some(observe)));
    for pending in [false, true] {
        for fault in [None, Some(0), Some(1), Some(2)] {
            CASE.set((pending, fault));
            COMPLETED.set(false);
            EXPECTED.set(None);
            let result = run_suffix(SuffixCase::Cells, capture, usize::MAX, LIMIT).0;
            assert!(
                COMPLETED.get(),
                "pending={pending}, fault={fault:?}: {result:?}"
            );
            if fault.is_none() {
                result.unwrap();
                assert_eq!(REACHED.get(), 1);
            } else {
                assert!(
                    matches!(result,
                    Err(ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(error))
                    if Some(error) == EXPECTED.get()),
                    "pending={pending}, fault={fault:?}: {result:?}"
                );
            }
        }
    }
}
