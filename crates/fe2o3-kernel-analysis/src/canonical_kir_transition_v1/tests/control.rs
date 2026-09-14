use super::*;
use fe2o3_kernel_ir::{IntegerSwitchCase, SwitchCase, UnaryOp};

fn diamond() -> Module {
    let mut entry = BasicBlock::new(BlockId(1000));
    entry.terminator = Some(Terminator::ConditionalBranch {
        condition: ValueId(1),
        then_target: BlockId(7),
        then_arguments: vec![ValueId(2), ValueId(3)],
        else_target: BlockId(7),
        else_arguments: vec![ValueId(3), ValueId(2)],
    });
    let mut join = returning(7, vec![], &[10]);
    join.parameters = vec![
        ValueDef::new(ValueId(10), U32),
        ValueDef::new(ValueId(11), U32),
    ];
    module(
        vec![Type::BOOL, U32, U32],
        vec![U32],
        vec![1, 2, 3],
        vec![entry, join],
    )
}

#[test]
fn repeated_targets_keep_ordered_occurrences_and_removed_parameter_slots() {
    let input = diamond();
    let mut output = input.clone();
    let body = output.functions[0].body.as_mut().unwrap();
    body.blocks[1].parameters.pop();
    let Some(Terminator::ConditionalBranch {
        then_arguments,
        else_arguments,
        ..
    }) = body.blocks[0].terminator.as_mut()
    else {
        panic!("conditional fixture")
    };
    then_arguments.pop();
    else_arguments.pop();
    inspect(
        input,
        output,
        |a, b| {
            Plan {
                chains: vec![vec![(0, None)], vec![(1, None)]],
                relations: vec![(1, 1, R), (2, 2, R), (3, 3, R), (10, 10, R)],
                uses: vec![term(0, 0), term(0, 1), term(0, 3), term(1, 0)],
                edges: vec![edge(0, 0), edge(0, 1)],
                edge_arguments: vec![edge_arg(0, 0, 0), edge_arg(0, 1, 0)],
                ..Plan::default()
            }
            .rows(a, b)
        },
        |a, b, rows, floor| {
            accepted(a, b, rows, floor);
            rows.edges.swap(0, 1);
            // Restore output framing while deliberately exchanging equal-target origins.
            rows.edges[0].output = edge(0, 0);
            rows.edges[1].output = edge(0, 1);
            assert_eq!(
                rejected(a, b, rows, floor),
                Error::Rule("successor occurrence order")
            );
            rows.edges[0].input = edge(0, 0);
            rows.edges[1].input = edge(0, 1);
            rows.edge_arguments[1].input = edge_arg(0, 1, 1);
            assert_eq!(
                rejected(a, b, rows, floor),
                Error::Rule("edge argument parameter transport")
            );
        },
    );
}

#[test]
fn signed_and_wide_raw_switches_preserve_key_domains_and_duplicate_tuple_rosters() {
    for signed in [false, true] {
        let mut entry = BasicBlock::new(BlockId(99));
        let case_target = BlockId(11);
        entry.terminator = Some(if signed {
            Terminator::IntegerSwitch {
                selector: ValueId(1),
                cases: vec![
                    IntegerSwitchCase {
                        value: Constant::I32(-7),
                        target: case_target,
                        arguments: vec![ValueId(2)],
                    },
                    IntegerSwitchCase {
                        value: Constant::I32(9),
                        target: case_target,
                        arguments: vec![ValueId(3)],
                    },
                ],
                default_target: case_target,
                default_arguments: vec![ValueId(2)],
            }
        } else {
            Terminator::Switch {
                selector: ValueId(1),
                cases: vec![
                    SwitchCase {
                        value: 0,
                        target: case_target,
                        arguments: vec![ValueId(2)],
                    },
                    SwitchCase {
                        value: u64::MAX,
                        target: case_target,
                        arguments: vec![ValueId(3)],
                    },
                ],
                default_target: case_target,
                default_arguments: vec![ValueId(2)],
            }
        });
        let mut join = returning(11, vec![], &[10]);
        join.parameters.push(ValueDef::new(ValueId(10), U32));
        let selector = Type::Scalar(if signed {
            ScalarType::I32
        } else {
            ScalarType::U128
        });
        let original = module(
            vec![selector, U32, U32],
            vec![U32],
            vec![1, 2, 3],
            vec![entry, join],
        );
        inspect(
            original.clone(),
            original,
            |a, _| Rows::identity(a),
            |a, b, rows, floor| {
                accepted(a, b, rows, floor);
                rows.edge_arguments[1].input = edge_arg(0, 0, 0);
                assert_eq!(
                    rejected(a, b, rows, floor),
                    Error::Rule("edge argument parameter transport")
                );
            },
        );
    }
}

#[test]
fn empty_switch_default_connector_can_merge_and_retire_selector_occurrence() {
    for signed in [false, true] {
        let mut entry = BasicBlock::new(BlockId(1));
        entry.terminator = Some(if signed {
            Terminator::IntegerSwitch {
                selector: ValueId(1),
                cases: vec![],
                default_target: BlockId(7),
                default_arguments: vec![ValueId(2)],
            }
        } else {
            Terminator::Switch {
                selector: ValueId(1),
                cases: vec![],
                default_target: BlockId(7),
                default_arguments: vec![ValueId(2)],
            }
        });
        let mut join = returning(7, vec![], &[10]);
        join.parameters.push(ValueDef::new(ValueId(10), U32));
        let parameters = vec![
            Type::Scalar(if signed {
                ScalarType::I32
            } else {
                ScalarType::U128
            }),
            U32,
        ];
        let input = module(parameters.clone(), vec![U32], vec![1, 2], vec![entry, join]);
        let output = module(
            parameters,
            vec![U32],
            vec![1, 2],
            vec![returning(1, vec![], &[2])],
        );
        inspect(
            input,
            output,
            |a, b| {
                Plan {
                    chains: vec![vec![(0, Some(edge(0, 0))), (1, None)]],
                    relations: vec![(1, 1, R), (2, 2, R), (10, 2, S)],
                    uses: vec![term(1, 0)],
                    ..Plan::default()
                }
                .rows(a, b)
            },
            |a, b, rows, floor| accepted(a, b, rows, floor),
        );
    }
}

#[test]
fn constant_selection_and_multiple_merge_segments_preserve_original_tail_use() {
    let mut entry = BasicBlock::new(BlockId(900));
    entry.operations = vec![
        constant(1, Constant::Bool(true)),
        constant(2, Constant::U32(7)),
    ];
    entry.terminator = Some(Terminator::ConditionalBranch {
        condition: ValueId(1),
        then_target: BlockId(7),
        then_arguments: vec![ValueId(2)],
        else_target: BlockId(99),
        else_arguments: vec![],
    });
    let mut middle = BasicBlock::new(BlockId(7));
    middle.parameters.push(ValueDef::new(ValueId(10), U32));
    middle.operations.push(value(
        11,
        U32,
        OperationKind::Unary {
            op: UnaryOp::Not,
            operand: ValueId(10),
        },
    ));
    middle.terminator = Some(Terminator::Branch {
        target: BlockId(8),
        arguments: vec![ValueId(11)],
    });
    let mut tail = returning(8, vec![], &[20]);
    tail.parameters.push(ValueDef::new(ValueId(20), U32));
    let dead = returning(99, vec![constant(90, Constant::U32(99))], &[90]);
    let input = module(vec![], vec![U32], vec![], vec![entry, middle, tail, dead]);
    let output = module(
        vec![],
        vec![U32],
        vec![],
        vec![returning(900, vec![constant(44, Constant::U32(!7))], &[44])],
    );
    inspect(
        input,
        output,
        |a, b| {
            Plan {
                chains: vec![vec![
                    (0, Some(edge(0, 0))),
                    (1, Some(edge(1, 0))),
                    (2, None),
                ]],
                operations: vec![Origin::ConstantFrom(result(1, 0, 0))],
                relations: vec![(11, 44, S), (20, 44, S)],
                uses: vec![term(2, 0)],
                ..Plan::default()
            }
            .rows(a, b)
        },
        |a, b, rows, floor| {
            accepted(a, b, rows, floor);
            rows.segments[0].connector = Some(edge(0, 1));
            assert_eq!(rejected(a, b, rows, floor), Error::Rule("merge connector"));
        },
    );
}

#[test]
fn unknown_duplicate_target_branch_does_not_justify_merge_or_selection() {
    let input = diamond();
    let output = module(
        vec![Type::BOOL, U32, U32],
        vec![U32],
        vec![1, 2, 3],
        vec![returning(1000, vec![], &[2])],
    );
    inspect(
        input,
        output,
        |a, b| {
            Plan {
                chains: vec![vec![(0, Some(edge(0, 0))), (1, None)]],
                relations: vec![(1, 1, R), (2, 2, R), (3, 3, R), (10, 2, S)],
                uses: vec![term(1, 0)],
                ..Plan::default()
            }
            .rows(a, b)
        },
        |a, b, rows, floor| {
            assert_eq!(
                rejected(a, b, rows, floor),
                Error::Rule("merge source has another executable edge")
            )
        },
    );
}

#[test]
fn mutually_recursive_phi_inputs_do_not_create_an_unjustified_alias() {
    let mut entry = BasicBlock::new(BlockId(80));
    entry.terminator = Some(Terminator::Branch {
        target: BlockId(2),
        arguments: vec![ValueId(2), ValueId(3)],
    });
    let mut loop_block = BasicBlock::new(BlockId(2));
    loop_block.parameters = vec![
        ValueDef::new(ValueId(10), U32),
        ValueDef::new(ValueId(11), U32),
    ];
    loop_block.terminator = Some(Terminator::ConditionalBranch {
        condition: ValueId(1),
        then_target: BlockId(9),
        then_arguments: vec![ValueId(10), ValueId(11)],
        else_target: BlockId(2),
        else_arguments: vec![ValueId(11), ValueId(10)],
    });
    let mut exit = returning(9, vec![], &[20, 21]);
    exit.parameters = vec![
        ValueDef::new(ValueId(20), U32),
        ValueDef::new(ValueId(21), U32),
    ];
    let original = module(
        vec![Type::BOOL, U32, U32],
        vec![U32, U32],
        vec![1, 2, 3],
        vec![entry, loop_block, exit],
    );
    inspect(
        original.clone(),
        original,
        |a, b| {
            Plan {
                chains: vec![vec![(0, None)], vec![(1, None)], vec![(2, None)]],
                relations: vec![
                    (1, 1, R),
                    (2, 2, R),
                    (3, 3, R),
                    (10, 10, R),
                    (11, 11, R),
                    (20, 20, R),
                    (21, 21, R),
                    (10, 11, S),
                ],
                uses: vec![
                    term(0, 0),
                    term(0, 1),
                    term(1, 0),
                    term(1, 1),
                    term(1, 2),
                    term(1, 3),
                    term(1, 4),
                    term(2, 0),
                    term(2, 1),
                ],
                edges: vec![edge(0, 0), edge(1, 0), edge(1, 1)],
                edge_arguments: vec![
                    edge_arg(0, 0, 0),
                    edge_arg(0, 0, 1),
                    edge_arg(1, 0, 0),
                    edge_arg(1, 0, 1),
                    edge_arg(1, 1, 0),
                    edge_arg(1, 1, 1),
                ],
                ..Plan::default()
            }
            .rows(a, b)
        },
        |a, b, rows, floor| {
            assert_eq!(
                rejected(a, b, rows, floor),
                Error::Rule("unproved typed substitution")
            )
        },
    );
}

#[test]
fn grounded_loop_invariant_can_remove_phi_payloads_without_pruning_loop_edges() {
    let mut entry = BasicBlock::new(BlockId(80));
    entry.operations.push(constant(2, Constant::U32(7)));
    entry.terminator = Some(Terminator::Branch {
        target: BlockId(2),
        arguments: vec![ValueId(2)],
    });
    let mut loop_block = BasicBlock::new(BlockId(2));
    loop_block.parameters.push(ValueDef::new(ValueId(10), U32));
    loop_block.terminator = Some(Terminator::ConditionalBranch {
        condition: ValueId(1),
        then_target: BlockId(9),
        then_arguments: vec![ValueId(10)],
        else_target: BlockId(2),
        else_arguments: vec![ValueId(10)],
    });
    let mut exit = returning(9, vec![], &[20]);
    exit.parameters.push(ValueDef::new(ValueId(20), U32));
    let input = module(
        vec![Type::BOOL],
        vec![U32],
        vec![1],
        vec![entry, loop_block, exit],
    );
    let mut entry = BasicBlock::new(BlockId(80));
    entry.operations.push(constant(2, Constant::U32(7)));
    entry.terminator = Some(Terminator::Branch {
        target: BlockId(2),
        arguments: vec![],
    });
    let mut loop_block = BasicBlock::new(BlockId(2));
    loop_block.terminator = Some(Terminator::ConditionalBranch {
        condition: ValueId(1),
        then_target: BlockId(9),
        then_arguments: vec![],
        else_target: BlockId(2),
        else_arguments: vec![],
    });
    let output = module(
        vec![Type::BOOL],
        vec![U32],
        vec![1],
        vec![entry, loop_block, returning(9, vec![], &[2])],
    );
    inspect(
        input,
        output,
        |a, b| {
            Plan {
                chains: vec![vec![(0, None)], vec![(1, None)], vec![(2, None)]],
                operations: vec![Origin::Retained(op(0, 0))],
                relations: vec![(1, 1, R), (2, 2, R), (10, 2, S), (20, 2, S)],
                uses: vec![term(1, 0), term(2, 0)],
                edges: vec![edge(0, 0), edge(1, 0), edge(1, 1)],
                ..Plan::default()
            }
            .rows(a, b)
        },
        |a, b, rows, floor| accepted(a, b, rows, floor),
    );
}

fn entry_parameter_with_literal_backedge() -> Module {
    // Structural canonical admission permits this parameter, but no function
    // argument or initial CFG edge supplies it. This is not formal-memory
    // admission or an assertion that its first-execution value is true.
    let mut entry = BasicBlock::new(BlockId(4_000_000_000));
    entry
        .parameters
        .push(ValueDef::new(ValueId(77), Type::BOOL));
    entry.operations.push(constant(1, Constant::Bool(true)));
    entry.terminator = Some(Terminator::ConditionalBranch {
        condition: ValueId(77),
        then_target: entry.id,
        then_arguments: vec![ValueId(1)],
        else_target: BlockId(7),
        else_arguments: vec![],
    });
    module(
        vec![],
        vec![],
        vec![],
        vec![entry, returning(7, vec![], &[])],
    )
}

#[test]
fn entry_parameter_backedge_literal_does_not_justify_an_initial_alias() {
    let input = entry_parameter_with_literal_backedge();
    inspect(
        input.clone(),
        input.clone(),
        |a, _| Rows::identity(a),
        |a, b, rows, floor| accepted(a, b, rows, floor),
    );
    inspect(
        input.clone(),
        input,
        |a, b| {
            Plan {
                chains: vec![vec![(0, None)], vec![(1, None)]],
                operations: vec![Origin::Retained(op(0, 0))],
                relations: vec![(77, 77, R), (1, 1, R), (77, 1, S)],
                uses: vec![term(0, 0), term(0, 1)],
                edges: vec![edge(0, 0), edge(0, 1)],
                edge_arguments: vec![edge_arg(0, 0, 0)],
            }
            .rows(a, b)
        },
        |a, b, rows, floor| {
            assert_eq!(
                rejected(a, b, rows, floor),
                Error::Rule("unproved typed substitution")
            );
        },
    );
}

#[test]
fn entry_parameter_backedge_literal_does_not_prune_the_initial_false_edge() {
    let input = entry_parameter_with_literal_backedge();
    let mut output = input.clone();
    let body = output.functions[0].body.as_mut().unwrap();
    body.blocks.pop();
    body.blocks[0].terminator = Some(Terminator::Branch {
        target: body.blocks[0].id,
        arguments: vec![ValueId(1)],
    });
    inspect(
        input,
        output,
        |a, b| {
            Plan {
                chains: vec![vec![(0, None)]],
                operations: vec![Origin::Retained(op(0, 0))],
                relations: vec![(77, 77, R), (1, 1, R)],
                uses: vec![term(0, 1)],
                edges: vec![edge(0, 0)],
                edge_arguments: vec![edge_arg(0, 0, 0)],
            }
            .rows(a, b)
        },
        |a, b, rows, floor| {
            assert_eq!(
                rejected(a, b, rows, floor),
                Error::Rule("executable edge occurrence removed")
            );
        },
    );
}
