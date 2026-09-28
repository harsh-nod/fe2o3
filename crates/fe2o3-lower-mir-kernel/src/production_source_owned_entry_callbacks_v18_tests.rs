#[repr(align(256))]
struct OriginalAnalysisCaptureV1747<'a, const N: usize> {
    bytes: [u8; N],
    drops: &'a std::cell::Cell<usize>,
    deny: Option<&'a ScopedSourceCleanupV29>,
    panic: Option<Box<u64>>,
}

impl<const N: usize> Drop for OriginalAnalysisCaptureV1747<'_, N> {
    fn drop(&mut self) {
        assert_eq!(self.bytes[0], 0x47);
        self.drops.set(self.drops.get() + 1);
        if let Some(cleanup) = self.deny {
            cleanup.deny_refund();
        }
        if let Some(payload) = self.panic.take() {
            std::panic::resume_unwind(payload);
        }
    }
}

fn original_analysis_fixture_v1747(
    consume: impl for<'source, 'work> FnOnce(
        &ProductionSourceOwnedViewV18<'source>,
        &mut ArgumentBudgetV1<'work>,
    ) -> SourceOwnedResultV18<()>,
) -> (SourceOwnedResultV18<()>, usize) {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(MODULE_LIMIT);
    let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
    budget.reserve_storage(MODULE_FLOOR).unwrap();
    let prepared = scalar_payload_prepared_v18(&mut budget);
    let entered = std::cell::Cell::new(false);
    let result = prepared.with_source_consumer_v18(&mut budget, |source, budget| {
        entered.set(true);
        consume(source, budget)
    });
    assert!(
        entered.get(),
        "genuine original source replay must complete"
    );
    (result, budget.storage())
}

fn original_analysis_header_oracle_v1747<T, E, F>(_: &F) -> usize {
    use std::mem::{align_of, size_of};
    use std::panic::AssertUnwindSafe;
    type Entry<F> = (usize, F);
    type Capture<'a, 'work, F> = (
        &'a ProductionSourceOwnedViewV18<'a>,
        &'a mut ArgumentBudgetV1<'work>,
        &'a std::cell::Cell<usize>,
        F,
    );
    type Invoke<'a, F> = (F, &'a std::cell::Cell<bool>);
    type Framework<'a, 'work, F> = (
        &'a ProductionSourceOwnedViewV18<'a>,
        &'a mut ArgumentBudgetV1<'work>,
        &'a fe2o3_pliron::CanonicalAnalysisCleanupV1<'a>,
        Invoke<'a, F>,
    );
    type Analysis<T, E> = Result<T, SourceAnalysisBoundaryV18<E>>;
    size_of::<Capture<'_, '_, F>>()
        + align_of::<Capture<'_, '_, F>>()
        + size_of::<AssertUnwindSafe<Capture<'_, '_, F>>>()
        + size_of::<Entry<F>>()
        + align_of::<Entry<F>>()
        + 2 * size_of::<SourceOwnedResultV18<Entry<F>>>()
        + size_of::<std::thread::Result<SourceOwnedResultV18<Entry<F>>>>()
        + size_of::<AssertUnwindSafe<Entry<F>>>()
        + size_of::<std::thread::Result<()>>()
        + size_of::<std::cell::Cell<usize>>()
        + 4 * size_of::<usize>()
        + size_of::<fe2o3_kernel_ir::CanonicalKernelIrWorkLedgerIdentityV1>()
        + size_of::<SourceOwnedResultV18<()>>()
        + size_of::<fe2o3_pliron::CanonicalAnalysisCleanupV1<'_>>()
        + size_of::<std::cell::Cell<bool>>()
        + size_of::<Invoke<'_, F>>()
        + align_of::<Invoke<'_, F>>()
        + size_of::<Framework<'_, '_, F>>()
        + align_of::<Framework<'_, '_, F>>()
        + size_of::<AssertUnwindSafe<Framework<'_, '_, F>>>()
        + size_of::<Analysis<T, E>>()
        + size_of::<std::thread::Result<Analysis<T, E>>>()
        + size_of::<std::thread::Result<Result<T, E>>>()
        + size_of::<Result<T, E>>()
        + size_of::<AssertUnwindSafe<Result<T, E>>>()
}

#[test]
fn original_analysis_generic_results_are_paid_independently_of_owned_capture() {
    #[repr(align(256))]
    struct Large([u8; 16384]);
    let consume = || ();
    let small = source_analysis_owned_headers_v18::<(), (), _>(&consume).unwrap();
    let large = source_analysis_owned_headers_v18::<Large, Large, _>(&consume).unwrap();
    assert_eq!(
        small,
        original_analysis_header_oracle_v1747::<(), (), _>(&consume)
    );
    assert_eq!(
        large,
        original_analysis_header_oracle_v1747::<Large, Large, _>(&consume)
    );
    assert!(large - small >= 5 * size_of::<Large>());
}

fn original_analysis_success_probe_v1747<const N: usize>(
    source: &ProductionSourceOwnedViewV18<'_>,
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<(usize, usize, usize)> {
    let drops = std::cell::Cell::new(0);
    let invoked = std::cell::Cell::new(0);
    let live = std::cell::Cell::new(0);
    let owned = OriginalAnalysisCaptureV1747 {
        bytes: [0x47; N],
        drops: &drops,
        deny: None,
        panic: None,
    };
    let called = &invoked;
    let recorded = &live;
    let consume = move |scope: &mut fe2o3_pliron::CanonicalAnalysisScopeV18<'_, '_, '_, '_>| {
        std::hint::black_box(&owned);
        called.set(called.get() + 1);
        scope.with_inventory_v1(|_, budget| {
            recorded.set(budget.storage());
            Ok::<_, ProductionSourceOwnedViewErrorV18>(())
        })
    };
    assert!(std::mem::size_of_val(&consume) >= N);
    assert_eq!(std::mem::align_of_val(&consume), 256);
    let header =
        original_analysis_header_oracle_v1747::<(), ProductionSourceOwnedViewErrorV18, _>(&consume);
    assert_eq!(
        source_analysis_owned_headers_v18::<(), ProductionSourceOwnedViewErrorV18, _>(&consume)?,
        header
    );
    let floor = budget.storage();
    let work = budget.work();
    source.with_analysis_v18(budget, consume)?;
    assert_eq!((invoked.get(), drops.get()), (1, 1));
    assert_eq!(budget.storage(), floor);
    Ok((header, live.get() - floor, budget.work() - work))
}

#[test]
fn original_analysis_owned_header_scales_without_consuming_caller_credit() {
    let (result, retained) = original_analysis_fixture_v1747(|source, budget| {
        budget.reserve_storage(37)?;
        let floor = budget.storage();
        let small = original_analysis_success_probe_v1747::<1>(source, budget)?;
        let large = original_analysis_success_probe_v1747::<2048>(source, budget)?;
        assert!(large.0 > small.0 + 2048);
        assert_eq!(large.1 - small.1, large.0 - small.0);
        assert_eq!(large.2, small.2);
        assert_eq!(budget.storage(), floor);
        budget.release_storage(37)?;
        Ok(())
    });
    result.unwrap();
    assert_eq!(retained, MODULE_FLOOR);
}

#[test]
fn original_analysis_header_exact_short_and_paid_work_refusal_settle_only_accepted_credit() {
    for (short, deny) in [(true, false), (false, false), (true, true), (false, true)] {
        let drops = std::cell::Cell::new(0);
        let invoked = std::cell::Cell::new(0);
        let selected = std::cell::Cell::new(None);
        let (result, retained) = original_analysis_fixture_v1747(|source, budget| {
            let owned = OriginalAnalysisCaptureV1747 {
                bytes: [0x47; 2048],
                drops: &drops,
                deny: deny.then_some(source.cleanup),
                panic: None,
            };
            let called = &invoked;
            let consume = move |_: &mut fe2o3_pliron::CanonicalAnalysisScopeV18<'_, '_, '_, '_>| {
                std::hint::black_box(&owned);
                called.set(called.get() + 1);
                Ok::<_, ProductionSourceOwnedViewErrorV18>(())
            };
            let header =
                original_analysis_header_oracle_v1747::<(), ProductionSourceOwnedViewErrorV18, _>(
                    &consume,
                );
            let filler = MODULE_LIMIT - budget.storage() - header + usize::from(short);
            budget.reserve_storage(filler)?;
            // One entry query succeeds; the framework's atomic four-unit
            // precharge is then one short, before inventory construction.
            budget.charge_work(MODULE_LIMIT - budget.work() - 4)?;
            let floor = budget.storage();
            let work = budget.work();
            let error = source.with_analysis_v18(budget, consume).unwrap_err();
            let ProductionSourceOwnedViewErrorV18::Resource(error) = error else {
                panic!("entry refusal must preserve its exact resource kind");
            };
            if short {
                assert!(matches!(error, ArgumentResourceV1::Storage(limit)
                    if limit.actual() == MODULE_LIMIT + 1 && limit.limit() == MODULE_LIMIT));
                assert_eq!(budget.failed_storage(), Some(MODULE_LIMIT + 1));
                assert_eq!(budget.peak_storage(), floor);
            } else {
                assert!(matches!(error, ArgumentResourceV1::Work(limit)
                    if limit.actual() == MODULE_LIMIT + 1 && limit.limit() == MODULE_LIMIT));
                assert_eq!(budget.peak_storage(), MODULE_LIMIT);
            }
            assert_eq!(budget.work(), work + 1);
            assert_eq!(
                budget.storage(),
                floor + if deny && !short { header } else { 0 }
            );
            assert_eq!((invoked.get(), drops.get()), (0, 1));
            assert_eq!(source.cleanup.is_denied(), deny);
            selected.set(Some(error));
            budget.release_storage(filler)?;
            Ok(())
        });
        assert!(
            matches!(result, Err(ProductionSourceOwnedViewErrorV18::Resource(error))
            if Some(error) == selected.get())
        );
        assert_eq!(retained == MODULE_FLOOR, !deny);
    }
}

#[test]
fn original_analysis_early_query_refusal_owns_drop_without_new_debits() {
    for deny in [false, true] {
        let drops = std::cell::Cell::new(0);
        let invoked = std::cell::Cell::new(0);
        let (result, retained) = original_analysis_fixture_v1747(|source, budget| {
            let _ = source.missing::<()>("V1747 earlier query");
            let owned = OriginalAnalysisCaptureV1747 {
                bytes: [0x47; 2048],
                drops: &drops,
                deny: deny.then_some(source.cleanup),
                panic: None,
            };
            let called = &invoked;
            let before = (budget.work(), budget.storage());
            let result = source.with_analysis_v18(budget, move |_| {
                std::hint::black_box(&owned);
                called.set(called.get() + 1);
                Ok::<_, ProductionSourceOwnedViewErrorV18>(())
            });
            assert!(matches!(
                result,
                Err(ProductionSourceOwnedViewErrorV18::Binding(
                    "V1747 earlier query"
                ))
            ));
            assert_eq!((invoked.get(), drops.get()), (0, 1));
            assert_eq!((budget.work(), budget.storage()), before);
            assert_eq!(source.cleanup.is_denied(), deny);
            Ok(())
        });
        assert!(matches!(
            result,
            Err(ProductionSourceOwnedViewErrorV18::Binding(
                "V1747 earlier query"
            ))
        ));
        assert_eq!(retained == MODULE_FLOOR, !deny);
    }
}

#[test]
fn original_analysis_foreign_ledger_and_same_ledger_wrong_slot_are_not_charged() {
    for wrong_slot in [false, true] {
        let drops = std::cell::Cell::new(0);
        let invoked = std::cell::Cell::new(0);
        let mut work = CanonicalKernelIrWorkBudgetV1::new(MODULE_LIMIT);
        let mut foreign_work = CanonicalKernelIrWorkBudgetV1::new(MODULE_LIMIT);
        let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
        let mut foreign = ArgumentBudgetV1::new(&mut foreign_work, MODULE_LIMIT);
        budget.reserve_storage(MODULE_FLOOR).unwrap();
        let prepared = scalar_payload_prepared_v18(&mut budget);
        let entered = std::cell::Cell::new(false);
        let result = prepared.with_source_consumer_v18(&mut budget, |source, budget| {
            entered.set(true);
            foreign.reserve_storage(budget.storage())?;
            let original = (budget.work(), budget.storage());
            let foreign_before = (foreign.work(), foreign.storage());
            let owned = OriginalAnalysisCaptureV1747 {
                bytes: [0x47; 2048],
                drops: &drops,
                deny: None,
                panic: None,
            };
            let called = &invoked;
            if wrong_slot {
                std::mem::swap(budget, &mut foreign);
            }
            let actual = source.with_analysis_v18(&mut foreign, move |_| {
                std::hint::black_box(&owned);
                called.set(called.get() + 1);
                Ok::<_, ProductionSourceOwnedViewErrorV18>(())
            });
            if wrong_slot {
                std::mem::swap(budget, &mut foreign);
            }
            assert!(matches!(
                actual,
                Err(ProductionSourceOwnedViewErrorV18::Resource(
                    ArgumentResourceV1::Accounting
                ))
            ));
            assert_eq!((budget.work(), budget.storage()), original);
            assert_eq!((foreign.work(), foreign.storage()), foreign_before);
            assert_eq!((invoked.get(), drops.get()), (0, 1));
            assert!(source.cleanup.is_denied());
            Ok(())
        });
        assert!(
            entered.get(),
            "genuine original source replay must complete"
        );
        let retained = budget.storage();
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
fn original_analysis_owned_disposal_preserves_typed_and_single_raw_outcomes() {
    // Non-panicking denial Drop may accompany a selected typed error or body
    // panic. A Drop-origin panic is tested only after a successful body.
    for mode in 0..4 {
        let drops = std::cell::Cell::new(0);
        let invoked = std::cell::Cell::new(0);
        let (result, retained) = original_analysis_fixture_v1747(|source, budget| {
            let deny = mode != 0;
            if mode >= 2 {
                budget.reserve_storage(size_of::<u64>())?;
            }
            let owned = OriginalAnalysisCaptureV1747 {
                bytes: [0x47; 2048],
                drops: &drops,
                deny: deny.then_some(source.cleanup),
                panic: (mode == 3).then(|| Box::new(0x1747_1747_u64)),
            };
            let called = &invoked;
            let before = budget.storage();
            let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                source.with_analysis_v18(budget, move |_| {
                    std::hint::black_box(&owned);
                    called.set(called.get() + 1);
                    match mode {
                        0 | 1 => Err(ProductionSourceOwnedViewErrorV18::Binding(
                            "V1747 selected consumer",
                        )),
                        2 => std::panic::resume_unwind(Box::new(0x1747_1747_u64)),
                        3 => Ok(()),
                        _ => unreachable!(),
                    }
                })
            }));
            assert_eq!((invoked.get(), drops.get()), (1, 1));
            assert_eq!(source.cleanup.is_denied(), deny);
            assert!(
                source.guard.first.get().is_none(),
                "consumer outcomes are not construction failures"
            );
            if mode < 2 {
                assert!(matches!(
                    caught.unwrap(),
                    Err(ProductionSourceOwnedViewErrorV18::Binding(
                        "V1747 selected consumer"
                    ))
                ));
            } else {
                assert_eq!(*caught.unwrap_err().downcast::<u64>().unwrap(), 0x1747_1747);
                budget.release_storage(size_of::<u64>())?;
            }
            if deny {
                assert!(budget.storage() > before);
            } else {
                assert_eq!(budget.storage(), before);
            }
            if mode < 2 {
                Err(ProductionSourceOwnedViewErrorV18::Binding(
                    "V1747 selected consumer",
                ))
            } else {
                Ok(())
            }
        });
        if mode < 2 {
            assert!(matches!(
                result,
                Err(ProductionSourceOwnedViewErrorV18::Binding(
                    "V1747 selected consumer"
                ))
            ));
        } else {
            assert!(matches!(
                result,
                Err(ProductionSourceOwnedViewErrorV18::Resource(
                    ArgumentResourceV1::Accounting
                ))
            ));
        }
        assert_eq!(retained == MODULE_FLOOR, mode == 0);
    }
}

#[test]
fn original_analysis_early_drop_panic_is_raw_and_keeps_prior_query_selection() {
    let drops = std::cell::Cell::new(0);
    let invoked = std::cell::Cell::new(0);
    let (result, retained) = original_analysis_fixture_v1747(|source, budget| {
        let _ = source.missing::<()>("V1747 before Drop panic");
        budget.reserve_storage(size_of::<u64>())?;
        let owned = OriginalAnalysisCaptureV1747 {
            bytes: [0x47; 2048],
            drops: &drops,
            deny: Some(source.cleanup),
            panic: Some(Box::new(0x1747_001_u64)),
        };
        let called = &invoked;
        let before = (budget.work(), budget.storage());
        let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            source.with_analysis_v18(budget, move |_| {
                std::hint::black_box(&owned);
                called.set(called.get() + 1);
                Ok::<_, ProductionSourceOwnedViewErrorV18>(())
            })
        }));
        assert_eq!(*caught.unwrap_err().downcast::<u64>().unwrap(), 0x1747_001);
        assert_eq!((invoked.get(), drops.get()), (0, 1));
        assert_eq!((budget.work(), budget.storage()), before);
        assert!(matches!(
            source.guard.first.get(),
            Some(SourceOwnedQueryFailureV18::Binding(
                "V1747 before Drop panic"
            ))
        ));
        budget.release_storage(size_of::<u64>())?;
        Ok(())
    });
    assert!(matches!(
        result,
        Err(ProductionSourceOwnedViewErrorV18::Binding(
            "V1747 before Drop panic"
        ))
    ));
    assert!(retained > MODULE_FLOOR);
}

#[test]
fn original_analysis_restored_floor_does_not_restore_cleanup_authority() {
    let drops = std::cell::Cell::new(0);
    let (result, retained) = original_analysis_fixture_v1747(|source, budget| {
        let owned = OriginalAnalysisCaptureV1747 {
            bytes: [0x47; 2048],
            drops: &drops,
            deny: None,
            panic: None,
        };
        let before = budget.storage();
        let result = source.with_analysis_v18(budget, move |scope| {
            std::hint::black_box(&owned);
            scope.with_inventory_v1(|_, budget| {
                let released = budget.storage() - source.guard.floor + 1;
                budget.release_storage(released)?;
                assert!(matches!(
                    source.check_query_v18(budget),
                    Err(ProductionSourceOwnedViewErrorV18::Resource(
                        ArgumentResourceV1::Accounting
                    ))
                ));
                budget.reserve_storage(released)?;
                Ok::<_, ProductionSourceOwnedViewErrorV18>(())
            })
        });
        assert!(matches!(
            result,
            Err(ProductionSourceOwnedViewErrorV18::Resource(
                ArgumentResourceV1::Accounting
            ))
        ));
        assert_eq!(drops.get(), 1);
        assert!(source.cleanup.is_denied());
        assert!(budget.storage() > before);
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

#[test]
fn prepared_original_retained_sum_excludes_attempt_headers_and_adopts_occurrences_once() {
    for abi in [false, true] {
        for preexisting in [false, true] {
            let mut work = CanonicalKernelIrWorkBudgetV1::new(MODULE_LIMIT);
            let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
            budget.reserve_storage(MODULE_FLOOR + 37).unwrap();
            let prepared = if abi {
                with_original_kernel_abi_input_v18(
                    ModuleFixture::Ordinary,
                    preexisting,
                    &mut budget,
                    |owner, launch, source, _, budget| {
                        let fixture = OriginalKernelAbiFixtureV18::ordinary(&owner);
                        let roots = fixture.roots();
                        ProductionPendingScopedSourceOwnerV29::prepare_source_with_kernel_abi_budget_v18(
                            owner, launch, source, ProductionKernelArgumentAbiInputV18 { roots: &roots },
                            ProductionSemanticKirLimitsV1::default(), budget,
                        ).unwrap()
                    },
                )
            } else {
                prepared_source_fixture(ModuleFixture::Ordinary, preexisting, &mut budget)
            };
            let occurrence = prepared
                .source
                .owner
                .occurrence_storage()
                .unwrap()
                .retained_storage();
            let payload = owned_input_payload(&prepared.source.input);
            assert!(occurrence > 0);
            assert_eq!(prepared.capture_storage, occurrence);
            assert_eq!(prepared.source.input.retained_storage, payload);
            assert_eq!(prepared.source.input.kernel_argument_abi.is_some(), abi);
            let exact = size_of::<ProductionPreparedSourceV18>() + payload + occurrence;
            assert_eq!(prepared.adopted_storage(), exact);
            assert_eq!(budget.storage(), MODULE_FLOOR + 37 + exact);
            let invoked = std::cell::Cell::new(false);
            prepared
                .with_checked_source_v18(&mut budget, |source, budget| {
                    source.with_analysis_v18(budget, |scope| {
                        scope.with_inventory_v1(|_, _| {
                            invoked.set(true);
                            Ok::<_, ProductionSourceOwnedViewErrorV18>(())
                        })
                    })
                })
                .unwrap();
            assert!(invoked.get());
            assert_eq!(budget.storage(), MODULE_FLOOR + 37);
            budget.release_storage(37).unwrap();
        }
    }
}

include!("production_source_consumer_query_drop_v1751_tests.rs");
