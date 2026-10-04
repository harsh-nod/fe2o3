use super::{
    CanonicalKernelIrOwnedVerificationResourceBudgetV1 as Owned,
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
    CanonicalKernelIrWorkBudgetV1 as Work, StorageState,
};
use std::panic::{AssertUnwindSafe, catch_unwind};

const fn storage_snapshot(budget: &Budget<'_>) -> (usize, usize, Option<usize>, usize) {
    (
        budget.storage(),
        budget.peak_storage(),
        budget.failed_storage(),
        budget.storage_limit(),
    )
}

#[test]
fn const_constructor_and_getters_support_an_inline_static_service_account() {
    const INITIAL: (usize, usize, usize, Option<usize>, usize) = {
        let account = Owned::new(Work::new(13), 17);
        (
            account.work(),
            account.storage(),
            account.peak_storage(),
            account.failed_storage(),
            account.storage_limit(),
        )
    };
    const INITIAL_WORK: (Option<usize>, usize) = {
        let account = Owned::new(Work::new(13), 17);
        (account.failed_work(), account.work_limit())
    };
    static ACCOUNT: std::sync::Mutex<Owned> = std::sync::Mutex::new(Owned::new(Work::new(13), 17));

    assert_eq!(INITIAL, (0, 0, 0, None, 17));
    assert_eq!(INITIAL_WORK, (None, 13));
    assert!(!std::mem::needs_drop::<Owned>());
    assert!(!std::mem::needs_drop::<Budget<'_>>());
    {
        let mut account = ACCOUNT.lock().unwrap();
        account.with_budget(|budget| {
            budget.charge_work(3).unwrap();
            budget.reserve_storage(5).unwrap();
        });
    }
    let mut account = ACCOUNT.lock().unwrap();
    account.with_budget(|budget| {
        assert_eq!(budget.work(), 3);
        assert_eq!(storage_snapshot(budget), (5, 5, None, 17));
        budget.charge_work(10).unwrap();
        budget.reserve_storage(12).unwrap();
    });
    assert_eq!((account.work(), account.storage()), (13, 17));
    account
        .with_budget(|budget| budget.release_storage(17))
        .unwrap();
    assert_eq!((account.storage(), account.peak_storage()), (0, 17));
}

#[test]
fn constructor_preserves_preused_work_and_denials_but_starts_empty_storage() {
    let mut work = Work::new(11);
    work.charge_work(4).unwrap();
    assert_eq!(work.charge_work(20).unwrap_err().actual(), 24);
    {
        let mut previous = Budget::new(&mut work, 7);
        previous.reserve_storage(5).unwrap();
        assert!(previous.reserve_storage(3).is_err());
    }

    let mut account = Owned::new(work, 9);
    assert_eq!(account.work(), 4);
    assert_eq!(account.failed_work(), Some(24));
    assert_eq!(account.work_limit(), 11);
    assert_eq!((account.storage(), account.peak_storage()), (0, 0));
    assert_eq!(account.failed_storage(), None);
    assert_eq!(account.storage_limit(), 9);
    account.with_budget(|budget| {
        assert_eq!(budget.work_limit_v1(), 11);
        assert_eq!(budget.work_budget_v1().failed_work(), Some(24));
        budget.charge_work(7).unwrap();
        budget.reserve_storage(9).unwrap();
    });
    account.with_budget(|budget| {
        assert!(matches!(budget.charge_work(1), Err(Resource::Work(error))
            if error.actual() == 12 && error.limit() == 11));
        assert_eq!(budget.work_budget_v1().failed_work(), Some(24));
        assert_eq!(storage_snapshot(budget), (9, 9, None, 9));
    });
    assert_eq!((account.work(), account.storage()), (11, 9));
    assert_eq!(account.failed_work(), Some(24));
    assert_eq!(account.work_limit(), 11);
}

#[test]
fn views_borrow_the_owned_fields_in_place() {
    let mut account = Owned::new(Work::new(7), 11);
    let work = &account.work as *const Work;
    let storage = &account.storage as *const StorageState;
    account.with_budget(|budget| {
        assert!(std::ptr::eq(budget.work_budget_v1(), work));
        assert!(std::ptr::eq(budget.storage.state(), storage));
        let identity = budget.work_ledger_identity_v1();
        budget.charge_work(2).unwrap();
        budget.reserve_storage(3).unwrap();
        budget
            .with_prepaid_scope::<_, Resource>(3, 1, 2, 5, |nested| {
                assert!(nested.work_ledger_identity_v1() == identity);
                assert!(std::ptr::eq(nested.work_budget_v1(), work));
                assert!(std::ptr::eq(nested.storage.state(), storage));
                Ok(())
            })
            .unwrap();
    });
    assert_eq!(
        (account.work(), account.storage(), account.peak_storage()),
        (4, 3, 8)
    );
}

#[test]
fn exact_and_one_short_limits_include_charges_from_earlier_views() {
    for (work_limit, storage_limit) in [(9, 11), (8, 11), (9, 10)] {
        let mut account = Owned::new(Work::new(work_limit), storage_limit);
        account.with_budget(|budget| {
            budget.charge_work(4).unwrap();
            budget.reserve_storage(5).unwrap();
        });
        let result = account.with_budget(|budget| {
            budget.charge_work(5)?;
            budget.reserve_storage(6)
        });
        if work_limit == 8 {
            assert!(matches!(result, Err(Resource::Work(error))
                if error.actual() == 9 && error.limit() == 8));
            assert_eq!(
                (account.work(), account.storage(), account.peak_storage()),
                (4, 5, 5)
            );
            assert_eq!(account.failed_work(), Some(9));
            assert_eq!(account.failed_storage(), None);
        } else if storage_limit == 10 {
            assert!(matches!(result, Err(Resource::Storage(error))
                if error.actual() == 11 && error.limit() == 10));
            assert_eq!(
                (account.work(), account.storage(), account.peak_storage()),
                (9, 5, 5)
            );
            assert_eq!(account.failed_work(), None);
            assert_eq!(account.failed_storage(), Some(11));
        } else {
            assert_eq!(result, Ok(()));
            assert_eq!(
                (account.work(), account.storage(), account.peak_storage()),
                (9, 11, 11)
            );
            assert_eq!(account.failed_work(), None);
            assert_eq!(account.failed_storage(), None);
        }
        assert_eq!(account.work_limit(), work_limit);
    }
}

#[test]
fn first_denials_and_peak_survive_later_views_releases_and_rollback() {
    let mut account = Owned::new(Work::new(10), 12);
    account.with_budget(|budget| {
        budget.charge_work(3).unwrap();
        budget.reserve_storage(4).unwrap();
        assert!(matches!(budget.charge_work(8), Err(Resource::Work(error))
            if error.actual() == 11));
        assert!(
            matches!(budget.reserve_storage(9), Err(Resource::Storage(error))
            if error.actual() == 13)
        );
    });
    account.with_budget(|budget| {
        budget.charge_work(2).unwrap();
        budget.reserve_storage(5).unwrap();
        assert!(matches!(budget.charge_work(10), Err(Resource::Work(error))
            if error.actual() == 15));
        assert!(
            matches!(budget.reserve_storage(10), Err(Resource::Storage(error))
            if error.actual() == 19)
        );
        assert!(budget.charge_work(usize::MAX).is_err());
        assert!(budget.reserve_storage(usize::MAX).is_err());
        budget.release_storage(4).unwrap();
        budget.rollback_storage(2).unwrap();
        assert_eq!(budget.storage_checkpoint(), 2);
    });
    assert_eq!(
        (account.work(), account.storage(), account.peak_storage()),
        (5, 2, 9)
    );
    assert_eq!(account.failed_work(), Some(11));
    assert_eq!(account.failed_storage(), Some(13));

    account.with_budget(|budget| {
        budget.charge_work(5).unwrap();
        budget.reserve_storage(10).unwrap();
        assert_eq!(budget.release_storage(13), Err(Resource::Accounting));
        assert_eq!(budget.rollback_storage(13), Err(Resource::Accounting));
        assert_eq!(budget.storage(), 12);
    });
    account
        .with_budget(|budget| budget.release_storage(12))
        .unwrap();
    assert_eq!(
        (account.work(), account.storage(), account.peak_storage()),
        (10, 0, 12)
    );
    assert_eq!(account.failed_work(), Some(11));
    assert_eq!(account.failed_storage(), Some(13));
}

#[test]
fn overflow_denials_preserve_prefixes_and_allow_later_exact_charges() {
    let mut account = Owned::new(Work::new(usize::MAX), usize::MAX);
    account.with_budget(|budget| {
        budget.charge_work(1).unwrap();
        budget.reserve_storage(1).unwrap();
    });
    account.with_budget(|budget| {
        assert!(
            matches!(budget.charge_work(usize::MAX), Err(Resource::Work(error))
            if error.actual() == usize::MAX && error.limit() == usize::MAX)
        );
        assert!(
            matches!(budget.reserve_storage(usize::MAX), Err(Resource::Storage(error))
            if error.actual() == usize::MAX && error.limit() == usize::MAX)
        );
    });
    assert_eq!(
        (account.work(), account.storage(), account.peak_storage()),
        (1, 1, 1)
    );
    account.with_budget(|budget| {
        budget.charge_work(usize::MAX - 1).unwrap();
        budget.reserve_storage(usize::MAX - 1).unwrap();
        budget.charge_work(0).unwrap();
        budget.reserve_storage(0).unwrap();
    });
    assert_eq!(
        (account.work(), account.storage()),
        (usize::MAX, usize::MAX)
    );
    account
        .with_budget(|budget| budget.release_storage(usize::MAX))
        .unwrap();
    assert_eq!((account.storage(), account.peak_storage()), (0, usize::MAX));
    assert_eq!(account.failed_work(), Some(usize::MAX));
    assert_eq!(account.work_limit(), usize::MAX);
    assert_eq!(account.failed_storage(), Some(usize::MAX));
}

#[test]
fn zero_limits_admit_only_zero_charges_across_views() {
    let mut account = Owned::new(Work::new(0), 0);
    for _ in 0..2 {
        account.with_budget(|budget| {
            budget.charge_work(0).unwrap();
            budget.reserve_storage(0).unwrap();
            budget.release_storage(0).unwrap();
            assert!(matches!(budget.charge_work(1), Err(Resource::Work(error))
                if error.actual() == 1 && error.limit() == 0));
            assert!(
                matches!(budget.reserve_storage(1), Err(Resource::Storage(error))
                if error.actual() == 1 && error.limit() == 0)
            );
        });
    }
    assert_eq!(
        (account.work(), account.storage(), account.peak_storage()),
        (0, 0, 0)
    );
    assert_eq!(account.failed_work(), Some(1));
    assert_eq!(account.work_limit(), 0);
    assert_eq!(account.failed_storage(), Some(1));
}

#[test]
fn nested_scopes_restore_the_persistent_pool_floor_on_success_error_and_unwind() {
    for exit in 0..3 {
        let mut account = Owned::new(Work::new(40), 30);
        account.with_budget(|budget| {
            budget.charge_work(2).unwrap();
            budget.reserve_storage(7).unwrap();
        });
        let result = catch_unwind(AssertUnwindSafe(|| {
            account.with_budget(|budget| {
                budget.with_prepaid_scope::<_, Resource>(7, 1, 4, 5, |budget| {
                    budget.charge_work(3)?;
                    budget.reserve_storage(6)?;
                    assert!(budget.charge_work(50).is_err());
                    assert!(budget.reserve_storage(20).is_err());
                    budget.with_prepaid_scope::<_, Resource>(18, 1, 2, 4, |budget| {
                        budget.reserve_storage(2)?;
                        match exit {
                            0 => Ok(17),
                            1 => Err(Resource::Arithmetic),
                            _ => panic!("nested service callback"),
                        }
                    })
                })
            })
        }));
        match exit {
            0 => assert_eq!(result.unwrap(), Ok(17)),
            1 => assert_eq!(result.unwrap(), Err(Resource::Arithmetic)),
            _ => assert!(result.is_err()),
        }
        assert_eq!(
            (account.work(), account.storage(), account.peak_storage()),
            (11, 7, 24)
        );
        assert_eq!(account.failed_work(), Some(59));
        assert_eq!(account.failed_storage(), Some(38));
        account.with_budget(|budget| {
            assert_eq!(storage_snapshot(budget), (7, 24, Some(38), 30));
            budget
                .with_prepaid_scope::<_, Resource>(7, 1, 2, 3, |_| Ok(()))
                .unwrap();
            budget.release_storage(7).unwrap();
        });
        assert_eq!(
            (account.work(), account.storage(), account.peak_storage()),
            (13, 0, 24)
        );
        assert_eq!(account.failed_work(), Some(59));
        assert_eq!(account.failed_storage(), Some(38));
    }
}

#[test]
fn an_error_or_unwind_without_a_scope_keeps_all_accepted_storage() {
    for panics in [false, true] {
        let mut account = Owned::new(Work::new(10), 11);
        account.with_budget(|budget| {
            budget.charge_work(2).unwrap();
            budget.reserve_storage(3).unwrap();
        });
        let result = catch_unwind(AssertUnwindSafe(|| {
            account.with_budget(|budget| {
                budget.charge_work(4)?;
                budget.reserve_storage(5)?;
                assert!(budget.reserve_storage(20).is_err());
                if panics {
                    panic!("service view callback");
                }
                Err::<(), _>(Resource::Arithmetic)
            })
        }));
        if panics {
            assert!(result.is_err());
        } else {
            assert_eq!(result.unwrap(), Err(Resource::Arithmetic));
        }
        assert_eq!(
            (account.work(), account.storage(), account.peak_storage()),
            (6, 8, 8)
        );
        assert_eq!(account.failed_storage(), Some(28));
        account
            .with_budget(|budget| budget.release_storage(8))
            .unwrap();
        assert_eq!(account.storage(), 0);
        assert_eq!(account.failed_storage(), Some(28));
    }
}

#[test]
fn moving_the_owner_preserves_accounting_without_a_persistent_identity_token() {
    let mut original = Owned::new(Work::new(7), 9);
    original.with_budget(|budget| {
        budget.charge_work(3).unwrap();
        budget.reserve_storage(5).unwrap();
        assert!(budget.charge_work(5).is_err());
        assert!(budget.reserve_storage(5).is_err());
    });
    let mut slot = Some(original);
    let mut moved = slot.take().unwrap();
    assert!(slot.is_none());
    moved.with_budget(|budget| {
        assert_eq!(budget.work(), 3);
        assert_eq!(storage_snapshot(budget), (5, 5, Some(10), 9));
        budget.charge_work(4).unwrap();
        budget.release_storage(5).unwrap();
    });
    assert_eq!(
        (moved.work(), moved.storage(), moved.peak_storage()),
        (7, 0, 5)
    );
    assert_eq!(moved.failed_work(), Some(8));
    assert_eq!(moved.work_limit(), 7);
    assert_eq!(moved.failed_storage(), Some(10));
}

// A foreign replacement must outlive any callback borrow. Only this test fixture
// allocates; the primitive stores and borrows both accounting states inline.
fn replacement_budget() -> Budget<'static> {
    Budget::new(Box::leak(Box::new(Work::new(100))), 100)
}

#[test]
fn replacing_a_view_does_not_replace_or_refund_the_owned_account() {
    for exit in 0..3 {
        let mut account = Owned::new(Work::new(10), 11);
        let mut replacement = replacement_budget();
        replacement.charge_work(2).unwrap();
        replacement.reserve_storage(3).unwrap();
        let result = catch_unwind(AssertUnwindSafe(|| {
            account.with_budget(move |budget| {
                budget.charge_work(3)?;
                budget.reserve_storage(5)?;
                assert!(budget.charge_work(8).is_err());
                assert!(budget.reserve_storage(7).is_err());
                let mut original = std::mem::replace(budget, replacement);
                assert!(original.work_ledger_identity_v1() != budget.work_ledger_identity_v1());
                original.charge_work(2)?;
                original.reserve_storage(2)?;
                budget.charge_work(4)?;
                budget.reserve_storage(8)?;
                budget.release_storage(11)?;
                match exit {
                    0 => Ok(29),
                    1 => Err(Resource::Arithmetic),
                    _ => panic!("replaced service view"),
                }
            })
        }));
        match exit {
            0 => assert_eq!(result.unwrap(), Ok(29)),
            1 => assert_eq!(result.unwrap(), Err(Resource::Arithmetic)),
            _ => assert!(result.is_err()),
        }
        assert_eq!(
            (account.work(), account.storage(), account.peak_storage()),
            (5, 7, 7)
        );
        assert_eq!(account.failed_work(), Some(11));
        assert_eq!(account.work_limit(), 10);
        assert_eq!(account.failed_storage(), Some(12));
        account.with_budget(|budget| {
            assert_eq!(storage_snapshot(budget), (7, 7, Some(12), 11));
            budget.release_storage(7).unwrap();
        });
        assert_eq!(account.storage(), 0);
    }
}

#[test]
fn replacement_in_a_scope_reports_accounting_and_retains_original_scratch() {
    for exit in 0..3 {
        let mut account = Owned::new(Work::new(20), 30);
        account.with_budget(|budget| {
            budget.charge_work(2).unwrap();
            budget.reserve_storage(7).unwrap();
        });
        let mut replacement = replacement_budget();
        replacement.charge_work(3).unwrap();
        replacement.reserve_storage(50).unwrap();
        let result = catch_unwind(AssertUnwindSafe(|| {
            account.with_budget(move |budget| {
                let result = budget.with_prepaid_scope::<(), Resource>(7, 1, 4, 5, |budget| {
                    *budget = replacement;
                    match exit {
                        0 => Ok(()),
                        1 => Err(Resource::Arithmetic),
                        _ => panic!("replaced prepaid view"),
                    }
                });
                assert_eq!(
                    (budget.work(), budget.storage(), budget.peak_storage()),
                    (3, 50, 50)
                );
                result
            })
        }));
        if exit == 2 {
            assert!(result.is_err());
        } else {
            assert_eq!(result.unwrap(), Err(Resource::Accounting));
        }
        assert_eq!(
            (account.work(), account.storage(), account.peak_storage()),
            (6, 12, 12)
        );
        account.with_budget(|budget| {
            budget.release_storage(5).unwrap();
            assert_eq!(budget.storage(), 7);
        });
        assert_eq!(account.storage(), 7);
    }
}

#[test]
fn borrowed_budget_still_owns_fresh_storage_while_sharing_existing_work() {
    let mut work = Work::new(10);
    work.charge_work(2).unwrap();
    {
        let mut budget = Budget::new(&mut work, 7);
        assert_eq!(storage_snapshot(&budget), (0, 0, None, 7));
        budget.charge_work(3).unwrap();
        budget.reserve_storage(4).unwrap();
        assert!(budget.charge_work(6).is_err());
        assert!(budget.reserve_storage(4).is_err());
        let floor = budget.storage_checkpoint();
        budget.reserve_storage(3).unwrap();
        budget.rollback_storage(floor).unwrap();
        assert_eq!(storage_snapshot(&budget), (4, 7, Some(8), 7));
    }
    let mut budget = Budget::new(&mut work, 9);
    assert_eq!(storage_snapshot(&budget), (0, 0, None, 9));
    assert_eq!(budget.work(), 5);
    assert_eq!(budget.work_budget_v1().failed_work(), Some(11));
    budget.charge_work(5).unwrap();
    budget.reserve_storage(9).unwrap();
    assert_eq!(budget.work(), 10);
    assert_eq!(storage_snapshot(&budget), (9, 9, None, 9));
}
