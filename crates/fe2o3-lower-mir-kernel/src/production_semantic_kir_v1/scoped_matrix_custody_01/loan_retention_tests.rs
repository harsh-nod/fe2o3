use fe2o3_pliron::plan_semantic_function_ssa_v1;

// As in capability_ssa_graph_01/tests.rs, scalar scaffolds exercise actual SSA
// identities and storage intervals. They are not Context/Matrix source issuers.
fn place(local: u32) -> SemanticPlaceV1 {
    SemanticPlaceV1::new(
        SemanticLocalIdV1::from_index(local),
        vec![],
        SemanticTypeIdV1::from_index(1),
    )
    .unwrap()
}
fn statement(kind: SemanticStatementKindV1) -> SemanticStatementV1 {
    SemanticStatementV1::new(SemanticSourceProvenanceV1::unavailable(), kind)
}
fn assign(local: u32, source: Option<u32>) -> SemanticStatementV1 {
    let operand = source
        .map(|s| SemanticOperandV1::Copy(place(s)))
        .unwrap_or_else(|| {
            SemanticOperandV1::Constant(SemanticConstantV1::new(
                SemanticTypeIdV1::from_index(1),
                SemanticConstantValueV1::Scalar(
                    SemanticScalarValueV1::new(local.into(), 4).unwrap(),
                ),
            ))
        });
    statement(SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
        place(local),
        SemanticRvalueV1::new(
            SemanticTypeIdV1::from_index(1),
            SemanticRvalueKindV1::Use(operand),
        ),
    )))
}
fn block(
    index: u8,
    statements: Vec<SemanticStatementV1>,
    next: Option<u32>,
) -> SemanticBasicBlockV1 {
    let source = SemanticSourceProvenanceV1::unavailable();
    SemanticBasicBlockV1::new(
        SemanticBlockIdentityV1::from_sha256([60 + index; 32]),
        source,
        statements,
        SemanticTerminatorV1::new(
            source,
            next.map(|next| {
                SemanticTerminatorKindV1::Goto(SemanticControlFlowEdgeV1::new(
                    SemanticEdgeRoleV1::Goto,
                    SemanticBlockIdV1::from_index(next),
                ))
            })
            .unwrap_or(SemanticTerminatorKindV1::Return),
        ),
    )
    .unwrap()
}
fn body(kill: Option<SemanticStatementV1>) -> SemanticFunctionDeclV1 {
    let original =
        super::super::super::resource_tests::noop_semantic_owner(&["scoped_loan_interval"]);
    SemanticFunctionDeclV1::new(
        SemanticFunctionIdentityV1::from_sha256([31; 32]),
        SemanticFunctionRoleV1::KernelRoot,
        SemanticItemDefinitionIdentityV1::from_sha256([32; 32]),
        SemanticMonomorphizationIdentityV1::from_sha256([33; 32]),
        SemanticGenericTypeArgumentsIdentityV1::from_sha256([34; 32]),
        SemanticConstGenericArgumentsIdentityV1::from_sha256([35; 32]),
        SemanticSourceProvenanceV1::unavailable(),
        original.semantic().functions()[0].abi().clone(),
        (0..7u8)
            .map(|i| {
                SemanticLocalDeclV1::new(
                    SemanticLocalIdentityV1::from_sha256([40 + i; 32]),
                    SemanticTypeIdV1::from_index(u32::from(i != 0)),
                    if i == 0 {
                        SemanticLocalRoleV1::Return
                    } else {
                        SemanticLocalRoleV1::Temporary
                    },
                    SemanticSourceProvenanceV1::unavailable(),
                )
            })
            .collect(),
        SemanticBlockIdV1::from_index(0),
        vec![
            block(
                0,
                vec![
                    assign(1, None),
                    assign(2, Some(1)),
                    assign(3, None),
                    assign(4, Some(3)),
                ],
                Some(1),
            ),
            block(1, kill.into_iter().collect(), Some(2)),
            block(2, vec![assign(5, Some(4))], None),
        ],
    )
    .unwrap()
}
fn loans(graph: &mut Graph<'_>) -> (Loan, Loan) {
    let context = Loan {
        borrow: Site {
            block: 0,
            statement: Some(1),
            local: 2,
        },
        owner_local: 1,
        owner_value: graph.use_value(0, 1).unwrap(),
    };
    let workgroup = Loan {
        borrow: Site {
            block: 0,
            statement: Some(3),
            local: 4,
        },
        owner_local: 3,
        owner_value: graph.use_value(0, 3).unwrap(),
    };
    (context, workgroup)
}
fn end() -> Site {
    Site {
        block: 2,
        statement: None,
        local: 0,
    }
}

#[test]
fn scoped_subgroup_keeps_context_death_and_redefinition_after_derivation() {
    for kill in [
        statement(SemanticStatementKindV1::StorageDead(
            SemanticLocalIdV1::from_index(1),
        )),
        statement(SemanticStatementKindV1::StorageLive(
            SemanticLocalIdV1::from_index(1),
        )),
        assign(1, None),
    ] {
        let body = body(Some(kill));
        let plan = plan_semantic_function_ssa_v1(
            SemanticFunctionIdV1::from_index(0),
            &body,
            ProductionSemanticSsaLimitsV1::default(),
        )
        .unwrap();
        let mut graph = Graph::new(&body, plan.plan(), 100_000).unwrap();
        let (context, workgroup) = loans(&mut graph);
        // The early derivation check succeeds and is cached. It is insufficient
        // for a later consumer. The surviving Workgroup-only loan also passes.
        graph
            .loan_live(
                context,
                Site {
                    block: 0,
                    statement: Some(2),
                    local: 3,
                },
            )
            .unwrap();
        graph.loan_live(workgroup, end()).unwrap();
        let retained = super::super::resolve::subgroup_loans(
            &mut graph,
            BTreeSet::from([context]),
            &[workgroup],
        )
        .unwrap();
        assert_eq!(retained, BTreeSet::from([context, workgroup]));
        let error = retained
            .iter()
            .try_for_each(|loan| graph.loan_live(*loan, end()))
            .unwrap_err();
        assert!(
            matches!(error, ProductionSemanticKirErrorV1::Unsupported { detail, .. }
            if detail == "capability loan crosses a move, overwrite, deinitialization or storage death"),
            "must reject the original Context lifetime, not fixture/work failure: {error:?}"
        );
    }
}

#[test]
fn scoped_subgroup_deinitialized_context_cannot_invent_an_ssa_loan() {
    // This source operation intentionally retains storage. It cannot enter
    // the loan query without an actual promoted owner use.
    let body = body(Some(statement(SemanticStatementKindV1::Deinitialize(
        place(1),
    ))));
    let plan = plan_semantic_function_ssa_v1(
        SemanticFunctionIdV1::from_index(0),
        &body,
        ProductionSemanticSsaLimitsV1::default(),
    )
    .unwrap();
    assert!(!plan.plan().promoted_variables().contains(
        &fe2o3_mir_model::SsaVariableIdV1::new(1)
    ));
    assert!(plan.plan().promoted_variables().contains(
        &fe2o3_mir_model::SsaVariableIdV1::new(3)
    ));
    assert!(super::super::super::capability_ssa_graph_01::invalidates(
        body.blocks()[1].statements()[0].kind(),
        1,
    ));
    let mut graph = Graph::new(&body, plan.plan(), 100_000).unwrap();
    for _ in 0..2 {
        let error = graph.use_value(0, 1).unwrap_err();
        assert!(
            matches!(&error, ProductionSemanticKirErrorV1::Unsupported {
                function: 0, block: None, statement: None,
                detail: "capability reference has no exact SSA use",
            }),
            "deinitialization must fail at the real SSA boundary: {error:?}"
        );
    }
    // The independent live owner still has its real use in this same plan.
    let workgroup = Loan {
        borrow: Site { block: 0, statement: Some(3), local: 4 },
        owner_local: 3,
        owner_value: graph.use_value(0, 3).unwrap(),
    };
    graph.loan_live(workgroup, end()).unwrap();
}

#[test]
fn scoped_subgroup_preserves_live_owner_when_only_forwarding_slot_dies() {
    for kill in [
        None,
        Some(statement(SemanticStatementKindV1::StorageDead(
            SemanticLocalIdV1::from_index(2),
        ))),
    ] {
        let body = body(kill);
        let plan = plan_semantic_function_ssa_v1(
            SemanticFunctionIdV1::from_index(0),
            &body,
            ProductionSemanticSsaLimitsV1::default(),
        )
        .unwrap();
        let mut graph = Graph::new(&body, plan.plan(), 100_000).unwrap();
        let (context, workgroup) = loans(&mut graph);
        let retained = super::super::resolve::subgroup_loans(
            &mut graph,
            BTreeSet::from([context]),
            &[workgroup],
        )
        .unwrap();
        assert_eq!(retained.len(), 2);
        for loan in retained {
            graph.loan_live(loan, end()).unwrap();
        }
    }
}

#[test]
fn scoped_subgroup_loan_copy_precharges_visits_including_duplicates() {
    let body = body(None);
    let plan = plan_semantic_function_ssa_v1(
        SemanticFunctionIdV1::from_index(0),
        &body,
        ProductionSemanticSsaLimitsV1::default(),
    )
    .unwrap();
    let mut probe = Graph::new(&body, plan.plan(), 100_000).unwrap();
    let (context, workgroup) = loans(&mut probe);
    let mut graph = Graph::new(&body, plan.plan(), 128).unwrap();
    let copies = vec![workgroup; 128];
    assert!(matches!(
        super::super::resolve::subgroup_loans(&mut graph, BTreeSet::from([context]), &copies),
        Err(ProductionSemanticKirErrorV1::ResourceLimit { .. })
    ));
    let merged =
        super::super::resolve::subgroup_loans(&mut probe, BTreeSet::from([context]), &copies)
            .unwrap();
    assert_eq!(merged, BTreeSet::from([context, workgroup]));
}
