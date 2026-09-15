use super::*;
use rustc_abi::FieldIdx;
use rustc_middle::mir::BasicBlockData;

fn assign<'tcx>(local: Local, operand: Operand<'tcx>) -> StatementKind<'tcx> {
    StatementKind::Assign(Box::new((local.into(), Rvalue::Use(operand))))
}

fn prepend<'tcx>(body: &mut Body<'tcx>, statements: Vec<StatementKind<'tcx>>) {
    body.basic_blocks_mut()[BasicBlock::from_usize(0)]
        .statements
        .splice(
            0..0,
            statements
                .into_iter()
                .map(|kind| Statement::new(SourceInfo::outermost(DUMMY_SP), kind)),
        );
}

pub(crate) fn storage_and_moves<'tcx>(tcx: TyCtxt<'tcx>, source: &Body<'tcx>, helper: Helper) {
    let mut base = source.clone();
    let temp = base
        .local_decls
        .push(source.local_decls[Local::from_usize(1)].clone());
    let output = base
        .local_decls
        .push(source.local_decls[Local::from_usize(1)].clone());
    let input = Operand::Copy(Local::from_usize(1).into());
    let copy = Operand::Copy(temp.into());
    let moved = Operand::Move(temp.into());
    for statements in [
        vec![assign(temp, input.clone()), assign(output, copy.clone())],
        vec![
            StatementKind::StorageLive(temp),
            assign(temp, input.clone()),
            StatementKind::StorageDead(temp),
            StatementKind::StorageLive(temp),
            assign(temp, input.clone()),
            assign(output, copy.clone()),
            StatementKind::StorageDead(temp),
        ],
        vec![
            assign(temp, input.clone()),
            assign(output, moved.clone()),
            assign(temp, input.clone()),
            assign(output, moved.clone()),
        ],
    ] {
        let mut body = base.clone();
        prepend(&mut body, statements);
        assert!(
            reviewed_body(tcx, &body, helper),
            "valid storage/move: {helper:?}"
        );
    }
    for statements in [
        vec![
            assign(temp, input.clone()),
            StatementKind::StorageDead(temp),
        ],
        vec![
            StatementKind::StorageLive(temp),
            StatementKind::StorageDead(temp),
            assign(temp, input.clone()),
        ],
        vec![
            StatementKind::StorageLive(temp),
            StatementKind::StorageLive(temp),
        ],
        vec![
            StatementKind::StorageLive(temp),
            StatementKind::StorageDead(temp),
            StatementKind::StorageDead(temp),
        ],
        vec![
            StatementKind::StorageLive(temp),
            assign(temp, input.clone()),
            StatementKind::StorageDead(temp),
            StatementKind::StorageLive(temp),
            assign(output, copy.clone()),
        ],
        vec![
            assign(temp, input.clone()),
            assign(output, moved.clone()),
            assign(output, copy.clone()),
        ],
        vec![
            assign(temp, input.clone()),
            assign(output, moved.clone()),
            assign(output, moved.clone()),
        ],
        vec![assign(temp, Operand::Move(Local::from_usize(1).into()))],
    ] {
        let mut body = base.clone();
        prepend(&mut body, statements);
        assert!(
            !reviewed_body(tcx, &body, helper),
            "unavailable storage/value: {helper:?}"
        );
    }

    // Call results obey the same destination-storage check as assignments.
    for (block, data) in source.basic_blocks.iter_enumerated() {
        let TerminatorKind::Call { destination, .. } = data.terminator().kind else {
            continue;
        };
        assert!(destination.projection.is_empty());
        if destination.local.as_usize() <= source.arg_count {
            continue;
        }
        let mut body = source.clone();
        for data in body.basic_blocks_mut().iter_mut() {
            data.statements.retain(|statement| {
                !matches!(statement.kind,
                StatementKind::StorageLive(local) | StatementKind::StorageDead(local)
                    if local == destination.local)
            });
        }
        prepend(
            &mut body,
            vec![StatementKind::StorageLive(destination.local)],
        );
        assert!(reviewed_body(tcx, &body, helper), "live call destination");
        body.basic_blocks_mut()[block]
            .statements
            .push(Statement::new(
                data.terminator().source_info,
                StatementKind::StorageDead(destination.local),
            ));
        assert!(!reviewed_body(tcx, &body, helper), "dead call destination");
    }
}

pub(crate) fn tuple_moves<'tcx>(tcx: TyCtxt<'tcx>, source: &Body<'tcx>, helper: Helper) {
    let (block, index, pair) = source
        .basic_blocks
        .iter_enumerated()
        .find_map(|(block, data)| {
            data.statements
                .iter()
                .enumerate()
                .find_map(|(index, statement)| {
                    let StatementKind::Assign(assignment) = &statement.kind else {
                        return None;
                    };
                    matches!(
                        assignment.1,
                        Rvalue::BinaryOp(
                            BinOp::AddWithOverflow
                                | BinOp::SubWithOverflow
                                | BinOp::MulWithOverflow,
                            _
                        )
                    )
                    .then_some((block, index, assignment.0))
                })
        })
        .expect("pinned overflowing arithmetic");
    let mut base = source.clone();
    let temp = base
        .local_decls
        .push(source.local_decls[pair.local].clone());
    let TyKind::Tuple(fields) = source.local_decls[pair.local].ty.kind() else {
        unreachable!()
    };
    let mut word_decl = source.local_decls[Local::from_usize(1)].clone();
    word_decl.ty = fields[0];
    let word = base.local_decls.push(word_decl);
    let mut flag_decl = source.local_decls[Local::from_usize(1)].clone();
    flag_decl.ty = tcx.types.bool;
    let flag = base.local_decls.push(flag_decl);
    let field = |index, ty| {
        Place::from(temp).project_deeper(
            &[ProjectionElem::Field(FieldIdx::from_usize(index), ty)],
            tcx,
        )
    };
    let first = field(0, fields[0]);
    let second = field(1, tcx.types.bool);
    for (extra, accepted) in [
        (
            vec![
                assign(word, Operand::Move(first)),
                assign(flag, Operand::Move(second)),
            ],
            true,
        ),
        (
            vec![
                assign(word, Operand::Move(first)),
                assign(word, Operand::Copy(first)),
            ],
            false,
        ),
        (
            vec![
                assign(flag, Operand::Move(second)),
                assign(flag, Operand::Move(second)),
            ],
            false,
        ),
        (
            vec![
                assign(word, Operand::Move(first)),
                assign(pair.local, Operand::Copy(temp.into())),
            ],
            false,
        ),
        (
            vec![
                assign(pair.local, Operand::Move(temp.into())),
                assign(flag, Operand::Copy(second)),
            ],
            false,
        ),
    ] {
        let mut body = base.clone();
        let statements = std::iter::once(assign(temp, Operand::Copy(pair)))
            .chain(extra)
            .map(|kind| Statement::new(SourceInfo::outermost(DUMMY_SP), kind));
        body.basic_blocks_mut()[block]
            .statements
            .splice(index + 1..index + 1, statements);
        assert_eq!(
            reviewed_body(tcx, &body, helper),
            accepted,
            "tuple-field move"
        );
    }
}

pub(crate) fn budget_boundaries<'tcx>(tcx: TyCtxt<'tcx>, source: &Body<'tcx>, helper: Helper) {
    let mut body = source.clone();
    while body.local_decls.len() < MAX_LOCALS {
        body.local_decls
            .push(source.local_decls[Local::from_usize(1)].clone());
    }
    assert!(reviewed_body(tcx, &body, helper), "local limit");
    body.local_decls
        .push(source.local_decls[Local::from_usize(1)].clone());
    assert!(!reviewed_body(tcx, &body, helper), "local limit + 1");

    let mut body = source.clone();
    let temp = body
        .local_decls
        .push(source.local_decls[Local::from_usize(1)].clone());
    let padding = Statement::new(
        SourceInfo::outermost(DUMMY_SP),
        assign(temp, Operand::Copy(Local::from_usize(1).into())),
    );
    let count: usize = body
        .basic_blocks
        .iter()
        .map(|block| block.statements.len())
        .sum();
    body.basic_blocks_mut()[BasicBlock::from_usize(0)]
        .statements
        .splice(
            0..0,
            std::iter::repeat_n(padding.clone(), MAX_STATEMENTS - count),
        );
    assert!(
        reviewed_body(tcx, &body, helper),
        "statement limit with valid assignments"
    );
    body.basic_blocks_mut()[BasicBlock::from_usize(0)]
        .statements
        .insert(0, padding);
    assert!(!reviewed_body(tcx, &body, helper), "statement limit + 1");

    let mut body = source.clone();
    let mut tail = body
        .basic_blocks
        .iter_enumerated()
        .find_map(|(block, data)| {
            matches!(data.terminator().kind, TerminatorKind::Return).then_some(block)
        })
        .unwrap();
    while body.basic_blocks.len() <= MAX_BLOCKS {
        if body.basic_blocks.len() == MAX_BLOCKS {
            assert!(
                reviewed_body(tcx, &body, helper),
                "block limit with reachable padding"
            );
        }
        let terminal = body.basic_blocks[tail].terminator().clone();
        let next =
            body.basic_blocks_mut()
                .push(BasicBlockData::new_stmts(vec![], Some(terminal), false));
        body.basic_blocks_mut()[tail].terminator_mut().kind = TerminatorKind::Goto { target: next };
        tail = next;
    }
    assert!(!reviewed_body(tcx, &body, helper), "block limit + 1");

    let mut body = source.clone();
    if matches!(helper, Helper::DivCeil | Helper::IsMultipleOf) {
        assert_eq!(
            body.source_scopes.len(),
            if helper == Helper::DivCeil { 3 } else { 1 },
            "exact quotient/remainder scope shape"
        );
    } else {
        while body.source_scopes.len() < MAX_SCOPES {
            body.source_scopes
                .push(source.source_scopes.iter().next().unwrap().clone());
        }
    }
    assert!(reviewed_body(tcx, &body, helper), "scope boundary");
    body.source_scopes
        .push(source.source_scopes.iter().next().unwrap().clone());
    assert!(!reviewed_body(tcx, &body, helper), "scope boundary + 1");
    if matches!(helper, Helper::DivCeil | Helper::IsMultipleOf) {
        let mut body = source.clone();
        let debug = body
            .var_debug_info
            .first()
            .expect("pinned argument debug info")
            .clone();
        body.var_debug_info.resize(MAX_LOCALS, debug.clone());
        assert!(reviewed_body(tcx, &body, helper), "debug limit");
        body.var_debug_info.push(debug);
        assert!(!reviewed_body(tcx, &body, helper), "debug limit + 1");
    }
}
