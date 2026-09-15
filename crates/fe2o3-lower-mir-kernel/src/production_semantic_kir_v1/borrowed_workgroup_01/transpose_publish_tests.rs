//! Endpoint/graph component tests. Scalar scaffolding below is not an
//! authenticated Workgroup or a production transpose issuance receipt.
use super::*;

fn rejected(error: ProductionSemanticKirErrorV1, expected: &'static str) {
    assert!(
        matches!(error, ProductionSemanticKirErrorV1::Unsupported {
        function: 0, block: None, statement: None, detail,
    } if detail == expected),
        "{error:?}"
    );
}

fn call_block(
    statements: Vec<SemanticStatementV1>,
    target: u32,
    move_owner: bool,
) -> SemanticBasicBlockV1 {
    SemanticBasicBlockV1::new(
        SemanticBlockIdentityV1::from_sha256([62; 32]),
        SemanticSourceProvenanceV1::unavailable(),
        statements,
        SemanticTerminatorV1::new(
            SemanticSourceProvenanceV1::unavailable(),
            SemanticTerminatorKindV1::Call(
                SemanticDirectCallV1::new_callable(
                    SemanticCallableIdV1::from_index(0),
                    if move_owner {
                        vec![SemanticOperandV1::Move(place(1))]
                    } else {
                        vec![]
                    },
                    Some(SemanticCallDestinationV1::new(
                        place(2),
                        SemanticControlFlowEdgeV1::new(
                            SemanticEdgeRoleV1::CallReturn,
                            SemanticBlockIdV1::from_index(target),
                        ),
                    )),
                    SemanticUnwindActionV1::Unreachable,
                )
                .unwrap(),
            ),
        ),
    )
    .unwrap()
}

fn fixture(statements: Vec<SemanticStatementV1>, move_owner: bool) -> SemanticFunctionDeclV1 {
    body(vec![
        block(0, vec![assign(1, None)], Some(1)),
        block(1, vec![assign(3, Some(1))], Some(2)),
        call_block(statements, 3, move_owner),
        block(3, vec![], None),
    ])
}

fn loan(graph: &mut Graph<'_>) -> Loan {
    Loan {
        borrow: Site {
            block: 1,
            statement: Some(0),
            local: 3,
        },
        owner_value: graph.use_value(1, 1).unwrap(),
        owner_local: 1,
    }
}

#[test]
fn pre_publish_point_excludes_only_its_own_call_move() {
    let body = fixture(vec![assign(3, Some(1))], true);
    let plan = plan(&body);
    let mut graph = Graph::new(&body, plan.plan(), 100_000).unwrap();
    let loan = loan(&mut graph);
    let before = transpose_before_publish_point(&mut graph, 2, 3).unwrap();
    assert_eq!(
        before,
        Site {
            block: 2,
            statement: Some(1),
            local: 0
        }
    );
    graph.loan_live(loan, before).unwrap();
    let error = graph
        .loan_live(
            loan,
            Site {
                block: 2,
                statement: None,
                local: 0,
            },
        )
        .unwrap_err();
    rejected(
        error,
        "capability loan crosses an owner call move or overwrite",
    );
}

#[test]
fn every_same_block_statement_invalidation_remains_before_publish() {
    let source = SemanticSourceProvenanceV1::unavailable();
    let mutations = [
        SemanticStatementV1::new(
            source,
            SemanticStatementKindV1::StorageDead(SemanticLocalIdV1::from_index(1)),
        ),
        assign(1, None),
        SemanticStatementV1::new(
            source,
            SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
                place(3),
                SemanticRvalueV1::new(
                    place(1).ty(),
                    SemanticRvalueKindV1::Use(SemanticOperandV1::Move(place(1))),
                ),
            )),
        ),
    ];
    for mutation in mutations {
        let body = fixture(vec![mutation], false);
        let plan = plan(&body);
        let mut graph = Graph::new(&body, plan.plan(), 100_000).unwrap();
        let loan = loan(&mut graph);
        let before = transpose_before_publish_point(&mut graph, 2, 3).unwrap();
        let error = graph.loan_live(loan, before).unwrap_err();
        rejected(
            error,
            "capability loan crosses a move, overwrite, deinitialization or storage death",
        );
    }
}

#[test]
fn deinitialized_owner_is_retained_and_cannot_supply_a_pre_publish_ssa_loan() {
    let body = fixture(
        vec![SemanticStatementV1::new(
            SemanticSourceProvenanceV1::unavailable(),
            SemanticStatementKindV1::Deinitialize(place(1)),
        )],
        false,
    );
    let plan = plan(&body);
    // The real adapter retains this scalar's storage. Do not invent a loan
    // from a different body's promoted SSA plan to exercise the later scanner.
    assert!(
        !plan.plan().promoted_variables().contains(
            &fe2o3_mir_model::SsaVariableIdV1::new(1)
        )
    );
    let mut graph = Graph::new(&body, plan.plan(), 100_000).unwrap();
    let before = transpose_before_publish_point(&mut graph, 2, 3).unwrap();
    assert_eq!(
        before,
        Site {
            block: 2,
            statement: Some(1),
            local: 0,
        }
    );
    assert!(
        super::super::super::capability_ssa_graph_01::invalidates(
            body.blocks()[2].statements()[0].kind(),
            1,
        )
    );
    rejected(
        graph.use_value(1, 1).unwrap_err(),
        "capability reference has no exact SSA use",
    );
}

#[test]
fn forbidden_borrow_is_still_a_statement_invalidation() {
    for kind in [SemanticBorrowKindV1::Mutable, SemanticBorrowKindV1::Shared] {
        let statement = SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
            place(3),
            SemanticRvalueV1::new(
                place(3).ty(),
                SemanticRvalueKindV1::Borrow {
                    kind,
                    place: place(1),
                },
            ),
        ));
        assert_eq!(
            super::super::super::capability_ssa_graph_01::invalidates(&statement, 1),
            kind != SemanticBorrowKindV1::Shared
        );
    }
}

#[test]
fn self_loop_and_nonempty_backedge_reject_before_any_loan_iteration() {
    for next in [2, 3] {
        let body = body(vec![
            block(0, vec![assign(1, None)], Some(1)),
            block(1, vec![assign(3, Some(1))], Some(2)),
            call_block(vec![], next, false),
            block(3, vec![], Some(4)),
            block(4, vec![], Some(2)),
        ]);
        let plan = plan(&body);
        let mut graph = Graph::new(&body, plan.plan(), 100_000).unwrap();
        // This production helper executes before looking at Origin.loans.
        let error = transpose_before_publish_point(&mut graph, 2, next).unwrap_err();
        rejected(
            error,
            "transpose Publish crosses a cycle without a proven epoch generation",
        );
    }
}

#[test]
fn unrelated_dead_backedge_does_not_become_a_current_publish_cycle() {
    let body = body(vec![
        block(0, vec![assign(1, None)], Some(1)),
        block(1, vec![assign(3, Some(1))], Some(2)),
        call_block(vec![], 3, true),
        block(3, vec![], None),
        block(4, vec![], Some(2)),
    ]);
    let plan = plan(&body);
    let mut graph = Graph::new(&body, plan.plan(), 100_000).unwrap();
    let loan = loan(&mut graph);
    let before = transpose_before_publish_point(&mut graph, 2, 3).unwrap();
    graph.loan_live(loan, before).unwrap();
}

#[test]
fn pre_publish_work_failure_preserves_exact_shared_resource_error() {
    let body = fixture(vec![], false);
    let plan = plan(&body);
    let mut graph = Graph::new(&body, plan.plan(), 100_000).unwrap();
    // Exhaust the SAME allowance without refunding already spent graph work.
    while graph.charge(1).is_ok() {}
    let error = transpose_before_publish_point(&mut graph, 2, 3).unwrap_err();
    assert!(matches!(
        error,
        ProductionSemanticKirErrorV1::ResourceLimit {
            resource: ProductionSemanticKirResourceV1::AnalysisWork,
            actual: 100_001,
            limit: 100_000,
        }
    ));
}
