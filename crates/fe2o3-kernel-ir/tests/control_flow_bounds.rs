use fe2o3_kernel_ir::*;

fn function_with_blocks(blocks: Vec<BasicBlock>) -> Function {
    Function::definition("cfg", Signature::new(vec![], vec![]), vec![], blocks)
}

fn reverse_chain(block_count: usize) -> Function {
    assert!(block_count > 0);
    let mut blocks = Vec::with_capacity(block_count);
    for position in 0..block_count {
        let id = BlockId(u32::try_from(block_count - position - 1).unwrap());
        let mut block = BasicBlock::new(id);
        block.terminator = if position + 1 == block_count {
            Some(Terminator::Return { values: vec![] })
        } else {
            Some(Terminator::Branch {
                target: BlockId(u32::try_from(block_count - position - 2).unwrap()),
                arguments: vec![],
            })
        };
        blocks.push(block);
    }
    function_with_blocks(blocks)
}

fn amplified_phi_function(incoming: usize, parameters: usize) -> Function {
    assert!(incoming > 0);
    let mut entry = BasicBlock::new(BlockId(0));
    entry.terminator = Some(Terminator::Switch {
        selector: ValueId(0),
        cases: (0..incoming - 1)
            .map(|value| SwitchCase {
                value: u64::try_from(value).unwrap(),
                target: BlockId(1),
                arguments: vec![],
            })
            .collect(),
        default_target: BlockId(1),
        default_arguments: vec![],
    });
    let mut target = BasicBlock::new(BlockId(1));
    target.parameters = (0..parameters)
        .map(|value| {
            ValueDef::new(
                ValueId(u32::try_from(value + 1).unwrap()),
                Type::Scalar(ScalarType::I32),
            )
        })
        .collect();
    target.terminator = Some(Terminator::Return { values: vec![] });
    function_with_blocks(vec![entry, target])
}

#[test]
fn exact_block_edge_argument_and_phi_boundaries_are_stable() {
    let mut function = amplified_phi_function(3, 2);
    let Terminator::Switch {
        cases,
        default_arguments,
        ..
    } = function.body.as_mut().unwrap().blocks[0]
        .terminator
        .as_mut()
        .unwrap()
    else {
        unreachable!();
    };
    for case in cases {
        case.arguments = vec![ValueId(10), ValueId(11)];
    }
    *default_arguments = vec![ValueId(10), ValueId(11)];
    let exact = ControlFlowLimits {
        blocks: 2,
        edges: 3,
        edge_arguments: 6,
        phi_inputs: 6,
        analysis_work: 100,
    };
    let analysis = analyze_control_flow_with_limits(&function, exact).unwrap();
    assert_eq!(analysis.block_count(), 2);
    assert_eq!(analysis.edge_count(), 3);
    assert_eq!(analysis.edge_argument_count(), 6);
    assert_eq!(analysis.phi_input_count(), 6);

    for (limits, resource, limit, actual) in [
        (
            ControlFlowLimits { blocks: 1, ..exact },
            ControlFlowResource::Blocks,
            1,
            2,
        ),
        (
            ControlFlowLimits { edges: 2, ..exact },
            ControlFlowResource::Edges,
            2,
            3,
        ),
        (
            ControlFlowLimits {
                edge_arguments: 5,
                ..exact
            },
            ControlFlowResource::EdgeArguments,
            5,
            6,
        ),
        (
            ControlFlowLimits {
                phi_inputs: 5,
                ..exact
            },
            ControlFlowResource::PhiInputs,
            5,
            6,
        ),
    ] {
        assert_eq!(
            analyze_control_flow_with_limits(&function, limits),
            Err(ControlFlowError::ResourceLimit {
                resource,
                limit,
                actual,
            })
        );
    }
}

fn assert_reverse_chain_work(block_count: usize) {
    let function = reverse_chain(block_count);
    let analysis = analyze_control_flow(&function).unwrap();
    let blocks = u64::try_from(block_count).unwrap();
    let work = analysis.work();
    assert_eq!(work.index_units, 3 * blocks - 2);
    assert_eq!(work.reachability_edge_visits, blocks - 1);
    assert_eq!(work.depth_first_edge_visits, blocks - 1);
    assert_eq!(work.dominator_predecessor_visits, 2 * (blocks - 1));
    assert_eq!(work.dominator_climbs, 0);
    assert_eq!(work.interval_node_visits, 2 * blocks);
    assert_eq!(work.reducibility_edge_visits, blocks - 1);
    assert_eq!(work.reducibility_node_visits, blocks);
    assert_eq!(work.total, 11 * blocks - 7);

    let mut module = Module::new(format!("cfg::reverse_chain_{block_count}"));
    module.functions.push(function);
    verify_module(&module).expect("bounded reverse chain must verify");
}

#[test]
fn one_block_has_exact_work_count() {
    assert_reverse_chain_work(1);
}

#[test]
fn reverse_chain_1024_has_exact_linear_work_count() {
    assert_reverse_chain_work(1_024);
}

#[test]
fn reverse_chain_2048_has_exact_linear_work_count() {
    assert_reverse_chain_work(2_048);
}

#[test]
fn reverse_chain_4096_has_exact_linear_work_count() {
    assert_reverse_chain_work(4_096);
}

#[test]
fn work_limit_reports_the_first_unit_beyond_the_boundary() {
    let function = reverse_chain(16);
    let exact_work = 11 * 16 - 7;
    let exact = ControlFlowLimits {
        analysis_work: exact_work,
        ..ControlFlowLimits::DEFAULT
    };
    assert_eq!(
        analyze_control_flow_with_limits(&function, exact)
            .unwrap()
            .work()
            .total,
        exact_work
    );
    assert_eq!(
        analyze_control_flow_with_limits(
            &function,
            ControlFlowLimits {
                analysis_work: exact_work - 1,
                ..exact
            }
        ),
        Err(ControlFlowError::ResourceLimit {
            resource: ControlFlowResource::AnalysisWork,
            limit: exact_work - 1,
            actual: exact_work,
        })
    );
}

#[test]
fn duplicate_switch_edges_are_indexed_once_per_edge_and_once_per_predecessor() {
    let function = amplified_phi_function(4_096, 0);
    let analysis = analyze_control_flow(&function).unwrap();
    assert_eq!(analysis.edge_count(), 4_096);
    assert_eq!(analysis.incoming_edges(BlockId(1)).unwrap().len(), 4_096);
    assert_eq!(
        analysis
            .predecessor_blocks(BlockId(1))
            .unwrap()
            .collect::<Vec<_>>(),
        vec![BlockId(0)]
    );
    assert!(analysis.work().total < 20_000);
}

#[test]
fn verifier_rejects_phi_amplification_before_ssa_indexing() {
    let function = amplified_phi_function(1_025, 1_025);
    let mut module = Module::new("cfg::phi_amplification");
    module.functions.push(function);
    let errors = verify_module(&module).unwrap_err();
    assert_eq!(errors.diagnostics().len(), 1);
    assert_eq!(errors.diagnostics()[0].code, DiagnosticCode::ResourceLimit);
    assert_eq!(
        errors.diagnostics()[0].message,
        "CFG phi inputs exceed the deterministic limit 1048576: found 1050625"
    );
}

#[test]
#[ignore = "exercises the full V1 block-count boundary"]
fn sparse_wire_maximum_block_count_is_bounded_and_admitted() {
    let analysis = analyze_control_flow(&reverse_chain(MAX_BLOCKS_V1)).unwrap();
    assert_eq!(analysis.block_count(), MAX_BLOCKS_V1);
    assert_eq!(analysis.edge_count(), MAX_BLOCKS_V1 - 1);
    assert_eq!(analysis.work().total, 11 * MAX_BLOCKS_V1 as u64 - 7);
}

fn lookup_chain(ids: &[u32], reachable_count: usize) -> Function {
    assert!((1..=ids.len()).contains(&reachable_count));
    function_with_blocks(
        ids.iter()
            .copied()
            .enumerate()
            .map(|(position, id)| {
                let mut block = BasicBlock::new(BlockId(id));
                block.terminator = Some(if position + 1 < reachable_count {
                    Terminator::Branch {
                        target: BlockId(ids[position + 1]),
                        arguments: vec![],
                    }
                } else {
                    Terminator::Return { values: vec![] }
                });
                block
            })
            .collect(),
    )
}

#[test]
fn dense_block_lookup_preserves_all_cfg_accessors() {
    let function = lookup_chain(&[0, 1, 2], 3);
    let analysis = analyze_control_flow(&function).unwrap();
    let work = analysis.work();
    assert_eq!(analysis.block_count(), 3);
    assert_eq!(analysis.edge_count(), 2);
    assert_eq!(analysis.edge_argument_count(), 0);
    assert_eq!(analysis.phi_input_count(), 0);
    assert!(analysis.is_reducible());
    assert!(analysis.irreducible_blocks().is_empty());
    for position in 0..3 {
        let id = BlockId(u32::try_from(position).unwrap());
        assert_eq!(analysis.block_position(id), Some(position));
        assert_eq!(analysis.block_id(position), Some(id));
        assert!(analysis.is_reachable(id));
        assert_eq!(
            analysis.outgoing_edges(id),
            Some(position.min(2)..(position + 1).min(2))
        );
        let incoming = if position == 0 {
            vec![]
        } else {
            vec![position - 1]
        };
        assert_eq!(analysis.incoming_edges(id).unwrap(), incoming);
        let successors = if position < 2 {
            vec![BlockId(id.0 + 1)]
        } else {
            vec![]
        };
        assert_eq!(
            analysis.successor_blocks(id).unwrap().collect::<Vec<_>>(),
            successors
        );
        let predecessors = if position == 0 {
            vec![]
        } else {
            vec![BlockId(id.0 - 1)]
        };
        assert_eq!(
            analysis.predecessor_blocks(id).unwrap().collect::<Vec<_>>(),
            predecessors
        );
        for other in 0..3 {
            assert_eq!(analysis.dominates(id, BlockId(other)), id.0 <= other);
        }
    }
    for edge in 0..2 {
        let info = analysis.edge(edge).unwrap();
        assert_eq!(info.ordinal(), 0);
        assert_eq!(info.argument_count(), 0);
        let source = u32::try_from(edge).unwrap();
        assert_eq!(analysis.edge_source(edge), Some(BlockId(source)));
        assert_eq!(analysis.edge_target(edge), Some(BlockId(source + 1)));
        assert!(analysis.edge_arguments(&function, edge).is_empty());
    }
    assert_eq!(analysis.block_id(3), None);
    assert_eq!(analysis.edge(2), None);
    assert_eq!(analysis.edge_source(2), None);
    assert_eq!(analysis.edge_target(2), None);
    assert_eq!(analysis.work(), work);
}

#[test]
fn sparse_block_lookup_checks_identity_before_using_an_in_range_id() {
    let analysis = analyze_control_flow(&lookup_chain(&[1, 7, u32::MAX], 3)).unwrap();
    let work = analysis.work();
    // ID 1 indexes the row for ID 7, and absent ID 0 indexes the row for ID 1.
    assert_eq!(analysis.block_position(BlockId(1)), Some(0));
    assert_eq!(analysis.block_position(BlockId(7)), Some(1));
    assert_eq!(analysis.block_position(BlockId(u32::MAX)), Some(2));
    assert_eq!(analysis.block_position(BlockId(0)), None);
    assert_eq!(analysis.block_position(BlockId(2)), None);
    assert!(analysis.dominates(BlockId(1), BlockId(u32::MAX)));
    assert!(!analysis.dominates(BlockId(7), BlockId(1)));
    assert_eq!(analysis.work(), work);
}

#[test]
fn reordered_block_lookup_preserves_physical_positions_and_nonzero_entry() {
    let analysis = analyze_control_flow(&lookup_chain(&[2, 0, 1], 3)).unwrap();
    let work = analysis.work();
    for (id, position) in [(2, 0), (0, 1), (1, 2)] {
        assert_eq!(analysis.block_position(BlockId(id)), Some(position));
    }
    assert!(analysis.dominates(BlockId(2), BlockId(0)));
    assert!(analysis.dominates(BlockId(0), BlockId(1)));
    assert!(!analysis.dominates(BlockId(0), BlockId(2)));
    assert_eq!(analysis.work(), work);
}

#[test]
fn missing_and_unreachable_block_queries_remain_distinct() {
    let analysis = analyze_control_flow(&lookup_chain(&[u32::MAX, 0, 7], 2)).unwrap();
    let work = analysis.work();
    assert_eq!(analysis.block_position(BlockId(7)), Some(2));
    assert!(!analysis.is_reachable(BlockId(7)));
    assert!(analysis.dominates(BlockId(7), BlockId(7)));
    assert!(!analysis.dominates(BlockId(u32::MAX), BlockId(7)));
    assert!(!analysis.dominates(BlockId(7), BlockId(0)));
    assert!(analysis.dominates(BlockId(u32::MAX), BlockId(0)));
    for missing in [BlockId(1), BlockId(2), BlockId(8), BlockId(u32::MAX - 1)] {
        assert_eq!(analysis.block_position(missing), None);
        assert!(!analysis.is_reachable(missing));
        assert_eq!(analysis.outgoing_edges(missing), None);
        assert_eq!(analysis.incoming_edges(missing), None);
        assert!(analysis.successor_blocks(missing).is_none());
        assert!(analysis.predecessor_blocks(missing).is_none());
        assert!(!analysis.dominates(missing, missing));
        assert!(!analysis.dominates(missing, BlockId(0)));
        assert!(!analysis.dominates(BlockId(0), missing));
    }
    assert_eq!(analysis.work(), work);
}

#[test]
fn block_lookup_matches_a_linear_oracle_for_every_small_physical_permutation() {
    fn visit(ids: &mut [u32], start: usize, cases: &mut usize) {
        if start != ids.len() {
            for next in start..ids.len() {
                ids.swap(start, next);
                visit(ids, start + 1, cases);
                ids.swap(start, next);
            }
            return;
        }
        let analysis = analyze_control_flow(&lookup_chain(ids, ids.len())).unwrap();
        let work = analysis.work();
        for query in [0, 1, 2, 3, 4, 7, 17, u32::MAX - 1, u32::MAX] {
            let expected = ids.iter().position(|id| *id == query);
            assert_eq!(analysis.block_position(BlockId(query)), expected);
            assert_eq!(analysis.is_reachable(BlockId(query)), expected.is_some());
        }
        for (definition, definition_id) in ids.iter().copied().enumerate() {
            for (use_position, use_id) in ids.iter().copied().enumerate() {
                assert_eq!(
                    analysis.dominates(BlockId(definition_id), BlockId(use_id)),
                    definition <= use_position
                );
            }
        }
        assert_eq!(analysis.work(), work);
        *cases += 1;
    }
    let mut cases = 0;
    for mut ids in [[0, 1, 2, 3], [1, 7, 17, u32::MAX]] {
        for count in 1..=ids.len() {
            visit(&mut ids[..count], 0, &mut cases);
        }
    }
    assert_eq!(cases, 2 * (1 + 2 + 6 + 24));
}

#[test]
fn block_lookup_leaves_exact_construction_work_and_one_under_limits_unchanged() {
    // Three blocks and two chain edges cost 11 * 3 - 7 construction units.
    const WORK: u64 = 26;
    for ids in [[0, 1, 2], [2, 0, 1], [1, 7, u32::MAX]] {
        let function = lookup_chain(&ids, 3);
        let limits = ControlFlowLimits {
            analysis_work: WORK,
            ..ControlFlowLimits::DEFAULT
        };
        let analysis = analyze_control_flow_with_limits(&function, limits).unwrap();
        let work = analysis.work();
        assert_eq!(work.total, WORK);
        for (position, id) in ids.iter().copied().enumerate() {
            assert_eq!(analysis.block_position(BlockId(id)), Some(position));
        }
        assert_eq!(analysis.work(), work);
        assert_eq!(
            analyze_control_flow_with_limits(
                &function,
                ControlFlowLimits {
                    analysis_work: WORK - 1,
                    ..limits
                }
            ),
            Err(ControlFlowError::ResourceLimit {
                resource: ControlFlowResource::AnalysisWork,
                limit: WORK - 1,
                actual: WORK,
            })
        );
    }
}
