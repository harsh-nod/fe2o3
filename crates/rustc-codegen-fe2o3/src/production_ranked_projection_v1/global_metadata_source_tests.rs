// Include beside typed_global_source_tests.rs in the existing projection tests.
#[test]
fn typed_global_metadata_snapshot_retains_exact_custody_with_unrelated_assertion() {
    let (types, callables, function) = typed_global_exclusive_metadata_snapshot_fixture_v1();
    let mut blocks = function.blocks().to_vec();
    let original = blocks[9].clone();
    let continuation = blocks.len() as u32;
    blocks.push(
        SemanticBasicBlockV1::new(
            SemanticBlockIdentityV1::from_sha256(bytes(249)),
            original.source(),
            vec![],
            original.terminator().clone(),
        )
        .unwrap(),
    );
    blocks[9] = SemanticBasicBlockV1::new(
        original.identity(),
        original.source(),
        original.statements().to_vec(),
        SemanticTerminatorV1::new(
            original.source(),
            SemanticTerminatorKindV1::Assert {
                condition: SemanticOperandV1::Constant(SemanticConstantV1::new(
                    BOOL_TYPE,
                    SemanticConstantValueV1::Scalar(SemanticScalarValueV1::new(1, 1).unwrap()),
                )),
                expected: true,
                message: fe2o3_mir_model::semantic_mir_v1::SemanticAssertMessageV1::BoundsCheck {
                    length: SemanticOperandV1::Constant(SemanticConstantV1::new(
                        U64_TYPE,
                        SemanticConstantValueV1::Scalar(SemanticScalarValueV1::new(4, 8).unwrap()),
                    )),
                    index: SemanticOperandV1::Constant(SemanticConstantV1::new(
                        U64_TYPE,
                        SemanticConstantValueV1::Scalar(SemanticScalarValueV1::new(0, 8).unwrap()),
                    )),
                },
                target: cfg_edge(SemanticEdgeRoleV1::AssertSuccess, continuation),
                unwind: SemanticUnwindActionV1::Unreachable,
            },
        ),
    )
    .unwrap();
    let function = typed_global_fixture_with_body_v1(&function, function.locals().to_vec(), blocks);
    let (projection, _) = project_capability_index_fixture(&types, &callables, &function).unwrap();
    let view = projection.global_views[6].unwrap();
    let origin = Some(ProjectedCapabilityValueV1::Known(
        ProjectedCapabilityOriginV1::GlobalPhysical {
            view,
            ty: function.locals()[31].ty(),
            borrowed: false,
        },
    ));
    assert!(global_mutable_metadata_snapshot_v1(&types, &function, 9, 3, origin, &mut 0).unwrap());
    assert!(!global_mutable_metadata_snapshot_v1(&types, &function, 9, 3, None, &mut 0).unwrap());
    audit_typed_global_statement_v1(
        &types,
        &function,
        &projection,
        9,
        &function.blocks()[9].statements()[3],
    )
    .unwrap();
    let read = projection.direct_read_effects[6].as_ref().unwrap();
    let write = projection.direct_write_effects[11].as_ref().unwrap();
    assert_eq!(read.indices, write.indices);
    assert_eq!(read.comparisons.len(), 1);
    assert_eq!(write.comparisons.len(), 1);
    assert_ne!(read.comparisons[0].1, write.comparisons[0].1);
    // No raw slice/allocation is minted by the use audit. These are the two
    // pre-existing independently authenticated physical ABI allocations.
    assert_ne!(
        projection.global_views[6].unwrap().allocation,
        projection.global_views[11].unwrap().allocation
    );
}
