// Real closed-pipeline mapping regressions; included inside the existing tests
// module to reuse owner(), run(), row(), and the independent owner/map checker.

fn mapped_block_argument_node(map: &KirOptimizationMapV12, block: u32, argument: u32) -> usize {
    let endpoint = Endpoint::BlockArgument {
        function: 0,
        block,
        argument,
    };
    let matches = map
        .data
        .nodes
        .iter()
        .enumerate()
        .filter_map(|(index, node)| (node.input == Some(endpoint)).then_some(index))
        .collect::<Vec<_>>();
    assert_eq!(
        matches.len(),
        1,
        "source block argument must have one stable capture ID"
    );
    matches[0]
}

#[test]
fn closed_policy_dce_cascading_duplicate_edges_preserves_abi_and_exact_map() {
    let ty = Type::Scalar(ScalarType::U32);
    let scalar = |id| ValueDef::new(ValueId(id), ty.clone());
    let mut entry = BasicBlock::new(BlockId(11));
    entry.terminator = Some(Terminator::ConditionalBranch {
        condition: ValueId(0),
        then_target: BlockId(31),
        then_arguments: vec![],
        else_target: BlockId(59),
        else_arguments: vec![],
    });
    let mut left = BasicBlock::new(BlockId(31));
    left.operations.push(KirOperation::effect_free(
        scalar(5),
        OperationKind::Binary {
            op: BinaryOp::Add,
            lhs: ValueId(2),
            rhs: ValueId(3),
        },
    ));
    left.terminator = Some(Terminator::Branch {
        target: BlockId(97),
        arguments: vec![ValueId(5)],
    });
    let mut right = BasicBlock::new(BlockId(59));
    right.operations.push(KirOperation::effect_free(
        scalar(6),
        OperationKind::Binary {
            op: BinaryOp::Subtract,
            lhs: ValueId(2),
            rhs: ValueId(3),
        },
    ));
    right.terminator = Some(Terminator::Branch {
        target: BlockId(97),
        arguments: vec![ValueId(6)],
    });
    let mut forward = BasicBlock::new(BlockId(97));
    forward.parameters.push(scalar(7));
    forward.terminator = Some(Terminator::ConditionalBranch {
        condition: ValueId(1),
        then_target: BlockId(131),
        then_arguments: vec![ValueId(7)],
        else_target: BlockId(131),
        else_arguments: vec![ValueId(7)],
    });
    let mut join = BasicBlock::new(BlockId(131));
    join.parameters.push(scalar(8));
    join.terminator = Some(Terminator::Return {
        values: vec![ValueId(2)],
    });

    let mut source = Module::new("closed-policy-duplicate-argument-cascade");
    source.functions.push(Function::internal_helper(
        "f",
        Signature::new(
            vec![Type::BOOL, Type::BOOL, ty.clone(), ty.clone(), ty.clone()],
            vec![ty],
        ),
        vec![ValueId(0), ValueId(1), ValueId(2), ValueId(3), ValueId(4)],
        vec![entry, left, right, forward, join],
    ));
    let input = owner(&source);
    // Both conditions remain dynamic. The forward block has two predecessors,
    // and the conditional has two successor edges even though they coincide.
    // SimplifyCFG therefore cannot merge away either argument before DCE.
    // Add/Subtract are conservatively retained by the actual GPU dialect.
    let (output, report, map) = run(&input);
    assert_eq!(
        report
            .passes()
            .iter()
            .map(|pass| pass.pass())
            .collect::<Vec<_>>(),
        crate::KIR_PLIRON_PRODUCTION_PASSES_V12
    );
    assert_eq!(
        report
            .passes()
            .iter()
            .map(|pass| pass.changed())
            .collect::<Vec<_>>(),
        [false, false, false, true, false, false, false],
        "the first DCE must perform the cascade, not an earlier CFG rewrite"
    );

    let function = &output.module().functions[0];
    assert_eq!(output.module().functions.len(), 1);
    assert_eq!(function.id, source.functions[0].id);
    assert_eq!(function.role, source.functions[0].role);
    assert_eq!(function.signature, source.functions[0].signature);
    let body = function.body.as_ref().unwrap();
    assert_eq!(
        body.parameters,
        source.functions[0].body.as_ref().unwrap().parameters,
        "all positional function arguments, including unused argument 4, remain"
    );
    let blocks = &body.blocks;
    assert_eq!(blocks.len(), 5);
    assert!(blocks.iter().all(|block| block.parameters.is_empty()));
    assert_eq!(
        blocks
            .iter()
            .map(|block| block.operations.len())
            .collect::<Vec<_>>(),
        [0, 1, 1, 0, 0],
        "arithmetic stays live in the closed dialect's conservative effect model"
    );
    assert_eq!(
        blocks[0].terminator.as_ref(),
        Some(&Terminator::ConditionalBranch {
            condition: body.parameters[0],
            then_target: blocks[1].id,
            then_arguments: vec![],
            else_target: blocks[2].id,
            else_arguments: vec![],
        })
    );
    for (index, kind) in [(1, BinaryOp::Add), (2, BinaryOp::Subtract)] {
        assert_eq!(
            blocks[index].operations[0].kind,
            OperationKind::Binary {
                op: kind,
                lhs: body.parameters[2],
                rhs: body.parameters[3],
            }
        );
        assert_eq!(
            blocks[index].terminator.as_ref(),
            Some(&Terminator::Branch {
                target: blocks[3].id,
                arguments: vec![],
            })
        );
    }
    assert_eq!(
        blocks[3].terminator.as_ref(),
        Some(&Terminator::ConditionalBranch {
            condition: body.parameters[1],
            then_target: blocks[4].id,
            then_arguments: vec![],
            else_target: blocks[4].id,
            else_arguments: vec![],
        }),
        "both original duplicate edges survive with correctly shortened segments"
    );
    assert_eq!(
        blocks[4].terminator.as_ref(),
        Some(&Terminator::Return {
            values: vec![body.parameters[2]],
        })
    );

    for block in [3, 4] {
        let removed = mapped_block_argument_node(&map, block, 0);
        assert_eq!(map.data.terminal[removed], None);
        let erased = map
            .data
            .events
            .iter()
            .filter(|event| matches!(event.change, Change::Erase(id) if id as usize == removed))
            .collect::<Vec<_>>();
        assert_eq!(
            erased.len(),
            1,
            "each stable argument is erased exactly once"
        );
        assert_eq!(erased[0].pass, 3);
        assert!(!map.data.events.iter().any(|event| {
            matches!(event.change, Change::Replace(old, _) if old as usize == removed)
        }));
    }
    for argument in 0..5 {
        let endpoint = Endpoint::FunctionArgument {
            function: 0,
            argument,
        };
        let nodes = map
            .data
            .nodes
            .iter()
            .enumerate()
            .filter_map(|(index, node)| (node.input == Some(endpoint)).then_some(index))
            .collect::<Vec<_>>();
        assert_eq!(nodes.len(), 1);
        assert_eq!(map.data.terminal[nodes[0]], Some(endpoint));
        assert!(
            !map.data.events.iter().any(|event| {
                matches!(event.change, Change::Erase(id) if id as usize == nodes[0])
            })
        );
    }
    let coordinates = (0..5)
        .map(|block| Coordinate::Terminator { function: 0, block })
        .chain([1, 2].map(|block| Coordinate::Operation {
            function: 0,
            block,
            operation: 0,
        }));
    for coordinate in coordinates {
        let relation = row(&map, coordinate);
        assert_eq!(
            relation.disposition(),
            KirOptimizationDispositionV12::Retained
        );
        assert!(relation.identity_survived());
        assert!(!relation.moved());
        assert_eq!(
            map.targets(relation).unwrap(),
            [Endpoint::Operation(coordinate)]
        );
    }
    assert!(map.synthesized_operations().is_empty());
    assert_eq!(map.input_identity(), input.canonical().identity());
    assert_eq!(map.output_identity(), output.canonical().identity());
    assert_eq!(input.module(), &source);

    // run() uses a fresh owned session and independently checks the complete map
    // against the connected owners after executing the fixed seven-pass policy.
    let (replay_output, replay_report, replay_map) = run(&input);
    assert_eq!(
        output.canonical().canonical_bytes(),
        replay_output.canonical().canonical_bytes()
    );
    assert_eq!(report, replay_report);
    assert_eq!(map, replay_map);
}

#[test]
fn actual_zero_result_conditional_replacement_maps_to_the_surviving_branch() {
    for condition in [false, true] {
        let ty = Type::Scalar(ScalarType::U32);
        let scalar = |id| ValueDef::new(ValueId(id), ty.clone());
        let mut entry = BasicBlock::new(BlockId(17));
        entry.operations.push(KirOperation::effect_free(
            ValueDef::new(ValueId(2), Type::BOOL),
            OperationKind::Constant(fe2o3_kernel_ir::Constant::Bool(condition)),
        ));
        entry.operations.push(KirOperation::effect_free(
            scalar(3),
            OperationKind::Constant(fe2o3_kernel_ir::Constant::U32(1)),
        ));
        let (then_target, else_target) = if condition {
            (BlockId(29), BlockId(53))
        } else {
            (BlockId(53), BlockId(29))
        };
        entry.terminator = Some(Terminator::ConditionalBranch {
            condition: ValueId(2),
            then_target,
            then_arguments: vec![ValueId(1)],
            else_target,
            else_arguments: vec![ValueId(1)],
        });
        let mut header = BasicBlock::new(BlockId(29));
        header.parameters.push(scalar(4));
        header.operations.push(KirOperation::effect_free(
            scalar(5),
            OperationKind::Binary {
                op: BinaryOp::Add,
                lhs: ValueId(4),
                rhs: ValueId(3),
            },
        ));
        // The backedge gives header two predecessors after folding. This keeps
        // the new entry Branch live through both SimplifyCFG passes.
        header.terminator = Some(Terminator::ConditionalBranch {
            condition: ValueId(0),
            then_target: BlockId(29),
            then_arguments: vec![ValueId(5)],
            else_target: BlockId(41),
            else_arguments: vec![ValueId(5)],
        });
        let mut exit = BasicBlock::new(BlockId(41));
        exit.parameters.push(scalar(6));
        exit.terminator = Some(Terminator::Return {
            values: vec![ValueId(6)],
        });
        let mut dead = BasicBlock::new(BlockId(53));
        dead.parameters.push(scalar(7));
        dead.terminator = Some(Terminator::Return {
            values: vec![ValueId(7)],
        });
        let mut source = Module::new("zero-result-branch");
        source.functions.push(Function::internal_helper(
            "f",
            Signature::new(vec![Type::BOOL, ty.clone()], vec![ty]),
            vec![ValueId(0), ValueId(1)],
            vec![entry, header, exit, dead],
        ));
        let input = owner(&source);
        let (output, report, map) = run(&input);
        assert_eq!(report.passes().len(), 7);
        assert!(report.passes()[1].changed());
        let blocks = &output.module().functions[0].body.as_ref().unwrap().blocks;
        assert_eq!(
            blocks.len(),
            3,
            "only the unreachable alternative is removed"
        );
        assert!(matches!(
            blocks[0].terminator.as_ref(),
            Some(Terminator::Branch { arguments, .. }) if arguments.len() == 1
        ));
        let source_terminator = Coordinate::Terminator {
            function: 0,
            block: 0,
        };
        let relation = row(&map, source_terminator);
        assert_eq!(
            relation.disposition(),
            KirOptimizationDispositionV12::Replaced
        );
        assert!(!relation.identity_survived());
        assert!(!relation.moved());
        assert_eq!(
            map.targets(relation).unwrap(),
            [Endpoint::Operation(source_terminator)]
        );
        let old = map
            .data
            .nodes
            .iter()
            .position(|node| node.input == Some(Endpoint::Operation(source_terminator)))
            .unwrap();
        assert_eq!(map.data.nodes[old].kind, Kind::Operation);
        assert!(!map.data.nodes.iter().any(|node| matches!(node.input,
            Some(Endpoint::Result { operation, .. }) if operation == source_terminator)));
        let replacements = map
            .data
            .events
            .iter()
            .enumerate()
            .filter_map(|(index, event)| match event.change {
                Change::Replace(a, b) if a as usize == old => Some((index, event.pass, b as usize)),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(replacements.len(), 1);
        let (replacement_index, pass, new) = replacements[0];
        assert_eq!(pass, 1, "BranchOpFoldInterface is executed by SimplifyCFG");
        assert_eq!(map.data.nodes[new].kind, Kind::Operation);
        assert!(map.data.nodes[new].input.is_none());
        assert_eq!(
            map.data.terminal[new],
            Some(Endpoint::Operation(source_terminator))
        );
        let erased_index = map
            .data
            .events
            .iter()
            .position(|event| matches!(event.change, Change::Erase(id) if id as usize == old))
            .unwrap();
        assert!(replacement_index < erased_index);
        assert_eq!(map.data.events[erased_index].pass, 1);
        assert!(!map.synthesized_operations().contains(&source_terminator));
    }
}

#[test]
fn sccp_join_argument_materialization_and_dce_preserve_shifted_value_identities() {
    let ty = Type::Scalar(ScalarType::U32);
    let scalar = |id| ValueDef::new(ValueId(id), ty.clone());
    let mut entry = BasicBlock::new(BlockId(11));
    for (id, value) in [(4, 7), (5, 1)] {
        entry.operations.push(KirOperation::effect_free(
            scalar(id),
            OperationKind::Constant(fe2o3_kernel_ir::Constant::U32(value)),
        ));
    }
    entry.terminator = Some(Terminator::ConditionalBranch {
        condition: ValueId(0),
        then_target: BlockId(31),
        then_arguments: vec![],
        else_target: BlockId(59),
        else_arguments: vec![],
    });
    let mut left = BasicBlock::new(BlockId(31));
    left.operations.push(KirOperation::effect_free(
        scalar(6),
        OperationKind::Binary {
            op: BinaryOp::Add,
            lhs: ValueId(1),
            rhs: ValueId(5),
        },
    ));
    left.terminator = Some(Terminator::Branch {
        target: BlockId(97),
        arguments: vec![ValueId(4), ValueId(6), ValueId(3), ValueId(2)],
    });
    let mut right = BasicBlock::new(BlockId(59));
    right.operations.push(KirOperation::effect_free(
        scalar(7),
        OperationKind::Binary {
            op: BinaryOp::Add,
            lhs: ValueId(2),
            rhs: ValueId(5),
        },
    ));
    right.terminator = Some(Terminator::Branch {
        target: BlockId(97),
        arguments: vec![ValueId(4), ValueId(7), ValueId(3), ValueId(1)],
    });
    let mut join = BasicBlock::new(BlockId(97));
    join.parameters
        .extend([scalar(8), scalar(9), scalar(10), scalar(11)]);
    join.operations.push(KirOperation::effect_free(
        scalar(12),
        OperationKind::Binary {
            op: BinaryOp::BitXor,
            lhs: ValueId(9),
            rhs: ValueId(11),
        },
    ));
    join.operations.push(KirOperation::effect_free(
        scalar(13),
        OperationKind::Binary {
            op: BinaryOp::Add,
            lhs: ValueId(8),
            rhs: ValueId(12),
        },
    ));
    join.terminator = Some(Terminator::Return {
        values: vec![ValueId(13)],
    });
    let mut source = Module::new("argument-shifts");
    source.functions.push(Function::internal_helper(
        "f",
        Signature::new(
            vec![Type::BOOL, ty.clone(), ty.clone(), ty.clone()],
            vec![ty],
        ),
        vec![ValueId(0), ValueId(1), ValueId(2), ValueId(3)],
        vec![entry, left, right, join],
    ));
    let input = owner(&source);
    let (output, report, map) = run(&input);
    assert!(
        report.passes()[0].changed(),
        "SCCP must materialize the constant join fact"
    );
    assert!(
        report.passes()[3].changed(),
        "the first DCE removes now-unused arguments"
    );
    let blocks = &output.module().functions[0].body.as_ref().unwrap().blocks;
    assert_eq!(
        blocks.len(),
        4,
        "the dynamic diamond remains a real multi-predecessor join"
    );
    let joins = blocks
        .iter()
        .enumerate()
        .filter(|(_, block)| matches!(block.terminator.as_ref(), Some(Terminator::Return { .. })))
        .collect::<Vec<_>>();
    assert_eq!(joins.len(), 1);
    let (join_index, join) = joins[0];
    let join_index = u32::try_from(join_index).unwrap();
    assert_eq!(join.parameters.len(), 2);
    let nodes = [
        mapped_block_argument_node(&map, 3, 0),
        mapped_block_argument_node(&map, 3, 1),
        mapped_block_argument_node(&map, 3, 2),
        mapped_block_argument_node(&map, 3, 3),
    ];
    assert_eq!(map.data.terminal[nodes[0]], None);
    assert_eq!(
        map.data.terminal[nodes[1]],
        Some(Endpoint::BlockArgument {
            function: 0,
            block: join_index,
            argument: 0
        })
    );
    assert_eq!(map.data.terminal[nodes[2]], None);
    assert_eq!(
        map.data.terminal[nodes[3]],
        Some(Endpoint::BlockArgument {
            function: 0,
            block: join_index,
            argument: 1
        })
    );
    for removed in [nodes[0], nodes[2]] {
        let erased = map
            .data
            .events
            .iter()
            .filter(|event| matches!(event.change, Change::Erase(id) if id as usize == removed))
            .collect::<Vec<_>>();
        assert_eq!(erased.len(), 1);
        assert_eq!(erased[0].pass, 3);
    }
    for retained in [nodes[1], nodes[3]] {
        assert!(
            !map.data
                .events
                .iter()
                .any(|event| matches!(event.change, Change::Erase(id) if id as usize == retained))
        );
    }
    let constants = join
        .operations
        .iter()
        .enumerate()
        .filter_map(|(index, operation)| {
            matches!(
                &operation.kind,
                OperationKind::Constant(fe2o3_kernel_ir::Constant::U32(7))
            )
            .then_some(Coordinate::Operation {
                function: 0,
                block: join_index,
                operation: u32::try_from(index).unwrap(),
            })
        })
        .collect::<Vec<_>>();
    assert_eq!(constants.len(), 1);
    let materialized = constants[0];
    let replacement = map
        .data
        .events
        .iter()
        .find_map(|event| match event.change {
            Change::Replace(old, new) if old as usize == nodes[0] => {
                Some((event.pass, new as usize))
            }
            _ => None,
        })
        .unwrap();
    assert_eq!(replacement.0, 0);
    assert!(map.data.nodes[replacement.1].input.is_none());
    assert_eq!(
        map.data.terminal[replacement.1],
        Some(Endpoint::Result {
            operation: materialized,
            result: 0
        })
    );
    assert!(
        map.synthesized_operations().contains(&materialized),
        "block-argument facts are not fabricated source-operation provenance"
    );
    assert!(
        !map.data.events.iter().any(
            |event| matches!(event.change, Change::Replace(old, _) if old as usize == nodes[2])
        )
    );
    // Both predecessor operand lists must follow the surviving argument order.
    let incoming = blocks
        .iter()
        .filter_map(|block| match &block.terminator {
            Some(Terminator::Branch { target, arguments }) if *target == join.id => Some(arguments),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(incoming.len(), 2);
    assert!(incoming.iter().all(|arguments| arguments.len() == 2));
}
