//! Closed original-MIR predicates. These do not authenticate provider identities.

use rustc_middle::mir::{
    Body, Local, MirPhase, Operand, RETURN_PLACE, RuntimePhase, Rvalue, StatementKind,
    TerminatorKind, UnwindAction,
};
use rustc_middle::ty::{EarlyBinder, Instance, Ty, TyCtxt, TyKind, TypingEnv};

fn original_body<'tcx>(
    instance: Instance<'tcx>,
    body: &Body<'tcx>,
    arguments: usize,
    locals: usize,
    blocks: usize,
) -> bool {
    body.source.instance == instance.def
        && body.source.promoted.is_none()
        && body.phase == MirPhase::Runtime(RuntimePhase::Optimized)
        && body.injection_phase.is_none()
        && body.tainted_by_errors.is_none()
        && body.coroutine.is_none()
        && body.spread_arg.is_none()
        && body.arg_count == arguments
        && body.local_decls.len() == locals
        && body.basic_blocks.len() == blocks
        && body.user_type_annotations.is_empty()
        && body
            .basic_blocks
            .iter()
            .all(|block| !block.is_cleanup && block.terminator.is_some())
}

fn normalized<'tcx>(tcx: TyCtxt<'tcx>, instance: Instance<'tcx>, ty: Ty<'tcx>) -> Option<Ty<'tcx>> {
    instance
        .try_instantiate_mir_and_normalize_erasing_regions(
            tcx,
            TypingEnv::fully_monomorphized(),
            EarlyBinder::bind(ty),
        )
        .ok()
}

fn local_type<'tcx>(
    tcx: TyCtxt<'tcx>,
    instance: Instance<'tcx>,
    body: &Body<'tcx>,
    local: usize,
) -> Option<Ty<'tcx>> {
    normalized(
        tcx,
        instance,
        body.local_decls.get(Local::from_usize(local))?.ty,
    )
}

fn shared_pointee(ty: Ty<'_>) -> Option<Ty<'_>> {
    match *ty.kind() {
        TyKind::Ref(_, pointee, rustc_hir::Mutability::Not) => Some(pointee),
        _ => None,
    }
}

fn exact_forward_call<'tcx>(
    tcx: TyCtxt<'tcx>,
    instance: Instance<'tcx>,
    body: &Body<'tcx>,
    callee: Instance<'tcx>,
    destination_local: usize,
) -> bool {
    let entry = &body.basic_blocks[rustc_middle::mir::START_BLOCK];
    let exit = &body.basic_blocks[rustc_middle::mir::BasicBlock::from_usize(1)];
    if !entry.statements.is_empty()
        || !exit.statements.is_empty()
        || !matches!(exit.terminator().kind, TerminatorKind::Return)
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
    } = &entry.terminator().kind
    else {
        return false;
    };
    if !matches!(func, Operand::Constant(_))
        || !args.is_empty()
        || destination.local != Local::from_usize(destination_local)
        || !destination.projection.is_empty()
        || target.is_none_or(|target| target.index() != 1)
        || !matches!(unwind, UnwindAction::Continue | UnwindAction::Unreachable)
    {
        return false;
    }
    let Some(ty) = normalized(tcx, instance, func.ty(&body.local_decls, tcx)) else {
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
        == Some(callee)
}

/// The reviewed getter must forward to the separately reviewed branded helper.
/// The receiver is deliberately retained even though optimized MIR never reads it.
pub(super) fn kernel_math_getter<'tcx>(
    tcx: TyCtxt<'tcx>,
    instance: Instance<'tcx>,
    body: &Body<'tcx>,
    bridge: Instance<'tcx>,
    reference: Ty<'tcx>,
    math: Ty<'tcx>,
) -> bool {
    original_body(instance, body, 1, 2, 2)
        && shared_pointee(reference).is_some()
        && local_type(tcx, instance, body, 0) == Some(math)
        && local_type(tcx, instance, body, 1) == Some(reference)
        && exact_forward_call(tcx, instance, body, bridge, 0)
}

/// The observed bridge has an optimized-away ZST return aggregate. It is not
/// independently a Math issuer: its parent getter's actual receiver is required.
pub(super) fn branded_math_bridge<'tcx>(
    tcx: TyCtxt<'tcx>,
    instance: Instance<'tcx>,
    body: &Body<'tcx>,
    terminal: Instance<'tcx>,
    math: Ty<'tcx>,
    unbranded: Ty<'tcx>,
) -> bool {
    original_body(instance, body, 0, 2, 2)
        && math != unbranded
        && local_type(tcx, instance, body, 0) == Some(math)
        && local_type(tcx, instance, body, 1) == Some(unbranded)
        && exact_forward_call(tcx, instance, body, terminal, 1)
}

pub(super) fn policy_math_bind<'tcx>(
    tcx: TyCtxt<'tcx>,
    instance: Instance<'tcx>,
    body: &Body<'tcx>,
    math_reference: Ty<'tcx>,
    policy_reference: Ty<'tcx>,
    bound: Ty<'tcx>,
) -> bool {
    if !original_body(instance, body, 2, 3, 1)
        || shared_pointee(math_reference).is_none()
        || shared_pointee(policy_reference).is_none()
        || math_reference == policy_reference
        || local_type(tcx, instance, body, 0) != Some(bound)
        || local_type(tcx, instance, body, 1) != Some(math_reference)
        || local_type(tcx, instance, body, 2) != Some(policy_reference)
    {
        return false;
    }
    let block = &body.basic_blocks[rustc_middle::mir::START_BLOCK];
    let [statement] = block.statements.as_slice() else {
        return false;
    };
    if !matches!(block.terminator().kind, TerminatorKind::Return) {
        return false;
    }
    let StatementKind::Assign(assignment) = &statement.kind else {
        return false;
    };
    let Rvalue::Aggregate(kind, operands) = &assignment.1 else {
        return false;
    };
    let rustc_middle::mir::AggregateKind::Adt(definition, variant, arguments, annotation, field) =
        &**kind
    else {
        return false;
    };
    let Some(aggregate) = normalized(
        tcx,
        instance,
        Ty::new_adt(tcx, tcx.adt_def(*definition), *arguments),
    ) else {
        return false;
    };
    let TyKind::Adt(bound_definition, bound_arguments) = *bound.kind() else {
        return false;
    };
    if assignment.0 != RETURN_PLACE.into()
        || aggregate != bound
        || !bound_definition.is_struct()
        || variant.as_u32() != 0
        || annotation.is_some()
        || field.is_some()
        || operands.len() != 3
        || bound_definition.non_enum_variant().fields.len() != 3
    {
        return false;
    }
    for (index, operand) in operands.iter().take(2).enumerate() {
        if !matches!(operand, Operand::Copy(place)
            if place.local == Local::from_usize(index + 1) && place.projection.is_empty())
        {
            return false;
        }
    }
    let field_types = bound_definition
        .non_enum_variant()
        .fields
        .iter()
        .map(|field| normalized(tcx, instance, field.ty(tcx, bound_arguments)))
        .collect::<Vec<_>>();
    if field_types[..2] != [Some(math_reference), Some(policy_reference)] {
        return false;
    }
    let Some(marker) = field_types[2] else {
        return false;
    };
    matches!(marker.kind(), TyKind::Adt(definition, _)
        if Some(definition.did()) == tcx.lang_items().phantom_data())
        && matches!(operands.iter().nth(2), Some(Operand::Constant(constant))
            if normalized(tcx, instance, constant.const_.ty()) == Some(marker))
}
