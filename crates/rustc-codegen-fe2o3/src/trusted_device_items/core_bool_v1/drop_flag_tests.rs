use super::*;

fn flag<'tcx>(tcx: TyCtxt<'tcx>, value: bool) -> Statement<'tcx> {
    Statement::new(
        SourceInfo::outermost(DUMMY_SP),
        StatementKind::Assign(Box::new((
            Local::from_usize(4).into(),
            Rvalue::Use(Operand::Constant(Box::new(ConstOperand {
                span: DUMMY_SP,
                user_ty: None,
                const_: Const::from_bool(tcx, value),
            }))),
        ))),
    )
}

// Exact six-block capture from core-device-amdgpu-3.log. Reuse the pinned
// core's nominal types/aggregates; the ignored AMD test checks the real body.
fn captured_body<'tcx>(tcx: TyCtxt<'tcx>, source: &Body<'tcx>) -> Body<'tcx> {
    if source.basic_blocks.len() == 6 {
        return source.clone();
    }
    let TerminatorKind::SwitchInt { targets, .. } = &source.basic_blocks[BasicBlock::from_usize(0)]
        .terminator()
        .kind
    else {
        unreachable!()
    };
    let yes = targets.target_for_value(1);
    let no = targets.target_for_value(0);
    let take = source.basic_blocks[yes].statements[1].clone();
    let some = source.basic_blocks[yes].statements[2].clone();
    let none = source.basic_blocks[no].statements[0].clone();
    let mut drop = source.basic_blocks[no].terminator().kind.clone();
    let TerminatorKind::Drop { target, unwind, .. } = &mut drop else {
        unreachable!()
    };
    *target = BasicBlock::from_usize(4);
    *unwind = if tcx.sess.panic_strategy().unwinds() {
        UnwindAction::Continue
    } else {
        UnwindAction::Unreachable
    };
    let mut body = source.clone();
    let mut local = source.local_decls[Local::from_usize(1)].clone();
    local.mutability = rustc_hir::Mutability::Mut;
    body.local_decls.push(local);
    body.basic_blocks.as_mut().raw.clear();
    for (statements, kind) in [
        (
            vec![flag(tcx, false), flag(tcx, true)],
            TerminatorKind::SwitchInt {
                discr: Operand::Copy(Local::from_usize(1).into()),
                targets: SwitchTargets::static_if(
                    0,
                    BasicBlock::from_usize(2),
                    BasicBlock::from_usize(1),
                ),
            },
        ),
        (
            vec![flag(tcx, false), take, some],
            TerminatorKind::Goto {
                target: BasicBlock::from_usize(3),
            },
        ),
        (
            vec![none],
            TerminatorKind::Goto {
                target: BasicBlock::from_usize(3),
            },
        ),
        (
            vec![],
            TerminatorKind::SwitchInt {
                discr: Operand::Copy(Local::from_usize(4).into()),
                targets: SwitchTargets::static_if(
                    0,
                    BasicBlock::from_usize(4),
                    BasicBlock::from_usize(5),
                ),
            },
        ),
        (vec![], TerminatorKind::Return),
        (vec![], drop),
    ] {
        let mut block = BasicBlockData::new(
            Some(Terminator {
                source_info: SourceInfo::outermost(DUMMY_SP),
                kind,
            }),
            false,
        );
        block.statements = statements;
        body.basic_blocks.as_mut().push(block);
    }
    body
}

pub(super) fn check<'tcx>(tcx: TyCtxt<'tcx>, caller: &str) {
    let instance = helper(tcx, caller);
    let contract = contract(tcx, instance).unwrap();
    let body = captured_body(tcx, tcx.instance_mir(instance.def));
    assert!(
        reviewed_body(tcx, instance, &body, &contract),
        "captured AMD drop flag: {caller}"
    );
    let reject = |b: &Body<'tcx>, why| {
        assert!(
            !reviewed_body(tcx, instance, b, &contract),
            "{caller}: {why}"
        )
    };
    let bb = BasicBlock::from_usize;
    for mutation in 0..24 {
        let mut b = body.clone();
        match mutation {
            0 => b.basic_blocks.as_mut()[bb(0)].statements[1] = flag(tcx, false),
            1 => b.basic_blocks.as_mut()[bb(1)].statements[0] = flag(tcx, true),
            2 => b.basic_blocks.as_mut()[bb(0)].statements[0] = flag(tcx, true),
            3 => {
                b.basic_blocks.as_mut()[bb(1)].statements.remove(0);
            }
            4 => b.basic_blocks.as_mut()[bb(2)]
                .statements
                .push(flag(tcx, false)),
            5 => b.basic_blocks.as_mut()[bb(3)]
                .statements
                .push(flag(tcx, false)),
            6 => b.basic_blocks.as_mut()[bb(1)].statements.swap(0, 1),
            7 => b.local_decls[Local::from_usize(4)].ty = tcx.types.u8,
            8 => {
                let StatementKind::Assign(a) =
                    &mut b.basic_blocks.as_mut()[bb(0)].statements[1].kind
                else {
                    unreachable!()
                };
                a.1 = Rvalue::Use(Operand::Copy(Local::from_usize(1).into()));
            }
            9 => {
                let StatementKind::Assign(a) =
                    &mut b.basic_blocks.as_mut()[bb(1)].statements[1].kind
                else {
                    unreachable!()
                };
                a.1 = Rvalue::Use(Operand::Copy(Local::from_usize(2).into()));
            }
            10 => {
                b.basic_blocks.as_mut()[bb(5)].terminator_mut().kind =
                    TerminatorKind::Goto { target: bb(4) }
            }
            11 => {
                b.basic_blocks.as_mut()[bb(1)].terminator_mut().kind =
                    TerminatorKind::Goto { target: bb(5) }
            }
            12 => {
                b.basic_blocks.as_mut()[bb(2)].terminator_mut().kind =
                    TerminatorKind::Goto { target: bb(4) }
            }
            13 => {
                b.basic_blocks.as_mut()[bb(1)].terminator_mut().kind =
                    TerminatorKind::Goto { target: bb(6) }
            }
            14 => b.basic_blocks.as_mut()[bb(5)].is_cleanup = true,
            15 => {
                let TerminatorKind::Drop { place, .. } =
                    &mut b.basic_blocks.as_mut()[bb(5)].terminator_mut().kind
                else {
                    unreachable!()
                };
                *place = Local::from_usize(0).into();
            }
            16 => {
                let TerminatorKind::Drop { target, .. } =
                    &mut b.basic_blocks.as_mut()[bb(5)].terminator_mut().kind
                else {
                    unreachable!()
                };
                *target = bb(3);
            }
            17 => {
                let TerminatorKind::Drop { unwind, .. } =
                    &mut b.basic_blocks.as_mut()[bb(5)].terminator_mut().kind
                else {
                    unreachable!()
                };
                *unwind = UnwindAction::Cleanup(bb(4));
            }
            18 => {
                let TerminatorKind::Drop { unwind, .. } =
                    &mut b.basic_blocks.as_mut()[bb(5)].terminator_mut().kind
                else {
                    unreachable!()
                };
                *unwind = UnwindAction::Terminate(UnwindTerminateReason::Abi);
            }
            19 => {
                let TerminatorKind::Drop { replace, .. } =
                    &mut b.basic_blocks.as_mut()[bb(5)].terminator_mut().kind
                else {
                    unreachable!()
                };
                *replace = true;
            }
            20 => {
                let TerminatorKind::Drop { drop, .. } =
                    &mut b.basic_blocks.as_mut()[bb(5)].terminator_mut().kind
                else {
                    unreachable!()
                };
                *drop = Some(bb(4));
            }
            21 => {
                let TerminatorKind::Drop { async_fut, .. } =
                    &mut b.basic_blocks.as_mut()[bb(5)].terminator_mut().kind
                else {
                    unreachable!()
                };
                *async_fut = Some(Local::from_usize(2));
            }
            22 => {
                b.basic_blocks.as_mut()[bb(4)].terminator_mut().kind = TerminatorKind::Unreachable
            }
            23 => append_block(&mut b, TerminatorKind::Unreachable, false),
            _ => unreachable!(),
        }
        reject(
            &b,
            "flag ownership, ordering, payload move, Drop, target, unwind, or whole-body budget",
        );
    }
    for root in [0, 3] {
        for mutation in 0..7 {
            let mut b = body.clone();
            let TerminatorKind::SwitchInt { discr, targets } =
                &mut b.basic_blocks.as_mut()[bb(root)].terminator_mut().kind
            else {
                unreachable!()
            };
            let zero = targets.target_for_value(0);
            let one = targets.target_for_value(1);
            match mutation {
                0 => *targets = SwitchTargets::static_if(0, one, zero),
                1 => *targets = SwitchTargets::static_if(0, zero, zero),
                2 => *targets = SwitchTargets::static_if(0, zero, bb(6)),
                3 => *targets = SwitchTargets::static_if(1, zero, one),
                4 => *targets = SwitchTargets::new([(0, zero), (0, one)].into_iter(), one),
                5 => {
                    *discr = Operand::Copy(Local::from_usize(if root == 0 { 4 } else { 1 }).into())
                }
                6 => {
                    let Operand::Copy(place) = *discr else {
                        unreachable!()
                    };
                    *discr = Operand::Move(place);
                }
                _ => unreachable!(),
            }
            reject(&b, "predicate/flag substitution or drop polarity/targets");
        }
    }
    let mut b = body.clone();
    let TerminatorKind::Drop { unwind, .. } =
        &mut b.basic_blocks.as_mut()[bb(5)].terminator_mut().kind
    else {
        unreachable!()
    };
    *unwind = UnwindAction::Unreachable;
    assert_eq!(
        reviewed_body(tcx, instance, &b, &contract),
        !tcx.sess.panic_strategy().unwinds()
    );
    for cleanup in [false, true] {
        let mut b = body.clone();
        append_block(&mut b, TerminatorKind::UnwindResume, cleanup);
        reject(&b, "extra dead/cleanup block");
    }
}
