use std::collections::BTreeSet;

use dialect_mir::{
    MirBasicBlock, MirBlockId, MirBody, MirBodyForm, MirControlFlowEdge, MirControlFlowError,
    MirEdge, MirTerminator, MirTerminatorKind, analyze_mir_control_flow,
};

fn edge(target: u32) -> MirEdge {
    MirEdge::new(MirBlockId(target))
}

fn block(kind: MirTerminatorKind) -> MirBasicBlock {
    MirBasicBlock {
        parameters: vec![],
        statements: vec![],
        terminator: MirTerminator { kind, span: None },
    }
}

fn goto(target: u32) -> MirBasicBlock {
    block(MirTerminatorKind::Goto(edge(target)))
}

fn branch(then_target: u32, else_target: u32) -> MirBasicBlock {
    block(MirTerminatorKind::SwitchInt {
        discr: dialect_mir::MirOperand::Constant(dialect_mir::MirConstant {
            ty: dialect_mir::MirTypeId(0),
            value: dialect_mir::MirConstantValue::Bool(true),
        }),
        targets: vec![(1, edge(then_target))],
        otherwise: edge(else_target),
    })
}

fn returning() -> MirBasicBlock {
    block(MirTerminatorKind::Return)
}

fn body(blocks: Vec<MirBasicBlock>) -> MirBody {
    MirBody {
        form: MirBodyForm::Places,
        locals: vec![],
        blocks,
        entry: MirBlockId(0),
    }
}

fn ids(values: &[u32]) -> BTreeSet<MirBlockId> {
    values.iter().copied().map(MirBlockId).collect()
}

#[test]
fn computes_diamond_dominance_and_frontiers() {
    let analysis =
        analyze_mir_control_flow(&body(vec![branch(1, 2), goto(3), goto(3), returning()])).unwrap();

    assert_eq!(analysis.predecessors(MirBlockId(3)), Some(&ids(&[1, 2])));
    assert_eq!(analysis.dominators(MirBlockId(3)), Some(&ids(&[0, 3])));
    assert_eq!(
        analysis.immediate_dominator(MirBlockId(3)),
        Some(Some(MirBlockId(0)))
    );
    assert_eq!(
        analysis.dominator_tree_children(MirBlockId(0)),
        Some(&ids(&[1, 2, 3]))
    );
    assert_eq!(analysis.dominance_frontier(MirBlockId(1)), Some(&ids(&[3])));
    assert_eq!(
        analysis.iterated_dominance_frontier(&ids(&[1, 2])),
        Some(ids(&[3]))
    );
}

#[test]
fn recognizes_nested_break_continue_loops() {
    let analysis = analyze_mir_control_flow(&body(vec![
        goto(1),
        branch(2, 7),
        branch(3, 6),
        branch(4, 5),
        goto(2),
        goto(1),
        goto(1),
        returning(),
    ]))
    .unwrap();

    assert_eq!(
        analysis.backedges(),
        &BTreeSet::from([
            MirControlFlowEdge {
                source: MirBlockId(4),
                target: MirBlockId(2),
            },
            MirControlFlowEdge {
                source: MirBlockId(5),
                target: MirBlockId(1),
            },
            MirControlFlowEdge {
                source: MirBlockId(6),
                target: MirBlockId(1),
            },
        ])
    );
    assert_eq!(
        analysis.loop_headers().collect::<Vec<_>>(),
        [MirBlockId(1), MirBlockId(2)]
    );
    assert_eq!(analysis.loop_body(MirBlockId(2)), Some(&ids(&[2, 3, 4])));
    assert_eq!(analysis.loop_latches(MirBlockId(1)), Some(&ids(&[5, 6])));
}

#[test]
fn dead_and_irreducible_control_flow_fail_with_stable_diagnostics() {
    let dead = analyze_mir_control_flow(&body(vec![returning(), returning()])).unwrap_err();
    assert_eq!(dead, MirControlFlowError::UnreachableBlock(MirBlockId(1)));
    assert_eq!(dead.to_string(), "bb1 is unreachable from the entry");

    let irreducible = analyze_mir_control_flow(&body(vec![
        branch(1, 2),
        goto(2),
        branch(1, 3),
        returning(),
    ]))
    .unwrap_err();
    assert_eq!(
        irreducible,
        MirControlFlowError::Irreducible {
            blocks: vec![MirBlockId(1), MirBlockId(2)],
            entries: vec![
                MirControlFlowEdge {
                    source: MirBlockId(0),
                    target: MirBlockId(1),
                },
                MirControlFlowEdge {
                    source: MirBlockId(0),
                    target: MirBlockId(2),
                },
            ],
        }
    );
    assert_eq!(
        irreducible.to_string(),
        "irreducible control flow in bb1, bb2; entries: bb0 -> bb1, bb0 -> bb2"
    );
}

#[test]
fn generated_reducible_graphs_have_reflexive_transitive_dominance() {
    // Every generated graph is a forward chain with deterministic diamonds
    // and optional natural-loop backedges. This covers sparse join patterns
    // without depending on a random-number generator or test ordering.
    for seed in 0_u32..128 {
        let count = 4 + (seed as usize % 13);
        let mut blocks = Vec::with_capacity(count);
        for index in 0..count {
            let kind = if index + 1 == count {
                MirTerminatorKind::Return
            } else if index + 2 < count && ((seed >> (index % 7)) & 1) == 1 {
                MirTerminatorKind::SwitchInt {
                    discr: dialect_mir::MirOperand::Constant(dialect_mir::MirConstant {
                        ty: dialect_mir::MirTypeId(0),
                        value: dialect_mir::MirConstantValue::Bool(true),
                    }),
                    targets: vec![(1, edge((index + 1) as u32))],
                    otherwise: edge((index + 2) as u32),
                }
            } else {
                MirTerminatorKind::Goto(edge((index + 1) as u32))
            };
            blocks.push(block(kind));
        }
        let analysis = analyze_mir_control_flow(&body(blocks)).unwrap();
        for block_index in 0..count {
            let block = MirBlockId(block_index as u32);
            assert!(analysis.dominates(block, block));
            assert!(analysis.dominates(MirBlockId(0), block));
            for dominator in analysis.dominators(block).unwrap() {
                for transitive in analysis.dominators(*dominator).unwrap() {
                    assert!(analysis.dominates(*transitive, block));
                }
            }
        }
    }
}
