fn with_selected_identity_transition_v30(
    original: &ProductionSourceCorrespondenceV18<'_>,
    budget: &mut ArgumentBudgetV1<'_>,
    consume: impl FnOnce(
        &ProductionOptimizedSourceCorrespondenceV18<'_>,
        &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<()>,
) -> SourceOwnedResultV18<()> {
    use fe2o3_kernel_ir::*;
    let input = original.inventory;
    let one = |i: usize| CanonicalKirTransitionRangeV1 {
        start: u32::try_from(i).unwrap(),
        len: 1,
    };
    // Test-owned complete candidate rows still pass the independent checker.
    let functions: Vec<_> = input
        .functions()
        .iter()
        .map(|row| CanonicalKirFunctionTransitionV1 {
            input: row.coordinate,
            output: row.coordinate,
        })
        .collect();
    let blocks: Vec<_> = input
        .blocks()
        .iter()
        .enumerate()
        .map(|(i, row)| CanonicalKirBlockTransitionV1 {
            output: row.coordinate,
            segments: one(i),
        })
        .collect();
    let segments: Vec<_> = input
        .blocks()
        .iter()
        .map(|row| CanonicalKirBlockSegmentV1 {
            input: row.coordinate,
            connector: None,
        })
        .collect();
    let operations: Vec<_> = input
        .operations()
        .iter()
        .map(|row| CanonicalKirOperationTransitionV1 {
            output: row.coordinate,
            origin: CanonicalKirOperationOriginV1::Retained(row.coordinate),
        })
        .collect();
    let definitions: Vec<_> = input
        .definitions()
        .iter()
        .enumerate()
        .map(|(i, row)| CanonicalKirDefinitionTransitionV1 {
            input: row.coordinate,
            outputs: one(i),
        })
        .collect();
    let outputs: Vec<_> = input
        .definitions()
        .iter()
        .map(|row| CanonicalKirDefinitionDescendantV1 {
            output: row.coordinate,
            kind: CanonicalKirDefinitionDescendantKindV1::Retained,
        })
        .collect();
    let uses: Vec<_> = input
        .uses()
        .iter()
        .map(|row| CanonicalKirUseTransitionV1 {
            input: row.coordinate,
            output: row.coordinate,
        })
        .collect();
    let edges: Vec<_> = input
        .edges()
        .iter()
        .map(|row| CanonicalKirEdgeTransitionV1 {
            input: row.coordinate,
            output: row.coordinate,
        })
        .collect();
    let arguments: Vec<_> = input
        .edge_arguments()
        .iter()
        .map(|row| CanonicalKirEdgeArgumentTransitionV1 {
            input: row.coordinate,
            output: row.coordinate,
        })
        .collect();
    let rows = CanonicalKirTransitionCandidateV1 {
        functions: &functions,
        blocks: &blocks,
        segments: &segments,
        operations: &operations,
        definitions: &definitions,
        definition_outputs: &outputs,
        uses: &uses,
        edges: &edges,
        edge_arguments: &arguments,
    };
    let paid = std::mem::size_of_val(rows.functions)
        + std::mem::size_of_val(rows.blocks)
        + std::mem::size_of_val(rows.segments)
        + std::mem::size_of_val(rows.operations)
        + std::mem::size_of_val(rows.definitions)
        + std::mem::size_of_val(rows.definition_outputs)
        + std::mem::size_of_val(rows.uses)
        + std::mem::size_of_val(rows.edges)
        + std::mem::size_of_val(rows.edge_arguments);
    budget.reserve_storage(paid)?;
    let result = (|| {
        let (checked, receipt) =
            fe2o3_kernel_analysis::check_canonical_kir_transition_v18(input, input, rows, budget)
                .map_err(|error| match error {
                fe2o3_kernel_analysis::CanonicalKirTransitionErrorV1::Resource(error) => {
                    ProductionSourceOwnedViewErrorV18::Resource(error)
                }
                _ => ProductionSourceOwnedViewErrorV18::Binding(
                    "genuine original identity transition differs",
                ),
            })?;
        budget.reserve_storage(receipt.retained_storage())?;
        let result = original.with_optimized_correspondence_v18(&checked, budget, consume);
        drop(checked);
        budget.release_storage(receipt.retained_storage())?;
        result
    })();
    drop((
        functions,
        blocks,
        segments,
        operations,
        definitions,
        outputs,
        uses,
        edges,
        arguments,
    ));
    budget.release_storage(paid)?;
    result
}

fn with_selected_actual_optimizer_v30(
    original: &ProductionSourceCorrespondenceV18<'_>,
    budget: &mut ArgumentBudgetV1<'_>,
    consume: impl FnOnce(
        &ProductionOptimizedSourceCorrespondenceV18<'_>,
        &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<()>,
) -> SourceOwnedResultV18<()> {
    let floor = budget.storage();
    let observed = fe2o3_pliron::optimize_neutral_kernel_ir_v18(
        &original.source.owner.inner.pending.graph,
        ProductionSemanticKirLimitsV1::default().storage_layout_limits,
        budget,
    )
    .expect("actual selected-source optimization");
    budget.reserve_storage(observed.storage().retained_storage())?;
    let result = observed.try_check_and_finish_with_v18(budget, |checked, budget| {
        original
            .source
            .with_ranked_correspondence_v18(checked.input(), budget, |source, budget| {
                source.with_optimized_correspondence_v18(checked, budget, consume)
            })
            .map(|()| ((), 0))
    });
    let result = match result {
        Ok((owner, (), receipt)) => {
            assert_eq!(
                receipt.retained_storage(),
                std::mem::size_of::<fe2o3_pliron::KirNeutralOwnedOriginStorageV1>()
            );
            drop(owner);
            Ok(())
        }
        Err(fe2o3_pliron::KirCheckedNeutralOptimizationErrorV1::Origin(error)) => Err(error),
        Err(fe2o3_pliron::KirCheckedNeutralOptimizationErrorV1::Resource(error)) => {
            Err(error.into())
        }
        Err(error) => panic!("selected checked optimizer result: {error:?}"),
    };
    assert_eq!(budget.storage(), floor);
    result
}

#[test]
fn selected_optimized_identity_retains_diamond_parallel_recurrence_and_mixed_original_obligations()
{
    for fixture in 0..5 {
        let owner = match fixture {
            0 => selection_owner(false, false),
            1 => selection_owner(true, false),
            2 => selection_owner(true, true),
            3 => mixed_selected_memory_owner(),
            _ => entry_loop_selection_owner(),
        };
        let reached = std::cell::Cell::new(false);
        let (result, _, _) = run_selected_memory_owner(
            owner,
            SELECTED_MEMORY_LIMIT,
            SELECTED_MEMORY_LIMIT,
            |original, budget| {
                with_selected_identity_transition_v30(original, budget, |view, budget| {
                    let accesses = if fixture == 3 {
                        1
                    } else if fixture == 4 {
                        0
                    } else {
                        2
                    };
                    optimized_source_v18::selection::assert_transport_v30(
                        view, accesses, true, budget,
                    )?;
                    reached.set(true);
                    Ok(())
                })
            },
        );
        result.unwrap_or_else(|error| panic!("identity fixture={fixture}: {error:?}"));
        assert!(reached.get());
    }
}

#[test]
fn selected_optimized_actual_checked_pipeline_preserves_complete_conditional_rosters() {
    for fixture in 0..3 {
        let owner = match fixture {
            0 => selection_owner(false, false),
            1 => selection_owner(true, true),
            _ => mixed_selected_memory_owner(),
        };
        let reached = std::cell::Cell::new(false);
        let (result, _, _) = run_selected_memory_owner(
            owner,
            SELECTED_MEMORY_LIMIT,
            SELECTED_MEMORY_LIMIT,
            |original, budget| {
                with_selected_actual_optimizer_v30(original, budget, |view, budget| {
                    optimized_source_v18::selection::assert_transport_v30(
                        view,
                        if fixture == 2 { 1 } else { 2 },
                        false,
                        budget,
                    )?;
                    reached.set(true);
                    Ok(())
                })
            },
        );
        result.unwrap_or_else(|error| panic!("actual optimizer fixture={fixture}: {error:?}"));
        assert!(reached.get());
    }
}

#[test]
fn selected_optimized_entry_loop_keeps_invocation_separate_from_ordered_backedges() {
    let joined = selection_owner(true, false);
    let looped = entry_loop_selection_owner();
    let source = joined.source_semantic();
    let helper = &looped.source_semantic().functions()[1];
    let owner = admitted_owner(
        source.types().to_vec(),
        vec![source.functions()[0].clone(), helper.clone()],
        source.callables().to_vec(),
    );
    let reached = std::cell::Cell::new(false);
    let (result, _, _) = run_selected_memory_owner(
        owner,
        SELECTED_MEMORY_LIMIT,
        SELECTED_MEMORY_LIMIT,
        |original, budget| {
            with_selected_identity_transition_v30(original, budget, |view, budget| {
                let (incoming, invocations, _) =
                    optimized_source_v18::selection::assert_transport_v30(view, 1, true, budget)?;
                assert!(
                    incoming >= 3 && invocations > 0,
                    "original invocation and recurrence must both survive"
                );
                reached.set(true);
                Ok(())
            })
        },
    );
    result.unwrap();
    assert!(reached.get());
}

#[test]
fn selected_optimized_replay_rejects_missing_reordered_foreign_and_erased_choice_facts() {
    for fault in 0..8 {
        let reached = std::cell::Cell::new(false);
        let (result, _, _) = run_selected_memory_owner(
            selection_owner(true, true),
            SELECTED_MEMORY_LIMIT,
            SELECTED_MEMORY_LIMIT,
            |original, budget| {
                with_selected_identity_transition_v30(original, budget, |view, budget| {
                    reached.set(true);
                    optimized_source_v18::selection::corrupt_transport_v30(view, fault, budget)
                })
            },
        );
        assert!(reached.get(), "fault={fault}, result={result:?}");
        assert!(
            matches!(
                result,
                Err(ProductionSourceOwnedViewErrorV18::Binding(
                    "selected optimized transport differs from its complete original relation"
                ))
            ),
            "fault={fault}: {result:?}"
        );
    }
    let (restored, _, _) = run_selected_memory_owner(
        selection_owner(true, true),
        SELECTED_MEMORY_LIMIT,
        SELECTED_MEMORY_LIMIT,
        |original, budget| {
            with_selected_identity_transition_v30(original, budget, |view, budget| {
                view.replay_selected_transport_v30(0, budget)
            })
        },
    );
    restored.unwrap();
}

#[test]
fn selected_optimized_full_transaction_exact_and_one_short_limits_preserve_cleanup() {
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
                    view.replay_selected_transport_v30(0, budget)?;
                    reached.set(true);
                    Ok(())
                })
            },
        )
    }
    let reached = std::cell::Cell::new(false);
    let (baseline, work, storage) = run(SELECTED_MEMORY_LIMIT, SELECTED_MEMORY_LIMIT, &reached);
    baseline.unwrap();
    assert!(reached.replace(false));
    run(work, storage, &reached).0.unwrap();
    assert!(reached.replace(false));
    let short = run(work - 1, storage, &reached).0;
    assert!(
        matches!(scoped_raw_admission_v29::issued_role_tests_v29::issued_role_resource_v18(short.unwrap_err()),
        ArgumentResourceV1::Work(error) if error.actual() == work && error.limit() == work - 1)
    );
    reached.set(false);
    let short = run(work, storage - 1, &reached).0;
    assert!(
        matches!(scoped_raw_admission_v29::issued_role_tests_v29::issued_role_resource_v18(short.unwrap_err()),
        ArgumentResourceV1::Storage(error) if error.actual() == storage && error.limit() == storage - 1)
    );
}
