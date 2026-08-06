use fe2o3_kernel_analysis::{ControlFlowDiagnostic, ControlFlowEdge, analyze_control_flow};
use fe2o3_kernel_ir::{
    BasicBlock, BlockId, Constant, Function, Operation, OperationKind, Signature, Terminator, Type,
    ValueDef, ValueId,
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

fn ids(values: &[u32]) -> BTreeSet<BlockId> {
    values.iter().copied().map(BlockId).collect()
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
    assert_eq!(analysis.dominators(BlockId(1)), Some(&ids(&[0, 1])));
    assert_eq!(analysis.dominators(BlockId(2)), Some(&ids(&[0, 2])));
    assert_eq!(analysis.dominators(BlockId(3)), Some(&ids(&[0, 3])));
    assert!(analysis.dominates(BlockId(0), BlockId(3)));
    assert!(!analysis.dominates(BlockId(1), BlockId(3)));
    assert!(analysis.backedges().is_empty());
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
