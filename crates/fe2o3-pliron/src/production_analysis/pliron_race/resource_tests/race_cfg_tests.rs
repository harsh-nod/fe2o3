mod race_cfg_tests {
    use super::*;

    #[test]
    fn race_cfg_growth_and_exact_admission() {
        for n in [1, 16, 32] {
            let (context, function) = cfg_fixture(n);
            let census = census(&context, &function);
            assert_eq!(
                (
                    census.blocks,
                    census.operations,
                    census.successors,
                    census.max_successor_arity
                ),
                (3 * n, 4 * n + 1, 4 * n - 2, 2),
            );
            let mut analyses = PlironAnalysisManagerV1::new(&function);
            analyses.prepare_sparse_indices(&context, &function);
            let inventory = analyses.function_inventory().unwrap();
            let sparse = analyses.sparse_indices().unwrap();
            let shape = static_invocation_shape_for_resource_v1(sparse, None);
            assert_eq!(shape, Some((1, 1)));
            let names = collect_race_name_census_v1(
                &context,
                &function,
                inventory,
                census,
                ProductionAnalysisResourceLimitsV1::new(
                    race_name_scan_work_v1(census).unwrap(),
                    RACE_NAME_CENSUS_SCRATCH_V1,
                ),
                |value| value.unique_name_byte_len(&context),
            )
            .unwrap();
            let bound =
                calculate_race_resource_upper_bound_for_shape_v1(census, names, shape, None)
                    .unwrap()
                    .bound;
            let work = bound.work_upper_bound();
            let peak = bound.peak_storage_upper_bound();
            if n == 32 {
                assert_eq!(
                    race_cfg_resource_bound_v1(census).unwrap(),
                    (946_256, 18_020)
                );
            }
            for (work, peak, refusal) in [
                (work, peak, None),
                (work - 1, peak, Some("work upper bound")),
                (work, peak - 1, Some("peak storage upper bound")),
            ] {
                let mut stats = RaceCfgStatsV1::default();
                let result = preflight_race_resource_upper_bound_v1(
                    &context,
                    &function,
                    Some(inventory),
                    census,
                    sparse,
                    None,
                    ProductionAnalysisResourceLimitsV1::new(work, peak),
                )
                .map(|actual| {
                    assert_eq!(actual, bound);
                    invocation_upper_bounds_by_block_impl(
                        &context,
                        &function,
                        inventory,
                        Some(&mut stats),
                    )
                });
                if let Some(resource) = refusal {
                    assert!(matches!(result, Err(error)
                        if error.phase == ProductionAnalysisResourcePhaseV1::RaceFreedom
                            && error.resource == resource));
                    assert_eq!((stats.pops, stats.edges, stats.capacity), (0, 0, 0));
                    continue;
                }
                let rows = result.unwrap().unwrap();
                let capacity = (2 * n).max(4);
                assert_eq!(
                    (
                        stats.pops,
                        stats.edges,
                        stats.peak,
                        stats.capacity,
                        stats.backing
                    ),
                    (
                        2 * n * n + 1,
                        2 * n * n,
                        2 * n,
                        capacity,
                        if n == 1 { 5 } else { capacity + capacity / 2 }
                    ),
                );
                assert!(rows[2 * n - 1..3 * n - 1].iter().all(|row|
                    row[0] == Some(n as u64) && row[1..].iter().all(Option::is_none)
                ));
            }
            cfg_fixed_pipeline(&context, &function);
        }
    }

    fn cfg_fixed_pipeline(context: &Context, function: &FuncOp) {
        let outcome = crate::production_analysis::require_production_pliron_checks_before_lowering_with_resource_limits_v1(
            context, function, ProductionAnalysisResourceLimitsV1::production_hard_ceiling(),
        ).unwrap();
        assert!(outcome.report.is_clean());
        assert_eq!(outcome.report.preservation().certificates().len(), 9);
        assert!(!outcome.report.grants_compiler_refinement_authority());
        assert!(!outcome.report.grants_artifact_or_launch_authority());
    }

    fn cfg_fixture(n: usize) -> (Context, FuncOp) {
        use pliron::basic_block::BasicBlock;

        let mut context = context();
        let ty = IndexType::get(&context).into();
        let signature = FunctionType::get(&context, vec![ty; 2], vec![]);
        let function = FuncOp::new(&mut context, "race_cfg".try_into().unwrap(), signature);
        let entry = function.get_entry_block(&context);
        let args = [
            entry.deref(&context).get_argument(0),
            entry.deref(&context).get_argument(1),
        ];
        let mut blocks = vec![entry];
        for _ in 1..3 * n {
            let block = BasicBlock::new(&mut context, None, vec![]);
            block.insert_at_back(function.get_region(&context), &context);
            blocks.push(block);
        }
        let invocation = InvocationIndexOp::new(&mut context, 0, 1);
        invocation.get_operation().insert_at_back(entry, &context);
        let index = invocation.result(&context);
        let mut constants = Vec::new();
        for k in 1..=n {
            let op = IndexConstantOp::new(&mut context, k as u64);
            op.get_operation().insert_at_back(entry, &context);
            constants.push(op.result(&context));
        }
        for k in 0..n {
            IndexLessThanBranchOp::new(
                &mut context,
                index,
                constants[k],
                blocks[n],
                blocks[if k + 1 < n { k + 1 } else { 3 * n - 1 }],
            )
            .get_operation()
            .insert_at_back(blocks[k], &context);
        }
        for k in 0..n - 1 {
            IndexLessThanBranchOp::new(
                &mut context,
                args[0],
                args[1],
                blocks[n + 2 * k + 1],
                blocks[n + 2 * k + 2],
            )
            .get_operation()
            .insert_at_back(blocks[n + k], &context);
        }
        for &block in &blocks[2 * n - 1..] {
            ReturnOp::new(&mut context)
                .get_operation()
                .insert_at_back(block, &context);
        }
        (context, function)
    }

    #[test]
    fn race_cfg_formula_empty_and_overflow() {
        assert_eq!(race_cfg_resource_bound_v1(Default::default()), Ok((0, 0)));
        for (blocks, successors, max_successor_arity) in
            [(usize::MAX, 0, 0), (2, usize::MAX, 2), (2, 2, usize::MAX)]
        {
            assert_eq!(
                race_cfg_resource_bound_v1(ProductionAnalysisInputCensusV1 {
                    blocks,
                    successors,
                    max_successor_arity,
                    ..Default::default()
                }),
                Err(race_resource_overflow_v1()),
            );
        }
    }

    use crate::production_analysis::pliron_resource_envelope::ProductionAnalysisResourceContractV1 as CfgLedger;

    type CfgEdges = (Option<(u32, u64)>, Vec<usize>);

    fn cfg_extra_fixture(spec: &[CfgEdges]) -> (Context, FuncOp) {
        let mut context = context();
        let ty = IndexType::get(&context).into();
        let signature = FunctionType::get(&context, vec![ty; 2], vec![]);
        let function = FuncOp::new(&mut context, "cfg_extra".try_into().unwrap(), signature);
        let entry = function.get_entry_block(&context);
        let args = (
            entry.deref(&context).get_argument(0),
            entry.deref(&context).get_argument(1),
        );
        let mut blocks = vec![entry];
        for _ in 1..spec.len() {
            let block = pliron::basic_block::BasicBlock::new(&mut context, None, vec![]);
            block.insert_at_back(function.get_region(&context), &context);
            blocks.push(block);
        }
        let mut conditions = Vec::new();
        for (guard, _) in spec {
            let pair = if let Some((dimension, bound)) = *guard {
                let index = InvocationIndexOp::new(&mut context, dimension, 1);
                index.get_operation().insert_at_back(entry, &context);
                let constant = IndexConstantOp::new(&mut context, bound);
                constant.get_operation().insert_at_back(entry, &context);
                (index.result(&context), constant.result(&context))
            } else {
                args
            };
            conditions.push(pair);
        }
        for (i, (_, successors)) in spec.iter().enumerate() {
            let operation = match successors.as_slice() {
                [] => ReturnOp::new(&mut context).get_operation(),
                [target] => {
                    dialect_kernel::BranchOp::new(&mut context, blocks[*target]).get_operation()
                }
                [a, b] => IndexLessThanBranchOp::new(
                    &mut context,
                    conditions[i].0,
                    conditions[i].1,
                    blocks[*a],
                    blocks[*b],
                )
                .get_operation(),
                _ => panic!("fixture supports at most two successors"),
            };
            operation.insert_at_back(blocks[i], &context);
        }
        (context, function)
    }

    fn cfg_extra_check(
        spec: &[CfgEdges],
        expected: (usize, usize, usize, usize, usize),
    ) -> (
        ProductionAnalysisInputCensusV1,
        Vec<[Option<u64>; MAX_RANKED_MEMORY_RANK]>,
    ) {
        let (context, function) = cfg_extra_fixture(spec);
        let census = census(&context, &function);
        let mut analyses = PlironAnalysisManagerV1::new(&function);
        analyses.prepare_sparse_indices(&context, &function);
        let inventory = analyses.function_inventory().unwrap();
        let sparse = analyses.sparse_indices().unwrap();
        let phase = ProductionAnalysisResourcePhaseV1::RaceFreedom;
        let bound = preflight_race_resource_upper_bound_v1(
            &context,
            &function,
            Some(inventory),
            census,
            sparse,
            None,
            unlimited(),
        )
        .unwrap();
        let floor =
            ProductionAnalysisResourceUpperBoundV1::checked_phase(phase, 11, 37, 101).unwrap();
        let full = floor.checked_then_retain(bound, phase).unwrap();
        let mut rows = None;
        for (work_short, storage_short, refusal) in [
            (0, 0, None),
            (1, 0, Some("work upper bound")),
            (0, 1, Some("peak storage upper bound")),
        ] {
            let mut ledger = CfgLedger::new(ProductionAnalysisResourceLimitsV1::new(
                full.work_upper_bound() - work_short,
                full.peak_storage_upper_bound() - storage_short,
            ));
            ledger.admit_retained(phase, floor).unwrap();
            let mut stats = RaceCfgStatsV1::default();
            let result = preflight_race_resource_upper_bound_v1(
                &context,
                &function,
                Some(inventory),
                census,
                sparse,
                None,
                ledger.remaining(phase).unwrap(),
            )
            .and_then(|actual| {
                assert_eq!(actual, bound);
                ledger.admit_retained(phase, actual)?;
                Ok(invocation_upper_bounds_by_block_impl(
                    &context,
                    &function,
                    inventory,
                    Some(&mut stats),
                )
                .unwrap())
            });
            let observed = (
                stats.pops,
                stats.edges,
                stats.peak,
                stats.capacity,
                stats.backing,
            );
            match refusal {
                Some(resource) => {
                    assert_eq!(
                        result.unwrap_err(),
                        ProductionAnalysisResourceLimitV1 { phase, resource }
                    );
                    assert_eq!(ledger.cumulative(), floor);
                    assert_eq!(observed, (0, 0, 0, 0, 0));
                }
                None => {
                    rows = Some(result.unwrap());
                    assert_eq!(ledger.cumulative(), full);
                    assert_eq!(observed, expected);
                }
            }
        }
        (census, rows.unwrap())
    }

    #[test]
    fn race_cfg_multidimensional_weakening_to_none() {
        let (_, rows) = cfg_extra_check(
            &[
                (Some((0, 2)), vec![1, 4]),
                (Some((1, 3)), vec![2, 7]),
                (None, vec![3]),
                (None, vec![]),
                (None, vec![5]),
                (None, vec![6]),
                (None, vec![3]),
                (None, vec![8]),
                (None, vec![3]),
            ],
            (11, 10, 3, 4, 5),
        );
        assert_eq!(&rows[2][..2], &[Some(2), Some(3)]);
        assert!(rows[3].iter().all(Option::is_none));
    }

    #[test]
    fn race_cfg_duplicate_edges_and_nonentry_backedge() {
        let (_, rows) = cfg_extra_check(
            &[
                (Some((0, 2)), vec![1, 4]),
                (Some((1, 3)), vec![2, 2]),
                (None, vec![1, 3]),
                (None, vec![]),
                (None, vec![2]),
            ],
            (8, 13, 4, 4, 5),
        );
        assert!(rows.iter().flatten().all(Option::is_none));
    }

    #[test]
    fn race_cfg_d0_d1_with_inherited_floor() {
        for (spec, arity, counts) in [
            (vec![(None, vec![])], 0, (1, 0, 1, 1, 1)),
            (
                vec![(None, vec![1]), (None, vec![2]), (None, vec![])],
                1,
                (3, 2, 1, 1, 1),
            ),
        ] {
            let (census, rows) = cfg_extra_check(&spec, counts);
            assert_eq!(census.max_successor_arity, arity);
            assert!(rows.iter().flatten().all(Option::is_none));
        }
    }

    #[test]
    fn race_cfg_independent_fanouts_share_terminal_leaves() {
        for n in [4usize, 8] {
            let leaf = n * n + n - 1;
            let exit = leaf + n;
            let mut spec = vec![(None, vec![]); exit + 1];
            for (i, (_, successors)) in spec.iter_mut().enumerate().take(n - 1) {
                *successors = vec![2 * i + 1, 2 * i + 2];
            }
            for i in 0..n {
                let base = 2 * n - 1 + i * (n - 1);
                spec[n - 1 + i] = (Some((0, (i + 1) as u64)), vec![base, exit]);
                for j in 0..n - 1 {
                    spec[base + j].1 = [2 * j + 1, 2 * j + 2]
                        .map(|k| {
                            if k < n - 1 {
                                base + k
                            } else {
                                leaf + k - (n - 1)
                            }
                        })
                        .to_vec();
                }
            }
            let (census, rows) = cfg_extra_check(
                &spec,
                (
                    2 * n * n + n,
                    2 * n * n + 2 * n - 2,
                    n * n,
                    n * n,
                    3 * n * n / 2,
                ),
            );
            assert_eq!(
                (
                    census.blocks,
                    census.operations,
                    census.successors,
                    census.max_successor_arity
                ),
                (n * n + 2 * n, n * n + 4 * n, 2 * n * n + 2 * n - 2, 2),
            );
            assert!(
                rows[leaf..exit]
                    .iter()
                    .all(|row| row[0] == Some(n as u64) && row[1..].iter().all(Option::is_none))
            );
            let (context, function) = cfg_extra_fixture(&spec);
            cfg_fixed_pipeline(&context, &function);
        }
    }
}
