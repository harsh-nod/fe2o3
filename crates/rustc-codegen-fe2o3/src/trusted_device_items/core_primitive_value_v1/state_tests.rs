use super::*;
use rustc_middle::mir::{BasicBlockData, SourceScope, Terminator};

fn assign<'tcx>(local: Local, value: Rvalue<'tcx>) -> Statement<'tcx> {
    Statement::new(
        SourceInfo::outermost(DUMMY_SP),
        StatementKind::Assign(Box::new((local.into(), value))),
    )
}

fn set_bool<'tcx>(tcx: TyCtxt<'tcx>, local: Local, value: bool) -> Statement<'tcx> {
    assign(local, Rvalue::Use(bool_operand(tcx, value)))
}

fn storage<'tcx>(local: Local, live: bool) -> Statement<'tcx> {
    Statement::new(
        SourceInfo::outermost(DUMMY_SP),
        if live {
            StatementKind::StorageLive(local)
        } else {
            StatementKind::StorageDead(local)
        },
    )
}

pub(super) fn resource_limits<'tcx>(tcx: TyCtxt<'tcx>, base: &Body<'tcx>, helper: Helper<'tcx>) {
    let entry = BasicBlock::from_usize(0);
    assert!(reviewed_body(tcx, base, helper));
    for count in [95, 96, 97] {
        let mut changed = base.clone();
        let statement = changed.basic_blocks[entry].statements[0].clone();
        changed.basic_blocks_mut()[entry].statements = vec![statement; count];
        assert_eq!(
            reviewed_body(tcx, &changed, helper),
            count <= MAX_STATEMENTS,
            "{helper:?}: {count} statements"
        );
    }
    assert_eq!(MAX_STATEMENTS, 96);

    for count in [7, 8, 9] {
        let mut changed = base.clone();
        let mut scope = changed.source_scopes[SourceScope::from_usize(0)].clone();
        scope.parent_scope = Some(SourceScope::from_usize(0));
        while changed.source_scopes.len() < count {
            changed.source_scopes.push(scope.clone());
        }
        assert_eq!(
            reviewed_body(tcx, &changed, helper),
            count <= MAX_SCOPES,
            "{helper:?}: {count} scopes"
        );
    }
    assert_eq!(MAX_SCOPES, 8);

    // Eight reconverging binary switches visit 255 switch states and 256
    // return states. Each state executes one statement: 2*(255+256) = 1022.
    // Root-only padding changes work by exactly one, without other limits.
    for padding in [1, 2, 3] {
        let mut changed = base.clone();
        let mut declaration = changed.local_decls[Local::from_usize(0)].clone();
        declaration.ty = tcx.types.bool;
        let condition = changed.local_decls.push(declaration);
        let final_block = changed.basic_blocks[entry].clone();
        changed.basic_blocks_mut().raw.clear();
        for index in 0..8 {
            let target = BasicBlock::from_usize(index + 1);
            let count = 1 + if index == 0 { padding } else { 0 };
            changed.basic_blocks_mut().push(BasicBlockData::new_stmts(
                vec![set_bool(tcx, condition, true); count],
                Some(Terminator {
                    source_info: SourceInfo::outermost(DUMMY_SP),
                    kind: TerminatorKind::SwitchInt {
                        discr: Operand::Move(condition.into()),
                        targets: SwitchTargets::new([(0, target)].into_iter(), target),
                    },
                }),
                false,
            ));
        }
        changed.basic_blocks_mut().push(final_block);
        let work = 2 * ((1 << 8) - 1 + (1 << 8)) + padding;
        assert_eq!(changed.basic_blocks.len(), 9);
        assert_eq!(changed.local_decls.len(), 3);
        assert!(changed.source_scopes.len() < MAX_SCOPES);
        assert!(
            changed
                .basic_blocks
                .iter()
                .map(|block| block.statements.len())
                .sum::<usize>()
                < MAX_STATEMENTS
        );
        assert_eq!(
            reviewed_body(tcx, &changed, helper),
            work <= MAX_WORK,
            "{helper:?}: {work} work units"
        );
    }
    assert_eq!(MAX_WORK, 1024);
}

pub(super) fn state_transfers<'tcx>(tcx: TyCtxt<'tcx>, source: &Body<'tcx>, helper: Helper<'tcx>) {
    assert!(reviewed_body(tcx, source, helper));
    let mut conditions = 0;
    for (block, data) in source.basic_blocks.iter_enumerated() {
        let TerminatorKind::SwitchInt {
            discr: Operand::Move(place),
            ..
        } = &data.terminator().kind
        else {
            continue;
        };
        let condition = place.local;
        if !data.statements.iter().any(|statement| {
            matches!(&statement.kind, StatementKind::Assign(assignment)
                if assignment.0 == *place && matches!(assignment.1, Rvalue::BinaryOp(BinOp::Le, _)))
        }) {
            continue;
        }
        conditions += 1;
        for value in [false, true] {
            let mut changed = source.clone();
            changed.basic_blocks_mut()[block]
                .statements
                .push(set_bool(tcx, condition, value));
            assert_eq!(
                reviewed_body(tcx, &changed, helper),
                value,
                "reassigned condition"
            );
        }

        let mut template = source.clone();
        let alias = template
            .local_decls
            .push(source.local_decls[condition].clone());
        let copy = || assign(alias, Rvalue::Use(Operand::Copy(condition.into())));
        let moved = || assign(alias, Rvalue::Use(Operand::Move(condition.into())));
        let cases = [
            (
                "copied snapshot survives source overwrite",
                vec![copy(), set_bool(tcx, condition, false)],
                alias,
                true,
            ),
            (
                "false snapshot is not repaired by source overwrite",
                vec![
                    set_bool(tcx, condition, false),
                    copy(),
                    set_bool(tcx, condition, true),
                ],
                alias,
                false,
            ),
            ("moved snapshot is initialized", vec![moved()], alias, true),
            (
                "original is unavailable after move",
                vec![moved()],
                condition,
                false,
            ),
            (
                "copy after move is unavailable",
                vec![moved(), copy()],
                alias,
                false,
            ),
            (
                "explicit assignment reinitializes moved local",
                vec![moved(), set_bool(tcx, condition, true)],
                condition,
                true,
            ),
            (
                "storage dead removes value",
                vec![storage(condition, false)],
                condition,
                false,
            ),
            (
                "storage live does not restore old value",
                vec![storage(condition, false), storage(condition, true)],
                condition,
                false,
            ),
            (
                "new lifetime can be initialized",
                vec![
                    storage(condition, false),
                    storage(condition, true),
                    set_bool(tcx, condition, true),
                ],
                condition,
                true,
            ),
            (
                "duplicate storage live",
                vec![storage(condition, true)],
                condition,
                false,
            ),
            (
                "duplicate storage dead",
                vec![storage(condition, false), storage(condition, false)],
                condition,
                false,
            ),
            (
                "copied snapshot survives source lifetime reset",
                vec![copy(), storage(condition, false), storage(condition, true)],
                alias,
                true,
            ),
            (
                "alias lifetime reset discards snapshot",
                vec![
                    storage(alias, true),
                    copy(),
                    storage(alias, false),
                    storage(alias, true),
                ],
                alias,
                false,
            ),
            (
                "alias can be initialized in its new lifetime",
                vec![
                    storage(alias, true),
                    copy(),
                    storage(alias, false),
                    storage(alias, true),
                    copy(),
                ],
                alias,
                true,
            ),
        ];
        for (label, statements, tested, accepted) in cases {
            let mut changed = template.clone();
            let data = &mut changed.basic_blocks_mut()[block];
            data.statements.extend(statements);
            let TerminatorKind::SwitchInt { discr, .. } = &mut data.terminator_mut().kind else {
                unreachable!()
            };
            *discr = Operand::Move(tested.into());
            assert_eq!(
                reviewed_body(tcx, &changed, helper),
                accepted,
                "{helper:?} {block:?}: {label}"
            );
        }
        let mut changed = source.clone();
        let statements = &mut changed.basic_blocks_mut()[block].statements;
        let live = statements.iter().position(|statement| {
            matches!(statement.kind, StatementKind::StorageLive(local) if local == condition)
        }).unwrap();
        let statement = statements.remove(live);
        assert!(
            !reviewed_body(tcx, &changed, helper),
            "assignment before storage live"
        );
        changed.basic_blocks_mut()[block].statements.push(statement);
        assert!(
            !reviewed_body(tcx, &changed, helper),
            "late storage live cannot repair assignment"
        );
    }
    assert_eq!(
        conditions, 2,
        "both retained constant preconditions are mutated"
    );
}

pub(super) fn primitive_equality<'tcx>(
    tcx: TyCtxt<'tcx>,
    called: &impl Fn(&str) -> Instance<'tcx>,
    body: &impl Fn(&str) -> &'tcx Body<'tcx>,
) {
    let entry = BasicBlock::from_usize(0);
    for name in EQ.iter().copied().chain(["wrong_eq"]) {
        let instance = called(name);
        let element = primitive_eq_identity(tcx, instance).expect(name);
        let actual = tcx.instance_mir(instance.def);
        assert!(
            authenticate_reviewed_safe_core_primitive_value_helper_v1(tcx, instance),
            "actual primitive {name}: {actual:#?}"
        );
        assert!(
            helper_identity(tcx, instance).is_none(),
            "Eq never enters the legacy/cast helper model"
        );
        assert!(
            prove_core_primitive_cast_v1(tcx, instance).is_none(),
            "Eq never gets a cast expansion"
        );
        let signature = tcx.instantiate_bound_regions_with_erased(
            tcx.fn_sig(instance.def_id())
                .instantiate(tcx, instance.args),
        );
        assert!(primitive_eq_signature(tcx, element, signature));
        for signature in [
            FnSig {
                safety: Safety::Unsafe,
                ..signature
            },
            FnSig {
                abi: ExternAbi::C { unwind: false },
                ..signature
            },
            FnSig {
                c_variadic: true,
                ..signature
            },
            FnSig {
                inputs_and_output: tcx.mk_type_list(&[element, element, tcx.types.bool]),
                ..signature
            },
            FnSig {
                inputs_and_output: tcx.mk_type_list(&[
                    signature.inputs()[0],
                    signature.inputs()[1],
                    tcx.types.u64,
                ]),
                ..signature
            },
        ] {
            assert!(
                !primitive_eq_signature(tcx, element, signature),
                "{name}: safe typed signature"
            );
        }
        assert!(
            primitive_eq_identity(
                tcx,
                Instance::new_raw(instance.def_id(), tcx.mk_args(&[element.into()]))
            )
            .is_none()
        );
        assert!(
            primitive_eq_identity(
                tcx,
                Instance {
                    def: InstanceKind::Intrinsic(instance.def_id()),
                    args: instance.args
                }
            )
            .is_none()
        );
        let reject = |b: &Body<'tcx>, label| {
            assert!(
                !reviewed_primitive_eq_body(tcx, instance, b, element),
                "{name}: {label}"
            )
        };
        for mutation in 0..18 {
            let mut b = actual.clone();
            match mutation {
                0 => b.arg_count = 1,
                1 => b.local_decls[Local::from_usize(0)].ty = tcx.types.u64,
                2 => {
                    b.local_decls[Local::from_usize(1)].ty =
                        Ty::new_mut_ref(tcx, tcx.lifetimes.re_erased, element)
                }
                3 => {
                    b.local_decls
                        .push(actual.local_decls[Local::from_usize(0)].clone());
                }
                4 => b.source.instance = called("ne_u64").def,
                5 => b.mentioned_items = None,
                6 => {
                    b.mentioned_items = Some(vec![rustc_span::Spanned {
                        span: DUMMY_SP,
                        node: rustc_middle::mir::MentionedItem::Drop(element),
                    }])
                }
                7 => b.required_consts = None,
                8 => {
                    b.required_consts = Some(vec![ConstOperand {
                        span: DUMMY_SP,
                        user_ty: None,
                        const_: Const::from_bool(tcx, true),
                    }])
                }
                9 => {
                    b.source_scopes[SourceScope::from_usize(0)].parent_scope =
                        Some(SourceScope::from_usize(0))
                }
                10 => {
                    b.source_scopes[SourceScope::from_usize(0)].inlined =
                        Some((called("ne_u64"), DUMMY_SP))
                }
                11 => b.basic_blocks_mut()[entry].is_cleanup = true,
                12 => b.basic_blocks_mut()[entry].terminator = None,
                13 => {
                    b.basic_blocks_mut()[entry].terminator_mut().kind = TerminatorKind::Unreachable
                }
                14 => {
                    b.basic_blocks_mut()[entry].terminator_mut().kind =
                        TerminatorKind::Goto { target: entry }
                }
                15 => {
                    b.basic_blocks_mut()[entry].terminator_mut().kind = TerminatorKind::Drop {
                        place: Local::from_usize(3).into(),
                        target: entry,
                        unwind: UnwindAction::Unreachable,
                        replace: false,
                        drop: None,
                        async_fut: None,
                    }
                }
                16 => {
                    b.basic_blocks_mut()[entry].terminator_mut().kind = body("eq_u64")
                        .basic_blocks
                        .iter()
                        .find(|b| matches!(b.terminator().kind, TerminatorKind::Call { .. }))
                        .unwrap()
                        .terminator()
                        .kind
                        .clone()
                }
                17 => b.basic_blocks_mut()[entry].statements.push(Statement::new(
                    SourceInfo::outermost(DUMMY_SP),
                    StatementKind::Nop,
                )),
                _ => unreachable!(),
            }
            reject(&b, "identity/header/types/effect/target/metadata/budget");
        }
        for cleanup in [false, true] {
            let mut b = actual.clone();
            b.basic_blocks_mut().push(BasicBlockData::new(
                Some(Terminator {
                    source_info: SourceInfo::outermost(DUMMY_SP),
                    kind: TerminatorKind::Unreachable,
                }),
                cleanup,
            ));
            reject(&b, "whole-body dead/cleanup coverage");
        }
        let mut compact = actual.clone();
        compact.basic_blocks_mut()[entry]
            .statements
            .retain(|s| matches!(s.kind, StatementKind::Assign(_)));
        assert!(
            reviewed_primitive_eq_body(tcx, instance, &compact, element),
            "no-storage form"
        );
        for copies in 0..4 {
            let mut b = compact.clone();
            let StatementKind::Assign(a) = &mut b.basic_blocks_mut()[entry].statements[2].kind
            else {
                unreachable!()
            };
            let Rvalue::BinaryOp(_, operands) = &mut a.1 else {
                unreachable!()
            };
            operands.0 = if copies & 1 == 0 {
                Operand::Move(Local::from_usize(3).into())
            } else {
                local_operand(3)
            };
            operands.1 = if copies & 2 == 0 {
                Operand::Move(Local::from_usize(4).into())
            } else {
                local_operand(4)
            };
            assert!(
                reviewed_primitive_eq_body(tcx, instance, &b, element),
                "primitive local copies or moves"
            );
        }
        for mutation in 0..13 {
            let mut b = compact.clone();
            let statements = &mut b.basic_blocks_mut()[entry].statements;
            match mutation {
                0 | 1 | 2 => {
                    let StatementKind::Assign(a) = &mut statements[0].kind else {
                        unreachable!()
                    };
                    let Rvalue::Use(Operand::Copy(place)) = a.1 else {
                        unreachable!()
                    };
                    a.1 = match mutation {
                        0 => Rvalue::Use(Operand::Move(place)),
                        1 => Rvalue::Use(Operand::Copy(rustc_middle::mir::Place {
                            local: Local::from_usize(2),
                            ..place
                        })),
                        _ => Rvalue::Use(local_operand(4)),
                    };
                }
                3 => statements.swap(0, 1),
                4 => {
                    let StatementKind::Assign(a) = &mut statements[1].kind else {
                        unreachable!()
                    };
                    a.0 = Local::from_usize(3).into();
                }
                5 | 6 | 7 | 8 => {
                    let StatementKind::Assign(a) = &mut statements[2].kind else {
                        unreachable!()
                    };
                    let Rvalue::BinaryOp(op, operands) = &mut a.1 else {
                        unreachable!()
                    };
                    match mutation {
                        5 => *op = BinOp::Ne,
                        6 => std::mem::swap(&mut operands.0, &mut operands.1),
                        7 => operands.1 = operands.0.clone(),
                        _ => operands.0 = local_operand(0),
                    }
                }
                9 => {
                    let StatementKind::Assign(a) = &mut statements[2].kind else {
                        unreachable!()
                    };
                    a.1 = Rvalue::Use(bool_operand(tcx, true));
                }
                10 => {
                    let StatementKind::Assign(a) = &mut statements[2].kind else {
                        unreachable!()
                    };
                    a.0 = Local::from_usize(3).into();
                }
                11 => {
                    let StatementKind::Assign(a) = &mut statements[2].kind else {
                        unreachable!()
                    };
                    a.1 = Rvalue::Cast(CastKind::IntToInt, local_operand(3), tcx.types.bool);
                }
                12 => {
                    b.local_decls[Local::from_usize(3)].ty = if element == tcx.types.u8 {
                        tcx.types.u16
                    } else {
                        tcx.types.u8
                    }
                }
                _ => unreachable!(),
            }
            reject(
                &b,
                "loads, ordered provenance, initialization, compare opcode, cast or result",
            );
        }
        let mut scoped = compact.clone();
        let statements = &compact.basic_blocks[entry].statements;
        scoped.basic_blocks_mut()[entry].statements = vec![
            storage(Local::from_usize(3), true),
            statements[0].clone(),
            storage(Local::from_usize(4), true),
            statements[1].clone(),
            statements[2].clone(),
            storage(Local::from_usize(4), false),
            storage(Local::from_usize(3), false),
        ];
        assert!(reviewed_primitive_eq_body(tcx, instance, &scoped, element));
        for (first, second) in [(0, 1), (2, 3), (4, 5), (5, 6)] {
            let mut b = scoped.clone();
            b.basic_blocks_mut()[entry].statements.swap(first, second);
            reject(&b, "storage liveness and killed/moved use");
        }
    }
    for name in [
        "ne_u64",
        "wrong_ref_eq",
        "wrong_unit_eq",
        "wrong_option_eq",
        "foreign_eq",
        "from_u32",
        "from_u8",
    ] {
        assert!(
            primitive_eq_identity(tcx, called(name)).is_none(),
            "Eq nominal negative: {name}"
        );
    }
    for name in [
        "wrong_ref_eq",
        "wrong_unit_eq",
        "wrong_option_eq",
        "foreign_eq",
    ] {
        assert!(
            !authenticate_reviewed_safe_core_primitive_value_helper_v1(tcx, called(name)),
            "no transferred source authentication: {name}"
        );
    }
}
