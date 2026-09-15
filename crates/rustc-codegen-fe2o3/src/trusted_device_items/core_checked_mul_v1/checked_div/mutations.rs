use super::*;

pub(super) fn arithmetic<'tcx>(tcx: TyCtxt<'tcx>, instance: Instance<'tcx>, body: &Body<'tcx>) {
    let word = identity(tcx, instance).unwrap();
    let TyKind::Uint(integer) = word.kind() else {
        panic!("unsigned word")
    };
    let bits = integer
        .bit_width()
        .unwrap_or(tcx.data_layout.pointer_size().bits());
    let eq = binary_site(body, BinOp::Eq);
    let div = binary_site(body, BinOp::Div);
    for operation in [
        BinOp::Rem,
        BinOp::Mul,
        BinOp::AddUnchecked,
        BinOp::Sub,
        BinOp::Eq,
    ] {
        let mut changed = body.clone();
        *binary_mut(&mut changed, div).0 = operation;
        assert!(
            !accepts(tcx, instance, &changed),
            "wrong quotient operation {operation:?}"
        );
    }
    for operation in [BinOp::Ne, BinOp::Lt, BinOp::Le] {
        let mut changed = body.clone();
        *binary_mut(&mut changed, eq).0 = operation;
        assert!(
            !accepts(tcx, instance, &changed),
            "wrong zero predicate {operation:?}"
        );
    }
    for rhs in [
        scalar(tcx, word, bits, 1),
        scalar(
            tcx,
            tcx.types.isize,
            tcx.data_layout.pointer_size().bits(),
            0,
        ),
        scalar(tcx, tcx.types.bool, 8, 0),
        scalar(tcx, word, if bits == 8 { 16 } else { 8 }, 0),
    ] {
        let mut changed = body.clone();
        binary_mut(&mut changed, eq).1.1 = rhs;
        assert!(
            !accepts(tcx, instance, &changed),
            "nonzero, signed, bool or wrong-size constant"
        );
    }
    let mut changed = body.clone();
    binary_mut(&mut changed, eq).1.0 = Operand::Copy(Local::from_usize(1).into());
    assert!(
        !accepts(tcx, instance, &changed),
        "testing dividend does not establish nonzero divisor"
    );
    for rhs in [
        scalar(tcx, word, bits, 0),
        Operand::Copy(Local::from_usize(1).into()),
    ] {
        let mut changed = body.clone();
        binary_mut(&mut changed, div).1.1 = rhs;
        assert!(
            !accepts(tcx, instance, &changed),
            "division must use the tested divisor"
        );
    }
    let mut changed = body.clone();
    binary_mut(&mut changed, div).1.0 = Operand::Copy(Local::from_usize(2).into());
    assert!(
        !accepts(tcx, instance, &changed),
        "quotient must use original dividend"
    );
    let mut changed = body.clone();
    let operands = binary_mut(&mut changed, div).1;
    std::mem::swap(&mut operands.0, &mut operands.1);
    assert!(!accepts(tcx, instance, &changed), "reversed division");

    // This includes u64 <-> usize on AMDGPU: same bits do not establish the
    // exact nominal type or the Option payload binding.
    for other in [
        tcx.types.u8,
        tcx.types.u16,
        tcx.types.u32,
        tcx.types.u64,
        tcx.types.u128,
        tcx.types.usize,
        tcx.types.isize,
    ] {
        if other == word {
            continue;
        }
        for local in [Local::from_usize(1), Local::from_usize(2)] {
            let mut changed = body.clone();
            changed.local_decls[local].ty = other;
            assert!(
                !accepts(tcx, instance, &changed),
                "changed argument type {other:?}"
            );
        }
        let mut changed = body.clone();
        binary_mut(&mut changed, eq).1.1 = scalar(tcx, other, bits, 0);
        assert!(
            !accepts(tcx, instance, &changed),
            "zero constant of another nominal type"
        );
    }
    let mut changed = body.clone();
    changed.local_decls[RETURN_PLACE].ty = Ty::new_tup(tcx, &[word, tcx.types.bool]);
    assert!(!accepts(tcx, instance, &changed), "non-Option return");
    let mut touched = false;
    for (block, data) in body.basic_blocks.iter_enumerated() {
        for (index, statement) in data.statements.iter().enumerate() {
            if let StatementKind::Assign(assignment) = &statement.kind
                && let Rvalue::Aggregate(kind, operands) = &assignment.1
                && matches!(&**kind, AggregateKind::Adt(_, _, _, _, _))
                && operands.len() == 1
            {
                let mut changed = body.clone();
                let StatementKind::Assign(assignment) =
                    &mut changed.basic_blocks_mut()[block].statements[index].kind
                else {
                    unreachable!()
                };
                let Rvalue::Aggregate(_, operands) = &mut assignment.1 else {
                    unreachable!()
                };
                operands.raw[0] = Operand::Copy(Local::from_usize(1).into());
                assert!(
                    !accepts(tcx, instance, &changed),
                    "Some must carry the exact quotient"
                );
                touched = true;
            }
        }
    }
    assert!(touched, "mutated actual Some payload");
}

pub(super) fn flow<'tcx>(tcx: TyCtxt<'tcx>, instance: Instance<'tcx>, body: &Body<'tcx>) {
    let word = identity(tcx, instance).unwrap();
    let div = binary_site(body, BinOp::Div);
    let switch = body
        .basic_blocks
        .iter_enumerated()
        .find_map(|(block, data)| {
            matches!(data.terminator().kind, TerminatorKind::SwitchInt { .. }).then_some(block)
        })
        .expect("zero-divisor branch");
    let TerminatorKind::SwitchInt { discr, targets } = &body.basic_blocks[switch].terminator().kind
    else {
        unreachable!()
    };
    let direct = discr.ty(&body.local_decls, tcx) == word;
    let nonzero = targets.target_for_value(u128::from(direct));
    let zero = targets.target_for_value(u128::from(!direct));
    assert_ne!(zero, nonzero);
    for target in [zero, nonzero] {
        let mut changed = body.clone();
        changed.basic_blocks_mut()[switch].terminator_mut().kind = TerminatorKind::Goto { target };
        assert!(
            !accepts(tcx, instance, &changed),
            "no actual zero/nonzero edge"
        );
    }
    let mut changed = body.clone();
    if let TerminatorKind::SwitchInt { targets, .. } =
        &mut changed.basic_blocks_mut()[switch].terminator_mut().kind
    {
        *targets = if direct {
            SwitchTargets::new([(0, nonzero)].into_iter(), zero)
        } else {
            SwitchTargets::new([(0, zero)].into_iter(), nonzero)
        };
    }
    assert!(
        !accepts(tcx, instance, &changed),
        "reversed zero/nonzero edge"
    );
    let mut changed = body.clone();
    if let TerminatorKind::SwitchInt { targets, .. } =
        &mut changed.basic_blocks_mut()[switch].terminator_mut().kind
    {
        *targets = SwitchTargets::new([(2, zero)].into_iter(), nonzero);
    }
    assert!(
        !accepts(tcx, instance, &changed),
        "nonzero switch value cannot stand for zero"
    );
    for value in [0, 1] {
        let mut changed = body.clone();
        if let TerminatorKind::SwitchInt { discr, .. } =
            &mut changed.basic_blocks_mut()[switch].terminator_mut().kind
        {
            *discr = scalar(tcx, tcx.types.bool, 8, value);
        }
        assert!(
            !accepts(tcx, instance, &changed),
            "constant branch has no divisor authority"
        );
    }
    // Neither a discarded quotient before the guard nor one on the None
    // path may hide behind the expected final return value.
    for block in [BasicBlock::from_usize(0), zero] {
        let mut changed = body.clone();
        changed.basic_blocks_mut()[block]
            .statements
            .insert(0, body.basic_blocks[div.0].statements[div.1].clone());
        assert!(
            !accepts(tcx, instance, &changed),
            "division before proof or on zero path"
        );
    }
    let mut changed = body.clone();
    changed.basic_blocks_mut()[zero].terminator_mut().kind = TerminatorKind::Unreachable;
    assert!(
        !accepts(tcx, instance, &changed),
        "zero divisor cannot abort instead of None"
    );
    let mut changed = body.clone();
    changed.basic_blocks_mut()[zero].terminator_mut().kind = TerminatorKind::UnwindResume;
    assert!(
        !accepts(tcx, instance, &changed),
        "zero divisor cannot unwind instead of None"
    );
    let mut changed = body.clone();
    changed.basic_blocks_mut()[zero].terminator_mut().kind =
        TerminatorKind::Goto { target: switch };
    assert!(
        !accepts(tcx, instance, &changed),
        "cycles are outside the finite proof"
    );

    // Find or add a retained real hint edge so host inlining cannot make
    // the foreign/panicking-call regression vacuous.
    let mut retained = body.clone();
    let call_block = retained
        .basic_blocks
        .iter_enumerated()
        .find_map(|(block, data)| {
            let TerminatorKind::Call { func, .. } = &data.terminator().kind else {
                return None;
            };
            closed_hint(tcx, resolve_callee(tcx, func)?).then_some(block)
        });
    if let Some(call_block) = call_block {
        for name in ["fake_hint", "panicking_hint"] {
            let mut changed = retained.clone();
            if let TerminatorKind::Call { func, .. } =
                &mut changed.basic_blocks_mut()[call_block].terminator_mut().kind
            {
                *func = called_operand(tcx, name);
            }
            assert!(
                !accepts(tcx, instance, &changed),
                "foreign or panicking hint {name}"
            );
        }
        let mut changed = retained.clone();
        if let TerminatorKind::Call { unwind, .. } =
            &mut changed.basic_blocks_mut()[call_block].terminator_mut().kind
        {
            *unwind = UnwindAction::Cleanup(zero);
        }
        assert!(!accepts(tcx, instance, &changed), "no added unwind path");
        let mut changed = retained.clone();
        if let TerminatorKind::Call { args, .. } =
            &mut changed.basic_blocks_mut()[call_block].terminator_mut().kind
        {
            args[0].node = scalar(tcx, word, tcx.data_layout.pointer_size().bits(), 0);
        }
        assert!(
            !accepts(tcx, instance, &changed),
            "word-typed hint argument is not bool"
        );
    } else {
        // Splice the compiler-produced call terminator, retaining its call
        // source metadata while rebinding only its typed input/output/edge.
        let mut term = tcx
            .instance_mir(local(tcx, "real_hint").def)
            .basic_blocks
            .iter()
            .find_map(|data| {
                matches!(data.terminator().kind, TerminatorKind::Call { .. })
                    .then(|| data.terminator().clone())
            })
            .expect("retained hint fixture");
        let TerminatorKind::SwitchInt { discr, .. } =
            &retained.basic_blocks[switch].terminator().kind
        else {
            unreachable!()
        };
        let predicate = if direct {
            let eq = binary_site(&retained, BinOp::Eq);
            let StatementKind::Assign(assignment) =
                &retained.basic_blocks[eq.0].statements[eq.1].kind
            else {
                unreachable!()
            };
            Operand::Copy(assignment.0)
        } else {
            discr.clone()
        };
        let temp = retained
            .local_decls
            .push(rustc_middle::mir::LocalDecl::new(tcx.types.bool, DUMMY_SP));
        let mut branch = retained.basic_blocks[switch].clone();
        branch.statements.clear();
        if let TerminatorKind::SwitchInt { discr, targets } = &mut branch.terminator_mut().kind {
            *discr = Operand::Move(temp.into());
            *targets = SwitchTargets::new([(0, nonzero)].into_iter(), zero);
        }
        let target = retained.basic_blocks_mut().push(branch);
        if let TerminatorKind::Call {
            args,
            destination,
            target: normal,
            unwind,
            ..
        } = &mut term.kind
        {
            args[0].node = predicate;
            *destination = temp.into();
            *normal = Some(target);
            *unwind = UnwindAction::Unreachable;
        }
        retained.basic_blocks_mut()[switch].terminator = Some(term);
        assert!(
            accepts(tcx, instance, &retained),
            "retaining exact hint preserves the proof"
        );
        for name in ["fake_hint", "panicking_hint"] {
            let mut changed = retained.clone();
            if let TerminatorKind::Call { func, .. } =
                &mut changed.basic_blocks_mut()[switch].terminator_mut().kind
            {
                *func = called_operand(tcx, name);
            }
            assert!(
                !accepts(tcx, instance, &changed),
                "foreign or panicking hint {name}"
            );
        }
    }
}

pub(super) fn state_and_bounds<'tcx>(
    tcx: TyCtxt<'tcx>,
    instance: Instance<'tcx>,
    body: &Body<'tcx>,
) {
    let word = identity(tcx, instance).unwrap();
    let eq = binary_site(body, BinOp::Eq);
    let div = binary_site(body, BinOp::Div);
    let mut alias = body.clone();
    let temp = alias
        .local_decls
        .push(rustc_middle::mir::LocalDecl::new(word, DUMMY_SP));
    binary_mut(&mut alias, eq).1.0 = Operand::Copy(temp.into());
    binary_mut(&mut alias, div).1.1 = Operand::Copy(temp.into());
    let init = assignment(
        temp.into(),
        Rvalue::Use(Operand::Copy(Local::from_usize(2).into())),
    );
    alias.basic_blocks_mut()[BasicBlock::from_usize(0)]
        .statements
        .insert(0, init.clone());
    assert!(
        accepts(tcx, instance, &alias),
        "exact copy alias preserves divisor provenance"
    );
    let alias_div = binary_site(&alias, BinOp::Div);
    let mut moved = alias.clone();
    binary_mut(&mut moved, alias_div).1.1 = Operand::Move(temp.into());
    assert!(
        accepts(tcx, instance, &moved),
        "last divisor use can move its exact alias"
    );
    for stale in [Operand::Copy(Local::from_usize(1).into()), {
        let TyKind::Uint(integer) = word.kind() else {
            unreachable!()
        };
        scalar(
            tcx,
            word,
            integer
                .bit_width()
                .unwrap_or(tcx.data_layout.pointer_size().bits()),
            0,
        )
    }] {
        let mut changed = alias.clone();
        changed.basic_blocks_mut()[alias_div.0]
            .statements
            .insert(alias_div.1, assignment(temp.into(), Rvalue::Use(stale)));
        assert!(
            !accepts(tcx, instance, &changed),
            "guard does not authorize overwritten divisor alias"
        );
    }
    let mut changed = body.clone();
    binary_mut(&mut changed, eq).1.0 = Operand::Move(Local::from_usize(2).into());
    assert!(
        !accepts(tcx, instance, &changed),
        "division reads a consumed divisor"
    );
    let mut changed = alias.clone();
    let consume = assignment(
        Local::from_usize(2).into(),
        Rvalue::Use(Operand::Move(temp.into())),
    );
    changed.basic_blocks_mut()[alias_div.0]
        .statements
        .insert(alias_div.1, consume);
    assert!(
        !accepts(tcx, instance, &changed),
        "moved alias cannot be read again"
    );
    let mut live = alias.clone();
    live.basic_blocks_mut()[BasicBlock::from_usize(0)]
        .statements
        .insert(
            0,
            Statement::new(
                SourceInfo::outermost(DUMMY_SP),
                StatementKind::StorageLive(temp),
            ),
        );
    assert!(
        accepts(tcx, instance, &live),
        "well-formed explicit storage"
    );
    let live_div = binary_site(&live, BinOp::Div);
    let mut changed = live.clone();
    changed.basic_blocks_mut()[live_div.0].statements.insert(
        live_div.1,
        Statement::new(
            SourceInfo::outermost(DUMMY_SP),
            StatementKind::StorageDead(temp),
        ),
    );
    assert!(
        !accepts(tcx, instance, &changed),
        "dead storage invalidates divisor alias"
    );
    let mut changed = live.clone();
    changed.basic_blocks_mut()[live_div.0].statements.splice(
        live_div.1..live_div.1,
        [
            Statement::new(
                SourceInfo::outermost(DUMMY_SP),
                StatementKind::StorageDead(temp),
            ),
            Statement::new(
                SourceInfo::outermost(DUMMY_SP),
                StatementKind::StorageLive(temp),
            ),
        ],
    );
    assert!(
        !accepts(tcx, instance, &changed),
        "storage reuse does not restore initialized value"
    );

    let mut changed = body.clone();
    changed.source.instance = local(tcx, "word").def;
    assert!(
        !accepts(tcx, instance, &changed),
        "body owner must match exact core instance"
    );
    let mut changed = body.clone();
    changed.arg_count = 1;
    assert!(!accepts(tcx, instance, &changed), "argument arity");
    let mut changed = body.clone();
    changed.is_polymorphic = true;
    assert!(!accepts(tcx, instance, &changed), "non-monomorphic body");
    let mut changed = body.clone();
    changed.basic_blocks_mut()[BasicBlock::from_usize(0)].is_cleanup = true;
    assert!(!accepts(tcx, instance, &changed), "cleanup graph");
    let mut changed = body.clone();
    changed.source_scopes[rustc_middle::mir::SourceScope::from_usize(0)].inlined =
        Some((local(tcx, "unlikely"), DUMMY_SP));
    assert!(
        !accepts(tcx, instance, &changed),
        "foreign inlined source scope"
    );
    let mut changed = body.clone();
    binary_mut(&mut changed, div).1.1 = Operand::Copy(Local::from_usize(MAX_LOCALS + 1).into());
    assert!(
        !accepts(tcx, instance, &changed),
        "invalid operand local fails closed"
    );

    let mut boundary = alias.clone();
    let count = boundary
        .basic_blocks
        .iter()
        .map(|block| block.statements.len())
        .sum::<usize>();
    for _ in count..MAX_STATEMENTS {
        boundary.basic_blocks_mut()[BasicBlock::from_usize(0)]
            .statements
            .insert(0, init.clone());
    }
    assert!(
        accepts(tcx, instance, &boundary),
        "statement bound inclusive"
    );
    boundary.basic_blocks_mut()[BasicBlock::from_usize(0)]
        .statements
        .insert(0, init);
    assert!(
        !accepts(tcx, instance, &boundary),
        "statement bound enforced"
    );
    let mut boundary = body.clone();
    while boundary.local_decls.len() < MAX_LOCALS {
        boundary
            .local_decls
            .push(rustc_middle::mir::LocalDecl::new(word, DUMMY_SP));
    }
    assert!(accepts(tcx, instance, &boundary), "local bound inclusive");
    boundary
        .local_decls
        .push(rustc_middle::mir::LocalDecl::new(word, DUMMY_SP));
    assert!(!accepts(tcx, instance, &boundary), "local bound enforced");
}
