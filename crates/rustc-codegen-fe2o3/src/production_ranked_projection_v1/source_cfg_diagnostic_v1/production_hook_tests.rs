// Included in the ranked parent's existing test module to reuse its fixtures.
include!("empty_goto_hook_tests.rs");

fn source_cfg_diagnostic_function(
    count: usize,
    mut terminator: impl FnMut(usize) -> SemanticTerminatorKindV1,
) -> SemanticFunctionDeclV1 {
    projection_function(
        (0..count)
            .map(|index| {
                let mut identity = [231; 32];
                identity[..8].copy_from_slice(&(index as u64).to_le_bytes());
                SemanticBasicBlockV1::new(
                    SemanticBlockIdentityV1::from_sha256(identity),
                    SemanticSourceProvenanceV1::unavailable(),
                    vec![],
                    SemanticTerminatorV1::new(
                        SemanticSourceProvenanceV1::unavailable(),
                        terminator(index),
                    ),
                )
                .unwrap()
            })
            .collect(),
    )
}

fn source_cfg_diagnostic_from_production_guard(function: &SemanticFunctionDeclV1) -> String {
    let original = function.clone();
    let error = projected_loop_cfg_graph_v1(function).unwrap_err();
    let ProductionRankedProjectionErrorV1::SourceCfgLimit(diagnostic) = error else {
        panic!("expected the existing raw-block gate's diagnostic variant: {error:?}");
    };
    assert_eq!(function, &original, "diagnostics cannot rewrite source MIR");
    let text = diagnostic.to_string();
    assert!(text.contains("complete-source-edge entry reachability:"));
    let (original, census) = text.split_once("; empty-Goto census: ").unwrap();
    assert!(census.contains("source_only_not_ranked_eligibility"));
    original.to_owned()
}

#[test]
fn source_cfg_diagnostic_production_hook_preserves_exact_old_block_limit() {
    let chain = |count| {
        source_cfg_diagnostic_function(count, |index| {
            if index + 1 == count {
                SemanticTerminatorKindV1::Return
            } else {
                SemanticTerminatorKindV1::Goto(cfg_edge(
                    SemanticEdgeRoleV1::Goto,
                    (index + 1) as u32,
                ))
            }
        })
    };
    let exact = chain(MAX_RANKED_BOUNDS_BLOCKS);
    let original = exact.clone();
    let graph = projected_loop_cfg_graph_v1(&exact).expect("unchanged inclusive old input limit");
    assert_eq!(graph.successors.len(), MAX_RANKED_BOUNDS_BLOCKS);
    assert!(graph.reachable.iter().all(|&reachable| reachable));
    assert_eq!(exact, original);

    let rejected = chain(MAX_RANKED_BOUNDS_BLOCKS + 1);
    let text = source_cfg_diagnostic_from_production_guard(&rejected);
    assert!(text.contains("declared_source_blocks=1025 limit=1024;"));
    assert!(text.ends_with("1025 blocks, 1024 edge visits"), "{text}");
    assert!(!text.contains("at least"));
    assert!(!text.contains("budget reached"));
}

#[test]
fn source_cfg_diagnostic_production_hook_labels_block_budget_as_lower_bound() {
    let count = MAX_RANKED_BOUNDS_BLOCKS + 2;
    let function = source_cfg_diagnostic_function(count, |index| {
        if index + 1 == count {
            SemanticTerminatorKindV1::Return
        } else {
            SemanticTerminatorKindV1::Goto(cfg_edge(SemanticEdgeRoleV1::Goto, (index + 1) as u32))
        }
    });
    let text = source_cfg_diagnostic_from_production_guard(&function);
    assert!(text.contains("declared_source_blocks=1026 limit=1024;"));
    assert!(
        text.ends_with("at least 1025 blocks, 1025 edge visits (diagnostic budget reached)"),
        "{text}"
    );
}

fn source_cfg_cleanup_call() -> SemanticTerminatorKindV1 {
    SemanticTerminatorKindV1::Call(
        SemanticDirectCallV1::new_callable(
            SemanticCallableIdV1::from_index(0),
            vec![],
            Some(SemanticCallDestinationV1::new(
                scalar_place(),
                cfg_edge(SemanticEdgeRoleV1::CallReturn, 1),
            )),
            SemanticUnwindActionV1::Cleanup(cfg_edge(SemanticEdgeRoleV1::CallUnwind, 2)),
        )
        .unwrap(),
    )
}

#[test]
fn source_cfg_diagnostic_capture_is_complete_source_not_normal_loop_graph() {
    let function = |count| {
        source_cfg_diagnostic_function(count, |index| match index {
            0 => source_cfg_cleanup_call(),
            2 => SemanticTerminatorKindV1::Goto(cfg_edge(SemanticEdgeRoleV1::Goto, 3)),
            _ => SemanticTerminatorKindV1::Return,
        })
    };
    let small = function(4);
    let normal = projected_loop_cfg_graph_v1(&small).unwrap();
    assert_eq!(normal.reachable, [true, true, false, false]);
    let complete = source_cfg_diagnostic_v1::SourceCfgLimitV1::capture(&small).to_string();
    assert!(complete.contains("4 blocks, 3 edge visits; empty-Goto census: "), "{complete}");

    let rejected = function(MAX_RANKED_BOUNDS_BLOCKS + 1);
    let text = source_cfg_diagnostic_from_production_guard(&rejected);
    assert!(text.contains("declared_source_blocks=1025 limit=1024;"));
    assert!(text.ends_with("4 blocks, 3 edge visits"), "{text}");
    assert!(!text.contains("at least"));
}

#[test]
fn source_cfg_diagnostic_capture_includes_all_canonical_edge_kinds() {
    let function =
        source_cfg_diagnostic_function(MAX_RANKED_BOUNDS_BLOCKS + 1, |index| match index {
            0 => source_cfg_cleanup_call(),
            1 => SemanticTerminatorKindV1::Assert {
                condition: typed_constant(BOOL_TYPE, 1, 1),
                expected: true,
                message: SemanticAssertMessageV1::DivisionByZero(constant(1)),
                target: cfg_edge(SemanticEdgeRoleV1::AssertSuccess, 3),
                unwind: SemanticUnwindActionV1::Cleanup(cfg_edge(
                    SemanticEdgeRoleV1::AssertUnwind,
                    4,
                )),
            },
            2 => SemanticTerminatorKindV1::Drop {
                place: scalar_place(),
                drop_glue: SemanticFunctionIdV1::from_index(0),
                target: cfg_edge(SemanticEdgeRoleV1::DropReturn, 5),
                unwind: SemanticUnwindActionV1::Cleanup(cfg_edge(
                    SemanticEdgeRoleV1::DropUnwind,
                    6,
                )),
            },
            3 => SemanticTerminatorKindV1::TailCall(
                fe2o3_mir_model::semantic_mir_v1::SemanticDirectTailCallV1::new_callable(
                    SemanticCallableIdV1::from_index(0),
                    vec![],
                    SemanticUnwindActionV1::Cleanup(cfg_edge(
                        SemanticEdgeRoleV1::TailCallUnwind,
                        7,
                    )),
                )
                .unwrap(),
            ),
            4 => SemanticTerminatorKindV1::FalseEdge {
                real_target: cfg_edge(SemanticEdgeRoleV1::FalseEdgeReal, 8),
                imaginary_target: cfg_edge(SemanticEdgeRoleV1::FalseEdgeImaginary, 9),
            },
            5 => SemanticTerminatorKindV1::SwitchInt {
                discriminant: typed_constant(BOOL_TYPE, 0, 1),
                targets: SemanticSwitchTargetsV1::new(
                    vec![
                        SemanticSwitchTargetV1::new(
                            0,
                            cfg_edge(SemanticEdgeRoleV1::SwitchValue, 10),
                        ),
                        SemanticSwitchTargetV1::new(
                            1,
                            cfg_edge(SemanticEdgeRoleV1::SwitchValue, 11),
                        ),
                    ],
                    cfg_edge(SemanticEdgeRoleV1::SwitchOtherwise, 12),
                )
                .unwrap(),
            },
            12 => SemanticTerminatorKindV1::Unreachable,
            _ => SemanticTerminatorKindV1::Return,
        });
    let text = source_cfg_diagnostic_from_production_guard(&function);
    assert!(text.ends_with("13 blocks, 12 edge visits"), "{text}");
    assert!(!text.contains("at least"));
}

#[test]
fn source_cfg_diagnostic_production_hook_counts_parallel_edges_before_deduplication() {
    for (raw_edges, bounded) in [
        (MAX_RANKED_BOUNDS_EDGES, false),
        (MAX_RANKED_BOUNDS_EDGES + 1, true),
    ] {
        let function = source_cfg_diagnostic_function(MAX_RANKED_BOUNDS_BLOCKS + 1, |index| {
            if index != 0 {
                return SemanticTerminatorKindV1::Return;
            }
            SemanticTerminatorKindV1::SwitchInt {
                discriminant: constant(0),
                targets: SemanticSwitchTargetsV1::new(
                    (0..raw_edges - 1)
                        .map(|value| {
                            SemanticSwitchTargetV1::new(
                                value as u128,
                                cfg_edge(SemanticEdgeRoleV1::SwitchValue, 1),
                            )
                        })
                        .collect(),
                    cfg_edge(SemanticEdgeRoleV1::SwitchOtherwise, 1),
                )
                .unwrap(),
            }
        });
        let text = source_cfg_diagnostic_from_production_guard(&function);
        assert_eq!(text.contains("at least"), bounded, "{text}");
        assert_eq!(
            text.contains("diagnostic budget reached"),
            bounded,
            "{text}"
        );
        assert!(text.contains("2 blocks, 2048 edge visits"), "{text}");
    }
}
