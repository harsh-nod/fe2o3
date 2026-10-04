use super::*;
use crate::{
    CanonicalKernelIrOwnedVerificationResourceBudgetV1 as Owned,
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
    CanonicalKernelIrWorkBudgetV1 as Work,
};

#[test]
fn window_preserves_send_and_requires_owned_storage() {
    fn send<T: Send>() {}
    send::<Budget<'static>>();
    send::<Owned>();
    let mut work = Work::new(100);
    let mut b = Budget::new(&mut work, 100);
    assert_eq!(
        b.with_additional_storage_window_v1(10, |_| Ok::<_, Resource>(())),
        Err(Resource::Accounting)
    );
    assert_eq!(b.storage_limit(), 100);
}

#[test]
fn window_enforces_both_ceilings_and_protects_entry_storage() {
    for (total, local, pass) in [(30, 10, true), (29, 10, false), (30, 9, false)] {
        let mut account = Owned::new(Work::new(100), total);
        account.with_budget(|b| {
            b.reserve_storage(20).unwrap();
            let address = b as *const Budget<'_> as usize;
            let ledger = b.work_ledger_identity_v1();
            let storage = b.storage_account_identity_v1();
            let result = b.with_additional_storage_window_v1(local, |b| {
                assert_eq!(b as *const Budget<'_> as usize, address);
                assert!(b.work_ledger_identity_v1() == ledger);
                assert_eq!(b.storage_account_identity_v1(), storage);
                assert_eq!(b.storage_limit(), total);
                assert_eq!(b.release_storage(1), Err(Resource::Accounting));
                b.reserve_storage(10)
            });
            assert_eq!(result.is_ok(), pass);
            assert_eq!(b.storage(), if pass { 30 } else { 20 });
            assert_eq!(b.failed_storage(), if pass { None } else { Some(30) });
            b.release_storage(b.storage()).unwrap();
        });
    }
}

#[test]
fn nested_window_cannot_widen_or_refund() {
    let mut account = Owned::new(Work::new(100), 100);
    account.with_budget(|b| {
        b.reserve_storage(20).unwrap();
        b.with_additional_storage_window_v1(10, |b| {
            b.reserve_storage(3)?;
            b.with_additional_storage_window_v1(99, |b| {
                assert_eq!(b.release_storage(1), Err(Resource::Accounting));
                b.reserve_storage(7)?;
                assert!(b.reserve_storage(1).is_err());
                Ok::<_, Resource>(())
            })?;
            assert_eq!(b.storage(), 30);
            b.release_storage(10)?;
            Ok::<_, Resource>(())
        })
        .unwrap();
        b.reserve_storage(80).unwrap();
        assert_eq!(b.peak_storage(), 100);
        assert_eq!(b.failed_storage(), Some(31));
    });
}

#[test]
fn entry_work_is_prepaid_before_window_activation_or_callback() {
    for limit in [
        Budget::STORAGE_WINDOW_WORK_V1 - 1,
        Budget::STORAGE_WINDOW_WORK_V1,
    ] {
        let mut account = Owned::new(Work::new(limit), 100);
        account.with_budget(|b| {
            b.reserve_storage(20).unwrap();
            let mut called = false;
            let result = b.with_additional_storage_window_v1(0, |_| {
                called = true;
                Ok::<_, Resource>(())
            });
            assert_eq!(called, limit == Budget::STORAGE_WINDOW_WORK_V1);
            assert_eq!(result.is_ok(), called);
            assert_eq!(b.work(), if called { limit } else { 0 });
            assert_eq!(
                b.failed_work(),
                if called {
                    None
                } else {
                    Some(Budget::STORAGE_WINDOW_WORK_V1)
                }
            );
            assert_eq!(b.window.unwrap().floor(), 0);
            assert_eq!(b.window.unwrap().ceiling(), usize::MAX);
            b.release_storage(20).unwrap();
            b.reserve_storage(100).unwrap();
        });
    }
}

#[test]
fn exhausted_parent_stays_exhausted_and_keeps_first_denial() {
    let mut account = Owned::new(Work::new(100), 100);
    account.with_budget(|b| {
        b.reserve_storage(20).unwrap();
        b.with_additional_storage_window_v1(10, |b| {
            b.reserve_storage(10)?;
            assert!(b.reserve_storage(2).is_err());
            assert_eq!(b.failed_storage(), Some(32));
            b.with_additional_storage_window_v1(60, |b| {
                assert!(b.reserve_storage(1).is_err());
                assert_eq!(b.release_storage(1), Err(Resource::Accounting));
                assert_eq!(b.failed_storage(), Some(32));
                Ok::<_, Resource>(())
            })?;
            assert!(b.reserve_storage(1).is_err());
            Ok::<_, Resource>(())
        })
        .unwrap();
        assert_eq!(b.storage(), 30);
        assert_eq!(b.failed_storage(), Some(32));
        b.reserve_storage(70).unwrap();
        assert_eq!(b.failed_storage(), Some(32));
    });
}

#[test]
fn exhausted_parent_work_refuses_nested_entry_without_changing_control() {
    let mut account = Owned::new(Work::new(Budget::STORAGE_WINDOW_WORK_V1), 100);
    account.with_budget(|b| {
        b.reserve_storage(20).unwrap();
        b.with_additional_storage_window_v1(10, |b| {
            assert!(b.charge_work(2).is_err());
            let denial = b.failed_work();
            let mut called = false;
            assert!(
                b.with_additional_storage_window_v1(0, |_| {
                    called = true;
                    Ok::<_, Resource>(())
                })
                .is_err()
            );
            assert!(!called);
            assert_eq!(b.failed_work(), denial);
            assert_eq!(b.window.unwrap().floor(), 20);
            assert_eq!(b.window.unwrap().ceiling(), 30);
            b.reserve_storage(10)?;
            Ok::<_, Resource>(())
        })
        .unwrap();
        assert_eq!(b.work(), Budget::STORAGE_WINDOW_WORK_V1);
        assert_eq!(b.failed_work(), Some(Budget::STORAGE_WINDOW_WORK_V1 + 2));
        b.reserve_storage(70).unwrap();
    });
}

#[test]
fn window_restores_only_control_on_all_exits() {
    for exit in 0..3 {
        let mut account = Owned::new(Work::new(40), 100);
        let result = catch_unwind(AssertUnwindSafe(|| {
            account.with_budget(|b| {
                b.reserve_storage(20).unwrap();
                b.with_additional_storage_window_v1(10, |b| {
                    b.reserve_storage(7)?;
                    b.charge_work(3)?;
                    assert!(b.reserve_storage(4).is_err());
                    assert!(b.charge_work(100).is_err());
                    match exit {
                        0 => Ok(()),
                        1 => Err(Resource::Allocation),
                        _ => panic!("window unwind"),
                    }
                })
            })
        }));
        match exit {
            0 => assert_eq!(result.unwrap(), Ok(())),
            1 => assert_eq!(result.unwrap(), Err(Resource::Allocation)),
            _ => assert!(result.is_err()),
        }
        assert_eq!(account.storage(), 27);
        assert_eq!(account.peak_storage(), 27);
        assert_eq!(account.failed_storage(), Some(31));
        assert_eq!(account.failed_work(), Some(111));
        assert_eq!(account.work(), 11);
        account.with_budget(|b| {
            b.release_storage(27).unwrap();
            b.reserve_storage(100).unwrap();
        });
    }
}

fn view(account: &mut Owned) -> Budget<'_> {
    Budget {
        work: &mut account.work,
        storage: Storage::Borrowed(&mut account.storage),
        window: Some(&account.window),
    }
}

#[test]
fn replacement_never_restores_into_foreign_account() {
    for exit in 0..3 {
        let mut original = Owned::new(Work::new(100), 100);
        let mut foreign = Owned::new(Work::new(100), 200);
        {
            let mut b = view(&mut original);
            let mut other = view(&mut foreign);
            b.reserve_storage(20).unwrap();
            other.reserve_storage(40).unwrap();
            let result = catch_unwind(AssertUnwindSafe(|| {
                b.with_additional_storage_window_v1(10, |b| {
                    b.reserve_storage(7)?;
                    std::mem::swap(b, &mut other);
                    b.reserve_storage(30)?;
                    match exit {
                        0 => Ok(()),
                        1 => Err(Resource::Allocation),
                        _ => panic!("replacement unwind"),
                    }
                })
            }));
            if exit == 2 {
                assert!(result.is_err());
            } else {
                assert_eq!(result.unwrap(), Err(Resource::Accounting));
            }
            // The moved-out original view observes restored account control.
            other.reserve_storage(73).unwrap();
            b.reserve_storage(130).unwrap();
        }
        assert_eq!((original.storage(), foreign.storage()), (100, 200));
    }
}

#[test]
fn rebuilding_inline_storage_with_same_work_cannot_launder_identity() {
    let mut original = Owned::new(Work::new(100), 100);
    let mut foreign_work = Work::new(100);
    {
        let mut b = view(&mut original);
        let mut replacement = Some(Budget::new(&mut foreign_work, 100));
        b.reserve_storage(20).unwrap();
        assert_eq!(
            b.with_additional_storage_window_v1(10, |b| {
                b.reserve_storage(7)?;
                let old = std::mem::replace(b, replacement.take().unwrap());
                *b = Budget::new(old.work, 100);
                Ok::<_, Resource>(())
            }),
            Err(Resource::Accounting)
        );
    }
    assert_eq!(original.storage(), 27);
    original.with_budget(|b| b.reserve_storage(73)).unwrap();
}

#[test]
fn overflow_and_already_overfull_account_refuse_before_callback() {
    let mut account = Owned::new(Work::new(100), 10);
    account.with_budget(|b| {
        b.reserve_storage(1).unwrap();
        assert_eq!(
            b.with_additional_storage_window_v1(usize::MAX, |_| Ok::<_, Resource>(())),
            Err(Resource::Arithmetic)
        );
        b.storage.state_mut().storage = 11;
        assert_eq!(
            b.with_additional_storage_window_v1(1, |_| panic!("overfull")),
            Err::<(), _>(Resource::Accounting)
        );
        assert_eq!(b.storage_limit(), 10);
    });
}
