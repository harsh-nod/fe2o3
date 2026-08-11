use fe2o3_kernel_analysis::{
    ControlFlowDiagnostic, ControlFlowEdge, ControlFlowResource, MAX_CONTROL_FLOW_BLOCKS,
    MAX_CONTROL_FLOW_DOMINANCE_FRONTIER_ENTRIES, MAX_CONTROL_FLOW_EDGES,
    MAX_CONTROL_FLOW_LOOP_BODY_MEMBERSHIPS, MAX_CONTROL_FLOW_NATURAL_LOOPS,
    MAX_CONTROL_FLOW_WORK_UNITS, analyze_control_flow,
};
use fe2o3_kernel_ir::{
    BasicBlock, BlockId, Constant, Function, Operation, OperationKind, Signature, SwitchCase,
    Terminator, Type, ValueDef, ValueId,
};
use std::collections::BTreeSet;

fn function(blocks: Vec<BasicBlock>) -> Function {
    Function::definition("cfg", Signature::new(vec![], vec![]), vec![], blocks)
}

fn returning(id: u32) -> BasicBlock {
    block(id, Terminator::Return { values: vec![] })
}

fn branch(id: u32, target: u32) -> BasicBlock {
    block(
        id,
        Terminator::Branch {
            target: BlockId(target),
            arguments: vec![],
        },
    )
}

fn conditional(id: u32, then_target: u32, else_target: u32) -> BasicBlock {
    let mut block = block(
        id,
        Terminator::ConditionalBranch {
            condition: ValueId(100 + id),
            then_target: BlockId(then_target),
            then_arguments: vec![],
            else_target: BlockId(else_target),
            else_arguments: vec![],
        },
    );
    block.operations.push(Operation::effect_free(
        ValueDef::new(ValueId(100 + id), Type::BOOL),
        OperationKind::Constant(Constant::Bool(true)),
    ));
    block
}

fn block(id: u32, terminator: Terminator) -> BasicBlock {
    let mut block = BasicBlock::new(BlockId(id));
    block.terminator = Some(terminator);
    block
}

fn raw_conditional(id: u32, then_target: u32, else_target: u32) -> BasicBlock {
    block(
        id,
        Terminator::ConditionalBranch {
            condition: ValueId(u32::MAX - id),
            then_target: BlockId(then_target),
            then_arguments: vec![],
            else_target: BlockId(else_target),
            else_arguments: vec![],
        },
    )
}

fn ids(values: &[u32]) -> BTreeSet<BlockId> {
    values.iter().copied().map(BlockId).collect()
}

fn two_arm_frontier_ladder(rungs: u32) -> Function {
    let left_start = 1_u32;
    let right_start = left_start + rungs;
    let exit = right_start + rungs;
    let mut blocks = Vec::with_capacity((2 * rungs + 2) as usize);
    blocks.push(raw_conditional(0, left_start, right_start));
    for rung in 0..rungs {
        let left = left_start + rung;
        let right = right_start + rung;
        let next_left = if rung + 1 == rungs { exit } else { left + 1 };
        let next_right = if rung + 1 == rungs { exit } else { right + 1 };
        blocks.push(raw_conditional(left, next_left, right));
        blocks.push(branch(right, next_right));
    }
    blocks.push(returning(exit));
    function(blocks)
}

#[test]
fn computes_diamond_predecessors_reachability_and_dominators() {
    let analysis = analyze_control_flow(&function(vec![
        conditional(0, 1, 2),
        branch(2, 3),
        returning(3),
        branch(1, 3),
    ]))
    .unwrap();

    assert_eq!(analysis.entry(), BlockId(0));
    assert_eq!(analysis.blocks(), &ids(&[0, 1, 2, 3]));
    assert_eq!(analysis.reachable_blocks(), &ids(&[0, 1, 2, 3]));
    assert_eq!(analysis.predecessors(BlockId(0)), Some(&ids(&[])));
    assert_eq!(analysis.predecessors(BlockId(3)), Some(&ids(&[1, 2])));
    let legacy_signature: fn(
        &fe2o3_kernel_analysis::ControlFlowAnalysis,
        BlockId,
    ) -> Option<&BTreeSet<BlockId>> = fe2o3_kernel_analysis::ControlFlowAnalysis::dominators;
    assert_eq!(legacy_signature(&analysis, BlockId(1)), Some(&ids(&[0, 1])));
    assert_eq!(analysis.dominators(BlockId(2)), Some(&ids(&[0, 2])));
    assert_eq!(analysis.dominators(BlockId(3)), Some(&ids(&[0, 3])));
    assert!(analysis.dominates(BlockId(0), BlockId(3)));
    assert!(!analysis.dominates(BlockId(1), BlockId(3)));
    assert_eq!(analysis.immediate_dominator(BlockId(0)), Some(None));
    assert_eq!(
        analysis.immediate_dominator(BlockId(3)),
        Some(Some(BlockId(0)))
    );
    assert_eq!(
        analysis.dominator_tree_children(BlockId(0)),
        Some(&ids(&[1, 2, 3]))
    );
    assert_eq!(analysis.dominance_frontier(BlockId(0)), Some(&ids(&[])));
    assert_eq!(analysis.dominance_frontier(BlockId(1)), Some(&ids(&[3])));
    assert_eq!(analysis.dominance_frontier(BlockId(2)), Some(&ids(&[3])));
    assert_eq!(
        analysis.iterated_dominance_frontier(&ids(&[1, 2])),
        Some(ids(&[3]))
    );
    assert!(analysis.backedges().is_empty());
    assert!(analysis.natural_loop_headers().is_empty());
}

#[test]
fn preserves_predecessors_but_omits_dominance_for_unreachable_blocks() {
    let analysis =
        analyze_control_flow(&function(vec![returning(0), branch(10, 11), returning(11)])).unwrap();

    assert_eq!(analysis.reachable_blocks(), &ids(&[0]));
    assert!(!analysis.is_reachable(BlockId(10)));
    assert_eq!(analysis.predecessors(BlockId(11)), Some(&ids(&[10])));
    assert_eq!(analysis.dominators(BlockId(10)), None);
    assert!(!analysis.dominates(BlockId(10), BlockId(10)));
    assert_eq!(analysis.immediate_dominator(BlockId(10)), None);
    assert_eq!(analysis.dominator_tree_children(BlockId(10)), None);
    assert_eq!(analysis.dominance_frontier(BlockId(10)), None);
    assert_eq!(analysis.containing_natural_loops(BlockId(10)), None);
    assert_eq!(analysis.natural_loop_depth(BlockId(10)), None);
    assert_eq!(analysis.immediate_dominator(BlockId(99)), None);
    assert_eq!(analysis.iterated_dominance_frontier(&ids(&[10])), None);
    assert!(analysis.backedges().is_empty());
}

#[test]
fn classifies_natural_loop_backedges() {
    let analysis = analyze_control_flow(&function(vec![
        branch(0, 1),
        conditional(1, 2, 4),
        branch(2, 3),
        branch(3, 1),
        returning(4),
    ]))
    .unwrap();

    assert_eq!(analysis.dominators(BlockId(3)), Some(&ids(&[0, 1, 2, 3])));
    assert_eq!(
        analysis.backedges(),
        &BTreeSet::from([ControlFlowEdge::new(BlockId(3), BlockId(1))])
    );
    assert_eq!(analysis.natural_loop_headers(), &ids(&[1]));
    assert_eq!(
        analysis.natural_loop_body(BlockId(1)),
        Some(&ids(&[1, 2, 3]))
    );
    assert_eq!(analysis.natural_loop_latches(BlockId(1)), Some(&ids(&[3])));
    assert_eq!(analysis.natural_loop_roots(), &ids(&[1]));
    assert_eq!(analysis.natural_loop_parent(BlockId(1)), None);
    assert_eq!(analysis.natural_loop_children(BlockId(1)), Some(&ids(&[])));
    assert_eq!(
        analysis.containing_natural_loops(BlockId(2)),
        Some([BlockId(1)].as_slice())
    );
    assert_eq!(analysis.natural_loop_depth(BlockId(2)), Some(1));
}

#[test]
fn accepts_nested_natural_loops() {
    let analysis = analyze_control_flow(&function(vec![
        branch(0, 1),
        conditional(1, 2, 6),
        conditional(2, 3, 5),
        branch(3, 4),
        branch(4, 2),
        branch(5, 1),
        returning(6),
    ]))
    .unwrap();

    assert_eq!(
        analysis.backedges(),
        &BTreeSet::from([
            ControlFlowEdge::new(BlockId(4), BlockId(2)),
            ControlFlowEdge::new(BlockId(5), BlockId(1)),
        ])
    );
    assert_eq!(analysis.natural_loop_headers(), &ids(&[1, 2]));
    assert_eq!(
        analysis.natural_loop_body(BlockId(1)),
        Some(&ids(&[1, 2, 3, 4, 5]))
    );
    assert_eq!(
        analysis.natural_loop_body(BlockId(2)),
        Some(&ids(&[2, 3, 4]))
    );
    assert_eq!(analysis.natural_loop_latches(BlockId(1)), Some(&ids(&[5])));
    assert_eq!(analysis.natural_loop_latches(BlockId(2)), Some(&ids(&[4])));
    assert_eq!(analysis.natural_loop_roots(), &ids(&[1]));
    assert_eq!(analysis.natural_loop_parent(BlockId(1)), None);
    assert_eq!(analysis.natural_loop_parent(BlockId(2)), Some(BlockId(1)));
    assert_eq!(analysis.natural_loop_children(BlockId(1)), Some(&ids(&[2])));
    assert_eq!(analysis.natural_loop_children(BlockId(2)), Some(&ids(&[])));
    assert_eq!(
        analysis.containing_natural_loops(BlockId(3)),
        Some([BlockId(1), BlockId(2)].as_slice())
    );
    assert_eq!(analysis.natural_loop_depth(BlockId(3)), Some(2));
    assert_eq!(analysis.dominance_frontier(BlockId(2)), Some(&ids(&[1, 2])));
    let usage = analysis.resource_usage();
    assert_eq!(
        (
            usage.blocks(),
            usage.edges(),
            usage.natural_loops(),
            usage.natural_loop_body_memberships(),
            usage.work_units(),
        ),
        (7, 8, 2, 8, 214)
    );
}

#[test]
fn release_complexity_wire_scale_self_loops_hit_exact_resource_boundaries() {
    let block_count = u32::try_from(MAX_CONTROL_FLOW_BLOCKS).unwrap();
    let blocks = (0..block_count)
        .map(|id| {
            let next = if id + 1 == block_count { id } else { id + 1 };
            raw_conditional(id, id, next)
        })
        .collect();
    let mut input = function(blocks);

    let analysis = analyze_control_flow(&input).unwrap();
    let usage = analysis.resource_usage();
    assert_eq!(usage.blocks(), MAX_CONTROL_FLOW_BLOCKS);
    assert_eq!(usage.edges(), MAX_CONTROL_FLOW_EDGES);
    assert_eq!(usage.natural_loops(), MAX_CONTROL_FLOW_NATURAL_LOOPS);
    assert_eq!(
        usage.natural_loop_body_memberships(),
        MAX_CONTROL_FLOW_LOOP_BODY_MEMBERSHIPS
    );
    assert_eq!(usage.work_units(), 2_555_892);
    assert!(usage.work_units() < MAX_CONTROL_FLOW_WORK_UNITS);

    input
        .body
        .as_mut()
        .unwrap()
        .blocks
        .last_mut()
        .unwrap()
        .terminator = Some(Terminator::Switch {
        selector: ValueId(0),
        cases: vec![
            SwitchCase {
                value: 0,
                target: BlockId(block_count - 1),
                arguments: vec![],
            },
            SwitchCase {
                value: 1,
                target: BlockId(block_count - 1),
                arguments: vec![],
            },
        ],
        default_target: BlockId(block_count - 1),
        default_arguments: vec![],
    });
    assert_eq!(
        analyze_control_flow(&input).unwrap_err().diagnostics(),
        &[ControlFlowDiagnostic::ResourceLimitExceeded {
            resource: ControlFlowResource::Edges,
            required: MAX_CONTROL_FLOW_EDGES + 1,
            limit: MAX_CONTROL_FLOW_EDGES,
            storage_items: 0,
            work_units: 0,
        }]
    );

    input
        .body
        .as_mut()
        .unwrap()
        .blocks
        .push(returning(block_count));
    assert_eq!(
        analyze_control_flow(&input).unwrap_err().diagnostics(),
        &[ControlFlowDiagnostic::ResourceLimitExceeded {
            resource: ControlFlowResource::Blocks,
            required: MAX_CONTROL_FLOW_BLOCKS + 1,
            limit: MAX_CONTROL_FLOW_BLOCKS,
            storage_items: 0,
            work_units: 0,
        }]
    );
}

#[test]
fn release_complexity_shared_multi_latch_body_is_walked_once() {
    let chain_count = 2_048_u32;
    let latch_count = 2_048_u32;
    let split = 2 + chain_count;
    let latch_start = split + 1;
    let exit = latch_start + latch_count;
    let mut blocks = vec![branch(0, 1), raw_conditional(1, 2, exit)];
    blocks.extend((0..chain_count).map(|offset| {
        let id = 2 + offset;
        branch(
            id,
            if offset + 1 == chain_count {
                split
            } else {
                id + 1
            },
        )
    }));
    blocks.push(block(
        split,
        Terminator::Switch {
            selector: ValueId(0),
            cases: (0..latch_count - 1)
                .map(|offset| SwitchCase {
                    value: u64::from(offset),
                    target: BlockId(latch_start + offset),
                    arguments: vec![],
                })
                .collect(),
            default_target: BlockId(latch_start + latch_count - 1),
            default_arguments: vec![],
        },
    ));
    blocks.extend((0..latch_count).map(|offset| branch(latch_start + offset, 1)));
    blocks.push(returning(exit));

    let analysis = analyze_control_flow(&function(blocks)).unwrap();
    assert_eq!(analysis.natural_loop_headers(), &ids(&[1]));
    assert_eq!(
        analysis.natural_loop_latches(BlockId(1)).unwrap().len(),
        latch_count as usize
    );
    assert_eq!(
        analysis.natural_loop_body(BlockId(1)).unwrap().len(),
        (chain_count + latch_count + 2) as usize
    );
    let usage = analysis.resource_usage();
    assert_eq!(
        (
            usage.blocks(),
            usage.edges(),
            usage.natural_loops(),
            usage.natural_loop_body_memberships(),
            usage.work_units(),
        ),
        (4_100, 6_147, 1, 4_098, 4_327_505)
    );
}

#[test]
fn release_complexity_frontier_ladder_accepts_boundary_and_rejects_next_entry() {
    let boundary = analyze_control_flow(&two_arm_frontier_ladder(512)).unwrap();
    let usage = boundary.resource_usage();
    assert_eq!(
        usage.dominance_frontier_entries(),
        MAX_CONTROL_FLOW_DOMINANCE_FRONTIER_ENTRIES
    );
    assert_eq!(usage.storage_items(), 137_482);
    assert_eq!(usage.work_units(), 419_104);

    let error = analyze_control_flow(&two_arm_frontier_ladder(513)).unwrap_err();
    assert_eq!(
        error.diagnostics(),
        &[ControlFlowDiagnostic::ResourceLimitExceeded {
            resource: ControlFlowResource::DominanceFrontierEntries,
            required: MAX_CONTROL_FLOW_DOMINANCE_FRONTIER_ENTRIES + 1,
            limit: MAX_CONTROL_FLOW_DOMINANCE_FRONTIER_ENTRIES,
            storage_items: 137_493,
            work_units: 409_897,
        }]
    );
    assert_eq!(
        error.to_string(),
        "control-flow analysis of cfg failed with 1 diagnostic(s)\n  dominance-frontier entries require 132353 items, exceeding the deterministic limit 132352; aggregate storage 137493, aggregate work 409897\n"
    );
}

#[test]
fn release_complexity_reviewer_8192_rung_ladder_rejects_before_frontier_growth() {
    let error = analyze_control_flow(&two_arm_frontier_ladder(8_192)).unwrap_err();
    assert_eq!(
        error.diagnostics(),
        &[ControlFlowDiagnostic::ResourceLimitExceeded {
            resource: ControlFlowResource::WorkUnits,
            required: MAX_CONTROL_FLOW_WORK_UNITS + 1,
            limit: MAX_CONTROL_FLOW_WORK_UNITS,
            storage_items: 32_772,
            work_units: MAX_CONTROL_FLOW_WORK_UNITS + 1,
        }]
    );
}

#[test]
fn analysis_is_deterministic_across_non_entry_block_order() {
    let first = analyze_control_flow(&function(vec![
        conditional(0, 1, 2),
        branch(1, 3),
        branch(2, 3),
        returning(3),
    ]))
    .unwrap();
    let reordered = analyze_control_flow(&function(vec![
        conditional(0, 1, 2),
        returning(3),
        branch(2, 3),
        branch(1, 3),
    ]))
    .unwrap();

    assert_eq!(first, reordered);
    assert_eq!(
        first.iterated_dominance_frontier(&ids(&[1, 2])),
        reordered.iterated_dominance_frontier(&ids(&[2, 1]))
    );
}

#[test]
fn handles_self_loop_and_sparse_boundary_block_ids() {
    let self_loop =
        analyze_control_flow(&function(vec![conditional(42, 42, 99), returning(99)])).unwrap();

    assert_eq!(self_loop.immediate_dominator(BlockId(42)), Some(None));
    assert_eq!(self_loop.dominance_frontier(BlockId(42)), Some(&ids(&[42])));
    assert_eq!(self_loop.natural_loop_headers(), &ids(&[42]));
    assert_eq!(self_loop.natural_loop_body(BlockId(42)), Some(&ids(&[42])));
    assert_eq!(
        self_loop.natural_loop_latches(BlockId(42)),
        Some(&ids(&[42]))
    );

    let high = u32::MAX;
    let sparse = analyze_control_flow(&function(vec![
        branch(high - 1, 7),
        branch(7, high),
        returning(high),
    ]))
    .unwrap();
    assert_eq!(sparse.entry(), BlockId(high - 1));
    assert_eq!(
        sparse.immediate_dominator(BlockId(high)),
        Some(Some(BlockId(7)))
    );
    assert_eq!(sparse.dominance_frontier(BlockId(high)), Some(&ids(&[])));
    assert_eq!(
        sparse.containing_natural_loops(BlockId(high)),
        Some([].as_slice())
    );
}

#[test]
fn rejects_two_entry_irreducible_scc_deterministically() {
    let error = analyze_control_flow(&function(vec![
        conditional(0, 1, 2),
        branch(1, 2),
        conditional(2, 1, 3),
        returning(3),
    ]))
    .unwrap_err();

    assert_eq!(
        error.diagnostics(),
        &[ControlFlowDiagnostic::IrreducibleControlFlow {
            blocks: vec![BlockId(1), BlockId(2)],
            entry_edges: vec![
                ControlFlowEdge::new(BlockId(0), BlockId(1)),
                ControlFlowEdge::new(BlockId(0), BlockId(2)),
            ],
        }]
    );
    assert_eq!(
        error.to_string(),
        "control-flow analysis of cfg failed with 1 diagnostic(s)\n  irreducible control flow in blocks bb1, bb2; entry edges: bb0 -> bb1, bb0 -> bb2\n"
    );
}

#[test]
fn rejects_irreducible_inner_cycle_beneath_a_natural_loop_header() {
    let error = analyze_control_flow(&function(vec![
        branch(0, 1),
        conditional(1, 2, 3),
        branch(2, 3),
        conditional(3, 2, 1),
    ]))
    .unwrap_err();

    assert_eq!(
        error.diagnostics(),
        &[ControlFlowDiagnostic::IrreducibleControlFlow {
            blocks: vec![BlockId(2), BlockId(3)],
            entry_edges: vec![
                ControlFlowEdge::new(BlockId(1), BlockId(2)),
                ControlFlowEdge::new(BlockId(1), BlockId(3)),
            ],
        }]
    );
}

#[test]
fn unreachable_irreducible_region_does_not_reject_executable_cfg() {
    let analysis = analyze_control_flow(&function(vec![
        returning(0),
        branch(10, 11),
        branch(11, 10),
    ]))
    .unwrap();

    assert_eq!(analysis.reachable_blocks(), &ids(&[0]));
}

#[test]
fn malformed_cfg_diagnostics_are_sorted_and_deduplicated() {
    let mut missing = BasicBlock::new(BlockId(4));
    missing.operations.clear();
    let error = analyze_control_flow(&function(vec![
        conditional(0, 9, 9),
        returning(2),
        returning(2),
        missing,
    ]))
    .unwrap_err();

    assert_eq!(
        error.diagnostics(),
        &[
            ControlFlowDiagnostic::DuplicateBlock { block: BlockId(2) },
            ControlFlowDiagnostic::MissingTerminator { block: BlockId(4) },
            ControlFlowDiagnostic::UnknownSuccessor {
                edge: ControlFlowEdge::new(BlockId(0), BlockId(9)),
            },
        ]
    );
}

#[test]
fn declarations_and_empty_definitions_fail_closed() {
    let declaration = Function::declaration("decl", Signature::new(vec![], vec![]));
    assert_eq!(
        analyze_control_flow(&declaration)
            .unwrap_err()
            .diagnostics(),
        &[ControlFlowDiagnostic::FunctionDeclaration]
    );

    let empty = function(vec![]);
    assert_eq!(
        analyze_control_flow(&empty).unwrap_err().diagnostics(),
        &[ControlFlowDiagnostic::EmptyFunction]
    );
}
