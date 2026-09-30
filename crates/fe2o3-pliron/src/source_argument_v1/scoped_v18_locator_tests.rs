#[test]
fn inert_function_locator_uses_the_checked_original_entry_and_one_work_unit() {
    let source = semantic(&[U32]);
    let target = target(vec![Type::Scalar(ScalarType::U32)]);
    let rows = [direct(1, 0)];
    let cleanup = CanonicalAnalysisCleanupV1::new();
    let mut work = Work::new(usize::MAX);
    let mut budget = ArgumentBudgetV1::new(&mut work, usize::MAX);
    budget.reserve_storage(19).unwrap();
    let locator = with_parameter_correspondence_v18(
        &source,
        entry(&target),
        &target,
        trace(&rows),
        &cleanup,
        &mut budget,
        None,
        |view, budget| {
            let before = budget.work();
            let floor = budget.storage();
            let locator = view.semantic_function(budget)?;
            assert_eq!(locator, ROOT);
            assert_eq!(budget.work(), before + 1);
            assert_eq!(budget.storage(), floor);
            Ok(locator)
        },
    )
    .unwrap();
    assert_eq!(locator, ROOT);
    assert_eq!(budget.storage(), 19);
    assert!(!cleanup.refund_denied());

    for wrong in 0..2 {
        let cleanup = CanonicalAnalysisCleanupV1::new();
        let mut selected = entry(&target);
        if wrong == 0 {
            selected.semantic_function = SemanticFunctionIdV1::from_index(1);
        } else {
            selected.correspondence_owner = SemanticFunctionIdV1::from_index(1);
        }
        let entered = Cell::new(false);
        let result = with_parameter_correspondence_v18(
            &source,
            selected,
            &target,
            trace(&rows),
            &cleanup,
            &mut budget,
            None,
            |view, budget| {
                entered.set(true);
                view.semantic_function(budget)
            },
        );
        assert!(matches!(
            result,
            Err(ProductionSourceArgumentErrorV1::CorrespondenceMismatch)
        ));
        assert!(!entered.get());
        assert_eq!(budget.storage(), 19);
    }
}

#[test]
fn inert_function_locator_refuses_foreign_slot_ledger_and_lost_floor_before_work() {
    for mutation in 0..3 {
        let source = semantic(&[U32]);
        let target = target(vec![Type::Scalar(ScalarType::U32)]);
        let rows = [direct(1, 0)];
        let parent = Cell::new(false);
        let cleanup = CanonicalAnalysisCleanupV1::linked(&parent);
        let mut work = Work::new(usize::MAX);
        let mut foreign_work = Work::new(usize::MAX);
        let mut budget = ArgumentBudgetV1::new(&mut work, usize::MAX);
        let mut foreign = ArgumentBudgetV1::new(&mut foreign_work, usize::MAX);
        budget.reserve_storage(19).unwrap();
        let result = with_parameter_correspondence_v18(
            &source,
            entry(&target),
            &target,
            trace(&rows),
            &cleanup,
            &mut budget,
            None,
            |view, budget| {
                if mutation == 0 {
                    foreign.reserve_storage(budget.storage())?;
                    let before = (foreign.work(), foreign.storage());
                    assert!(view.semantic_function(&mut foreign).is_err());
                    assert_eq!((foreign.work(), foreign.storage()), before);
                } else {
                    if mutation == 1 {
                        foreign.reserve_storage(budget.storage())?;
                        std::mem::swap(budget, &mut foreign);
                    } else {
                        budget.release_storage(budget.storage())?;
                    }
                    let before = (budget.work(), budget.storage());
                    assert!(view.semantic_function(budget).is_err());
                    assert_eq!((budget.work(), budget.storage()), before);
                }
                Ok(())
            },
        );
        assert!(matches!(
            result,
            Err(
                ProductionSourceArgumentErrorV1::ArgumentCorrespondenceResource(
                    ArgumentResourceV1::Accounting
                )
            )
        ));
        assert!(parent.get());
        assert!(cleanup.refund_denied());
        if mutation == 2 {
            assert_eq!(budget.storage(), 0);
        } else {
            assert!(budget.storage() > 19);
        }
    }
}
