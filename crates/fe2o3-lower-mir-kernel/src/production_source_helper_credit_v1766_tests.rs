fn physical_noop_callback_v1766(
    physical: &scoped_raw_admission_v29::CheckedSourceMemoryV29<'_>,
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<()> {
    use scoped_raw_admission_v29::PendingSourceMemoryEffectV29 as Effect;
    let mut effects = [0; 2];
    physical.visit_effects(budget, |effect, _| {
        match effect {
            Effect::Invocation(instance) => {
                assert_eq!(instance.index(), 0);
                effects[0] += 1;
            }
            Effect::Return { instance, block } => {
                assert_eq!(instance.index(), 0);
                assert_eq!(block.index(), 0);
                effects[1] += 1;
            }
            other => panic!("unexpected no-op source effect: {other:?}"),
        }
        Ok(())
    })?;
    assert_eq!(effects, [1, 1]);
    Ok(())
}

#[test]
fn original_memory_scope_excludes_transient_helper_header_and_preserves_callback_credit() {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(500_000_000);
    let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
    budget.reserve_storage(MODULE_FLOOR).unwrap();
    let (prepared, _) = closed_prepared_v1760(ClosedCaseV1760::Noop, true, &mut budget);
    let calls = std::cell::Cell::new(0);
    prepared
        .with_source_consumer_v18(&mut budget, |source, budget| -> SourceOwnedResultV18<()> {
            source.with_analysis_v18(budget, |scope| {
                scope.with_inventory_v1(|inventory, budget| {
                    source.with_ranked_correspondence_v18(inventory, budget, |relation, budget| {
                        for root in 0..source.root_count(budget)? {
                            let before = budget.storage();
                            scoped_raw_admission_v29::with_checked_source_memory_v29(
                                relation,
                                root,
                                None,
                                budget,
                                |physical, budget| -> SourceOwnedResultV18<()> {
                                    physical_noop_callback_v1766(physical, budget)?;
                                    budget.reserve_storage(17)?;
                                    calls.set(calls.get() + 1);
                                    Ok(())
                                },
                            )?;
                            assert_eq!(budget.storage(), before + 17);
                            assert!(!source.cleanup.is_denied());
                            source.check_query_v18(budget)?;
                            budget.release_storage(17)?;
                        }
                        Ok(())
                    })
                })
            })
        })
        .unwrap();
    assert_eq!(calls.get(), 2);
    assert_eq!(budget.storage(), MODULE_FLOOR);
}

#[test]
fn original_memory_scope_real_peak_and_one_short_keep_exact_storage_refusal() {
    for short in [false, true] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(500_000_000);
        let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
        budget.reserve_storage(MODULE_FLOOR).unwrap();
        let (prepared, _) = closed_prepared_v1760(ClosedCaseV1760::Noop, true, &mut budget);
        let checked = std::cell::Cell::new(false);
        let result = prepared.with_source_consumer_v18(&mut budget, |source, budget| -> SourceOwnedResultV18<()> {
            source.with_analysis_v18(budget, |scope| scope.with_inventory_v1(|inventory, budget| {
                source.with_ranked_correspondence_v18(inventory, budget, |relation, budget| {
                    let before = budget.storage();
                    let probe_floor = budget.peak_storage() + 1024;
                    let probe_filler = probe_floor - before;
                    budget.reserve_storage(probe_filler)?;
                    scoped_raw_admission_v29::with_checked_source_memory_v29(relation, 0, None, budget,
                        physical_noop_callback_v1766)?;
                    assert_eq!(budget.storage(), probe_floor);
                    let peak_delta = budget.peak_storage() - probe_floor;
                    assert!(peak_delta > 0);
                    budget.release_storage(probe_filler)?;
                    assert_eq!(budget.storage(), before);
                    let filler = MODULE_LIMIT - before - peak_delta + usize::from(short);
                    budget.reserve_storage(filler)?;
                    let result = scoped_raw_admission_v29::with_checked_source_memory_v29(relation, 0, None, budget,
                        physical_noop_callback_v1766);
                    assert_eq!(budget.storage(), before + filler);
                    budget.release_storage(filler)?;
                    assert_eq!(budget.storage(), before);
                    checked.set(true);
                    if short {
                        let error = result.err().expect("one-short must fail");
                        assert!(matches!(error, ProductionSourceOwnedViewErrorV18::Resource(
                            ArgumentResourceV1::Storage(error)) if error.actual() == MODULE_LIMIT + 1
                                && error.limit() == MODULE_LIMIT), "{error:?}");
                        assert_eq!(budget.failed_storage(), Some(MODULE_LIMIT + 1));
                        Err(error)
                    } else {
                        result?;
                        assert_eq!(budget.peak_storage(), MODULE_LIMIT);
                        assert_eq!(budget.failed_storage(), None);
                        Ok(())
                    }
                })
            }))
        });
        assert!(checked.get());
        if short {
            assert!(
                matches!(result, Err(ProductionSourceOwnedViewErrorV18::Resource(
                ArgumentResourceV1::Storage(error))) if error.actual() == MODULE_LIMIT + 1
                    && error.limit() == MODULE_LIMIT)
            );
        } else {
            result.unwrap();
        }
        assert_eq!(budget.storage(), MODULE_FLOOR);
    }
}

#[test]
fn scalar_leaf_returned_floor_excludes_helper_header_and_refunds_only_owned_scope() {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(500_000_000);
    let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
    budget.reserve_storage(MODULE_FLOOR).unwrap();
    let prepared = scalar_payload_prepared_v18(&mut budget);
    let calls = std::cell::Cell::new(0);
    prepared
        .with_source_consumer_v18(&mut budget, |source, budget| -> SourceOwnedResultV18<()> {
            source.with_analysis_v18(budget, |scope| {
                scope.with_inventory_v1(|inventory, budget| {
                    source.with_ranked_correspondence_v18(inventory, budget, |relation, budget| {
                        let before = budget.storage();
                        relation.with_source_scalar_leaves_v18(
                            0,
                            budget,
                            |view, budget| -> SourceOwnedResultV18<()> {
                                assert_eq!(view.leaves.floor, budget.storage());
                                view.leaves.query(budget)?;
                                assert!(view.leaves.rows.len() >= 4);
                                assert_eq!(view.leaves.lookup.len(), view.leaves.rows.len() * 3);
                                budget.reserve_storage(19)?;
                                calls.set(calls.get() + 1);
                                Ok(())
                            },
                        )?;
                        assert_eq!(budget.storage(), before + 19);
                        assert!(!source.cleanup.is_denied());
                        source.check_query_v18(budget)?;
                        budget.release_storage(19)?;
                        assert_eq!(budget.storage(), before);
                        Ok(())
                    })
                })
            })
        })
        .unwrap();
    assert_eq!(calls.get(), 1);
    assert_eq!(budget.storage(), MODULE_FLOOR);
}

#[test]
fn scalar_leaf_rebased_floor_still_denies_restored_undercut_and_keeps_selected_error() {
    for selected in [false, true] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(500_000_000);
        let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
        budget.reserve_storage(MODULE_FLOOR).unwrap();
        let prepared = scalar_payload_prepared_v18(&mut budget);
        let paid = std::cell::Cell::new(0);
        let result = prepared.with_source_consumer_v18(
            &mut budget,
            |source, budget| -> SourceOwnedResultV18<()> {
                source.with_analysis_v18(budget, |scope| {
                    scope.with_inventory_v1(|inventory, budget| {
                        source.with_ranked_correspondence_v18(
                            inventory,
                            budget,
                            |relation, budget| {
                                relation.with_source_scalar_leaves_v18(
                                    0,
                                    budget,
                                    |view, budget| -> SourceOwnedResultV18<()> {
                                        assert_eq!(view.leaves.floor, budget.storage());
                                        paid.set(budget.storage());
                                        if selected {
                                            let error = source
                                                .missing::<()>("selected leaf before undercut")
                                                .unwrap_err();
                                            assert!(matches!(
                                                error,
                                                ProductionSourceOwnedViewErrorV18::Binding(
                                                    "selected leaf before undercut"
                                                )
                                            ));
                                        }
                                        budget.release_storage(1)?;
                                        let error = view.leaves.query(budget).unwrap_err();
                                        budget.reserve_storage(1)?;
                                        assert_eq!(budget.storage(), paid.get());
                                        assert!(source.cleanup.is_denied());
                                        Err(error)
                                    },
                                )
                            },
                        )
                    })
                })
            },
        );
        assert!(paid.get() > MODULE_FLOOR);
        if selected {
            assert!(matches!(
                result,
                Err(ProductionSourceOwnedViewErrorV18::Binding(
                    "selected leaf before undercut"
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
        assert_eq!(budget.storage(), paid.get());
    }
}
