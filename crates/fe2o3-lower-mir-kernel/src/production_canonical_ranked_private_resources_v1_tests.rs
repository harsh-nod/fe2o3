use super::*;
use std::cell::{Cell, RefCell};

#[test]
fn private_source_protection_has_independent_two_one_work_boundary() {
    for work_limit in [1, 2] {
        let mut work = Work::new(work_limit);
        let mut budget = Budget::new(&mut work, SIBLING);
        budget.reserve_storage(SIBLING).unwrap();
        let entered = Cell::new(false);
        let result = cr_private_protected_v1(&mut budget, |_| {
            entered.set(true);
            Ok(())
        });
        assert_eq!(result.is_ok(), work_limit == 2);
        assert_eq!(entered.get(), work_limit == 2);
        assert_eq!(budget.storage(), SIBLING);
    }
}

#[test]
fn private_source_callback_requires_exact_local_floor_but_allows_restored_scratch() {
    let erased_owner = erased();
    for excess in [false, true] {
        let error = run_private(erased_owner.original_source(), |_, budget| {
            if excess {
                budget.reserve_storage(1)?;
            } else {
                budget.release_storage(1)?;
            }
            Ok(())
        })
        .unwrap_err();
        assert!(matches!(
            error,
            PolicyError::Source(ProductionCanonicalRankedSourceErrorV1::Resource(
                ArgumentResourceV1::Accounting
            ))
        ));
    }
    run_private(erased_owner.original_source(), |view, budget| {
        let floor = budget.storage();
        budget.reserve_storage(17)?;
        assert_eq!(view.census(budget)?, (1, 2, 1));
        budget.release_storage(17)?;
        assert_eq!(budget.storage(), floor);
        Ok(())
    })
    .unwrap();
}

#[test]
fn rejected_private_callback_owner_drops_before_any_paid_budget_refund() {
    struct PaidDrop<'a, 'w> {
        original: Option<Budget<'w>>,
        saved: &'a RefCell<Option<Budget<'w>>>,
        observed: &'a Cell<usize>,
        drops: &'a Cell<usize>,
        panic: bool,
    }
    impl Drop for PaidDrop<'_, '_> {
        fn drop(&mut self) {
            let original = self.original.take().unwrap();
            self.observed.set(original.storage());
            self.drops.set(self.drops.get() + 1);
            *self.saved.borrow_mut() = Some(original);
            if self.panic {
                panic!("private rejected owner destructor");
            }
        }
    }
    let erased_owner = erased();
    let owner = erased_owner.original_source();
    for panic in [false, true] {
        let mut work = Work::new(1 << 48);
        let mut foreign_work = Work::new(1 << 48);
        let mut budget = Budget::new(&mut work, S);
        let mut foreign = Budget::new(&mut foreign_work, S);
        foreign.reserve_storage(SIBLING).unwrap();
        let floor = owner.unit_local_source_storage_floor_v1().unwrap()
            + std::mem::size_of::<PaidDrop<'_, '_>>()
            + SIBLING;
        budget.reserve_storage(floor).unwrap();
        let ledger = budget.work_ledger_identity_v1();
        let saved = RefCell::new(None);
        let observed = Cell::new(0);
        let drops = Cell::new(0);
        let paid = Cell::new(0);
        let result =
            owner.with_checked_canonical_ranked_source_v1(&mut budget, |source, budget| {
                Ok(
                    source.with_private_policy_checks_v1(budget, |view, budget| {
                        assert_eq!(view.census(budget)?, (1, 2, 1));
                        paid.set(budget.storage());
                        let original = std::mem::replace(budget, foreign);
                        Ok(PaidDrop {
                            original: Some(original),
                            saved: &saved,
                            observed: &observed,
                            drops: &drops,
                            panic,
                        })
                    }),
                )
            });
        assert!(result.is_err());
        assert_eq!(drops.get(), 1);
        assert_eq!(observed.get(), paid.get());
        assert!(observed.get() > floor);
        assert_eq!((budget.storage(), budget.work()), (SIBLING, 0));
        let original = saved.borrow_mut().take().unwrap();
        assert!(original.work_ledger_identity_v1() == ledger);
        assert_eq!(original.storage(), paid.get());
        let _foreign = std::mem::replace(&mut budget, original);
        budget.release_storage(paid.get() - floor).unwrap();
        assert_eq!(budget.storage(), floor);
    }
}
