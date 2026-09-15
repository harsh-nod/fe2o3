//! Original wrapper predicate. Provider and nominal authentication are separate.
use rustc_middle::mir::{
    BasicBlock, Body, Local, MirPhase, Operand, RETURN_PLACE, RuntimePhase, START_BLOCK,
    TerminatorKind, UnwindAction,
};
use rustc_middle::ty::{EarlyBinder, Instance, Ty, TyCtxt, TyKind, TypingEnv};

pub fn constructor<'tcx>(
    tcx: TyCtxt<'tcx>,
    instance: Instance<'tcx>,
    body: &Body<'tcx>,
    inputs: [Ty<'tcx>; 6],
    output: Ty<'tcx>,
    checked: Instance<'tcx>,
) -> bool {
    if body.source.instance != instance.def
        || body.source.promoted.is_some()
        || body.phase != MirPhase::Runtime(RuntimePhase::Optimized)
        || body.injection_phase.is_some()
        || body.tainted_by_errors.is_some()
        || body.coroutine.is_some()
        || body.spread_arg.is_some()
        || body.arg_count != 6
        || body.local_decls.len() != 7
        || body.basic_blocks.len() != 2
        || !body.user_type_annotations.is_empty()
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
    if !body
        .local_decls
        .iter()
        .zip(std::iter::once(output).chain(inputs))
        .all(|(declaration, expected)| normalize(declaration.ty) == Some(expected))
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
    } = &body.basic_blocks[START_BLOCK].terminator().kind
    else {
        return false;
    };
    if !matches!(func, Operand::Constant(_))
        || args.len() != 5
        || destination != &RETURN_PLACE.into()
        || *target != Some(BasicBlock::from_usize(1))
        || *unwind != UnwindAction::Unreachable
        || !matches!(
            body.basic_blocks[BasicBlock::from_usize(1)]
                .terminator()
                .kind,
            TerminatorKind::Return
        )
        || !args.iter().enumerate().all(|(index, argument)| {
            matches!(&argument.node, Operand::Copy(place)
                if place.local == Local::from_usize(index + 2) && place.projection.is_empty())
        })
    {
        return false;
    }
    let Some(ty) = normalize(func.ty(&body.local_decls, tcx)) else {
        return false;
    };
    let TyKind::FnDef(definition, arguments) = *ty.kind() else {
        return false;
    };
    Instance::try_resolve(
        tcx,
        TypingEnv::fully_monomorphized(),
        definition,
        tcx.erase_and_anonymize_regions(arguments),
    )
    .ok()
    .flatten()
        == Some(checked)
}
