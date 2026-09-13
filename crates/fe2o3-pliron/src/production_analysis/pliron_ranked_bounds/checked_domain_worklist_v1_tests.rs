fn worklist_state(block: usize) -> CheckedDomainStateV1 {
    CheckedDomainStateV1 {
        block,
        environment: [IndexExpr::Constant(0); 6],
        goal: CheckedDomainGoalV1::Defined(IndexExpr::Constant(0)),
    }
}

#[test]
fn checked_domain_worklist_owner_has_literal_exact_and_one_under_admission() {
    let context = Context::new();
    let graph = BoundsEdgeTransportV1 {
        context: &context,
        blocks: &[],
        predecessors: &[],
    };
    // Row depth8/B0: 2611 query cells + 8*768 frame cells = 8755.
    // Depth32 + block check4 + init/drop(2*8755) + reserve1 = 17547.
    for (work, storage) in [(17_547, 8_755), (17_546, 8_755), (17_547, 8_754)] {
        let dag = CheckedDomainDagV1::build(ROW, &mut RankedBoundsBudget::default())
            .unwrap()
            .unwrap();
        let mut budget = RankedBoundsBudget {
            work_units: MAX_RANKED_BOUNDS_WORK_UNITS - work,
            storage_items: MAX_RANKED_BOUNDS_STORAGE_ITEMS - storage,
            ..RankedBoundsBudget::default()
        };
        let result = CheckedDomainProofV1::new(&graph, dag, &mut budget);
        if work == 17_547 && storage == 8_755 {
            let proof = result.unwrap();
            assert!(proof.fold_cache.is_none());
            assert_eq!(proof.depth_limit, 8);
            assert_eq!(proof.active_queries, 0);
            assert_eq!(proof.worklists.len(), 128);
            assert!(
                proof
                    .worklists
                    .iter()
                    .all(|list| list.is_empty() && list.capacity() == 0)
            );
            assert_eq!(
                (budget.work_units, budget.storage_items),
                (
                    MAX_RANKED_BOUNDS_WORK_UNITS,
                    MAX_RANKED_BOUNDS_STORAGE_ITEMS
                )
            );
        } else if work == 17_546 {
            assert!(
                matches!(result, Err(RankedBoundsFindingV1::ResourceLimitExceeded {
                resource: "analysis work unit", actual, ..
            }) if actual == MAX_RANKED_BOUNDS_WORK_UNITS + 1)
            );
            assert_eq!(budget.work_units, MAX_RANKED_BOUNDS_WORK_UNITS - 17_510);
            assert_eq!(budget.storage_items, MAX_RANKED_BOUNDS_STORAGE_ITEMS);
        } else {
            assert!(
                matches!(result, Err(RankedBoundsFindingV1::ResourceLimitExceeded {
                resource: "analysis storage item", actual, ..
            }) if actual == MAX_RANKED_BOUNDS_STORAGE_ITEMS + 1)
            );
            assert_eq!(budget.work_units, MAX_RANKED_BOUNDS_WORK_UNITS - 17_511);
            assert_eq!(
                budget.storage_items,
                MAX_RANKED_BOUNDS_STORAGE_ITEMS - 8_754
            );
        }
    }
}

#[test]
fn checked_domain_worklist_reuse_clears_facts_and_restores_after_errors() {
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
    let mut budget = RankedBoundsBudget::default();
    let first = proof.with_worklist(&mut budget, |_, visited, budget| {
        assert!(visited.is_empty());
        checked_domain_enqueue_v1(visited, worklist_state(0), budget)?;
        Err::<(), _>(RankedBoundsFindingV1::StructuralVerificationFailed)
    });
    assert_eq!(
        first,
        Err(RankedBoundsFindingV1::StructuralVerificationFailed)
    );
    // Scope32 + first enqueue135, with one 4*64-cell capacity.
    assert_eq!((budget.work_units, budget.storage_items), (167, 256));
    assert_eq!(proof.active_queries, 0);
    assert!(proof.worklists[0].is_empty());
    assert_eq!(proof.worklists[0].capacity(), 4);
    proof
        .with_worklist(&mut budget, |_, visited, budget| {
            assert!(visited.is_empty());
            checked_domain_enqueue_v1(visited, worklist_state(0), budget)?;
            assert_eq!(visited.len(), 1);
            Ok(())
        })
        .unwrap();
    // Reuse scope32 + empty scan1 + publication129; no growth or storage.
    assert_eq!((budget.work_units, budget.storage_items), (329, 256));
    assert_eq!(proof.active_queries, 0);
    assert!(proof.worklists.iter().all(Vec::is_empty));
    for available in [162, 161, 31] {
        let mut meter = RankedBoundsBudget {
            work_units: MAX_RANKED_BOUNDS_WORK_UNITS - available,
            storage_items: MAX_RANKED_BOUNDS_STORAGE_ITEMS,
            ..RankedBoundsBudget::default()
        };
        let result = proof.with_worklist(&mut meter, |_, visited, budget| {
            assert!(visited.is_empty());
            checked_domain_enqueue_v1(visited, worklist_state(1), budget)
        });
        if available == 162 {
            assert_eq!(result, Ok(()));
            assert_eq!(meter.work_units, MAX_RANKED_BOUNDS_WORK_UNITS);
        } else {
            assert!(
                matches!(result, Err(RankedBoundsFindingV1::ResourceLimitExceeded {
                resource: "analysis work unit", actual, ..
            }) if actual == MAX_RANKED_BOUNDS_WORK_UNITS + 1)
            );
            assert_eq!(
                meter.work_units,
                MAX_RANKED_BOUNDS_WORK_UNITS - if available == 161 { 128 } else { 31 }
            );
        }
        assert_eq!(meter.storage_items, MAX_RANKED_BOUNDS_STORAGE_ITEMS);
        assert_eq!(proof.active_queries, 0);
        assert!(proof.worklists.iter().all(Vec::is_empty));
        assert_eq!(proof.worklists[0].capacity(), 4);
        assert!(proof.memo.iter().all(Option::is_none));
    }
}

fn nested_worklists(
    proof: &mut CheckedDomainProofV1<'_, '_>,
    depth: usize,
    budget: &mut RankedBoundsBudget,
) -> Result<(), RankedBoundsFindingV1> {
    proof.with_worklist(budget, |proof, visited, budget| {
        assert!(visited.is_empty());
        let state = worklist_state(depth);
        checked_domain_enqueue_v1(visited, state, budget)?;
        if depth > 1 {
            nested_worklists(proof, depth - 1, budget)?;
        }
        assert_eq!(visited.as_slice(), &[state]);
        Ok(())
    })
}

#[test]
fn checked_domain_worklist_nested_owners_obey_the_actual_dag_depth() {
    let context = Context::new();
    let graph = BoundsEdgeTransportV1 {
        context: &context,
        blocks: &[],
        predecessors: &[],
    };
    for depth in [8, 9] {
        let dag = CheckedDomainDagV1::build(ROW, &mut RankedBoundsBudget::default())
            .unwrap()
            .unwrap();
        let mut proof =
            CheckedDomainProofV1::new(&graph, dag, &mut RankedBoundsBudget::default()).unwrap();
        let mut budget = RankedBoundsBudget::default();
        let result = nested_worklists(&mut proof, depth, &mut budget);
        if depth == 8 {
            assert_eq!(result, Ok(()));
            assert_eq!(budget.work_units, 8 * 167);
        } else {
            assert!(matches!(
                result,
                Err(RankedBoundsFindingV1::ResourceLimitExceeded {
                    resource: "checked domain worklist depth",
                    limit: 8,
                    actual: 9,
                })
            ));
            assert_eq!(budget.work_units, 8 * 167 + 32);
        }
        assert_eq!(budget.storage_items, 8 * 256);
        assert_eq!(proof.active_queries, 0);
        assert!(proof.worklists.iter().all(Vec::is_empty));
        assert!(proof.worklists[..8].iter().all(|list| list.capacity() == 4));
        assert!(proof.worklists[8..].iter().all(|list| list.capacity() == 0));
    }
    let mut dag = CheckedDomainDagV1::build(ROW, &mut RankedBoundsBudget::default())
        .unwrap()
        .unwrap();
    dag.formulas[0].depth = 129;
    let mut budget = RankedBoundsBudget::default();
    assert!(matches!(
        CheckedDomainProofV1::new(&graph, dag, &mut budget),
        Err(RankedBoundsFindingV1::StructuralVerificationFailed)
    ));
    assert_eq!((budget.work_units, budget.storage_items), (32, 0));
    let mut dag = CheckedDomainDagV1::build(ROW, &mut RankedBoundsBudget::default())
        .unwrap()
        .unwrap();
    dag.formula_count = 0;
    dag.required = 0;
    let mut budget = RankedBoundsBudget::default();
    let mut proof = CheckedDomainProofV1::new(&graph, dag, &mut budget).unwrap();
    // A zero-formula helper owner still prepays one frame, never zero.
    assert_eq!(proof.depth_limit, 1);
    assert_eq!((budget.work_units, budget.storage_items), (6_795, 3_379));
    let mut query = RankedBoundsBudget::default();
    assert!(matches!(
        nested_worklists(&mut proof, 2, &mut query),
        Err(RankedBoundsFindingV1::ResourceLimitExceeded {
            resource: "checked domain worklist depth",
            limit: 1,
            actual: 2,
        })
    ));
    assert_eq!((query.work_units, query.storage_items), (199, 256));
    assert_eq!(proof.active_queries, 0);
    assert!(proof.worklists.iter().all(Vec::is_empty));
}

#[test]
fn checked_domain_worklist_predicate_answers_do_not_leak_between_environments() {
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
    let mut budget = RankedBoundsBudget::default();
    // Formula0 is the component<elements predicate of the ordinary Row DAG.
    let formula = proof.dag.formulas[0];
    assert_eq!(
        proof.dag.nodes[usize::from(formula.atoms[0].lhs)].kind,
        CheckedDomainNodeKindV1::Leaf(1)
    );
    assert_eq!(
        proof.dag.nodes[usize::from(formula.atoms[0].rhs)].kind,
        CheckedDomainNodeKindV1::Constant(2)
    );
    for (component, expected) in [(0, true), (2, false), (1, true)] {
        let mut environment = [IndexExpr::Constant(0); 6];
        environment[1] = IndexExpr::Constant(component);
        let state = CheckedDomainStateV1 {
            block: 0,
            environment,
            goal: CheckedDomainGoalV1::Predicate(CheckedDomainResidualV1::new(0, &proof.dag)),
        };
        assert_eq!(proof.predicate(state, &mut budget), Ok(expected));
        assert_eq!(budget.storage_items, 256);
        assert_eq!(proof.active_queries, 0);
        assert!(proof.worklists.iter().all(Vec::is_empty));
    }
}

#[test]
fn checked_domain_worklist_reused_capacity_growth_is_admitted_before_allocation() {
    let context = Context::new();
    let graph = BoundsEdgeTransportV1 {
        context: &context,
        blocks: &[],
        predecessors: &[],
    };
    for (work, storage) in [(1_597, 512), (1_596, 512), (1_597, 511)] {
        let dag = CheckedDomainDagV1::build(ROW, &mut RankedBoundsBudget::default())
            .unwrap()
            .unwrap();
        let mut proof =
            CheckedDomainProofV1::new(&graph, dag, &mut RankedBoundsBudget::default()).unwrap();
        let mut setup = RankedBoundsBudget::default();
        proof
            .with_worklist(&mut setup, |_, visited, budget| {
                for block in 0..4 {
                    checked_domain_enqueue_v1(visited, worklist_state(block), budget)?;
                }
                Ok(())
            })
            .unwrap();
        // Scope32 + 4 publications130 + 65*(0+1+2+3) + initial growth5.
        assert_eq!((setup.work_units, setup.storage_items), (947, 256));
        assert_eq!(proof.worklists[0].capacity(), 4);
        let mut budget = RankedBoundsBudget {
            work_units: MAX_RANKED_BOUNDS_WORK_UNITS - work,
            storage_items: MAX_RANKED_BOUNDS_STORAGE_ITEMS - storage,
            ..RankedBoundsBudget::default()
        };
        let result = proof.with_worklist(&mut budget, |_, visited, budget| {
            assert!(visited.is_empty());
            for block in 0..5 {
                checked_domain_enqueue_v1(visited, worklist_state(block), budget)?;
            }
            Ok(())
        });
        // Scope32 + 5 publications130 + 65*(0+1+2+3+4)
        // + growth(4*64+8+1) =1597; new capacity8 costs512 cells.
        if work == 1_597 && storage == 512 {
            assert_eq!(result, Ok(()));
            assert_eq!(
                (budget.work_units, budget.storage_items),
                (
                    MAX_RANKED_BOUNDS_WORK_UNITS,
                    MAX_RANKED_BOUNDS_STORAGE_ITEMS
                )
            );
            assert_eq!(proof.worklists[0].capacity(), 8);
        } else if work == 1_596 {
            assert!(
                matches!(result, Err(RankedBoundsFindingV1::ResourceLimitExceeded {
                resource: "analysis work unit", actual, ..
            }) if actual == MAX_RANKED_BOUNDS_WORK_UNITS + 1)
            );
            assert_eq!(budget.work_units, MAX_RANKED_BOUNDS_WORK_UNITS - 128);
            assert_eq!(budget.storage_items, MAX_RANKED_BOUNDS_STORAGE_ITEMS);
            assert_eq!(proof.worklists[0].capacity(), 8);
        } else {
            assert!(
                matches!(result, Err(RankedBoundsFindingV1::ResourceLimitExceeded {
                resource: "analysis storage item", actual, ..
            }) if actual == MAX_RANKED_BOUNDS_STORAGE_ITEMS + 1)
            );
            assert_eq!(budget.work_units, MAX_RANKED_BOUNDS_WORK_UNITS - 394);
            assert_eq!(budget.storage_items, MAX_RANKED_BOUNDS_STORAGE_ITEMS - 511);
            assert_eq!(proof.worklists[0].capacity(), 4);
        }
        assert_eq!(proof.active_queries, 0);
        assert!(proof.worklists.iter().all(Vec::is_empty));
        assert!(proof.memo.iter().all(Option::is_none));
    }
}
