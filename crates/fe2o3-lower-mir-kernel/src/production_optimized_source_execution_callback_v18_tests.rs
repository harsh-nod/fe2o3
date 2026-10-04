#[repr(align(256))]
struct ExecutionOwnedCaptureV18<'a> {
    bytes: [u8; 2048],
    drops: &'a std::cell::Cell<usize>,
    deny: Option<&'a ScopedSourceCleanupV29>,
    panic: Option<Box<u64>>,
}

impl Drop for ExecutionOwnedCaptureV18<'_> {
    fn drop(&mut self) {
        assert_eq!(self.bytes[0], 0x48);
        self.drops.set(self.drops.get() + 1);
        if let Some(cleanup) = self.deny {
            cleanup.deny_refund();
        }
        if let Some(payload) = self.panic.take() {
            std::panic::resume_unwind(payload);
        }
    }
}

#[allow(dead_code)]
struct ExecutionRecipeMirrorV18 {
    root: usize,
    instance: usize,
    block: SemanticBlockIdV1,
    kind: ProductionOptimizedExecutionKindV18,
    input: fe2o3_kernel_ir::CanonicalKirOperationCoordinateV1,
    output: Option<fe2o3_kernel_ir::CanonicalKirOperationCoordinateV1>,
}

fn execution_owned_header_oracle_v18<T, E, F>(_: &F) -> usize {
    use std::mem::{align_of, size_of};
    use std::panic::AssertUnwindSafe;
    type Entry<F> = (Vec<ExecutionRecipeMirrorV18>, usize, F);
    type Capture<'a, 'work, F> = (
        &'a ProductionOptimizedSourceCorrespondenceV18<'a>,
        &'a ProductionSourceOwnedViewV18<'a>,
        &'a mut ArgumentBudgetV1<'work>,
        &'a std::cell::Cell<usize>,
        &'a std::cell::Cell<usize>,
        F,
        bool,
    );
    type Construct<'a, 'work, F> = (
        &'a ProductionOptimizedSourceCorrespondenceV18<'a>,
        &'a ProductionSourceOwnedViewV18<'a>,
        &'a mut ArgumentBudgetV1<'work>,
        &'a F,
        usize,
    );
    type Invoke<'a, 'work, F> = (
        F,
        &'a ProductionOptimizedExecutionRecipesV18<'a>,
        &'a mut ArgumentBudgetV1<'work>,
    );
    type OwnedConstruct<'a, F> = (
        F,
        &'a ProductionSourceOwnedViewV18<'a>,
        &'a ProductionOptimizedSourceCorrespondenceV18<'a>,
    );
    type Payload = Box<dyn std::any::Any + Send>;
    let cleanup = size_of::<[Option<Payload>; 2]>()
        + size_of::<AssertUnwindSafe<[Option<Payload>; 2]>>()
        + 2 * size_of::<Payload>()
        + 2 * size_of::<std::thread::Result<()>>()
        + size_of::<AssertUnwindSafe<Payload>>()
        + size_of::<std::ops::Range<usize>>()
        + size_of::<usize>()
        + size_of::<bool>();
    2 * size_of::<Capture<'_, '_, F>>()
        + 2 * align_of::<Capture<'_, '_, F>>()
        + size_of::<AssertUnwindSafe<Capture<'_, '_, F>>>()
        + size_of::<Construct<'_, '_, F>>()
        + size_of::<AssertUnwindSafe<Construct<'_, '_, F>>>()
        + 2 * size_of::<OwnedConstruct<'_, F>>()
        + 2 * align_of::<OwnedConstruct<'_, F>>()
        + size_of::<AssertUnwindSafe<OwnedConstruct<'_, F>>>()
        + size_of::<Entry<F>>()
        + 2 * size_of::<SourceOwnedResultV18<Entry<F>>>()
        + size_of::<std::thread::Result<SourceOwnedResultV18<Entry<F>>>>()
        + size_of::<AssertUnwindSafe<Entry<F>>>()
        + size_of::<std::thread::Result<()>>()
        + size_of::<Invoke<'_, '_, F>>()
        + size_of::<AssertUnwindSafe<Invoke<'_, '_, F>>>()
        + size_of::<ProductionOptimizedExecutionRecipesV18<'_>>()
        + size_of::<Vec<ExecutionRecipeMirrorV18>>()
        + 2 * size_of::<SourceOwnedResultV18<Vec<ExecutionRecipeMirrorV18>>>()
        + size_of::<std::thread::Result<Result<T, E>>>()
        + size_of::<Result<T, E>>()
        + size_of::<AssertUnwindSafe<Result<T, E>>>()
        + size_of::<fe2o3_kernel_ir::CanonicalKernelIrWorkLedgerIdentityV1>()
        + 12 * size_of::<usize>()
        + cleanup
}

fn execution_owned_fixture_v18(
    consume: impl for<'scope, 'work> FnOnce(
        &ProductionOptimizedSourceCorrespondenceV18<'scope>,
        &mut ArgumentBudgetV1<'work>,
    ) -> SourceOwnedResultV18<()>,
) -> (SourceOwnedResultV18<()>, usize) {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(OPTIMIZED_SOURCE_WORK_LIMIT_V18);
    let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
    budget.reserve_storage(MODULE_FLOOR).unwrap();
    let prepared = prepared_optimized_module_v18(ModuleFixture::Mixed, &mut budget);
    let entered = std::cell::Cell::new(false);
    let result = with_actual_optimized_source_v18(prepared, &mut budget, |view, budget| {
        entered.set(true);
        consume(view, budget)
    });
    assert!(entered.get(), "the genuine optimized fixture must enter");
    (result, budget.storage())
}

fn execution_main_attempt_header_oracle_v18<F>(_: &F) -> usize {
    use std::mem::{align_of, size_of};
    type Run<'a, F> = (
        F,
        &'a ProductionSourceOwnedViewV18<'a>,
        &'a ProductionOptimizedSourceCorrespondenceV18<'a>,
    );
    type Value<F> = (Vec<ExecutionRecipeMirrorV18>, F);
    type Error = ProductionSourceOwnedViewErrorV18;
    type Frame<'a, 'work, F> = (
        &'a ScopedSourceCleanupV29,
        &'a mut ArgumentBudgetV1<'work>,
        usize,
        Run<'a, F>,
    );
    type Capture<'a, 'work, F> = (
        &'a mut Option<Run<'a, F>>,
        &'a mut ArgumentBudgetV1<'work>,
        &'a Result<(), ArgumentResourceV1>,
        &'a mut Option<usize>,
        &'a usize,
    );
    size_of::<Run<'_, F>>()
        + align_of::<Run<'_, F>>()
        + size_of::<Option<Run<'_, F>>>()
        + align_of::<Option<Run<'_, F>>>()
        + size_of::<Frame<'_, '_, F>>()
        + size_of::<Capture<'_, '_, F>>()
        + size_of::<std::panic::AssertUnwindSafe<Capture<'_, '_, F>>>()
        + size_of::<std::thread::Result<Result<Value<F>, Error>>>()
        + size_of::<Result<Value<F>, Error>>()
        + size_of::<Value<F>>()
        + size_of::<Error>()
        + size_of::<Box<dyn std::any::Any + Send>>()
        + size_of::<fe2o3_kernel_ir::CanonicalKernelIrWorkLedgerIdentityV1>()
        + 8 * size_of::<usize>()
        + size_of::<Option<usize>>()
        + size_of::<bool>()
        + size_of::<Result<usize, ArgumentResourceV1>>()
        + size_of::<Result<(), ArgumentResourceV1>>()
        + source_owned_finish_header_oracle_v26::<Value<F>, Error>()
}

fn execution_has_main_attempt_header_v18() -> bool {
    let cleanup = ScopedSourceCleanupV29 {
        denied: std::cell::Cell::new(false),
        fault: std::cell::RefCell::new(None),
        fault_storage: std::cell::Cell::new(None),
        fault_skip: std::cell::Cell::new(0),
    };
    let mut work = CanonicalKernelIrWorkBudgetV1::new(32 + 2 + 1 + 4);
    let mut budget = ArgumentBudgetV1::new(&mut work, 0);
    let result = scoped_source_attempt_v29(&cleanup, &mut budget, 0, |_| {
        Ok::<(), ArgumentResourceV1>(())
    });
    assert_eq!((budget.work(), budget.storage()), (32 + 2 + 1 + 4, 0));
    assert!(!cleanup.is_denied());
    match result {
        Err(ArgumentResourceV1::Storage(error)) => {
            assert_eq!(error.limit(), 0);
            assert!(error.actual() > 0);
            true
        }
        Ok(()) => false,
        other => panic!("unexpected shared attempt convention: {other:?}"),
    }
}

#[test]
fn execution_main_helper_header_refusal_settles_only_accepted_local_credit() {
    let main_header = execution_has_main_attempt_header_v18();
    for deny in [false, true] {
        let drops = std::cell::Cell::new(0);
        let refusal = std::cell::Cell::new(None);
        let (result, remaining) = execution_owned_fixture_v18(|view, budget| {
            let source = view.original_source(budget)?;
            let input = view.input_inventory(budget)?;
            let operations = input.operations().len();
            let recipes = input
                .operations()
                .iter()
                .filter(|row| matches!(row.operation.kind, OperationKind::Execution(_)))
                .count();
            assert!(recipes > 0);
            let owned = ExecutionOwnedCaptureV18 {
                bytes: [0x48; 2048],
                drops: &drops,
                deny: deny.then_some(source.cleanup),
                panic: None,
            };
            let consume = move |_: &ProductionOptimizedExecutionRecipesV18<'_>,
                                _: &mut ArgumentBudgetV1<'_>| {
                std::hint::black_box(&owned);
                panic!("no construction storage remains for callback invocation");
                #[allow(unreachable_code)]
                Ok::<_, ProductionSourceOwnedViewErrorV18>(())
            };
            let header =
                execution_owned_header_oracle_v18::<(), ProductionSourceOwnedViewErrorV18, _>(
                    &consume,
                );
            let helper = execution_main_attempt_header_oracle_v18(&consume);
            let returned = size_of::<Result<(), ProductionSourceOwnedViewErrorV18>>();
            let filler = MODULE_LIMIT - budget.storage() - returned - header;
            budget.reserve_storage(filler)?;
            let before = (budget.work(), budget.storage());
            let Err(ProductionSourceOwnedViewErrorV18::Resource(ArgumentResourceV1::Storage(
                error,
            ))) = view.with_execution_recipes_v18(budget, consume)
            else {
                panic!("shared construction must refuse after local header acceptance");
            };
            assert_eq!(error.limit(), MODULE_LIMIT);
            assert_eq!(budget.failed_storage(), Some(error.actual()));
            if main_header {
                assert_eq!(error.actual(), MODULE_LIMIT + helper);
                assert_eq!(
                    budget.work(),
                    before.0 + 1 + 32 + 2 + 1 + 4,
                    "MAIN generic header refuses before recipe census"
                );
            } else {
                // PRIMARY's unchanged historical helper has no generic header;
                // its first closed construction allocation refuses instead.
                assert_eq!(
                    error.actual(),
                    MODULE_LIMIT + recipes * size_of::<ExecutionRecipeMirrorV18>()
                );
                assert_eq!(budget.work(), before.0 + 1 + operations + 1);
            }
            assert_eq!(
                budget.storage(),
                before.1 + returned + if deny { header } else { 0 }
            );
            assert_eq!(drops.get(), 1);
            assert_eq!(source.cleanup.is_denied(), deny);
            refusal.set(Some(error));
            release_execution_unit_result_v18(budget)?;
            budget.release_storage(filler)?;
            Ok(())
        });
        assert!(
            matches!(result, Err(ProductionSourceOwnedViewErrorV18::Resource(ArgumentResourceV1::Storage(error))) if Some(error) == refusal.get())
        );
        if deny {
            assert!(remaining > MODULE_FLOOR);
        } else {
            assert_eq!(remaining, MODULE_FLOOR);
        }
    }
}

#[test]
fn execution_owned_header_rows_and_public_result_credit_are_independent() {
    let drops = std::cell::Cell::new(0);
    let calls = std::cell::Cell::new(0);
    let (result, remaining) = execution_owned_fixture_v18(|view, budget| {
        budget.reserve_storage(37)?;
        let floor = budget.storage();
        let expected = std::cell::Cell::new(0);
        let owned = ExecutionOwnedCaptureV18 {
            bytes: [0x48; 2048],
            drops: &drops,
            deny: None,
            panic: None,
        };
        let expected_ref = &expected;
        let calls_ref = &calls;
        let consume = move |recipes: &ProductionOptimizedExecutionRecipesV18<'_>,
                            budget: &mut ArgumentBudgetV1<'_>| {
            std::hint::black_box(&owned);
            calls_ref.set(calls_ref.get() + 1);
            let rows = recipes.len(budget)?;
            assert!(rows > 0);
            assert_eq!(
                budget.storage(),
                floor
                    + expected_ref.get()
                    + size_of::<Result<(), ProductionSourceOwnedViewErrorV18>>()
                    + rows * size_of::<ExecutionRecipeMirrorV18>()
            );
            Ok::<_, ProductionSourceOwnedViewErrorV18>(())
        };
        assert!(std::mem::size_of_val(&consume) >= 2048);
        assert_eq!(std::mem::align_of_val(&consume), 256);
        expected.set(execution_owned_header_oracle_v18::<
            (),
            ProductionSourceOwnedViewErrorV18,
            _,
        >(&consume));
        view.with_execution_recipes_v18(budget, consume)?;
        assert_eq!((calls.get(), drops.get()), (1, 1));
        assert_eq!(
            budget.storage(),
            floor + size_of::<Result<(), ProductionSourceOwnedViewErrorV18>>()
        );
        release_execution_unit_result_v18(budget)?;
        budget.release_storage(37)?;
        Ok(())
    });
    result.unwrap();
    assert_eq!(remaining, MODULE_FLOOR);
}

#[test]
fn execution_owned_header_one_short_disposes_before_any_recipe_census() {
    for deny in [false, true] {
        let drops = std::cell::Cell::new(0);
        let calls = std::cell::Cell::new(0);
        let refusal = std::cell::Cell::new(None);
        let (result, remaining) = execution_owned_fixture_v18(|view, budget| {
            let source = view.original_source(budget)?;
            let owned = ExecutionOwnedCaptureV18 {
                bytes: [0x48; 2048],
                drops: &drops,
                deny: deny.then_some(source.cleanup),
                panic: None,
            };
            let calls_ref = &calls;
            let consume = move |_: &ProductionOptimizedExecutionRecipesV18<'_>,
                                _: &mut ArgumentBudgetV1<'_>| {
                std::hint::black_box(&owned);
                calls_ref.set(calls_ref.get() + 1);
                Ok::<_, ProductionSourceOwnedViewErrorV18>(())
            };
            let header =
                execution_owned_header_oracle_v18::<(), ProductionSourceOwnedViewErrorV18, _>(
                    &consume,
                );
            let returned = size_of::<Result<(), ProductionSourceOwnedViewErrorV18>>();
            let filler = MODULE_LIMIT - budget.storage() - returned - header + 1;
            budget.reserve_storage(filler)?;
            let before = (budget.work(), budget.storage());
            let Err(ProductionSourceOwnedViewErrorV18::Resource(ArgumentResourceV1::Storage(
                error,
            ))) = view.with_execution_recipes_v18(budget, consume)
            else {
                panic!("actual owned header must refuse before construction");
            };
            assert_eq!(
                (error.actual(), error.limit()),
                (MODULE_LIMIT + 1, MODULE_LIMIT)
            );
            assert_eq!(budget.failed_storage(), Some(MODULE_LIMIT + 1));
            assert_eq!(
                (budget.work(), budget.storage()),
                (before.0 + 1, before.1 + returned)
            );
            assert_eq!((calls.get(), drops.get()), (0, 1));
            assert_eq!(source.cleanup.is_denied(), deny);
            refusal.set(Some(error));
            release_execution_unit_result_v18(budget)?;
            budget.release_storage(filler)?;
            Ok(())
        });
        assert!(
            matches!(result, Err(ProductionSourceOwnedViewErrorV18::Resource(ArgumentResourceV1::Storage(error))) if Some(error) == refusal.get())
        );
        if deny {
            assert!(remaining > MODULE_FLOOR);
        } else {
            assert_eq!(remaining, MODULE_FLOOR);
        }
    }
}

#[test]
fn execution_early_query_refusal_disposes_owned_capture_without_new_credit() {
    let drops = std::cell::Cell::new(0);
    let calls = std::cell::Cell::new(0);
    let (result, remaining) = execution_owned_fixture_v18(|view, budget| {
        let source = view.original_source(budget)?;
        let _ = source.retain_query::<()>(Err(ProductionSourceOwnedViewErrorV18::Binding(
            "execution early selected",
        )));
        let owned = ExecutionOwnedCaptureV18 {
            bytes: [0x48; 2048],
            drops: &drops,
            deny: Some(source.cleanup),
            panic: None,
        };
        let before = (budget.work(), budget.storage());
        let calls_ref = &calls;
        let result = view.with_execution_recipes_v18(budget, move |_, _| {
            std::hint::black_box(&owned);
            calls_ref.set(calls_ref.get() + 1);
            Ok::<_, ProductionSourceOwnedViewErrorV18>(())
        });
        assert!(matches!(
            result,
            Err(ProductionSourceOwnedViewErrorV18::Binding(
                "execution early selected"
            ))
        ));
        assert_eq!((budget.work(), budget.storage()), before);
        assert_eq!((calls.get(), drops.get()), (0, 1));
        assert!(source.cleanup.is_denied());
        Ok(())
    });
    assert!(matches!(
        result,
        Err(ProductionSourceOwnedViewErrorV18::Binding(
            "execution early selected"
        ))
    ));
    assert!(remaining > MODULE_FLOOR);
}

#[test]
fn execution_foreign_ledger_entry_never_charges_or_refunds_the_foreign_budget() {
    let drops = std::cell::Cell::new(0);
    let (result, remaining) = execution_owned_fixture_v18(|view, budget| {
        let source = view.original_source(budget)?;
        let owned = ExecutionOwnedCaptureV18 {
            bytes: [0x48; 2048],
            drops: &drops,
            deny: None,
            panic: None,
        };
        let mut work = CanonicalKernelIrWorkBudgetV1::new(OPTIMIZED_SOURCE_WORK_LIMIT_V18);
        let mut foreign = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
        foreign.reserve_storage(budget.storage())?;
        let before = (foreign.work(), foreign.storage());
        let result = view.with_execution_recipes_v18(&mut foreign, move |_, _| {
            std::hint::black_box(&owned);
            panic!("foreign ledger cannot invoke execution callback");
            #[allow(unreachable_code)]
            Ok::<_, ProductionSourceOwnedViewErrorV18>(())
        });
        assert!(matches!(
            result,
            Err(ProductionSourceOwnedViewErrorV18::Resource(
                ArgumentResourceV1::Accounting
            ))
        ));
        assert_eq!((foreign.work(), foreign.storage()), before);
        assert_eq!(drops.get(), 1);
        assert!(source.cleanup.is_denied());
        Ok(())
    });
    assert!(matches!(
        result,
        Err(ProductionSourceOwnedViewErrorV18::Resource(
            ArgumentResourceV1::Accounting
        ))
    ));
    assert!(remaining > MODULE_FLOOR);
}

#[test]
fn execution_construction_work_failure_disposes_before_owned_header_refund() {
    for deny in [false, true] {
        let drops = std::cell::Cell::new(0);
        let refusal = std::cell::Cell::new(None);
        let (result, remaining) = execution_owned_fixture_v18(|view, budget| {
            let source = view.original_source(budget)?;
            let operations = view.input_inventory(budget)?.operations().len();
            assert!(operations > 0);
            // Admit the initial query and the 32 bounded disposal attempts plus
            // seven finalizer steps; the original recipe census must refuse.
            budget.charge_work(OPTIMIZED_SOURCE_WORK_LIMIT_V18 - budget.work() - 1 - (32 + 7))?;
            let floor = budget.storage();
            let owned = ExecutionOwnedCaptureV18 {
                bytes: [0x48; 2048],
                drops: &drops,
                deny: deny.then_some(source.cleanup),
                panic: None,
            };
            let Err(ProductionSourceOwnedViewErrorV18::Resource(ArgumentResourceV1::Work(error))) =
                view.with_execution_recipes_v18(budget, move |_, _| {
                    std::hint::black_box(&owned);
                    panic!("failed initial recipe census cannot invoke callback");
                    #[allow(unreachable_code)]
                    Ok::<_, ProductionSourceOwnedViewErrorV18>(())
                })
            else {
                panic!("initial recipe work must refuse");
            };
            assert_eq!(
                (error.actual(), error.limit()),
                (
                    OPTIMIZED_SOURCE_WORK_LIMIT_V18 + operations,
                    OPTIMIZED_SOURCE_WORK_LIMIT_V18
                )
            );
            assert_eq!(budget.work(), OPTIMIZED_SOURCE_WORK_LIMIT_V18);
            assert_eq!(drops.get(), 1);
            if deny {
                assert!(
                    budget.storage()
                        > floor + size_of::<Result<(), ProductionSourceOwnedViewErrorV18>>()
                );
            } else {
                assert_eq!(
                    budget.storage(),
                    floor + size_of::<Result<(), ProductionSourceOwnedViewErrorV18>>()
                );
            }
            assert_eq!(source.cleanup.is_denied(), deny);
            refusal.set(Some(error));
            release_execution_unit_result_v18(budget)?;
            Ok(())
        });
        assert!(
            matches!(result, Err(ProductionSourceOwnedViewErrorV18::Resource(ArgumentResourceV1::Work(error))) if Some(error) == refusal.get())
        );
        if deny {
            assert!(remaining > MODULE_FLOOR);
        } else {
            assert_eq!(remaining, MODULE_FLOOR);
        }
    }
}

#[test]
fn execution_rejected_view_disposes_owned_capture_inside_the_checked_boundary() {
    for deny in [false, true] {
        let drops = std::cell::Cell::new(0);
        let (result, remaining) = execution_owned_fixture_v18(|view, budget| {
            let source = view.original_source(budget)?;
            let floor = budget.storage();
            let owned = ExecutionOwnedCaptureV18 {
                bytes: [0x48; 2048],
                drops: &drops,
                deny: deny.then_some(source.cleanup),
                panic: None,
            };
            view.test_execution_recipe_refuse_before_invocation_v18();
            let result = view.with_execution_recipes_v18(budget, move |_, _| {
                std::hint::black_box(&owned);
                panic!("selected view refusal must precede invocation");
                #[allow(unreachable_code)]
                Ok::<_, ProductionSourceOwnedViewErrorV18>(())
            });
            assert!(matches!(
                result,
                Err(ProductionSourceOwnedViewErrorV18::Binding(
                    "execution recipe test pre-invocation refusal"
                ))
            ));
            assert_eq!(drops.get(), 1);
            if deny {
                assert!(
                    budget.storage()
                        > floor + size_of::<Result<(), ProductionSourceOwnedViewErrorV18>>()
                );
            } else {
                assert_eq!(
                    budget.storage(),
                    floor + size_of::<Result<(), ProductionSourceOwnedViewErrorV18>>()
                );
            }
            assert_eq!(source.cleanup.is_denied(), deny);
            release_execution_unit_result_v18(budget)?;
            Ok(())
        });
        assert!(matches!(
            result,
            Err(ProductionSourceOwnedViewErrorV18::Binding(
                "execution recipe test pre-invocation refusal"
            ))
        ));
        if deny {
            assert!(remaining > MODULE_FLOOR);
        } else {
            assert_eq!(remaining, MODULE_FLOOR);
        }
    }
}

#[test]
fn execution_owned_callback_keeps_typed_body_and_single_drop_panic_chronology() {
    // A raw body panic and a destructor panic are distinct cases, never two
    // simultaneous unwinding destructors.
    for mode in 0..3 {
        for deny in [false, true] {
            let drops = std::cell::Cell::new(0);
            let (result, remaining) = execution_owned_fixture_v18(|view, budget| {
                let source = view.original_source(budget)?;
                if mode == 2 {
                    budget.reserve_storage(size_of::<u64>())?;
                }
                let floor = budget.storage();
                let owned = ExecutionOwnedCaptureV18 {
                    bytes: [0x48; 2048],
                    drops: &drops,
                    deny: deny.then_some(source.cleanup),
                    panic: (mode == 2).then(|| Box::new(0x1748_u64)),
                };
                let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    view.with_execution_recipes_v18(budget, move |recipes, budget| {
                        std::hint::black_box(&owned);
                        assert!(recipes.len(budget)? > 0);
                        if mode == 1 {
                            budget.reserve_storage(size_of::<u64>())?;
                            std::panic::resume_unwind(Box::new(0x1748_u64));
                        }
                        if mode == 0 {
                            return Err(ProductionSourceOwnedViewErrorV18::Binding(
                                "execution selected consumer",
                            ));
                        }
                        Ok(())
                    })
                }));
                assert_eq!(drops.get(), 1);
                if mode == 0 {
                    assert!(matches!(
                        caught.unwrap(),
                        Err(ProductionSourceOwnedViewErrorV18::Binding(
                            "execution selected consumer"
                        ))
                    ));
                } else {
                    assert_eq!(*caught.unwrap_err().downcast::<u64>().unwrap(), 0x1748);
                }
                let expected = floor
                    + size_of::<Result<(), ProductionSourceOwnedViewErrorV18>>()
                    + if mode == 1 { size_of::<u64>() } else { 0 };
                if deny {
                    assert!(budget.storage() > expected);
                } else {
                    assert_eq!(budget.storage(), expected);
                }
                if mode != 0 {
                    budget.release_storage(size_of::<u64>())?;
                }
                release_execution_unit_result_v18(budget)?;
                assert_eq!(source.cleanup.is_denied(), deny);
                Ok(())
            });
            if deny {
                assert!(matches!(
                    result,
                    Err(ProductionSourceOwnedViewErrorV18::Resource(
                        ArgumentResourceV1::Accounting
                    ))
                ));
                assert!(remaining > MODULE_FLOOR);
            } else {
                result.unwrap();
                assert_eq!(remaining, MODULE_FLOOR);
            }
        }
    }
}
