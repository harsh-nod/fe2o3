use super::*;
use rustc_abi::{FieldIdx, VariantIdx};
use rustc_middle::mir::{
    AggregateKind, BasicBlock, BasicBlockData, CallSource, ConstOperand, Place, ProjectionElem,
    Rvalue, SourceInfo, Statement, SwitchTargets, Terminator, UnwindAction, UnwindTerminateReason,
};
use rustc_span::DUMMY_SP;

pub(super) fn statement<'tcx>(kind: StatementKind<'tcx>) -> Statement<'tcx> {
    Statement::new(SourceInfo::outermost(DUMMY_SP), kind)
}

pub(super) fn append<'tcx>(
    body: &mut Body<'tcx>,
    kind: TerminatorKind<'tcx>,
    cleanup: bool,
) -> BasicBlock {
    body.basic_blocks.as_mut().push(BasicBlockData::new(
        Some(Terminator {
            source_info: SourceInfo::outermost(DUMMY_SP),
            kind,
        }),
        cleanup,
    ))
}

fn boolean<'tcx>(tcx: TyCtxt<'tcx>, value: bool) -> Operand<'tcx> {
    Operand::Constant(Box::new(ConstOperand {
        span: DUMMY_SP,
        user_ty: None,
        const_: Const::from_bool(tcx, value),
    }))
}

pub(super) fn check(tcx: TyCtxt<'_>, low_opt: bool) {
    let instance = helper(tcx, "owned");
    let c = contract(tcx, instance).unwrap();
    let source = tcx.instance_mir(instance.def);
    check_body(tcx, instance, source, &c);
    if low_opt {
        let model = local_body(tcx, "model");
        assert!(
            !reviewed_body(tcx, instance, model, &c),
            "source instance cannot be replaced by an equivalent local body"
        );
        let mut changed = model.clone();
        changed.source.instance = instance.def;
        check_body(tcx, instance, &changed, &c);
        drop_flags(tcx, instance, &changed, &c);
    }
}

fn check_body<'tcx>(
    tcx: TyCtxt<'tcx>,
    instance: Instance<'tcx>,
    source: &Body<'tcx>,
    c: &Contract<'tcx>,
) {
    assert!(
        reviewed_body(tcx, instance, source, c),
        "closed source shape: {source:#?}"
    );
    let reject =
        |body: &Body<'tcx>, label| assert!(!reviewed_body(tcx, instance, body, c), "{label}");
    let call = source
        .basic_blocks
        .iter_enumerated()
        .find(|(_, b)| matches!(b.terminator().kind, TerminatorKind::Call { .. }))
        .unwrap()
        .0;
    let drop = source
        .basic_blocks
        .iter_enumerated()
        .find(|(_, b)| matches!(b.terminator().kind, TerminatorKind::Drop { .. }))
        .unwrap()
        .0;
    let ret = source
        .basic_blocks
        .iter_enumerated()
        .find(|(_, b)| matches!(b.terminator().kind, TerminatorKind::Return))
        .unwrap()
        .0;
    let entry = BasicBlock::from_usize(0);
    let panic_call = local_body(tcx, "panic_route")
        .basic_blocks
        .iter()
        .find(|b| matches!(b.terminator().kind, TerminatorKind::Call { .. }))
        .unwrap()
        .terminator()
        .kind
        .clone();
    for cleanup in [false, true] {
        let mut b = source.clone();
        append(&mut b, panic_call.clone(), cleanup);
        reject(&b, "dead panic call still fails whole-body proof");
        let mut b = source.clone();
        let id = append(&mut b, TerminatorKind::Return, cleanup);
        b.basic_blocks_mut()[id]
            .statements
            .push(statement(StatementKind::Nop));
        reject(&b, "unvisited executable block is not a pruning exemption");
    }
    for replacement in [
        panic_call,
        TerminatorKind::Unreachable,
        TerminatorKind::Goto { target: call },
    ] {
        let mut b = source.clone();
        b.basic_blocks_mut()[call].terminator_mut().kind = replacement;
        reject(&b, "callback cannot be replaced, removed or cycled");
    }
    for mutation in 0..13 {
        let mut b = source.clone();
        let TerminatorKind::Call {
            func,
            args,
            destination,
            target,
            unwind,
            call_source,
            ..
        } = &mut b.basic_blocks_mut()[call].terminator_mut().kind
        else {
            unreachable!()
        };
        match mutation {
            0 => args.swap(0, 1),
            1 => *args = args[..1].to_vec().into_boxed_slice(),
            2 => {
                let mut a = args.to_vec();
                a.push(a[0].clone());
                *args = a.into_boxed_slice();
            }
            3 | 4 => {
                let arg = &mut args[mutation - 3].node;
                let Operand::Move(p) = arg else {
                    unreachable!()
                };
                *arg = Operand::Copy(*p);
            }
            5 => *destination = Local::from_usize(1).into(),
            6 => *target = None,
            7 => *unwind = UnwindAction::Terminate(UnwindTerminateReason::Abi),
            8 => *unwind = UnwindAction::Terminate(UnwindTerminateReason::InCleanup),
            9 => *unwind = UnwindAction::Cleanup(entry),
            10 => *call_source = CallSource::Misc,
            11 => {
                let Operand::Constant(k) = func else {
                    unreachable!()
                };
                k.const_ = Const::Val(
                    ConstValue::ZeroSized,
                    Ty::new_fn_def(tcx, helper(tcx, "foreign").def_id(), instance.args),
                );
            }
            12 => {
                let Operand::Constant(k) = func else {
                    unreachable!()
                };
                let TyKind::FnDef(id, _) = *c.callback.kind() else {
                    unreachable!()
                };
                k.const_ = Const::Val(
                    ConstValue::ZeroSized,
                    Ty::new_fn_def(
                        tcx,
                        id,
                        tcx.mk_args(&[
                            c.parameters[3].into(),
                            Ty::new_tup(tcx, &[c.parameters[0]]).into(),
                        ]),
                    ),
                );
            }
            _ => unreachable!(),
        }
        reject(
            &b,
            "exact callback, two moved source arguments, result and unwind",
        );
    }
    for mutation in 0..5 {
        let mut b = source.clone();
        let TerminatorKind::Drop {
            place,
            replace,
            drop: coroutine_drop,
            async_fut,
            target,
            ..
        } = &mut b.basic_blocks_mut()[drop].terminator_mut().kind
        else {
            unreachable!()
        };
        match mutation {
            0 => *place = Local::from_usize(0).into(),
            1 => *replace = true,
            2 => *coroutine_drop = Some(entry),
            3 => *async_fut = Some(Local::from_usize(2)),
            4 => {
                let target = *target;
                b.basic_blocks_mut()[drop].terminator_mut().kind = TerminatorKind::Goto { target };
            }
            _ => unreachable!(),
        }
        reject(
            &b,
            "callback must be dropped exactly once on Ok, not its output or an async replacement",
        );
    }
    for (block, data) in source.basic_blocks.iter_enumerated() {
        for (index, s) in data.statements.iter().enumerate() {
            if let StatementKind::StorageLive(local) = s.kind {
                let mut b = source.clone();
                b.basic_blocks_mut()[block]
                    .statements
                    .insert(index + 1, statement(StatementKind::StorageLive(local)));
                reject(&b, "StorageLive cannot reset a live slot");
            }
            let StatementKind::Assign(a) = &s.kind else {
                continue;
            };
            if let Rvalue::Use(Operand::Move(place)) = &a.1 {
                if !place.projection.is_empty() {
                    for mutation in 0..4 {
                        let mut b = source.clone();
                        let StatementKind::Assign(a) =
                            &mut b.basic_blocks_mut()[block].statements[index].kind
                        else {
                            unreachable!()
                        };
                        let Rvalue::Use(Operand::Move(p)) = &mut a.1 else {
                            unreachable!()
                        };
                        let [
                            ProjectionElem::Downcast(name, variant),
                            ProjectionElem::Field(field, ty),
                        ] = p.projection.as_slice()
                        else {
                            unreachable!()
                        };
                        p.projection = tcx.mk_place_elems(&[
                            ProjectionElem::Downcast(
                                *name,
                                if mutation == 0 {
                                    VariantIdx::from_usize(1 - variant.as_usize())
                                } else {
                                    *variant
                                },
                            ),
                            ProjectionElem::Field(
                                if mutation == 1 {
                                    FieldIdx::from_usize(1)
                                } else {
                                    *field
                                },
                                if mutation == 2 { c.parameters[2] } else { *ty },
                            ),
                        ]);
                        if mutation == 3 {
                            a.1 = Rvalue::Use(Operand::Copy(*p));
                        }
                        reject(
                            &b,
                            "nominal input variant, field, generic role and move are exact",
                        );
                    }
                    let mut b = source.clone();
                    b.basic_blocks_mut()[block]
                        .statements
                        .insert(index + 1, s.clone());
                    reject(&b, "cannot move a payload twice");
                    let mut b = source.clone();
                    let local = a.0.local;
                    b.basic_blocks_mut()[block]
                        .statements
                        .insert(index + 1, statement(StatementKind::StorageDead(local)));
                    reject(&b, "live payload cannot be killed before transfer");
                    let mut b = source.clone();
                    b.basic_blocks_mut()[block].statements.insert(
                        index + 1,
                        statement(StatementKind::Assign(Box::new((
                            a.0,
                            Rvalue::Use(Operand::Copy(a.0)),
                        )))),
                    );
                    reject(&b, "no stale payload copy aliases");
                }
            }
            if let Rvalue::Aggregate(kind, operands) = &a.1 {
                for mutation in 0..4 {
                    let mut b = source.clone();
                    let StatementKind::Assign(a) =
                        &mut b.basic_blocks_mut()[block].statements[index].kind
                    else {
                        unreachable!()
                    };
                    let Rvalue::Aggregate(changed_kind, changed_operands) = &mut a.1 else {
                        unreachable!()
                    };
                    match mutation {
                        0 => changed_operands.raw.clear(),
                        1 => changed_operands.raw.push(operands.raw[0].clone()),
                        2 => changed_operands.raw[0] = Operand::Move(Local::from_usize(2).into()),
                        3 => match &**kind {
                            AggregateKind::Tuple => {
                                *changed_kind = Box::new(AggregateKind::Adt(
                                    c.result,
                                    c.variants[1],
                                    tcx.mk_args(&[c.parameters[0].into(), c.parameters[2].into()]),
                                    None,
                                    None,
                                ))
                            }
                            AggregateKind::Adt(id, v, args, _, _) => {
                                *changed_kind = Box::new(AggregateKind::Adt(
                                    *id,
                                    VariantIdx::from_usize(1 - v.as_usize()),
                                    *args,
                                    None,
                                    None,
                                ))
                            }
                            _ => unreachable!(),
                        },
                        _ => unreachable!(),
                    }
                    reject(&b, "tuple/result shape and payload provenance are exact");
                }
                let mut b = source.clone();
                b.basic_blocks_mut()[block]
                    .statements
                    .insert(index + 1, s.clone());
                reject(&b, "last-use Copy cannot transfer ownership twice");
            }
        }
    }
    let mut b = source.clone();
    b.basic_blocks_mut()[ret]
        .statements
        .push(statement(StatementKind::StorageDead(Local::from_usize(0))));
    reject(&b, "return ownership must remain live");
    let TerminatorKind::SwitchInt { discr, targets } =
        &source.basic_blocks[entry].terminator().kind
    else {
        unreachable!()
    };
    for targets in [
        SwitchTargets::new(
            [
                (
                    c.discriminants[0],
                    targets.target_for_value(c.discriminants[1]),
                ),
                (
                    c.discriminants[1],
                    targets.target_for_value(c.discriminants[0]),
                ),
            ]
            .into_iter(),
            targets.otherwise(),
        ),
        SwitchTargets::new([(0, entry), (0, entry)].into_iter(), targets.otherwise()),
        SwitchTargets::static_if(
            0,
            BasicBlock::from_usize(source.basic_blocks.len()),
            targets.otherwise(),
        ),
    ] {
        let mut b = source.clone();
        b.basic_blocks_mut()[entry].terminator_mut().kind = TerminatorKind::SwitchInt {
            discr: discr.clone(),
            targets,
        };
        reject(
            &b,
            "branch role, duplicate discriminants and dangling edges",
        );
    }
    let mut b = source.clone();
    b.basic_blocks_mut()[entry].terminator_mut().kind = TerminatorKind::Goto { target: entry };
    reject(&b, "CFG cycles never become a work-limit exemption");
    // A different closure instance cannot reuse the retained call proof.
    let other = helper(tcx, "panic_callback");
    assert!(!reviewed_body(tcx, other, source, c));
}

fn drop_flags<'tcx>(
    tcx: TyCtxt<'tcx>,
    instance: Instance<'tcx>,
    source: &Body<'tcx>,
    c: &Contract<'tcx>,
) {
    let (block, discr) = source
        .basic_blocks
        .iter_enumerated()
        .find_map(|(id, b)| match &b.terminator().kind {
            TerminatorKind::SwitchInt {
                discr: Operand::Copy(p),
                ..
            } if source.local_decls[p.local].ty == tcx.types.bool => Some((id, *p)),
            _ => None,
        })
        .expect("low-opt generic drop flag");
    for value in [false, true] {
        let mut b = source.clone();
        b.basic_blocks_mut()[block]
            .statements
            .push(statement(StatementKind::Assign(Box::new((
                discr,
                Rvalue::Use(boolean(tcx, value)),
            )))));
        assert!(
            !reviewed_body(tcx, instance, &b, c),
            "reassigned drop condition cannot cover both variants"
        );
    }
    let mut b = source.clone();
    let alias = b.local_decls.push(source.local_decls[discr.local].clone());
    b.basic_blocks_mut()[block].statements.extend([
        statement(StatementKind::Assign(Box::new((
            alias.into(),
            Rvalue::Use(Operand::Copy(discr)),
        )))),
        statement(StatementKind::Assign(Box::new((
            discr,
            Rvalue::Use(boolean(tcx, false)),
        )))),
    ]);
    let TerminatorKind::SwitchInt { discr, .. } =
        &mut b.basic_blocks_mut()[block].terminator_mut().kind
    else {
        unreachable!()
    };
    *discr = Operand::Copy(Place::from(alias));
    assert!(
        reviewed_body(tcx, instance, &b, c),
        "scalar alias retains its captured condition, not the reassigned flag"
    );
}
