use super::*;
use fe2o3_kernel_ir::{Constant, ScalarType, ValueDef, verify_module};

fn pair() -> (Module, Module) {
    let after = module();
    let mut before = after.clone();
    let body = before.functions[0].body.as_mut().unwrap();
    let mut successor = BasicBlock::new(BlockId(9));
    successor.operations = body.blocks[0].operations.split_off(1);
    successor.terminator = body.blocks[0].terminator.take();
    body.blocks[0].terminator = Some(Terminator::Branch {
        target: successor.id,
        arguments: vec![],
    });
    body.blocks.push(successor);
    (before, after)
}

#[test]
fn exact_coalescing_keeps_every_operation_and_rebases_only_coordinates() {
    let (before, after) = pair();
    let analysis = analyze_kernel_capability_preservation_v1(&before, 7).unwrap();
    assert!(matches!(
        analysis.replay_candidate(7, 8, 1, &after),
        Err(KernelCapabilityPreservationErrorV1::ContextUsesChanged)
    ));
    let replay = analysis
        .replay_control_flow_coalescing(&before, 7, 8, 1, &after)
        .unwrap();
    assert!(replay.changed());
    assert_eq!(replay.input_identity(), analysis.canonical_identity());
    assert!(!replay.grants_semantic_preservation_authority());
    assert!(!replay.grants_artifact_or_launch_authority());
}

#[test]
fn coalescing_rejects_source_epoch_and_identity_substitutions() {
    let (before, after) = pair();
    let analysis = analyze_kernel_capability_preservation_v1(&before, 7).unwrap();
    assert!(
        analysis
            .replay_control_flow_coalescing(&after, 7, 8, 1, &after)
            .is_err()
    );
    for (input, output, mutations) in [(6, 8, 1), (7, 9, 1), (7, 8, 0), (7, 8, 2)] {
        assert!(
            analysis
                .replay_control_flow_coalescing(&before, input, output, mutations, &after)
                .is_err()
        );
    }
}

#[test]
fn coalescing_rejects_operation_deletion_insertion_and_provenance_changes() {
    let (before, after) = pair();
    let analysis = analyze_kernel_capability_preservation_v1(&before, 0).unwrap();
    for mutation in 0..4 {
        let mut candidate = after.clone();
        let operations = &mut candidate.functions[0].body.as_mut().unwrap().blocks[0].operations;
        match mutation {
            0 => {
                operations.pop();
            }
            1 => operations.push(operations[1].clone()),
            2 => {
                operations[0] =
                    Operation::kernel_context_issue(ValueId(0), context("entry"), source(99))
            }
            _ => operations.push(Operation::effect_free(
                ValueDef::new(ValueId(1), Type::Scalar(ScalarType::U32)),
                OperationKind::Constant(Constant::U32(7)),
            )),
        }
        verify_module(&candidate).unwrap();
        assert!(matches!(
            analysis.replay_control_flow_coalescing(&before, 0, 1, 1, &candidate),
            Err(KernelCapabilityPreservationErrorV1::ControlFlowCoalescingMismatch)
        ));
    }
}

#[test]
fn coalescing_does_not_erase_unreachable_blocks_or_change_contracts() {
    let (mut before, after) = pair();
    let mut dead = BasicBlock::new(BlockId(99));
    dead.terminator = Some(Terminator::Return { values: vec![] });
    before.functions[0].body.as_mut().unwrap().blocks.push(dead);
    let analysis = analyze_kernel_capability_preservation_v1(&before, 0).unwrap();
    assert!(
        analysis
            .replay_control_flow_coalescing(&before, 0, 1, 1, &after)
            .is_err()
    );

    let (before, mut after) = pair();
    let analysis = analyze_kernel_capability_preservation_v1(&before, 0).unwrap();
    after.required_capabilities.clear();
    after.functions[0]
        .required_capabilities
        .insert(requirement());
    assert!(
        analysis
            .replay_control_flow_coalescing(&before, 0, 1, 1, &after)
            .is_err()
    );
}

#[test]
fn coalescing_does_not_remove_block_arguments() {
    let (mut before, after) = pair();
    let body = before.functions[0].body.as_mut().unwrap();
    body.blocks[1].parameters.push(ValueDef::new(
        ValueId(10),
        Type::KernelContext(context("entry")),
    ));
    body.blocks[1].operations[0].kind = OperationKind::Call {
        callee: FunctionId::new("helper"),
        arguments: vec![ValueId(10)],
    };
    body.blocks[0].terminator = Some(Terminator::Branch {
        target: BlockId(9),
        arguments: vec![ValueId(0)],
    });
    let analysis = analyze_kernel_capability_preservation_v1(&before, 0).unwrap();
    assert!(
        analysis
            .replay_control_flow_coalescing(&before, 0, 1, 1, &after)
            .is_err()
    );
}

#[test]
fn coalescing_does_not_fold_conditional_paths() {
    let (mut before, mut after) = pair();
    let condition = Operation::effect_free(
        ValueDef::new(ValueId(20), Type::Scalar(ScalarType::Bool)),
        OperationKind::Constant(Constant::Bool(true)),
    );
    before.functions[0].body.as_mut().unwrap().blocks[0]
        .operations
        .push(condition.clone());
    after.functions[0].body.as_mut().unwrap().blocks[0]
        .operations
        .insert(1, condition);
    before.functions[0].body.as_mut().unwrap().blocks[0].terminator =
        Some(Terminator::ConditionalBranch {
            condition: ValueId(20),
            then_target: BlockId(9),
            then_arguments: vec![],
            else_target: BlockId(9),
            else_arguments: vec![],
        });
    let analysis = analyze_kernel_capability_preservation_v1(&before, 0).unwrap();
    assert!(
        analysis
            .replay_control_flow_coalescing(&before, 0, 1, 1, &after)
            .is_err()
    );
}

#[test]
fn composed_replay_requires_exact_adjacent_graphs_and_epochs() {
    let (before, after) = pair();
    let analysis = analyze_kernel_capability_preservation_v1(&before, 0).unwrap();
    let first = analysis
        .replay_control_flow_coalescing(&before, 0, 1, 1, &after)
        .unwrap();
    let next = analyze_kernel_capability_preservation_v1(&after, 1)
        .unwrap()
        .replay_candidate(1, 1, 0, &after)
        .unwrap();
    assert_eq!(first.then(next).unwrap(), first);
    assert!(first.then(first).is_err());
    let wrong_epoch = analyze_kernel_capability_preservation_v1(&after, 2)
        .unwrap()
        .replay_candidate(2, 2, 0, &after)
        .unwrap();
    assert!(first.then(wrong_epoch).is_err());
    let mut other = after.clone();
    other.required_capabilities.insert(TargetCapability::Int64);
    let wrong_identity = analyze_kernel_capability_preservation_v1(&other, 1)
        .unwrap()
        .replay_candidate(1, 1, 0, &other)
        .unwrap();
    assert!(matches!(
        first.then(wrong_identity),
        Err(KernelCapabilityPreservationErrorV1::ReplayChainMismatch)
    ));
}

#[test]
fn coalescing_preserves_a_cycle_and_rejects_its_removal() {
    let (mut before, mut after) = pair();
    let blocks = &mut before.functions[1].body.as_mut().unwrap().blocks;
    blocks[0].terminator = Some(Terminator::Branch {
        target: BlockId(1),
        arguments: vec![],
    });
    let mut tail = BasicBlock::new(BlockId(1));
    tail.terminator = Some(Terminator::Branch {
        target: BlockId(0),
        arguments: vec![],
    });
    blocks.push(tail);
    after.functions[1].body.as_mut().unwrap().blocks[0].terminator = Some(Terminator::Branch {
        target: BlockId(0),
        arguments: vec![],
    });
    verify_module(&after).unwrap();
    let analysis = analyze_kernel_capability_preservation_v1(&before, 4).unwrap();
    analysis
        .replay_control_flow_coalescing(&before, 4, 5, 1, &after)
        .unwrap();
    after.functions[1].body.as_mut().unwrap().blocks[0].terminator =
        Some(Terminator::Return { values: vec![] });
    verify_module(&after).unwrap();
    assert!(matches!(
        analysis.replay_control_flow_coalescing(&before, 4, 5, 1, &after),
        Err(KernelCapabilityPreservationErrorV1::ControlFlowCoalescingMismatch)
    ));
}

#[test]
fn coalescing_cannot_delete_an_unreachable_cycle_or_bypass_another_predecessor() {
    let (mut before, after) = pair();
    for (id, target) in [(90, 91), (91, 90)] {
        let mut block = BasicBlock::new(BlockId(id));
        block.terminator = Some(Terminator::Branch {
            target: BlockId(target),
            arguments: vec![],
        });
        before.functions[0]
            .body
            .as_mut()
            .unwrap()
            .blocks
            .push(block);
    }
    verify_module(&after).unwrap();
    let analysis = analyze_kernel_capability_preservation_v1(&before, 0).unwrap();
    assert!(matches!(
        analysis.replay_control_flow_coalescing(&before, 0, 1, 1, &after),
        Err(KernelCapabilityPreservationErrorV1::ControlFlowCoalescingMismatch)
    ));

    let (mut before, mut after) = pair();
    let mut predecessor = BasicBlock::new(BlockId(99));
    predecessor.terminator = Some(Terminator::Branch {
        target: BlockId(9),
        arguments: vec![],
    });
    before.functions[0]
        .body
        .as_mut()
        .unwrap()
        .blocks
        .push(predecessor.clone());
    predecessor.terminator = Some(Terminator::Branch {
        target: BlockId(0),
        arguments: vec![],
    });
    after.functions[0]
        .body
        .as_mut()
        .unwrap()
        .blocks
        .push(predecessor);
    verify_module(&after).unwrap();
    let analysis = analyze_kernel_capability_preservation_v1(&before, 0).unwrap();
    assert!(matches!(
        analysis.replay_control_flow_coalescing(&before, 0, 1, 1, &after),
        Err(KernelCapabilityPreservationErrorV1::ControlFlowCoalescingMismatch)
    ));
}

#[test]
fn coalescing_preserves_exact_execution_contracts_after_coordinate_changes() {
    let fixtures: [&[u8]; 2] = [
        include_bytes!(
            "../../../fe2o3-kernel-opt/tests/fixtures/transformation_preservation_v1/workgroup_barrier.bin"
        ),
        include_bytes!(
            "../../../fe2o3-kernel-opt/tests/fixtures/transformation_preservation_v1/dynamic_private_view.bin"
        ),
    ];
    for bytes in fixtures {
        let after = fe2o3_kernel_ir::decode_module_v13(bytes).unwrap();
        let mut before = after.clone();
        let body = before
            .functions
            .iter_mut()
            .filter_map(|function| function.body.as_mut())
            .find(|body| {
                body.blocks.iter().any(|block| {
                    block.operations.iter().any(|operation| {
                        matches!(operation.kind, OperationKind::ExecutionCapability(_))
                    })
                })
            })
            .unwrap();
        let id = BlockId(body.blocks.iter().map(|block| block.id.0).max().unwrap() + 1);
        let block = body
            .blocks
            .iter_mut()
            .find(|block| {
                block.operations.iter().any(|operation| {
                    matches!(operation.kind, OperationKind::ExecutionCapability(_))
                })
            })
            .unwrap();
        let position = block
            .operations
            .iter()
            .position(|operation| matches!(operation.kind, OperationKind::ExecutionCapability(_)))
            .unwrap();
        let mut successor = BasicBlock::new(id);
        successor.operations = block.operations.split_off(position);
        successor.terminator = block.terminator.take();
        block.terminator = Some(Terminator::Branch {
            target: id,
            arguments: vec![],
        });
        body.blocks.push(successor);
        let analysis = analyze_kernel_capability_preservation_v1(&before, 11).unwrap();
        analysis
            .replay_control_flow_coalescing(&before, 11, 12, 1, &after)
            .unwrap();

        for mutation in 0..3 {
            let mut candidate = after.clone();
            match mutation {
                0 => {
                    let contract = candidate
                        .functions
                        .iter_mut()
                        .filter_map(|function| function.body.as_mut())
                        .flat_map(|body| &mut body.blocks)
                        .flat_map(|block| &mut block.operations)
                        .find_map(|operation| match &mut operation.kind {
                            OperationKind::ExecutionCapability(contract) => Some(contract),
                            _ => None,
                        })
                        .unwrap();
                    contract.source.operation[0] ^= 0x5a;
                }
                1 => substitute_execution_provenance(&mut candidate),
                _ => candidate.kernels[0].id = fe2o3_kernel_ir::KernelId::new("substituted-kernel"),
            }
            verify_module(&candidate).unwrap();
            assert!(matches!(
                analysis.replay_control_flow_coalescing(&before, 11, 12, 1, &candidate),
                Err(KernelCapabilityPreservationErrorV1::ControlFlowCoalescingMismatch)
            ));
        }
    }
}

fn substitute_execution_provenance(module: &mut Module) {
    let substitute_type = |ty: &mut Type| {
        if let Type::ExecutionCapability(capability) = ty {
            capability.provenance.frontend_unit[0] ^= 0x11;
        }
    };
    for function in &mut module.functions {
        for ty in function
            .signature
            .parameters
            .iter_mut()
            .chain(&mut function.signature.results)
        {
            substitute_type(ty);
        }
        if let Some(body) = &mut function.body {
            for block in &mut body.blocks {
                for parameter in &mut block.parameters {
                    substitute_type(&mut parameter.ty);
                }
                for operation in &mut block.operations {
                    for result in &mut operation.results {
                        substitute_type(&mut result.ty);
                    }
                    if let OperationKind::ExecutionCapability(contract) = &mut operation.kind {
                        contract.provenance.frontend_unit[0] ^= 0x11;
                    }
                }
            }
        }
    }
}

#[test]
fn replay_composition_keeps_mutation_epochs_when_endpoint_identity_returns() {
    let input = module();
    let mut middle = input.clone();
    middle.required_capabilities.insert(TargetCapability::Int64);
    let first = analyze_kernel_capability_preservation_v1(&input, 7)
        .unwrap()
        .replay_candidate(7, 8, 1, &middle)
        .unwrap();
    let second = analyze_kernel_capability_preservation_v1(&middle, 8)
        .unwrap()
        .replay_candidate(8, 9, 1, &input)
        .unwrap();
    let replay = first.then(second).unwrap();
    assert!(!replay.changed());
    assert_eq!(replay.input_identity(), replay.output_identity());
    assert_eq!((replay.input_epoch(), replay.output_epoch()), (7, 9));
}

#[test]
fn coalescing_rejects_epoch_overflow_and_stale_analysis_before_validation() {
    let (before, after) = pair();
    let analysis = analyze_kernel_capability_preservation_v1(&before, u64::MAX).unwrap();
    assert!(matches!(
        analysis.replay_control_flow_coalescing(&before, u64::MAX, 0, 1, &after),
        Err(KernelCapabilityPreservationErrorV1::MutationEpochOverflow { .. })
    ));
    assert!(
        !analysis
            .replay_control_flow_coalescing(&before, u64::MAX, u64::MAX, 0, &before)
            .unwrap()
            .changed()
    );
    let invalid = Module::new("");
    assert!(matches!(
        analysis.replay_control_flow_coalescing(&invalid, 0, 1, 1, &invalid),
        Err(KernelCapabilityPreservationErrorV1::StaleAnalysisEpoch { .. })
    ));
}

#[test]
fn coalescing_keeps_switch_decisions_and_retained_block_order() {
    let (mut before, mut after) = pair();
    for (module, tail) in [(&mut before, 1), (&mut after, 0)] {
        let body = module.functions[0].body.as_mut().unwrap();
        body.blocks[tail].operations.push(Operation::effect_free(
            ValueDef::new(ValueId(20), Type::Scalar(ScalarType::U32)),
            OperationKind::Constant(Constant::U32(0)),
        ));
        body.blocks[tail].terminator = Some(Terminator::IntegerSwitch {
            selector: ValueId(20),
            cases: vec![fe2o3_kernel_ir::IntegerSwitchCase {
                value: Constant::U32(0),
                target: BlockId(30),
                arguments: vec![],
            }],
            default_target: BlockId(31),
            default_arguments: vec![],
        });
        for id in [30, 31] {
            let mut block = BasicBlock::new(BlockId(id));
            block.terminator = Some(Terminator::Return { values: vec![] });
            body.blocks.push(block);
        }
    }
    verify_module(&after).unwrap();
    let analysis = analyze_kernel_capability_preservation_v1(&before, 0).unwrap();
    analysis
        .replay_control_flow_coalescing(&before, 0, 1, 1, &after)
        .unwrap();
    let mut changed = after.clone();
    let Some(Terminator::IntegerSwitch {
        cases,
        default_target,
        ..
    }) = &mut changed.functions[0].body.as_mut().unwrap().blocks[0].terminator
    else {
        unreachable!()
    };
    std::mem::swap(&mut cases[0].target, default_target);
    verify_module(&changed).unwrap();
    assert!(matches!(
        analysis.replay_control_flow_coalescing(&before, 0, 1, 1, &changed),
        Err(KernelCapabilityPreservationErrorV1::ControlFlowCoalescingMismatch)
    ));
    after.functions[0].body.as_mut().unwrap().blocks.swap(1, 2);
    verify_module(&after).unwrap();
    assert!(matches!(
        analysis.replay_control_flow_coalescing(&before, 0, 1, 1, &after),
        Err(KernelCapabilityPreservationErrorV1::ControlFlowCoalescingMismatch)
    ));
}
