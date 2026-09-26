use super::*;

#[test]
fn source_alias_callee_and_original_disposition_mutations_are_not_transportable() {
    let owner = cpc_prepare(cpc_shared_source());
    let original = snapshots(&owner);
    for fault in 0..9 {
        let mut work = Work::new(1 << 48);
        let mut budget = Budget::new(&mut work, S);
        let floor = owner.retained_storage_floor_v1() + SIBLING;
        budget.reserve_storage(floor).unwrap();
        let error =
            canonical_assertion_v1::read_test_private_call_state_v1(&owner, &mut budget, fault)
                .err()
                .expect("unauthenticated component state must refuse");
        assert!(
            matches!(error, HistoryError::Invalid(_)),
            "fault {fault}: {error:?}"
        );
        assert_eq!(budget.storage(), floor);
        assert_eq!(snapshots(&owner), original);
    }
    cpc_run(&owner, |view, budget| cpc_reports(&owner, view, budget)).unwrap();
}

#[test]
fn byte_identical_foreign_original_cannot_replace_exact_source_custody() {
    let owner = cpc_prepare(cpc_memory_source(false, false, false, 11));
    let donor = cpc_prepare(cpc_memory_source(false, false, false, 11));
    assert_eq!(snapshots(&owner), snapshots(&donor));
    assert!(!std::ptr::eq(
        owner.original_source().executable(),
        donor.original_source().executable()
    ));
    let mut work = Work::new(1 << 48);
    let mut budget = Budget::new(&mut work, S);
    let floor = owner.retained_storage_floor_v1() + donor.retained_storage_floor_v1() + SIBLING;
    budget.reserve_storage(floor).unwrap();
    assert!(matches!(
        canonical_assertion_v1::read_test_private_call_foreign_source_v1(
            &owner,
            &donor,
            &mut budget,
        ),
        Err(HistoryError::InputCustody)
    ));
    assert_eq!(budget.storage(), floor);
    for inventory_foreign in [false, true] {
        assert!(matches!(
            canonical_assertion_v1::read_test_private_call_final_subject_v1(
                &owner,
                &donor,
                inventory_foreign,
                &mut budget,
            ),
            Err(HistoryError::InputCustody)
        ));
        assert_eq!(budget.storage(), floor);
    }
    cpc_run(&owner, |view, budget| cpc_reports(&owner, view, budget)).unwrap();
}

#[test]
fn ordered_store_pointer_and_value_occurrences_cannot_be_interchanged() {
    let owner = cpc_prepare(cpc_memory_source(true, true, true, 11));
    let mut work = Work::new(1 << 48);
    let mut budget = Budget::new(&mut work, S);
    let floor = owner.retained_storage_floor_v1() + SIBLING;
    budget.reserve_storage(floor).unwrap();
    assert!(matches!(
        canonical_assertion_v1::read_test_private_call_slot_v1(&owner, &mut budget),
        Err(HistoryError::Invalid(
            "ordered private/call operand occurrence"
        ))
    ));
    assert_eq!(budget.storage(), floor);
}

#[test]
fn bad_occurrence_query_is_sticky_but_a_fresh_full_consumption_recovers() {
    let owner = cpc_prepare(cpc_memory_source(true, true, true, 11));
    let result = cpc_run(&owner, |view, budget| {
        assert!(view.operation(usize::MAX, budget).is_err());
        assert!(view.memory_census(budget).is_err());
        Ok(())
    });
    assert!(matches!(
        result,
        Err(HistoryError::Source(
            ProductionCanonicalRankedSourceErrorV1::Invalid("private operation ordinal")
        ))
    ));
    cpc_run(&owner, |view, budget| cpc_reports(&owner, view, budget)).unwrap();
}

#[test]
fn foreign_and_moved_budget_queries_do_not_debit_substitutes() {
    let owner = cpc_prepare(cpc_memory_source(false, false, false, 11));
    for moved in [false, true] {
        let mut work = Work::new(1 << 48);
        let mut foreign_work = Work::new(1 << 48);
        let mut budget = Budget::new(&mut work, S);
        let mut foreign = Budget::new(&mut foreign_work, S);
        let floor = owner.retained_storage_floor_v1() + SIBLING;
        budget.reserve_storage(floor).unwrap();
        foreign.reserve_storage(SIBLING).unwrap();
        let result = owner.with_private_call_policy_checks_v1(&mut budget, |view, budget| {
            if moved {
                std::mem::swap(budget, &mut foreign);
                let before = foreign.work();
                assert!(view.calls(&mut foreign).is_err());
                assert_eq!(foreign.work(), before);
                std::mem::swap(budget, &mut foreign);
            } else {
                assert!(view.calls(&mut foreign).is_err());
            }
            Ok(())
        });
        assert!(matches!(
            result,
            Err(HistoryError::Source(
                ProductionCanonicalRankedSourceErrorV1::Resource(ArgumentResourceV1::Accounting)
            ))
        ));
        assert_eq!(budget.storage(), floor);
        assert_eq!((foreign.storage(), foreign.work()), (SIBLING, 0));
    }
}

#[test]
fn restored_floor_does_not_clear_a_query_undercut() {
    let owner = cpc_prepare(cpc_memory_source(false, true, false, 11));
    let result = cpc_run(&owner, |view, budget| {
        let floor = budget.storage();
        budget.release_storage(1)?;
        assert!(view.operation_count(budget).is_err());
        budget.reserve_storage(1)?;
        assert_eq!(budget.storage(), floor);
        assert!(view.original_metadata(budget).is_err());
        Ok(())
    });
    assert!(matches!(
        result,
        Err(HistoryError::Source(
            ProductionCanonicalRankedSourceErrorV1::Resource(ArgumentResourceV1::Accounting)
        ))
    ));
    cpc_run(&owner, |view, budget| cpc_reports(&owner, view, budget)).unwrap();
}

#[test]
fn permanent_ledger_substitution_never_refunds_the_substitute() {
    let owner = cpc_prepare(cpc_memory_source(false, true, false, 11));
    let mut work = Work::new(1 << 48);
    let mut foreign_work = Work::new(1 << 48);
    let mut budget = Budget::new(&mut work, S);
    let mut foreign = Budget::new(&mut foreign_work, S);
    let floor = owner.retained_storage_floor_v1() + SIBLING;
    budget.reserve_storage(floor).unwrap();
    foreign.reserve_storage(SIBLING).unwrap();
    let original = budget.work_ledger_identity_v1();
    assert!(
        owner
            .with_private_call_policy_checks_v1(&mut budget, |_, budget| {
                std::mem::swap(budget, &mut foreign);
                Ok(())
            })
            .is_err()
    );
    assert_eq!((budget.storage(), budget.work()), (SIBLING, 0));
    assert!(foreign.work_ledger_identity_v1() == original);
    assert!(foreign.storage() > floor);
    // The rejected borrowed view and all scoped backing have actually dropped.
    foreign.release_storage(foreign.storage() - floor).unwrap();
    assert_eq!(foreign.storage(), floor);
}

struct NestedPayload {
    ordinal: usize,
    last: usize,
    drops: std::sync::Arc<std::sync::Mutex<Vec<usize>>>,
}
impl Drop for NestedPayload {
    fn drop(&mut self) {
        self.drops.lock().unwrap().push(self.ordinal);
        if self.ordinal < self.last {
            std::panic::panic_any(Self {
                ordinal: self.ordinal + 1,
                last: self.last,
                drops: self.drops.clone(),
            });
        }
    }
}

#[test]
fn real_final_callback_drains_recursive_payloads_and_rejected_values_then_recovers() {
    let owner = cpc_prepare(cpc_memory_source(false, false, false, 11));
    for rejected_value in [false, true] {
        let drops = std::sync::Arc::new(std::sync::Mutex::new(Vec::with_capacity(3)));
        let result = cpc_run(&owner, |view, budget| -> HistoryResult<NestedPayload> {
            let payload = NestedPayload {
                ordinal: 0,
                last: 2,
                drops: drops.clone(),
            };
            if rejected_value {
                assert!(view.operation(usize::MAX, budget).is_err());
                Ok(payload)
            } else {
                std::panic::panic_any(payload)
            }
        });
        if rejected_value {
            assert!(matches!(
                result,
                Err(HistoryError::Source(
                    ProductionCanonicalRankedSourceErrorV1::Invalid("private operation ordinal")
                ))
            ));
        } else {
            assert!(matches!(result, Err(HistoryError::Assertion(Af::Panicked))));
        }
        assert_eq!(*drops.lock().unwrap(), [0, 1, 2]);
        cpc_run(&owner, |view, budget| cpc_reports(&owner, view, budget)).unwrap();
    }
}

#[test]
fn original_plus_history_floor_is_checked_before_any_consumer_scope_scratch() {
    let owner = cpc_prepare(cpc_memory_source(false, true, false, 11));
    for zero_work in [false, true] {
        let mut work = Work::new(if zero_work { 0 } else { 1 });
        {
            let floor = owner.retained_storage_floor_v1() - usize::from(!zero_work);
            let mut budget = Budget::new(&mut work, S);
            budget.reserve_storage(floor).unwrap();
            assert!(
                owner
                    .with_private_call_policy_checks_v1(&mut budget, |_, _| -> HistoryResult<()> {
                        panic!("denied entry reached callback")
                    })
                    .is_err()
            );
            assert_eq!(budget.work(), usize::from(!zero_work));
            assert_eq!((budget.storage(), budget.peak_storage()), (floor, floor));
            assert_eq!(budget.failed_storage(), None);
        }
        assert_eq!(work.failed_work(), zero_work.then_some(1));
    }
}

#[test]
fn consuming_constructor_first_three_work_cuts_are_independent_of_success_totals() {
    // Upfront owner authentication +1; existing protected entry +2; factory +3.
    for (limit, accepted, denied) in [(0, 0, 1), (2, 1, 3), (5, 3, 6)] {
        let source = cpc_memory_source(false, true, false, 11);
        let floor = source.unit_local_source_storage_floor_v1().unwrap() + SIBLING;
        let mut work = Work::new(limit);
        {
            let mut budget = Budget::new(&mut work, S);
            budget.reserve_storage(floor).unwrap();
            assert!(
                ProductionCanonicalScalarFixedPointOwnerV1::try_prepare_with_private_calls_v1(
                    source,
                    &mut budget
                )
                .is_err()
            );
            assert_eq!(
                (budget.work(), budget.storage(), budget.peak_storage()),
                (accepted, floor, floor)
            );
            assert_eq!(budget.failed_storage(), None);
        }
        assert_eq!(work.failed_work(), Some(denied));
    }
}

#[test]
fn constructor_owner_header_one_short_precedes_source_admission_and_optimizer() {
    let source = cpc_memory_source(false, true, false, 11);
    let floor = source.unit_local_source_storage_floor_v1().unwrap() + SIBLING;
    let requested = floor + std::mem::size_of::<ProductionCanonicalScalarFixedPointOwnerV1>();
    let mut work = Work::new(6);
    let mut budget = Budget::new(&mut work, requested - 1);
    budget.reserve_storage(floor).unwrap();
    assert!(
        ProductionCanonicalScalarFixedPointOwnerV1::try_prepare_with_private_calls_v1(
            source,
            &mut budget
        )
        .is_err()
    );
    assert_eq!(
        (budget.work(), budget.storage(), budget.peak_storage()),
        (6, floor, floor)
    );
    assert_eq!(budget.failed_storage(), Some(requested));
}
