#[test]
fn qualified_ranked_reader_refuses_changed_keys_recipe_owner_duplicate_rows_and_lost_floor() {
    for fault in [4, 0, 1, 2, 3] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(OPTIMIZED_SOURCE_WORK_LIMIT_V18);
        let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
        budget.reserve_storage(MODULE_FLOOR).unwrap();
        let prepared = scalar_payload_prepared_from_v18(scalar_payload_owner_v18, &mut budget);
        let reached = std::cell::Cell::new(false);
        let result = prepared.with_source_consumer_v18(&mut budget, |source, budget| {
            let attempted =
                source.with_checked_optimization_v18(budget, |original, optimized, budget| {
                    let floor = budget.storage();
                    let local = (|| {
                        let function =
                            optimized_source_root_function_v18(original, optimized, 0, budget)?
                                .function;
                        let recipe = effect_order_recipe_v18(function, 2, false);
                        let other_recipe = effect_order_recipe_v18(function, 2, false);
                        let source_instance = 1;
                        let source_function =
                            original.source.instance(0, source_instance, budget)?.0;
                        assert_eq!(source_function.index(), 2);
                        let mut rows = [0, 1].map(|ordinal| {
                            ProductionSourceRankedAccessV18::new(
                                source_instance,
                                source_function,
                                ProductionRankedAccessSourceV1 {
                                    semantic_block: 0,
                                    semantic_statement: Some(0),
                                    semantic_access_ordinal: ordinal,
                                    ranked_block: 0,
                                    ranked_operation: 2 + ordinal,
                                    output_extent: None,
                                },
                            )
                        });
                        prepare_source_ranked_accesses_v18(
                            original, optimized, 0, &mut rows, budget,
                        )?;
                        let positive_floor = budget.storage();
                        {
                            let data = QualifiedSourceRankedDataV18::build(
                                original,
                                optimized,
                                0,
                                &recipe,
                                &rows,
                                MODULE_LIMIT,
                                budget,
                            )?;
                            let ledger = CorrelationLedgerV18::new(budget, original.source.cleanup);
                            let query = data.query_index(&recipe, &ledger)?;
                            assert!(query.ready());
                            drop(query);
                            assert_eq!(ledger.failure.get(), None);
                            assert!(!ledger.inconsistent_inventory.get());
                        }
                        budget.release_storage(budget.storage() - positive_floor)?;
                        match fault {
                            0 => rows[1].solver = rows[0].solver,
                            1 | 3 | 4 => {}
                            2 => {
                                rows[1].projected.ranked_operation =
                                    rows[0].projected.ranked_operation
                            }
                            _ => unreachable!(),
                        }
                        let checked = QualifiedSourceRankedDataV18::build(
                            original,
                            optimized,
                            0,
                            &recipe,
                            &rows,
                            MODULE_LIMIT,
                            budget,
                        );
                        let error = if matches!(fault, 1 | 3 | 4) {
                            let data = checked?;
                            if fault == 3 {
                                budget.release_storage(1)?;
                            }
                            let before = (budget.storage(), budget.work());
                            // Construct the local query ledger after the malicious
                            // refund: the retained owner must still notice its floor.
                            let ledger = CorrelationLedgerV18::new(budget, original.source.cleanup);
                            let selected = if fault == 1 { &other_recipe } else { &recipe };
                            if fault == 4 {
                                let query = data.query_index(selected, &ledger)?;
                                assert!(
                                    query.ready(),
                                    "the unchanged candidate still has a valid ranked query"
                                );
                                drop(query);
                                assert_eq!(ledger.failure.get(), None);
                                assert!(!ledger.inconsistent_inventory.get());
                                drop(ledger);
                                drop(data);
                                return Ok(());
                            }
                            let error = match data.query_index(selected, &ledger) {
                                Ok(_) => {
                                    panic!("substituted recipe or underfunded ranked rows admitted")
                                }
                                Err(error) => error,
                            };
                            if fault == 1 {
                                assert!(ledger.inconsistent_inventory.get());
                                assert_eq!(
                                    ledger.failure.get(),
                                    None,
                                    "recipe identity is not resource exhaustion"
                                );
                            } else {
                                assert_eq!(
                                    ledger.failure.get(),
                                    Some(ArgumentResourceV1::Accounting)
                                );
                                assert!(!ledger.inconsistent_inventory.get());
                            }
                            drop(ledger);
                            if fault == 3 {
                                assert_eq!(
                                    (budget.storage(), budget.work()),
                                    before,
                                    "no query-header reinflation may conceal lost retained credit"
                                );
                            }
                            error
                        } else {
                            match checked {
                                Ok(_) => panic!(
                                    "changed key or repeated actual ranked operation admitted"
                                ),
                                Err(error) => error,
                            }
                        };
                        assert!(if fault == 3 {
                            matches!(
                                error,
                                ProductionSourceOwnedViewErrorV18::Resource(
                                    ArgumentResourceV1::Accounting
                                )
                            )
                        } else {
                            matches!(error, ProductionSourceOwnedViewErrorV18::Binding(_))
                        });
                        reached.set(true);
                        Err::<(), _>(error)
                    })();
                    if fault != 3 {
                        // This closed test construction has returned and dropped
                        // all local decoded rows and query handles. No arbitrary
                        // callback result owns any of its prepaid storage.
                        let storage = budget.storage().checked_sub(floor).unwrap();
                        budget.release_storage(storage)?;
                        assert_eq!(budget.storage(), floor);
                    }
                    if fault == 4 {
                        local?;
                        reached.set(true);
                        Ok(((), 0))
                    } else {
                        Err(local.unwrap_err())
                    }
                });
            assert_eq!(attempted.is_ok(), fault == 4);
            Ok::<_, ProductionSourceOwnedViewErrorV18>(())
        });
        assert!(reached.get());
        if fault == 4 {
            result.unwrap();
            assert_eq!(budget.storage(), MODULE_FLOOR);
        } else if fault == 3 {
            assert!(matches!(
                result,
                Err(ProductionSourceOwnedViewErrorV18::Resource(
                    ArgumentResourceV1::Accounting
                ))
            ));
            assert!(
                budget.storage() > MODULE_FLOOR,
                "an observed lost floor denies enclosing refunds"
            );
        } else {
            assert!(matches!(
                result,
                Err(ProductionSourceOwnedViewErrorV18::Binding(_))
            ));
            assert_eq!(budget.storage(), MODULE_FLOOR);
        }
    }
}

#[test]
fn independently_rederived_output_inventory_is_not_the_checked_endpoint() {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(OPTIMIZED_SOURCE_WORK_LIMIT_V18);
    let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
    budget.reserve_storage(MODULE_FLOOR).unwrap();
    let prepared = scalar_payload_prepared_from_v18(folding_source_owner_v18, &mut budget);
    let reached = std::cell::Cell::new(false);
    let result = prepared.with_source_consumer_v18(&mut budget, |source, budget| {
        let attempted =
            source.with_checked_optimization_v18(budget, |original, optimized, budget| {
                let output = optimized.output_inventory(budget)?;
                let (second, receipt) =
                    fe2o3_kernel_analysis::CanonicalKirInventoryV18::derive_v18(
                        output.owner(),
                        budget,
                    )
                    .map_err(|error| {
                        ProductionSourceOwnedViewErrorV18::from(
                            fe2o3_pliron::CanonicalAnalysisScopeErrorV1::Inventory(error),
                        )
                    })?;
                budget.reserve_storage(receipt.retained_storage())?;
                assert!(!std::ptr::eq(output, &second));
                assert!(second.belongs_to(output.owner()));
                let root = optimized_source_root_function_v18(original, optimized, 0, budget)?;
                let query = value_origin_v1::with_optimized_whole_value_origins_v18(
                    original,
                    optimized,
                    &second,
                    root.coordinate,
                    budget,
                    |_, _| -> SourceOwnedResultV18<()> { panic!("rederived inventory admitted") },
                );
                reached.set(true);
                drop(second);
                budget.release_storage(receipt.retained_storage())?;
                query?;
                Ok::<_, ProductionSourceOwnedViewErrorV18>(((), 0))
            });
        assert!(attempted.is_err());
        Ok::<_, ProductionSourceOwnedViewErrorV18>(())
    });
    assert!(reached.get());
    assert!(matches!(
        result,
        Err(ProductionSourceOwnedViewErrorV18::Binding(
            "optimized whole-value endpoint association"
        ))
    ));
    assert_eq!(budget.storage(), MODULE_FLOOR);
}

#[test]
fn caught_optimized_analysis_floor_refusal_stays_sticky_after_reinflation() {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(OPTIMIZED_SOURCE_WORK_LIMIT_V18);
    let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
    budget.reserve_storage(MODULE_FLOOR).unwrap();
    let prepared = scalar_payload_prepared_from_v18(folding_source_owner_v18, &mut budget);
    let reached = std::cell::Cell::new(false);
    let result = prepared.with_source_consumer_v18(&mut budget, |source, budget| {
        let attempted =
            source.with_checked_optimization_v18(budget, |original, optimized, budget| {
                let ignored =
                    original.with_optimized_analysis_v18(optimized, budget, |analyses, budget| {
                        budget.release_storage(1)?;
                        let denied = analyses.with_projection_facts(
                            budget,
                            |_, _, _, _| -> SourceOwnedResultV18<()> {
                                panic!("underfunded cache admitted")
                            },
                        );
                        assert!(matches!(
                            denied,
                            Err(ProductionSourceOwnedViewErrorV18::Resource(
                                ArgumentResourceV1::Accounting
                            ))
                        ));
                        budget.reserve_storage(1)?;
                        reached.set(true);
                        Ok::<_, ProductionSourceOwnedViewErrorV18>(())
                    });
                assert!(ignored.is_err());
                Ok::<_, ProductionSourceOwnedViewErrorV18>(((), 0))
            });
        assert!(attempted.is_err());
        Ok::<_, ProductionSourceOwnedViewErrorV18>(())
    });
    assert!(reached.get());
    assert!(matches!(
        result,
        Err(ProductionSourceOwnedViewErrorV18::Resource(
            ArgumentResourceV1::Accounting
        ))
    ));
    assert!(
        budget.storage() > MODULE_FLOOR,
        "denied custody must not refund live owner credit"
    );
}
#[test]
fn indexed_optimized_span_missing_site_or_ordinal_is_a_sticky_refusal() {
    for wrong_site in [false, true] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(OPTIMIZED_SOURCE_WORK_LIMIT_V18);
        let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
        budget.reserve_storage(MODULE_FLOOR).unwrap();
        let prepared = scalar_payload_prepared_from_v18(scalar_payload_owner_v18, &mut budget);
        let reached = std::cell::Cell::new(false);
        let result = prepared.with_source_consumer_v18(&mut budget, |source, budget| {
            let attempted = source.with_checked_optimization_v18(budget, |_, optimized, budget| {
                let block = SemanticBlockIdV1::from_index(0);
                let count = optimized.source_span_entry_count(0, 0, block, None, budget)?;
                assert!(count > 0);
                let error = if wrong_site {
                    optimized.source_span_entry(
                        0,
                        0,
                        SemanticBlockIdV1::from_index(u32::MAX),
                        None,
                        0,
                        budget,
                    )
                } else {
                    optimized.source_span_entry(0, 0, block, None, count, budget)
                };
                assert!(matches!(
                    error,
                    Err(ProductionSourceOwnedViewErrorV18::Binding(_))
                ));
                assert!(
                    optimized
                        .source_span_entry_count(0, 0, block, None, budget)
                        .is_err()
                );
                reached.set(true);
                Ok::<_, ProductionSourceOwnedViewErrorV18>(((), 0))
            });
            assert!(attempted.is_err());
            Ok::<_, ProductionSourceOwnedViewErrorV18>(())
        });
        assert!(reached.get());
        assert!(matches!(
            result,
            Err(ProductionSourceOwnedViewErrorV18::Binding(_))
        ));
        assert_eq!(budget.storage(), MODULE_FLOOR);
    }
}
#[derive(Debug)]
enum OptimizedConsumerSelectedErrorV18 {
    Source(ProductionSourceOwnedViewErrorV18),
    Selected(u32),
}

#[test]
fn retained_optimizer_refuses_callback_panic_without_exporting_or_refunding_lost_custody() {
    static DROPS: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    struct PanickingDrop;
    impl Drop for PanickingDrop {
        fn drop(&mut self) {
            DROPS.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            panic!("selected optimizer payload destructor");
        }
    }
    DROPS.store(0, std::sync::atomic::Ordering::SeqCst);
    for underpay in [false, true] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(OPTIMIZED_SOURCE_WORK_LIMIT_V18);
        let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
        budget.reserve_storage(MODULE_FLOOR).unwrap();
        let prepared = scalar_payload_prepared_from_v18(scalar_payload_owner_v18, &mut budget);
        let reached = std::cell::Cell::new(false);
        let result = prepared.with_source_consumer_v18(&mut budget, |source, budget| {
            source
                .with_retained_checked_optimization_v18(budget, |_, _, budget| {
                    reached.set(true);
                    if !underpay {
                        panic!("selected optimizer callback panic");
                    }
                    budget.release_storage(1)?;
                    // B1 must discard this rejected result before the panic reaches
                    // Stage A; it cannot turn it into an adopted output owner.
                    Ok::<_, ProductionSourceOwnedViewErrorV18>((
                        PanickingDrop,
                        size_of::<PanickingDrop>(),
                    ))
                })
                .map(|_| ())
        });
        assert!(reached.get());
        assert!(result.is_err());
        if underpay {
            assert!(matches!(
                result,
                Err(ProductionSourceOptimizationErrorV18::Source(
                    ProductionSourceOwnedViewErrorV18::Resource(ArgumentResourceV1::Accounting)
                ))
            ));
            assert!(
                budget.storage() > MODULE_FLOOR,
                "the retained wrapper cannot erase a nested deny-refund disposition"
            );
            assert_eq!(DROPS.load(std::sync::atomic::Ordering::SeqCst), 1);
        } else {
            assert!(matches!(
                result,
                Err(ProductionSourceOptimizationErrorV18::Source(
                    ProductionSourceOwnedViewErrorV18::Binding(
                        "actual source optimizer adoption rejected"
                    )
                ))
            ));
            assert_eq!(budget.storage(), MODULE_FLOOR);
            assert_eq!(DROPS.load(std::sync::atomic::Ordering::SeqCst), 0);
        }
    }
}

impl From<ProductionSourceOwnedViewErrorV18> for OptimizedConsumerSelectedErrorV18 {
    fn from(error: ProductionSourceOwnedViewErrorV18) -> Self {
        Self::Source(error)
    }
}

#[test]
fn actual_optimizer_callback_error_is_not_replaced_by_a_generic_source_refusal() {
    for retained in [false, true] {
        for source_refusal in [false, true] {
            let mut work = CanonicalKernelIrWorkBudgetV1::new(OPTIMIZED_SOURCE_WORK_LIMIT_V18);
            let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
            budget.reserve_storage(MODULE_FLOOR).unwrap();
            let prepared = scalar_payload_prepared_from_v18(scalar_payload_owner_v18, &mut budget);
            let reached = std::cell::Cell::new(false);
            let result = prepared.with_source_consumer_v18(&mut budget, |source, budget| {
                let consume = |_: &ProductionSourceCorrespondenceV18<'_>,
                               optimized: &ProductionOptimizedSourceCorrespondenceV18<'_>,
                               budget: &mut ArgumentBudgetV1<'_>| {
                    if source_refusal {
                        let refused = optimized.source_span_entry_count(
                            0,
                            0,
                            SemanticBlockIdV1::from_index(u32::MAX),
                            None,
                            budget,
                        );
                        assert!(matches!(
                            refused,
                            Err(ProductionSourceOwnedViewErrorV18::Binding(_))
                        ));
                    }
                    reached.set(true);
                    Err::<((), usize), _>(OptimizedConsumerSelectedErrorV18::Selected(73))
                };
                if retained {
                    source
                        .with_retained_checked_optimization_v18(budget, consume)
                        .map(|_| ())
                } else {
                    source
                        .with_checked_optimization_v18(budget, consume)
                        .map(|_| ())
                }
            });
            assert!(reached.get());
            if source_refusal {
                assert!(matches!(
                    result,
                    Err(ProductionSourceOptimizationErrorV18::Source(
                        ProductionSourceOwnedViewErrorV18::Binding("optimized source site span")
                    ))
                ));
            } else {
                assert!(matches!(
                    result,
                    Err(ProductionSourceOptimizationErrorV18::Adoption(
                        fe2o3_pliron::KirCheckedNeutralOptimizationErrorV1::Origin(
                            OptimizedConsumerSelectedErrorV18::Selected(73)
                        )
                    ))
                ));
            }
            assert_eq!(budget.storage(), MODULE_FLOOR);
        }
    }
}
