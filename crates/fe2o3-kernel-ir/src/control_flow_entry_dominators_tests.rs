use super::*;
use crate::{BasicBlock, Signature, SwitchCase, Type, ValueDef};

fn block_id(position: usize) -> BlockId {
    BlockId(u32::MAX - u32::try_from(position).unwrap() * 65_537)
}

fn function(successors: &[Vec<usize>]) -> Function {
    let blocks = successors
        .iter()
        .enumerate()
        .map(|(position, targets)| {
            let mut block = BasicBlock::new(block_id(position));
            block.terminator = Some(match targets.as_slice() {
                [] => Terminator::Return { values: vec![] },
                [target] => Terminator::Branch {
                    target: block_id(*target),
                    arguments: vec![],
                },
                [cases @ .., default] => Terminator::Switch {
                    selector: ValueId(0),
                    cases: cases
                        .iter()
                        .enumerate()
                        .map(|(ordinal, target)| SwitchCase {
                            value: ordinal as u64,
                            target: block_id(*target),
                            arguments: vec![],
                        })
                        .collect(),
                    default_target: block_id(*default),
                    default_arguments: vec![],
                },
            });
            block
        })
        .collect();
    Function::definition(
        "entry_dominators",
        Signature::new(vec![], vec![]),
        vec![],
        blocks,
    )
}

fn set_oracle(successors: &[Vec<usize>]) -> (Vec<bool>, Vec<Vec<bool>>) {
    let blocks = successors.len();
    let mut reachable = vec![false; blocks];
    let mut pending = vec![0];
    reachable[0] = true;
    while let Some(block) = pending.pop() {
        for successor in successors[block].iter().copied() {
            if !reachable[successor] {
                reachable[successor] = true;
                pending.push(successor);
            }
        }
    }

    // Independent set fixed point, with no RPO, tree intervals or IDOM climbs.
    let mut dominators = vec![vec![false; blocks]; blocks];
    dominators[0][0] = true;
    for block in 1..blocks {
        if reachable[block] {
            dominators[block] = reachable.clone();
        }
    }
    loop {
        let mut changed = false;
        for block in 1..blocks {
            if !reachable[block] {
                continue;
            }
            let mut next = reachable.clone();
            for predecessor in 0..blocks {
                if reachable[predecessor] && successors[predecessor].contains(&block) {
                    for definition in 0..blocks {
                        next[definition] &= dominators[predecessor][definition];
                    }
                }
            }
            next[block] = true;
            if next != dominators[block] {
                dominators[block] = next;
                changed = true;
            }
        }
        if !changed {
            return (reachable, dominators);
        }
    }
}

fn assert_set_oracle(successors: &[Vec<usize>]) -> IndexedControlFlow {
    let (reachable, dominators) = set_oracle(successors);
    let flow = analyze_control_flow(&function(successors)).unwrap();
    for block in 0..successors.len() {
        assert_eq!(flow.is_reachable(block_id(block)), reachable[block]);
        for (definition, dominates) in dominators[block].iter().copied().enumerate() {
            let expected = if reachable[block] {
                dominates
            } else {
                definition == block
            };
            assert_eq!(
                flow.dominates(block_id(definition), block_id(block)),
                expected,
                "definition={definition}, block={block}, successors={successors:?}",
            );
        }
    }
    flow
}

fn fan_in(chain: usize, anchor: usize) -> Vec<Vec<usize>> {
    let exit = chain + 1;
    let mut successors = vec![vec![]; exit + 1];
    for (block, targets) in successors.iter_mut().enumerate().take(exit) {
        targets.push(block + 1);
        if block >= anchor && block + 1 != exit {
            targets.push(exit);
        }
    }
    successors
}

fn work_error(limit: u64, actual: u64) -> ControlFlowError {
    ControlFlowError::ResourceLimit {
        resource: ControlFlowResource::AnalysisWork,
        limit,
        actual,
    }
}

#[test]
fn all_512_three_block_graphs_match_independent_set_oracle() {
    for edges in 0_u16..512 {
        let successors = (0..3)
            .map(|source| {
                (0..3)
                    .filter(|&target| edges & (1 << (source * 3 + target)) != 0)
                    .collect()
            })
            .collect::<Vec<Vec<usize>>>();
        assert_set_oracle(&successors);
    }
}

#[test]
fn entry_and_non_entry_high_fan_in_match_independent_set_oracle() {
    for chain in [1_usize, 2, 8, 64] {
        let flow = assert_set_oracle(&fan_in(chain, 0));
        assert_eq!(flow.work().dominator_climbs, 0);
        assert_eq!(
            flow.work().dominator_predecessor_visits,
            2 * (chain as u64 + 1)
        );
        assert!(flow.dominates(block_id(0), block_id(chain + 1)));
        assert!(!flow.dominates(block_id(1), block_id(chain + 1)));

        let flow = assert_set_oracle(&fan_in(chain, 1));
        assert!(flow.dominates(block_id(1), block_id(chain + 1)));
        if chain > 1 {
            assert!(flow.work().dominator_climbs > 0);
            assert!(!flow.dominates(block_id(2), block_id(chain + 1)));
        }
    }
}

#[test]
fn late_entry_intersection_unreachable_predecessors_and_irreducible_loops_match_oracle() {
    let late_entry = vec![vec![1, 2], vec![3, 5], vec![5], vec![4, 5], vec![5], vec![]];
    let flow = assert_set_oracle(&late_entry);
    assert_eq!(flow.predecessors[5], [1, 2, 3, 4]);
    assert!(!flow.dominates(block_id(1), block_id(5)));

    let unreachable = vec![vec![2], vec![5], vec![3, 4], vec![5], vec![5], vec![]];
    let flow = assert_set_oracle(&unreachable);
    assert_eq!(flow.predecessors[5], [1, 3, 4]);
    assert!(flow.dominates(block_id(2), block_id(5)));

    let irreducible = vec![vec![1, 2], vec![2], vec![1], vec![3, 1]];
    let flow = assert_set_oracle(&irreducible);
    assert_eq!(flow.irreducible_blocks(), [block_id(2), block_id(1)]);
}

#[test]
fn entry_fan_in_has_exact_legacy_idom_work_and_one_under_limit() {
    for chain in [0_usize, 1, 8, 512, 4_094] {
        let blocks = chain + 2;
        let mut predecessors = vec![vec![]; blocks];
        for (block, incoming) in predecessors.iter_mut().enumerate().take(chain + 1).skip(1) {
            incoming.push(block - 1);
        }
        predecessors[chain + 1] = (0..=chain).collect();
        let reachable = vec![true; blocks];
        let reverse_postorder = (0..blocks).collect::<Vec<_>>();

        // Two sweeps each inspect one predecessor per non-entry block, no climbs.
        let exact_work = 2 * (chain as u64 + 1);
        for limit in [exact_work, exact_work - 1, 0] {
            let mut meter = WorkMeter::new(limit);
            let result = compute_immediate_dominators(
                &predecessors,
                &reachable,
                &reverse_postorder,
                &mut meter,
            );
            assert_eq!(meter.work.dominator_climbs, 0);
            if limit < exact_work {
                assert_eq!(result, Err(work_error(limit, limit + 1)));
                assert_eq!(meter.work.total, limit + 1);
                assert_eq!(meter.work.dominator_predecessor_visits, limit + 1);
            } else {
                let dominators = result.unwrap();
                assert_eq!(dominators[0], None);
                assert_eq!(
                    &dominators[1..chain + 1],
                    &(0..chain).map(Some).collect::<Vec<_>>()
                );
                assert_eq!(dominators[chain + 1], Some(0));
                assert_eq!(meter.work.dominator_predecessor_visits, exact_work);
                assert_eq!(meter.work.total, exact_work);
            }
        }
    }
}

#[test]
fn entry_reached_after_intersection_stops_before_later_predecessors() {
    let predecessors = vec![vec![], vec![0], vec![0], vec![1], vec![3], vec![1, 2, 3, 4]];
    let reachable = vec![true; 6];
    let reverse_postorder = [0, 2, 1, 3, 4, 5];
    // Each sweep visits four single predecessors and two join predecessors,
    // then climbs from both join arms to entry: 2 * (6 + 2) = 16 units.
    for limit in [16, 15] {
        let mut meter = WorkMeter::new(limit);
        let result =
            compute_immediate_dominators(&predecessors, &reachable, &reverse_postorder, &mut meter);
        assert_eq!(meter.work.dominator_predecessor_visits, 12);
        assert_eq!(meter.work.dominator_climbs, 4);
        assert_eq!(meter.work.total, 16);
        if limit == 16 {
            assert_eq!(
                result.unwrap(),
                [None, Some(0), Some(0), Some(1), Some(3), Some(0)]
            );
        } else {
            assert_eq!(result, Err(work_error(15, 16)));
        }
    }
}

#[test]
fn entry_fan_in_has_exact_full_cfg_work_and_failure_prefixes() {
    for chain in [0_usize, 1, 8, 512] {
        let function = function(&fan_in(chain, 0));
        let n = chain as u64;
        // B=N+2, E=2N+1, no phi inputs. Index B+2E, three edge sweeps,
        // two IDOM sweeps, two interval visits and one reducibility visit per B.
        let exact_work = 16 * n + 15;
        let limits = ControlFlowLimits {
            analysis_work: exact_work,
            ..ControlFlowLimits::DEFAULT
        };
        let flow = analyze_control_flow_with_limits(&function, limits).unwrap();
        assert_eq!(
            flow.work(),
            ControlFlowWork {
                index_units: 5 * n + 4,
                reachability_edge_visits: 2 * n + 1,
                depth_first_edge_visits: 2 * n + 1,
                dominator_predecessor_visits: 2 * n + 2,
                dominator_climbs: 0,
                interval_node_visits: 2 * n + 4,
                reducibility_edge_visits: 2 * n + 1,
                reducibility_node_visits: n + 2,
                total: exact_work,
            }
        );
        for (limit, actual) in [
            (exact_work - 1, exact_work),
            (5 * n + 3, 5 * n + 4),
            (0, 5 * n + 4),
        ] {
            assert_eq!(
                analyze_control_flow_with_limits(
                    &function,
                    ControlFlowLimits {
                        analysis_work: limit,
                        ..limits
                    }
                ),
                Err(work_error(limit, actual)),
            );
        }
        assert_eq!(analyze_control_flow(&function).unwrap(), flow);
    }
}

#[test]
fn absorbing_entry_preserves_duplicate_edges_arguments_and_phi_accounting() {
    let successors = fan_in(8, 0)
        .into_iter()
        .map(|targets| targets.into_iter().flat_map(|target| [target; 3]).collect())
        .collect::<Vec<Vec<usize>>>();
    assert_set_oracle(&successors);
    let mut function = function(&successors);
    for block in &mut function.body.as_mut().unwrap().blocks {
        block
            .parameters
            .push(ValueDef::new(ValueId(1), Type::INDEX));
        if let Some(Terminator::Switch {
            cases,
            default_arguments,
            ..
        }) = &mut block.terminator
        {
            for case in cases {
                case.arguments.push(ValueId(0));
            }
            default_arguments.push(ValueId(0));
        }
    }
    let limits = ControlFlowLimits {
        blocks: 10,
        edges: 51,
        edge_arguments: 51,
        phi_inputs: 51,
        analysis_work: 262,
    };
    let flow = analyze_control_flow_with_limits(&function, limits).unwrap();
    assert_eq!(flow.edge_count(), 51);
    assert_eq!(flow.edge_argument_count(), 51);
    assert_eq!(flow.phi_input_count(), 51);
    assert_eq!(flow.incoming_edges(block_id(9)).unwrap().len(), 27);
    assert_eq!(flow.predecessors[9], (0..9).collect::<Vec<_>>());
    assert_eq!(flow.work().dominator_predecessor_visits, 18);
    assert_eq!(flow.work().dominator_climbs, 0);
    // Baseline 143 + 2 * 34 extra edge rows + 51 phi inputs.
    assert_eq!(flow.work().index_units, 163);
    assert_eq!(flow.work().total, 262);
    for (limits, resource) in [
        (
            ControlFlowLimits {
                edges: 50,
                ..limits
            },
            ControlFlowResource::Edges,
        ),
        (
            ControlFlowLimits {
                edge_arguments: 50,
                ..limits
            },
            ControlFlowResource::EdgeArguments,
        ),
        (
            ControlFlowLimits {
                phi_inputs: 50,
                ..limits
            },
            ControlFlowResource::PhiInputs,
        ),
    ] {
        assert_eq!(
            analyze_control_flow_with_limits(&function, limits),
            Err(ControlFlowError::ResourceLimit {
                resource,
                limit: 50,
                actual: 51,
            })
        );
    }
}

#[test]
fn absorbing_entry_does_not_skip_late_unknown_reference_validation() {
    let mut function = function(&fan_in(8, 0));
    function.body.as_mut().unwrap().blocks[7].terminator = Some(Terminator::Switch {
        selector: ValueId(0),
        cases: vec![SwitchCase {
            value: 0,
            target: block_id(9),
            arguments: vec![],
        }],
        default_target: BlockId(0),
        default_arguments: vec![],
    });
    let expected = ControlFlowError::UnknownSuccessor {
        source: block_id(7),
        target: BlockId(0),
    };
    for analysis_work in [0, ControlFlowLimits::DEFAULT.analysis_work] {
        assert_eq!(
            analyze_control_flow_with_limits(
                &function,
                ControlFlowLimits {
                    analysis_work,
                    ..ControlFlowLimits::DEFAULT
                }
            ),
            Err(expected.clone()),
        );
    }
}

#[test]
fn absorbing_entry_preserves_duplicate_block_and_missing_terminator_errors() {
    let mut duplicate = function(&fan_in(8, 0));
    duplicate.body.as_mut().unwrap().blocks[7].id = block_id(0);
    assert_eq!(
        analyze_control_flow(&duplicate),
        Err(ControlFlowError::DuplicateBlock(block_id(0)))
    );

    let mut missing = function(&fan_in(8, 0));
    missing.body.as_mut().unwrap().blocks[7].terminator = None;
    assert_eq!(
        analyze_control_flow(&missing),
        Err(ControlFlowError::MissingTerminator(block_id(7)))
    );
}
