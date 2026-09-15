//! Rejection-only component tests. Cyclic type tables exercise traversal, not
//! semantic admission; CFG cases use the real SSA planner and shared Graph.
use super::*;
use fe2o3_mir_model::semantic_mir_v1::*;
use fe2o3_pliron::{ProductionSemanticSsaLimitsV1, plan_semantic_function_ssa_v1};

#[path = "old_epoch/inventory_tests.rs"]
mod inventory_tests;

#[path = "old_epoch/reverse_type_tests.rs"]
mod reverse_type_tests;

fn id(index: u32) -> SemanticTypeIdV1 {
    SemanticTypeIdV1::from_index(index)
}
fn fields(values: &[u32]) -> SemanticAggregateTypeV1 {
    SemanticAggregateTypeV1::new(values.iter().copied().map(id).collect()).unwrap()
}
fn declarations() -> Vec<SemanticTypeDeclV1> {
    let shapes = [
        SemanticTypeShapeV1::Unit,
        SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
            signed: false,
            bits: 32,
        }),
        SemanticTypeShapeV1::Aggregate(fields(&[])), // Actual old type seed in these component cases.
        SemanticTypeShapeV1::Tuple(fields(&[2])),
        SemanticTypeShapeV1::Pointer(
            SemanticPointerTypeV1::new_with_kind(
                id(3),
                SemanticPointerKindV1::Reference,
                SemanticMutabilityV1::Immutable,
                0,
                64,
                SemanticPointerMetadataV1::None,
            )
            .unwrap(),
        ),
        SemanticTypeShapeV1::Array {
            element: id(3),
            length: 4,
        },
        SemanticTypeShapeV1::Tuple(fields(&[7, 2])),
        SemanticTypeShapeV1::Tuple(fields(&[6])),
        SemanticTypeShapeV1::Tuple(fields(&[8])),
        SemanticTypeShapeV1::FunctionPointer {
            safety: SemanticFunctionSafetyV1::Safe,
            extern_abi: SemanticExternAbiV1::Rust,
            c_variadic: false,
            arguments: fields(&[3]),
            return_type: id(0),
        },
        SemanticTypeShapeV1::Union(fields(&[1, 3])),
        SemanticTypeShapeV1::Slice { element: id(4) },
    ];
    shapes
        .into_iter()
        .enumerate()
        .map(|(index, shape)| {
            SemanticTypeDeclV1::new(
                SemanticTypeIdentityV1::from_sha256([index as u8 + 1; 32]),
                SemanticLayoutIdentityV1::from_sha256([index as u8 + 1; 32]),
                SemanticTypeLayoutV1::new(
                    Some(if index == 1 { 4 } else { 0 }),
                    if index == 1 { 4 } else { 1 },
                )
                .unwrap(),
                shape,
            )
        })
        .collect()
}

#[test]
fn old_epoch_type_closure_follows_exact_nested_edges_and_scc_peers() {
    let declarations = declarations();
    let mut types = Types {
        types: &declarations,
        seeds: &[id(2)],
        complete: BTreeMap::new(),
    };
    for index in [2, 3, 4, 5, 6, 7, 9, 10, 11] {
        assert!(
            types.contains(id(index), &mut |_| Ok(())).unwrap(),
            "type {index}"
        );
    }
    // In particular querying 6 must not cache its SCC peer 7 as false.
    assert_eq!(types.complete.get(&id(7)), Some(&true));
    for index in [0, 1, 8] {
        assert!(!types.contains(id(index), &mut |_| Ok(())).unwrap());
    }
}

#[test]
fn failed_type_query_never_publishes_a_partial_cache_entry() {
    let declarations = declarations();
    let mut work = 0;
    let mut types = Types {
        types: &declarations,
        seeds: &[id(2)],
        complete: BTreeMap::new(),
    };
    assert!(
        types
            .contains(id(7), &mut |n| {
                work += n;
                Ok(())
            })
            .unwrap()
    );
    assert!(work > 0);
    for limit in 0..work {
        let mut types = Types {
            types: &declarations,
            seeds: &[id(2)],
            complete: BTreeMap::new(),
        };
        let mut remaining = limit;
        let error = types
            .contains(id(7), &mut |n| {
                remaining = remaining.checked_sub(n).ok_or(
                    ProductionSemanticKirErrorV1::ResourceLimit {
                        resource: ProductionSemanticKirResourceV1::AnalysisWork,
                        limit,
                        actual: limit + 1,
                    },
                )?;
                Ok(())
            })
            .unwrap_err();
        assert!(
            matches!(error, ProductionSemanticKirErrorV1::ResourceLimit {
            resource: ProductionSemanticKirResourceV1::AnalysisWork, actual, limit: found,
        } if actual == limit + 1 && found == limit)
        );
        assert!(types.complete.is_empty(), "failed query at budget {limit}");
    }
}

fn place(local: u32, ty: u32) -> SemanticPlaceV1 {
    SemanticPlaceV1::new(SemanticLocalIdV1::from_index(local), vec![], id(ty)).unwrap()
}
fn constant(ty: u32) -> SemanticOperandV1 {
    SemanticOperandV1::Constant(SemanticConstantV1::new(
        id(ty),
        if ty == 1 {
            SemanticConstantValueV1::Scalar(SemanticScalarValueV1::new(0, 4).unwrap())
        } else {
            SemanticConstantValueV1::ZeroSized
        },
    ))
}
fn statement(kind: SemanticStatementKindV1) -> SemanticStatementV1 {
    SemanticStatementV1::new(SemanticSourceProvenanceV1::unavailable(), kind)
}
fn assign(local: u32, ty: u32) -> SemanticStatementV1 {
    statement(SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
        place(local, ty),
        SemanticRvalueV1::new(id(ty), SemanticRvalueKindV1::Use(constant(ty))),
    )))
}
fn block(
    index: u8,
    statements: Vec<SemanticStatementV1>,
    next: Option<u32>,
) -> SemanticBasicBlockV1 {
    SemanticBasicBlockV1::new(
        SemanticBlockIdentityV1::from_sha256([index + 20; 32]),
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
    let source = SemanticSourceProvenanceV1::unavailable();
    let abi = SemanticFunctionAbiV1::from_rustc(
        SemanticAbiIdentityV1::from_sha256([40; 32]),
        SemanticLayoutIdentityV1::from_sha256([41; 32]),
        SemanticCanonAbiV1::Rust,
        SemanticExternAbiV1::Rust,
        false,
        false,
        0,
        vec![],
        SemanticAbiValueV1::new(id(0), SemanticAbiPassModeV1::Ignore),
    )
    .unwrap();
    SemanticFunctionDeclV1::new(
        SemanticFunctionIdentityV1::from_sha256([42; 32]),
        SemanticFunctionRoleV1::InternalHelper,
        SemanticItemDefinitionIdentityV1::from_sha256([43; 32]),
        SemanticMonomorphizationIdentityV1::from_sha256([44; 32]),
        SemanticGenericTypeArgumentsIdentityV1::from_sha256([45; 32]),
        SemanticConstGenericArgumentsIdentityV1::from_sha256([46; 32]),
        source,
        abi,
        (0..=2u8)
            .map(|index| {
                SemanticLocalDeclV1::new(
                    SemanticLocalIdentityV1::from_sha256([50 + index; 32]),
                    id(index.into()),
                    if index == 0 {
                        SemanticLocalRoleV1::Return
                    } else {
                        SemanticLocalRoleV1::Temporary
                    },
                    source,
                )
            })
            .collect(),
        SemanticBlockIdV1::from_index(0),
        blocks,
    )
    .unwrap()
}
fn scan(body: &SemanticFunctionDeclV1, after: u32) -> Option<(u32, Option<u32>)> {
    let plan = plan_semantic_function_ssa_v1(
        SemanticFunctionIdV1::from_index(0),
        body,
        ProductionSemanticSsaLimitsV1::default(),
    )
    .unwrap();
    let mut graph = Graph::new(body, plan.plan(), 100_000).unwrap();
    first_use(&declarations(), &mut graph, after, &[id(2)]).unwrap()
}

#[test]
fn old_epoch_constant_after_publish_is_rejected_but_scalar_and_storage_end_are_not_uses() {
    let good = body(vec![
        block(0, vec![assign(2, 2)], Some(1)),
        block(
            1,
            vec![
                assign(1, 1),
                statement(SemanticStatementKindV1::StorageDead(
                    SemanticLocalIdV1::from_index(2),
                )),
            ],
            None,
        ),
    ]);
    assert_eq!(scan(&good, 1), None);
    let changed = body(vec![
        block(0, vec![assign(2, 2)], Some(1)),
        block(1, vec![assign(2, 2)], None),
    ]);
    assert_eq!(scan(&changed, 1), Some((1, Some(0))));
}

#[test]
fn old_epoch_descendant_check_keeps_nonempty_backedges_and_excludes_dead_paths() {
    let dead = body(vec![
        block(0, vec![assign(2, 2)], Some(1)),
        block(1, vec![], None),
        block(2, vec![assign(2, 2)], Some(0)),
    ]);
    assert_eq!(scan(&dead, 1), None);
    let looped = body(vec![
        block(0, vec![assign(2, 2)], Some(1)),
        block(1, vec![], Some(0)),
    ]);
    assert_eq!(scan(&looped, 1), Some((0, Some(0))));
}

#[test]
fn old_epoch_call_drop_and_assert_message_operands_are_not_omitted() {
    let declarations = declarations();
    let body = body(vec![block(0, vec![], None)]);
    let mut types = Types {
        types: &declarations,
        seeds: &[id(2)],
        complete: BTreeMap::new(),
    };
    let edge = SemanticControlFlowEdgeV1::new(
        SemanticEdgeRoleV1::DropReturn,
        SemanticBlockIdV1::from_index(0),
    );
    let drop = SemanticTerminatorKindV1::Drop {
        place: place(2, 2),
        drop_glue: SemanticFunctionIdV1::from_index(0),
        target: edge,
        unwind: SemanticUnwindActionV1::Unreachable,
    };
    assert!(types.terminator(&body, &drop, &mut |_| Ok(())).unwrap());
    // Direct operand helper is the same exhaustive path used for both Call and TailCall.
    assert!(
        types
            .operands(&body, &[constant(1), constant(2)], &mut |_| Ok(()))
            .unwrap()
    );
    assert!(
        !types
            .operands(&body, &[constant(1)], &mut |_| Ok(()))
            .unwrap()
    );
    let assertion = SemanticTerminatorKindV1::Assert {
        condition: constant(1),
        expected: true,
        message: SemanticAssertMessageV1::BoundsCheck {
            length: constant(1),
            index: constant(2),
        },
        target: SemanticControlFlowEdgeV1::new(
            SemanticEdgeRoleV1::AssertSuccess,
            SemanticBlockIdV1::from_index(0),
        ),
        unwind: SemanticUnwindActionV1::Unreachable,
    };
    assert!(
        types
            .terminator(&body, &assertion, &mut |_| Ok(()))
            .unwrap()
    );
}

#[test]
fn projected_scalar_copy_and_move_keep_the_old_carrier_dependency() {
    let mut declarations = declarations();
    declarations[2] = SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256([3; 32]),
        SemanticLayoutIdentityV1::from_sha256([3; 32]),
        SemanticTypeLayoutV1::aggregate(
            Some(4),
            4,
            SemanticAggregateLayoutV1::new(vec![0], vec![]).unwrap(),
        )
        .unwrap(),
        SemanticTypeShapeV1::Aggregate(fields(&[1])),
    );
    let body = body(vec![block(0, vec![], None)]);
    let mut types = Types {
        types: &declarations,
        seeds: &[id(2)],
        complete: BTreeMap::new(),
    };
    let projected = SemanticPlaceV1::new(
        SemanticLocalIdV1::from_index(2),
        vec![SemanticProjectionV1::new(SemanticProjectionKindV1::Field(0), id(1)).unwrap()],
        id(1),
    )
    .unwrap();
    assert!(
        types
            .operand(
                &body,
                &SemanticOperandV1::Copy(projected.clone()),
                &mut |_| Ok(())
            )
            .unwrap()
    );
    assert!(
        types
            .operand(&body, &SemanticOperandV1::Move(projected), &mut |_| Ok(()))
            .unwrap()
    );
    assert!(
        !types
            .operand(
                &body,
                &SemanticOperandV1::Copy(place(1, 1)),
                &mut |_| Ok(())
            )
            .unwrap()
    );
}

#[test]
fn opaque_carrier_is_not_assumed_free_of_epoch_descendants() {
    let mut declarations = declarations();
    declarations[8] = SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256([9; 32]),
        SemanticLayoutIdentityV1::from_sha256([9; 32]),
        SemanticTypeLayoutV1::new(Some(0), 1).unwrap(),
        SemanticTypeShapeV1::Opaque,
    );
    let mut types = Types {
        types: &declarations,
        seeds: &[id(2)],
        complete: BTreeMap::new(),
    };
    assert!(types.contains(id(8), &mut |_| Ok(())).unwrap());
    assert!(!types.contains(id(1), &mut |_| Ok(())).unwrap());
}
