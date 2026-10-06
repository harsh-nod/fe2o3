//! Runs in the existing actual-rustc primitive/core fixture transaction.
use super::*;
use rustc_middle::mir::{Local, LocalDecl, Place};

fn only_call(body: &Body<'_>) -> BasicBlock {
    let mut calls = body
        .basic_blocks
        .iter_enumerated()
        .filter_map(|(block, data)| {
            matches!(data.terminator().kind, TerminatorKind::Call { .. }).then_some(block)
        });
    let block = calls.next().unwrap();
    assert!(calls.next().is_none());
    block
}

fn run<'tcx>(
    tcx: TyCtxt<'tcx>,
    caller: Instance<'tcx>,
    body: &Body<'tcx>,
    block: BasicBlock,
    limit: usize,
) -> (
    Result<Option<CorePanicV50<'tcx>>, CorePanicErrorV50<usize>>,
    usize,
) {
    let mut work = 0usize;
    let result = observe(tcx, caller, body, block, &mut |amount| {
        work = work.checked_add(amount).unwrap();
        if work > limit { Err(work) } else { Ok(()) }
    });
    (result, work)
}

pub(crate) fn original_calls<'tcx>(
    tcx: TyCtxt<'tcx>,
    caller: Instance<'tcx>,
    block: BasicBlock,
    dynamic: Instance<'tcx>,
    lookalike: Instance<'tcx>,
) {
    let body = tcx.instance_mir(caller.def);
    assert_eq!(only_call(body), block);
    let (result, work) = run(tcx, caller, body, block, usize::MAX);
    let checked = result.unwrap().unwrap();
    let literal = b"primitive From audit literal";
    assert_eq!(checked.bytes(), literal);
    assert_eq!(
        Some(checked.instance().def_id()),
        tcx.lang_items().panic_fn()
    );
    assert_eq!(work, 1 + 12 + 3 + 2 + literal.len() + 1);
    let exact = run(tcx, caller, body, block, work);
    assert_eq!(exact.1, work);
    assert!(checked.same_producers(exact.0.unwrap().unwrap()));
    let short = run(tcx, caller, body, block, work - 1);
    assert!(matches!(short.0, Err(CorePanicErrorV50::Work(actual)) if actual == work));
    assert_eq!(short.1, work);

    let dynamic_body = tcx.instance_mir(dynamic.def);
    assert!(matches!(
        run(
            tcx,
            dynamic,
            dynamic_body,
            only_call(dynamic_body),
            usize::MAX
        )
        .0,
        Err(CorePanicErrorV50::Refused(
            "core panic message requires retained argument evaluation"
        ))
    ));
    let local_body = tcx.instance_mir(lookalike.def);
    let local_block = only_call(local_body);
    assert!(
        run(tcx, lookalike, local_body, local_block, usize::MAX)
            .0
            .unwrap()
            .is_none()
    );

    for fault in 0..8 {
        let mut changed = body.clone();
        let message_local = changed
            .local_decls
            .push(LocalDecl::new(checked.message_type(), changed.span));
        let absent = Local::from_usize(changed.local_decls.len());
        let TerminatorKind::Call {
            args,
            target,
            unwind,
            destination,
            ..
        } = &mut changed.basic_blocks.as_mut()[block]
            .terminator
            .as_mut()
            .unwrap()
            .kind
        else {
            unreachable!()
        };
        match fault {
            0 => *target = Some(block),
            1 => *unwind = UnwindAction::Continue,
            2 => args[0].node = Operand::Copy(Place::from(message_local)),
            3 => args[0].node = Operand::Move(Place::from(message_local)),
            4 => *destination = Place::from(absent),
            5 => *destination = Place::from(message_local),
            6 | 7 => {
                let Operand::Constant(message) = &mut args[0].node else {
                    unreachable!()
                };
                let Const::Val(ConstValue::Slice { alloc_id, meta }, ty) = message.const_ else {
                    unreachable!()
                };
                message.const_ = if fault == 6 {
                    Const::Val(
                        ConstValue::Slice {
                            alloc_id,
                            meta: u64::MAX,
                        },
                        ty,
                    )
                } else {
                    Const::Val(ConstValue::Slice { alloc_id, meta }, tcx.types.u32)
                };
            }
            _ => unreachable!(),
        }
        let result = run(tcx, caller, &changed, block, usize::MAX).0;
        let expected = match fault {
            0 | 1 | 4 | 5 => "core panic requires its exact nonreturning call contract",
            2 | 3 => "core panic message requires retained argument evaluation",
            6 => "core panic literal allocation contract differs",
            7 => "core panic original literal type differs",
            _ => unreachable!(),
        };
        assert!(
            matches!(result, Err(CorePanicErrorV50::Refused(reason)) if reason == expected),
            "fault {fault}: {result:?}"
        );
    }
    let replay = run(tcx, caller, body, block, usize::MAX)
        .0
        .unwrap()
        .unwrap();
    assert!(checked.same_producers(replay));
    let mut substituted = replay;
    substituted.caller = dynamic;
    assert!(!checked.same_producers(substituted));
    substituted = replay;
    substituted.block = BasicBlock::from_usize(body.basic_blocks.len());
    assert!(!checked.same_producers(substituted));
}
