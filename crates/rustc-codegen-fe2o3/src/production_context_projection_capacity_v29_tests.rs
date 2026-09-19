use super::*;
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;
use std::cell::Cell;

#[derive(Clone, Copy)]
enum Allocation {
    Normal,
    Extra(usize),
    Fail,
    Panic,
}

#[derive(Clone, Copy)]
struct Plan {
    actions: [Allocation; 3],
    attempts: usize,
    capacities: [usize; 3],
}

thread_local! {
    static PLAN: Cell<Option<Plan>> = const { Cell::new(None) };
}

struct AllocationGuard(Option<Plan>);

impl AllocationGuard {
    fn install(actions: [Allocation; 3]) -> Self {
        Self(PLAN.replace(Some(Plan {
            actions,
            attempts: 0,
            capacities: [0; 3],
        })))
    }

    fn state(&self) -> Plan {
        PLAN.get().unwrap()
    }
}

impl Drop for AllocationGuard {
    fn drop(&mut self) {
        PLAN.set(self.0);
    }
}

pub(super) fn allocation_request(count: usize) -> Result<usize, ProductionContextRootErrorV29> {
    let Some(mut plan) = PLAN.get() else {
        return Ok(count);
    };
    let action = plan.actions[plan.attempts];
    plan.attempts += 1;
    PLAN.set(Some(plan));
    match action {
        Allocation::Normal => Ok(count),
        Allocation::Extra(extra) => count.checked_add(extra).ok_or(Resource::Arithmetic.into()),
        Allocation::Fail => Err(Resource::Allocation.into()),
        Allocation::Panic => std::panic::panic_any(String::from("projection allocator panic")),
    }
}

pub(super) fn record_capacity(bytes: usize) {
    if let Some(mut plan) = PLAN.get() {
        plan.capacities[plan.attempts - 1] = bytes;
        PLAN.set(Some(plan));
    }
}

fn with_source<R>(test: impl FnOnce(&RetainedExecutionSourceV29<'_>) -> R) -> R {
    with_execution_source(|source, _, _| test(source))
}

fn with_execution_source<R>(
    test: impl FnOnce(
        &RetainedExecutionSourceV29<'_>,
        &ProductionSemanticSsaOwnerV1,
        &ProductionSourceLaunchRosterV1,
    ) -> R,
) -> R {
    let (ssa, launch, receipt) = RetainedContextEntriesV29::projection_test_fixture_v29();
    let mut work = Work::new(100_000);
    let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, 1 << 20);
    let source = receipt
        .materialization_source_v29(ssa.source_semantic(), &mut budget)
        .unwrap()
        .unwrap();
    test(&source, &ssa, &launch)
}

fn requested_rows(source: &RetainedExecutionSourceV29<'_>) -> [usize; 3] {
    [
        projection_bytes::<ProductionContextRootInputV29<'_>>(source.roots().len()).unwrap(),
        projection_bytes::<ProductionScopeCallableCandidateV29>(source.classes().len()).unwrap(),
        projection_bytes::<ProductionScopeEventCandidateV29>(source.events().len()).unwrap(),
    ]
}

fn measured_projection(source: &RetainedExecutionSourceV29<'_>) -> (usize, [usize; 3]) {
    let guard = AllocationGuard::install([Allocation::Extra(2); 3]);
    let mut work = Work::new(100_000);
    let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, 1 << 20);
    with_projected_execution_source_v29(source, &mut budget, |_, _| Ok(())).unwrap();
    assert_eq!(budget.storage(), 0);
    assert_eq!(
        budget.work(),
        18 + 32 * source.roots().len() + 12 * source.classes().len() + 8 * source.events().len()
    );
    (budget.work(), guard.state().capacities)
}

#[test]
fn projection_capacity_preserves_consumer_output_on_success_error_and_panic() {
    with_source(|source| {
        for mode in 0..3 {
            let guard = AllocationGuard::install([Allocation::Extra(2); 3]);
            let mut work = Work::new(100_000);
            let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, 1 << 20);
            budget.reserve_storage(7).unwrap();
            let result = catch_unwind(AssertUnwindSafe(|| {
                with_projected_execution_source_v29(source, &mut budget, |input, budget| {
                    assert_eq!(guard.state().attempts, 3);
                    assert_eq!(
                        budget.storage(),
                        7 + guard.state().capacities.iter().sum::<usize>()
                    );
                    assert_eq!(input.roots.len(), source.roots().len());
                    assert_eq!(input.classes.len(), source.classes().len());
                    assert_eq!(input.events.len(), source.events().len());
                    for (projected, retained) in input.roots.iter().zip(source.roots()) {
                        assert!(std::ptr::eq(
                            projected.helper_arguments,
                            retained.helper_operands()
                        ));
                    }
                    budget.reserve_storage(11)?;
                    match mode {
                        0 => Ok(42),
                        1 => Err(ProductionContextRootErrorV29::Arguments),
                        _ => std::panic::panic_any(String::from("consumer output panic")),
                    }
                })
            }));
            match mode {
                0 => assert_eq!(result.unwrap(), Ok(42)),
                1 => assert_eq!(
                    result.unwrap(),
                    Err(ProductionContextRootErrorV29::Arguments)
                ),
                _ => assert_eq!(
                    *result.unwrap_err().downcast::<String>().unwrap(),
                    "consumer output panic"
                ),
            }
            assert_eq!(budget.storage(), 18);
            assert_eq!(
                budget.peak_storage(),
                18 + guard.state().capacities.iter().sum::<usize>()
            );
            assert!(budget.failed_storage().is_none());
            assert!(budget.work() > 0);
        }
    });
}

#[test]
fn projection_capacity_returns_owned_output_after_backing_cleanup() {
    with_source(|source| {
        let _guard = AllocationGuard::install([Allocation::Extra(2); 3]);
        let mut work = Work::new(100_000);
        let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, 1 << 20);
        budget.reserve_storage(7).unwrap();
        let output = with_projected_execution_source_v29(source, &mut budget, |_, budget| {
            budget.reserve_storage(11)?;
            Ok(Box::new([42_u8; 11]))
        })
        .unwrap();
        assert_eq!(*output, [42; 11]);
        assert_eq!(budget.storage(), 18);
        drop(output);
        assert_eq!(budget.storage(), 18);
        budget.release_storage(11).unwrap();
        assert_eq!(budget.storage(), 7);
    });
}

#[test]
fn projection_capacity_prepays_before_any_allocation() {
    with_source(|source| {
        let guard = AllocationGuard::install([Allocation::Panic; 3]);
        let requested: usize = requested_rows(source).iter().sum();
        let mut work = Work::new(100_000);
        let mut budget =
            CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, requested + 6);
        budget.reserve_storage(7).unwrap();
        let error = with_projected_execution_source_v29(
            source,
            &mut budget,
            |_, _| -> Result<(), ProductionContextRootErrorV29> {
                panic!("consumer reached without prepayment")
            },
        )
        .unwrap_err();
        assert!(matches!(
            error,
            ProductionContextRootErrorV29::Resource(Resource::Storage { .. })
        ));
        assert_eq!(guard.state().attempts, 0);
        assert_eq!(budget.storage(), 7);
        assert_eq!(budget.peak_storage(), 7);
        assert!(budget.failed_storage().is_some());
        assert!(budget.work() > 0);
    });
}

#[test]
fn projection_capacity_partial_allocation_failure_and_panic_restore_exact_floor() {
    with_source(|source| {
        let requested = requested_rows(source);
        for stop in 0..3 {
            for panic in [false, true] {
                let mut actions = [Allocation::Extra(2); 3];
                actions[stop] = if panic {
                    Allocation::Panic
                } else {
                    Allocation::Fail
                };
                let guard = AllocationGuard::install(actions);
                let mut work = Work::new(100_000);
                let mut budget =
                    CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, 1 << 20);
                budget.reserve_storage(7).unwrap();
                let result = catch_unwind(AssertUnwindSafe(|| {
                    with_projected_execution_source_v29(
                        source,
                        &mut budget,
                        |_, _| -> Result<(), ProductionContextRootErrorV29> {
                            panic!("partial allocation reached consumer")
                        },
                    )
                }));
                if panic {
                    assert_eq!(
                        *result.unwrap_err().downcast::<String>().unwrap(),
                        "projection allocator panic"
                    );
                } else {
                    assert_eq!(
                        result.unwrap(),
                        Err(ProductionContextRootErrorV29::Resource(
                            Resource::Allocation
                        ))
                    );
                }
                let state = guard.state();
                assert_eq!(state.attempts, stop + 1);
                let accepted_extra: usize =
                    (0..stop).map(|i| state.capacities[i] - requested[i]).sum();
                assert_eq!(budget.storage(), 7);
                assert_eq!(
                    budget.peak_storage(),
                    7 + requested.iter().sum::<usize>() + accepted_extra
                );
                assert!(budget.failed_storage().is_none());
                assert!(budget.work() > 0);
            }
        }
    });
}

#[test]
fn projection_capacity_denied_surplus_is_never_refunded() {
    with_source(|source| {
        let requested = requested_rows(source);
        let (_, measured) = measured_projection(source);
        let extra: [usize; 3] = std::array::from_fn(|i| measured[i] - requested[i]);
        assert!(extra.iter().all(|&bytes| bytes > 0));
        for stop in 0..3 {
            let guard = AllocationGuard::install([Allocation::Extra(2); 3]);
            let mut work = Work::new(100_000);
            let limit =
                7 + requested.iter().sum::<usize>() + extra[..=stop].iter().sum::<usize>() - 1;
            let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, limit);
            budget.reserve_storage(7).unwrap();
            let error = with_projected_execution_source_v29(
                source,
                &mut budget,
                |_, _| -> Result<(), ProductionContextRootErrorV29> {
                    panic!("denied surplus reached consumer")
                },
            )
            .unwrap_err();
            assert!(matches!(
                error,
                ProductionContextRootErrorV29::Resource(Resource::Storage { .. })
            ));
            assert_eq!(guard.state().attempts, stop + 1);
            assert_eq!(budget.storage(), 7);
            assert_eq!(
                budget.peak_storage(),
                7 + requested.iter().sum::<usize>() + extra[..stop].iter().sum::<usize>()
            );
            assert!(budget.failed_storage().is_some());
            assert!(budget.work() > 0);
        }
    });
}

#[test]
fn projection_capacity_obeys_exact_and_one_short_work_and_storage() {
    with_source(|source| {
        let (exact_work, measured) = measured_projection(source);
        let exact_storage: usize = measured.iter().sum();
        for (work_limit, storage_limit, success) in [
            (exact_work, exact_storage, true),
            (exact_work - 1, exact_storage, false),
            (exact_work, exact_storage - 1, false),
            (0, exact_storage, false),
            (exact_work, 0, false),
        ] {
            let guard = AllocationGuard::install([Allocation::Extra(2); 3]);
            let mut work = Work::new(work_limit);
            let mut budget =
                CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, storage_limit + 7);
            budget.reserve_storage(7).unwrap();
            let mut visits = 0;
            let result = with_projected_execution_source_v29(source, &mut budget, |_, _| {
                visits += 1;
                Ok(())
            });
            if success {
                result.unwrap();
                assert_eq!(visits, 1);
                assert_eq!(budget.work(), exact_work);
                assert_eq!(budget.peak_storage(), exact_storage + 7);
            } else if work_limit < exact_work {
                assert!(matches!(
                    result,
                    Err(ProductionContextRootErrorV29::Resource(
                        Resource::Work { .. }
                    ))
                ));
                assert_eq!(guard.state().attempts, 0);
                assert_eq!(visits, 0);
            } else {
                assert!(matches!(
                    result,
                    Err(ProductionContextRootErrorV29::Resource(
                        Resource::Storage { .. }
                    ))
                ));
                assert_eq!(visits, 0);
            }
            assert_eq!(budget.storage(), 7);
        }
    });
}

#[test]
fn projection_capacity_foreign_ledger_is_untouched_and_panic_payload_survives() {
    with_source(|source| {
        for panic in [false, true] {
            let _guard = AllocationGuard::install([Allocation::Extra(2); 3]);
            let mut work = Work::new(100_000);
            let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, 1 << 20);
            budget.reserve_storage(7).unwrap();
            let mut foreign = CanonicalKernelIrVerificationResourceBudgetV1::new(
                Box::leak(Box::new(Work::new(100_000))),
                101,
            );
            foreign.reserve_storage(101).unwrap();
            foreign.charge_work(5).unwrap();
            let mut foreign = Some(foreign);
            let result = catch_unwind(AssertUnwindSafe(|| {
                with_projected_execution_source_v29(source, &mut budget, |_, budget| {
                    let _original = std::mem::replace(budget, foreign.take().unwrap());
                    if panic {
                        std::panic::panic_any(String::from("foreign ledger panic"));
                    }
                    Ok(())
                })
            }));
            if panic {
                assert_eq!(
                    *result.unwrap_err().downcast::<String>().unwrap(),
                    "foreign ledger panic"
                );
            } else {
                assert_eq!(
                    result.unwrap(),
                    Err(ProductionContextRootErrorV29::Resource(
                        Resource::Accounting
                    ))
                );
            }
            assert_eq!(budget.work(), 5);
            assert_eq!(budget.storage(), 101);
            assert_eq!(budget.peak_storage(), 101);
            assert!(budget.failed_storage().is_none());
        }
    });
}

#[test]
fn projection_capacity_nested_root_visitor_preserves_foreign_ledger() {
    with_execution_source(|source, ssa, launch| {
        for panic in [false, true] {
            let _guard = AllocationGuard::install([Allocation::Extra(2); 3]);
            let mut work = Work::new(100_000);
            let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, 1 << 20);
            budget.reserve_storage(7).unwrap();
            let mut foreign = CanonicalKernelIrVerificationResourceBudgetV1::new(
                Box::leak(Box::new(Work::new(100_000))),
                101,
            );
            foreign.reserve_storage(101).unwrap();
            foreign.charge_work(5).unwrap();
            let mut foreign = Some(foreign);
            let mut visits = 0;
            let result = catch_unwind(AssertUnwindSafe(|| {
                with_projected_execution_source_v29(source, &mut budget, |input, budget| {
                    with_checked_execution_source_v29(ssa, launch, input, budget, |_, budget| {
                        visits += 1;
                        let _original = std::mem::replace(budget, foreign.take().unwrap());
                        if panic {
                            std::panic::panic_any(String::from("nested ledger panic"));
                        }
                        Ok(())
                    })
                })
            }));
            if panic {
                assert_eq!(
                    *result.unwrap_err().downcast::<String>().unwrap(),
                    "nested ledger panic"
                );
            } else {
                assert_eq!(
                    result.unwrap(),
                    Err(ProductionContextRootErrorV29::Resource(
                        Resource::Accounting
                    ))
                );
            }
            assert_eq!(visits, 1);
            assert_eq!(budget.work(), 5);
            assert_eq!(budget.storage(), 101);
            assert_eq!(budget.peak_storage(), 101);
            assert!(budget.failed_storage().is_none());
        }
    });
}

#[test]
fn projection_capacity_arithmetic_and_allocation_overflow_preserve_receipts() {
    assert_eq!(
        projection_bytes::<u64>(usize::MAX),
        Err(Resource::Arithmetic.into())
    );
    let _guard =
        AllocationGuard::install([Allocation::Extra(2), Allocation::Normal, Allocation::Normal]);
    let mut work = Work::new(100_000);
    let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, 100);
    budget.reserve_storage(7).unwrap();
    let mut charged = usize::MAX;
    assert_eq!(
        projection_rows::<u64>(1, &mut budget, &mut charged),
        Err(Resource::Arithmetic.into())
    );
    assert_eq!(charged, usize::MAX);
    assert_eq!(budget.storage(), 7);

    with_source(|source| {
        let _guard = AllocationGuard::install([Allocation::Extra(usize::MAX); 3]);
        let mut work = Work::new(100_000);
        let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, 1 << 20);
        budget.reserve_storage(7).unwrap();
        let result = with_projected_execution_source_v29(
            source,
            &mut budget,
            |_, _| -> Result<(), ProductionContextRootErrorV29> {
                panic!("overflow reached consumer")
            },
        );
        assert_eq!(
            result,
            Err(ProductionContextRootErrorV29::Resource(
                Resource::Arithmetic
            ))
        );
        assert_eq!(budget.storage(), 7);
    });
}

#[test]
fn projection_allocation_plan_restores_after_nested_unwind() {
    let outer = AllocationGuard::install([Allocation::Extra(2); 3]);
    assert_eq!(allocation_request(3).unwrap(), 5);
    let result = catch_unwind(|| {
        let _inner = AllocationGuard::install([Allocation::Panic; 3]);
        allocation_request(3).unwrap();
    });
    assert!(result.is_err());
    assert_eq!(outer.state().attempts, 1);
    assert_eq!(allocation_request(3).unwrap(), 5);
    assert_eq!(outer.state().attempts, 2);
}
