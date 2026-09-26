//! Account/lifetime components only: no fabricated Request, execution or proof.
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
const EXTRA: usize = 13;

#[derive(Debug)]
struct Witness<'a>(&'a Cell<usize>);
impl Drop for Witness<'_> {
    fn drop(&mut self) {
        self.0.set(self.0.get() + 1);
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

fn peak() -> usize {
    FLOOR + IMPORT_CHECK_STORAGE + RETAINED_STORAGE + retention::PREPARATION_STORAGE
}

fn component<T, E>(
    owner: T,
    budget: &mut Budget<'_>,
    check: impl for<'owner> FnOnce(&'owner T, &mut Budget<'_>) -> Result<(), E>,
    postcheck: impl FnOnce(&mut Budget<'_>) -> Result<(), Error>,
) -> Result<Result<T, E>, ConditionalFormulaImportCheckErrorV2> {
    finish_import_check(with_scratch_using(budget, IMPORT_CHECK_STORAGE, |budget| {
        retention::retain_reservation_using(budget, RETAINED_STORAGE, |budget| {
            with_scratch_using(budget, retention::PREPARATION_STORAGE, |budget| {
                check_imported_owner(owner, budget, check, postcheck)
            })
        })
    }))
}

fn accounting<T, E>(result: &Result<Result<T, E>, ConditionalFormulaImportCheckErrorV2>) {
    assert!(matches!(
        result,
        Err(ConditionalFormulaImportCheckErrorV2(Error::Formula(
            ProductionConditionalFormulaErrorV1::Resource(Resource::Accounting)
        )))
    ));
}

#[test]
fn v2_import_check_borrows_once_and_moves_only_after_postcheck() {
    let mut ledger = ledger(usize::MAX, usize::MAX);
    let drops = Cell::new(0);
    let visits = Cell::new(0);
    let result = ledger.with_budget(|budget| {
        let account = budget.work_ledger_identity_v1();
        component(
            Witness(&drops),
            budget,
            |owner, budget| {
                assert!(std::ptr::eq(owner.0, &drops));
                assert!(budget.work_ledger_identity_v1() == account);
                assert_eq!(budget.storage(), peak());
                visits.set(visits.get() + 1);
                Ok::<_, ()>(())
            },
            |budget| {
                assert_eq!(visits.get(), 1);
                assert_eq!(drops.get(), 0);
                assert!(budget.work_ledger_identity_v1() == account);
                Ok(())
            },
        )
    });
    assert_eq!(ledger.storage(), FLOOR + RETAINED_STORAGE);
    assert_eq!(ledger.work(), PREFIX + 3 + 9);
    assert_eq!(drops.get(), 0);
    drop(result.unwrap().unwrap());
    assert_eq!(drops.get(), 1);
    ledger.with_budget(|budget| budget.release_storage(RETAINED_STORAGE).unwrap());
    assert_eq!(ledger.storage(), FLOOR);
}

#[test]
fn v2_import_check_error_is_nested_until_postchecks_then_drops_owner() {
    let mut ledger = ledger(usize::MAX, usize::MAX);
    let owner_drops = Cell::new(0);
    let error_drops = Cell::new(0);
    let result = ledger.with_budget(|budget| {
        component(
            Witness(&owner_drops),
            budget,
            |_, budget| {
                budget.reserve_storage(EXTRA).unwrap();
                Err(Witness(&error_drops))
            },
            |_| {
                assert_eq!((owner_drops.get(), error_drops.get()), (0, 0));
                Ok(())
            },
        )
    });
    assert!(matches!(&result, Ok(Err(_))));
    assert_eq!((owner_drops.get(), error_drops.get()), (1, 0));
    // No floor reset or retained-owner credit for an opaque callback refusal.
    assert_eq!(ledger.storage(), FLOOR + RETAINED_STORAGE + EXTRA);
    drop(result);
    assert_eq!(error_drops.get(), 1);
}

#[test]
fn v2_import_check_postcheck_error_overrides_callback_error() {
    let mut ledger = ledger(usize::MAX, usize::MAX);
    let owner_drops = Cell::new(0);
    let error_drops = Cell::new(0);
    let result = ledger.with_budget(|budget| {
        component(
            Witness(&owner_drops),
            budget,
            |_, budget| {
                budget.reserve_storage(EXTRA).unwrap();
                Err(Witness(&error_drops))
            },
            |_| Err(Error::Subject("late graph refusal")),
        )
    });
    assert!(matches!(
        result,
        Err(ConditionalFormulaImportCheckErrorV2(Error::Subject(
            "late graph refusal"
        )))
    ));
    assert_eq!((owner_drops.get(), error_drops.get()), (1, 1));
    assert_eq!(ledger.storage(), FLOOR + EXTRA);
}

#[test]
fn v2_import_check_success_cannot_retain_callback_allocation() {
    let mut ledger = ledger(usize::MAX, usize::MAX);
    let drops = Cell::new(0);
    let postchecks = Cell::new(0);
    let result = ledger.with_budget(|budget| {
        component(
            Witness(&drops),
            budget,
            |_, budget| {
                budget.reserve_storage(EXTRA).unwrap();
                Ok::<_, ()>(())
            },
            |_| {
                postchecks.set(postchecks.get() + 1);
                Ok(())
            },
        )
    });
    accounting(&result);
    assert_eq!((drops.get(), postchecks.get()), (1, 0));
    assert_eq!(ledger.storage(), FLOOR + EXTRA);
}

#[test]
fn v2_import_check_lost_floor_and_postcheck_floor_loss_refuse() {
    for late in [false, true] {
        for callback_error in [false, true] {
            let mut ledger = ledger(usize::MAX, usize::MAX);
            let drops = Cell::new(0);
            let postchecks = Cell::new(0);
            let result = ledger.with_budget(|budget| {
                component(
                    Witness(&drops),
                    budget,
                    |_, budget| {
                        if !late {
                            budget.release_storage(1).unwrap();
                        }
                        if callback_error { Err(()) } else { Ok(()) }
                    },
                    |budget| {
                        postchecks.set(postchecks.get() + 1);
                        budget.release_storage(1).unwrap();
                        Ok(())
                    },
                )
            });
            accounting(&result);
            assert_eq!(drops.get(), 1);
            assert_eq!(postchecks.get(), usize::from(late));
            // The damaged preparation reservation is not refunded. Intact
            // surrounding scopes release only their own fixed headers.
            assert_eq!(ledger.storage(), FLOOR + retention::PREPARATION_STORAGE - 1);
        }
    }
}

#[test]
fn v2_import_check_unwind_preserves_callback_charges_and_drops_owner() {
    let mut ledger = ledger(usize::MAX, usize::MAX);
    let drops = Cell::new(0);
    let result = catch_unwind(AssertUnwindSafe(|| {
        ledger.with_budget(|budget| {
            component(
                Witness(&drops),
                budget,
                |_, budget| -> Result<(), ()> {
                    budget.reserve_storage(EXTRA).unwrap();
                    panic!("opaque callback unwind");
                },
                |_| panic!("postcheck must not consume unwinding state"),
            )
        })
    }));
    assert!(result.is_err());
    assert_eq!(drops.get(), 1);
    assert_eq!(ledger.storage(), FLOOR + EXTRA);
}

#[test]
fn v2_import_check_exact_independent_work_and_storage_limits() {
    let work = PREFIX + 3 + 9 + 4 + 11;
    let storage = peak() + EXTRA;
    for (w, s, success) in [
        (work, storage, true),
        (work - 1, storage, false),
        (work, storage - 1, false),
    ] {
        let mut ledger = ledger(w, s);
        let drops = Cell::new(0);
        let result = ledger.with_budget(|budget| {
            component(
                Witness(&drops),
                budget,
                |_, budget| {
                    budget.charge_work(4).unwrap();
                    let result = budget.reserve_storage(EXTRA);
                    if result.is_ok() {
                        budget.release_storage(EXTRA).unwrap();
                    }
                    result
                },
                |budget| budget.charge_work(11).map_err(Error::from),
            )
        });
        assert_eq!(matches!(&result, Ok(Ok(_))), success);
        if success {
            assert_eq!(ledger.peak_storage(), storage);
            assert_eq!(ledger.work(), work);
            assert_eq!(drops.get(), 0);
        } else if w < work {
            assert!(result.is_err());
            assert_eq!(ledger.failed_work(), Some(work));
            assert_eq!(ledger.storage(), FLOOR);
            assert_eq!(drops.get(), 1);
        } else {
            assert!(matches!(&result, Ok(Err(Resource::Storage(_)))));
            assert_eq!(ledger.failed_storage(), Some(storage));
            assert_eq!(ledger.work(), work);
            assert_eq!(ledger.storage(), FLOOR + RETAINED_STORAGE);
            assert_eq!(drops.get(), 1);
        }
        drop(result);
        assert_eq!(drops.get(), 1);
    }
}

#[test]
fn v2_import_check_prepayment_refuses_before_callback() {
    for (w, s) in [(PREFIX, usize::MAX), (usize::MAX, FLOOR)] {
        let mut ledger = ledger(w, s);
        let drops = Cell::new(0);
        let result = ledger.with_budget(|budget| {
            component(
                Witness(&drops),
                budget,
                |_, _| -> Result<(), ()> { panic!("unpaid callback") },
                |_| panic!("unpaid postcheck"),
            )
        });
        assert!(result.is_err());
        assert_eq!(ledger.storage(), FLOOR);
        assert_eq!(drops.get(), 1);
    }
}

#[test]
fn v2_import_check_preserves_prior_denials() {
    let w = PREFIX + 100;
    let s = peak() + EXTRA;
    let mut ledger = ledger(w, s);
    ledger.with_budget(|budget| {
        assert!(budget.charge_work(w).is_err());
        assert!(budget.reserve_storage(s).is_err());
    });
    let first = (ledger.failed_work(), ledger.failed_storage());
    let result =
        ledger.with_budget(|budget| component((), budget, |_, _| Ok::<_, ()>(()), |_| Ok(())));
    assert!(matches!(result, Ok(Ok(()))));
    assert_eq!((ledger.failed_work(), ledger.failed_storage()), first);
    assert_eq!(ledger.work(), PREFIX + 3 + 9);
}

#[test]
fn v2_import_check_foreign_account_never_refunds_or_postchecks() {
    for unwind in [false, true] {
        let mut ledger = ledger(usize::MAX, usize::MAX);
        let drops = Cell::new(0);
        let result = catch_unwind(AssertUnwindSafe(|| {
            ledger.with_budget(|budget| {
                let result = catch_unwind(AssertUnwindSafe(|| {
                    component(
                        Witness(&drops),
                        budget,
                        |_, budget| {
                            let foreign = Box::leak(Box::new(Work::new(usize::MAX)));
                            *budget = Budget::new(foreign, usize::MAX);
                            budget.reserve_storage(peak()).unwrap();
                            if unwind {
                                panic!("foreign callback unwind");
                            }
                            Ok::<_, ()>(())
                        },
                        |_| panic!("foreign ledger postcheck"),
                    )
                }));
                assert_eq!(budget.storage(), peak());
                match result {
                    Ok(value) => value,
                    Err(payload) => std::panic::resume_unwind(payload),
                }
            })
        }));
        if unwind {
            assert!(result.is_err());
        } else {
            accounting(&result.unwrap());
        }
        assert_eq!(drops.get(), 1);
        assert_eq!(ledger.storage(), peak());
        assert_eq!(ledger.work(), PREFIX + 3 + 9);
    }
}

#[test]
fn v2_import_check_outer_refusal_stays_opaque_to_refund_classifiers() {
    for cause in [Error::Subject("late subject"), Resource::Accounting.into()] {
        let result = finish_import_check::<(), ()>(Err(cause));
        let error = result.unwrap_err();
        assert!(std::error::Error::source(&error).is_none());
    }
}

#[test]
fn v2_import_check_late_cpu_scope_failure_discards_nested_result() {
    let fixture = fixtures::Fixture::new();
    for refuse in [false, true] {
        let mut ledger = ledger(usize::MAX, usize::MAX);
        let owner_drops = Cell::new(0);
        let error_drops = Cell::new(0);
        let terminal = Cell::new(0);
        let result = ledger.with_budget(|budget| {
            finish_import_check(with_scratch_using(budget, IMPORT_CHECK_STORAGE, |budget| {
                retention::retain_reservation_using(budget, RETAINED_STORAGE, |budget| {
                    crate::portable_reference_v1::with_replayed_output_writes_v1(
                        fixture.input().replay,
                        budget,
                        |_, budget| {
                            let checked = with_scratch_using(
                                budget,
                                retention::PREPARATION_STORAGE,
                                |budget| {
                                    check_imported_owner(
                                        Witness(&owner_drops),
                                        budget,
                                        |_, _| {
                                            if refuse {
                                                Err(Witness(&error_drops))
                                            } else {
                                                Ok(())
                                            }
                                        },
                                        |_| Ok(()),
                                    )
                                },
                            );
                            assert!(checked.is_ok());
                            assert_eq!((owner_drops.get(), error_drops.get()), (0, 0));
                            budget.release_storage(1).unwrap();
                            terminal.set(budget.storage());
                            checked
                        },
                    )
                    .map_err(|error| {
                        Error::Correspondence(ConditionalReferenceErrorV1::ProofExecution(
                            error.to_string(),
                        ))
                    })?
                })
            }))
        });
        assert!(matches!(
            &result,
            Err(ConditionalFormulaImportCheckErrorV2(Error::Correspondence(
                _
            )))
        ));
        assert_eq!(owner_drops.get(), 1);
        assert_eq!(error_drops.get(), usize::from(refuse));
        assert_eq!(
            ledger.storage(),
            terminal.get() - RETAINED_STORAGE - IMPORT_CHECK_STORAGE
        );
        assert!(ledger.storage() > FLOOR);
    }
}

#[test]
fn v2_import_check_damaged_floor_unwind_keeps_terminal_charge() {
    let mut ledger = ledger(usize::MAX, usize::MAX);
    let drops = Cell::new(0);
    let result = catch_unwind(AssertUnwindSafe(|| {
        ledger.with_budget(|budget| {
            component(
                Witness(&drops),
                budget,
                |_, budget| -> Result<(), ()> {
                    budget.release_storage(1).unwrap();
                    panic!("damaged callback floor");
                },
                |_| panic!("unwinding postcheck"),
            )
        })
    }));
    assert!(result.is_err());
    assert_eq!(drops.get(), 1);
    assert_eq!(ledger.storage(), FLOOR + retention::PREPARATION_STORAGE - 1);
}

// This typechecks access from the parent module and the unit-only higher-ranked
// callback with a real caller-provided Request. It is never a fixture constructor.
#[allow(dead_code)]
fn genuine_caller_shape(
    request: &Request<'_>,
    input: &DecodedNativeCpuInputV1,
    signature: &InertFunctionalRefinementReceiptSignatureV2,
    accepted: &FunctionalRefinementImportPolicyV2,
    budget: &mut Budget<'_>,
) -> Result<Result<RetainedProductionConditionalFormulaV2, ()>, ConditionalFormulaImportCheckErrorV2>
{
    crate::conditional_ranked_formulas_v1::import_and_check_conditional_ranked_formula_v2(
        request,
        input,
        signature,
        accepted,
        budget,
        |execution, _| {
            let _ = execution.signed_receipt_wire();
            let _ = execution.report();
            Ok(())
        },
    )
}
