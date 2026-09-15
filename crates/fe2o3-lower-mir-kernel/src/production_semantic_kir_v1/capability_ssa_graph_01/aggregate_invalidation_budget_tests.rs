// Source-scanner scaffolds only, not tuple typing or capability admission proofs.
fn aggregate_budget_body(
    operands: usize,
    moved: Option<usize>,
    destination: u32,
) -> SemanticFunctionDeclV1 {
    let values = (0..operands)
        .map(|index| {
            if moved == Some(index) {
                SemanticOperandV1::Move(place(1))
            } else {
                SemanticOperandV1::Constant(SemanticConstantV1::new(
                    SemanticTypeIdV1::from_index(1),
                    SemanticConstantValueV1::Scalar(SemanticScalarValueV1::new(0, 4).unwrap()),
                ))
            }
        })
        .collect();
    let aggregate = statement(SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
        place(destination),
        SemanticRvalueV1::new(
            SemanticTypeIdV1::from_index(1),
            SemanticRvalueKindV1::aggregate(SemanticAggregateKindV1::Tuple, values).unwrap(),
        ),
    )));
    body(vec![block(
        0,
        vec![
            assign(1, None),
            assign(2, Some(SemanticOperandV1::Copy(place(1)))),
            aggregate,
            statement(SemanticStatementKindV1::Nop),
        ],
        None,
    )])
}

#[test]
fn capability_aggregate_invalidation_increasing_operands_debit_cold_and_warm() {
    for operands in [0, 1, 2, 8, 32, 257] {
        let body = aggregate_budget_body(operands, None, 3);
        let plan = plan(&body);
        let mut graph = CapabilitySsaGraphV1::new(&body, plan.plan(), 100_000).unwrap();
        let expected = owner_index_expected_build_cost(&body, 1, 0);
        graph.remaining = expected;
        let index = graph.owner_statement_index(1).unwrap();
        assert_eq!(graph.remaining, 0);
        owner_index_assert_layout(&index, &body, 1);
        assert_eq!(index.offsets, [0, 1]);
        assert_eq!(index.positions, [0]);
        let hit = lookup_work(1) + 1;
        graph.remaining = hit;
        assert!(Arc::ptr_eq(
            &index,
            &graph.owner_statement_index(1).unwrap()
        ));
        assert_eq!(graph.remaining, 0);
        // Building was charged once; querying a warm row does not rescan operands.
        graph.remaining = 5;
        assert!(
            !graph
                .indexed_statement_invalidated(&index, 0, 2, 3)
                .unwrap()
        );
        assert_eq!(graph.remaining, 0);
    }
}

#[test]
fn capability_aggregate_invalidation_precharge_failure_never_publishes_loan() {
    for operands in [1, 2, 8, 32, 257] {
        for moved in [None, Some(0), Some(operands - 1)] {
            let body = aggregate_budget_body(operands, moved, 3);
            let plan = plan(&body);
            let mut graph = CapabilitySsaGraphV1::new(&body, plan.plan(), 100_000).unwrap();
            let vector_words =
                std::mem::size_of::<Vec<usize>>().div_ceil(std::mem::size_of::<usize>());
            // Before statement 2: lookup, two headers, offsets, block/statement
            // scan, then statement 0's push and first four-position allocation.
            let before_operands: usize = [
                lookup_work(0) + 1,
                2 * vector_words,
                body.blocks().len() + 1,
                1,
                body.blocks()[0].statements().len(),
                2,
                4,
            ]
            .into_iter()
            .sum();
            graph.remaining = before_operands + operands - 1;
            assert!(matches!(
                graph.owner_statement_index(1),
                Err(ProductionSemanticKirErrorV1::ResourceLimit { .. })
            ));
            assert_eq!(graph.remaining, operands - 1);
            assert!(graph.reuse.owner_invalidations.is_empty());
            assert!(graph.reuse.loans.is_empty());
        }
    }
}

#[test]
fn capability_aggregate_invalidation_exact_scan_budget_preserves_move_rejection() {
    for operands in [1, 8, 257] {
        for moved in [Some(0), Some(operands - 1)] {
            let body = aggregate_budget_body(operands, moved, 3);
            let plan = plan(&body);
            let mut graph = CapabilitySsaGraphV1::new(&body, plan.plan(), 100_000).unwrap();
            graph.remaining = owner_index_expected_build_cost(&body, 1, 0);
            let index = graph.owner_statement_index(1).unwrap();
            assert_eq!(graph.remaining, 0);
            owner_index_assert_layout(&index, &body, 1);
            assert_eq!(index.offsets, [0, 2]);
            assert_eq!(index.positions, [0, 2]);
            graph.remaining = 6;
            assert!(
                graph
                    .indexed_statement_invalidated(&index, 0, 2, 3)
                    .unwrap()
            );
            assert_eq!(graph.remaining, 0);
            graph.remaining = 100_000;
            let owner = graph.use_value(0, 1).unwrap();
            let mut reference = CapabilitySsaGraphV1::new(&body, plan.plan(), 100_000).unwrap();
            let actual = graph.loan_live(loan(owner, 1), reuse_consumer(0, Some(3)));
            assert!(matches!(
                &actual,
                Err(ProductionSemanticKirErrorV1::Unsupported {
                    detail: "capability loan crosses a move, overwrite, deinitialization or storage death",
                    ..
                })
            ));
            reuse_assert_same(
                actual,
                reference.loan_live_before_owner_index(loan(owner, 1), reuse_consumer(0, Some(3))),
            );
            assert!(graph.reuse.loans.is_empty());
        }
    }
}

#[test]
fn capability_aggregate_invalidation_excluded_windows_and_overwrites_keep_semantics() {
    for (start, end, destination, rejects) in [(1, 2, 3, false), (2, 3, 3, false), (1, 3, 1, true)]
    {
        for operands in [0, 1, 8, 257] {
            let body = aggregate_budget_body(operands, None, destination);
            let plan = plan(&body);
            let mut graph = CapabilitySsaGraphV1::new(&body, plan.plan(), 100_000).unwrap();
            let mut reference = CapabilitySsaGraphV1::new(&body, plan.plan(), 100_000).unwrap();
            let owner = graph.use_value(0, 1).unwrap();
            let borrowed = loan(owner, start);
            let consumer = reuse_consumer(0, Some(end));
            for _ in 0..2 {
                let actual = graph.loan_live(borrowed, consumer);
                assert_eq!(actual.is_err(), rejects);
                reuse_assert_same(
                    actual,
                    reference.loan_live_before_owner_index(borrowed, consumer),
                );
            }
            if rejects {
                assert!(graph.reuse.loans.is_empty());
            }
        }
    }
}
