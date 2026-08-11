use std::collections::BTreeSet;

use fe2o3_kernel_analysis::{
    ControlFlowResource, MAX_SSA_PLACEMENT_OUTPUT_ITEMS, SsaPlacementDiagnostic, SsaVariable,
    SsaVariablePlacement, analyze_control_flow, place_pruned_ssa_parameters,
};
use fe2o3_kernel_ir::{
    BasicBlock, BlockId, Constant, Function, Operation, OperationKind, Signature, Terminator, Type,
    ValueDef, ValueId,
};

fn ids(values: &[u32]) -> BTreeSet<BlockId> {
    values.iter().copied().map(BlockId).collect()
}

fn block(id: u32, terminator: Terminator) -> BasicBlock {
    let mut block = BasicBlock::new(BlockId(id));
    block.terminator = Some(terminator);
    block
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
    let condition = ValueId(100 + id);
    let mut block = block(
        id,
        Terminator::ConditionalBranch {
            condition,
            then_target: BlockId(then_target),
            then_arguments: vec![],
            else_target: BlockId(else_target),
            else_arguments: vec![],
        },
    );
    block.operations.push(Operation::effect_free(
        ValueDef::new(condition, Type::BOOL),
        OperationKind::Constant(Constant::Bool(true)),
    ));
    block
}

fn returning(id: u32) -> BasicBlock {
    block(id, Terminator::Return { values: vec![] })
}

fn function(blocks: Vec<BasicBlock>) -> Function {
    Function::definition("ssa", Signature::new(vec![], vec![]), vec![], blocks)
}

#[test]
fn places_only_live_diamond_and_loop_parameters() {
    let diamond = analyze_control_flow(&function(vec![
        conditional(0, 1, 2),
        branch(1, 3),
        branch(2, 3),
        returning(3),
    ]))
    .unwrap();
    let placement = place_pruned_ssa_parameters(
        &diamond,
        &[
            SsaVariablePlacement {
                variable: SsaVariable(9),
                definition_blocks: ids(&[1, 2]),
                live_in_blocks: ids(&[3]),
            },
            SsaVariablePlacement {
                variable: SsaVariable(2),
                definition_blocks: ids(&[0]),
                live_in_blocks: ids(&[1, 2, 3]),
            },
        ],
    )
    .unwrap();
    assert_eq!(placement.blocks_for(SsaVariable(9)), Some(&ids(&[3])));
    assert_eq!(placement.blocks_for(SsaVariable(2)), Some(&ids(&[])));
    assert_eq!(
        placement.variables_at(BlockId(3)).unwrap(),
        &BTreeSet::from([SsaVariable(9)])
    );

    let loop_cfg = analyze_control_flow(&function(vec![
        branch(0, 1),
        conditional(1, 2, 3),
        branch(2, 1),
        returning(3),
    ]))
    .unwrap();
    let loop_placement = place_pruned_ssa_parameters(
        &loop_cfg,
        &[SsaVariablePlacement {
            variable: SsaVariable(7),
            definition_blocks: ids(&[0, 2]),
            live_in_blocks: ids(&[1, 2, 3]),
        }],
    )
    .unwrap();
    assert_eq!(loop_placement.blocks_for(SsaVariable(7)), Some(&ids(&[1])));
}

#[test]
fn rejects_malformed_facts_in_stable_order() {
    let control_flow =
        analyze_control_flow(&function(vec![returning(0), branch(10, 11), returning(11)])).unwrap();
    let duplicate = SsaVariablePlacement {
        variable: SsaVariable(4),
        definition_blocks: ids(&[]),
        live_in_blocks: ids(&[10, 99]),
    };
    let error =
        place_pruned_ssa_parameters(&control_flow, &[duplicate.clone(), duplicate]).unwrap_err();
    assert_eq!(
        error.diagnostics(),
        &[
            SsaPlacementDiagnostic::DuplicateVariable {
                variable: SsaVariable(4),
            },
            SsaPlacementDiagnostic::MissingDefinition {
                variable: SsaVariable(4),
            },
            SsaPlacementDiagnostic::UnknownLiveInBlock {
                variable: SsaVariable(4),
                block: BlockId(99),
            },
            SsaPlacementDiagnostic::UnreachableLiveInBlock {
                variable: SsaVariable(4),
                block: BlockId(10),
            },
        ]
    );
    assert_eq!(
        error.to_string(),
        "SSA placement failed with 4 diagnostic(s)\n  duplicate SSA variable v4\n  SSA variable v4 has no definition block\n  SSA variable v4 has unknown live-in block bb99\n  SSA variable v4 has unreachable live-in block bb10\n"
    );
}

#[test]
fn placement_is_deterministic_across_variable_input_order() {
    let control_flow = analyze_control_flow(&function(vec![
        conditional(0, 1, 2),
        branch(1, 3),
        branch(2, 3),
        returning(3),
    ]))
    .unwrap();
    let first = SsaVariablePlacement {
        variable: SsaVariable(1),
        definition_blocks: ids(&[1, 2]),
        live_in_blocks: ids(&[3]),
    };
    let second = SsaVariablePlacement {
        variable: SsaVariable(2),
        definition_blocks: ids(&[1, 2]),
        live_in_blocks: ids(&[3]),
    };
    assert_eq!(
        place_pruned_ssa_parameters(&control_flow, &[first.clone(), second.clone()]).unwrap(),
        place_pruned_ssa_parameters(&control_flow, &[second, first]).unwrap()
    );
}

#[test]
fn pruned_ssa_output_accepts_the_exact_boundary_and_rejects_next_parameter() {
    let control_flow = analyze_control_flow(&function(vec![branch(0, 0)])).unwrap();
    let variables = (0..=u32::try_from(MAX_SSA_PLACEMENT_OUTPUT_ITEMS).unwrap())
        .map(|variable| SsaVariablePlacement {
            variable: SsaVariable(variable),
            definition_blocks: ids(&[0]),
            live_in_blocks: ids(&[0]),
        })
        .collect::<Vec<_>>();

    let boundary =
        place_pruned_ssa_parameters(&control_flow, &variables[..MAX_SSA_PLACEMENT_OUTPUT_ITEMS])
            .unwrap();
    assert_eq!(
        boundary.resource_usage().ssa_placement_output_items(),
        MAX_SSA_PLACEMENT_OUTPUT_ITEMS
    );

    let error = place_pruned_ssa_parameters(&control_flow, &variables).unwrap_err();
    assert_eq!(
        error.diagnostics(),
        &[SsaPlacementDiagnostic::ResourceLimitExceeded {
            resource: ControlFlowResource::SsaPlacementOutputItems,
            required: MAX_SSA_PLACEMENT_OUTPUT_ITEMS + 1,
            limit: MAX_SSA_PLACEMENT_OUTPUT_ITEMS,
            storage_items: 458_766,
            work_units: 2_818_116,
        }]
    );
    assert_eq!(
        error.to_string(),
        "SSA placement failed with 1 diagnostic(s)\n  SSA placement pruned-SSA output items require 65537 items, exceeding the deterministic limit 65536; aggregate storage 458766, aggregate work 2818116\n"
    );
}
