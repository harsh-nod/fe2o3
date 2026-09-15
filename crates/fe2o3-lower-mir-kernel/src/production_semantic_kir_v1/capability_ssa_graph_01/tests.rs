use super::*;
use fe2o3_mir_model::semantic_mir_v1::*;
use fe2o3_pliron::plan_semantic_function_ssa_v1;

// Scalar scaffolds test graph identities and storage intervals, not issuance.
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

fn assign(local: u32, operand: Option<SemanticOperandV1>) -> SemanticStatementV1 {
    statement(SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
        place(local),
        SemanticRvalueV1::new(
            SemanticTypeIdV1::from_index(1),
            SemanticRvalueKindV1::Use(operand.unwrap_or_else(|| {
                SemanticOperandV1::Constant(SemanticConstantV1::new(
                    SemanticTypeIdV1::from_index(1),
                    SemanticConstantValueV1::Scalar(
                        SemanticScalarValueV1::new(local.into(), 4).unwrap(),
                    ),
                ))
            })),
        ),
    )))
}

fn block(
    index: u8,
    statements: Vec<SemanticStatementV1>,
    next: Option<u32>,
) -> SemanticBasicBlockV1 {
    SemanticBasicBlockV1::new(
        SemanticBlockIdentityV1::from_sha256([60 + index; 32]),
        SemanticSourceProvenanceV1::unavailable(),
        statements,
        SemanticTerminatorV1::new(
            SemanticSourceProvenanceV1::unavailable(),
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

fn body(blocks: Vec<SemanticBasicBlockV1>) -> SemanticFunctionDeclV1 {
    let original =
        super::super::resource_tests::noop_semantic_owner(&["capability_graph_scaffold"]);
    SemanticFunctionDeclV1::new(
        SemanticFunctionIdentityV1::from_sha256([31; 32]),
        SemanticFunctionRoleV1::KernelRoot,
        SemanticItemDefinitionIdentityV1::from_sha256([32; 32]),
        SemanticMonomorphizationIdentityV1::from_sha256([33; 32]),
        SemanticGenericTypeArgumentsIdentityV1::from_sha256([34; 32]),
        SemanticConstGenericArgumentsIdentityV1::from_sha256([35; 32]),
        SemanticSourceProvenanceV1::unavailable(),
        original.semantic().functions()[0].abi().clone(),
        (0..4)
            .map(|index| {
                SemanticLocalDeclV1::new(
                    SemanticLocalIdentityV1::from_sha256([40 + index; 32]),
                    SemanticTypeIdV1::from_index(u32::from(index != 0)),
                    if index == 0 {
                        SemanticLocalRoleV1::Return
                    } else {
                        SemanticLocalRoleV1::Temporary
                    },
                    SemanticSourceProvenanceV1::unavailable(),
                )
            })
            .collect(),
        SemanticBlockIdV1::from_index(0),
        blocks,
    )
    .unwrap()
}

fn plan(body: &SemanticFunctionDeclV1) -> ProductionSemanticSsaFunctionPlanV1 {
    plan_semantic_function_ssa_v1(
        SemanticFunctionIdV1::from_index(0),
        body,
        ProductionSemanticSsaLimitsV1::default(),
    )
    .unwrap()
}

fn loan(value: SsaValueV1, statement: u32) -> CapabilityLoanV1 {
    CapabilityLoanV1 {
        borrow: CapabilityDefinitionSiteV1 {
            block: 0,
            statement: Some(statement),
            local: 2,
        },
        owner_local: 1,
        owner_value: value,
    }
}

#[test]
fn capability_graph_distinguishes_same_typed_owner_definitions() {
    let body = body(vec![block(
        0,
        vec![
            assign(1, None),
            assign(2, None),
            assign(3, Some(SemanticOperandV1::Copy(place(1)))),
            assign(3, Some(SemanticOperandV1::Copy(place(2)))),
        ],
        None,
    )]);
    let plan = plan(&body);
    let mut graph = CapabilitySsaGraphV1::new(&body, plan.plan(), 1000).unwrap();
    let first = graph.use_value(0, 1).unwrap();
    let second = graph.use_value(0, 2).unwrap();
    assert_ne!(first, second);
    assert_eq!(graph.definition(first).unwrap().statement, Some(0));
    assert_eq!(graph.definition(second).unwrap().statement, Some(1));
    assert!(
        graph
            .loan_live(
                loan(second, 2),
                CapabilityDefinitionSiteV1 {
                    block: 0,
                    statement: None,
                    local: 0
                }
            )
            .is_err()
    );
}

#[test]
fn capability_graph_lifetime_scan_respects_before_borrow_and_after_consumer_order() {
    let body = body(vec![block(
        0,
        vec![
            statement(SemanticStatementKindV1::StorageLive(
                SemanticLocalIdV1::from_index(1),
            )),
            assign(1, None),
            assign(2, Some(SemanticOperandV1::Copy(place(1)))),
            assign(3, Some(SemanticOperandV1::Copy(place(2)))),
            statement(SemanticStatementKindV1::StorageDead(
                SemanticLocalIdV1::from_index(1),
            )),
        ],
        None,
    )]);
    let plan = plan(&body);
    let mut graph = CapabilitySsaGraphV1::new(&body, plan.plan(), 1000).unwrap();
    let owner = graph.use_value(0, 1).unwrap();
    graph
        .loan_live(
            loan(owner, 2),
            CapabilityDefinitionSiteV1 {
                block: 0,
                statement: Some(3),
                local: 3,
            },
        )
        .unwrap();
    assert!(
        graph
            .loan_live(
                loan(owner, 2),
                CapabilityDefinitionSiteV1 {
                    block: 0,
                    statement: None,
                    local: 0
                }
            )
            .is_err()
    );
}

#[test]
fn capability_graph_rejects_owner_move_overwrite_and_storage_death() {
    for invalidation in [
        assign(3, Some(SemanticOperandV1::Move(place(1)))),
        assign(1, None),
        statement(SemanticStatementKindV1::StorageDead(
            SemanticLocalIdV1::from_index(1),
        )),
    ] {
        let body = body(vec![
            block(
                0,
                vec![
                    assign(1, None),
                    assign(2, Some(SemanticOperandV1::Copy(place(1)))),
                    invalidation,
                ],
                Some(1),
            ),
            block(
                1,
                vec![assign(3, Some(SemanticOperandV1::Copy(place(2))))],
                None,
            ),
        ]);
        let plan = plan(&body);
        let mut graph = CapabilitySsaGraphV1::new(&body, plan.plan(), 1000).unwrap();
        let owner = graph.use_value(0, 1).unwrap();
        assert!(
            graph
                .loan_live(
                    loan(owner, 1),
                    CapabilityDefinitionSiteV1 {
                        block: 1,
                        statement: Some(0),
                        local: 3
                    }
                )
                .is_err()
        );
    }
    assert!(invalidates(
        &SemanticStatementKindV1::Deinitialize(place(1)),
        1
    ));
}

#[test]
fn capability_graph_rejects_backedge_loan_without_storage_generation_proof() {
    let body = body(vec![
        block(
            0,
            vec![
                assign(1, None),
                assign(2, Some(SemanticOperandV1::Copy(place(1)))),
            ],
            Some(1),
        ),
        block(
            1,
            vec![assign(3, Some(SemanticOperandV1::Copy(place(2))))],
            Some(1),
        ),
    ]);
    let plan = plan(&body);
    let mut graph = CapabilitySsaGraphV1::new(&body, plan.plan(), 1000).unwrap();
    let owner = graph.use_value(0, 1).unwrap();
    assert!(
        graph
            .loan_live(
                loan(owner, 1),
                CapabilityDefinitionSiteV1 {
                    block: 1,
                    statement: Some(0),
                    local: 3
                }
            )
            .is_err()
    );
}

#[test]
fn capability_graph_rejects_multiple_use_versions_and_enforces_work_bound() {
    let body = body(vec![block(
        0,
        vec![
            assign(1, None),
            assign(2, Some(SemanticOperandV1::Copy(place(1)))),
            assign(1, None),
            assign(3, Some(SemanticOperandV1::Copy(place(1)))),
        ],
        None,
    )]);
    let plan = plan(&body);
    assert!(CapabilitySsaGraphV1::new(&body, plan.plan(), 4).is_err());
    let mut graph = CapabilitySsaGraphV1::new(&body, plan.plan(), 1000).unwrap();
    assert!(graph.use_value(0, 1).is_err());
    assert!(matches!(
        graph.charge(1001),
        Err(ProductionSemanticKirErrorV1::ResourceLimit {
            resource: ProductionSemanticKirResourceV1::AnalysisWork,
            ..
        })
    ));
}

fn graph_body(edges: &[Vec<u32>]) -> SemanticFunctionDeclV1 {
    body(
        edges
            .iter()
            .enumerate()
            .map(|(index, successors)| {
                let mut identity = [91; 32];
                identity[..4].copy_from_slice(&(index as u32).to_le_bytes());
                let terminator = match successors.as_slice() {
                    [] => SemanticTerminatorKindV1::Return,
                    [target] => SemanticTerminatorKindV1::Goto(SemanticControlFlowEdgeV1::new(
                        SemanticEdgeRoleV1::Goto,
                        SemanticBlockIdV1::from_index(*target),
                    )),
                    [targets @ .., otherwise] => SemanticTerminatorKindV1::SwitchInt {
                        discriminant: SemanticOperandV1::Constant(SemanticConstantV1::new(
                            SemanticTypeIdV1::from_index(1),
                            SemanticConstantValueV1::Scalar(
                                SemanticScalarValueV1::new(0, 4).unwrap(),
                            ),
                        )),
                        targets: SemanticSwitchTargetsV1::new(
                            targets
                                .iter()
                                .enumerate()
                                .map(|(value, target)| {
                                    SemanticSwitchTargetV1::new(
                                        value as u128,
                                        SemanticControlFlowEdgeV1::new(
                                            SemanticEdgeRoleV1::SwitchValue,
                                            SemanticBlockIdV1::from_index(*target),
                                        ),
                                    )
                                })
                                .collect(),
                            SemanticControlFlowEdgeV1::new(
                                SemanticEdgeRoleV1::SwitchOtherwise,
                                SemanticBlockIdV1::from_index(*otherwise),
                            ),
                        )
                        .unwrap(),
                    },
                };
                SemanticBasicBlockV1::new(
                    SemanticBlockIdentityV1::from_sha256(identity),
                    SemanticSourceProvenanceV1::unavailable(),
                    vec![],
                    SemanticTerminatorV1::new(
                        SemanticSourceProvenanceV1::unavailable(),
                        terminator,
                    ),
                )
                .unwrap()
            })
            .collect(),
    )
}

#[test]
fn capability_path_region_matches_pairwise_reachability_on_seeded_graphs() {
    let mut seed = 0x59c3_04ad_u32;
    for _ in 0..64 {
        let mut next = || {
            seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            seed >> 16
        };
        let edges: Vec<Vec<u32>> = (0..8)
            .map(|_| (0..next() % 4).map(|_| next() % 8).collect())
            .collect();
        let body = graph_body(&edges);
        let plan = plan(&body);
        for from in 0..8 {
            for to in 0..8 {
                let mut graph = CapabilitySsaGraphV1::new(&body, plan.plan(), 10_000).unwrap();
                let region = graph.path_region(from, to).unwrap();
                let mut reference = CapabilitySsaGraphV1::new(&body, plan.plan(), 10_000).unwrap();
                for block in 0..8 {
                    assert_eq!(
                        region[block as usize],
                        reference.reaches(from, block).unwrap()
                            && reference.reaches(block, to).unwrap(),
                        "edges={edges:?}, from={from}, to={to}, block={block}"
                    );
                }
            }
        }
    }
}

#[test]
fn capability_path_region_keeps_cycles_and_excludes_dead_end_branches() {
    let body = graph_body(&[vec![1, 4], vec![2], vec![1, 3], vec![], vec![4], vec![3]]);
    let plan = plan(&body);
    let mut graph = CapabilitySsaGraphV1::new(&body, plan.plan(), 1000).unwrap();
    assert_eq!(
        graph.path_region(0, 3).unwrap(),
        vec![true, true, true, true, false, false]
    );
    assert_eq!(graph.path_region(5, 3).unwrap(), vec![false; 6]);
    assert!(matches!(
        graph.path_region(0, 6),
        Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch)
    ));
}

#[test]
fn capability_path_region_enforces_its_exact_work_boundary() {
    let body = graph_body(&[vec![1, 2], vec![3], vec![3], vec![]]);
    let plan = plan(&body);
    let mut probe = CapabilitySsaGraphV1::new(&body, plan.plan(), 1000).unwrap();
    let expected = probe.path_region(0, 3).unwrap();
    let used = probe.limit - probe.remaining;
    let mut exact = CapabilitySsaGraphV1::new(&body, plan.plan(), used).unwrap();
    assert_eq!(exact.path_region(0, 3).unwrap(), expected);
    assert_eq!(exact.remaining, 0);
    let mut insufficient = CapabilitySsaGraphV1::new(&body, plan.plan(), used - 1).unwrap();
    assert!(
        matches!(insufficient.path_region(0, 3), Err(ProductionSemanticKirErrorV1::ResourceLimit {
        resource: ProductionSemanticKirResourceV1::AnalysisWork, actual, limit,
    }) if actual == used && limit == used - 1)
    );
}

#[test]
fn capability_path_region_avoids_quadratic_context_prefix_walks() {
    let count = 1536;
    let edges: Vec<_> = (0..count)
        .map(|block| {
            if block + 1 < count {
                vec![block + 1]
            } else {
                vec![]
            }
        })
        .collect();
    let body = graph_body(&edges);
    let plan = plan(&body);
    let mut graph = CapabilitySsaGraphV1::new(&body, plan.plan(), 20_000).unwrap();
    let region = graph.path_region(0, 1).unwrap();
    assert_eq!(&region[..2], &[true, true]);
    assert!(region[2..].iter().all(|inside| !inside));
    let mut repeated = CapabilitySsaGraphV1::new(&body, plan.plan(), 20_000).unwrap();
    let result: Result<(), ProductionSemanticKirErrorV1> = (0..count).try_for_each(|block| {
        repeated.reaches(0, block)?;
        repeated.reaches(block, 1)?;
        Ok(())
    });
    assert!(matches!(
        result,
        Err(ProductionSemanticKirErrorV1::ResourceLimit {
            resource: ProductionSemanticKirResourceV1::AnalysisWork,
            ..
        })
    ));
}
