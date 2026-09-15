mod edge_rejections77 {
    use super::*;
    use fe2o3_mir_model::semantic_mir_v1::{
        SemanticCallDestinationV1, SemanticDirectCallV1, SemanticDirectTailCallV1,
        SemanticEdgeRoleV1, SemanticFunctionDeclV1, SemanticFunctionIdV1, SemanticLocalIdV1,
        SemanticRvalueKindV1, SemanticSwitchTargetV1, SemanticSwitchTargetsV1,
        SemanticTerminatorKindV1, SemanticTypeDeclV1, SemanticTypeIdV1, SemanticUnwindActionV1,
    };

    fn assert_cleanup_rejected(
        types: &[SemanticTypeDeclV1],
        function: &SemanticFunctionDeclV1,
        ty: SemanticTypeIdV1,
        source: usize,
        normal_successors: &[usize],
        unwind_role: SemanticEdgeRoleV1,
    ) {
        let mut source_edges = Vec::new();
        function.blocks()[source]
            .terminator()
            .kind()
            .try_for_each_edge::<std::convert::Infallible>(|edge| {
                source_edges.push(edge);
                Ok(())
            })
            .unwrap();
        assert!(source_edges.contains(&cfg_edge(unwind_role, 1)));

        let mut proof = SemanticAssertProofsV1::new(types, function).unwrap();
        assert_eq!(proof.definition_counts[1], 2);
        assert_eq!(proof.definition_counts[2], 1);
        assert_eq!(proof.graph.successors(source).unwrap(), normal_successors);
        assert!(
            proof
                .graph
                .query(
                    CsrWorkV1::new(&mut proof.work, MAX_PROJECTED_LOOP_GRAPH_WORK_V1),
                    |mut query| query.enum_definitions_cover_use(
                        [
                            ScalarAssignmentSiteV1 {
                                block: 4,
                                statement: 0,
                            },
                            ScalarAssignmentSiteV1 {
                                block: 5,
                                statement: 0,
                            },
                        ],
                        ScalarAssignmentSiteV1 {
                            block: 1,
                            statement: 0,
                        },
                    ),
                )
                .unwrap(),
            "normal-edge coverage alone omits the cleanup incoming",
        );
        let conditions = global_enum_transport_v1::Conditions::new(types, function).unwrap();
        assert!(conditions.allows(types, function, SemanticLocalIdV1::from_index(2), 0, 3));
        assert!(resolve(types, function, ty).is_none());
    }

    #[test]
    fn exclusive_enum_join_rejects_call_cleanup_incoming() {
        let (types, function, ty) = fixture(false);
        for cleanup in [false, true] {
            let mut blocks = function.blocks().to_vec();
            let branch = blocks[0].terminator().kind().clone();
            let unwind = if cleanup {
                SemanticUnwindActionV1::Cleanup(cfg_edge(SemanticEdgeRoleV1::CallUnwind, 1))
            } else {
                SemanticUnwindActionV1::Unreachable
            };
            blocks[0] = block(
                210,
                vec![],
                SemanticTerminatorKindV1::Call(
                    SemanticDirectCallV1::new(
                        SemanticFunctionIdV1::from_index(0),
                        vec![],
                        Some(SemanticCallDestinationV1::new(
                            typed_place(0, U64_TYPE),
                            cfg_edge(SemanticEdgeRoleV1::CallReturn, 7),
                        )),
                        unwind,
                    )
                    .unwrap(),
                ),
            );
            blocks.push(block(217, vec![], branch));
            let changed = replace(&function, blocks);
            if cleanup {
                assert_cleanup_rejected(
                    &types,
                    &changed,
                    ty,
                    0,
                    &[7],
                    SemanticEdgeRoleV1::CallUnwind,
                );
            } else {
                assert!(resolve(&types, &changed, ty).is_some());
            }
        }
    }

    #[test]
    fn exclusive_enum_join_rejects_drop_cleanup_incoming() {
        let (types, function, ty) = fixture(false);
        for cleanup in [false, true] {
            let mut blocks = function.blocks().to_vec();
            let branch = blocks[0].terminator().kind().clone();
            let unwind = if cleanup {
                SemanticUnwindActionV1::Cleanup(cfg_edge(SemanticEdgeRoleV1::DropUnwind, 1))
            } else {
                SemanticUnwindActionV1::Unreachable
            };
            blocks[0] = block(
                210,
                vec![typed_assignment(
                    0,
                    U64_TYPE,
                    SemanticRvalueKindV1::Use(typed_constant(U64_TYPE, 0, 8)),
                )],
                SemanticTerminatorKindV1::Drop {
                    place: typed_place(0, U64_TYPE),
                    drop_glue: SemanticFunctionIdV1::from_index(0),
                    target: cfg_edge(SemanticEdgeRoleV1::DropReturn, 7),
                    unwind,
                },
            );
            blocks.push(block(217, vec![], branch));
            let changed = replace(&function, blocks);
            if cleanup {
                assert_cleanup_rejected(
                    &types,
                    &changed,
                    ty,
                    0,
                    &[7],
                    SemanticEdgeRoleV1::DropUnwind,
                );
            } else {
                assert!(resolve(&types, &changed, ty).is_some());
            }
        }
    }

    #[test]
    fn exclusive_enum_join_rejects_tail_call_cleanup_incoming() {
        let (types, function, ty) = fixture(false);
        for cleanup in [false, true] {
            let mut blocks = function.blocks().to_vec();
            let original_branch = blocks[0].terminator().kind().clone();
            let unwind = if cleanup {
                SemanticUnwindActionV1::Cleanup(cfg_edge(SemanticEdgeRoleV1::TailCallUnwind, 1))
            } else {
                SemanticUnwindActionV1::Unreachable
            };
            // A tail call has no normal continuation. Keep the original join
            // reachable through the other branch, not through an invented edge.
            blocks[0] = block(210, vec![], branch(typed_constant(U64_TYPE, 0, 8), 8, 7));
            blocks.push(block(
                217,
                vec![],
                SemanticTerminatorKindV1::TailCall(
                    SemanticDirectTailCallV1::new(
                        SemanticFunctionIdV1::from_index(0),
                        vec![],
                        unwind,
                    )
                    .unwrap(),
                ),
            ));
            blocks.push(block(218, vec![], original_branch));
            let changed = replace(&function, blocks);
            let mut proof = SemanticAssertProofsV1::new(&types, &changed).unwrap();
            assert!(
                proof
                    .graph
                    .query(
                        CsrWorkV1::new(&mut proof.work, MAX_PROJECTED_LOOP_GRAPH_WORK_V1),
                        |mut query| query.reaches(0, 7),
                    )
                    .unwrap(),
            );
            if cleanup {
                assert_cleanup_rejected(
                    &types,
                    &changed,
                    ty,
                    7,
                    &[],
                    SemanticEdgeRoleV1::TailCallUnwind,
                );
            } else {
                assert!(resolve(&types, &changed, ty).is_some());
            }
        }
    }

    #[test]
    fn exclusive_enum_join_rejects_shared_switch_target() {
        let (types, function, ty) = fixture(false);
        assert!(resolve(&types, &function, ty).is_some());
        for explicit_other in [false, true] {
            let mut blocks = function.blocks().to_vec();
            let mut targets = vec![SemanticSwitchTargetV1::new(
                0,
                cfg_edge(SemanticEdgeRoleV1::SwitchValue, 3),
            )];
            if explicit_other {
                targets.push(SemanticSwitchTargetV1::new(
                    1,
                    cfg_edge(SemanticEdgeRoleV1::SwitchValue, 3),
                ));
            }
            blocks[2] = block(
                212,
                blocks[2].statements().to_vec(),
                SemanticTerminatorKindV1::SwitchInt {
                    discriminant: typed_operand(3, U64_TYPE),
                    targets: SemanticSwitchTargetsV1::new(
                        targets,
                        cfg_edge(
                            SemanticEdgeRoleV1::SwitchOtherwise,
                            if explicit_other { 6 } else { 3 },
                        ),
                    )
                    .unwrap(),
                },
            );
            let changed = replace(&function, blocks);
            let mut proof = SemanticAssertProofsV1::new(&types, &changed).unwrap();
            assert!(
                proof
                    .graph
                    .query(
                        CsrWorkV1::new(&mut proof.work, MAX_PROJECTED_LOOP_GRAPH_WORK_V1),
                        |mut query| query.guard_authenticates_each_use((2, 3), 3),
                    )
                    .unwrap(),
                "a deduplicated block edge is not an exclusive discriminator value",
            );
            let conditions = global_enum_transport_v1::Conditions::new(&types, &changed).unwrap();
            for variant in [0, 1] {
                assert!(!conditions.allows(
                    &types,
                    &changed,
                    SemanticLocalIdV1::from_index(2),
                    variant,
                    3,
                ));
            }
            assert!(resolve(&types, &changed, ty).is_none());
        }
    }
}
