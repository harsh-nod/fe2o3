//! Original-account scope tests, not fabricated formula/request/proof fixtures.
use super::*;
use fe2o3_kernel_ir::{
    CanonicalKernelIrOwnedVerificationResourceBudgetV1 as Owned,
    CanonicalKernelIrWorkBudgetV1 as Work,
};
use std::{
    cell::Cell,
    panic::{AssertUnwindSafe, catch_unwind},
};

const FLOOR: usize = 31;
const PREFIX: usize = 17;

#[derive(Clone, Copy)]
enum Scope {
    Scratch,
    Retained,
}
impl Scope {
    fn run<T>(
        self,
        budget: &mut Budget<'_>,
        bytes: usize,
        run: impl FnOnce(&mut Budget<'_>) -> Result<T, Error>,
    ) -> Result<T, Error> {
        match self {
            Self::Scratch => with_scratch_using(budget, bytes, run),
            Self::Retained => retention::retain_reservation_using(budget, bytes, run),
        }
    }
}

fn ledger(work: usize, storage: usize) -> Owned {
    let mut ledger = Owned::new(Work::new(work), storage);
    ledger.with_budget(|budget| {
        budget.charge_work(PREFIX).unwrap();
        budget.reserve_storage(FLOOR).unwrap();
    });
    ledger
}

struct InertDrop<'a>(&'a Cell<usize>);
impl Drop for InertDrop<'_> {
    fn drop(&mut self) {
        self.0.set(self.0.get() + 1);
    }
}

#[test]
fn v2_nested_scopes_have_exact_independent_work_and_storage_boundaries() {
    let scratch = retention::PREPARATION_STORAGE;
    let peak = FLOOR + RETAINED_STORAGE + scratch + 13;
    let exact_work = PREFIX + 2 + 11;
    for (work, storage, success) in [
        (exact_work, peak, true),
        (exact_work - 1, peak, false),
        (exact_work, peak - 1, false),
    ] {
        let mut ledger = ledger(work, storage);
        let drops = Cell::new(0);
        let result = ledger.with_budget(|budget| {
            let account = budget.work_ledger_identity_v1();
            retention::retain_reservation_using(budget, RETAINED_STORAGE, |budget| {
                with_scratch_using::<_, Error>(budget, scratch, |budget| {
                    assert!(budget.work_ledger_identity_v1() == account);
                    assert_eq!(budget.storage(), FLOOR + RETAINED_STORAGE + scratch);
                    budget.charge_work(11)?;
                    budget.reserve_storage(13)?;
                    Ok(InertDrop(&drops))
                })
            })
        });
        assert_eq!(result.is_ok(), success);
        if success {
            assert_eq!(ledger.storage(), FLOOR + RETAINED_STORAGE + 13);
            assert_eq!(ledger.peak_storage(), peak);
            assert_eq!(ledger.work(), exact_work);
            assert_eq!(drops.get(), 0);
            drop(result);
            assert_eq!(drops.get(), 1);
            ledger
                .with_budget(|budget| budget.release_storage(RETAINED_STORAGE))
                .unwrap();
            assert_eq!(ledger.storage(), FLOOR + 13);
        } else {
            assert_eq!(ledger.storage(), FLOOR);
            assert_eq!(drops.get(), 0);
            if work < exact_work {
                assert!(matches!(
                    result,
                    Err(Error::Formula(
                        ProductionConditionalFormulaErrorV1::Resource(Resource::Work(_))
                    ))
                ));
                assert_eq!(
                    (ledger.work(), ledger.failed_work()),
                    (PREFIX + 2, Some(exact_work))
                );
            } else {
                assert!(matches!(
                    result,
                    Err(Error::Formula(
                        ProductionConditionalFormulaErrorV1::Resource(Resource::Storage(_))
                    ))
                ));
                assert_eq!(
                    (ledger.work(), ledger.failed_storage()),
                    (exact_work, Some(peak))
                );
            }
        }
    }
}

#[test]
fn v2_scopes_deny_before_callback_and_keep_the_first_denial() {
    for scope in [Scope::Scratch, Scope::Retained] {
        let mut work_denied = ledger(PREFIX, FLOOR + 20);
        let result = work_denied
            .with_budget(|budget| scope.run::<()>(budget, 20, |_| panic!("work denial bypassed")));
        assert!(matches!(
            result,
            Err(Error::Formula(
                ProductionConditionalFormulaErrorV1::Resource(Resource::Work(_))
            ))
        ));
        assert_eq!(
            (
                work_denied.storage(),
                work_denied.work(),
                work_denied.failed_work()
            ),
            (FLOOR, PREFIX, Some(PREFIX + 1))
        );
        work_denied.with_budget(|budget| assert!(budget.charge_work(99).is_err()));
        assert_eq!(work_denied.failed_work(), Some(PREFIX + 1));

        let mut storage_denied = ledger(1000, FLOOR + 19);
        let result = storage_denied.with_budget(|budget| {
            scope.run::<()>(budget, 20, |_| panic!("storage denial bypassed"))
        });
        assert!(matches!(
            result,
            Err(Error::Formula(
                ProductionConditionalFormulaErrorV1::Resource(Resource::Storage(_))
            ))
        ));
        assert_eq!(
            (
                storage_denied.storage(),
                storage_denied.work(),
                storage_denied.failed_storage()
            ),
            (FLOOR, PREFIX + 1, Some(FLOOR + 20))
        );
        storage_denied.with_budget(|budget| assert!(budget.reserve_storage(99).is_err()));
        assert_eq!(storage_denied.failed_storage(), Some(FLOOR + 20));
    }
}

#[test]
fn v2_scopes_preserve_callback_charges_on_refusal_and_unwind() {
    for scope in [Scope::Scratch, Scope::Retained] {
        for unwind in [false, true] {
            let mut ledger = ledger(1000, 1000);
            let outcome = catch_unwind(AssertUnwindSafe(|| {
                ledger.with_budget(|budget| {
                    scope.run::<()>(budget, 20, |budget| {
                        budget.charge_work(11)?;
                        budget.reserve_storage(13)?;
                        if unwind {
                            panic!("callback unwind");
                        }
                        Err(Error::Subject("callback refusal"))
                    })
                })
            }));
            if unwind {
                assert!(outcome.is_err());
            } else {
                assert!(matches!(
                    outcome,
                    Ok(Err(Error::Subject("callback refusal")))
                ));
            }
            assert_eq!(
                (ledger.storage(), ledger.work(), ledger.peak_storage()),
                (FLOOR + 13, PREFIX + 12, FLOOR + 33)
            );
        }
    }
}

#[test]
fn v2_damaged_floor_overrides_success_and_drops_provisional_result() {
    for scope in [Scope::Scratch, Scope::Retained] {
        let mut ledger = ledger(1000, 1000);
        let drops = Cell::new(0);
        let result = ledger.with_budget(|budget| {
            scope.run(budget, 20, |budget| {
                budget.release_storage(1)?;
                Ok(InertDrop(&drops))
            })
        });
        assert!(matches!(
            result,
            Err(Error::Formula(
                ProductionConditionalFormulaErrorV1::Resource(Resource::Accounting)
            ))
        ));
        assert_eq!(drops.get(), 1);
        assert_eq!((ledger.storage(), ledger.work()), (FLOOR + 19, PREFIX + 1));
    }
}

#[test]
fn v2_damaged_floor_never_refunds_an_error_or_unwind() {
    for scope in [Scope::Scratch, Scope::Retained] {
        for unwind in [false, true] {
            let mut ledger = ledger(1000, 1000);
            let outcome = catch_unwind(AssertUnwindSafe(|| {
                ledger.with_budget(|budget| {
                    scope.run::<()>(budget, 20, |budget| {
                        budget.release_storage(1)?;
                        if unwind {
                            panic!("damaged floor unwind");
                        }
                        Err(Error::Subject("must be overridden"))
                    })
                })
            }));
            if unwind {
                assert!(outcome.is_err());
            } else {
                assert!(matches!(
                    outcome,
                    Ok(Err(Error::Formula(
                        ProductionConditionalFormulaErrorV1::Resource(Resource::Accounting)
                    )))
                ));
            }
            assert_eq!((ledger.storage(), ledger.work()), (FLOOR + 19, PREFIX + 1));
        }
    }
}

#[test]
fn v2_funded_foreign_account_never_returns_success_or_refunds_either_account() {
    for scope in [Scope::Scratch, Scope::Retained] {
        for unwind in [false, true] {
            let mut ledger = ledger(1000, 1000);
            let drops = Cell::new(0);
            let outcome = catch_unwind(AssertUnwindSafe(|| {
                ledger.with_budget(|budget| {
                    let result = catch_unwind(AssertUnwindSafe(|| {
                        scope.run(budget, 20, |budget| {
                            let foreign = Box::leak(Box::new(Work::new(1000)));
                            *budget = Budget::new(foreign, 1000);
                            budget.reserve_storage(FLOOR + 20)?;
                            budget.charge_work(11)?;
                            if unwind {
                                panic!("foreign account unwind");
                            }
                            Ok(InertDrop(&drops))
                        })
                    }));
                    assert_eq!((budget.storage(), budget.work()), (FLOOR + 20, 11));
                    match result {
                        Ok(value) => value,
                        Err(panic) => std::panic::resume_unwind(panic),
                    }
                })
            }));
            if unwind {
                assert!(outcome.is_err());
                assert_eq!(drops.get(), 0);
            } else {
                assert!(matches!(
                    outcome,
                    Ok(Err(Error::Formula(
                        ProductionConditionalFormulaErrorV1::Resource(Resource::Accounting)
                    )))
                ));
                assert_eq!(drops.get(), 1);
            }
            assert_eq!((ledger.storage(), ledger.work()), (FLOOR + 20, PREFIX + 1));
        }
    }
}

#[test]
fn v2_late_codec_cleanup_error_discards_provisional_result_before_return() {
    let fixture = fixtures::Fixture::new();
    let mut ledger = ledger(usize::MAX, usize::MAX);
    let drops = Cell::new(0);
    let codec_floor = Cell::new(0);
    let result = ledger.with_budget(|budget| {
        retention::retain_reservation_using(budget, RETAINED_STORAGE, |budget| {
            with_encoded_native_cpu_input_v1(fixture.input(), budget, |_, _, budget| {
                codec_floor.set(budget.storage());
                budget.release_storage(1).unwrap();
                InertDrop(&drops)
            })
            .map_err(Error::Codec)
        })
    });
    assert!(matches!(
        result,
        Err(Error::Codec(NativeCpuCodecErrorV1::Resource(
            Resource::Accounting
        )))
    ));
    assert_eq!(drops.get(), 1);
    // Codec refuses to refund its damaged floor. Only our intact outer retained
    // reservation is released; no caller can install the provisional result.
    assert_eq!(ledger.storage(), codec_floor.get() - 1 - RETAINED_STORAGE);
    assert!(ledger.storage() > FLOOR);
    assert!(ledger.work() > PREFIX);
}

#[test]
fn v2_error_adapter_preserves_v1_scratch_debits() {
    for refuse in [false, true] {
        let mut old = ledger(1000, 1000);
        let mut new = ledger(1000, 1000);
        let old_result = old.with_budget(|budget| {
            with_scratch(budget, 20, |budget| {
                budget.charge_work(11)?;
                budget.reserve_storage(13)?;
                if refuse {
                    Err(ProductionConditionalFormulaErrorV1::Subject("refusal"))
                } else {
                    Ok(())
                }
            })
        });
        let new_result = new.with_budget(|budget| {
            with_scratch_using::<_, Error>(budget, 20, |budget| {
                budget.charge_work(11)?;
                budget.reserve_storage(13)?;
                if refuse {
                    Err(Error::Subject("refusal"))
                } else {
                    Ok(())
                }
            })
        });
        assert_eq!(old_result.is_ok(), new_result.is_ok());
        assert_eq!(
            (
                old.work(),
                old.storage(),
                old.peak_storage(),
                old.failed_work(),
                old.failed_storage()
            ),
            (
                new.work(),
                new.storage(),
                new.peak_storage(),
                new.failed_work(),
                new.failed_storage()
            )
        );
    }
}
