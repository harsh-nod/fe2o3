fn with_selected_final_domains_v30(
    optimized: &ProductionOptimizedSourceCorrespondenceV18<'_>,
    budget: &mut ArgumentBudgetV1<'_>,
    consume: impl for<'scope, 'work> FnOnce(
        &optimized_source_v18::selection::final_source::CheckedSelectedFinalSourcesV30<'scope>,
        &mut ArgumentBudgetV1<'work>,
    ) -> SourceOwnedResultV18<()>,
) -> SourceOwnedResultV18<()> {
    use fe2o3_kernel_ir::*;
    let output = optimized.output_inventory(budget)?;
    let launches = vec![
        ExplicitLaunchExtent::Exact {
            rank: 3,
            extents: [64, 1, 1]
        };
        output.functions().len()
    ];
    let floor = budget.storage();
    let result = with_canonical_selected_slice_domains_v30(
        output.owner(),
        &launches,
        FormalIndexWidth::Bits64,
        CanonicalGuardedGlobalReadLimitsV1::default(),
        budget,
        |domains, budget| Ok(optimized.with_selected_final_sources_v30(domains, 0, budget, consume)),
    );
    let result = match result {
        Ok(Some(result)) => result,
        Ok(None) => Err(ProductionSourceOwnedViewErrorV18::Binding(
            "genuine selected source has no complete actual domain",
        )),
        Err(error) => Err(optimized_source_observed_formal_error_v18(
            optimized.original_for_test_v30(),
            &error,
        )),
    };
    assert_eq!(budget.storage(), floor);
    result
}

fn selected_final_entry_loop_owner_v30() -> ProductionSemanticSsaOwnerV1 {
    let joined = selection_owner(true, false);
    let looped = entry_loop_selection_owner();
    let source = joined.source_semantic();
    admitted_owner(
        source.types().to_vec(),
        vec![
            source.functions()[0].clone(),
            looped.source_semantic().functions()[1].clone(),
        ],
        source.callables().to_vec(),
    )
}

#[test]
fn selected_final_source_domains_join_genuine_diamonds_parallel_edges_loops_and_mixed_roots() {
    for fixture in 0..5 {
        for actual_optimizer in [false, true] {
            let owner = match fixture {
                0 => selection_owner(false, false),
                1 => selection_owner(true, false),
                2 => selection_owner(true, true),
                3 => mixed_selected_memory_owner(),
                _ => selected_final_entry_loop_owner_v30(),
            };
            let reached = std::cell::Cell::new(false);
            let (result, _, _) = run_selected_memory_owner(
                owner,
                SELECTED_MEMORY_LIMIT,
                SELECTED_MEMORY_LIMIT,
                |original, budget| {
                    let inspect =
                        |view: &ProductionOptimizedSourceCorrespondenceV18<'_>,
                         budget: &mut ArgumentBudgetV1<'_>| {
                            with_selected_final_domains_v30(view, budget, |joined, budget| {
                                optimized_source_v18::selection::final_source::assert_join_v30(
                                    joined,
                                    if fixture < 3 { 2 } else { 1 },
                                    fixture == 3,
                                    budget,
                                )?;
                                reached.set(true);
                                Ok(())
                            })
                        };
                    if actual_optimizer {
                        with_selected_actual_optimizer_v30(original, budget, inspect)
                    } else {
                        with_selected_identity_transition_v30(original, budget, inspect)
                    }
                },
            );
            result.unwrap_or_else(|error| {
                panic!("selected final fixture={fixture} optimized={actual_optimizer}: {error:?}")
            });
            assert!(reached.get());
        }
    }
}

#[test]
fn selected_final_source_empty_roster_does_not_claim_global_completion() {
    let reached = std::cell::Cell::new(false);
    let (result, _, _) = run_selected_memory_owner(
        helper_owner(false, false, false),
        SELECTED_MEMORY_LIMIT,
        SELECTED_MEMORY_LIMIT,
        |original, budget| {
            with_selected_identity_transition_v30(original, budget, |view, budget| {
                with_selected_final_domains_v30(view, budget, |joined, budget| {
                    assert_eq!(joined.original_access_count(budget)?, 0);
                    assert_eq!(joined.retained_choice_count(budget)?, 0);
                    assert!(!joined.grants_artifact_or_launch_authority());
                    reached.set(true);
                    Ok(())
                })
            })
        },
    );
    result.unwrap();
    assert!(reached.get());
}

#[test]
fn selected_final_source_full_transaction_keeps_exact_and_one_short_resource_counts() {
    fn run(
        work: usize,
        storage: usize,
        reached: &std::cell::Cell<bool>,
    ) -> (SourceOwnedResultV18<()>, usize, usize) {
        run_selected_memory_owner(
            selection_owner(true, true),
            work,
            storage,
            |original, budget| {
                with_selected_identity_transition_v30(original, budget, |view, budget| {
                    with_selected_final_domains_v30(view, budget, |joined, budget| {
                        optimized_source_v18::selection::final_source::assert_join_v30(
                            joined, 2, false, budget,
                        )?;
                        reached.set(true);
                        Ok(())
                    })
                })
            },
        )
    }
    let reached = std::cell::Cell::new(false);
    let (baseline, work, storage) = run(SELECTED_MEMORY_LIMIT, SELECTED_MEMORY_LIMIT, &reached);
    baseline.unwrap();
    assert!(reached.replace(false));
    let exact = run(work, storage, &reached);
    exact.0.unwrap();
    assert_eq!((exact.1, exact.2), (work, storage));
    assert!(reached.replace(false));
    let short = run(work - 1, storage, &reached).0;
    assert!(
        matches!(scoped_raw_admission_v29::issued_role_tests_v29::issued_role_resource_v18(short.unwrap_err()), ArgumentResourceV1::Work(error) if error.actual() == work && error.limit() == work - 1)
    );
    reached.set(false);
    let short = run(work, storage - 1, &reached).0;
    assert!(
        matches!(scoped_raw_admission_v29::issued_role_tests_v29::issued_role_resource_v18(short.unwrap_err()), ArgumentResourceV1::Storage(error) if error.actual() == storage && error.limit() == storage - 1)
    );
}

#[test]
fn selected_final_source_rejects_changed_projection_indexes_with_exact_diagnostics() {
    for fault in 0..4 {
        let reached = std::cell::Cell::new(false);
        let (result, _, _) = run_selected_memory_owner(
            selection_owner(true, true),
            SELECTED_MEMORY_LIMIT,
            SELECTED_MEMORY_LIMIT,
            |original, budget| {
                with_selected_identity_transition_v30(original, budget, |view, budget| {
                    with_selected_final_domains_v30(view, budget, |joined, budget| {
                        reached.set(true);
                        optimized_source_v18::selection::final_source::hostile_index_v30(
                            joined, fault, budget,
                        )
                    })
                })
            },
        );
        result.unwrap_or_else(|error| panic!("hostile setup {fault}: {error:?}"));
        assert!(reached.get());
    }
}

#[test]
fn selected_final_source_rejects_byte_identical_foreign_domain_owner_before_consumer() {
    let (result, _, _) = run_selected_memory_owner(
        selection_owner(true, false),
        SELECTED_MEMORY_LIMIT,
        SELECTED_MEMORY_LIMIT,
        |original, budget| {
            with_selected_identity_transition_v30(original, budget, |view, budget| {
                optimized_source_v18::selection::final_source::foreign_domain_owner_v30(
                    view, budget,
                )
            })
        },
    );
    assert!(
        matches!(
            result,
            Err(ProductionSourceOwnedViewErrorV18::Binding(
                "selected final domain owner differs from the exact optimized source output"
            ))
        ),
        "{result:?}"
    );
}

#[test]
fn selected_final_source_queries_latch_funded_foreign_ledger_and_restored_credit_refusals() {
    for lost_credit in [false, true] {
        let retained = std::cell::Cell::new(None);
        let reached = std::cell::Cell::new(false);
        let (result, _, _) =
            scoped_raw_admission_v29::issued_role_tests_v29::run_issued_role_owner_v18(
                selection_owner(true, false),
                SELECTED_MEMORY_LIMIT,
                SELECTED_MEMORY_LIMIT,
                &retained,
                |original, budget| {
                    let result =
                        with_selected_identity_transition_v30(original, budget, |view, budget| {
                            optimized_source_v18::selection::final_source::poison_join_v30(
                                view,
                                lost_credit,
                                budget,
                            )
                        });
                    assert!(
                        matches!(
                            result,
                            Err(ProductionSourceOwnedViewErrorV18::Resource(
                                ArgumentResourceV1::Accounting
                            ))
                        ),
                        "{result:?}"
                    );
                    retained.set(Some(budget.storage()));
                    reached.set(true);
                    Ok(())
                },
            );
        assert!(reached.get());
        assert!(
            matches!(
                result,
                Err(ProductionSourceOwnedViewErrorV18::Resource(
                    ArgumentResourceV1::Accounting
                ))
            ),
            "{result:?}"
        );
        assert!(retained.get().unwrap() > 37);
    }
}

#[test]
fn selected_final_source_raw_callback_and_constructor_accounting_deny_refund_before_cleanup() {
    for case in [1usize, 2] {
        let retained = std::cell::Cell::new(None);
        let reached = std::cell::Cell::new(false);
        let (result, _, _) =
            scoped_raw_admission_v29::issued_role_tests_v29::run_issued_role_owner_v18(
                selection_owner(true, false),
                SELECTED_MEMORY_LIMIT,
                SELECTED_MEMORY_LIMIT,
                &retained,
                |original, budget| {
                    let result =
                        with_selected_identity_transition_v30(original, budget, |view, budget| {
                            optimized_source_v18::selection::final_source::raw_result_custody_v30(
                                view, case, budget,
                            )
                        });
                    assert!(
                        matches!(
                            result,
                            Err(ProductionSourceOwnedViewErrorV18::Resource(
                                ArgumentResourceV1::Accounting
                            ))
                        ),
                        "{result:?}"
                    );
                    retained.set(Some(budget.storage()));
                    reached.set(true);
                    Ok(())
                },
            );
        assert!(reached.get());
        assert!(
            matches!(
                result,
                Err(ProductionSourceOwnedViewErrorV18::Resource(
                    ArgumentResourceV1::Accounting
                ))
            ),
            "{result:?}"
        );
        assert!(retained.get().unwrap() > 37);
    }
}

#[test]
fn selected_final_source_ordinary_callback_binding_keeps_normal_refund_and_exact_error() {
    let (result, _, _) = run_selected_memory_owner(
        selection_owner(true, false),
        SELECTED_MEMORY_LIMIT,
        SELECTED_MEMORY_LIMIT,
        |original, budget| {
            with_selected_identity_transition_v30(original, budget, |view, budget| {
                optimized_source_v18::selection::final_source::raw_result_custody_v30(
                    view, 0, budget,
                )
            })
        },
    );
    assert!(
        matches!(
            result,
            Err(ProductionSourceOwnedViewErrorV18::Binding(
                "selected test callback binding"
            ))
        ),
        "{result:?}"
    );
}

#[test]
fn selected_final_source_accounting_keeps_the_prior_binding_and_denied_credit() {
    let retained = std::cell::Cell::new(None);
    let reached = std::cell::Cell::new(false);
    let (result, _, _) = scoped_raw_admission_v29::issued_role_tests_v29::run_issued_role_owner_v18(
        selection_owner(true, false),
        SELECTED_MEMORY_LIMIT,
        SELECTED_MEMORY_LIMIT,
        &retained,
        |original, budget| {
            let result = with_selected_identity_transition_v30(original, budget, |view, budget| {
                optimized_source_v18::selection::final_source::raw_result_custody_v30(
                    view, 3, budget,
                )
            });
            assert!(
                matches!(
                    result,
                    Err(ProductionSourceOwnedViewErrorV18::Binding(
                        "selected test first binding"
                    ))
                ),
                "{result:?}"
            );
            retained.set(Some(budget.storage()));
            reached.set(true);
            Ok(())
        },
    );
    assert!(reached.get());
    assert!(
        matches!(
            result,
            Err(ProductionSourceOwnedViewErrorV18::Binding(
                "selected test first binding"
            ))
        ),
        "{result:?}"
    );
    assert!(retained.get().unwrap() > 37);
}
