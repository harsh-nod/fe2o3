#[test]
fn explicit_source_tile_expansion_retains_source_layout_and_complete_operation_spans() {
    for layout in [
        ExecutionTileLayoutV1::Blocked,
        ExecutionTileLayoutV1::Striped,
    ] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(OPTIMIZED_SOURCE_WORK_LIMIT_V18);
        let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
        budget.reserve_storage(MODULE_FLOOR).unwrap();
        let prepared = prepared_tile_schedule_v155(&mut budget);
        with_actual_optimized_source_v18(prepared, &mut budget, |view, budget| {
            let floor = budget.storage();
            let expanded = view.prepare_tile_expansion_v159(0, layout, budget)?;
            assert_eq!(budget.storage(), floor + expanded.retained_storage(budget)?);
            assert_eq!(expanded.selections(budget)?.len(), 1);
            assert_eq!(expanded.selections(budget)?[0].layout, layout);
            expanded.replay(budget)?;
            let mut loads = 0;
            let mut fragments = 0;
            let mut prefix_rewrites = 0;
            for row in view.input_inventory(budget)?.operations() {
                if matches!(
                    view.operation(row.coordinate, budget)?,
                    ProductionOptimizedSourceOperationV18::Rewritten { .. }
                ) {
                    prefix_rewrites += 1;
                    continue;
                }
                let Some(span) = expanded.operation_span(row.coordinate, budget)? else {
                    continue;
                };
                assert_eq!(span.original, row.coordinate);
                let module = expanded.output(budget)?.module();
                let block = &module.functions[span.expansion.input.block.function.0 as usize]
                    .body
                    .as_ref()
                    .unwrap()
                    .blocks[span.expansion.input.block.block as usize];
                let actual =
                    &block.operations[span.expansion.first as usize..span.expansion.end as usize];
                match row.operation.kind {
                    OperationKind::Execution(ExecutionOperationV15::MaskedTileLoadU32 {
                        elements,
                        ..
                    }) => {
                        assert_eq!(actual.len(), 5 + 11 * elements as usize);
                        assert_eq!(
                            actual
                                .iter()
                                .filter(|op| matches!(op.kind, OperationKind::GuardedLoad { .. }))
                                .count(),
                            elements as usize
                        );
                        loads += 1;
                    }
                    OperationKind::Execution(ExecutionOperationV15::TileIntoFragmentU32 {
                        ..
                    }) => {
                        assert!(actual.is_empty());
                        fragments += 1;
                    }
                    _ => {}
                }
            }
            assert!(loads >= 2 && fragments >= 1);
            assert!(prefix_rewrites > 0);
            assert!(!expanded.grants_artifact_or_launch_authority());
            expanded.discard(budget)?;
            assert_eq!(budget.storage(), floor);
            Ok(())
        })
        .unwrap();
        assert_eq!(budget.storage(), MODULE_FLOOR);
    }
}

#[test]
fn explicit_source_tile_expansion_refuses_foreign_budget_and_live_storage_refund() {
    for foreign in [false, true] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(OPTIMIZED_SOURCE_WORK_LIMIT_V18);
        let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
        budget.reserve_storage(MODULE_FLOOR).unwrap();
        let prepared = prepared_tile_schedule_v155(&mut budget);
        let result = with_actual_optimized_source_v18(prepared, &mut budget, |view, budget| {
            let expanded =
                view.prepare_tile_expansion_v159(0, ExecutionTileLayoutV1::Blocked, budget)?;
            let failure = if foreign {
                let mut other_work =
                    CanonicalKernelIrWorkBudgetV1::new(OPTIMIZED_SOURCE_WORK_LIMIT_V18);
                let mut other = ArgumentBudgetV1::new(&mut other_work, MODULE_LIMIT);
                other.reserve_storage(budget.storage())?;
                expanded.output(&other).err().unwrap()
            } else {
                budget.release_storage(1)?;
                expanded.output(budget).err().unwrap()
            };
            assert!(matches!(
                failure,
                ProductionSourceOwnedViewErrorV18::Resource(ArgumentResourceV1::Accounting)
            ));
            assert!(expanded.output(budget).is_err());
            assert!(expanded.discard(budget).is_err());
            Err::<(), _>(failure)
        });
        assert!(result.is_err());
    }
}

#[test]
fn explicit_source_tile_expansion_does_not_invent_spans_for_prefix_rewrites() {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(OPTIMIZED_SOURCE_WORK_LIMIT_V18);
    let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
    budget.reserve_storage(MODULE_FLOOR).unwrap();
    let prepared = scalar_payload_prepared_from_v18(folding_source_owner_v18, &mut budget);
    let result = with_actual_optimized_source_v18(prepared, &mut budget, |view, budget| {
        let mut rewritten = None;
        for row in view.input_inventory(budget)?.operations() {
            if matches!(
                view.operation(row.coordinate, budget)?,
                ProductionOptimizedSourceOperationV18::Rewritten { .. }
            ) {
                rewritten = Some(row.coordinate);
                break;
            }
        }
        let rewritten = rewritten.expect("actual neutral scalar fold");
        let floor = budget.storage();
        let expanded =
            view.prepare_tile_expansion_v159(0, ExecutionTileLayoutV1::Blocked, budget)?;
        let error = expanded.operation_span(rewritten, budget).unwrap_err();
        assert!(matches!(
            error,
            ProductionSourceOwnedViewErrorV18::Binding(
                "source tile span prefix operation was rewritten"
            )
        ));
        assert!(expanded.discard(budget).is_err());
        assert_eq!(
            budget.storage(),
            floor,
            "selected refusal still disposes intact owned credit"
        );
        Err::<(), _>(error)
    });
    assert!(matches!(
        result,
        Err(ProductionSourceOwnedViewErrorV18::Binding(
            "source tile span prefix operation was rewritten"
        ))
    ));
    assert_eq!(budget.storage(), MODULE_FLOOR);
}

#[test]
fn explicit_source_tile_expansion_panic_disposes_retained_graph_before_refund() {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(OPTIMIZED_SOURCE_WORK_LIMIT_V18);
    let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
    budget.reserve_storage(MODULE_FLOOR).unwrap();
    let prepared = prepared_tile_schedule_v155(&mut budget);
    let result = with_actual_optimized_source_v18(prepared, &mut budget, |view, budget| {
        view.test_tile_expansion_panic_after_replay_v159();
        let expanded =
            view.prepare_tile_expansion_v159(0, ExecutionTileLayoutV1::Blocked, budget)?;
        expanded.discard(budget)?;
        panic!("constructor panic injection did not run")
    });
    assert!(matches!(
        result,
        Err(ProductionSourceOwnedViewErrorV18::Binding(
            "actual optimizer consumer panicked"
        ))
    ));
    assert_eq!(budget.storage(), MODULE_FLOOR);
}

fn tile_expansion_resource_run_v159(
    work_limit: usize,
    storage_limit: usize,
) -> (
    Result<(), ProductionSourceOptimizationErrorV18<ProductionSourceOwnedViewErrorV18>>,
    usize,
    usize,
) {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(work_limit);
    let mut budget = ArgumentBudgetV1::new(&mut work, storage_limit);
    budget.reserve_storage(MODULE_FLOOR).unwrap();
    let result = (|| {
        let prepared = prepared_tile_schedule_result_v155(&mut budget)
            .map_err(ProductionSourceOptimizationErrorV18::Source)?;
        prepared.with_source_consumer_v18(&mut budget, |source, budget| {
            let (output, (), _) =
                source.with_checked_optimization_v18(budget, |_, view, budget| {
                    let expanded = view.prepare_tile_expansion_v159(
                        0,
                        ExecutionTileLayoutV1::Striped,
                        budget,
                    )?;
                    let replay = expanded.replay(budget);
                    let settled = expanded.discard(budget);
                    replay?;
                    settled?;
                    Ok::<_, ProductionSourceOwnedViewErrorV18>(((), 0))
                })?;
            drop(output);
            Ok(())
        })
    })();
    assert_eq!(budget.storage(), MODULE_FLOOR, "{result:?}");
    (result, budget.work(), budget.peak_storage())
}

#[test]
fn explicit_source_tile_expansion_transaction_has_exact_and_one_short_resource_boundaries() {
    let (result, work, storage) =
        tile_expansion_resource_run_v159(OPTIMIZED_SOURCE_WORK_LIMIT_V18, MODULE_LIMIT);
    result.unwrap();
    let (exact, used, peak) = tile_expansion_resource_run_v159(work, storage);
    exact.unwrap();
    assert_eq!((used, peak), (work, storage));
    for (work_limit, storage_limit, is_work) in
        [(work - 1, storage, true), (work, storage - 1, false)]
    {
        let (result, _, _) = tile_expansion_resource_run_v159(work_limit, storage_limit);
        let error = match result {
            Err(ProductionSourceOptimizationErrorV18::Source(
                ProductionSourceOwnedViewErrorV18::Resource(error),
            ))
            | Err(ProductionSourceOptimizationErrorV18::Adoption(
                fe2o3_pliron::KirCheckedNeutralOptimizationErrorV1::Origin(
                    ProductionSourceOwnedViewErrorV18::Resource(error),
                ),
            ))
            | Err(ProductionSourceOptimizationErrorV18::Adoption(
                fe2o3_pliron::KirCheckedNeutralOptimizationErrorV1::Resource(error),
            )) => error,
            other => panic!("expected exact expansion resource refusal: {other:?}"),
        };
        assert!(if is_work {
            matches!(error, ArgumentResourceV1::Work(_))
        } else {
            matches!(error, ArgumentResourceV1::Storage(_))
        });
    }
}
