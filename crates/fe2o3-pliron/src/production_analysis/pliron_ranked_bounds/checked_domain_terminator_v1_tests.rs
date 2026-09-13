const TERMINATOR_CACHE_SOURCE: &str = r#"builtin.func @terminator_cache: builtin.function <(kernel.index, kernel.index) -> ()>
{
  ^entry(a: kernel.index, b: kernel.index):
    kernel.index_lt_br_args (a, b, a, a, a, a, a, a, b, b, b, b, b, b) [^target, ^target] []: <(kernel.index, kernel.index, kernel.index, kernel.index, kernel.index, kernel.index, kernel.index, kernel.index, kernel.index, kernel.index, kernel.index, kernel.index, kernel.index, kernel.index) -> ()>
  ^target(v: kernel.index, c: kernel.index, r: kernel.index, n: kernel.index, s: kernel.index, x: kernel.index):
    kernel.return () [] []: <() -> ()>
}
"#;

#[test]
fn checked_domain_terminator_cache_prepays_literal_cold_hit_and_denied_prefixes() {
    with_domain_graph(
        &domain_source(ROW, FixtureOptions::default()),
        |graph, _| {
            let dag = CheckedDomainDagV1::build(ROW, &mut RankedBoundsBudget::default())
                .unwrap()
                .unwrap();
            let mut proof =
                CheckedDomainProofV1::new(graph, dag, &mut RankedBoundsBudget::default()).unwrap();
            let terminator = graph.blocks[0]
                .deref(graph.context)
                .get_terminator(graph.context)
                .unwrap();
            let mut uncached = RankedBoundsBudget::default();
            assert_eq!(
                proof.authenticate_terminator_uncached(terminator, &mut uncached),
                Ok(())
            );
            // Old prelude8 + operand count3 + argument(12+7+8+8)
            // + constant(12+2+8+8) =76. Cold adds lookup12+publication5.
            assert_eq!((uncached.work_units, uncached.storage_items), (76, 0));
            assert!(proof.authenticated_terminators.iter().all(Option::is_none));
            for (available, last) in [
                (11, 12),
                (19, 8),
                (22, 3),
                (34, 12),
                (41, 7),
                (49, 8),
                (57, 8),
                (69, 12),
                (71, 2),
                (79, 8),
                (87, 8),
                (92, 5),
                (93, 0),
            ] {
                proof.authenticated_terminators.fill(None);
                let mut budget = RankedBoundsBudget {
                    work_units: MAX_RANKED_BOUNDS_WORK_UNITS - available,
                    storage_items: MAX_RANKED_BOUNDS_STORAGE_ITEMS,
                    ..RankedBoundsBudget::default()
                };
                let result = proof.authenticate_terminator(0, &mut budget);
                if available == 93 {
                    assert_eq!(result, Ok(()));
                    assert_eq!(budget.work_units, MAX_RANKED_BOUNDS_WORK_UNITS);
                    assert_eq!(proof.authenticated_terminators[0], Some(terminator));
                } else {
                    assert!(
                        matches!(result, Err(RankedBoundsFindingV1::ResourceLimitExceeded {
                    resource: "analysis work unit", actual, ..
                }) if actual == MAX_RANKED_BOUNDS_WORK_UNITS + 1)
                    );
                    assert_eq!(budget.work_units, MAX_RANKED_BOUNDS_WORK_UNITS - (last - 1));
                    assert!(proof.authenticated_terminators.iter().all(Option::is_none));
                }
                assert_eq!(budget.storage_items, MAX_RANKED_BOUNDS_STORAGE_ITEMS);
            }
            for available in [11, 12] {
                let mut budget = RankedBoundsBudget {
                    work_units: MAX_RANKED_BOUNDS_WORK_UNITS - available,
                    ..RankedBoundsBudget::default()
                };
                let result = proof.authenticate_terminator(0, &mut budget);
                if available == 12 {
                    assert_eq!(result, Ok(()));
                    assert_eq!(budget.work_units, MAX_RANKED_BOUNDS_WORK_UNITS);
                } else {
                    assert!(
                        matches!(result, Err(RankedBoundsFindingV1::ResourceLimitExceeded {
                    resource: "analysis work unit", actual, ..
                }) if actual == MAX_RANKED_BOUNDS_WORK_UNITS + 1)
                    );
                    assert_eq!(budget.work_units, MAX_RANKED_BOUNDS_WORK_UNITS - 11);
                }
                assert_eq!(proof.authenticated_terminators[0], Some(terminator));
                assert_eq!(budget.storage_items, 0);
            }
            assert!(proof.memo.iter().all(Option::is_none));
            assert!(proof.fold_cache.is_none());
            assert!(proof.worklists.iter().all(Vec::is_empty));
        },
    );
}

#[test]
fn checked_domain_terminator_cache_owner_allocation_has_exact_bounds() {
    with_domain_graph(TERMINATOR_CACHE_SOURCE, |graph, _| {
        assert_eq!(graph.blocks.len(), 2);
        assert_eq!(
            checked_domain_resource_bound_v1(ProductionAnalysisInputCensusV1 {
                ranked_accesses: 1,
                operands: 1,
                blocks: 2,
                ..ProductionAnalysisInputCensusV1::default()
            }),
            Ok((8_751_128, 131_072))
        );
        // Row owner: query2611 + frame8*768 + 2 rows*3 =8761.
        // Depth32 + bounds4 + init/drop17522 + reserve1 =17559.
        for (work, storage) in [(17_559, 8_761), (17_558, 8_761), (17_559, 8_760)] {
            let dag = CheckedDomainDagV1::build(ROW, &mut RankedBoundsBudget::default())
                .unwrap()
                .unwrap();
            let mut budget = RankedBoundsBudget {
                work_units: MAX_RANKED_BOUNDS_WORK_UNITS - work,
                storage_items: MAX_RANKED_BOUNDS_STORAGE_ITEMS - storage,
                ..RankedBoundsBudget::default()
            };
            let result = CheckedDomainProofV1::new(graph, dag, &mut budget);
            if work == 17_559 && storage == 8_761 {
                let proof = result.unwrap();
                assert_eq!(proof.authenticated_terminators.len(), 2);
                assert!(proof.authenticated_terminators.iter().all(Option::is_none));
                assert_eq!(
                    (budget.work_units, budget.storage_items),
                    (
                        MAX_RANKED_BOUNDS_WORK_UNITS,
                        MAX_RANKED_BOUNDS_STORAGE_ITEMS
                    )
                );
            } else if work == 17_558 {
                assert!(
                    matches!(result, Err(RankedBoundsFindingV1::ResourceLimitExceeded {
                    resource: "analysis work unit", actual, ..
                }) if actual == MAX_RANKED_BOUNDS_WORK_UNITS + 1)
                );
                assert_eq!(budget.work_units, MAX_RANKED_BOUNDS_WORK_UNITS - 17_522);
                assert_eq!(budget.storage_items, MAX_RANKED_BOUNDS_STORAGE_ITEMS);
            } else {
                assert!(
                    matches!(result, Err(RankedBoundsFindingV1::ResourceLimitExceeded {
                    resource: "analysis storage item", actual, ..
                }) if actual == MAX_RANKED_BOUNDS_STORAGE_ITEMS + 1)
                );
                assert_eq!(budget.work_units, MAX_RANKED_BOUNDS_WORK_UNITS - 17_523);
                assert_eq!(
                    budget.storage_items,
                    MAX_RANKED_BOUNDS_STORAGE_ITEMS - 8_760
                );
            }
        }
        let blocks = vec![graph.blocks[0]; MAX_RANKED_BOUNDS_BLOCKS + 1];
        let oversized = BoundsEdgeTransportV1 {
            context: graph.context,
            blocks: &blocks,
            predecessors: &[],
        };
        let dag = CheckedDomainDagV1::build(ROW, &mut RankedBoundsBudget::default())
            .unwrap()
            .unwrap();
        let mut budget = RankedBoundsBudget::default();
        assert!(matches!(
            CheckedDomainProofV1::new(&oversized, dag, &mut budget),
            Err(RankedBoundsFindingV1::StructuralVerificationFailed)
        ));
        assert_eq!((budget.work_units, budget.storage_items), (36, 0));
    });
}

#[test]
fn checked_domain_terminator_cache_does_not_cache_parallel_edges_or_successors() {
    with_domain_graph(TERMINATOR_CACHE_SOURCE, |graph, _| {
        let dag = CheckedDomainDagV1::build(ROW, &mut RankedBoundsBudget::default())
            .unwrap()
            .unwrap();
        let mut proof =
            CheckedDomainProofV1::new(graph, dag, &mut RankedBoundsBudget::default()).unwrap();
        let environment = core::array::from_fn(|index| {
            IndexExpr::Value(graph.blocks[1].deref(graph.context).get_argument(index))
        });
        let state = CheckedDomainStateV1 {
            block: 1,
            environment,
            goal: CheckedDomainGoalV1::Equal {
                actual: environment[5],
                expected: 5,
            },
        };
        let first = proof
            .pull_environment(
                state,
                &graph.predecessors[1][0],
                &mut RankedBoundsBudget::default(),
            )
            .unwrap();
        let second = proof
            .pull_environment(
                state,
                &graph.predecessors[1][1],
                &mut RankedBoundsBudget::default(),
            )
            .unwrap();
        let a = IndexExpr::Value(graph.blocks[0].deref(graph.context).get_argument(0));
        let b = IndexExpr::Value(graph.blocks[0].deref(graph.context).get_argument(1));
        assert_ne!(a, b);
        assert_eq!(first, ([a; 6], Some(a)));
        assert_eq!(second, ([b; 6], Some(b)));
        let terminator = graph.blocks[0]
            .deref(graph.context)
            .get_terminator(graph.context)
            .unwrap();
        assert_eq!(proof.authenticated_terminators[0], Some(terminator));
        assert!(proof.authenticated_terminators[1].is_none());
        assert!(proof.memo.iter().all(Option::is_none));
        let wrong = PredecessorEdge {
            block: 0,
            successor: 2,
            guard_fact: None,
        };
        assert_eq!(
            proof.pull_environment(state, &wrong, &mut RankedBoundsBudget::default()),
            Err(RankedBoundsFindingV1::StructuralVerificationFailed)
        );
        // Deliberate hostile mutation is outside the immutable-owner promise.
        // Even then, a completed metadata row cannot bypass the exact successor.
        Operation::replace_successor(terminator, graph.context, 0, graph.blocks[0]);
        assert_eq!(
            proof.pull_environment(
                state,
                &graph.predecessors[1][0],
                &mut RankedBoundsBudget::default()
            ),
            Err(RankedBoundsFindingV1::StructuralVerificationFailed)
        );
        assert_eq!(proof.authenticated_terminators[0], Some(terminator));
    });
}

#[test]
fn checked_domain_terminator_cache_rejects_foreign_rows_and_fresh_foreign_operands() {
    let mut context = Context::new();
    fe2o3_pliron_owner_core::ensure_context_identity(&mut context).unwrap();
    register_dialect(&mut context, &DialectName::try_new(DIALECT_NAME).unwrap()).unwrap();
    let first = parse_from_str(
        Operation::top_level_parser(),
        &mut context,
        TERMINATOR_CACHE_SOURCE,
    )
    .unwrap();
    let second = parse_from_str(
        Operation::top_level_parser(),
        &mut context,
        &TERMINATOR_CACHE_SOURCE.replace("@terminator_cache", "@foreign_cache"),
    )
    .unwrap();
    verify_operation(first, &context).unwrap();
    verify_operation(second, &context).unwrap();
    let first = FuncOp::from_operation(first);
    let second = FuncOp::from_operation(second);
    let blocks = first
        .get_region(&context)
        .deref(&context)
        .iter(&context)
        .collect::<Vec<_>>();
    let foreign_block = second
        .get_region(&context)
        .deref(&context)
        .iter(&context)
        .next()
        .unwrap();
    let terminator = blocks[0].deref(&context).get_terminator(&context).unwrap();
    let foreign_terminator = foreign_block
        .deref(&context)
        .get_terminator(&context)
        .unwrap();
    let graph = BoundsEdgeTransportV1 {
        context: &context,
        blocks: &blocks,
        predecessors: &[],
    };
    let dag = || {
        CheckedDomainDagV1::build(ROW, &mut RankedBoundsBudget::default())
            .unwrap()
            .unwrap()
    };
    let mut proof =
        CheckedDomainProofV1::new(&graph, dag(), &mut RankedBoundsBudget::default()).unwrap();
    proof.authenticated_terminators[0] = Some(foreign_terminator);
    let mut budget = RankedBoundsBudget::default();
    assert_eq!(
        proof.authenticate_terminator(0, &mut budget),
        Err(RankedBoundsFindingV1::StructuralVerificationFailed)
    );
    assert_eq!((budget.work_units, budget.storage_items), (12, 0));
    assert_eq!(proof.authenticated_terminators[0], Some(foreign_terminator));
    proof.authenticated_terminators.fill(None);
    proof
        .authenticate_terminator(0, &mut RankedBoundsBudget::default())
        .unwrap();
    let foreign_value = foreign_block.deref(&context).get_argument(0);
    Operation::replace_operand(terminator, &context, 0, foreign_value);
    assert!(matches!(
        crate::derive_pliron_ir_structural_identity_v1(&context, &first),
        Err(crate::PlironIrIdentityErrorV1::ExternalOperand { .. })
    ));
    // Revalidation belongs to a new immutable proof owner after mutation.
    let mut fresh =
        CheckedDomainProofV1::new(&graph, dag(), &mut RankedBoundsBudget::default()).unwrap();
    assert!(fresh.authenticated_terminators.iter().all(Option::is_none));
    assert_eq!(
        fresh.authenticate_terminator(0, &mut RankedBoundsBudget::default()),
        Err(RankedBoundsFindingV1::StructuralVerificationFailed)
    );
    assert!(fresh.authenticated_terminators.iter().all(Option::is_none));
    assert!(fresh.memo.iter().all(Option::is_none));
    with_domain_graph(TERMINATOR_CACHE_SOURCE, |local, _| {
        with_domain_graph(TERMINATOR_CACHE_SOURCE, |foreign, _| {
            let local_term = local.blocks[0]
                .deref(local.context)
                .get_terminator(local.context)
                .unwrap();
            let foreign_term = foreign.blocks[0]
                .deref(foreign.context)
                .get_terminator(foreign.context)
                .unwrap();
            assert_ne!(local_term, foreign_term);
            let mut local_proof =
                CheckedDomainProofV1::new(local, dag(), &mut RankedBoundsBudget::default())
                    .unwrap();
            local_proof.authenticated_terminators[0] = Some(foreign_term);
            assert_eq!(
                local_proof.authenticate_terminator(0, &mut RankedBoundsBudget::default()),
                Err(RankedBoundsFindingV1::StructuralVerificationFailed)
            );
            let mut foreign_proof =
                CheckedDomainProofV1::new(foreign, dag(), &mut RankedBoundsBudget::default())
                    .unwrap();
            let mut budget = RankedBoundsBudget::default();
            assert_eq!(
                foreign_proof.authenticate_terminator(0, &mut budget),
                Ok(())
            );
            assert_eq!((budget.work_units, budget.storage_items), (474, 0));
            assert_eq!(
                foreign_proof.authenticated_terminators[0],
                Some(foreign_term)
            );
        });
    });
}
