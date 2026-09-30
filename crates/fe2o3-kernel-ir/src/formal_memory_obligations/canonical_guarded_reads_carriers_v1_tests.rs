use super::*;

#[derive(Clone, Copy)]
enum Carrier {
    Root,
    Conjuncts,
    NotOperand,
}

fn forwarded(carrier: Carrier) -> Module {
    let mut module = fixture();
    let body = module.functions[0].body.as_mut().unwrap();
    let mut guard = BasicBlock::new(BlockId(15));
    guard
        .parameters
        .push(ValueDef::new(ValueId(40), Type::BOOL));
    let (arguments, condition) = match carrier {
        Carrier::Root => (vec![ValueId(9)], ValueId(40)),
        Carrier::Conjuncts => {
            guard
                .parameters
                .push(ValueDef::new(ValueId(41), Type::BOOL));
            guard.operations.push(op(
                42,
                Type::BOOL,
                OperationKind::Binary {
                    op: BinaryOp::BitAnd,
                    lhs: ValueId(40),
                    rhs: ValueId(41),
                },
            ));
            (vec![ValueId(7), ValueId(8)], ValueId(42))
        }
        Carrier::NotOperand => {
            guard
                .parameters
                .push(ValueDef::new(ValueId(41), Type::BOOL));
            guard.operations.extend([
                op(
                    42,
                    Type::BOOL,
                    OperationKind::Unary {
                        op: crate::UnaryOp::Not,
                        operand: ValueId(41),
                    },
                ),
                op(
                    43,
                    Type::BOOL,
                    OperationKind::Binary {
                        op: BinaryOp::BitAnd,
                        lhs: ValueId(40),
                        rhs: ValueId(42),
                    },
                ),
            ]);
            (vec![ValueId(7), ValueId(6)], ValueId(43))
        }
    };
    body.blocks[0].terminator = Some(Terminator::Branch {
        target: BlockId(15),
        arguments,
    });
    guard.terminator = Some(Terminator::ConditionalBranch {
        condition,
        then_target: BlockId(20),
        then_arguments: vec![],
        else_target: BlockId(30),
        else_arguments: vec![],
    });
    body.blocks.push(guard);
    module
}

fn prove_carried_read(module: Module) {
    let legacy = derive_kernel_memory_obligations(
        &module,
        &KernelId::new("kernel"),
        ExplicitLaunchExtent1d::Exact(64),
        FormalIndexWidth::Bits64,
    )
    .unwrap();
    assert!(!legacy.is_complete());
    let graph = owner(module);
    run(&graph, |view, budget| {
        let at = coordinate(1, 2);
        let CanonicalGuardedGlobalReadOutcomeV1::ProvedLocalConditions(read) =
            view.read_at(at, budget)?
        else {
            panic!("the carried actual guard must bound the read");
        };
        assert!(std::ptr::eq(read.owner(), &graph));
        assert_eq!(read.operation(), at);
        assert_eq!(read.domain().slice(), ValueId(0));
        assert_eq!(read.domain().index(), ValueId(5));
        assert_eq!(read.domain().length(), ValueId(4));
        assert_eq!(read.domain().predicate(), ValueId(7));
        assert_eq!(read.domain().pointer(), ValueId(11));
        assert_eq!(
            read.domain().path(),
            FormalGuardedPathV1::TrueEdge {
                source: BlockId(15),
                ordinal: 0,
                target: BlockId(20),
            }
        );
        assert!(read.requires_runtime_allocation_binding());
        let bound = view.true_at(at, ValueId(7), budget)?.unwrap();
        assert_eq!(bound.value(), ValueId(7));
        assert_eq!(bound.edge(), (BlockId(15), 0, BlockId(20)));
        let no_wrap = view.no_wrap_at(at, coordinate(0, 1), budget)?.unwrap();
        assert_eq!(no_wrap.value(), ValueId(5));
        assert_eq!(no_wrap.overflow(), ValueId(6));
        assert_eq!(no_wrap.operands(), (ValueId(1), ValueId(2)));
        assert!(std::ptr::eq(no_wrap.predicate().owner(), &graph));
        assert!(!no_wrap.predicate().is_true());
        assert_eq!(no_wrap.predicate().edge(), (BlockId(15), 0, BlockId(20)));
        assert!(std::ptr::eq(
            no_wrap.operation(),
            &graph.module().functions[0].body.as_ref().unwrap().blocks[0].operations[1]
        ));
        Ok(())
    })
    .unwrap();
}

fn refuse_carried_read(module: Module) {
    let graph = owner(module);
    run(&graph, |view, budget| {
        let at = coordinate(1, 2);
        assert!(matches!(
            view.read_at(at, budget)?,
            CanonicalGuardedGlobalReadOutcomeV1::NotProved(_)
        ));
        assert!(view.true_at(at, ValueId(7), budget)?.is_none());
        assert!(view.false_at(at, ValueId(6), budget)?.is_none());
        assert!(view.no_wrap_at(at, coordinate(0, 1), budget)?.is_none());
        Ok(())
    })
    .unwrap();
}

#[test]
fn actual_forwarded_root_conjuncts_and_not_operand_keep_exact_read_subjects() {
    for carrier in [Carrier::Root, Carrier::Conjuncts, Carrier::NotOperand] {
        prove_carried_read(forwarded(carrier));
    }
}

#[derive(Clone, Copy)]
enum Cycle {
    Seeded,
    Unseeded,
    Mixed,
}

fn cyclic(kind: Cycle) -> Module {
    let mut module = forwarded(Carrier::Root);
    let body = module.functions[0].body.as_mut().unwrap();
    if matches!(kind, Cycle::Unseeded) {
        // Entry parameters have no initial SSA edge. A backedge cannot create
        // the missing seed, even though this complete graph is well typed.
        body.blocks[0]
            .parameters
            .push(ValueDef::new(ValueId(39), Type::BOOL));
        body.blocks[0].terminator = Some(Terminator::Branch {
            target: BlockId(15),
            arguments: vec![ValueId(39)],
        });
    }
    body.blocks[3].terminator = Some(Terminator::ConditionalBranch {
        condition: ValueId(40),
        then_target: BlockId(20),
        then_arguments: vec![],
        else_target: BlockId(16),
        else_arguments: vec![ValueId(40)],
    });
    let mut latch = BasicBlock::new(BlockId(16));
    latch
        .parameters
        .push(ValueDef::new(ValueId(41), Type::BOOL));
    latch.terminator = Some(if matches!(kind, Cycle::Mixed) {
        Terminator::ConditionalBranch {
            condition: ValueId(3),
            then_target: BlockId(15),
            then_arguments: vec![ValueId(41)],
            else_target: BlockId(15),
            else_arguments: vec![ValueId(3)],
        }
    } else {
        Terminator::Branch {
            target: BlockId(15),
            arguments: vec![ValueId(41)],
        }
    });
    body.blocks.push(latch);
    module
}

#[test]
fn actual_seeded_boolean_scc_preserves_the_guard_and_unseeded_or_mixed_do_not() {
    prove_carried_read(cyclic(Cycle::Seeded));
    refuse_carried_read(cyclic(Cycle::Unseeded));
    refuse_carried_read(cyclic(Cycle::Mixed));
}

#[test]
fn carried_true_or_and_false_and_do_not_introduce_operand_facts() {
    let mut disjunction = forwarded(Carrier::Root);
    disjunction.functions[0].body.as_mut().unwrap().blocks[0].operations[4].kind =
        OperationKind::Binary {
            op: BinaryOp::BitOr,
            lhs: ValueId(7),
            rhs: ValueId(8),
        };
    refuse_carried_read(disjunction);

    let mut false_and = forwarded(Carrier::Root);
    let guard = &mut false_and.functions[0].body.as_mut().unwrap().blocks[3];
    guard.operations.push(op(
        42,
        Type::BOOL,
        OperationKind::Unary {
            op: crate::UnaryOp::Not,
            operand: ValueId(40),
        },
    ));
    let Some(Terminator::ConditionalBranch { condition, .. }) = &mut guard.terminator else {
        unreachable!()
    };
    *condition = ValueId(42);
    let graph = owner(false_and.clone());
    run(&graph, |view, budget| {
        let false_root = view
            .false_at(coordinate(1, 2), ValueId(9), budget)?
            .unwrap();
        assert_eq!(false_root.value(), ValueId(9));
        assert_eq!(false_root.edge(), (BlockId(15), 0, BlockId(20)));
        Ok(())
    })
    .unwrap();
    refuse_carried_read(false_and);
}

#[test]
fn carried_guards_keep_false_edge_and_parallel_edge_bypasses_unknown() {
    for parallel in [false, true] {
        let mut module = forwarded(Carrier::Root);
        let guard = &mut module.functions[0].body.as_mut().unwrap().blocks[3];
        let Some(Terminator::ConditionalBranch {
            then_target,
            else_target,
            ..
        }) = &mut guard.terminator
        else {
            unreachable!()
        };
        *else_target = BlockId(20);
        if !parallel {
            *then_target = BlockId(30);
        }
        refuse_carried_read(module);
    }
}

#[test]
fn parallel_origin_edges_must_agree_even_when_the_read_edge_is_unique() {
    for same in [true, false] {
        let mut module = forwarded(Carrier::Root);
        module.functions[0].body.as_mut().unwrap().blocks[0].terminator =
            Some(Terminator::ConditionalBranch {
                condition: ValueId(3),
                then_target: BlockId(15),
                then_arguments: vec![ValueId(9)],
                else_target: BlockId(15),
                else_arguments: vec![if same { ValueId(9) } else { ValueId(8) }],
            });
        if same {
            prove_carried_read(module);
        } else {
            refuse_carried_read(module);
        }
    }
}

#[test]
fn wrong_typed_boolean_carrier_is_rejected_by_verified_owner_construction() {
    let mut module = forwarded(Carrier::Root);
    module.functions[0].body.as_mut().unwrap().blocks[3].parameters[0].ty = Type::INDEX;
    assert!(matches!(
        try_owner(&module),
        Err(crate::CanonicalKernelIrReplayAdmissionErrorV12::Verification(_))
    ));
    let graph = owner(forwarded(Carrier::Root));
    run(&graph, |view, budget| {
        assert!(
            view.true_at(coordinate(1, 2), ValueId(5), budget)?
                .is_none()
        );
        assert!(
            view.false_at(coordinate(1, 2), ValueId(5), budget)?
                .is_none()
        );
        Ok(())
    })
    .unwrap();
}

#[test]
fn carried_checked_flag_and_not_keep_polarity_without_a_bound() {
    for (seed, expected_no_wrap) in [(8, true), (6, false)] {
        let mut module = forwarded(Carrier::Root);
        module.functions[0].body.as_mut().unwrap().blocks[0].terminator =
            Some(Terminator::Branch {
                target: BlockId(15),
                arguments: vec![ValueId(seed)],
            });
        let graph = owner(module);
        run(&graph, |view, budget| {
            let at = coordinate(1, 2);
            assert!(matches!(
                view.read_at(at, budget)?,
                CanonicalGuardedGlobalReadOutcomeV1::NotProved(_)
            ));
            assert!(view.true_at(at, ValueId(7), budget)?.is_none());
            assert_eq!(
                view.false_at(at, ValueId(6), budget)?.is_some(),
                expected_no_wrap
            );
            assert_eq!(
                view.true_at(at, ValueId(6), budget)?.is_some(),
                !expected_no_wrap
            );
            let no_wrap = view.no_wrap_at(at, coordinate(0, 1), budget)?;
            assert_eq!(no_wrap.is_some(), expected_no_wrap);
            if let Some(fact) = no_wrap {
                assert_eq!(fact.overflow(), ValueId(6));
                assert!(!fact.predicate().is_true());
            }
            Ok(())
        })
        .unwrap();
    }
}
