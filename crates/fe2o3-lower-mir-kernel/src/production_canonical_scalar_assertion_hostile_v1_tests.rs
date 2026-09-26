use super::*;

fn refused(source: ProductionPreRankedKirOwnerV1) -> HistoryError {
    let mut work = Work::new(1 << 48);
    let mut budget = Budget::new(&mut work, S);
    let floor = source.unit_local_source_storage_floor_v1().unwrap() + SIBLING;
    budget.reserve_storage(floor).unwrap();
    let ledger = budget.work_ledger_identity_v1();
    let error = ProductionCanonicalScalarFixedPointOwnerV1::try_prepare_with_assertions_v1(
        source,
        &mut budget,
    )
    .err()
    .expect("source must refuse before preparing history");
    assert_eq!(budget.storage(), floor);
    assert!(budget.work_ledger_identity_v1() == ledger);
    error
}

#[test]
fn unknown_dynamic_predicate_cannot_become_assertion_history_success() {
    let error = refused(dynamic_source(true, true, false));
    assert!(matches!(
        error,
        HistoryError::Assertion(Af::NotProved {
            reason: fe2o3_mir_model::SemanticAssertionNotProvedV1::Unknown,
            ..
        })
    ));
}

#[test]
fn legacy_bounds_deferred_is_not_a_fresh_source_fact() {
    let seed = dynamic_source(true, true, false);
    let source = seed.semantic_ssa().source_semantic();
    let root = &source.functions()[0];
    let boolean = SemanticTypeIdV1::from_index(1);
    let word = SemanticTypeIdV1::from_index(2);
    let blocks = vec![
        body(
            221,
            root.blocks()[0].statements().to_vec(),
            assertion(
                local(2, boolean),
                true,
                SemanticAssertMessageV1::BoundsCheck {
                    length: operand(word, 17, 8),
                    index: local(1, word),
                },
                1,
            ),
        ),
        body(222, vec![], SemanticTerminatorKindV1::Return),
    ];
    let error = refused(rebuild(
        &seed,
        source.types().to_vec(),
        vec![copy_function(root, root.locals().to_vec(), blocks)],
    ));
    assert!(matches!(
        error,
        HistoryError::Assertion(Af::NotProved {
            reason: fe2o3_mir_model::SemanticAssertionNotProvedV1::BoundsNeedsIndependentRule,
            ..
        })
    ));
}

#[test]
fn genuinely_refuted_literal_remains_distinct_from_unknown() {
    let seed = literal(true, false);
    let source = seed.semantic_ssa().source_semantic();
    let root = &source.functions()[0];
    let blocks = vec![
        body(
            223,
            vec![],
            assertion(
                operand(SemanticTypeIdV1::from_index(1), 0, 1),
                true,
                SemanticAssertMessageV1::NullPointerDereference,
                1,
            ),
        ),
        body(224, vec![], SemanticTerminatorKindV1::Return),
    ];
    let error = refused(rebuild(
        &seed,
        source.types().to_vec(),
        vec![copy_function(root, root.locals().to_vec(), blocks)],
    ));
    assert!(matches!(error, HistoryError::Assertion(Af::Refuted { .. })));
}

#[test]
fn unmaterialized_source_assertion_is_not_silently_omitted() {
    let owner = super::super::hostile::unmaterialized_assertion_owner();
    assert_eq!(owner.assert_origins().source_site_count(), 1);
    let error = refused(owner);
    assert!(
        matches!(
            error,
            HistoryError::Assertion(Af::Binding {
                detail: "source assertion is outside the original materialization roster",
                ..
            })
        ),
        "{error:?}"
    );
}

#[test]
fn unreachable_only_source_assertion_orphan_trap_is_rejected_before_history() {
    let mut work = Work::new(1 << 48);
    let mut budget = Budget::new(&mut work, S);
    let owner = original_assertion_owner(Fixture::Unreachable, false, &mut budget);
    assert_eq!(owner.assert_origins().source_site_count(), 0);
    let error = refused(owner);
    assert!(
        matches!(
            error,
            HistoryError::Query(
                fe2o3_pliron::CanonicalRankedPolicyFailureV1::PrivateRequirement {
                    requirement: fe2o3_pliron::CanonicalPrivateRequirementV1::TerminalPairs,
                    ..
                }
            )
        ),
        "{error:?}"
    );
}

#[test]
fn scalar_increment_keeps_real_user_call_and_private_memory_profiles_closed() {
    let call = literal(true, true);
    assert!(
        call.executable()
            .module()
            .functions
            .iter()
            .filter(|f| f.body.is_some())
            .count()
            > 1
    );
    let _ = refused(call);
    // Genuine existing UnitLocal source, not an erased/no-op stand-in.
    let private = unit_assertion(SemanticAssertMessageV1::BoundsCheck {
        length: operand(SemanticTypeIdV1::from_index(1), 8, 8),
        index: operand(SemanticTypeIdV1::from_index(1), 0, 8),
    });
    let _ = refused(private);
}

#[test]
fn ordinary_scalar_constructor_does_not_gain_assertion_admission() {
    let source = literal(true, false);
    let mut work = Work::new(1 << 48);
    let mut budget = Budget::new(&mut work, S);
    let floor = source.unit_local_source_storage_floor_v1().unwrap() + SIBLING;
    budget.reserve_storage(floor).unwrap();
    assert!(
        ProductionCanonicalScalarFixedPointOwnerV1::try_prepare_v1(source, &mut budget).is_err()
    );
    assert_eq!(budget.storage(), floor);
}

#[test]
fn bad_final_query_is_sticky_and_fresh_consumption_recovers() {
    let owner = prepare(dynamic_source(true, false, false));
    assert!(
        history_run(&owner, |view, budget| {
            assert!(view.assertion(usize::MAX, budget).is_err());
            assert!(view.assertion_count(budget).is_err());
            Ok(())
        })
        .is_err()
    );
    history_run(&owner, |view, budget| final_reports(&owner, view, budget)).unwrap();
}

#[test]
fn rejected_callback_value_is_dropped_before_recovery() {
    struct Value(std::rc::Rc<std::cell::Cell<bool>>);
    impl Drop for Value {
        fn drop(&mut self) {
            self.0.set(true);
        }
    }
    let owner = prepare(literal(true, false));
    let dropped = std::rc::Rc::new(std::cell::Cell::new(false));
    assert!(
        history_run(&owner, |view, budget| {
            assert!(view.assertion(usize::MAX, budget).is_err());
            Ok(Value(dropped.clone()))
        })
        .is_err()
    );
    assert!(dropped.get());
    history_run(&owner, |view, budget| final_reports(&owner, view, budget)).unwrap();
}

#[test]
fn callback_panic_payload_is_dropped_and_real_final_consumer_recovers() {
    struct Payload(std::sync::Arc<std::sync::atomic::AtomicBool>);
    impl Drop for Payload {
        fn drop(&mut self) {
            self.0.store(true, std::sync::atomic::Ordering::SeqCst);
        }
    }
    let owner = prepare(literal(true, false));
    let dropped = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    assert!(
        history_run(&owner, |_, _| -> HistoryResult<()> {
            std::panic::panic_any(Payload(dropped.clone()));
        })
        .is_err()
    );
    assert!(dropped.load(std::sync::atomic::Ordering::SeqCst));
    history_run(&owner, |view, budget| final_reports(&owner, view, budget)).unwrap();
}

#[test]
fn retained_floor_one_short_and_zero_work_refuse_before_user_callback() {
    let owner = prepare(literal(true, false));
    for short_floor in [false, true] {
        let mut work = Work::new(if short_floor { 1 << 48 } else { 0 });
        let mut budget = Budget::new(&mut work, S);
        let floor = owner.retained_storage_floor_v1() - usize::from(short_floor);
        budget.reserve_storage(floor).unwrap();
        assert!(
            owner
                .with_assertion_policy_checks_v1(&mut budget, |_, _| -> HistoryResult<()> {
                    panic!("denied entry reached user callback")
                })
                .is_err()
        );
        assert_eq!(budget.storage(), floor);
    }
}

#[test]
fn unauthenticated_sidecar_omission_removal_and_preselected_event_are_rejected() {
    for retained in [false, true] {
        let owner = prepare(if retained {
            dynamic_source(true, false, false)
        } else {
            literal(true, false)
        });
        let before = snapshots(&owner);
        for fault in 0..3 {
            let mut work = Work::new(1 << 48);
            let mut budget = Budget::new(&mut work, S);
            let floor = owner.retained_storage_floor_v1() + SIBLING;
            budget.reserve_storage(floor).unwrap();
            let error = canonical_assertion_v1::read_test_scalar_assertion_sidecar_v1(
                &owner,
                &mut budget,
                fault,
            )
            .err()
            .expect("forged sidecar must not transport");
            assert!(matches!(error, HistoryError::AssertionTransport { .. }));
            assert_eq!(budget.storage(), floor);
            assert_eq!(snapshots(&owner), before);
        }
        history_run(&owner, |view, budget| {
            final_reports(&owner, view, budget)?;
            assert_eq!(
                view.assertion(0, budget)?.disposition(),
                if retained {
                    Disposition::Retained
                } else {
                    Disposition::HistoryElided
                }
            );
            Ok(())
        })
        .unwrap();
    }
}

#[test]
fn new_consumer_foreign_or_moved_query_budget_never_debits_substitute() {
    let owner = prepare(dynamic_source(true, false, false));
    for moved in [false, true] {
        let mut work = Work::new(1 << 48);
        let mut foreign_work = Work::new(1 << 48);
        let mut budget = Budget::new(&mut work, S);
        let mut foreign = Budget::new(&mut foreign_work, S);
        let floor = owner.retained_storage_floor_v1() + SIBLING;
        budget.reserve_storage(floor).unwrap();
        foreign.reserve_storage(SIBLING).unwrap();
        let result = owner.with_assertion_policy_checks_v1(&mut budget, |view, budget| {
            if moved {
                std::mem::swap(budget, &mut foreign);
                let before = foreign.work();
                assert!(view.assertion_count(&mut foreign).is_err());
                assert_eq!(foreign.work(), before);
                std::mem::swap(budget, &mut foreign);
            } else {
                assert!(view.assertion_count(&mut foreign).is_err());
            }
            Ok(())
        });
        assert!(result.is_err());
        assert_eq!(budget.storage(), floor);
        assert_eq!((foreign.storage(), foreign.work()), (SIBLING, 0));
    }
    history_run(&owner, |view, budget| final_reports(&owner, view, budget)).unwrap();
}

#[test]
fn new_consumer_floor_undercut_is_sticky_even_when_payment_is_restored() {
    let owner = prepare(literal(true, false));
    assert!(
        history_run(&owner, |view, budget| {
            let floor = budget.storage();
            budget.release_storage(1)?;
            assert!(view.assertion_count(budget).is_err());
            budget.reserve_storage(1)?;
            assert_eq!(budget.storage(), floor);
            assert!(view.final_inventory(budget).is_err());
            Ok(())
        })
        .is_err()
    );
    history_run(&owner, |view, budget| final_reports(&owner, view, budget)).unwrap();
}

#[test]
fn substituted_ledger_keeps_original_payment_until_backing_has_dropped() {
    let owner = prepare(literal(true, false));
    let mut work = Work::new(1 << 48);
    let mut foreign_work = Work::new(1 << 48);
    let mut budget = Budget::new(&mut work, S);
    let mut foreign = Budget::new(&mut foreign_work, S);
    let floor = owner.retained_storage_floor_v1() + SIBLING;
    budget.reserve_storage(floor).unwrap();
    foreign.reserve_storage(SIBLING).unwrap();
    let original_ledger = budget.work_ledger_identity_v1();
    let result = owner.with_assertion_policy_checks_v1(&mut budget, |_, budget| {
        std::mem::swap(budget, &mut foreign);
        Ok(())
    });
    assert!(result.is_err());
    assert_eq!((budget.storage(), budget.work()), (SIBLING, 0));
    assert!(foreign.work_ledger_identity_v1() == original_ledger);
    assert!(
        foreign.storage() > floor,
        "substituted ledger must not receive a scope refund"
    );
    // All rejected query backing has now dropped; the caller retains the owner.
    foreign.release_storage(foreign.storage() - floor).unwrap();
    assert_eq!(foreign.storage(), floor);
}

#[test]
fn assertion_constructor_first_work_and_owner_header_denials_are_source_derived() {
    for (limit, accepted, denied) in [(1, 0, 2), (4, 2, 5)] {
        let source = literal(true, false);
        let floor = source.unit_local_source_storage_floor_v1().unwrap() + SIBLING;
        let mut work = Work::new(limit);
        {
            let mut budget = Budget::new(&mut work, S);
            budget.reserve_storage(floor).unwrap();
            assert!(
                ProductionCanonicalScalarFixedPointOwnerV1::try_prepare_with_assertions_v1(
                    source,
                    &mut budget
                )
                .is_err()
            );
            assert_eq!(
                (budget.storage(), budget.work(), budget.peak_storage()),
                (floor, accepted, floor)
            );
            assert_eq!(budget.failed_storage(), None);
        }
        assert_eq!(work.failed_work(), Some(denied));
    }
    let source = literal(true, false);
    let floor = source.unit_local_source_storage_floor_v1().unwrap() + SIBLING;
    let requested = floor + std::mem::size_of::<ProductionCanonicalScalarFixedPointOwnerV1>();
    let mut work = Work::new(5);
    let mut budget = Budget::new(&mut work, requested - 1);
    budget.reserve_storage(floor).unwrap();
    assert!(
        ProductionCanonicalScalarFixedPointOwnerV1::try_prepare_with_assertions_v1(
            source,
            &mut budget
        )
        .is_err()
    );
    assert_eq!(
        (budget.storage(), budget.work(), budget.peak_storage()),
        (floor, 5, floor)
    );
    assert_eq!(budget.failed_storage(), Some(requested));
}
