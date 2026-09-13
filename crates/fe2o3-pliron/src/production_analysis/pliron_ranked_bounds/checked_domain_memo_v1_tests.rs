fn memo_key(block: usize) -> CheckedDomainStateV1 {
    CheckedDomainStateV1 {
        block,
        environment: [IndexExpr::Constant(0); 6],
        goal: CheckedDomainGoalV1::Defined(IndexExpr::Constant(0)),
    }
}

fn with_checked_domain_memo(query: impl FnOnce(&mut CheckedDomainProofV1<'_, '_>)) {
    let context = Context::new();
    let graph = BoundsEdgeTransportV1 {
        context: &context,
        blocks: &[],
        predecessors: &[],
    };
    let dag = CheckedDomainDagV1::build(ROW, &mut RankedBoundsBudget::default())
        .unwrap()
        .unwrap();
    let mut proof =
        CheckedDomainProofV1::new(&graph, dag, &mut RankedBoundsBudget::default()).unwrap();
    query(&mut proof);
}

fn memo_identity_values(graph: &BoundsEdgeTransportV1<'_>) -> [Value; 3] {
    let entry = graph.blocks[0].deref(graph.context);
    let arguments = [entry.get_argument(0), entry.get_argument(1)];
    for block in graph.blocks {
        for operation in block.deref(graph.context).iter(graph.context) {
            if let Some(access) = Operation::get_op::<RankedAccessOp>(operation, graph.context) {
                return [arguments[0], arguments[1], access.view(graph.context)];
            }
        }
    }
    panic!("fixture must retain its following effect")
}

#[test]
fn checked_domain_memo_prefix_matches_complete_key_equality() {
    let source = domain_source(ROW, FixtureOptions::default());
    with_domain_graph(&source, |graph, _| {
        with_domain_graph(&source, |foreign, _| {
            let [first, second, view] = memo_identity_values(graph);
            let [foreign_first, _, foreign_view] = memo_identity_values(foreign);
            assert_ne!(first, second);
            assert_ne!(first, foreign_first);
            assert_ne!(view, foreign_view);
            // These are opaque equality keys, not independently admitted proofs.
            let expressions = [
                IndexExpr::Constant(0),
                IndexExpr::Constant(u64::MAX),
                IndexExpr::Value(first),
                IndexExpr::Value(second),
                IndexExpr::Value(foreign_first),
                IndexExpr::Dimension { view, dimension: 0 },
                IndexExpr::Dimension { view, dimension: 1 },
                IndexExpr::Dimension {
                    view: foreign_view,
                    dimension: 0,
                },
            ];
            let mut states = vec![memo_key(0), memo_key(1)];
            for expression in expressions {
                for slot in 0..6 {
                    let mut state = memo_key(0);
                    state.environment[slot] = expression;
                    states.push(state);
                }
                states.push(CheckedDomainStateV1 {
                    goal: CheckedDomainGoalV1::Defined(expression),
                    ..memo_key(0)
                });
                for expected in [0, 1] {
                    states.push(CheckedDomainStateV1 {
                        goal: CheckedDomainGoalV1::Equal {
                            actual: expression,
                            expected,
                        },
                        ..memo_key(0)
                    });
                }
            }
            for changed in 0..7 {
                let mut residual = CheckedDomainResidualV1 {
                    formula: 0,
                    positive: [0; 2],
                    negative: [0; 2],
                    clauses: 0,
                };
                match changed {
                    0 => {}
                    1 => residual.formula = 1,
                    2 | 3 => residual.positive[changed - 2] = 1,
                    4 | 5 => residual.negative[changed - 4] = 1,
                    6 => residual.clauses = 1,
                    _ => unreachable!(),
                }
                states.push(CheckedDomainStateV1 {
                    goal: CheckedDomainGoalV1::Predicate(residual),
                    ..memo_key(0)
                });
            }
            with_checked_domain_memo(|proof| {
                for lhs in &states {
                    proof.memo[0] = Some(CheckedDomainMemoV1 {
                        state: *lhs,
                        result: true,
                    });
                    for rhs in &states {
                        let mut budget = RankedBoundsBudget::default();
                        assert_eq!(
                            proof.memoized(*rhs, &mut budget),
                            Ok((lhs == rhs).then_some(true)),
                            "{lhs:?} versus {rhs:?}"
                        );
                        assert!(budget.work_units <= 98);
                        assert_eq!(budget.storage_items, 0);
                    }
                }
            });
        });
    });
}

#[test]
fn checked_domain_memo_prefix_has_exact_and_one_under_admission() {
    with_checked_domain_memo(|proof| {
        let original = memo_key(0);
        for result in [false, true] {
            proof.memo[0] = Some(CheckedDomainMemoV1 {
                state: original,
                result,
            });
            let snapshot = proof
                .memo
                .map(|entry| entry.map(|entry| (entry.state, entry.result)));
            let mut cases = vec![(memo_key(1), None, 34, 2)];
            cases.push((
                CheckedDomainStateV1 {
                    goal: CheckedDomainGoalV1::Defined(IndexExpr::Constant(1)),
                    ..original
                },
                None,
                50,
                16,
            ));
            for (slot, exact) in [(0, 58), (1, 66), (2, 74), (3, 82), (4, 90), (5, 98)] {
                let mut query = original;
                query.environment[slot] = IndexExpr::Constant(1);
                cases.push((query, None, exact, 8));
            }
            cases.push((original, Some(result), 98, 8));
            for (query, expected, exact, last_charge) in cases {
                for available in [exact, exact - 1] {
                    let mut budget = RankedBoundsBudget {
                        work_units: MAX_RANKED_BOUNDS_WORK_UNITS - available,
                        storage_items: MAX_RANKED_BOUNDS_STORAGE_ITEMS,
                        ..RankedBoundsBudget::default()
                    };
                    let actual = proof.memoized(query, &mut budget);
                    if available == exact {
                        assert_eq!(actual, Ok(expected));
                        assert_eq!(budget.work_units, MAX_RANKED_BOUNDS_WORK_UNITS);
                    } else {
                        assert!(matches!(actual,
                            Err(RankedBoundsFindingV1::ResourceLimitExceeded {
                                resource: "analysis work unit", actual, ..
                            }) if actual == MAX_RANKED_BOUNDS_WORK_UNITS + 1
                        ));
                        // The denied field is never charged or compared.
                        assert_eq!(
                            budget.work_units,
                            MAX_RANKED_BOUNDS_WORK_UNITS - (last_charge - 1)
                        );
                    }
                    assert_eq!(budget.storage_items, MAX_RANKED_BOUNDS_STORAGE_ITEMS);
                    assert_eq!(proof.next_memo, 0);
                    assert_eq!(
                        proof
                            .memo
                            .map(|entry| entry.map(|entry| (entry.state, entry.result))),
                        snapshot
                    );
                    assert!(proof.worklists.iter().all(Vec::is_empty));
                }
            }
        }
        // Denying the fixed Option-row scan does not inspect the occupied key.
        let mut budget = RankedBoundsBudget {
            work_units: MAX_RANKED_BOUNDS_WORK_UNITS - 31,
            ..RankedBoundsBudget::default()
        };
        assert!(matches!(proof.memoized(original, &mut budget),
            Err(RankedBoundsFindingV1::ResourceLimitExceeded {
                resource: "analysis work unit", actual, ..
            }) if actual == MAX_RANKED_BOUNDS_WORK_UNITS + 1
        ));
        assert_eq!(budget.work_units, MAX_RANKED_BOUNDS_WORK_UNITS - 31);
    });
}

#[test]
fn checked_domain_memo_prefix_preserves_full_table_and_ring_order() {
    with_checked_domain_memo(|proof| {
        let mut publication = RankedBoundsBudget::default();
        for row in 0..32 {
            let mut state = memo_key(0);
            state.environment[5] = IndexExpr::Constant(row);
            proof
                .remember(state, row % 2 == 0, &mut publication)
                .unwrap();
        }
        assert_eq!(
            (publication.work_units, publication.storage_items),
            (2_144, 0)
        );
        assert_eq!(proof.next_memo, 0);
        let mut last = memo_key(0);
        last.environment[5] = IndexExpr::Constant(31);
        let mut absent = last;
        absent.environment[5] = IndexExpr::Constant(32);
        let wrong_goal = CheckedDomainStateV1 {
            goal: CheckedDomainGoalV1::Defined(IndexExpr::Constant(1)),
            ..last
        };
        let mut wrong_first = last;
        wrong_first.environment[0] = IndexExpr::Constant(1);
        // 32-row scan + 32*(2, 18, 26, or 66); last-slot hit and miss
        // keep the original 2144 worst case. The first hit costs32+66.
        for (query, expected, exact, last_charge) in [
            (memo_key(1), None, 96, 2),
            (wrong_goal, None, 608, 16),
            (wrong_first, None, 864, 8),
            (absent, None, 2_144, 8),
            (last, Some(false), 2_144, 8),
            (memo_key(0), Some(true), 98, 8),
        ] {
            for available in [exact, exact - 1] {
                let mut budget = RankedBoundsBudget {
                    work_units: MAX_RANKED_BOUNDS_WORK_UNITS - available,
                    storage_items: MAX_RANKED_BOUNDS_STORAGE_ITEMS,
                    ..RankedBoundsBudget::default()
                };
                let result = proof.memoized(query, &mut budget);
                if available == exact {
                    assert_eq!(result, Ok(expected));
                    assert_eq!(budget.work_units, MAX_RANKED_BOUNDS_WORK_UNITS);
                } else {
                    assert!(matches!(result,
                        Err(RankedBoundsFindingV1::ResourceLimitExceeded {
                            resource: "analysis work unit", actual, ..
                        }) if actual == MAX_RANKED_BOUNDS_WORK_UNITS + 1
                    ));
                    assert_eq!(
                        budget.work_units,
                        MAX_RANKED_BOUNDS_WORK_UNITS - (last_charge - 1)
                    );
                }
                assert_eq!(budget.storage_items, MAX_RANKED_BOUNDS_STORAGE_ITEMS);
                assert_eq!(proof.next_memo, 0);
            }
        }
        let mut budget = RankedBoundsBudget::default();
        proof.remember(absent, false, &mut budget).unwrap();
        assert_eq!((budget.work_units, budget.storage_items), (67, 0));
        assert_eq!(proof.next_memo, 1);
        assert_eq!(proof.memoized(memo_key(0), &mut budget), Ok(None));
        assert_eq!(proof.memoized(absent, &mut budget), Ok(Some(false)));
        assert_eq!(proof.memoized(last, &mut budget), Ok(Some(false)));
        assert!(proof.worklists.iter().all(Vec::is_empty));
    });
}
