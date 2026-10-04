use super::tests::{LAYOUTS, coordinate, fixture, with_checked};
use super::*;
use fe2o3_kernel_ir::{Module, OperationKind, ValueId};

const AMPLE: usize = 1 << 40;

thread_local! {
    static ENTERED: Cell<usize> = const { Cell::new(0) };
}

fn guarded_fixture(count: usize) -> Module {
    let mut module = fixture(count);
    for function in module.functions.iter_mut().skip(1) {
        let operations = &mut function.body.as_mut().unwrap().blocks[0].operations;
        let OperationKind::Store {
            pointer,
            value,
            access,
        } = operations[4].kind
        else {
            unreachable!()
        };
        operations[4].kind = OperationKind::GuardedStore {
            pointer,
            predicate: ValueId(3),
            value,
            access,
        };
    }
    module
}

fn noop(_: &PendingCanonicalGlobalAccessesV18<'_, '_>, _: &mut Budget<'_>) -> Result<(), Failure> {
    ENTERED.set(ENTERED.get() + 1);
    Ok(())
}

#[test]
fn pending_guarded_v87_opt_in_preserves_three_operands_and_legacy_refusal() {
    for count in [1, 3] {
        with_checked(&guarded_fixture(count), |checked, budget| {
            with_pending_canonical_ranked_source_roles_v18(
                checked,
                LAYOUTS,
                budget,
                |pending, budget| {
                    pending.with_pending_global_accesses_v18(
                        pending.owner,
                        budget,
                        |view, budget| {
                            for function in 1..=count {
                                assert!(
                                    view.operation(
                                        pending.owner,
                                        coordinate(function as u32, 4),
                                        budget
                                    )?
                                    .is_none()
                                );
                            }
                            Ok(())
                        },
                    )?;
                    pending.with_pending_global_accesses_v87(
                        pending.owner,
                        budget,
                        |view, budget| {
                            for function in 1..=count {
                                let before = budget.work();
                                let operation = view
                                    .operation(
                                        pending.owner,
                                        coordinate(function as u32, 4),
                                        budget,
                                    )?
                                    .unwrap();
                                assert_eq!(budget.work() - before, 16);
                                let expected = &pending.owner.module().functions[function]
                                    .body
                                    .as_ref()
                                    .unwrap()
                                    .blocks[0]
                                    .operations[4];
                                assert!(std::ptr::eq(operation, expected));
                                assert!(matches!(
                                    operation.kind,
                                    OperationKind::GuardedStore {
                                        pointer: ValueId(6),
                                        predicate: ValueId(3),
                                        value: ValueId(2),
                                        ..
                                    }
                                ));
                                assert!(operation.results.is_empty());
                            }
                            for (location, supported) in
                                [(coordinate(1, 0), true), (coordinate(0, 0), false)]
                            {
                                let before = budget.work();
                                assert_eq!(
                                    view.operation(pending.owner, location, budget)?.is_some(),
                                    supported
                                );
                                assert_eq!(budget.work() - before, 16);
                            }
                            assert!(!view.memory_safety_is_complete());
                            assert!(!view.native_stage_coverage_is_complete());
                            assert!(!view.grants_artifact_or_launch_authority());
                            Ok(())
                        },
                    )
                },
            )
            .unwrap();
        });
    }
}

#[test]
fn pending_guarded_v87_exact_census_work_and_storage_refuse_before_callback() {
    for storage_short in [false, true] {
        for work_short in [false, true] {
            with_checked(&guarded_fixture(2), |checked, budget| {
                let result = with_pending_canonical_ranked_source_roles_v18(
                    checked,
                    LAYOUTS,
                    budget,
                    |pending, budget| {
                        let start = budget.work();
                        pending.with_pending_global_accesses_v87(pending.owner, budget, noop)?;
                        let required_work = budget.work() - start;
                        let required_storage =
                            headers(std::mem::size_of_val(&noop), std::mem::align_of_val(&noop))?
                                + crate::KirPlironGraphV18::pending_guarded_scan_headers_v87()?;
                        let floor = budget.storage();
                        let extra = AMPLE - floor - required_storage + usize::from(storage_short);
                        budget.reserve_storage(extra)?;
                        budget.charge_work(
                            AMPLE - budget.work() - required_work + usize::from(work_short),
                        )?;
                        let before = budget.work();
                        ENTERED.set(0);
                        let checked =
                            pending.with_pending_global_accesses_v87(pending.owner, budget, noop);
                        assert_eq!(ENTERED.get(), usize::from(!storage_short && !work_short));
                        assert_eq!(budget.storage(), floor + extra);
                        if storage_short {
                            assert!(matches!(
                                checked,
                                Err(Failure::Resource(Resource::Storage(_)))
                            ));
                            // The established scope entry's two-unit debit precedes frame reservation.
                            assert_eq!(budget.work(), before + 2);
                        } else if work_short {
                            assert!(matches!(checked, Err(Failure::Resource(Resource::Work(_)))));
                        } else {
                            checked.unwrap();
                            assert_eq!(budget.work(), AMPLE);
                        }
                        if storage_short || work_short {
                            let stopped = budget.work();
                            for _ in 0..2 {
                                assert!(
                                    pending
                                        .with_pending_global_accesses_v87(
                                            pending.owner,
                                            budget,
                                            noop
                                        )
                                        .is_err()
                                );
                                assert_eq!(budget.work(), stopped);
                            }
                        }
                        budget.release_storage(extra)?;
                        Ok(())
                    },
                );
                assert_eq!(result.is_ok(), !storage_short && !work_short);
            });
        }
    }
}

#[test]
fn pending_guarded_v87_foreign_owner_and_ledger_are_zero_debit_sticky() {
    for foreign_ledger in [false, true] {
        with_checked(&guarded_fixture(1), |checked, budget| {
            with_checked(&guarded_fixture(1), |other, other_budget| {
                let foreign = other.inventory(other_budget).unwrap().owner();
                let result = with_pending_canonical_ranked_source_roles_v18(
                    checked,
                    LAYOUTS,
                    budget,
                    |pending, budget| {
                        pending.with_pending_global_accesses_v87(
                            pending.owner,
                            budget,
                            |view, budget| {
                                let before = (budget.work(), budget.storage());
                                if foreign_ledger {
                                    let mut work =
                                        fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(AMPLE);
                                    let mut foreign_budget = Budget::new(&mut work, AMPLE);
                                    foreign_budget.reserve_storage(budget.storage())?;
                                    assert!(matches!(
                                        view.operation(
                                            pending.owner,
                                            coordinate(1, 4),
                                            &mut foreign_budget
                                        ),
                                        Err(Failure::Resource(Resource::Accounting))
                                    ));
                                    assert_eq!(foreign_budget.work(), 0);
                                } else {
                                    assert!(matches!(
                                        view.operation(foreign, coordinate(1, 4), budget),
                                        Err(Failure::ExactGraph)
                                    ));
                                }
                                assert_eq!((budget.work(), budget.storage()), before);
                                for _ in 0..2 {
                                    assert!(
                                        view.operation(pending.owner, coordinate(1, 4), budget)
                                            .is_err()
                                    );
                                    assert_eq!((budget.work(), budget.storage()), before);
                                }
                                Ok(())
                            },
                        )
                    },
                );
                assert!(result.is_err());
            });
        });
    }
}

#[test]
fn pending_guarded_v87_fixed_native_extractor_refuses_shape_metadata_and_operand_mutants() {
    for fault in 0..12 {
        with_checked(&guarded_fixture(2), |checked, budget| {
            let result = with_pending_canonical_ranked_source_roles_v18(
                checked,
                LAYOUTS,
                budget,
                |pending, budget| {
                    pending.with_pending_global_accesses_v87(
                        pending.owner,
                        budget,
                        |view, budget| {
                            // The second effect is never queried: the complete extra census must still reject it.
                            assert!(
                                view.operation(pending.owner, coordinate(1, 4), budget)?
                                    .is_some()
                            );
                            view.graph.test_guarded_extraction_v87(fault, budget);
                            Ok(())
                        },
                    )
                },
            );
            if fault == 0 || fault == 7 || fault == 8 {
                result.unwrap();
            } else {
                assert!(result.is_err());
            }
        });
    }
}

#[test]
fn pending_guarded_v87_callback_error_and_unwind_drop_captures_before_refund() {
    struct Dropped<'a>(&'a Cell<usize>);
    impl Drop for Dropped<'_> {
        fn drop(&mut self) {
            self.0.set(self.0.get() + 1);
        }
    }
    for panic in [false, true] {
        let drops = Cell::new(0);
        with_checked(&guarded_fixture(1), |checked, budget| {
            with_pending_canonical_ranked_source_roles_v18(
                checked,
                LAYOUTS,
                budget,
                |pending, budget| {
                    let floor = budget.storage();
                    let capture = Dropped(&drops);
                    let owner = pending.owner;
                    let result = pending.with_pending_global_accesses_v87(
                        owner,
                        budget,
                        move |view, budget| {
                            let _capture = capture;
                            assert!(view.operation(owner, coordinate(1, 4), budget)?.is_some());
                            budget.reserve_storage(17)?;
                            if panic {
                                panic!("guarded callback sentinel");
                            }
                            Err(Failure::Callback("guarded callback sentinel"))
                        },
                    );
                    assert_eq!(drops.get(), 1);
                    assert_eq!(budget.storage(), floor);
                    if panic {
                        assert!(matches!(result, Err(Failure::Panicked)));
                    } else {
                        assert!(matches!(
                            result,
                            Err(Failure::Callback("guarded callback sentinel"))
                        ));
                    }
                    Ok(())
                },
            )
            .unwrap();
        });
    }
}
