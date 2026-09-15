include!("natural_loop_generation_v1_tests/legacy.rs");

fn natural_generation_switch_v1(first: usize, otherwise: usize) -> SemanticTerminatorKindV1 {
    SemanticTerminatorKindV1::SwitchInt {
        discriminant: constant(0),
        targets: SemanticSwitchTargetsV1::new(
            vec![SemanticSwitchTargetV1::new(
                0,
                cfg_edge(SemanticEdgeRoleV1::SwitchValue, first as u32),
            )],
            cfg_edge(SemanticEdgeRoleV1::SwitchOtherwise, otherwise as u32),
        )
        .unwrap(),
    }
}

fn natural_generation_pair_v1(
    function: &SemanticFunctionDeclV1,
    callables: &[SemanticCallableDeclV1],
    header: usize,
    body: usize,
    exit: usize,
) -> Result<Option<ProjectedNaturalLoopTopologyV1>, ProductionRankedProjectionErrorV1> {
    let original = function.clone();
    let mut build_work = 0;
    let mut graph = LosslessCsrV1::build(
        function,
        CsrWorkV1::new(&mut build_work, MAX_PROJECTED_LOOP_GRAPH_WORK_V1),
    )
    .unwrap();
    let old = projected_loop_cfg_graph_v1(function).unwrap();
    let identity = graph.workspace_identity();
    let mut work = 0;
    let current =
        project_natural_loop_topology_v1(callables, &mut graph, header, body, exit, &mut work);
    let expected = legacy_project_natural_loop_topology_v1(
        callables, function, &old, header, body, exit, &mut 0,
    );
    assert_eq!(format!("{current:?}"), format!("{expected:?}"));
    assert_eq!(graph.workspace_identity(), identity);
    assert_eq!(function, &original);
    current
}

#[test]
fn natural_generation_exhaustive_four_node_graphs_match_legacy() {
    for encoding in 0..4096_usize {
        let function = csr_source(4, |block| {
            if block == 1 {
                return natural_generation_switch_v1(3, 2);
            }
            let shift = match block {
                0 => 0,
                2 => 4,
                3 => 8,
                _ => unreachable!(),
            };
            let mask = (encoding >> shift) & 15;
            let targets = (0..4)
                .filter(|target| mask & (1 << target) != 0)
                .collect::<Vec<_>>();
            match targets.as_slice() {
                [] => SemanticTerminatorKindV1::Return,
                [target] => SemanticTerminatorKindV1::Goto(cfg_edge(
                    SemanticEdgeRoleV1::Goto,
                    *target as u32,
                )),
                [first, second] => natural_generation_switch_v1(*first, *second),
                _ => csr_switch(&targets, targets[0]),
            }
        });
        let _ = natural_generation_pair_v1(&function, &[], 1, 2, 3);
    }
}

#[test]
fn natural_generation_real_checked_source_keeps_all_latch_predecessors() {
    let types = assertion_proof_types();
    let source = widened_u64_induction_function_with_latch(16, WidenedLatchKind::Checked);
    for extra_dead_predecessor in [false, true] {
        let mut blocks = source.blocks().to_vec();
        if extra_dead_predecessor {
            blocks.push(block(
                216,
                vec![],
                SemanticTerminatorKindV1::Goto(cfg_edge(SemanticEdgeRoleV1::Goto, 4)),
            ));
        }
        let function = projection_function_with_locals(blocks, source.locals().to_vec());
        let topology = natural_generation_pair_v1(&function, &[], 1, 2, 5)
            .unwrap()
            .unwrap();
        assert_eq!(topology.loop_blocks, [1, 2, 3, 4]);
        let definitions = local_definition_counts(&function);
        let mut proof = SemanticAssertProofsV1::new(&types, &function).unwrap();
        project_natural_loop_topology_v1(&[], &mut proof.graph, 1, 2, 5, &mut 0)
            .unwrap()
            .unwrap();
        let update = source_induction_update_v1(
            &mut proof,
            topology.latch,
            &topology.loop_blocks,
            SemanticLocalIdV1::from_index(1),
            0,
            &definitions,
            &mut 0,
        );
        if extra_dead_predecessor {
            assert_incomplete(
                update,
                "a checked induction latch does not have one exact producer predecessor",
            );
            assert_eq!(proof.graph.predecessors(4), Some([3, 6].as_slice()));
        } else {
            assert!(matches!(
                update.unwrap().unwrap().0,
                ProjectedSourceInductionUpdateV1::Checked { .. }
            ));
        }
    }
}

#[test]
fn natural_generation_real_ordinary_unchecked_and_checked_loops_match() {
    for kind in [
        WidenedLatchKind::Ordinary,
        WidenedLatchKind::Unchecked,
        WidenedLatchKind::Checked,
        WidenedLatchKind::CheckedWrongMessage,
        WidenedLatchKind::CheckedExpectedOverflow,
        WidenedLatchKind::CheckedReachableUnwind,
    ] {
        let function = widened_u64_induction_function_with_latch(16, kind);
        let topology = natural_generation_pair_v1(&function, &[], 1, 2, 5)
            .unwrap()
            .unwrap();
        assert_eq!(topology.loop_blocks, [1, 2, 3, 4]);
    }
}

#[test]
fn natural_generation_trap_paths_keep_source_effect_and_intrinsic_checks() {
    for mutation in 0..5 {
        let callables = [compiler_intrinsic_callable(if mutation == 1 {
            SemanticCompilerIntrinsicOperationV1::ColdPath
        } else {
            SemanticCompilerIntrinsicOperationV1::Trap
        })];
        let mut function = csr_source(6, |block| match block {
            0 => SemanticTerminatorKindV1::Goto(cfg_edge(SemanticEdgeRoleV1::Goto, 1)),
            1 => natural_generation_switch_v1(4, 2),
            2 => natural_generation_switch_v1(5, 3),
            3 => SemanticTerminatorKindV1::Goto(cfg_edge(SemanticEdgeRoleV1::Goto, 1)),
            4 => SemanticTerminatorKindV1::Return,
            5 if mutation == 3 => {
                SemanticTerminatorKindV1::Goto(cfg_edge(SemanticEdgeRoleV1::Goto, 5))
            }
            5 => SemanticTerminatorKindV1::Call(
                SemanticDirectCallV1::new_callable(
                    SemanticCallableIdV1::from_index(0),
                    vec![],
                    None,
                    if mutation == 4 {
                        SemanticUnwindActionV1::Cleanup(cfg_edge(SemanticEdgeRoleV1::CallUnwind, 4))
                    } else {
                        SemanticUnwindActionV1::Unreachable
                    },
                )
                .unwrap(),
            ),
            _ => unreachable!(),
        });
        if mutation == 2 {
            let mut blocks = function.blocks().to_vec();
            blocks[5] = block(
                222,
                vec![statement(SemanticStatementKindV1::Assign(
                    SemanticAssignmentV1::new(
                        scalar_place(),
                        SemanticRvalueV1::new(SCALAR_TYPE, SemanticRvalueKindV1::Use(constant(0))),
                    ),
                ))],
                blocks[5].terminator().kind().clone(),
            );
            function = projection_function_with_locals(blocks, function.locals().to_vec());
        }
        let result = natural_generation_pair_v1(&function, &callables, 1, 2, 4);
        if mutation == 0 {
            assert_eq!(result.unwrap().unwrap().loop_blocks, [1, 2, 3]);
        } else {
            assert_incomplete(
                result,
                "a uniform induction region does not have one unique header exit",
            );
        }
    }
}

#[test]
fn natural_generation_exact_work_ceiling_reuses_workspace_after_failure() {
    let function = widened_u64_induction_function_with_latch(16, WidenedLatchKind::Checked);
    let mut build_work = 0;
    let mut graph = LosslessCsrV1::build(
        &function,
        CsrWorkV1::new(&mut build_work, MAX_PROJECTED_LOOP_GRAPH_WORK_V1),
    )
    .unwrap();
    let identity = graph.workspace_identity();
    let mut measured = 0;
    let expected = project_natural_loop_topology_v1(&[], &mut graph, 1, 2, 5, &mut measured)
        .unwrap()
        .unwrap();
    assert!(measured > 0);
    for ceiling in [measured - 1, measured] {
        let mut work = 0;
        let result = graph.query(CsrWorkV1::new(&mut work, ceiling), |mut query| {
            query.natural_loop_topology(&[], 1, 2, 5)
        });
        if ceiling == measured {
            assert_eq!(
                format!("{:?}", result.unwrap().unwrap()),
                format!("{expected:?}")
            );
            assert_eq!(work, measured);
        } else {
            assert_loop_unsupported(result, "CSR analysis exceeds its inherited work limit");
            assert!(work > ceiling);
        }
        assert_eq!(graph.workspace_identity(), identity);
    }
}

#[test]
fn natural_generation_caps_and_exact_source_rows_remain_unchanged() {
    for nodes in [MAX_RANKED_BOUNDS_BLOCKS, MAX_RANKED_BOUNDS_BLOCKS + 1] {
        let function = csr_source(nodes, |_| SemanticTerminatorKindV1::Return);
        let mut work = 0;
        let result = LosslessCsrV1::build(
            &function,
            CsrWorkV1::new(&mut work, MAX_PROJECTED_LOOP_GRAPH_WORK_V1),
        );
        if nodes == MAX_RANKED_BOUNDS_BLOCKS {
            let mut graph = result.unwrap();
            assert!(std::ptr::eq(graph.source(), &function));
            assert_eq!(graph.predecessors(nodes - 1), Some([].as_slice()));
            assert!(
                project_natural_loop_topology_v1(&[], &mut graph, 0, 0, 1, &mut work)
                    .unwrap()
                    .is_none()
            );
        } else {
            assert!(matches!(
                result,
                Err(ProductionRankedProjectionErrorV1::SourceCfgLimit(_))
            ));
        }
    }
}

#[test]
fn natural_generation_complete_discovery_retains_bound_and_checked_source_gates() {
    let types = assertion_proof_types();
    for (kind, accepted) in [
        (WidenedLatchKind::Ordinary, true),
        (WidenedLatchKind::Unchecked, true),
        (WidenedLatchKind::Checked, true),
        (WidenedLatchKind::CheckedWrongMessage, false),
        (WidenedLatchKind::CheckedExpectedOverflow, false),
        (WidenedLatchKind::CheckedReachableUnwind, false),
    ] {
        let function = widened_u64_induction_function_with_latch(16, kind);
        let original = function.clone();
        let locals = function.locals().len();
        let mut arguments = vec![None; locals];
        let mut next_argument = 1;
        let mut operations = Vec::new();
        let mut next_value = 0;
        let result = project_uniform_inductions_v1(
            &[],
            &types,
            &function,
            &vec![None; locals],
            &vec![None; locals],
            &local_definition_counts(&function),
            &mut arguments,
            &mut next_argument,
            &mut operations,
            &mut next_value,
        );
        if accepted {
            let loops = result.unwrap();
            assert_eq!(loops.len(), 1);
            let induction = &loops[0];
            assert_eq!(induction.loop_blocks, [1, 2, 3, 4]);
            assert_eq!(induction.bound, ProductionRankedValueV1::Argument(1));
            assert_eq!(induction.bound_cast.as_ref().unwrap().bit_width, 32);
            assert_eq!(induction.source_progress.ranked_bound, induction.bound);
            assert_eq!(induction.source_progress.step_value, 16);
            assert_eq!(next_argument, 2);
            assert_eq!(operations.len(), 2);
        } else {
            assert_incomplete(
                result,
                "a checked induction overflow assertion does not authenticate its exact Add result and success edge",
            );
        }
        assert_eq!(function, original);
    }
}
