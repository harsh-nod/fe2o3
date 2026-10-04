fn completion_resource_observer_v1(
    _: &ExecutionLifecycleSourceV29<'_>,
    instances: &ExecutionInstancesV29<'_>,
    emitted: &mut [Option<LoweredFunctionResultV1>],
    _: &OwnedScopedSourceSlotsV29,
    original_budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    let count = instances.instances().len();
    let floor = 37;
    let header = std::mem::size_of::<ExecutionReturnCompletionV1<'_, '_>>()
        + std::mem::size_of::<
            Result<ExecutionReturnCompletionV1<'_, '_>, ProductionSemanticKirErrorV1>,
        >();
    let bytes = header + count * std::mem::size_of::<Option<Vec<Option<usize>>>>();
    // Original source20 + owner2 + Vec allocation3 + row initializationN;
    // consuming settlement repeats owner3 + original source20.
    let exact_work = 48 + count;
    for (work_limit, storage_limit, success) in [
        (exact_work, floor + bytes, true),
        (exact_work - 1, floor + bytes, false),
        (exact_work, floor + bytes - 1, false),
    ] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(work_limit);
        let mut budget = ArgumentBudgetV1::new(&mut work, storage_limit);
        budget.reserve_storage(floor)?;
        assert!(budget.reserve_storage(storage_limit + 17).is_err());
        assert!(budget.charge_work(work_limit + 19).is_err());
        let history = budget.failed_storage();
        let mut constructed = false;
        let result = scoped_slot_attempt_v29(&mut budget, |budget| {
            let completion = ExecutionReturnCompletionV1::new(instances, None, None, budget)?;
            constructed = true;
            assert_eq!(completion.producers.len(), count);
            assert_eq!(completion.credit, bytes);
            assert!(completion.producers.iter().all(Option::is_none));
            completion.discard(budget)
        });
        assert_eq!(result.is_ok(), success);
        assert_eq!(constructed, storage_limit == floor + bytes);
        assert_eq!(budget.storage(), floor);
        assert_eq!(budget.failed_storage(), history);
        assert_eq!(
            budget.peak_storage(),
            if storage_limit == floor + bytes {
                floor + bytes
            } else {
                floor + header
            }
        );
        drop(budget);
        assert_eq!(
            work.work(),
            if storage_limit != floor + bytes {
                25
            } else if success {
                exact_work
            } else {
                exact_work - 20
            }
        );
        assert_eq!(work.failed_work(), Some(work_limit + 19));
    }
    // Source-owner construction is not archive authority: a foreign ledger
    // cannot use an actual paid archive, even with the same original instances.
    let mut work = CanonicalKernelIrWorkBudgetV1::new(LIMIT);
    let mut foreign = ArgumentBudgetV1::new(&mut work, LIMIT);
    foreign.reserve_storage(floor)?;
    let mut reached = false;
    let refused = scoped_slot_attempt_v29(&mut foreign, |budget| {
        let mut completion = ExecutionReturnCompletionV1::new(instances, None, None, budget)?;
        reached = true;
        completion.record_completed(emitted[0].as_ref().unwrap(), budget)
    });
    assert!(reached);
    assert!(matches!(
        refused,
        Err(
            ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                ArgumentResourceV1::Accounting
            )
        )
    ));
    assert_eq!(foreign.storage(), floor);

    let floor = original_budget.storage();
    let mut completion = ExecutionReturnCompletionV1::new(instances, None, None, original_budget)?;
    for output in emitted.iter().flatten() {
        completion.record_completed(output, original_budget)?;
    }
    let retained = original_budget.storage();
    assert!(
        completion
            .record_completed(emitted[0].as_ref().unwrap(), original_budget)
            .is_err()
    );
    assert_eq!(original_budget.storage(), retained);
    let borrowed: Vec<_> = emitted.iter().map(Option::as_ref).collect();
    for (ordinal, row) in emitted.iter().enumerate() {
        let id = instances.id_at(ordinal).unwrap();
        let found = completion.completed_output(id, &borrowed, original_budget)?;
        assert!(std::ptr::eq(found, row.as_ref().unwrap()));
    }
    drop(borrowed);
    completion.discard(original_budget)?;
    assert_eq!(original_budget.storage(), floor);
    FRESH_SOURCE_OBSERVED_V1.set((count, 1));
    Ok(())
}

#[test]
fn completed_producer_index_has_independent_exact_limits_and_original_archive_custody() {
    FRESH_SOURCE_OBSERVED_V1.set((0, 0));
    let (result, _, _) = run_profiled_suffix_owner(
        || fresh_tile_return_owner_v1(FreshReturnCaseV1::Straight),
        completion_resource_observer_v1,
        LIMIT,
        LIMIT,
        |_, _, _| Ok(()),
    );
    assert!(result.is_ok(), "{result:?}");
    assert_eq!(FRESH_SOURCE_OBSERVED_V1.get(), (7, 1));
}

fn completion_error_panic_observer_v1(
    _: &ExecutionLifecycleSourceV29<'_>,
    instances: &ExecutionInstancesV29<'_>,
    _: &mut [Option<LoweredFunctionResultV1>],
    _: &OwnedScopedSourceSlotsV29,
    _: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    for panic in [false, true] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(LIMIT);
        let mut budget = ArgumentBudgetV1::new(&mut work, LIMIT);
        budget.reserve_storage(53)?;
        assert!(budget.reserve_storage(LIMIT).is_err());
        let history = budget.failed_storage();
        let mut entered = false;
        let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            scoped_slot_attempt_v29(
                &mut budget,
                |budget| -> Result<(), ProductionSemanticKirErrorV1> {
                    let _owner = ExecutionReturnCompletionV1::new(instances, None, None, budget)?;
                    entered = true;
                    if panic {
                        std::panic::panic_any(1191usize);
                    }
                    Err(execution_identity_error_v1())
                },
            )
        }));
        assert!(entered);
        if panic {
            assert_eq!(*outcome.unwrap_err().downcast::<usize>().unwrap(), 1191);
        } else {
            assert!(matches!(
                outcome,
                Ok(Err(ProductionSemanticKirErrorV1::Unsupported {
                    detail: "nominal identity equations differ from their original source",
                    ..
                }))
            ));
        }
        assert_eq!(budget.storage(), 53);
        assert_eq!(budget.failed_storage(), history);
    }
    FRESH_SOURCE_OBSERVED_V1.set((2, 1));
    Ok(())
}

#[test]
fn producer_index_construction_scope_keeps_first_denial_and_selected_error_or_panic() {
    FRESH_SOURCE_OBSERVED_V1.set((0, 0));
    let (result, _, _) = run_profiled_suffix_owner(
        || fresh_tile_return_owner_v1(FreshReturnCaseV1::Straight),
        completion_error_panic_observer_v1,
        LIMIT,
        LIMIT,
        |_, _, _| Ok(()),
    );
    assert!(result.is_ok(), "{result:?}");
    assert_eq!(FRESH_SOURCE_OBSERVED_V1.get(), (2, 1));
}
