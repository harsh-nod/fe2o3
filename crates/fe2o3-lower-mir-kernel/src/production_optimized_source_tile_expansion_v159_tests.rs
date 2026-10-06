#[test]
fn explicit_source_tile_expansion_binds_original_leaves_to_actual_scalar_definitions() {
    use fe2o3_kernel_ir::{CanonicalKirDefinitionCoordinateV1 as Definition, ScalarType};
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
            assert!(std::ptr::eq(expanded.neutral_source_v162(budget)?, view));
            assert!(std::ptr::eq(
                expanded.original_source_v162(budget)?.source(budget)?,
                view.original_source(budget)?
            ));
            let (function, selected, lanes) = expanded.root_policy_v162(0, budget)?.unwrap();
            assert_eq!(selected, layout);
            assert_eq!(lanes, 64);
            let mut roles = 0;
            for row in view.input_inventory(budget)?.operations() {
                let elements = match row.operation.kind {
                    OperationKind::Execution(
                        ExecutionOperationV15::MaskedTileLoadU32 { elements, .. }
                        | ExecutionOperationV15::TileIntoFragmentU32 { elements, .. },
                    ) => elements,
                    _ => continue,
                };
                let original = Definition::Result {
                    operation: row.coordinate,
                    result: 0,
                };
                for marker in [2, 3] {
                    assert_eq!(
                        expanded.aggregate_leaf_v162(original, &[marker], budget)?,
                        ProductionSourceTileLeafV162::Unit
                    );
                }
                for field in 0..2 {
                    for element in 0..u32::from(elements) {
                        let ProductionSourceTileLeafV162::Scalar {
                            function: actual_function,
                            value,
                            scalar,
                        } = expanded.aggregate_leaf_v162(original, &[field, element], budget)?
                        else {
                            panic!("scalar tile leaf returned unit");
                        };
                        assert_eq!(actual_function, function);
                        let expected = if field == 0 {
                            ScalarType::U32
                        } else {
                            ScalarType::Bool
                        };
                        assert_eq!(scalar, expected);
                        let body = expanded.output(budget)?.module().functions[function.0 as usize]
                            .body
                            .as_ref()
                            .unwrap();
                        let definition = body
                            .blocks
                            .iter()
                            .flat_map(|block| &block.operations)
                            .flat_map(|operation| &operation.results)
                            .find(|result| result.id == value)
                            .unwrap();
                        assert_eq!(definition.ty, Type::Scalar(expected));
                    }
                }
                roles += 1;
            }
            assert!(roles >= 3);
            expanded.replay(budget)?;
            expanded.discard(budget)?;
            assert_eq!(budget.storage(), floor);
            Ok(())
        })
        .unwrap();
        assert_eq!(budget.storage(), MODULE_FLOOR);
    }
}

#[test]
fn explicit_source_tile_expansion_refuses_invalid_aggregate_leaf_paths() {
    use fe2o3_kernel_ir::CanonicalKirDefinitionCoordinateV1 as Definition;
    for path in [
        vec![],
        vec![0],
        vec![0, u32::MAX],
        vec![1, u32::MAX],
        vec![2, 0],
        vec![4],
    ] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(OPTIMIZED_SOURCE_WORK_LIMIT_V18);
        let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
        budget.reserve_storage(MODULE_FLOOR).unwrap();
        let prepared = prepared_tile_schedule_v155(&mut budget);
        let result = with_actual_optimized_source_v18(prepared, &mut budget, |view, budget| {
            let original = view
                .input_inventory(budget)?
                .operations()
                .iter()
                .find(|row| {
                    matches!(
                        row.operation.kind,
                        OperationKind::Execution(ExecutionOperationV15::MaskedTileLoadU32 { .. })
                    )
                })
                .unwrap()
                .coordinate;
            let floor = budget.storage();
            let expanded =
                view.prepare_tile_expansion_v159(0, ExecutionTileLayoutV1::Blocked, budget)?;
            let error = expanded
                .aggregate_leaf_v162(
                    Definition::Result {
                        operation: original,
                        result: 0,
                    },
                    &path,
                    budget,
                )
                .unwrap_err();
            assert!(matches!(
                error,
                ProductionSourceOwnedViewErrorV18::Binding("source tile leaf field path differs")
            ));
            assert!(expanded.discard(budget).is_err());
            assert_eq!(budget.storage(), floor);
            Err::<(), _>(error)
        });
        assert!(result.is_err());
        assert_eq!(budget.storage(), MODULE_FLOOR);
    }
}

#[test]
fn explicit_source_tile_expansion_consumer_queries_keep_original_account_custody() {
    use fe2o3_kernel_ir::CanonicalKirDefinitionCoordinateV1 as Definition;
    for query in 0..4 {
        for foreign in [false, true] {
            let mut work = CanonicalKernelIrWorkBudgetV1::new(OPTIMIZED_SOURCE_WORK_LIMIT_V18);
            let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
            budget.reserve_storage(MODULE_FLOOR).unwrap();
            let prepared = prepared_tile_schedule_v155(&mut budget);
            let result = with_actual_optimized_source_v18(prepared, &mut budget, |view, budget| {
                let original = view
                    .input_inventory(budget)?
                    .operations()
                    .iter()
                    .find(|row| {
                        matches!(
                            row.operation.kind,
                            OperationKind::Execution(
                                ExecutionOperationV15::MaskedTileLoadU32 { .. }
                            )
                        )
                    })
                    .unwrap()
                    .coordinate;
                let expanded =
                    view.prepare_tile_expansion_v159(0, ExecutionTileLayoutV1::Blocked, budget)?;
                let check = |account: &mut ArgumentBudgetV1<'_>| match query {
                    0 => expanded.neutral_source_v162(account).map(|_| ()),
                    1 => expanded.original_source_v162(account).map(|_| ()),
                    2 => expanded.root_policy_v162(0, account).map(|_| ()),
                    _ => expanded
                        .aggregate_leaf_v162(
                            Definition::Result {
                                operation: original,
                                result: 0,
                            },
                            &[0, 0],
                            account,
                        )
                        .map(|_| ()),
                };
                let error = if foreign {
                    let mut other_work =
                        CanonicalKernelIrWorkBudgetV1::new(OPTIMIZED_SOURCE_WORK_LIMIT_V18);
                    let mut other = ArgumentBudgetV1::new(&mut other_work, MODULE_LIMIT);
                    other.reserve_storage(budget.storage())?;
                    check(&mut other).unwrap_err()
                } else {
                    budget.release_storage(1)?;
                    check(budget).unwrap_err()
                };
                assert!(matches!(
                    error,
                    ProductionSourceOwnedViewErrorV18::Resource(ArgumentResourceV1::Accounting)
                ));
                assert!(expanded.output(budget).is_err());
                assert!(expanded.discard(budget).is_err());
                Err::<(), _>(error)
            });
            assert!(result.is_err());
        }
    }
}

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
            let mut unreachable = 0;
            let mut retained = 0;
            let original_count = view.input_inventory(budget)?.operations().len();
            for row in view.input_inventory(budget)?.operations() {
                match view.operation(row.coordinate, budget)? {
                    ProductionOptimizedSourceOperationV18::Retained { .. } => retained += 1,
                    ProductionOptimizedSourceOperationV18::Rewritten { .. } => {
                        prefix_rewrites += 1;
                        continue;
                    }
                    ProductionOptimizedSourceOperationV18::RemovedUnreachable { .. } => {
                        unreachable += 1;
                        continue;
                    }
                }
                let span = expanded.operation_span(row.coordinate, budget)?.unwrap();
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
            assert_eq!(retained + prefix_rewrites + unreachable, original_count);
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
                    expanded.root_policy_v162(0, budget)?;
                    let gap_floor = budget.storage();
                    let gap = expanded
                        .source_block_entry_gap_v177(
                            0,
                            0,
                            SemanticBlockIdV1::from_index(0),
                            budget,
                        )?
                        .unwrap();
                    gap.prefix_disposition(budget)?;
                    for candidate in 0..gap.candidate_count(budget)? {
                        gap.candidate(candidate, budget)?;
                    }
                    drop(gap);
                    budget.release_storage(budget.storage() - gap_floor)?;
                    for row in view.input_inventory(budget)?.operations() {
                        if matches!(
                            row.operation.kind,
                            OperationKind::Execution(
                                ExecutionOperationV15::MaskedTileLoadU32 { .. }
                            )
                        ) {
                            expanded.aggregate_leaf_v162(
                                fe2o3_kernel_ir::CanonicalKirDefinitionCoordinateV1::Result {
                                    operation: row.coordinate,
                                    result: 0,
                                },
                                &[0, 0],
                                budget,
                            )?;
                        }
                    }
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

#[test]
fn explicit_source_tile_gap_candidates_never_enter_scalar_expansions() {
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
            let mut loads = Vec::new();
            for row in view.input_inventory(budget)?.operations() {
                if matches!(
                    row.operation.kind,
                    OperationKind::Execution(ExecutionOperationV15::MaskedTileLoadU32 { .. })
                ) {
                    let span = expanded
                        .operation_span(row.coordinate, budget)?
                        .unwrap()
                        .expansion;
                    assert!(span.end > span.first + 1);
                    loads.push(span);
                }
            }
            assert!(loads.len() >= 2);
            let mut candidates = 0usize;
            for block in view.input_inventory(budget)?.blocks() {
                for operation in 0..=block.operations.len() {
                    let gap_floor = budget.storage();
                    let gap =
                        expanded.original_gap_v177(block.coordinate, operation as u32, budget)?;
                    let prefix = gap.prefix_disposition(budget)?;
                    let expected = match prefix {
                        ProductionOptimizedSourceGapV18::Reachable(interval)
                        | ProductionOptimizedSourceGapV18::Unreachable {
                            placement: Some(interval),
                        } => (interval.last - interval.first) as usize + 1,
                        ProductionOptimizedSourceGapV18::Unreachable { placement: None } => 0,
                    };
                    assert_eq!(gap.candidate_count(budget)?, expected);
                    let mut previous = None;
                    for index in 0..expected {
                        let point = gap.candidate(index, budget)?;
                        assert_eq!(point.first, point.last);
                        if let Some((block, operation)) = previous {
                            assert_eq!(point.block, block);
                            assert!(point.first >= operation);
                        }
                        previous = Some((point.block, point.first));
                        for span in &loads {
                            assert!(
                                point.block != span.input.block
                                    || point.first <= span.first
                                    || point.first >= span.end
                            );
                        }
                        candidates += 1;
                    }
                    drop(gap);
                    budget.release_storage(budget.storage() - gap_floor)?;
                }
            }
            assert!(candidates > 0);
            let source = view.original_source(budget)?;
            let semantic = source.source_semantic(budget)?;
            let mut merged_entries = 0;
            for root in 0..source.root_count(budget)? {
                for instance in 0..source.instance_count(root, budget)? {
                    let function = source.instance(root, instance, budget)?.0;
                    for block in 0..semantic.functions()[function.index() as usize]
                        .blocks()
                        .len()
                    {
                        let gap_floor = budget.storage();
                        let gap = expanded.source_block_entry_gap_v177(
                            root,
                            instance,
                            SemanticBlockIdV1::from_index(block as u32),
                            budget,
                        )?;
                        if let Some(gap) = &gap
                            && let ProductionOptimizedSourceGapV18::Reachable(interval) =
                                gap.prefix_disposition(budget)?
                            && interval.first > 0
                        {
                            assert!(gap.candidate_count(budget)? > 0);
                            if gap.candidate(0, budget)?.first > 0 {
                                merged_entries += 1;
                            }
                        }
                        drop(gap);
                        budget.release_storage(budget.storage() - gap_floor)?;
                    }
                }
            }
            assert!(
                merged_entries > 0,
                "fixture must include an actual source entry merged into a nonzero output gap"
            );
            expanded.discard(budget)?;
            assert_eq!(budget.storage(), floor);
            Ok(())
        })
        .unwrap();
        assert_eq!(budget.storage(), MODULE_FLOOR);
    }
}

#[test]
fn explicit_source_tile_gap_candidates_keep_owner_and_header_credit() {
    for foreign in [false, true] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(OPTIMIZED_SOURCE_WORK_LIMIT_V18);
        let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
        budget.reserve_storage(MODULE_FLOOR).unwrap();
        let prepared = prepared_tile_schedule_v155(&mut budget);
        let result = with_actual_optimized_source_v18(prepared, &mut budget, |view, budget| {
            let expanded =
                view.prepare_tile_expansion_v159(0, ExecutionTileLayoutV1::Blocked, budget)?;
            let gap = expanded
                .source_block_entry_gap_v177(0, 0, SemanticBlockIdV1::from_index(0), budget)?
                .unwrap();
            let error = if foreign {
                let mut other_work =
                    CanonicalKernelIrWorkBudgetV1::new(OPTIMIZED_SOURCE_WORK_LIMIT_V18);
                let mut other = ArgumentBudgetV1::new(&mut other_work, MODULE_LIMIT);
                other.reserve_storage(budget.storage())?;
                gap.candidate_count(&mut other).unwrap_err()
            } else {
                budget.release_storage(1)?;
                gap.candidate_count(budget).unwrap_err()
            };
            assert!(matches!(
                error,
                ProductionSourceOwnedViewErrorV18::Resource(ArgumentResourceV1::Accounting)
            ));
            assert!(expanded.output(budget).is_err());
            Err::<(), _>(error)
        });
        assert!(result.is_err());
    }
}

#[test]
fn explicit_source_tile_gap_candidates_refuse_out_of_range_indices() {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(OPTIMIZED_SOURCE_WORK_LIMIT_V18);
    let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
    budget.reserve_storage(MODULE_FLOOR).unwrap();
    let prepared = prepared_tile_schedule_v155(&mut budget);
    let result = with_actual_optimized_source_v18(prepared, &mut budget, |view, budget| {
        let expanded =
            view.prepare_tile_expansion_v159(0, ExecutionTileLayoutV1::Striped, budget)?;
        let gap = expanded
            .source_block_entry_gap_v177(0, 0, SemanticBlockIdV1::from_index(0), budget)?
            .unwrap();
        let count = gap.candidate_count(budget)?;
        let error = gap.candidate(count, budget).unwrap_err();
        assert!(matches!(
            error,
            ProductionSourceOwnedViewErrorV18::Binding(
                "tile gap candidate index outside prefix interval"
            )
        ));
        assert!(expanded.output(budget).is_err());
        Err::<(), _>(error)
    });
    assert!(result.is_err());
}

#[test]
fn explicit_source_tile_gap_candidates_coincide_across_erased_transport() {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(OPTIMIZED_SOURCE_WORK_LIMIT_V18);
    let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
    budget.reserve_storage(MODULE_FLOOR).unwrap();
    let prepared = prepared_tile_schedule_v155(&mut budget);
    with_actual_optimized_source_v18(prepared, &mut budget, |view, budget| {
        let floor = budget.storage();
        let expanded =
            view.prepare_tile_expansion_v159(0, ExecutionTileLayoutV1::Blocked, budget)?;
        let mut erased = 0;
        for row in view.input_inventory(budget)?.operations() {
            if !matches!(
                row.operation.kind,
                OperationKind::Execution(ExecutionOperationV15::TileIntoFragmentU32 { .. })
            ) {
                continue;
            }
            let span = expanded
                .operation_span(row.coordinate, budget)?
                .unwrap()
                .expansion;
            assert_eq!(
                span.first, span.end,
                "transport has no extra read or scalar operation"
            );
            let gap_floor = budget.storage();
            let before = expanded.original_gap_v177(
                row.coordinate.block,
                row.coordinate.operation,
                budget,
            )?;
            let after = expanded.original_gap_v177(
                row.coordinate.block,
                row.coordinate.operation + 1,
                budget,
            )?;
            let mut before_has_boundary = false;
            for index in 0..before.candidate_count(budget)? {
                let point = before.candidate(index, budget)?;
                before_has_boundary |= point.block == span.input.block && point.first == span.first;
            }
            let mut after_has_boundary = false;
            for index in 0..after.candidate_count(budget)? {
                let point = after.candidate(index, budget)?;
                after_has_boundary |= point.block == span.input.block && point.first == span.end;
            }
            assert!(before_has_boundary && after_has_boundary);
            drop((before, after));
            budget.release_storage(budget.storage() - gap_floor)?;
            erased += 1;
        }
        assert!(erased > 0);
        expanded.discard(budget)?;
        assert_eq!(budget.storage(), floor);
        Ok(())
    })
    .unwrap();
}

#[test]
fn explicit_source_tile_gap_candidates_keep_actual_unreachable_omission() {
    run_optimized_source_v18(optimizer_dead_helper_source_owner_v18, |view, budget| {
        let floor = budget.storage();
        let expanded =
            view.prepare_tile_expansion_v159(0, ExecutionTileLayoutV1::Striped, budget)?;
        let gap_floor = budget.storage();
        let gap = expanded
            .source_block_entry_gap_v177(0, 0, SemanticBlockIdV1::from_index(1), budget)?
            .unwrap();
        assert_eq!(
            gap.prefix_disposition(budget)?,
            ProductionOptimizedSourceGapV18::Unreachable { placement: None }
        );
        assert_eq!(gap.candidate_count(budget)?, 0);
        drop(gap);
        budget.release_storage(budget.storage() - gap_floor)?;
        expanded.discard(budget)?;
        assert_eq!(budget.storage(), floor);
        Ok(())
    });
}
