#[test]
fn explicit_source_tile_graph_builds_and_replays_whole_actual_owner_with_both_layouts() {
    for layout in [
        ExecutionTileLayoutV1::Blocked,
        ExecutionTileLayoutV1::Striped,
    ] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(OPTIMIZED_SOURCE_WORK_LIMIT_V18);
        let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
        budget.reserve_storage(MODULE_FLOOR).unwrap();
        let prepared = prepared_tile_schedule_v155(&mut budget);
        with_actual_optimized_source_v18(prepared, &mut budget, |view, budget| {
            let original = view.output_inventory(budget)?;
            let loads = original
                .operations()
                .iter()
                .filter(|row| {
                    matches!(
                        row.operation.kind,
                        OperationKind::Execution(ExecutionOperationV15::MaskedTileLoadU32 { .. })
                    )
                })
                .count();
            assert!(loads >= 2);
            let floor = budget.storage();
            let candidate = view.prepare_tile_scalar_candidate_v157(
                0,
                layout,
                ProductionSemanticKirLimitsV1::default().storage_layout_limits(),
                budget,
            )?;
            assert_eq!(budget.storage(), floor);
            let retained = candidate.retained_storage();
            budget.reserve_storage(retained)?;
            candidate.replay_against(original.owner(), budget).unwrap();
            assert!(!candidate.grants_authority());
            let mut reads = 0;
            for function in &candidate.output().module().functions {
                if let Some(body) = &function.body {
                    for block in &body.blocks {
                        for operation in &block.operations {
                            assert!(!matches!(
                                operation.kind,
                                OperationKind::Execution(
                                    ExecutionOperationV15::MaskedTileLoadU32 { .. }
                                        | ExecutionOperationV15::TileIntoFragmentU32 { .. }
                                        | ExecutionOperationV15::FragmentIntoPartsU32 { .. }
                                )
                            ));
                            reads += usize::from(matches!(
                                operation.kind,
                                OperationKind::GuardedLoad { .. }
                            ));
                        }
                    }
                }
            }
            assert!(reads >= loads);
            drop(candidate);
            budget.release_storage(retained)?;
            assert_eq!(budget.storage(), floor);
            Ok(())
        })
        .unwrap();
        assert_eq!(budget.storage(), MODULE_FLOOR);
    }
}
