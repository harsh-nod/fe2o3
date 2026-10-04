// The pre-cache fold evaluator, independent of the new key and cache path.
fn uncached_domain_fold_oracle(
    dag: &CheckedDomainDagV1,
    environment: CheckedDomainEnvironmentV1,
) -> [Option<u64>; CHECKED_DOMAIN_NODES_V1] {
    let mut values = [None; CHECKED_DOMAIN_NODES_V1];
    for (index, node) in dag.nodes[..dag.node_count].iter().enumerate() {
        values[index] = match node.kind {
            CheckedDomainNodeKindV1::Leaf(role) => match environment[usize::from(role)] {
                IndexExpr::Constant(value) => Some(value),
                _ => None,
            },
            CheckedDomainNodeKindV1::Constant(value) => Some(value),
            CheckedDomainNodeKindV1::Binary { kind, lhs, rhs } => values[usize::from(lhs)]
                .zip(values[usize::from(rhs)])
                .and_then(|(lhs, rhs)| match kind {
                    IndexBinaryKindAttr::Add => lhs.checked_add(rhs),
                    IndexBinaryKindAttr::Multiply => lhs.checked_mul(rhs),
                    IndexBinaryKindAttr::Divide => lhs.checked_div(rhs),
                    IndexBinaryKindAttr::Remainder => lhs.checked_rem(rhs),
                }),
        };
    }
    values
}

#[test]
fn checked_domain_fold_cache_matches_uncached_literals_and_opaque_values() {
    let source = domain_source(ROW, FixtureOptions::default());
    with_domain_graph(&source, |graph, _| {
        with_domain_graph(&source, |foreign, _| {
            let [first, second, view] = memo_identity_values(graph);
            let [foreign_first, _, foreign_view] = memo_identity_values(foreign);
            let base = [3, 1, 16, 16, 16, 256].map(IndexExpr::Constant);
            let mut environments = vec![
                base,
                [IndexExpr::Constant(0); 6],
                [IndexExpr::Constant(u64::MAX); 6],
            ];
            for expression in [
                IndexExpr::Constant(0),
                IndexExpr::Constant(1),
                IndexExpr::Constant(u64::MAX),
                IndexExpr::Value(first),
                IndexExpr::Value(second),
                IndexExpr::Value(foreign_first),
                IndexExpr::Dimension { view, dimension: 0 },
                IndexExpr::Dimension {
                    view: foreign_view,
                    dimension: 0,
                },
            ] {
                for slot in 0..6 {
                    let mut environment = base;
                    environment[slot] = expression;
                    environments.push(environment);
                }
            }
            for geometry in [ROW, TILE] {
                let dag = CheckedDomainDagV1::build(geometry, &mut RankedBoundsBudget::default())
                    .unwrap()
                    .unwrap();
                let mut proof =
                    CheckedDomainProofV1::new(graph, dag, &mut RankedBoundsBudget::default())
                        .unwrap();
                for environment in &environments {
                    let expected = uncached_domain_fold_oracle(&proof.dag, *environment);
                    assert_eq!(
                        proof.fold(*environment, &mut RankedBoundsBudget::default()),
                        Ok(expected)
                    );
                    let mut hit = RankedBoundsBudget::default();
                    let mut result = proof.fold(*environment, &mut hit).unwrap();
                    assert_eq!(result, expected);
                    assert_eq!((hit.work_units, hit.storage_items), (202, 0));
                    result[0] = Some(999);
                    assert_eq!(result[0], Some(999));
                    assert_eq!(proof.fold_cache.as_ref().unwrap().values, expected);
                    assert!(proof.memo.iter().all(Option::is_none));
                }
                // Different, even foreign, nonliteral identities produce the
                // same None key, not a claim that the SSA values are equal.
                for value in [first, second, foreign_first] {
                    let environment = [IndexExpr::Value(value); 6];
                    let expected = uncached_domain_fold_oracle(&proof.dag, environment);
                    assert_eq!(
                        proof.fold(environment, &mut RankedBoundsBudget::default()),
                        Ok(expected)
                    );
                    assert_eq!(proof.fold_cache.as_ref().unwrap().key, [None; 6]);
                    assert!(expected[..6].iter().all(Option::is_none));
                }
            }
        });
    });
}

#[test]
fn checked_domain_fold_cache_prepays_each_prefix_and_preserves_denied_owner() {
    with_checked_domain_memo(|proof| {
        let base = [3, 1, 16, 16, 16, 256].map(IndexExpr::Constant);
        let expected = uncached_domain_fold_oracle(&proof.dag, base);
        // Empty miss: key49 + presence1 + old evaluation512 + publication142.
        for (available, last_charge) in [(48, 49), (49, 1), (561, 512), (703, 142), (704, 0)] {
            proof.fold_cache = None;
            let mut budget = RankedBoundsBudget {
                work_units: MAX_RANKED_BOUNDS_WORK_UNITS - available,
                storage_items: MAX_RANKED_BOUNDS_STORAGE_ITEMS,
                ..RankedBoundsBudget::default()
            };
            let result = proof.fold(base, &mut budget);
            if available == 704 {
                assert_eq!(result, Ok(expected));
                assert_eq!(budget.work_units, MAX_RANKED_BOUNDS_WORK_UNITS);
            } else {
                assert!(
                    matches!(result, Err(RankedBoundsFindingV1::ResourceLimitExceeded {
                    resource: "analysis work unit", actual, ..
                }) if actual == MAX_RANKED_BOUNDS_WORK_UNITS + 1)
                );
                assert_eq!(
                    budget.work_units,
                    MAX_RANKED_BOUNDS_WORK_UNITS - (last_charge - 1)
                );
                assert!(proof.fold_cache.is_none());
            }
            assert_eq!(budget.storage_items, MAX_RANKED_BOUNDS_STORAGE_ITEMS);
        }
        let snapshot = proof.fold_cache;
        // Each compared Option field4, then the complete copied result128.
        for (available, last_charge) in [
            (53, 4),
            (57, 4),
            (61, 4),
            (65, 4),
            (69, 4),
            (73, 4),
            (201, 128),
            (202, 0),
        ] {
            let mut budget = RankedBoundsBudget {
                work_units: MAX_RANKED_BOUNDS_WORK_UNITS - available,
                storage_items: MAX_RANKED_BOUNDS_STORAGE_ITEMS,
                ..RankedBoundsBudget::default()
            };
            let result = proof.fold(base, &mut budget);
            if available == 202 {
                assert_eq!(result, Ok(expected));
                assert_eq!(budget.work_units, MAX_RANKED_BOUNDS_WORK_UNITS);
            } else {
                assert!(
                    matches!(result, Err(RankedBoundsFindingV1::ResourceLimitExceeded {
                    resource: "analysis work unit", actual, ..
                }) if actual == MAX_RANKED_BOUNDS_WORK_UNITS + 1)
                );
                assert_eq!(
                    budget.work_units,
                    MAX_RANKED_BOUNDS_WORK_UNITS - (last_charge - 1)
                );
            }
            assert_eq!(proof.fold_cache, snapshot);
            assert_eq!(budget.storage_items, MAX_RANKED_BOUNDS_STORAGE_ITEMS);
        }
        for (slot, exact) in [(0, 708), (1, 712), (2, 716), (3, 720), (4, 724), (5, 728)] {
            let mut changed = base;
            changed[slot] = IndexExpr::Constant(999);
            for available in [exact - 1, exact] {
                proof.fold_cache = snapshot;
                let mut budget = RankedBoundsBudget {
                    work_units: MAX_RANKED_BOUNDS_WORK_UNITS - available,
                    storage_items: MAX_RANKED_BOUNDS_STORAGE_ITEMS,
                    ..RankedBoundsBudget::default()
                };
                let result = proof.fold(changed, &mut budget);
                if available == exact {
                    assert_eq!(result, Ok(uncached_domain_fold_oracle(&proof.dag, changed)));
                    assert_eq!(budget.work_units, MAX_RANKED_BOUNDS_WORK_UNITS);
                    assert_ne!(proof.fold_cache, snapshot);
                } else {
                    assert!(
                        matches!(result, Err(RankedBoundsFindingV1::ResourceLimitExceeded {
                        resource: "analysis work unit", actual, ..
                    }) if actual == MAX_RANKED_BOUNDS_WORK_UNITS + 1)
                    );
                    assert_eq!(budget.work_units, MAX_RANKED_BOUNDS_WORK_UNITS - 141);
                    assert_eq!(proof.fold_cache, snapshot);
                }
                assert_eq!(budget.storage_items, MAX_RANKED_BOUNDS_STORAGE_ITEMS);
                assert!(proof.memo.iter().all(Option::is_none));
                assert_eq!(proof.next_memo, 0);
                assert_eq!(proof.active_queries, 0);
                assert!(proof.worklists.iter().all(Vec::is_empty));
            }
        }
    });
}

#[test]
fn checked_domain_fold_cache_is_private_to_its_immutable_dag_owner() {
    let context = Context::new();
    let graph = BoundsEdgeTransportV1 {
        context: &context,
        blocks: &[],
        predecessors: &[],
    };
    let dag = |geometry| {
        CheckedDomainDagV1::build(geometry, &mut RankedBoundsBudget::default())
            .unwrap()
            .unwrap()
    };
    let mut first =
        CheckedDomainProofV1::new(&graph, dag(ROW), &mut RankedBoundsBudget::default()).unwrap();
    let mut second = CheckedDomainProofV1::new(
        &graph,
        dag(CheckedDomainGeometryV1::RowStriped([4, 2])),
        &mut RankedBoundsBudget::default(),
    )
    .unwrap();
    let environment = [3, 1, 16, 16, 16, 256].map(IndexExpr::Constant);
    let expected_first = uncached_domain_fold_oracle(&first.dag, environment);
    let expected_second = uncached_domain_fold_oracle(&second.dag, environment);
    assert_ne!(expected_first, expected_second);
    for (proof, expected) in [(&mut first, expected_first), (&mut second, expected_second)] {
        let mut budget = RankedBoundsBudget::default();
        assert_eq!(proof.fold(environment, &mut budget), Ok(expected));
        assert_eq!((budget.work_units, budget.storage_items), (704, 0));
    }
    let mut budget = RankedBoundsBudget::default();
    assert_eq!(first.fold(environment, &mut budget), Ok(expected_first));
    assert_eq!(second.fold(environment, &mut budget), Ok(expected_second));
    assert_eq!((budget.work_units, budget.storage_items), (404, 0));
}
