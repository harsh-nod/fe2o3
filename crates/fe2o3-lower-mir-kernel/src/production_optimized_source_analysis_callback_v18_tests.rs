#[repr(align(256))]
struct OptimizedAnalysisOwnedCaptureV18<'a> {
    bytes: [u8; 2048],
    drops: &'a std::cell::Cell<usize>,
    deny: Option<&'a ScopedSourceCleanupV29>,
}

impl Drop for OptimizedAnalysisOwnedCaptureV18<'_> {
    fn drop(&mut self) {
        assert_eq!(self.bytes[0], 0x27);
        self.drops.set(self.drops.get() + 1);
        if let Some(cleanup) = self.deny {
            cleanup.deny_refund();
        }
    }
}

fn optimized_analysis_callback_fixture_v18(
    consume: impl for<'scope, 'work> FnOnce(
        &ProductionSourceCorrespondenceV18<'scope>,
        &ProductionOptimizedSourceCorrespondenceV18<'scope>,
        &mut ArgumentBudgetV1<'work>,
    ) -> SourceOwnedResultV18<()>,
) -> (SourceOwnedResultV18<()>, usize) {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(OPTIMIZED_SOURCE_WORK_LIMIT_V18);
    let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
    budget.reserve_storage(MODULE_FLOOR).unwrap();
    let prepared = scalar_payload_prepared_from_v18(folding_source_owner_v18, &mut budget);
    let entered = std::cell::Cell::new(false);
    let result = prepared.with_source_consumer_v18(&mut budget, |source, budget| {
        let attempted =
            source.with_checked_optimization_v18(budget, |original, optimized, budget| {
                entered.set(true);
                consume(original, optimized, budget)?;
                Ok::<_, ProductionSourceOwnedViewErrorV18>(((), 0))
            });
        assert!(
            entered.get(),
            "genuine optimizer must reach the test: {:?}",
            attempted.as_ref().err()
        );
        match attempted {
            Ok((owner, (), _)) => drop(owner),
            Err(error) => assert!(
                source.guard.first.get().is_some() || source.cleanup.is_denied(),
                "fixture must not swallow an unrelated optimizer failure: {error:?}"
            ),
        }
        Ok::<_, ProductionSourceOwnedViewErrorV18>(())
    });
    (result, budget.storage())
}

#[test]
fn optimized_analysis_owned_capture_header_is_exact_and_refusal_drops_before_settlement() {
    for (short, deny_drop) in [(false, false), (true, false), (true, true)] {
        let invoked = std::cell::Cell::new(0);
        let drops = std::cell::Cell::new(0);
        let refusal = std::cell::Cell::new(None);
        let (result, retained) =
            optimized_analysis_callback_fixture_v18(|original, optimized, budget| {
                let owned = OptimizedAnalysisOwnedCaptureV18 {
                    bytes: [0x27; 2048],
                    drops: &drops,
                    deny: deny_drop.then_some(original.source.cleanup),
                };
                let called = &invoked;
                let consume = move |_: &mut ProductionOptimizedSourceAnalysisV18<'_>,
                                    _: &mut ArgumentBudgetV1<'_>| {
                    std::hint::black_box(&owned);
                    called.set(called.get() + 1);
                    Ok::<_, ProductionSourceOwnedViewErrorV18>(())
                };
                assert!(std::mem::size_of_val(&consume) >= 2048);
                assert_eq!(std::mem::align_of_val(&consume), 256);
                let expected = optimized_analysis_header_oracle_v18::<
                    (),
                    ProductionSourceOwnedViewErrorV18,
                    _,
                >(&consume);
                assert_eq!(
                    optimized_source_consumer_resources_v18::analysis_headers::<
                        (),
                        ProductionSourceOwnedViewErrorV18,
                        _,
                    >(&consume)?,
                    expected
                );
                let filler = MODULE_LIMIT - budget.storage() - expected + usize::from(short);
                budget.reserve_storage(filler)?;
                let floor = budget.storage();
                let actual = original.with_optimized_analysis_v18(optimized, budget, consume);
                assert_eq!(drops.get(), 1);
                assert_eq!(budget.storage(), floor);
                if short {
                    let Err(ProductionSourceOwnedViewErrorV18::Resource(
                        ArgumentResourceV1::Storage(error),
                    )) = actual
                    else {
                        panic!("full owned header must refuse before invocation");
                    };
                    assert_eq!(invoked.get(), 0);
                    assert_eq!(
                        (error.actual(), error.limit()),
                        (MODULE_LIMIT + 1, MODULE_LIMIT)
                    );
                    assert_eq!(budget.failed_storage(), Some(MODULE_LIMIT + 1));
                    assert_eq!(budget.peak_storage(), floor);
                    assert_eq!(original.source.cleanup.is_denied(), deny_drop);
                    refusal.set(Some(error));
                } else {
                    actual?;
                    assert_eq!(invoked.get(), 1);
                    assert!(budget.failed_storage().is_none());
                    assert_eq!(budget.peak_storage(), MODULE_LIMIT);
                }
                // Only the test-owned filler is removed after observing the boundary.
                budget.release_storage(filler)?;
                Ok(())
            });
        if short {
            assert!(
                matches!(result, Err(ProductionSourceOwnedViewErrorV18::Resource(ArgumentResourceV1::Storage(error))) if Some(error) == refusal.get())
            );
        } else {
            result.unwrap();
        }
        assert_eq!(drops.get(), 1);
        if deny_drop {
            assert!(retained > MODULE_FLOOR);
        } else {
            assert_eq!(retained, MODULE_FLOOR);
        }
    }
}

#[test]
fn optimized_analysis_paid_header_work_refusal_settles_only_accepted_credit() {
    for deny_drop in [false, true] {
        let invoked = std::cell::Cell::new(0);
        let drops = std::cell::Cell::new(0);
        let refusal = std::cell::Cell::new(None);
        let (result, retained) =
            optimized_analysis_callback_fixture_v18(|original, optimized, budget| {
                // Query the same immutable endpoints to locate the entry-work
                // boundary, without changing any analysis or storage state.
                let query_start = budget.work();
                original.query(budget)?;
                assert!(std::ptr::eq(
                    optimized.input_inventory(budget)?,
                    original.inventory
                ));
                let _ = optimized.output_inventory(budget)?;
                budget.charge_work(2)?;
                assert!(std::ptr::eq(
                    optimized.original_source(budget)?,
                    original.source
                ));
                let query_work = budget.work() - query_start;
                let final_charge = 1 + SOURCE_REFERENCE_PAYLOAD_ATTEMPTS_V29;
                budget.charge_work(
                    OPTIMIZED_SOURCE_WORK_LIMIT_V18 - budget.work() - query_work - final_charge + 1,
                )?;
                let owned = OptimizedAnalysisOwnedCaptureV18 {
                    bytes: [0x27; 2048],
                    drops: &drops,
                    deny: deny_drop.then_some(original.source.cleanup),
                };
                let called = &invoked;
                let consume = move |_: &mut ProductionOptimizedSourceAnalysisV18<'_>,
                                    _: &mut ArgumentBudgetV1<'_>| {
                    std::hint::black_box(&owned);
                    called.set(called.get() + 1);
                    Ok::<_, ProductionSourceOwnedViewErrorV18>(())
                };
                let header = optimized_analysis_header_oracle_v18::<
                    (),
                    ProductionSourceOwnedViewErrorV18,
                    _,
                >(&consume);
                let floor = budget.storage();
                let Err(ProductionSourceOwnedViewErrorV18::Resource(ArgumentResourceV1::Work(
                    error,
                ))) = original.with_optimized_analysis_v18(optimized, budget, consume)
                else {
                    panic!("the post-header atomic work charge must refuse");
                };
                assert_eq!(
                    (error.actual(), error.limit()),
                    (
                        OPTIMIZED_SOURCE_WORK_LIMIT_V18 + 1,
                        OPTIMIZED_SOURCE_WORK_LIMIT_V18
                    )
                );
                assert_eq!(
                    budget.work(),
                    OPTIMIZED_SOURCE_WORK_LIMIT_V18 - final_charge + 1
                );
                assert_eq!(
                    budget.failed_work(),
                    Some(OPTIMIZED_SOURCE_WORK_LIMIT_V18 + 1)
                );
                assert_eq!(budget.failed_storage(), None);
                assert_eq!((invoked.get(), drops.get()), (0, 1));
                assert_eq!(budget.storage(), floor + if deny_drop { header } else { 0 });
                assert_eq!(original.source.cleanup.is_denied(), deny_drop);
                refusal.set(Some(error));
                Ok(())
            });
        assert!(matches!(result,
            Err(ProductionSourceOwnedViewErrorV18::Resource(ArgumentResourceV1::Work(error)))
                if Some(error) == refusal.get()));
        assert_eq!(drops.get(), 1);
        if deny_drop {
            assert!(retained > MODULE_FLOOR);
        } else {
            assert_eq!(retained, MODULE_FLOOR);
        }
    }
}

#[test]
fn optimized_analysis_owned_capture_drop_denial_keeps_the_paid_analysis_frame() {
    let drops = std::cell::Cell::new(0);
    let invoked = std::cell::Cell::new(0);
    let expected = std::cell::Cell::new(0);
    let (result, retained) =
        optimized_analysis_callback_fixture_v18(|original, optimized, budget| {
            let owned = OptimizedAnalysisOwnedCaptureV18 {
                bytes: [0x27; 2048],
                drops: &drops,
                deny: Some(original.source.cleanup),
            };
            let called = &invoked;
            let last = &expected;
            let actual =
                original.with_optimized_analysis_v18(optimized, budget, move |_, budget| {
                    std::hint::black_box(&owned);
                    called.set(called.get() + 1);
                    last.set(budget.storage());
                    Ok::<_, ProductionSourceOwnedViewErrorV18>(())
                });
            assert!(matches!(
                actual,
                Err(ProductionSourceOwnedViewErrorV18::Resource(
                    ArgumentResourceV1::Accounting
                ))
            ));
            assert_eq!((invoked.get(), drops.get()), (1, 1));
            assert_eq!(budget.storage(), expected.get());
            assert!(original.source.cleanup.is_denied());
            Ok(())
        });
    assert!(matches!(
        result,
        Err(ProductionSourceOwnedViewErrorV18::Resource(
            ArgumentResourceV1::Accounting
        ))
    ));
    assert!(retained >= expected.get());
}

#[test]
fn optimized_analysis_owned_capture_preserves_selected_error_and_raw_payload() {
    for panic in [false, true] {
        for deny_drop in [false, true] {
            let drops = std::cell::Cell::new(0);
            let invoked = std::cell::Cell::new(0);
            let (result, retained) =
                optimized_analysis_callback_fixture_v18(|original, optimized, budget| {
                    let owned = OptimizedAnalysisOwnedCaptureV18 {
                        bytes: [0x27; 2048],
                        drops: &drops,
                        deny: deny_drop.then_some(original.source.cleanup),
                    };
                    let floor = budget.storage();
                    let last = std::cell::Cell::new(0);
                    let called = &invoked;
                    let paid = &last;
                    let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        original.with_optimized_analysis_v18(
                            optimized,
                            budget,
                            move |_, budget| -> SourceOwnedResultV18<()> {
                                std::hint::black_box(&owned);
                                called.set(called.get() + 1);
                                if panic {
                                    budget.reserve_storage(std::mem::size_of::<u64>())?;
                                }
                                paid.set(budget.storage());
                                if panic {
                                    std::panic::resume_unwind(Box::new(0x1741_u64));
                                }
                                Err(ProductionSourceOwnedViewErrorV18::Binding(
                                    "owned analysis selected error",
                                ))
                            },
                        )
                    }));
                    assert_eq!((invoked.get(), drops.get()), (1, 1));
                    if panic {
                        assert_eq!(*caught.unwrap_err().downcast::<u64>().unwrap(), 0x1741);
                    } else {
                        assert!(matches!(
                            caught.unwrap(),
                            Err(ProductionSourceOwnedViewErrorV18::Binding(
                                "owned analysis selected error"
                            ))
                        ));
                    }
                    let payload = if panic { std::mem::size_of::<u64>() } else { 0 };
                    assert_eq!(
                        budget.storage(),
                        if deny_drop {
                            last.get()
                        } else {
                            floor + payload
                        }
                    );
                    if payload != 0 {
                        budget.release_storage(payload)?;
                    }
                    assert_eq!(original.source.cleanup.is_denied(), deny_drop);
                    Ok(())
                });
            if deny_drop {
                assert!(matches!(
                    result,
                    Err(ProductionSourceOwnedViewErrorV18::Resource(
                        ArgumentResourceV1::Accounting
                    ))
                ));
                assert!(retained > MODULE_FLOOR);
            } else {
                result.unwrap();
                assert_eq!(retained, MODULE_FLOOR);
            }
        }
    }
}

#[test]
fn optimized_analysis_early_query_refusal_owns_and_drops_the_rejected_capture() {
    let drops = std::cell::Cell::new(0);
    let invoked = std::cell::Cell::new(0);
    let (result, retained) =
        optimized_analysis_callback_fixture_v18(|original, optimized, budget| {
            let _: SourceOwnedResultV18<()> = original.retain_query(Err(
                ProductionSourceOwnedViewErrorV18::Binding("early analysis query refusal"),
            ));
            assert!(!original.source.cleanup.is_denied());
            let owned = OptimizedAnalysisOwnedCaptureV18 {
                bytes: [0x27; 2048],
                drops: &drops,
                deny: Some(original.source.cleanup),
            };
            let before = (budget.work(), budget.storage());
            let called = &invoked;
            let actual = original.with_optimized_analysis_v18(optimized, budget, move |_, _| {
                std::hint::black_box(&owned);
                called.set(called.get() + 1);
                Ok::<_, ProductionSourceOwnedViewErrorV18>(())
            });
            assert!(matches!(
                actual,
                Err(ProductionSourceOwnedViewErrorV18::Binding(
                    "early analysis query refusal"
                ))
            ));
            assert_eq!((invoked.get(), drops.get()), (0, 1));
            assert_eq!((budget.work(), budget.storage()), before);
            assert!(original.source.cleanup.is_denied());
            Ok(())
        });
    assert!(matches!(
        result,
        Err(ProductionSourceOwnedViewErrorV18::Binding(
            "early analysis query refusal"
        ))
    ));
    assert!(retained > MODULE_FLOOR);
}

#[test]
fn optimized_analysis_owned_capture_rejects_foreign_ledger_and_same_ledger_wrong_slot() {
    for wrong_slot in [false, true] {
        let drops = std::cell::Cell::new(0);
        let invoked = std::cell::Cell::new(0);
        let (result, retained) =
            optimized_analysis_callback_fixture_v18(|original, optimized, budget| {
                let owned = OptimizedAnalysisOwnedCaptureV18 {
                    bytes: [0x27; 2048],
                    drops: &drops,
                    deny: None,
                };
                let called = &invoked;
                let consume = move |_: &mut ProductionOptimizedSourceAnalysisV18<'_>,
                                    _: &mut ArgumentBudgetV1<'_>| {
                    std::hint::black_box(&owned);
                    called.set(called.get() + 1);
                    Ok::<_, ProductionSourceOwnedViewErrorV18>(())
                };
                let before = (budget.work(), budget.storage());
                let meter = Box::leak(Box::new(CanonicalKernelIrWorkBudgetV1::new(
                    OPTIMIZED_SOURCE_WORK_LIMIT_V18,
                )));
                let mut foreign = ArgumentBudgetV1::new(meter, MODULE_LIMIT);
                foreign.reserve_storage(before.1)?;
                if wrong_slot {
                    std::mem::swap(budget, &mut foreign);
                }
                let foreign_before = (foreign.work(), foreign.storage());
                let actual = original.with_optimized_analysis_v18(optimized, &mut foreign, consume);
                assert!(matches!(
                    actual,
                    Err(ProductionSourceOwnedViewErrorV18::Resource(
                        ArgumentResourceV1::Accounting
                    ))
                ));
                assert_eq!((foreign.work(), foreign.storage()), foreign_before);
                if wrong_slot {
                    std::mem::swap(budget, &mut foreign);
                }
                assert_eq!((budget.work(), budget.storage()), before);
                assert_eq!((invoked.get(), drops.get()), (0, 1));
                assert!(original.source.cleanup.is_denied());
                Ok(())
            });
        assert!(matches!(
            result,
            Err(ProductionSourceOwnedViewErrorV18::Resource(
                ArgumentResourceV1::Accounting
            ))
        ));
        assert!(retained > MODULE_FLOOR);
    }
}

#[test]
fn optimized_analysis_owned_capture_keeps_restored_floor_refusal_sticky() {
    let drops = std::cell::Cell::new(0);
    let invoked = std::cell::Cell::new(0);
    let last = std::cell::Cell::new(0);
    let (result, retained) =
        optimized_analysis_callback_fixture_v18(|original, optimized, budget| {
            let owned = OptimizedAnalysisOwnedCaptureV18 {
                bytes: [0x27; 2048],
                drops: &drops,
                deny: None,
            };
            let called = &invoked;
            let paid = &last;
            let actual =
                original.with_optimized_analysis_v18(optimized, budget, move |analysis, budget| {
                    std::hint::black_box(&owned);
                    called.set(called.get() + 1);
                    budget.release_storage(1)?;
                    let before_work = budget.work();
                    assert!(matches!(
                        analysis.check(budget),
                        Err(ProductionSourceOwnedViewErrorV18::Resource(
                            ArgumentResourceV1::Accounting
                        ))
                    ));
                    assert_eq!(budget.work(), before_work);
                    budget.reserve_storage(1)?;
                    paid.set(budget.storage());
                    Ok::<_, ProductionSourceOwnedViewErrorV18>(())
                });
            assert!(matches!(
                actual,
                Err(ProductionSourceOwnedViewErrorV18::Resource(
                    ArgumentResourceV1::Accounting
                ))
            ));
            assert_eq!((invoked.get(), drops.get()), (1, 1));
            assert_eq!(budget.storage(), last.get());
            assert!(original.source.cleanup.is_denied());
            Ok(())
        });
    assert!(matches!(
        result,
        Err(ProductionSourceOwnedViewErrorV18::Resource(
            ArgumentResourceV1::Accounting
        ))
    ));
    assert!(retained >= last.get());
}

#[test]
fn optimized_analysis_mismatched_inventory_refusal_drops_capture_inside_preflight() {
    let drops = std::cell::Cell::new(0);
    let invoked = std::cell::Cell::new(0);
    let (result, retained) =
        optimized_analysis_callback_fixture_v18(|original, optimized, budget| {
            let output = optimized.output_inventory(budget)?;
            assert!(!std::ptr::eq(original.inventory, output));
            let mismatched = ProductionSourceCorrespondenceV18 {
                source: original.source,
                inventory: output,
                attachments: original.attachments,
                slot: original.slot,
                ledger: original.ledger,
                floor: original.floor,
            };
            let owned = OptimizedAnalysisOwnedCaptureV18 {
                bytes: [0x27; 2048],
                drops: &drops,
                deny: Some(original.source.cleanup),
            };
            let before = budget.storage();
            let called = &invoked;
            let actual = mismatched.with_optimized_analysis_v18(optimized, budget, move |_, _| {
                std::hint::black_box(&owned);
                called.set(called.get() + 1);
                Ok::<_, ProductionSourceOwnedViewErrorV18>(())
            });
            assert!(matches!(
                actual,
                Err(ProductionSourceOwnedViewErrorV18::Binding(
                    "optimized analysis changed original source or inventory"
                ))
            ));
            assert_eq!((invoked.get(), drops.get()), (0, 1));
            assert_eq!(budget.storage(), before);
            assert!(original.source.cleanup.is_denied());
            Ok(())
        });
    assert!(matches!(
        result,
        Err(ProductionSourceOwnedViewErrorV18::Binding(
            "optimized analysis changed original source or inventory"
        ))
    ));
    assert!(retained > MODULE_FLOOR);
}

#[test]
fn optimized_analysis_single_drop_origin_panic_keeps_its_payload_and_custody() {
    struct PanicOnDrop<'a> {
        owned: OptimizedAnalysisOwnedCaptureV18<'a>,
        payload: Option<Box<u64>>,
    }
    impl Drop for PanicOnDrop<'_> {
        fn drop(&mut self) {
            // The callback body returns normally. Its one destructor panic is
            // distinct from, and does not promise recovery from, a double panic.
            std::panic::resume_unwind(self.payload.take().unwrap());
        }
    }
    for deny_drop in [false, true] {
        let invoked = std::cell::Cell::new(0);
        let drops = std::cell::Cell::new(0);
        let (result, retained) =
            optimized_analysis_callback_fixture_v18(|original, optimized, budget| {
                budget.reserve_storage(std::mem::size_of::<u64>())?;
                let floor = budget.storage();
                let capture = PanicOnDrop {
                    owned: OptimizedAnalysisOwnedCaptureV18 {
                        bytes: [0x27; 2048],
                        drops: &drops,
                        deny: deny_drop.then_some(original.source.cleanup),
                    },
                    payload: Some(Box::new(0x1741_0027)),
                };
                let last = std::cell::Cell::new(0);
                let called = &invoked;
                let paid = &last;
                let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    original.with_optimized_analysis_v18(optimized, budget, move |_, budget| {
                        std::hint::black_box(&capture.owned);
                        called.set(called.get() + 1);
                        paid.set(budget.storage());
                        Ok::<_, ProductionSourceOwnedViewErrorV18>(())
                    })
                }));
                assert_eq!(*caught.unwrap_err().downcast::<u64>().unwrap(), 0x1741_0027);
                assert_eq!((invoked.get(), drops.get()), (1, 1));
                assert_eq!(budget.storage(), if deny_drop { last.get() } else { floor });
                assert_eq!(original.source.cleanup.is_denied(), deny_drop);
                budget.release_storage(std::mem::size_of::<u64>())?;
                Ok(())
            });
        if deny_drop {
            assert!(matches!(
                result,
                Err(ProductionSourceOwnedViewErrorV18::Resource(
                    ArgumentResourceV1::Accounting
                ))
            ));
            assert!(retained > MODULE_FLOOR);
        } else {
            result.unwrap();
            assert_eq!(retained, MODULE_FLOOR);
        }
    }
}
