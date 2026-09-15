// Include in the ranked parent's existing test module; no production hook yet.
use super::lossless_csr_v1::{CsrWorkV1, LosslessCsrV1};

fn csr_source(
    count: usize,
    mut terminator: impl FnMut(usize) -> SemanticTerminatorKindV1,
) -> SemanticFunctionDeclV1 {
    projection_function(
        (0..count)
            .map(|index| {
                let mut identity = [239; 32];
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

fn csr_switch(targets: &[usize], otherwise: usize) -> SemanticTerminatorKindV1 {
    SemanticTerminatorKindV1::SwitchInt {
        discriminant: constant(0),
        targets: SemanticSwitchTargetsV1::new(
            targets
                .iter()
                .enumerate()
                .map(|(value, &target)| {
                    SemanticSwitchTargetV1::new(
                        value as u128,
                        cfg_edge(SemanticEdgeRoleV1::SwitchValue, target as u32),
                    )
                })
                .collect(),
            cfg_edge(SemanticEdgeRoleV1::SwitchOtherwise, otherwise as u32),
        )
        .unwrap(),
    }
}

fn csr_compare_source(function: &SemanticFunctionDeclV1) {
    let original = function.clone();
    let old = projected_loop_cfg_graph_v1(function).unwrap();
    let mut used = 0;
    let mut csr = LosslessCsrV1::build(
        function,
        CsrWorkV1::new(&mut used, MAX_PROJECTED_LOOP_GRAPH_WORK_V1),
    )
    .unwrap();
    assert!(std::ptr::eq(csr.source(), function));
    for block in 0..function.blocks().len() {
        assert_eq!(csr.successors(block).unwrap(), old.successors[block]);
        assert_eq!(csr.predecessors(block).unwrap(), old.predecessors[block]);
        assert_eq!(csr.is_entry_reachable(block), old.reachable[block]);
    }
    let mut proof = primary_csr_legacy_v1::LegacyQueriesV1::new(function).unwrap();
    csr.query(
        CsrWorkV1::new(&mut used, MAX_PROJECTED_LOOP_GRAPH_WORK_V1),
        |mut query| {
            for target in 0..function.blocks().len() {
                let reaching = proof.blocks_reaching(target).unwrap();
                for source in 0..function.blocks().len() {
                    assert_eq!(query.reaches(source, target)?, reaching[source]);
                    assert_eq!(
                        query.block_dominates(source, target)?,
                        proof.block_dominates(source, target).unwrap()
                    );
                    for &successor in &old.successors[source] {
                        let edges = HashSet::from([(source, successor)]);
                        assert_eq!(
                            query.edge_set_dominates(&edges, target)?,
                            proof.edge_set_dominates(&edges, target).unwrap()
                        );
                    }
                }
            }
            Ok(())
        },
    )
    .unwrap();
    assert_eq!(function, &original);
}

#[test]
fn lossless_csr_exhaustive_three_node_graphs_match_legacy_queries() {
    // All directed simple graphs, including dead components, cycles, and self edges.
    for encoding in 0..512_usize {
        let function = csr_source(3, |block| {
            let mask = (encoding >> (block * 3)) & 7;
            let targets = (0..3)
                .filter(|target| mask & (1 << target) != 0)
                .collect::<Vec<_>>();
            targets
                .first()
                .map_or(SemanticTerminatorKindV1::Return, |&otherwise| {
                    csr_switch(&targets, otherwise)
                })
        });
        csr_compare_source(&function);
    }
}

#[test]
fn lossless_csr_retains_unreachable_predecessor_in_exact_latch_roster() {
    let function = csr_source(3, |block| match block {
        0 | 1 => SemanticTerminatorKindV1::Goto(cfg_edge(SemanticEdgeRoleV1::Goto, 2)),
        _ => SemanticTerminatorKindV1::Return,
    });
    let mut used = 0;
    let csr = LosslessCsrV1::build(
        &function,
        CsrWorkV1::new(&mut used, MAX_PROJECTED_LOOP_GRAPH_WORK_V1),
    )
    .unwrap();
    assert!(!csr.is_entry_reachable(1));
    assert_eq!(csr.predecessors(2), Some([0, 1].as_slice()));
    assert_ne!(csr.predecessors(2), Some([0].as_slice()));
    csr_compare_source(&function);
}

#[test]
fn lossless_csr_keeps_exact_boolean_fallback_normalization_and_duplicate_targets() {
    for fallback_is_empty in [false, true] {
        let fallback_statements = if fallback_is_empty {
            vec![]
        } else {
            vec![statement(SemanticStatementKindV1::Nop)]
        };
        let function = projection_function(vec![
            block(201, vec![], csr_switch(&[2, 1], 3)),
            block(202, vec![], SemanticTerminatorKindV1::Return),
            block(203, vec![], SemanticTerminatorKindV1::Return),
            block(
                204,
                fallback_statements,
                SemanticTerminatorKindV1::Unreachable,
            ),
        ]);
        csr_compare_source(&function);
        let mut used = 0;
        let csr = LosslessCsrV1::build(
            &function,
            CsrWorkV1::new(&mut used, MAX_PROJECTED_LOOP_GRAPH_WORK_V1),
        )
        .unwrap();
        assert_eq!(
            csr.successors(0).unwrap(),
            if fallback_is_empty {
                &[1, 2][..]
            } else {
                &[1, 2, 3][..]
            }
        );
    }
    csr_compare_source(&csr_source(3, |block| {
        if block == 0 {
            csr_switch(&[2, 1, 2], 1)
        } else {
            SemanticTerminatorKindV1::Return
        }
    }));
}

#[test]
fn lossless_csr_keeps_normal_call_assert_drop_edges_not_cleanup_edges() {
    for kind in 0..4 {
        let function = csr_source(4, |block| {
            if block != 0 {
                SemanticTerminatorKindV1::Return
            } else {
                match kind {
                    0 => SemanticTerminatorKindV1::Call(
                        SemanticDirectCallV1::new_callable(
                            SemanticCallableIdV1::from_index(0),
                            vec![],
                            Some(SemanticCallDestinationV1::new(
                                scalar_place(),
                                cfg_edge(SemanticEdgeRoleV1::CallReturn, 1),
                            )),
                            SemanticUnwindActionV1::Cleanup(cfg_edge(
                                SemanticEdgeRoleV1::CallUnwind,
                                2,
                            )),
                        )
                        .unwrap(),
                    ),
                    1 => SemanticTerminatorKindV1::Assert {
                        condition: constant(1),
                        expected: true,
                        message: SemanticAssertMessageV1::DivisionByZero(constant(1)),
                        target: cfg_edge(SemanticEdgeRoleV1::AssertSuccess, 1),
                        unwind: SemanticUnwindActionV1::Cleanup(cfg_edge(
                            SemanticEdgeRoleV1::AssertUnwind,
                            2,
                        )),
                    },
                    2 => SemanticTerminatorKindV1::Drop {
                        place: scalar_place(),
                        drop_glue: SemanticFunctionIdV1::from_index(0),
                        target: cfg_edge(SemanticEdgeRoleV1::DropReturn, 1),
                        unwind: SemanticUnwindActionV1::Cleanup(cfg_edge(
                            SemanticEdgeRoleV1::DropUnwind,
                            2,
                        )),
                    },
                    _ => SemanticTerminatorKindV1::TailCall(
                        fe2o3_mir_model::semantic_mir_v1::SemanticDirectTailCallV1::new_callable(
                            SemanticCallableIdV1::from_index(0),
                            vec![],
                            SemanticUnwindActionV1::Cleanup(cfg_edge(
                                SemanticEdgeRoleV1::TailCallUnwind,
                                2,
                            )),
                        )
                        .unwrap(),
                    ),
                }
            }
        });
        csr_compare_source(&function);
    }
}

#[test]
fn lossless_csr_invalid_targets_and_false_edges_still_reject() {
    for terminator in [
        SemanticTerminatorKindV1::Goto(cfg_edge(SemanticEdgeRoleV1::Goto, 2)),
        csr_switch(&[0, 1], 2),
        SemanticTerminatorKindV1::FalseEdge {
            real_target: cfg_edge(SemanticEdgeRoleV1::FalseEdgeReal, 0),
            imaginary_target: cfg_edge(SemanticEdgeRoleV1::FalseEdgeImaginary, 1),
        },
    ] {
        let function = csr_source(2, |block| {
            if block == 0 {
                terminator.clone()
            } else {
                SemanticTerminatorKindV1::Unreachable
            }
        });
        let old = projected_loop_cfg_graph_v1(&function).unwrap_err();
        let mut used = 0;
        let error = LosslessCsrV1::build(
            &function,
            CsrWorkV1::new(&mut used, MAX_PROJECTED_LOOP_GRAPH_WORK_V1),
        )
        .err()
        .unwrap();
        match (old, error) {
            (
                ProductionRankedProjectionErrorV1::Unsupported(a),
                ProductionRankedProjectionErrorV1::Unsupported(b),
            )
            | (
                ProductionRankedProjectionErrorV1::Incomplete(a),
                ProductionRankedProjectionErrorV1::Incomplete(b),
            ) => assert_eq!(a, b),
            errors => panic!("normalization rejection changed: {errors:?}"),
        }
    }
}

#[test]
fn lossless_csr_current_raw_node_cap_stays_inclusive_measured_sizes_stay_rejected() {
    for nodes in [
        MAX_RANKED_BOUNDS_BLOCKS,
        MAX_RANKED_BOUNDS_BLOCKS + 1,
        1310,
        1765,
        2361,
    ] {
        let function = csr_source(nodes, |_| SemanticTerminatorKindV1::Return);
        let mut used = 0;
        let result = LosslessCsrV1::build(
            &function,
            CsrWorkV1::new(&mut used, MAX_PROJECTED_LOOP_GRAPH_WORK_V1),
        );
        if nodes == MAX_RANKED_BOUNDS_BLOCKS {
            assert!(result.is_ok());
        } else {
            assert!(matches!(
                result,
                Err(ProductionRankedProjectionErrorV1::SourceCfgLimit(_))
            ));
            assert_eq!(used, 0, "raw guard precedes CSR reservations");
        }
    }
}

#[test]
fn lossless_csr_edge_cap_counts_unique_edges_in_all_raw_rows() {
    for extra in [false, true] {
        let function = csr_source(64, |block| {
            let targets = (0..32 + usize::from(extra && block == 63)).collect::<Vec<_>>();
            csr_switch(&targets, 0)
        });
        let mut used = 0;
        let result = LosslessCsrV1::build(
            &function,
            CsrWorkV1::new(&mut used, MAX_PROJECTED_LOOP_GRAPH_WORK_V1),
        );
        if extra {
            assert!(matches!(
                result,
                Err(ProductionRankedProjectionErrorV1::Unsupported(
                    "semantic CFG exceeds the ranked edge limit before loop analysis"
                ))
            ));
        } else {
            let csr = result.unwrap();
            assert!(!csr.is_entry_reachable(63));
            assert_eq!(
                csr.storage_items(),
                6 * 64 + 2 * MAX_RANKED_BOUNDS_EDGES + 2
            );
        }
    }
}

#[test]
fn lossless_csr_exact_construction_work_and_query_storage_identity() {
    let function = csr_source(3, |block| {
        if block < 2 {
            SemanticTerminatorKindV1::Goto(cfg_edge(SemanticEdgeRoleV1::Goto, (block + 1) as u32))
        } else {
            SemanticTerminatorKindV1::Return
        }
    });
    let mut used = 0;
    let mut csr = LosslessCsrV1::build(
        &function,
        CsrWorkV1::new(&mut used, MAX_PROJECTED_LOOP_GRAPH_WORK_V1),
    )
    .unwrap();
    // 11N + 5E + 2Q + sum(d*ceil(log2(d))) + R + E_R + 2.
    assert_eq!(used, 54);
    assert_eq!(csr.storage_items(), 24);
    let allocation = csr.workspace_identity();
    for _ in 0..32 {
        csr.query(
            CsrWorkV1::new(&mut used, MAX_PROJECTED_LOOP_GRAPH_WORK_V1),
            |mut query| {
                assert!(query.block_dominates(0, 2)?);
                assert!(query.reaches(1, 2)?);
                assert!(!query.reaches(2, 1)?);
                Ok(())
            },
        )
        .unwrap();
        assert_eq!(csr.workspace_identity(), allocation);
    }
    used = MAX_PROJECTED_LOOP_GRAPH_WORK_V1;
    assert!(
        csr.query(
            CsrWorkV1::new(&mut used, MAX_PROJECTED_LOOP_GRAPH_WORK_V1),
            |mut query| query.reaches(0, 2)
        )
        .is_err()
    );
    assert_eq!(csr.workspace_identity(), allocation);
}

#[test]
fn lossless_csr_inherited_ceiling_rejects_before_constructor_reservation() {
    let function = csr_source(3, |_| SemanticTerminatorKindV1::Return);
    let mut used = 7;
    assert!(matches!(
        LosslessCsrV1::build(&function, CsrWorkV1::new(&mut used, 7)),
        Err(ProductionRankedProjectionErrorV1::Unsupported(
            "CSR analysis exceeds its inherited work limit"
        ))
    ));
    assert_eq!(used, 7 + 6 * 3 + 2);
}

#[test]
fn lossless_csr_revalidation_preserves_exact_guard_and_source_definitions() {
    for bypass_guard in [false, true] {
        for overwrite_use in [false, true] {
            let definition =
                || typed_assignment(2, SCALAR_TYPE, SemanticRvalueKindV1::Use(constant(7)));
            let function = projection_function(vec![
                block(
                    201,
                    vec![],
                    SemanticTerminatorKindV1::Goto(cfg_edge(SemanticEdgeRoleV1::Goto, 1)),
                ),
                block(202, vec![], zero_switch(2, SCALAR_TYPE, 5, 2)),
                // The latch can run before the tested use, not only after it.
                block(203, vec![], csr_switch(&[3], 4)),
                block(
                    204,
                    vec![definition()],
                    SemanticTerminatorKindV1::Goto(cfg_edge(
                        SemanticEdgeRoleV1::Goto,
                        if bypass_guard { 4 } else { 1 },
                    )),
                ),
                block(
                    205,
                    if overwrite_use {
                        vec![definition()]
                    } else {
                        vec![]
                    },
                    SemanticTerminatorKindV1::Goto(cfg_edge(SemanticEdgeRoleV1::Goto, 3)),
                ),
                block(206, vec![], SemanticTerminatorKindV1::Return),
            ]);
            let mut old = primary_csr_legacy_v1::LegacyQueriesV1::new(&function).unwrap();
            let can_reach = old.blocks_reaching(4).unwrap();
            let expected = old.edge_set_dominates(&HashSet::from([(1, 2)]), 4).unwrap()
                && old
                    .local_is_stable_from_revalidating_edge_to_use(
                        2,
                        1,
                        2,
                        4,
                        &can_reach,
                        &mut vec![0; 6],
                        &mut VecDeque::new(),
                        1,
                    )
                    .unwrap();
            assert_eq!(expected, !bypass_guard && !overwrite_use);
            let mut used = 0;
            let mut csr = LosslessCsrV1::build(
                &function,
                CsrWorkV1::new(&mut used, MAX_PROJECTED_LOOP_GRAPH_WORK_V1),
            )
            .unwrap();
            csr.query(
                CsrWorkV1::new(&mut used, MAX_PROJECTED_LOOP_GRAPH_WORK_V1),
                |mut query| {
                    assert_eq!(query.guarded_local_stable(2, (1, 2), 4)?, expected);
                    assert!(!query.guarded_local_stable(2, (1, 5), 4)?);
                    Ok(())
                },
            )
            .unwrap();
        }
    }
}
