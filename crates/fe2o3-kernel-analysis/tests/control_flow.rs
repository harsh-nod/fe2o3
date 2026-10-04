use fe2o3_kernel_analysis::{
    ControlFlowDiagnostic, ControlFlowDiagnosticV2, ControlFlowEdge, ControlFlowResource,
    MAX_CONTROL_FLOW_BLOCKS, MAX_CONTROL_FLOW_DOMINANCE_FRONTIER_ENTRIES, MAX_CONTROL_FLOW_EDGES,
    MAX_CONTROL_FLOW_LOOP_BODY_MEMBERSHIPS, MAX_CONTROL_FLOW_NATURAL_LOOPS,
    MAX_CONTROL_FLOW_WORK_UNITS, analyze_control_flow,
};
use fe2o3_kernel_ir::{
    BasicBlock, BlockId, Constant, Function, Operation, OperationKind, Signature, SwitchCase,
    Terminator, Type, ValueDef, ValueId,
};
use std::collections::{BTreeSet, VecDeque};

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

fn graph(rows: &[&[usize]]) -> Vec<BTreeSet<usize>> {
    rows.iter()
        .map(|targets| targets.iter().copied().collect())
        .collect()
}

fn graph_function(successors: &[BTreeSet<usize>]) -> Function {
    function(
        successors
            .iter()
            .enumerate()
            .map(|(source, targets)| {
                let source = u32::try_from(source).unwrap();
                let mut targets = targets.iter().copied();
                match (targets.next(), targets.next(), targets.next()) {
                    (None, None, None) => returning(source),
                    (Some(target), None, None) => branch(source, u32::try_from(target).unwrap()),
                    (Some(left), Some(right), None) => raw_conditional(
                        source,
                        u32::try_from(left).unwrap(),
                        u32::try_from(right).unwrap(),
                    ),
                    _ => panic!("test graph has more than two successors"),
                }
            })
            .collect(),
    )
}

fn reference_dominators(successors: &[BTreeSet<usize>]) -> (BTreeSet<usize>, Vec<BTreeSet<usize>>) {
    let mut reachable = BTreeSet::new();
    let mut pending = VecDeque::from([0]);
    while let Some(block) = pending.pop_front() {
        if reachable.insert(block) {
            pending.extend(successors[block].iter().copied());
        }
    }

    let mut dominators = vec![BTreeSet::new(); successors.len()];
    for block in &reachable {
        dominators[*block] = if *block == 0 {
            BTreeSet::from([0])
        } else {
            reachable.clone()
        };
    }

    loop {
        let mut changed = false;
        for block in reachable.iter().copied().filter(|block| *block != 0) {
            let mut predecessors = reachable
                .iter()
                .copied()
                .filter(|predecessor| successors[*predecessor].contains(&block));
            let first = predecessors
                .next()
                .expect("every reachable non-entry block has a reachable predecessor");
            let mut next = dominators[first].clone();
            for predecessor in predecessors {
                next = next
                    .intersection(&dominators[predecessor])
                    .copied()
                    .collect();
            }
            next.insert(block);
            if dominators[block] != next {
                dominators[block] = next;
                changed = true;
            }
        }
        if !changed {
            return (reachable, dominators);
        }
    }
}

fn reference_is_reducible(
    successors: &[BTreeSet<usize>],
    reachable: &BTreeSet<usize>,
    dominators: &[BTreeSet<usize>],
) -> bool {
    let mut indegree = vec![0_usize; successors.len()];
    for source in reachable {
        for target in &successors[*source] {
            if reachable.contains(target) && !dominators[*source].contains(target) {
                indegree[*target] += 1;
            }
        }
    }

    let mut pending = reachable
        .iter()
        .copied()
        .filter(|block| indegree[*block] == 0)
        .collect::<BTreeSet<_>>();
    let mut visited = 0_usize;
    while let Some(block) = pending.pop_first() {
        visited += 1;
        for target in &successors[block] {
            if reachable.contains(target) && !dominators[block].contains(target) {
                indegree[*target] -= 1;
                if indegree[*target] == 0 {
                    pending.insert(*target);
                }
            }
        }
    }
    visited == reachable.len()
}

fn assert_matches_reference(successors: &[BTreeSet<usize>]) -> (usize, bool) {
    let (reachable, dominators) = reference_dominators(successors);
    let reducible = reference_is_reducible(successors, &reachable, &dominators);
    let result = analyze_control_flow(&graph_function(successors));
    if reducible {
        let analysis = result.expect("reference-reducible graph must be accepted");
        assert_eq!(
            analysis.reachable_blocks(),
            &reachable
                .iter()
                .map(|block| BlockId(u32::try_from(*block).unwrap()))
                .collect()
        );
        for (block, expected_dominators) in dominators.iter().enumerate() {
            let block_id = BlockId(u32::try_from(block).unwrap());
            if reachable.contains(&block) {
                let expected = expected_dominators
                    .iter()
                    .map(|dominator| BlockId(u32::try_from(*dominator).unwrap()))
                    .collect::<BTreeSet<_>>();
                assert_eq!(analysis.dominators(block_id), Some(&expected));
            } else {
                assert_eq!(analysis.dominators(block_id), None);
            }
        }
    } else {
        let error = result.expect_err("reference-irreducible graph must be rejected");
        assert!(!error.diagnostics().is_empty());
        assert!(error.diagnostics().iter().all(|diagnostic| matches!(
            diagnostic,
            ControlFlowDiagnostic::IrreducibleControlFlow { .. }
        )));
    }
    (reachable.len(), reducible)
}

#[derive(Clone, Copy)]
struct OracleRng(u64);

impl OracleRng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }

    fn index(&mut self, upper: usize) -> usize {
        usize::try_from(self.next() % u64::try_from(upper).unwrap()).unwrap()
    }
}

fn legacy_diagnostic_kind(diagnostic: &ControlFlowDiagnostic) -> u8 {
    match diagnostic {
        ControlFlowDiagnostic::FunctionDeclaration => 0,
        ControlFlowDiagnostic::EmptyFunction => 1,
        ControlFlowDiagnostic::DuplicateBlock { .. } => 2,
        ControlFlowDiagnostic::MissingTerminator { .. } => 3,
        ControlFlowDiagnostic::UnknownSuccessor { .. } => 4,
        ControlFlowDiagnostic::IrreducibleControlFlow { .. } => 5,
    }
}

#[test]
fn legacy_diagnostic_enum_remains_exhaustively_matchable() {
    assert_eq!(
        legacy_diagnostic_kind(&ControlFlowDiagnostic::EmptyFunction),
        1
    );
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
fn ignores_unreachable_predecessors_during_chk_intersection() {
    let exact = function(vec![
        conditional(0, 3, 5),
        branch(1, 1),
        branch(2, 3),
        branch(3, 3),
        branch(4, 4),
        returning(5),
    ]);
    let analysis = analyze_control_flow(&exact).unwrap();

    assert_eq!(analysis.reachable_blocks(), &ids(&[0, 3, 5]));
    assert_eq!(analysis.predecessors(BlockId(3)), Some(&ids(&[0, 2, 3])));
    assert_eq!(analysis.dominators(BlockId(3)), Some(&ids(&[0, 3])));
    assert_eq!(analysis.dominators(BlockId(5)), Some(&ids(&[0, 5])));
    assert_eq!(analysis.natural_loop_headers(), &ids(&[3]));

    let variants = [
        graph(&[&[3, 5], &[2], &[1, 3], &[3], &[1, 5], &[]]),
        graph(&[&[5], &[2, 3], &[1], &[4, 5], &[3], &[]]),
        graph(&[&[3], &[2], &[1, 3], &[3, 5], &[3], &[]]),
    ];
    for successors in variants {
        let (reachable, _) = assert_matches_reference(&successors);
        assert!(reachable < successors.len());
    }
}

#[test]
fn arbitrary_cfgs_with_unreachable_regions_match_reference() {
    const GRAPH_COUNT: usize = 50_000;
    let mut rng = OracleRng(0x9e37_79b9_7f4a_7c15);
    let mut graphs_with_unreachable_blocks = 0_usize;
    let mut reducible_graphs = 0_usize;
    let mut irreducible_graphs = 0_usize;

    for _ in 0..GRAPH_COUNT {
        let block_count = 1 + rng.index(8);
        let mut successors = Vec::with_capacity(block_count);
        for _ in 0..block_count {
            let targets = match rng.index(4) {
                0 => BTreeSet::new(),
                1 => BTreeSet::from([rng.index(block_count)]),
                _ => BTreeSet::from([rng.index(block_count), rng.index(block_count)]),
            };
            successors.push(targets);
        }

        let (reachable, reducible) = assert_matches_reference(&successors);
        graphs_with_unreachable_blocks += usize::from(reachable < block_count);
        reducible_graphs += usize::from(reducible);
        irreducible_graphs += usize::from(!reducible);
    }

    assert!(graphs_with_unreachable_blocks > GRAPH_COUNT / 4);
    assert!(reducible_graphs > 0);
    assert!(irreducible_graphs > 0);
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
        analyze_control_flow(&input).unwrap_err().diagnostics_v2(),
        &[ControlFlowDiagnosticV2::ResourceLimitExceeded {
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
        analyze_control_flow(&input).unwrap_err().diagnostics_v2(),
        &[ControlFlowDiagnosticV2::ResourceLimitExceeded {
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
        error.diagnostics_v2(),
        &[ControlFlowDiagnosticV2::ResourceLimitExceeded {
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
        error.diagnostics_v2(),
        &[ControlFlowDiagnosticV2::ResourceLimitExceeded {
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
