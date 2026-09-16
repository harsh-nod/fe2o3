#[test]
fn no_overflow_proof_does_not_use_a_future_entry_backedge_guard() {
    let mut entry = BasicBlock::new(BlockId(0));
    entry.operations.extend([
        intrinsic(
            0,
            IntrinsicKind::InvocationIndex {
                kind: IndexKind::Global,
                axis: Axis::X,
            },
        ),
        cast(10, CastKind::Bitcast, 0, Type::Scalar(ScalarType::U64)),
        cast(1, CastKind::Truncate, 10, Type::Scalar(ScalarType::U32)),
        constant(2, Constant::U32(1)),
        constant(3, Constant::U32(u32::MAX)),
        Operation::checked_binary(
            ValueDef::new(ValueId(4), Type::Scalar(ScalarType::U32)),
            ValueDef::new(ValueId(5), Type::BOOL),
            CheckedBinaryOperator::Add,
            ValueId(1),
            ValueId(2),
        ),
        compare_with(6, ComparePredicate::LessThan, 1, 3),
    ]);
    // The initial invocation does not traverse this explicit backedge. At
    // x == u32::MAX the earlier addition overflows before the guard is false.
    entry.terminator = Some(Terminator::ConditionalBranch {
        condition: ValueId(6),
        then_target: BlockId(0),
        then_arguments: vec![],
        else_target: BlockId(1),
        else_arguments: vec![],
    });
    let mut exit = returning(1);
    exit.operations.push(workgroup_barrier());
    let function = function(vec![], vec![entry, exit]);
    let mut module = Module::new("entry_backedge_overflow");
    module.functions.push(function.clone());
    fe2o3_kernel_ir::verify_module(&module)
        .expect("entry-backedge fixture must be structurally well typed");
    let report = analyze_function(&function);
    assert_eq!(report.value(ValueId(5)), Variation::Varying);
    assert!(report.diagnostics().iter().any(|diagnostic| matches!(
        diagnostic,
        Diagnostic::DivergentBarrier {
            block: BlockId(1),
            ..
        }
    )));
}

#[derive(Clone, Copy, Debug)]
enum SafeOverflowControlCase {
    Exact,
    UnknownOverflow,
    ActualOverflow,
    WrongFlag,
    SumInsteadOfFlag,
    InvertedEdge,
    RetainedSideExit,
    SignedAdd,
}

// Only some lanes enter the arithmetic block. Its impossible overflow edge
// must disappear before that divergent branch can reconverge at the barrier.
fn safe_overflow_control(case: SafeOverflowControlCase) -> Function {
    let mut entry = BasicBlock::new(BlockId(0));
    entry.operations.extend([
        intrinsic(
            0,
            IntrinsicKind::InvocationIndex {
                kind: IndexKind::Global,
                axis: Axis::X,
            },
        ),
        constant(1, Constant::Index(0)),
        compare(2, 0, 1),
        cast(10, CastKind::Bitcast, 0, Type::Scalar(ScalarType::U64)),
        cast(3, CastKind::Truncate, 10, Type::Scalar(ScalarType::U32)),
    ]);
    entry.terminator = Some(Terminator::ConditionalBranch {
        condition: ValueId(2),
        then_target: BlockId(1),
        then_arguments: vec![],
        else_target: BlockId(3),
        else_arguments: vec![],
    });

    let mut arithmetic = BasicBlock::new(BlockId(1));
    let ty = if matches!(case, SafeOverflowControlCase::SignedAdd) {
        arithmetic.operations.extend([
            cast(8, CastKind::Bitcast, 3, Type::Scalar(ScalarType::I32)),
            cast(4, CastKind::SignExtend, 8, Type::Scalar(ScalarType::I64)),
            cast(5, CastKind::SignExtend, 8, Type::Scalar(ScalarType::I64)),
        ]);
        Type::Scalar(ScalarType::I64)
    } else {
        arithmetic.operations.extend(match case {
            SafeOverflowControlCase::UnknownOverflow => [
                cast(4, CastKind::Bitcast, 0, Type::Scalar(ScalarType::U64)),
                constant(5, Constant::U64(1)),
            ],
            SafeOverflowControlCase::ActualOverflow => [
                constant(4, Constant::U64(u64::MAX)),
                constant(5, Constant::U64(1)),
            ],
            _ => [
                cast(4, CastKind::ZeroExtend, 3, Type::Scalar(ScalarType::U64)),
                cast(5, CastKind::ZeroExtend, 3, Type::Scalar(ScalarType::U64)),
            ],
        });
        Type::Scalar(ScalarType::U64)
    };
    arithmetic.operations.push(Operation::checked_binary(
        ValueDef::new(ValueId(6), ty),
        ValueDef::new(ValueId(7), Type::BOOL),
        CheckedBinaryOperator::Add,
        ValueId(4),
        ValueId(5),
    ));
    let condition = match case {
        SafeOverflowControlCase::WrongFlag => ValueId(2),
        SafeOverflowControlCase::SumInsteadOfFlag => {
            arithmetic
                .operations
                .extend([constant(8, Constant::U64(0)), compare(9, 6, 8)]);
            ValueId(9)
        }
        _ => ValueId(7),
    };
    let success = if matches!(case, SafeOverflowControlCase::RetainedSideExit) {
        BlockId(4)
    } else {
        BlockId(3)
    };
    arithmetic.terminator = Some(match case {
        SafeOverflowControlCase::InvertedEdge => Terminator::ConditionalBranch {
            condition,
            then_target: success,
            then_arguments: vec![],
            else_target: BlockId(2),
            else_arguments: vec![],
        },
        _ => Terminator::ConditionalBranch {
            condition,
            then_target: BlockId(2),
            then_arguments: vec![],
            else_target: success,
            else_arguments: vec![],
        },
    });
    let mut trap = BasicBlock::new(BlockId(2));
    trap.terminator = Some(Terminator::Unreachable);
    let mut merge = returning(3);
    merge.operations.push(workgroup_barrier());
    let mut blocks = vec![entry, arithmetic, trap, merge];
    if matches!(case, SafeOverflowControlCase::RetainedSideExit) {
        let mut side_exit = BasicBlock::new(BlockId(4));
        side_exit.terminator = Some(Terminator::ConditionalBranch {
            condition: ValueId(2),
            then_target: BlockId(2),
            then_arguments: vec![],
            else_target: BlockId(3),
            else_arguments: vec![],
        });
        blocks.push(side_exit);
    }
    let function = function(vec![], blocks);
    let mut module = Module::new("safe_overflow_control");
    module.functions.push(function.clone());
    fe2o3_kernel_ir::verify_module(&module)
        .expect("overflow-control fixture must be structurally well typed");
    function
}

#[test]
fn exact_no_overflow_edge_restores_divergent_entry_reconvergence() {
    let report = analyze_function(&safe_overflow_control(SafeOverflowControlCase::Exact));
    assert_eq!(report.value(ValueId(6)), Variation::Varying);
    assert_eq!(report.value(ValueId(7)), Variation::GridUniform);
    assert_eq!(report.block_control(BlockId(1)), Variation::Varying);
    assert_eq!(report.block_control(BlockId(3)), Variation::GridUniform);
    assert!(
        report.diagnostics().is_empty(),
        "{:?}",
        report.diagnostics()
    );
}

#[test]
fn no_overflow_edge_refinement_retains_possible_or_wrong_edge_exits() {
    for case in [
        SafeOverflowControlCase::UnknownOverflow,
        SafeOverflowControlCase::ActualOverflow,
        SafeOverflowControlCase::WrongFlag,
        SafeOverflowControlCase::SumInsteadOfFlag,
        SafeOverflowControlCase::InvertedEdge,
        SafeOverflowControlCase::RetainedSideExit,
        SafeOverflowControlCase::SignedAdd,
    ] {
        let report = analyze_function(&safe_overflow_control(case));
        assert_eq!(
            report.block_control(BlockId(3)),
            Variation::Varying,
            "{case:?}"
        );
        assert!(
            report.diagnostics().iter().any(|diagnostic| matches!(
                diagnostic,
                Diagnostic::DivergentBarrier {
                    block: BlockId(3),
                    ..
                }
            )),
            "{case:?} discarded a possible varying exit: {:?}",
            report.diagnostics()
        );
    }
}

#[test]
fn no_overflow_refinement_does_not_make_payload_or_guard_uniform() {
    let report = analyze_function(&safe_overflow_control(SafeOverflowControlCase::Exact));
    for value in [
        ValueId(0),
        ValueId(2),
        ValueId(3),
        ValueId(4),
        ValueId(5),
        ValueId(6),
    ] {
        assert_eq!(report.value(value), Variation::Varying, "{value:?}");
    }

    assert_eq!(report.value(ValueId(7)), Variation::GridUniform);
    assert!(report.diagnostics().is_empty());
}

#[test]
fn no_overflow_refinement_rejects_malformed_checked_result_types() {
    for (result_index, mutated_type) in [
        (1, Type::Scalar(ScalarType::U32)),
        (0, Type::BOOL),
        (0, Type::Scalar(ScalarType::U32)),
    ] {
        let mut function = safe_overflow_control(SafeOverflowControlCase::Exact);
        let body = function.body.as_mut().expect("defined fixture");
        let arithmetic = body
            .blocks
            .iter_mut()
            .find(|block| block.id == BlockId(1))
            .expect("arithmetic block");
        let checked = arithmetic.operations.last_mut().expect("checked addition");
        checked.results[result_index].ty = mutated_type.clone();
        let mut module = Module::new("malformed_checked_result");
        module.functions.push(function.clone());
        assert!(fe2o3_kernel_ir::verify_module(&module).is_err());
        let report = analyze_function(&function);
        assert_eq!(
            report.block_control(BlockId(3)),
            Variation::Varying,
            "result {result_index} changed to {mutated_type:?}"
        );
        assert!(report.diagnostics().iter().any(|diagnostic| matches!(
            diagnostic,
            Diagnostic::DivergentBarrier {
                block: BlockId(3),
                ..
            }
        )));
    }
}
