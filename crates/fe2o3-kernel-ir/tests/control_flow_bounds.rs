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

#[test]
fn reverse_chains_have_exact_linear_work_counts() {
    for block_count in [1usize, 1_024, 2_048, 4_096] {
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
