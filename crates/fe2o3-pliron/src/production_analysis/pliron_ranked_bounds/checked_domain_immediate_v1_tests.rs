#[test]
fn checked_domain_immediate_constants_have_exact_work_and_zero_storage() {
    let context = Context::new();
    let graph = BoundsEdgeTransportV1 {
        context: &context,
        blocks: &[],
        predecessors: &[],
    };
    for leaf in [false, true] {
        for actual in [8, 9] {
            for available in [115, 114] {
                let dag = CheckedDomainDagV1::build(ROW, &mut RankedBoundsBudget::default())
                    .unwrap()
                    .unwrap();
                let expected = if leaf {
                    0
                } else {
                    dag.nodes[..dag.node_count]
                        .iter()
                        .position(|node| node.kind == CheckedDomainNodeKindV1::Constant(8))
                        .unwrap() as u8
                };
                let mut proof =
                    CheckedDomainProofV1::new(&graph, dag, &mut RankedBoundsBudget::default())
                        .unwrap();
                let state = CheckedDomainStateV1 {
                    block: 0,
                    environment: [IndexExpr::Constant(8); 6],
                    goal: CheckedDomainGoalV1::Equal {
                        actual: IndexExpr::Constant(actual),
                        expected,
                    },
                };
                let mut budget = RankedBoundsBudget {
                    work_units: MAX_RANKED_BOUNDS_WORK_UNITS - available,
                    storage_items: MAX_RANKED_BOUNDS_STORAGE_ITEMS,
                    ..RankedBoundsBudget::default()
                };
                let result = proof.equality(state, &mut budget);
                // Empty memo scan32 + immediate inspection16 + publication67.
                if available == 115 {
                    assert_eq!(result, Ok(actual == 8));
                    assert_eq!(budget.work_units, MAX_RANKED_BOUNDS_WORK_UNITS);
                    assert_eq!(proof.next_memo, 1);
                } else {
                    assert!(matches!(result,
                        Err(RankedBoundsFindingV1::ResourceLimitExceeded {
                            resource: "analysis work unit", actual, ..
                        }) if actual == MAX_RANKED_BOUNDS_WORK_UNITS + 1));
                    assert_eq!(budget.work_units, MAX_RANKED_BOUNDS_WORK_UNITS - 66);
                    assert!(proof.memo.iter().all(Option::is_none));
                    assert_eq!(proof.next_memo, 0);
                }
                assert_eq!(budget.storage_items, MAX_RANKED_BOUNDS_STORAGE_ITEMS);
            }
        }
    }
}

#[test]
fn checked_domain_immediate_entry_argument_has_exact_owner_roster_charge() {
    let source = domain_source(ROW, FixtureOptions::default());
    with_domain_graph(&source, |graph, _| {
        assert_eq!(graph.blocks[0].deref(graph.context).get_num_arguments(), 6);
        let actual = IndexExpr::Value(graph.blocks[0].deref(graph.context).get_argument(0));
        for available in [142, 141] {
            let dag = CheckedDomainDagV1::build(ROW, &mut RankedBoundsBudget::default())
                .unwrap()
                .unwrap();
            assert_eq!(dag.nodes[0].kind, CheckedDomainNodeKindV1::Leaf(0));
            let mut proof =
                CheckedDomainProofV1::new(graph, dag, &mut RankedBoundsBudget::default()).unwrap();
            let state = CheckedDomainStateV1 {
                block: 0,
                environment: [actual; 6],
                goal: CheckedDomainGoalV1::Equal {
                    actual,
                    expected: 0,
                },
            };
            let mut budget = RankedBoundsBudget {
                work_units: MAX_RANKED_BOUNDS_WORK_UNITS - available,
                storage_items: MAX_RANKED_BOUNDS_STORAGE_ITEMS,
                ..RankedBoundsBudget::default()
            };
            let result = proof.equality(state, &mut budget);
            // Memo32 + inspection16 + leaf8 + owner12 + roster(6+1) + publish67.
            if available == 142 {
                assert_eq!(result, Ok(true));
                assert_eq!(budget.work_units, MAX_RANKED_BOUNDS_WORK_UNITS);
                assert_eq!(proof.next_memo, 1);
            } else {
                assert!(matches!(result,
                    Err(RankedBoundsFindingV1::ResourceLimitExceeded {
                        resource: "analysis work unit", actual, ..
                    }) if actual == MAX_RANKED_BOUNDS_WORK_UNITS + 1));
                assert_eq!(budget.work_units, MAX_RANKED_BOUNDS_WORK_UNITS - 66);
                assert!(proof.memo.iter().all(Option::is_none));
                assert_eq!(proof.next_memo, 0);
            }
            assert_eq!(budget.storage_items, MAX_RANKED_BOUNDS_STORAGE_ITEMS);
        }
    });
}

#[test]
fn checked_domain_immediate_foreign_entry_argument_is_not_ownerless_authority() {
    let source = domain_source(ROW, FixtureOptions::default());
    let mut context = Context::new();
    fe2o3_pliron_owner_core::ensure_context_identity(&mut context).unwrap();
    register_dialect(&mut context, &DialectName::try_new(DIALECT_NAME).unwrap()).unwrap();
    let first = parse_from_str(Operation::top_level_parser(), &mut context, &source).unwrap();
    let second = parse_from_str(
        Operation::top_level_parser(),
        &mut context,
        &source.replace("@checked_domain", "@foreign_immediate"),
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
    let foreign = IndexExpr::Value(foreign_block.deref(&context).get_argument(0));
    let graph = BoundsEdgeTransportV1 {
        context: &context,
        blocks: &blocks,
        predecessors: &[],
    };
    for available in [68, 67] {
        let dag = CheckedDomainDagV1::build(ROW, &mut RankedBoundsBudget::default())
            .unwrap()
            .unwrap();
        let mut proof =
            CheckedDomainProofV1::new(&graph, dag, &mut RankedBoundsBudget::default()).unwrap();
        let state = CheckedDomainStateV1 {
            block: 0,
            environment: [foreign; 6],
            goal: CheckedDomainGoalV1::Equal {
                actual: foreign,
                expected: 0,
            },
        };
        let mut budget = RankedBoundsBudget {
            work_units: MAX_RANKED_BOUNDS_WORK_UNITS - available,
            storage_items: MAX_RANKED_BOUNDS_STORAGE_ITEMS,
            ..RankedBoundsBudget::default()
        };
        let result = proof.equality(state, &mut budget);
        // Memo32 + inspection16 + leaf8 + owner12; foreign region fails
        // before its roster scan and before any memo/worklist publication.
        if available == 68 {
            assert_eq!(
                result,
                Err(RankedBoundsFindingV1::StructuralVerificationFailed)
            );
            assert_eq!(budget.work_units, MAX_RANKED_BOUNDS_WORK_UNITS);
        } else {
            assert!(matches!(result,
                Err(RankedBoundsFindingV1::ResourceLimitExceeded {
                    resource: "analysis work unit", actual, ..
                }) if actual == MAX_RANKED_BOUNDS_WORK_UNITS + 1));
            assert_eq!(budget.work_units, MAX_RANKED_BOUNDS_WORK_UNITS - 11);
        }
        assert_eq!(budget.storage_items, MAX_RANKED_BOUNDS_STORAGE_ITEMS);
        assert!(proof.memo.iter().all(Option::is_none));
        assert_eq!(proof.next_memo, 0);
    }
}

#[test]
fn checked_domain_immediate_partial_same_value_leaf_still_uses_definedness() {
    let source = r#"builtin.func @partial_immediate: builtin.function <(kernel.index, kernel.index) -> ()>
{
  ^entry(a: kernel.index, b: kernel.index):
    q = kernel.index_binary (a, b) [] [kernel_index_binary_kind: kernel.index_binary_kind Divide]: <(kernel.index, kernel.index) -> (kernel.index)>;
    kernel.return () [] []: <() -> ()>
}
"#;
    with_domain_graph(source, |graph, _| {
        let operation = graph.blocks[0]
            .deref(graph.context)
            .iter(graph.context)
            .find(|operation| Operation::is_op::<IndexBinaryOp>(*operation, graph.context))
            .unwrap();
        let actual = IndexExpr::Value(operation.deref(graph.context).get_result(0));
        let dag = CheckedDomainDagV1::build(ROW, &mut RankedBoundsBudget::default())
            .unwrap()
            .unwrap();
        let mut proof =
            CheckedDomainProofV1::new(graph, dag, &mut RankedBoundsBudget::default()).unwrap();
        let state = CheckedDomainStateV1 {
            block: 0,
            environment: [actual; 6],
            goal: CheckedDomainGoalV1::Equal {
                actual,
                expected: 0,
            },
        };
        let mut budget = RankedBoundsBudget::default();
        assert_eq!(proof.equality(state, &mut budget), Ok(false));
        assert!(budget.storage_items > 0);
    });
}
