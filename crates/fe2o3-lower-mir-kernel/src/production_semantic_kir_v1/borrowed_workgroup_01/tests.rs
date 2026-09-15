//! Tests of the source-SSA matcher, not fabricated Workgroup issuance.
//! The real importer/lowering callback remains a separate integration gate.

use super::*;
use fe2o3_mir_model::semantic_mir_v1::*;
use fe2o3_pliron::plan_semantic_function_ssa_v1;

#[path = "allocation_tests.rs"]
mod allocation_tests;
#[path = "transpose_publish_tests.rs"]
mod transpose_publish_tests;

fn place(local: u32) -> SemanticPlaceV1 {
    SemanticPlaceV1::new(
        SemanticLocalIdV1::from_index(local),
        vec![],
        SemanticTypeIdV1::from_index(1),
    )
    .unwrap()
}

fn assign(local: u32, source: Option<u32>) -> SemanticStatementV1 {
    let operand = source
        .map(|source| SemanticOperandV1::Copy(place(source)))
        .unwrap_or_else(|| {
            SemanticOperandV1::Constant(SemanticConstantV1::new(
                SemanticTypeIdV1::from_index(1),
                SemanticConstantValueV1::Scalar(
                    SemanticScalarValueV1::new(u128::from(local), 4).unwrap(),
                ),
            ))
        });
    SemanticStatementV1::new(
        SemanticSourceProvenanceV1::unavailable(),
        SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
            place(local),
            SemanticRvalueV1::new(
                SemanticTypeIdV1::from_index(1),
                SemanticRvalueKindV1::Use(operand),
            ),
        )),
    )
}

fn block(
    index: u8,
    statements: Vec<SemanticStatementV1>,
    next: Option<u32>,
) -> SemanticBasicBlockV1 {
    SemanticBasicBlockV1::new(
        SemanticBlockIdentityV1::from_sha256([index + 60; 32]),
        SemanticSourceProvenanceV1::unavailable(),
        statements,
        SemanticTerminatorV1::new(
            SemanticSourceProvenanceV1::unavailable(),
            next.map(|target| {
                SemanticTerminatorKindV1::Goto(SemanticControlFlowEdgeV1::new(
                    SemanticEdgeRoleV1::Goto,
                    SemanticBlockIdV1::from_index(target),
                ))
            })
            .unwrap_or(SemanticTerminatorKindV1::Return),
        ),
    )
    .unwrap()
}

fn body(blocks: Vec<SemanticBasicBlockV1>) -> SemanticFunctionDeclV1 {
    let original = super::super::resource_tests::noop_semantic_owner(&["borrow_graph_scaffold"]);
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

#[test]
fn distinct_same_typed_source_definitions_remain_distinct_ssa() {
    let body = body(vec![block(
        0,
        vec![
            assign(1, None),
            assign(2, None),
            assign(3, Some(1)),
            assign(3, Some(2)),
        ],
        None,
    )]);
    let plan = plan(&body);
    let mut graph = Graph::new(&body, plan.plan(), 1000).unwrap();
    let first = graph.use_value(0, 1).unwrap();
    let second = graph.use_value(0, 2).unwrap();
    assert_ne!(first, second);
    assert_eq!(
        graph.definition(first).unwrap(),
        Site {
            block: 0,
            statement: Some(0),
            local: 1
        }
    );
    assert_eq!(
        graph.definition(second).unwrap(),
        Site {
            block: 0,
            statement: Some(1),
            local: 2
        }
    );
}

#[test]
fn different_versions_of_one_local_do_not_collapse() {
    let body = body(vec![block(
        0,
        vec![
            assign(1, None),
            assign(3, Some(1)),
            assign(1, None),
            assign(3, Some(1)),
        ],
        None,
    )]);
    let plan = plan(&body);
    let mut graph = Graph::new(&body, plan.plan(), 1000).unwrap();
    assert!(graph.use_value(0, 1).is_err());
}

#[test]
fn absent_ssa_use_and_unreachable_use_fail_closed() {
    let body = body(vec![
        block(0, vec![assign(1, None)], None),
        block(1, vec![assign(2, None), assign(3, Some(2))], None),
    ]);
    let plan = plan(&body);
    let mut graph = Graph::new(&body, plan.plan(), 1000).unwrap();
    assert!(graph.use_value(0, 1).is_err());
    assert!(graph.use_value(1, 2).is_err());
}

#[test]
fn retained_storage_kill_rejects_even_without_a_later_owned_use() {
    for kill in [false, true] {
        let middle = if kill {
            vec![SemanticStatementV1::new(
                SemanticSourceProvenanceV1::unavailable(),
                SemanticStatementKindV1::StorageDead(SemanticLocalIdV1::from_index(1)),
            )]
        } else {
            vec![]
        };
        let body = body(vec![
            block(0, vec![assign(1, None)], Some(1)),
            block(1, vec![assign(3, Some(1))], Some(2)),
            block(2, middle, Some(3)),
            block(3, vec![], None),
        ]);
        let plan = plan(&body);
        let mut graph = Graph::new(&body, plan.plan(), 10000).unwrap();
        let referent = graph.use_value(1, 1).unwrap();
        // This private matcher input tests lifetime rejection, not authentication.
        let loan = Loan {
            borrow: Site {
                block: 1,
                statement: Some(0),
                local: 3,
            },
            owner_value: referent,
            owner_local: 1,
        };
        assert_eq!(graph.loan_live(loan, Site { block: 3, statement: None, local: 0 }).is_ok(), !kill);
    }
}

#[test]
fn referent_overwrite_rejects_even_when_the_old_ssa_value_still_exists() {
    let body = body(vec![
        block(0, vec![assign(1, None)], Some(1)),
        block(1, vec![assign(3, Some(1))], Some(2)),
        block(2, vec![assign(1, None)], Some(3)),
        block(3, vec![], None),
    ]);
    let plan = plan(&body);
    let mut graph = Graph::new(&body, plan.plan(), 10000).unwrap();
    let referent = graph.use_value(1, 1).unwrap();
    assert!(
        graph
            .loan_live(
                Loan {
                    borrow: Site {
                        block: 1,
                        statement: Some(0),
                        local: 3
                    },
                    owner_value: referent,
                    owner_local: 1
                },
                Site { block: 3, statement: None, local: 0 }
            )
            .is_err()
    );
}

#[test]
fn graph_work_budget_has_an_exact_boundary() {
    let body = body(vec![block(0, vec![], None)]);
    let plan = plan(&body);
    // Observe this tiny fixture's exact cost without duplicating the private
    // cache-header layout or index-insertion accounting in another module.
    let first_budget = |include_reach: bool| {
        for budget in 0..=256 {
            let result = Graph::new(&body, plan.plan(), budget).and_then(|mut graph| {
                if include_reach {
                    assert!(graph.reaches(0, 0)?);
                }
                Ok(())
            });
            match result {
                Ok(()) => return budget,
                Err(ProductionSemanticKirErrorV1::ResourceLimit {
                    resource: ProductionSemanticKirResourceV1::AnalysisWork,
                    actual,
                    limit,
                }) => {
                    assert_eq!(limit, budget);
                    assert_eq!(actual, budget + 1);
                }
                Err(error) => panic!("unexpected graph boundary error: {error:?}"),
            }
        }
        panic!("tiny graph fixture exceeds its bounded test probe");
    };
    let minimum = first_budget(false);
    assert!(minimum > body.blocks().len() + body.locals().len(),
        "the new cache headers must be charged");
    assert!(matches!(
        Graph::new(&body, plan.plan(), minimum - 1),
        Err(ProductionSemanticKirErrorV1::ResourceLimit {
            resource: ProductionSemanticKirResourceV1::AnalysisWork,
            actual,
            limit,
        }) if limit == minimum - 1 && actual == minimum
    ));
    let mut graph = Graph::new(&body, plan.plan(), minimum).unwrap();
    graph.charge(0).unwrap();
    assert!(matches!(
        graph.reaches(0, 0),
        Err(ProductionSemanticKirErrorV1::ResourceLimit {
            resource: ProductionSemanticKirResourceV1::AnalysisWork,
            actual,
            limit,
        }) if limit == minimum && actual == minimum + 1
    ));
    let with_reach = first_budget(true);
    assert!(with_reach > minimum, "cold lookup, visit and cache insertion cost work");
    let mut graph = Graph::new(&body, plan.plan(), with_reach - 1).unwrap();
    assert!(matches!(
        graph.reaches(0, 0),
        Err(ProductionSemanticKirErrorV1::ResourceLimit {
            resource: ProductionSemanticKirResourceV1::AnalysisWork,
            actual,
            limit,
        }) if limit == with_reach - 1 && actual == with_reach
    ));
    let mut graph = Graph::new(&body, plan.plan(), with_reach).unwrap();
    assert!(graph.reaches(0, 0).unwrap());
    graph.charge(0).unwrap();
    assert!(matches!(
        graph.charge(1),
        Err(ProductionSemanticKirErrorV1::ResourceLimit {
            resource: ProductionSemanticKirResourceV1::AnalysisWork,
            actual,
            limit,
        }) if limit == with_reach && actual == with_reach + 1
    ));
}

#[test]
fn a_merge_preserves_both_incoming_definition_identities() {
    let branch = SemanticBasicBlockV1::new(
        SemanticBlockIdentityV1::from_sha256([90; 32]),
        SemanticSourceProvenanceV1::unavailable(),
        vec![assign(2, None)],
        SemanticTerminatorV1::new(
            SemanticSourceProvenanceV1::unavailable(),
            SemanticTerminatorKindV1::SwitchInt {
                discriminant: SemanticOperandV1::Copy(place(2)),
                targets: SemanticSwitchTargetsV1::new(
                    vec![SemanticSwitchTargetV1::new(
                        0,
                        SemanticControlFlowEdgeV1::new(
                            SemanticEdgeRoleV1::SwitchValue,
                            SemanticBlockIdV1::from_index(1),
                        ),
                    )],
                    SemanticControlFlowEdgeV1::new(
                        SemanticEdgeRoleV1::SwitchOtherwise,
                        SemanticBlockIdV1::from_index(2),
                    ),
                )
                .unwrap(),
            },
        ),
    )
    .unwrap();
    let body = body(vec![
        branch,
        block(1, vec![assign(1, None)], Some(3)),
        block(2, vec![assign(1, None)], Some(3)),
        block(3, vec![assign(3, Some(1))], None),
    ]);
    let plan = plan(&body);
    let mut graph = Graph::new(&body, plan.plan(), 10000).unwrap();
    assert!(matches!(
        graph.use_value(3, 1).unwrap(),
        SsaValueV1::BlockArgument { .. }
    ));
    let incoming = graph.incoming(3, 1).unwrap();
    assert_eq!(incoming.len(), 2);
    assert_ne!(incoming[0], incoming[1]);
    assert_ne!(
        graph.definition(incoming[0]).unwrap().block,
        graph.definition(incoming[1]).unwrap().block
    );
}

#[test]
fn only_the_exact_immutable_reference_edge_matches() {
    let owned = SemanticTypeIdV1::from_index(0);
    let reference = SemanticTypeIdV1::from_index(1);
    for (kind, mutability, pointee, address_space, width, accepted) in [
        (
            SemanticPointerKindV1::Reference,
            SemanticMutabilityV1::Immutable,
            owned,
            0,
            64,
            true,
        ),
        (
            SemanticPointerKindV1::Reference,
            SemanticMutabilityV1::Mutable,
            owned,
            0,
            64,
            false,
        ),
        (
            SemanticPointerKindV1::Raw,
            SemanticMutabilityV1::Immutable,
            owned,
            0,
            64,
            false,
        ),
        (
            SemanticPointerKindV1::Reference,
            SemanticMutabilityV1::Immutable,
            reference,
            0,
            64,
            false,
        ),
        (
            SemanticPointerKindV1::Reference,
            SemanticMutabilityV1::Immutable,
            owned,
            1,
            64,
            false,
        ),
        (
            SemanticPointerKindV1::Reference,
            SemanticMutabilityV1::Immutable,
            owned,
            0,
            32,
            false,
        ),
    ] {
        let declaration = |tag, shape| {
            SemanticTypeDeclV1::new(
                SemanticTypeIdentityV1::from_sha256([tag; 32]),
                SemanticLayoutIdentityV1::from_sha256([tag; 32]),
                SemanticTypeLayoutV1::new(Some(8), 8).unwrap(),
                shape,
            )
        };
        let types = vec![
            declaration(
                1,
                SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
                    signed: false,
                    bits: 64,
                }),
            ),
            declaration(
                2,
                SemanticTypeShapeV1::Pointer(
                    SemanticPointerTypeV1::new_with_kind(
                        pointee,
                        kind,
                        mutability,
                        address_space,
                        width,
                        SemanticPointerMetadataV1::None,
                    )
                    .unwrap(),
                ),
            ),
        ];
        assert_eq!(shared_type(&types, reference, owned), accepted);
        assert!(!shared_type(&types, owned, owned));
    }
}

#[test]
fn legacy_subgroup_signature_cannot_preserve_borrowed_source_identity() {
    let id = |n| ExecutionTypeIdentityV1::new([n; 32]);
    let operation = ExecutionCapabilityOperationV1::SubgroupDerive {
        workgroup: id(1),
        subgroup: id(2),
        width: 64,
    };
    let owned = ExecutionCapabilitySignatureV1::new(&[id(1)], id(2)).unwrap();
    let borrowed = ExecutionCapabilitySignatureV1::new(&[id(3)], id(2)).unwrap();
    assert!(operation.signature_matches(owned));
    assert!(!operation.signature_matches(borrowed));
}
