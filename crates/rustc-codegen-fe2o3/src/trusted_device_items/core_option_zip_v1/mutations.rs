use super::*;
use rustc_middle::mir::{
    AggregateKind, BasicBlockData, ConstOperand, MirPhase, Place, ProjectionElem, Promoted,
    RuntimePhase, Rvalue, SourceInfo, SourceScope, Statement, StatementKind, SwitchTargets,
    Terminator, UnwindAction,
};
use rustc_span::DUMMY_SP;

fn statement(kind: StatementKind<'_>) -> Statement<'_> {
    Statement::new(SourceInfo::outermost(DUMMY_SP), kind)
}

pub(super) fn check<'tcx>(tcx: TyCtxt<'tcx>) {
    let instance = helper(tcx, "owned");
    let c = contract(tcx, instance).unwrap();
    let source = tcx.instance_mir(instance.def);
    let accepts = |b: &Body<'tcx>| body::reviewed(tcx, instance, b, &c);
    assert!(accepts(source));
    let bb0 = BasicBlock::from_usize(0);
    let mut b = source.clone();
    let TerminatorKind::SwitchInt { targets, .. } =
        &mut b.basic_blocks_mut()[bb0].terminator_mut().kind
    else {
        panic!("actual input-variant guard")
    };
    let none = targets.target_for_value(c.discriminants[0]);
    let some = targets.target_for_value(c.discriminants[1]);
    assert_ne!(none, some);
    *targets = SwitchTargets::new(
        [(c.discriminants[0], some), (c.discriminants[1], none)].into_iter(),
        targets.otherwise(),
    );
    assert!(!accepts(&b), "swapped actual input-variant guard");
    for mutation in 0..14 {
        let mut b = source.clone();
        match mutation {
            0 => b.source.instance = helper(tcx, "foreign").def,
            1 => b.source.promoted = Some(Promoted::from_usize(0)),
            2 => b.phase = MirPhase::Runtime(RuntimePhase::Initial),
            3 => b.injection_phase = Some(b.phase),
            4 => b.is_polymorphic = false,
            5 => b.spread_arg = Some(Local::from_usize(2)),
            6 => b.arg_count = 1,
            7 => b.required_consts = None,
            8 => b.local_decls[Local::from_usize(0)].ty = c.inputs[0],
            9 => b.local_decls[Local::from_usize(1)].ty = c.inputs[1],
            10 => b.local_decls[Local::from_usize(2)].ty = c.inputs[0],
            11 => b.source_scopes.raw[0].parent_scope = Some(SourceScope::from_usize(0)),
            12 => b.source_scopes.raw[0].inlined_parent_scope = Some(SourceScope::from_usize(0)),
            13 => b.basic_blocks_mut()[bb0].terminator = None,
            _ => unreachable!(),
        }
        assert!(!accepts(&b), "header mutation {mutation}");
    }
    for (block, data) in source.basic_blocks.iter_enumerated() {
        if matches!(data.terminator().kind, TerminatorKind::SwitchInt { .. }) && !data.is_cleanup {
            let mut b = source.clone();
            b.basic_blocks_mut()[block].terminator_mut().kind =
                TerminatorKind::Goto { target: bb0 };
            assert!(!accepts(&b), "normal switch loop {block:?}");
        }
        if let TerminatorKind::Drop { target, .. } = data.terminator().kind {
            if data.is_cleanup {
                continue;
            }
            let mut b = source.clone();
            b.basic_blocks_mut()[block].terminator_mut().kind = TerminatorKind::Goto { target };
            assert!(!accepts(&b), "bypassed normal drop {block:?}");
            let mut b = source.clone();
            let TerminatorKind::Drop { place, .. } =
                &mut b.basic_blocks_mut()[block].terminator_mut().kind
            else {
                unreachable!()
            };
            let mut projection = place.projection.to_vec();
            let ProjectionElem::Field(role, _) = &mut projection[0] else {
                unreachable!()
            };
            *role = rustc_abi::FieldIdx::from_usize(1 - role.as_usize());
            *place = Place {
                local: place.local,
                projection: tcx.mk_place_elems(&projection),
            };
            assert!(!accepts(&b), "wrong dropped payload {block:?}");
            let mut b = source.clone();
            let TerminatorKind::Drop { replace, .. } =
                &mut b.basic_blocks_mut()[block].terminator_mut().kind
            else {
                unreachable!()
            };
            *replace = true;
            assert!(!accepts(&b), "replacement drop {block:?}");
            if tcx.sess.panic_strategy().unwinds() {
                let mut b = source.clone();
                let TerminatorKind::Drop { unwind, .. } =
                    &mut b.basic_blocks_mut()[block].terminator_mut().kind
                else {
                    unreachable!()
                };
                *unwind = UnwindAction::Unreachable;
                assert!(!accepts(&b), "removed unwind edge {block:?}");
            }
        }
    }
    let mut aggregates = 0;
    let mut payloads = 0;
    for (block, data) in source.basic_blocks.iter_enumerated() {
        for (index, stmt) in data.statements.iter().enumerate() {
            let StatementKind::Assign(a) = &stmt.kind else {
                continue;
            };
            if matches!(&a.1, Rvalue::Aggregate(kind, ops) if matches!(&**kind, AggregateKind::Tuple) && ops.len() == 2)
            {
                aggregates += 1;
                for duplicate in [false, true] {
                    let mut b = source.clone();
                    let StatementKind::Assign(a) =
                        &mut b.basic_blocks_mut()[block].statements[index].kind
                    else {
                        unreachable!()
                    };
                    let Rvalue::Aggregate(_, ops) = &mut a.1 else {
                        unreachable!()
                    };
                    if duplicate {
                        ops.raw[1] = ops.raw[0].clone();
                    } else {
                        ops.raw.swap(0, 1);
                    }
                    assert!(
                        !accepts(&b),
                        "tuple transfer duplicate={duplicate} {block:?}:{index}"
                    );
                }
            }
            if let Rvalue::Use(Operand::Move(p)) = &a.1
                && !p.projection.is_empty()
            {
                payloads += 1;
                let mut b = source.clone();
                let StatementKind::Assign(a) =
                    &mut b.basic_blocks_mut()[block].statements[index].kind
                else {
                    unreachable!()
                };
                a.1 = Rvalue::Use(Operand::Copy(*p));
                assert!(!accepts(&b), "copy generic payload {block:?}:{index}");
                let mut b = source.clone();
                b.basic_blocks_mut()[block]
                    .statements
                    .insert(index + 1, stmt.clone());
                assert!(!accepts(&b), "duplicate payload move {block:?}:{index}");
                let mut b = source.clone();
                let mut projection = p.projection.to_vec();
                let ProjectionElem::Downcast(_, variant) = &mut projection[1] else {
                    panic!("payload downcast")
                };
                *variant = c.none;
                let StatementKind::Assign(a) =
                    &mut b.basic_blocks_mut()[block].statements[index].kind
                else {
                    unreachable!()
                };
                a.1 = Rvalue::Use(Operand::Move(Place {
                    local: p.local,
                    projection: tcx.mk_place_elems(&projection),
                }));
                assert!(!accepts(&b), "wrong payload variant {block:?}:{index}");
            }
            if let Rvalue::Aggregate(kind, _) = &a.1
                && matches!(&**kind, AggregateKind::Adt(_, variant, ..) if *variant == c.some)
            {
                let mut b = source.clone();
                b.basic_blocks_mut()[block].statements[index].kind = StatementKind::Nop;
                assert!(!accepts(&b), "missing Some output");
            }
        }
    }
    assert_eq!(aggregates, 2);
    assert_eq!(payloads, 2);
    let (switch, discr) = source
        .basic_blocks
        .iter_enumerated()
        .find_map(|(bb, b)| {
            if let TerminatorKind::SwitchInt { discr, .. } = &b.terminator().kind {
                Some((bb, discr.clone()))
            } else {
                None
            }
        })
        .unwrap();
    for targets in [
        SwitchTargets::new([(9, bb0)].into_iter(), bb0),
        SwitchTargets::new([(0, bb0), (0, bb0)].into_iter(), bb0),
    ] {
        let mut b = source.clone();
        b.basic_blocks_mut()[switch].terminator_mut().kind = TerminatorKind::SwitchInt {
            discr: discr.clone(),
            targets,
        };
        assert!(!accepts(&b), "invalid switch table");
    }
    let mut b = source.clone();
    b.basic_blocks_mut()[switch].terminator_mut().kind = TerminatorKind::SwitchInt {
        discr: Operand::Constant(Box::new(ConstOperand {
            span: DUMMY_SP,
            user_ty: None,
            const_: Const::from_usize(tcx, 0),
        })),
        targets: SwitchTargets::new([(0, bb0)].into_iter(), bb0),
    };
    assert!(!accepts(&b), "wrong declared constant type");
    let panic_call = local_body(tcx, "panic_route")
        .basic_blocks
        .iter()
        .find(|b| matches!(b.terminator().kind, TerminatorKind::Call { .. }))
        .unwrap()
        .terminator()
        .clone();
    let mut b = source.clone();
    b.basic_blocks_mut()
        .push(BasicBlockData::new(Some(panic_call), false));
    assert!(
        !accepts(&b),
        "even an unreachable inserted call is outside the closed body"
    );
    for count in [MAX_LOCALS, MAX_LOCALS + 1] {
        let mut b = source.clone();
        b.local_decls
            .raw
            .resize(count, source.local_decls[Local::from_usize(3)].clone());
        assert_eq!(accepts(&b), count <= MAX_LOCALS, "local bound {count}");
    }
    for count in [MAX_BLOCKS, MAX_BLOCKS + 1] {
        let mut b = source.clone();
        while b.basic_blocks.len() < count {
            b.basic_blocks_mut().push(BasicBlockData::new(
                Some(Terminator {
                    source_info: SourceInfo::outermost(DUMMY_SP),
                    kind: TerminatorKind::Unreachable,
                }),
                false,
            ));
        }
        assert_eq!(accepts(&b), count <= MAX_BLOCKS, "block bound {count}");
    }
    for count in [MAX_STATEMENTS, MAX_STATEMENTS + 1] {
        let mut b = source.clone();
        let old = b
            .basic_blocks
            .iter()
            .map(|b| b.statements.len())
            .sum::<usize>();
        b.basic_blocks_mut()[bb0]
            .statements
            .extend((old..count).map(|_| statement(StatementKind::Nop)));
        assert_eq!(
            accepts(&b),
            count <= MAX_STATEMENTS,
            "statement bound {count}"
        );
    }
}
