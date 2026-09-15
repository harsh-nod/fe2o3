//! Shape of the authenticated source stub, never a successful constructor proof.
use rustc_middle::mir::{
    BasicBlock, Body, ConstOperand, Local, MirPhase, Operand, RuntimePhase, START_BLOCK,
    TerminatorKind, UnwindAction,
};
use rustc_middle::ty::{EarlyBinder, Instance, Ty, TyCtxt, TyKind, TypingEnv};

pub fn issuer_stub<'tcx>(
    tcx: TyCtxt<'tcx>,
    instance: Instance<'tcx>,
    body: &Body<'tcx>,
    receiver: Ty<'tcx>,
    output: Ty<'tcx>,
    format: Instance<'tcx>,
    panic: Instance<'tcx>,
    original_message: &ConstOperand<'tcx>,
) -> bool {
    if body.source.instance != instance.def
        || body.source.promoted.is_some()
        || body.phase != MirPhase::Runtime(RuntimePhase::Optimized)
        || body.injection_phase.is_some()
        || body.tainted_by_errors.is_some()
        || body.coroutine.is_some()
        || body.spread_arg.is_some()
        || !body.user_type_annotations.is_empty()
        || body.arg_count != 1
        || body.local_decls.len() != 4
        || body.basic_blocks.len() != 2
        || body.basic_blocks.iter().any(|block| {
            block.is_cleanup || block.terminator.is_none() || !block.statements.is_empty()
        })
    {
        return false;
    }
    let normalize = |ty| {
        instance
            .try_instantiate_mir_and_normalize_erasing_regions(
                tcx,
                TypingEnv::fully_monomorphized(),
                EarlyBinder::bind(ty),
            )
            .ok()
    };
    let format_signature = tcx.instantiate_bound_regions_with_erased(
        tcx.fn_sig(format.def_id()).instantiate(tcx, format.args),
    );
    if !body
        .local_decls
        .iter()
        .zip([output, receiver, tcx.types.never, format_signature.output()])
        .all(|(local, expected)| normalize(local.ty) == Some(expected))
    {
        return false;
    }
    let resolve = |operand: &Operand<'tcx>| {
        if !matches!(operand, Operand::Constant(_)) {
            return None;
        }
        let TyKind::FnDef(definition, arguments) =
            *normalize(operand.ty(&body.local_decls, tcx))?.kind()
        else {
            return None;
        };
        Instance::try_resolve(
            tcx,
            TypingEnv::fully_monomorphized(),
            definition,
            tcx.erase_and_anonymize_regions(arguments),
        )
        .ok()
        .flatten()
    };
    let TerminatorKind::Call {
        func,
        args,
        destination,
        target,
        unwind,
        ..
    } = &body.basic_blocks[START_BLOCK].terminator().kind
    else {
        return false;
    };
    if resolve(func) != Some(format)
        || !matches!(args.as_ref(), [argument] if matches!(&argument.node,
            Operand::Constant(message) if message.const_ == original_message.const_))
        || *destination != Local::from_usize(3).into()
        || *target != Some(BasicBlock::from_usize(1))
        || *unwind != UnwindAction::Unreachable
    {
        return false;
    }
    let TerminatorKind::Call {
        func,
        args,
        destination,
        target,
        unwind,
        ..
    } = &body.basic_blocks[BasicBlock::from_usize(1)]
        .terminator()
        .kind
    else {
        return false;
    };
    resolve(func) == Some(panic)
        && matches!(args.as_ref(), [argument] if matches!(&argument.node,
            Operand::Move(place) if *place == Local::from_usize(3).into()))
        && *destination == Local::from_usize(2).into()
        && target.is_none()
        && *unwind == UnwindAction::Unreachable
}
