use super::*;
use rustc_abi::FieldIdx;

pub(super) fn check<'tcx>(
    tcx: TyCtxt<'tcx>,
    called: impl Fn(&'static str) -> Instance<'tcx>,
    body: impl Fn(&'static str) -> &'tcx Body<'tcx>,
) {
    for (checked, overflowing, route, word) in [
        ("checked", "overflowing", "route", Word::Usize),
        ("checked_u32", "overflowing_u32", "route_u32", Word::U32),
        ("checked_u64", "overflowing_u64", "route_u64", Word::U64),
    ] {
        let helper = Helper::Checked(Operation::Mul, word);
        let actual = tcx.instance_mir(called(checked).def);
        assert!(reviewed_body(tcx, actual, helper));
        for other in [Word::Usize, Word::U32, Word::U64] {
            if other != word {
                assert!(!helper.may_call(Helper::Overflowing(Operation::Mul, other)));
                let wrong = match other {
                    Word::Usize => called("overflowing"),
                    Word::U32 => called("overflowing_u32"),
                    Word::U64 => called("overflowing_u64"),
                };
                let mut changed = actual.clone();
                changed.source_scopes.iter_mut().next().unwrap().inlined = Some((wrong, DUMMY_SP));
                assert!(
                    !reviewed_body(tcx, &changed, helper),
                    "cross-width inlined helper"
                );
                let mut changed = body(route).clone();
                let TerminatorKind::Call { func, .. } = &mut changed.basic_blocks_mut()
                    [BasicBlock::from_usize(0)]
                .terminator_mut()
                .kind
                else {
                    unreachable!()
                };
                *func = Operand::Constant(Box::new(ConstOperand {
                    span: DUMMY_SP,
                    user_ty: None,
                    const_: Const::Val(
                        ConstValue::ZeroSized,
                        Ty::new_fn_def(tcx, wrong.def_id(), wrong.args),
                    ),
                }));
                assert!(
                    !reviewed_body(tcx, &changed, helper),
                    "cross-width retained helper"
                );

                let mut changed = actual.clone();
                let TyKind::Adt(adt, _) = changed.local_decls[RETURN_PLACE].ty.kind() else {
                    unreachable!()
                };
                changed.local_decls[RETURN_PLACE].ty =
                    Ty::new_adt(tcx, *adt, tcx.mk_args(&[other.ty(tcx).into()]));
                assert!(
                    !reviewed_body(tcx, &changed, helper),
                    "Option payload type is exact, not merely same-width"
                );
            }
            let cast_body =
                overflow_with_casts(tcx, tcx.instance_mir(called(overflowing).def), word, other);
            assert_eq!(
                reviewed_body(tcx, &cast_body, Helper::Overflowing(Operation::Mul, word)),
                word.bits(tcx) == other.bits(tcx),
                "casting to {other:?} before overflow computation for {word:?}",
            );
        }
        let instance = called(checked);
        assert!(!authenticate_reviewed_safe_core_arithmetic_helper_v1(
            tcx,
            Instance {
                def: instance.def,
                args: tcx.mk_args(&[word.ty(tcx).into()]),
            }
        ));
        if word != Word::Usize {
            for operation in [Operation::Add, Operation::Sub] {
                let reviewed = word == Word::U64 && operation == Operation::Add;
                assert_eq!(Helper::Checked(operation, word).reviewed_width(), reviewed);
                assert_eq!(Helper::Overflowing(operation, word).reviewed_width(), reviewed);
            }
        }
    }
}

fn overflow_with_casts<'tcx>(
    tcx: TyCtxt<'tcx>,
    source: &Body<'tcx>,
    word: Word,
    intermediate: Word,
) -> Body<'tcx> {
    let mut body = source.clone();
    let mut add_local = |ty| {
        let mut decl = source.local_decls[Local::from_usize(1)].clone();
        decl.ty = ty;
        body.local_decls.push(decl)
    };
    let left = add_local(intermediate.ty(tcx));
    let right = add_local(intermediate.ty(tcx));
    let pair = add_local(Ty::new_tup(tcx, &[intermediate.ty(tcx), tcx.types.bool]));
    let low = add_local(word.ty(tcx));
    let flag = add_local(tcx.types.bool);
    let field = |index, ty| {
        Place::from(pair).project_deeper(
            &[ProjectionElem::Field(FieldIdx::from_usize(index), ty)],
            tcx,
        )
    };
    let assignment = |local: Local, expression| {
        Statement::new(
            SourceInfo::outermost(DUMMY_SP),
            StatementKind::Assign(Box::new((local.into(), expression))),
        )
    };
    let block = &mut body.basic_blocks_mut()[BasicBlock::from_usize(0)];
    block.statements = vec![
        assignment(
            left,
            Rvalue::Cast(
                CastKind::IntToInt,
                Operand::Copy(Local::from_usize(1).into()),
                intermediate.ty(tcx),
            ),
        ),
        assignment(
            right,
            Rvalue::Cast(
                CastKind::IntToInt,
                Operand::Copy(Local::from_usize(2).into()),
                intermediate.ty(tcx),
            ),
        ),
        assignment(
            pair,
            Rvalue::BinaryOp(
                BinOp::MulWithOverflow,
                Box::new((Operand::Copy(left.into()), Operand::Copy(right.into()))),
            ),
        ),
        assignment(
            low,
            Rvalue::Cast(
                CastKind::IntToInt,
                Operand::Move(field(0, intermediate.ty(tcx))),
                word.ty(tcx),
            ),
        ),
        assignment(flag, Rvalue::Use(Operand::Move(field(1, tcx.types.bool)))),
        assignment(
            RETURN_PLACE,
            Rvalue::Aggregate(
                Box::new(AggregateKind::Tuple),
                [Operand::Move(low.into()), Operand::Move(flag.into())]
                    .into_iter()
                    .collect(),
            ),
        ),
    ];
    assert_eq!(body.basic_blocks.len(), 1, "pinned overflowing helper");
    body
}
