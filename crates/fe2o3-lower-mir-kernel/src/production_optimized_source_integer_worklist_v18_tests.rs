#[test]
fn source_worklist_continuation_uses_live_correspondences_and_preserves_memory() {
    for factory in [
        integer_add_source_v18 as fn() -> _,
        integer_multiply_source_v18,
        integer_and_source_v18,
    ] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(OPTIMIZED_SOURCE_WORK_LIMIT_V18);
        let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
        budget.reserve_storage(MODULE_FLOOR).unwrap();
        let prepared = scalar_payload_prepared_from_v18(factory, &mut budget);
        let completed = std::cell::Cell::new(false);
        prepared
            .with_source_consumer_v18(&mut budget, |source, budget| {
                let floor = budget.storage();
                let (owner, (), receipt) =
                    source
                        .with_checked_integer_worklist_optimization_v18(
                            budget,
                            |original, optimized, budget| {
                                let input = optimized.input_inventory(budget)?;
                                assert!(std::ptr::eq(input, original.inventory(budget)?));
                                assert!(std::ptr::eq(
                                    optimized.original_source(budget)?,
                                    original.source
                                ));
                                let output = optimized.output_inventory(budget)?;
                                assert!(input.operations().iter().any(|r| matches!(
                                    r.operation.kind,
                                    OperationKind::Binary { .. }
                                )));
                                assert!(!output.operations().iter().any(|r| matches!(
                                    r.operation.kind,
                                    OperationKind::Binary { .. }
                                )));
                                let mut memory = [0usize; 2];
                                for row in input.operations() {
                                    let kind = match row.operation.kind {
                                        OperationKind::Load { .. } => Some(0),
                                        OperationKind::Store { .. } => Some(1),
                                        _ => None,
                                    };
                                    if let Some(kind) = kind {
                                        assert!(matches!(
                                            optimized.operation(row.coordinate, budget)?,
                                            ProductionOptimizedSourceOperationV18::Retained { .. }
                                        ));
                                        memory[kind] += 1;
                                    }
                                }
                                assert!(memory[0] > 0 && memory[1] > 0);
                                Ok::<_, ProductionSourceOwnedViewErrorV18>(((), 0))
                            },
                        )
                        .unwrap();
                assert_eq!(owner.execution().policy_version(), 9);
                assert_eq!(
                    &owner.execution().canonical_bytes()[..8],
                    &[9, 0, 1, 0, 2, 0, 18, 0]
                );
                assert_eq!(
                    owner.report().passes()[0].pass(),
                    fe2o3_pliron::PlironOptimizationPassV1::IntegerNeutralWorklistCanonicalization
                );
                assert!(owner.report().passes()[0].changed());
                assert!(!owner.grants_authority() && !receipt.grants_authority());
                assert_eq!(owner.map().output_identity(), owner.owner().identity());
                drop(owner);
                assert_eq!(budget.storage(), floor);
                completed.set(true);
                Ok::<_, ProductionSourceOwnedViewErrorV18>(())
            })
            .unwrap();
        assert!(completed.get());
        assert_eq!(budget.storage(), MODULE_FLOOR);
    }
}

#[test]
fn source_worklist_non_neutral_rules_and_historical_integer_policy_are_unchanged() {
    for factory in [
        integer_non_neutral_source_v18 as fn() -> _,
        integer_divide_source_v18,
    ] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(OPTIMIZED_SOURCE_WORK_LIMIT_V18);
        let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
        budget.reserve_storage(MODULE_FLOOR).unwrap();
        let prepared = scalar_payload_prepared_from_v18(factory, &mut budget);
        prepared
            .with_source_consumer_v18(&mut budget, |source, budget| {
                let (old, (), _) = source
                    .with_checked_integer_optimization_v18(budget, integer_noop_consumer_v18)
                    .unwrap();
                let before = *old.owner().identity();
                assert_eq!(old.execution().policy_version(), 6);
                drop(old);
                let (new, (), _) = source
                    .with_checked_integer_worklist_optimization_v18(
                        budget,
                        integer_noop_consumer_v18,
                    )
                    .unwrap();
                assert_eq!(new.owner().identity(), &before);
                assert!(!new.report().passes()[0].changed());
                assert_eq!(new.execution().policy_version(), 9);
                drop(new);
                Ok::<_, ProductionSourceOwnedViewErrorV18>(())
            })
            .unwrap();
        assert_eq!(budget.storage(), MODULE_FLOOR);
    }
}

#[test]
fn retained_source_worklist_owner_keeps_exact_storage_until_owned_disposal() {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(OPTIMIZED_SOURCE_WORK_LIMIT_V18);
    let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
    budget.reserve_storage(MODULE_FLOOR).unwrap();
    let prepared = scalar_payload_prepared_from_v18(integer_add_source_v18, &mut budget);
    prepared
        .with_source_consumer_v18(&mut budget, |source, budget| {
            let floor = budget.storage();
            let (owner, (), receipt) = source
                .with_retained_checked_integer_worklist_optimization_v18(
                    budget,
                    integer_noop_consumer_v18,
                )
                .unwrap();
            let retained = owner.storage().retained_storage() + receipt.retained_storage();
            assert_eq!(budget.storage(), floor + retained);
            assert_eq!(owner.execution().policy_version(), 9);
            drop(owner);
            assert_eq!(budget.storage(), floor + retained);
            budget.release_storage(retained)?;
            assert_eq!(budget.storage(), floor);
            Ok::<_, ProductionSourceOwnedViewErrorV18>(())
        })
        .unwrap();
    assert_eq!(budget.storage(), MODULE_FLOOR);
}

#[test]
fn source_worklist_entry_one_short_preserves_original_refusal_on_retry() {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(OPTIMIZED_SOURCE_WORK_LIMIT_V18);
    let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
    budget.reserve_storage(MODULE_FLOOR).unwrap();
    let prepared = scalar_payload_prepared_from_v18(integer_add_source_v18, &mut budget);
    let completed = std::cell::Cell::new(false);
    let result = prepared.with_source_consumer_v18(&mut budget, |source, budget| {
        let consume = integer_noop_consumer_v18;
        // The new nominal wrappers retain exactly the predecessor's field layout;
        // use the independent predecessor header equation, not the production helper.
        assert_eq!(
            size_of::<fe2o3_pliron::CheckedNeutralKernelIrOwnerIntegerWorklistV18>(),
            size_of::<fe2o3_pliron::CheckedNeutralKernelIrOwnerIntegerContinuationV18>()
        );
        assert_eq!(
            size_of::<fe2o3_pliron::KirNeutralOptimizationOutputIntegerWorklistV18<'_>>(),
            size_of::<fe2o3_pliron::KirNeutralOptimizationOutputIntegerContinuationV18<'_>>()
        );
        let headers = integer_entry_header_oracle_v18(&consume);
        assert_eq!(
            headers,
            IntegerWorklistSourceOptimizerV18::headers::<(), ProductionSourceOwnedViewErrorV18, _>(
                &consume
            )
            .unwrap()
        );
        let padding = MODULE_LIMIT - budget.storage() - headers + 1;
        budget.reserve_storage(padding).unwrap();
        let first = match source.with_checked_integer_worklist_optimization_v18(budget, consume) {
            Err(ProductionSourceOptimizationErrorV18::Source(
                ProductionSourceOwnedViewErrorV18::Resource(error),
            )) => error,
            _ => panic!("worklist entry must refuse its one-short header"),
        };
        assert!(matches!(first, ArgumentResourceV1::Storage(_)));
        budget.release_storage(padding).unwrap();
        let before = (budget.work(), budget.storage());
        match source.with_checked_integer_worklist_optimization_v18(budget, consume) {
            Err(ProductionSourceOptimizationErrorV18::Source(
                ProductionSourceOwnedViewErrorV18::Resource(error),
            )) => assert_eq!(error, first),
            _ => panic!("worklist retry forgot its original refusal"),
        }
        assert_eq!((budget.work(), budget.storage()), before);
        completed.set(true);
        Ok::<_, ProductionSourceOwnedViewErrorV18>(())
    });
    assert!(completed.get());
    assert!(matches!(
        result,
        Err(ProductionSourceOwnedViewErrorV18::Resource(
            ArgumentResourceV1::Storage(_)
        ))
    ));
    assert_eq!(budget.storage(), MODULE_FLOOR);
}

#[test]
fn source_worklist_callback_error_is_not_relabelled_as_admission() {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(OPTIMIZED_SOURCE_WORK_LIMIT_V18);
    let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
    budget.reserve_storage(MODULE_FLOOR).unwrap();
    let prepared = scalar_payload_prepared_from_v18(integer_add_source_v18, &mut budget);
    let completed = std::cell::Cell::new(false);
    prepared
        .with_source_consumer_v18(&mut budget, |source, budget| {
            let floor = budget.storage();
            let result =
                source.with_checked_integer_worklist_optimization_v18(budget, |_, _, _| {
                    Err::<((), usize), _>(ProductionSourceOwnedViewErrorV18::Binding(
                        "worklist consumer refusal",
                    ))
                });
            assert!(matches!(
                result,
                Err(ProductionSourceOptimizationErrorV18::Adoption(
                    fe2o3_pliron::KirCheckedNeutralOptimizationErrorV1::Origin(
                        ProductionSourceOwnedViewErrorV18::Binding("worklist consumer refusal")
                    )
                ))
            ));
            assert_eq!(budget.storage(), floor);
            completed.set(true);
            Ok::<_, ProductionSourceOwnedViewErrorV18>(())
        })
        .unwrap();
    assert!(completed.get());
    assert_eq!(budget.storage(), MODULE_FLOOR);
}

mod bound_private_tests_v21 {
    use super::*;
    include!("production_source_bound_worklist_handoff_v21_tests.rs");
}
