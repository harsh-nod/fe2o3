use super::*;

// A static Work borrow can safely shorten to the classifier callback's inner
// lifetime. The caller's ordinary captured Budget cannot generally do that.
fn swapped_query<'work>(
    budget: &mut Budget<'work>,
    foreign: Budget<'static>,
    mut query: impl FnMut(&mut Budget<'work>) -> Result<(), LocalFrameErrorV1>,
) -> Result<(), LocalFrameErrorV1> {
    query(budget)?;
    let full = budget.storage();
    let original_prefix = budget.work();
    let slot = budget as *const Budget<'_>;
    let mut other: Budget<'work> = foreign;
    other.reserve_storage(full)?;
    std::mem::swap(budget, &mut other);
    assert_eq!(budget as *const Budget<'_>, slot);
    let result = query(budget);
    let foreign_state = (
        budget.work(),
        budget.storage(),
        budget.peak_storage(),
        budget.failed_storage(),
    );
    std::mem::swap(budget, &mut other);
    assert_eq!(
        result,
        Err(LocalFrameErrorV1::Resource(ResourceError::Accounting))
    );
    assert_eq!(budget.work(), original_prefix);
    assert_eq!(budget.storage(), full);
    assert_eq!(foreign_state, (5, full, full, None));
    assert_eq!(other.storage(), full);
    other.release_storage(full)?;
    query(budget)
}

fn replace_without_restoring<'work>(
    budget: &mut Budget<'work>,
    foreign: Budget<'static>,
    mode: u8,
) -> Result<(), LocalFrameErrorV1> {
    let full = budget.storage();
    let slot = budget as *const Budget<'_>;
    let mut other: Budget<'work> = foreign;
    other.reserve_storage(full)?;
    std::mem::swap(budget, &mut other);
    assert_eq!(budget as *const Budget<'_>, slot);
    // This HRTB callback cannot export the displaced Budget. Dropping its
    // wrapper does not grant the classifier access to either Work reference.
    drop(other);
    match mode {
        0 => Ok(()),
        1 => Err(refusal(99, None, LocalFrameRefusalReasonV1::Index)),
        _ => std::panic::panic_any("replaced Work callback"),
    }
}

#[test]
fn chain_getters_reject_same_slot_replacement_and_allow_restored_reentry() {
    let module = assert_chain(0);
    let verified = verify_module_ref(&module).unwrap();
    for query in 0..4 {
        let mut original_work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
        let mut budget = Budget::new(&mut original_work, usize::MAX);
        budget.reserve_storage(FLOOR).unwrap();
        let foreign = Budget::new(
            Box::leak(Box::new(CanonicalKernelIrWorkBudgetV1::new(usize::MAX))),
            usize::MAX,
        );
        with_checked_local_frame_chain_function_v1(verified, 0, &mut budget, |checked, budget| {
            swapped_query(budget, foreign, |budget| match query {
                0 => checked.allocations(budget).map(|_| ()),
                1 => checked.accesses(budget).map(|_| ()),
                2 => checked.control(budget).map(|_| ()),
                _ => checked.edge_bindings(budget).map(|_| ()),
            })
        })
        .unwrap();
        assert_eq!(budget.storage(), FLOOR);
        with_checked_local_frame_chain_function_v1(verified, 0, &mut budget, |_, _| Ok(()))
            .unwrap();
        assert_eq!(budget.storage(), FLOOR);
    }
}

#[test]
fn chain_no_getter_replacement_precedes_callback_results_and_foreign_cleanup() {
    let module = assert_chain(0);
    let verified = verify_module_ref(&module).unwrap();
    for mode in 0..3 {
        let mut original_work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
        let mut budget = Budget::new(&mut original_work, usize::MAX);
        budget.reserve_storage(FLOOR).unwrap();
        let foreign = Budget::new(
            Box::leak(Box::new(CanonicalKernelIrWorkBudgetV1::new(usize::MAX))),
            usize::MAX,
        );
        let mut entered = false;
        let mut full = 0;
        let mut original_prefix = 0;
        let outcome = catch_unwind(AssertUnwindSafe(|| {
            with_checked_local_frame_chain_function_v1(verified, 0, &mut budget, |_, budget| {
                entered = true;
                full = budget.storage();
                original_prefix = budget.work();
                replace_without_restoring(budget, foreign, mode)
            })
        }));
        assert!(entered && full > FLOOR);
        assert_eq!(
            outcome.expect("Accounting must precede the callback panic"),
            Err(LocalFrameErrorV1::Resource(ResourceError::Accounting))
        );
        assert_eq!(budget.work(), 0, "postflight is prepaid on original Work");
        assert_eq!(budget.storage(), full, "no foreign storage rollback");
        assert_eq!(budget.peak_storage(), full);
        assert_eq!(budget.failed_storage(), None);
        // Only the test owns this unrelated reservation. No original storage
        // ledger was restored or recreated after its wrapper was dropped.
        budget.release_storage(full).unwrap();
        assert_eq!(original_work.work(), original_prefix);
    }
}

#[test]
fn chain_entry_and_foreign_getter_have_isolated_work_first_boundaries() {
    let module = assert_chain(0);
    let verified = verify_module_ref(&module).unwrap();
    let header_total = FLOOR + chain_header_bytes();
    for limit in [3, 4] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(limit);
        let mut budget = Budget::new(&mut work, header_total - 1);
        budget.reserve_storage(FLOOR).unwrap();
        let mut entered = false;
        let result =
            with_checked_local_frame_chain_function_v1(verified, 0, &mut budget, |_, _| {
                entered = true;
                Ok(())
            });
        assert!(!entered);
        if limit == 3 {
            assert!(matches!(result,
                Err(LocalFrameErrorV1::Resource(ResourceError::Work(error)))
                    if error.actual() == 4 && error.limit() == 3
            ));
            assert_eq!(budget.work(), 0);
            assert_eq!(budget.failed_storage(), None);
        } else {
            assert!(matches!(result,
                Err(LocalFrameErrorV1::Resource(ResourceError::Storage(error)))
                    if error.actual() == header_total && error.limit() == header_total - 1
            ));
            assert_eq!(budget.work(), 4);
            assert_eq!(budget.failed_storage(), Some(header_total));
        }
        assert_eq!((budget.storage(), budget.peak_storage()), (FLOOR, FLOOR));
        drop(budget);
        assert_eq!(work.work(), if limit == 3 { 0 } else { 4 });
        assert_eq!(work.failed_work(), if limit == 3 { Some(4) } else { None });
    }
    let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
    let mut budget = Budget::new(&mut work, usize::MAX);
    budget.reserve_storage(FLOOR).unwrap();
    with_checked_local_frame_chain_function_v1(verified, 0, &mut budget, |checked, budget| {
        let full = budget.storage();
        let original_prefix = budget.work();
        for limit in [4, 5] {
            let mut foreign_work = CanonicalKernelIrWorkBudgetV1::new(limit);
            let mut foreign = Budget::new(&mut foreign_work, full);
            foreign.reserve_storage(full).unwrap();
            let result = checked.control(&mut foreign);
            if limit == 4 {
                assert!(matches!(result,
                    Err(LocalFrameErrorV1::Resource(ResourceError::Work(error)))
                        if error.actual() == 5 && error.limit() == 4
                ));
            } else {
                assert!(matches!(
                    result,
                    Err(LocalFrameErrorV1::Resource(ResourceError::Accounting))
                ));
            }
            let accepted = if limit == 4 { 0 } else { 5 };
            assert_eq!(foreign.work(), accepted);
            assert_eq!((foreign.storage(), foreign.peak_storage()), (full, full));
            assert_eq!(foreign.failed_storage(), None);
            drop(foreign);
            assert_eq!(foreign_work.work(), accepted);
            assert_eq!(
                foreign_work.failed_work(),
                if limit == 4 { Some(5) } else { None }
            );
            assert_eq!((budget.work(), budget.storage()), (original_prefix, full));
        }
        assert_eq!(checked.control(budget)?.len(), 5);
        Ok(())
    })
    .unwrap();
    assert_eq!(budget.storage(), FLOOR);
}
