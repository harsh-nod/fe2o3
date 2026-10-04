use super::fixture::*;
use super::*;

#[test]
fn bounded_unroll_literal_widths_plain_checked_zero_one_many_and_ceiling() {
    for s in [
        ScalarType::U8,
        ScalarType::U16,
        ScalarType::U32,
        ScalarType::U64,
    ] {
        for checked in [false, true] {
            for n in [0, 1, 2, 3, 8] {
                with_input(fixture(s, Some((0, n)), 1, checked), |input, b| {
                    let o = run(input, b);
                    assert_eq!(
                        o.origins().selection,
                        Some(Selection {
                            fact: 0,
                            iterations: n as u8
                        })
                    );
                    let blocks = &o.output().module().functions[0]
                        .body
                        .as_ref()
                        .unwrap()
                        .blocks;
                    assert_eq!(blocks.len(), 3 + 2 * n as usize);
                    assert_eq!(
                        o.origins()
                            .blocks
                            .iter()
                            .filter(|r| matches!(r.copy, CopyRole::Header(_)))
                            .count(),
                        n as usize + 1
                    );
                    assert_eq!(
                        o.origins()
                            .blocks
                            .iter()
                            .filter(|r| matches!(r.copy, CopyRole::Body(_)))
                            .count(),
                        n as usize
                    );
                    assert_eq!(
                        o.origins()
                            .blocks
                            .iter()
                            .filter(|r| r.copy == CopyRole::OmittedBody)
                            .count(),
                        usize::from(n == 0)
                    );
                    assert_eq!(
                        o.origins()
                            .edges
                            .iter()
                            .filter(|r| matches!(r.copy, CopyRole::Header(_)) && r.output.is_none())
                            .count(),
                        n as usize + 1
                    );
                    assert_eq!(blocks[0].id, BlockId(13));
                    assert_eq!(blocks.last().unwrap().id, BlockId(701));
                    assert_eq!(blocks[1].id, BlockId(702));
                    assert_eq!(blocks[1].parameters[0].id, ValueId(41));
                    replay(&o, input, b);
                    release(o, b);
                });
            }
        }
    }
}
#[test]
fn bounded_unroll_nondivisible_stride_and_last_nonwrapping_update() {
    for (s, a, b, step, n) in [
        (ScalarType::U8, 1, 8, 3, 3),
        (ScalarType::U8, 250, 255, 1, 5),
        (ScalarType::U16, 65526, 65535, 3, 3),
        (ScalarType::U32, u32::MAX as u64 - 9, u32::MAX as u64, 3, 3),
        (ScalarType::U64, u64::MAX - 9, u64::MAX, 3, 3),
        (ScalarType::U32, 9, 3, 1, 0),
    ] {
        with_input(fixture(s, Some((a, b)), step, true), |input, budget| {
            let o = run(input, budget);
            assert_eq!(o.origins().selection.unwrap().iterations, n);
            replay(&o, input, budget);
            release(o, budget);
        });
    }
}
#[test]
fn bounded_unroll_diamond_parameters_and_duplicate_successor_occurrences() {
    for duplicate in [false, true] {
        let mut m = diamond(ScalarType::U32, 3, true);
        if duplicate {
            let blocks = &mut m.functions[0].body.as_mut().unwrap().blocks;
            let Some(Terminator::ConditionalBranch { else_target, .. }) = &mut blocks[2].terminator
            else {
                panic!("diamond");
            };
            *else_target = BlockId(100);
            // The now-disconnected right branch is outside the natural loop and remains exact.
            blocks[5].terminator = Some(Terminator::Return { values: vec![] });
        }
        with_input(m, |input, b| {
            let o = run(input, b);
            assert_eq!(o.origins().selection.unwrap().iterations, 3);
            let old_body = Block {
                function: fe2o3_kernel_ir::CanonicalKirFunctionCoordinateV1(0),
                block: 2,
            };
            for n in 0..3 {
                let edges: Vec<_> = o
                    .origins()
                    .edges
                    .iter()
                    .filter(|r| r.input.source == old_body && r.copy == CopyRole::Body(n))
                    .collect();
                assert_eq!(edges.len(), 2);
                assert_eq!(edges[0].input.successor, 0);
                assert_eq!(edges[1].input.successor, 1);
                assert_ne!(edges[0].output, edges[1].output);
            }
            replay(&o, input, b);
            release(o, b);
        });
    }
}
#[test]
fn bounded_unroll_unsupported_trip_scalar_overflow_and_symbolic_are_exact_noops() {
    for m in [
        fixture(ScalarType::U32, Some((0, 9)), 1, true),
        fixture(ScalarType::U8, Some((250, 255)), 10, true),
        fixture(ScalarType::U32, None, 1, true),
        fixture(ScalarType::I32, Some((0, 3)), 1, true),
        fixture(ScalarType::Index, Some((0, 3)), 1, true),
    ] {
        no_op(m);
    }
}
#[test]
fn bounded_unroll_alloca_switch_and_normal_early_exit_are_exact_noops() {
    for mode in 0..3 {
        let mut m = fixture(ScalarType::U32, Some((0, 3)), 1, true);
        let b = &mut m.functions[0].body.as_mut().unwrap().blocks;
        match mode {
            0 => b[2].operations.push(Operation::effect_free(
                ValueDef::new(
                    ValueId(50),
                    Type::pointer(
                        Type::Scalar(ScalarType::U32),
                        AddressSpace::Private,
                        fe2o3_kernel_ir::AccessMode::ReadWrite,
                    ),
                ),
                OperationKind::Alloca {
                    element: Type::Scalar(ScalarType::U32),
                    count: None,
                    address_space: AddressSpace::Private,
                    alignment: 4,
                },
            )),
            1 => {
                b[2].terminator = Some(Terminator::Switch {
                    selector: ValueId(25),
                    cases: vec![fe2o3_kernel_ir::SwitchCase {
                        value: 0,
                        target: BlockId(41),
                        arguments: vec![ValueId(30)],
                    }],
                    default_target: BlockId(41),
                    default_arguments: vec![ValueId(30)],
                })
            }
            2 => {
                b[2].terminator = Some(Terminator::ConditionalBranch {
                    condition: ValueId(2),
                    then_target: BlockId(41),
                    then_arguments: vec![ValueId(30)],
                    else_target: BlockId(701),
                    else_arguments: vec![ValueId(25)],
                })
            }
            _ => unreachable!(),
        }
        no_op(m);
    }
}
#[test]
fn bounded_unroll_loop_external_ssa_use_is_not_implicitly_repaired() {
    let mut m = fixture(ScalarType::U32, Some((0, 3)), 1, true);
    m.functions[0].body.as_mut().unwrap().blocks[3]
        .operations
        .push(Operation::effect_free(
            ValueDef::new(ValueId(55), scalar(ScalarType::U32)),
            OperationKind::Binary {
                op: BinaryOp::Add,
                lhs: ValueId(20),
                rhs: ValueId(10),
            },
        ));
    no_op(m);
}
#[test]
fn bounded_unroll_first_qualifying_fact_and_other_functions_remain_unchanged() {
    let mut m = fixture(ScalarType::U32, None, 1, true);
    let mut qualifying = fixture(ScalarType::U32, Some((0, 2)), 1, true)
        .functions
        .remove(0);
    qualifying.id = "second".into();
    m.functions.push(qualifying);
    let mut third = m.functions[1].clone();
    third.id = "third".into();
    m.functions.push(third);
    with_input(m, |input, b| {
        let o = run(input, b);
        assert_eq!(
            o.origins().selection,
            Some(Selection {
                fact: 1,
                iterations: 2
            })
        );
        assert_eq!(
            input.module().functions[0],
            o.output().module().functions[0]
        );
        assert_eq!(
            input.module().functions[2],
            o.output().module().functions[2]
        );
        assert_ne!(
            input.module().functions[1],
            o.output().module().functions[1]
        );
        replay(&o, input, b);
        release(o, b);
    });
}
#[test]
fn bounded_unroll_id_overflow_refuses_before_candidate_copy() {
    for value in [false, true] {
        let mut m = fixture(ScalarType::U32, Some((0, 2)), 1, true);
        let blocks = &mut m.functions[0].body.as_mut().unwrap().blocks;
        if value {
            blocks[0]
                .operations
                .push(literal(u32::MAX, ScalarType::U32, 0));
        } else {
            blocks[3].id = BlockId(u32::MAX);
            let Some(Terminator::ConditionalBranch { else_target, .. }) = &mut blocks[1].terminator
            else {
                panic!("header");
            };
            *else_target = BlockId(u32::MAX);
        }
        with_input(m, |input, b| {
            let floor = b.storage();
            assert!(matches!(
                unroll_canonical_kir_loops_v1(input, Limits::default(), b),
                Err(Error::Resource(Resource::Arithmetic))
            ));
            assert_eq!(b.storage(), floor);
        });
    }
}
#[test]
fn bounded_unroll_complete_rows_and_actual_admitted_payload_mutations_reject() {
    with_input(diamond(ScalarType::U32, 2, true), |input, b| {
        let mut o = run(input, b);
        for mode in 0..9 {
            let saved = o.rows.blocks.clone();
            let defs = o.rows.definitions.clone();
            let ops = o.rows.operations.clone();
            let terms = o.rows.terminators.clone();
            let edges = o.rows.edges.clone();
            let args = o.rows.arguments.clone();
            let selection = o.rows.selection;
            match mode {
                0 => o.rows.selection.as_mut().unwrap().iterations = 1,
                1 => o.rows.blocks[1].copy = CopyRole::Header(1),
                2 => {
                    o.rows.blocks.pop();
                }
                3 => o.rows.definitions.swap(4, 5),
                4 => {
                    o.rows.operations.pop();
                }
                5 => {
                    o.rows.terminators.pop();
                }
                6 => o.rows.edges.swap(1, 2),
                7 => {
                    o.rows.arguments.pop();
                }
                8 => o.rows.blocks[1].input.block = 2,
                _ => unreachable!(),
            }
            assert!(matches!(
                check_pair(input, o.output(), o.origins(), Limits::default(), b),
                Err(PairError::Mismatch(_))
            ));
            o.rows.blocks.clear();
            o.rows.blocks.extend_from_slice(&saved);
            o.rows.definitions.clear();
            o.rows.definitions.extend_from_slice(&defs);
            o.rows.operations.clear();
            o.rows.operations.extend_from_slice(&ops);
            o.rows.terminators.clear();
            o.rows.terminators.extend_from_slice(&terms);
            o.rows.edges.clear();
            o.rows.edges.extend_from_slice(&edges);
            o.rows.arguments.clear();
            o.rows.arguments.extend_from_slice(&args);
            o.rows.selection = selection;
        }
        for mode in 0..4 {
            let mut changed = o.output().module().clone();
            match mode {
                0 => changed.id = "different".into(),
                1 => {
                    let OperationKind::Constant(c) =
                        &mut changed.functions[0].body.as_mut().unwrap().blocks[0].operations[0]
                            .kind
                    else {
                        panic!("step");
                    };
                    *c = Constant::U32(2);
                }
                2 => {
                    let block = &mut changed.functions[0].body.as_mut().unwrap().blocks[1];
                    let OperationKind::Compare { predicate, .. } = &mut block.operations[0].kind
                    else {
                        panic!("guard");
                    };
                    *predicate = ComparePredicate::LessThanOrEqual;
                }
                3 => {
                    let block = &mut changed.functions[0].body.as_mut().unwrap().blocks[2];
                    let Some(Terminator::ConditionalBranch { condition, .. }) =
                        &mut block.terminator
                    else {
                        panic!("body");
                    };
                    *condition = ValueId(21);
                }
                _ => unreachable!(),
            }
            // This mutation needs the cloned header's Bool, not an erased old ID.
            if mode == 3 {
                let bool_id = changed.functions[0].body.as_ref().unwrap().blocks[1].operations[0]
                    .results[0]
                    .id;
                let Some(Terminator::ConditionalBranch { condition, .. }) =
                    &mut changed.functions[0].body.as_mut().unwrap().blocks[2].terminator
                else {
                    panic!("body");
                };
                *condition = bool_id;
            }
            let (actual, bytes) = admit(&changed);
            b.reserve_storage(bytes).unwrap();
            assert!(matches!(
                check_pair(input, &actual, o.origins(), Limits::default(), b),
                Err(PairError::Mismatch(_))
            ));
            drop(actual);
            b.release_storage(bytes).unwrap();
        }
        replay(&o, input, b);
        release(o, b);
    });
}
#[test]
fn bounded_unroll_repeat_output_and_all_lineage_are_deterministic() {
    with_input(diamond(ScalarType::U64, 3, true), |input, b| {
        let first = run(input, b);
        let second = run(input, b);
        assert_eq!(
            first.output().canonical().canonical_bytes(),
            second.output().canonical().canonical_bytes()
        );
        assert_eq!(first.rows.selection, second.rows.selection);
        assert_eq!(first.rows.blocks, second.rows.blocks);
        assert_eq!(first.rows.definitions, second.rows.definitions);
        assert_eq!(first.rows.operations, second.rows.operations);
        assert_eq!(first.rows.terminators, second.rows.terminators);
        assert_eq!(first.rows.edges, second.rows.edges);
        assert_eq!(first.rows.arguments, second.rows.arguments);
        replay(&first, input, b);
        replay(&second, input, b);
        release(second, b);
        release(first, b);
    });
}

#[test]
fn bounded_unroll_nested_and_irreducible_regions_remain_exact_noops() {
    let mut nested = fixture(ScalarType::U32, Some((0, 3)), 1, true);
    let b = &mut nested.functions[0].body.as_mut().unwrap().blocks;
    b[0].operations.push(literal(50, ScalarType::U32, 0));
    let Some(Terminator::ConditionalBranch {
        then_target,
        then_arguments,
        ..
    }) = &mut b[1].terminator
    else {
        panic!("header");
    };
    *then_target = BlockId(110);
    then_arguments.clear();
    b.extend([
        block(110, vec![], vec![], branch(111, &[50])),
        block(
            111,
            vec![ValueDef::new(ValueId(52), scalar(ScalarType::U32))],
            vec![Operation::effect_free(
                ValueDef::new(ValueId(53), Type::BOOL),
                OperationKind::Compare {
                    predicate: ComparePredicate::LessThan,
                    lhs: ValueId(52),
                    rhs: ValueId(12),
                },
            )],
            Terminator::ConditionalBranch {
                condition: ValueId(53),
                then_target: BlockId(112),
                then_arguments: vec![],
                else_target: BlockId(97),
                else_arguments: vec![ValueId(20)],
            },
        ),
        block(
            112,
            vec![],
            vec![Operation::effect_free(
                ValueDef::new(ValueId(54), scalar(ScalarType::U32)),
                OperationKind::Binary {
                    op: BinaryOp::Add,
                    lhs: ValueId(52),
                    rhs: ValueId(10),
                },
            )],
            branch(111, &[54]),
        ),
    ]);
    no_op(nested);
    let mut irreducible = fixture(ScalarType::U32, Some((0, 3)), 1, true);
    let b = &mut irreducible.functions[0].body.as_mut().unwrap().blocks;
    b[0].terminator = Some(Terminator::ConditionalBranch {
        condition: ValueId(2),
        then_target: BlockId(41),
        then_arguments: vec![ValueId(11)],
        else_target: BlockId(97),
        else_arguments: vec![ValueId(11)],
    });
    let OperationKind::Binary { lhs, .. } = &mut b[2].operations[0].kind else {
        panic!("update");
    };
    *lhs = ValueId(25);
    no_op(irreducible);
}
#[test]
fn bounded_unroll_trap_calls_and_convergent_operations_are_not_cloned() {
    use fe2o3_kernel_ir::{
        AmdGpuDiagnosticOperation, Barrier, BarrierSemantics, MemoryOrdering, SynchronizationScope,
    };
    for trap in [false, true] {
        let mut m = fixture(ScalarType::U32, Some((0, 3)), 1, true);
        if trap {
            let blocks = &mut m.functions[0].body.as_mut().unwrap().blocks;
            blocks[2].terminator = Some(Terminator::ConditionalBranch {
                condition: ValueId(2),
                then_target: BlockId(41),
                then_arguments: vec![ValueId(30)],
                else_target: BlockId(997),
                else_arguments: vec![],
            });
            // Keep the real backedge and a separately well-formed terminating exit.
            blocks.push(block(
                997,
                vec![],
                vec![AmdGpuDiagnosticOperation::Trap.operation(None)],
                Terminator::Unreachable,
            ));
            m.functions
                .push(AmdGpuDiagnosticOperation::Trap.declaration());
        } else {
            m.functions[0].body.as_mut().unwrap().blocks[2]
                .operations
                .push(Operation::new(
                    vec![],
                    OperationKind::Barrier(Barrier {
                        execution_scope: SynchronizationScope::Workgroup,
                        memory_scope: SynchronizationScope::Workgroup,
                        semantics: BarrierSemantics::new(
                            MemoryOrdering::AcquireRelease,
                            [AddressSpace::Global],
                        ),
                    }),
                ));
        }
        no_op(m);
    }
}
#[test]
fn bounded_unroll_invalid_ssa_dominance_is_an_input_admission_failure() {
    let mut m = fixture(ScalarType::U32, Some((0, 3)), 1, true);
    let OperationKind::Compare { rhs, .. } =
        &mut m.functions[0].body.as_mut().unwrap().blocks[1].operations[0].kind
    else {
        panic!("guard");
    };
    *rhs = ValueId(30);
    let mut w = Work::new(W);
    let mut b = Budget::new(&mut w, S);
    assert!(Owner::from_module_ref_with_verification_budget_v12(&m, &mut b).is_err());
    assert_eq!(b.storage(), 0);
}
#[test]
fn bounded_unroll_actual_wrong_latch_value_and_cross_copy_operand_refuse() {
    with_input(
        fixture(ScalarType::U32, Some((0, 3)), 1, true),
        |input, b| {
            let o = run(input, b);
            for cross_copy in [false, true] {
                let mut m = o.output().module().clone();
                let blocks = &mut m.functions[0].body.as_mut().unwrap().blocks;
                if cross_copy {
                    let first_header = blocks[1].parameters[0].id;
                    let OperationKind::Binary { lhs, .. } = &mut blocks[4].operations[0].kind
                    else {
                        panic!("second body");
                    };
                    *lhs = first_header;
                } else {
                    let Some(Terminator::Branch { arguments, .. }) = &mut blocks[2].terminator
                    else {
                        panic!("first latch");
                    };
                    arguments[0] = ValueId(10);
                }
                let (actual, bytes) = admit(&m);
                b.reserve_storage(bytes).unwrap();
                assert!(matches!(
                    check_pair(input, &actual, o.origins(), Limits::default(), b),
                    Err(PairError::Mismatch(_))
                ));
                drop(actual);
                b.release_storage(bytes).unwrap();
            }
            release(o, b);
        },
    );
}

#[test]
fn bounded_unroll_slice_length_gep_and_index_payload_transport_keep_exact_types() {
    use fe2o3_kernel_ir::AccessMode;
    for n in [0, 2] {
        let mut m = fixture(ScalarType::U32, Some((0, n)), 1, true);
        let f = &mut m.functions[0];
        f.signature.parameters.push(Type::slice(
            scalar(ScalarType::U32),
            AddressSpace::Global,
            AccessMode::ReadOnly,
        ));
        let body = f.body.as_mut().unwrap();
        body.parameters.push(ValueId(3));
        let b = &mut body.blocks;
        b[0].operations.push(Operation::effect_free(
            ValueDef::new(ValueId(50), Type::INDEX),
            OperationKind::SliceLength { slice: ValueId(3) },
        ));
        let Some(Terminator::Branch { arguments, .. }) = &mut b[0].terminator else {
            panic!("preheader");
        };
        arguments.push(ValueId(50));
        b[1].parameters
            .push(ValueDef::new(ValueId(22), Type::INDEX));
        b[1].operations.push(Operation::effect_free(
            ValueDef::new(ValueId(51), Type::INDEX),
            OperationKind::SliceLength { slice: ValueId(3) },
        ));
        let Some(Terminator::ConditionalBranch {
            then_arguments,
            else_arguments,
            ..
        }) = &mut b[1].terminator
        else {
            panic!("header");
        };
        then_arguments.push(ValueId(22));
        else_arguments.push(ValueId(22));
        b[2].parameters
            .push(ValueDef::new(ValueId(26), Type::INDEX));
        let pointer = || {
            Type::pointer(
                scalar(ScalarType::U32),
                AddressSpace::Global,
                AccessMode::ReadOnly,
            )
        };
        b[2].operations.extend([
            Operation::effect_free(
                ValueDef::new(ValueId(52), pointer()),
                OperationKind::SliceData { slice: ValueId(3) },
            ),
            Operation::effect_free(
                ValueDef::new(ValueId(53), pointer()),
                OperationKind::GetElementPointer {
                    base: ValueId(52),
                    offset: ValueId(26),
                },
            ),
            Operation::effect_free(
                ValueDef::new(ValueId(54), pointer()),
                OperationKind::GetElementPointer {
                    base: ValueId(52),
                    offset: ValueId(51),
                },
            ),
        ]);
        let Some(Terminator::Branch { arguments, .. }) = &mut b[2].terminator else {
            panic!("latch");
        };
        arguments.push(ValueId(22));
        b[3].parameters
            .push(ValueDef::new(ValueId(41), Type::INDEX));
        with_input(m, |input, budget| {
            let o = run(input, budget);
            assert_eq!(o.origins().selection.unwrap().iterations, n as u8);
            let output = &o.output().module().functions[0]
                .body
                .as_ref()
                .unwrap()
                .blocks;
            assert_eq!(
                output
                    .iter()
                    .flat_map(|b| &b.operations)
                    .filter(|op| matches!(op.kind, OperationKind::SliceLength { .. }))
                    .count(),
                n as usize + 2
            );
            assert_eq!(
                output
                    .iter()
                    .flat_map(|b| &b.operations)
                    .filter(|op| matches!(op.kind, OperationKind::GetElementPointer { .. }))
                    .count(),
                n as usize * 2
            );
            replay(&o, input, budget);
            release(o, budget);
        });
    }
}
#[test]
fn bounded_unroll_opaque_index_does_not_admit_index_arithmetic_constants_or_casts() {
    for mode in 0..3 {
        let mut m = fixture(ScalarType::U32, Some((0, 2)), 1, true);
        let f = &mut m.functions[0];
        f.signature.parameters.push(Type::INDEX);
        let body = f.body.as_mut().unwrap();
        body.parameters.push(ValueId(3));
        let kind = match mode {
            0 => OperationKind::Constant(Constant::Index(0)),
            1 => OperationKind::Binary {
                op: BinaryOp::Add,
                lhs: ValueId(3),
                rhs: ValueId(3),
            },
            2 => OperationKind::Cast {
                kind: CastKind::ZeroExtend,
                value: ValueId(25),
                to: Type::INDEX,
            },
            _ => unreachable!(),
        };
        body.blocks[2].operations.push(Operation::effect_free(
            ValueDef::new(ValueId(51), Type::INDEX),
            kind,
        ));
        no_op(m);
    }
}
#[test]
fn bounded_unroll_disjoint_loops_select_first_qualifying_in_same_function() {
    let mut m = fixture(ScalarType::U32, Some((0, 9)), 1, true);
    let b = &mut m.functions[0].body.as_mut().unwrap().blocks;
    b[3].terminator = Some(branch(801, &[]));
    b.extend([
        block(
            801,
            vec![],
            vec![
                literal(80, ScalarType::U32, 0),
                literal(81, ScalarType::U32, 2),
            ],
            branch(841, &[80]),
        ),
        block(
            841,
            vec![ValueDef::new(ValueId(90), scalar(ScalarType::U32))],
            vec![Operation::effect_free(
                ValueDef::new(ValueId(91), Type::BOOL),
                OperationKind::Compare {
                    predicate: ComparePredicate::LessThan,
                    lhs: ValueId(90),
                    rhs: ValueId(81),
                },
            )],
            Terminator::ConditionalBranch {
                condition: ValueId(91),
                then_target: BlockId(897),
                then_arguments: vec![],
                else_target: BlockId(970),
                else_arguments: vec![],
            },
        ),
        block(
            897,
            vec![],
            vec![Operation::checked_binary(
                ValueDef::new(ValueId(93), scalar(ScalarType::U32)),
                ValueDef::new(ValueId(94), Type::BOOL),
                CheckedBinaryOperator::Add,
                ValueId(90),
                ValueId(10),
            )],
            branch(841, &[93]),
        ),
        block(970, vec![], vec![], Terminator::Return { values: vec![] }),
    ]);
    with_input(m, |input, budget| {
        let o = run(input, budget);
        assert_eq!(
            o.origins().selection,
            Some(Selection {
                fact: 1,
                iterations: 2
            })
        );
        let before = &input.module().functions[0].body.as_ref().unwrap().blocks;
        let after = &o.output().module().functions[0]
            .body
            .as_ref()
            .unwrap()
            .blocks;
        assert_eq!(&before[..4], &after[..4]);
        assert_eq!(after.len(), 11);
        replay(&o, input, budget);
        release(o, budget);
    });
}

#[test]
fn bounded_unroll_actual_kernel_launch_and_target_metadata_cannot_change() {
    use fe2o3_kernel_ir::{
        FunctionRole, Kernel, LaunchDomain, LaunchExtent, TargetCapability, WaveWidth,
        WorkgroupSize,
    };
    let mut m = fixture(ScalarType::U32, Some((0, 2)), 1, true);
    m.functions[0].role = FunctionRole::KernelEntry;
    m.kernels.push(Kernel::new(
        "unroll",
        "unroll",
        LaunchDomain::D1 {
            x: LaunchExtent::Dynamic,
        },
    ));
    with_input(m, |input, b| {
        let o = run(input, b);
        for mode in 0..4 {
            let mut changed = o.output().module().clone();
            match mode {
                0 => changed.kernels[0].id = "changed".into(),
                1 => {
                    changed.kernels[0].domain = LaunchDomain::D1 {
                        x: LaunchExtent::Static(8),
                    }
                }
                2 => changed.kernels[0].workgroup_size = Some(WorkgroupSize::new(1, 1, 1)),
                3 => {
                    changed.kernels[0]
                        .required_capabilities
                        .insert(TargetCapability::WaveWidth(WaveWidth::Wave64));
                }
                _ => unreachable!(),
            }
            let (actual, bytes) = admit(&changed);
            b.reserve_storage(bytes).unwrap();
            assert!(matches!(
                check_pair(input, &actual, o.origins(), Limits::default(), b),
                Err(PairError::Mismatch("module target/kernel/function roster"))
            ));
            drop(actual);
            b.release_storage(bytes).unwrap();
        }
        release(o, b);
    });
}

fn exact_origin_capacity(m: Module, trips: u64, expected: usize) {
    with_input(m, |input, budget| {
        let floor = budget.storage();
        let limits = Limits {
            max_origin_rows: expected,
            ..Limits::default()
        };
        let (o, storage) = unroll_canonical_kir_loops_v1(input, limits, budget).unwrap();
        budget.reserve_storage(storage.retained_storage()).unwrap();
        assert_eq!(
            o.origins().selection,
            Some(Selection {
                fact: 0,
                iterations: trips as u8
            })
        );
        let rows = o.origins();
        assert_eq!(
            rows.blocks.len()
                + rows.definitions.len()
                + rows.operations.len()
                + rows.terminators.len()
                + rows.edges.len()
                + rows.arguments.len(),
            expected
        );
        replay(&o, input, budget);
        release(o, budget);
        assert_eq!(budget.storage(), floor);
        let short = Limits {
            max_origin_rows: expected - 1,
            ..limits
        };
        let error = unroll_canonical_kir_loops_v1(input, short, budget).unwrap_err();
        let Error::OutputLimit {
            kind,
            actual,
            limit,
        } = error
        else {
            panic!("exact complete-origin cap");
        };
        assert_eq!(
            (kind, actual, limit),
            ("origin rows", expected, expected - 1)
        );
        assert_eq!(budget.storage(), floor);
    });
}

#[test]
fn bounded_unroll_exact_origin_cap_preserves_large_unaffected_tail() {
    for trips in [0, 1, 3, 8] {
        for length in [1u32, 128] {
            let mut m = fixture(ScalarType::U32, Some((0, trips)), 1, true);
            let blocks = &mut m.functions[0].body.as_mut().unwrap().blocks;
            blocks[3].terminator = Some(branch(9000, &[]));
            for n in 0..length {
                blocks.push(block(
                    9000 + n,
                    vec![],
                    vec![literal(1000 + n, ScalarType::U32, u64::from(n))],
                    if n + 1 == length {
                        Terminator::Return { values: vec![] }
                    } else {
                        branch(9001 + n, &[])
                    },
                ));
            }
            // Positive N: [2N+3,5N+9,2N+4,2N+3,3N+3,3N+3].
            // N=0 retains explicit omitted-body rows: [4,12,5,4,4,4].
            // Each tail adds block/definition/op/terminator and one net edge row.
            let original = if trips == 0 {
                33
            } else {
                25 + 17 * trips as usize
            };
            exact_origin_capacity(m, trips, original + 5 * length as usize);
        }
    }
}

#[test]
fn bounded_unroll_exact_origin_cap_counts_unaffected_function_arguments_once() {
    for trips in [0, 1, 3, 8] {
        for count in [1u32, 64] {
            let mut m = fixture(ScalarType::U32, Some((0, trips)), 1, true);
            for n in 0..count {
                m.functions.push(Function::internal_helper(
                    format!("keep_{n}"),
                    Signature::new(
                        vec![scalar(ScalarType::U32); 2],
                        vec![scalar(ScalarType::U32)],
                    ),
                    vec![ValueId(0), ValueId(1)],
                    vec![block(
                        0,
                        vec![],
                        vec![Operation::effect_free(
                            ValueDef::new(ValueId(2), scalar(ScalarType::U32)),
                            OperationKind::Binary {
                                op: BinaryOp::Add,
                                lhs: ValueId(0),
                                rhs: ValueId(1),
                            },
                        )],
                        Terminator::Return {
                            values: vec![ValueId(2)],
                        },
                    )],
                ));
            }
            // Each unchanged function adds [1,3,1,1,0,0], including both ABI args.
            let original = if trips == 0 {
                33
            } else {
                25 + 17 * trips as usize
            };
            exact_origin_capacity(m, trips, original + 6 * count as usize);
        }
    }
}
