//! Original optimized rustc MIR predicates; provider authentication is separate.
use super::*;
use rustc_middle::mir::{
    AggregateKind, Body, Local, MirPhase, Operand, ProjectionElem, RETURN_PLACE, RuntimePhase,
    Rvalue, START_BLOCK, StatementKind, TerminatorKind, UnwindAction,
};
use rustc_middle::ty::{EarlyBinder, TypingEnv};

fn original<'tcx>(
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
            .all(|b| !b.is_cleanup && b.terminator.is_some())
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

fn locals<'tcx>(
    tcx: TyCtxt<'tcx>,
    instance: Instance<'tcx>,
    body: &Body<'tcx>,
    types: &[Ty<'tcx>],
) -> bool {
    body.local_decls.len() == types.len()
        && body
            .local_decls
            .iter()
            .zip(types)
            .all(|(local, ty)| normalized(tcx, instance, local.ty) == Some(*ty))
}

fn copied(operand: &Operand<'_>, local: usize) -> bool {
    matches!(operand, Operand::Copy(place) if place.local == Local::from_usize(local) && place.projection.is_empty())
}

fn returned_aggregate<'tcx>(
    tcx: TyCtxt<'tcx>,
    instance: Instance<'tcx>,
    body: &Body<'tcx>,
    block: usize,
    output: Ty<'tcx>,
    references: &[usize],
    field_count: usize,
) -> bool {
    let block = &body.basic_blocks[rustc_middle::mir::BasicBlock::from_usize(block)];
    if !matches!(block.terminator().kind, TerminatorKind::Return) {
        return false;
    }
    let [statement] = block.statements.as_slice() else {
        return false;
    };
    let StatementKind::Assign(assignment) = &statement.kind else {
        return false;
    };
    let Rvalue::Aggregate(kind, operands) = &assignment.1 else {
        return false;
    };
    let AggregateKind::Adt(definition, variant, arguments, annotation, field) = &**kind else {
        return false;
    };
    let TyKind::Adt(output_definition, output_arguments) = *output.kind() else {
        return false;
    };
    if assignment.0 != RETURN_PLACE.into()
        || !output_definition.is_struct()
        || normalized(
            tcx,
            instance,
            Ty::new_adt(tcx, tcx.adt_def(*definition), *arguments),
        ) != Some(output)
        || variant.as_u32() != 0
        || annotation.is_some()
        || field.is_some()
        || operands.len() != field_count
        || output_definition.non_enum_variant().fields.len() != field_count
    {
        return false;
    }
    for (index, (operand, field)) in operands
        .iter()
        .zip(output_definition.non_enum_variant().fields.iter())
        .enumerate()
    {
        let Some(expected) = normalized(tcx, instance, field.ty(tcx, output_arguments)) else {
            return false;
        };
        if let Some(local) = references.get(index) {
            if !copied(operand, *local)
                || normalized(
                    tcx,
                    instance,
                    body.local_decls[Local::from_usize(*local)].ty,
                ) != Some(expected)
            {
                return false;
            }
        } else if !matches!(expected.kind(), TyKind::Adt(definition, _) if Some(definition.did()) == tcx.lang_items().phantom_data())
            || !matches!(operand, Operand::Constant(constant) if normalized(tcx, instance, constant.const_.ty()) == Some(expected))
        {
            return false;
        }
    }
    true
}

pub(super) fn bind<'tcx>(
    tcx: TyCtxt<'tcx>,
    instance: Instance<'tcx>,
    body: &Body<'tcx>,
    types: [Ty<'tcx>; 5],
) -> bool {
    let [matrix_reference, _, policy_reference, _, bound] = types;
    original(instance, body, 2, 3, 1)
        && locals(
            tcx,
            instance,
            body,
            &[bound, matrix_reference, policy_reference],
        )
        && returned_aggregate(tcx, instance, body, 0, bound, &[1, 2], 4)
}

pub(super) fn narrow<'tcx>(
    tcx: TyCtxt<'tcx>,
    instance: Instance<'tcx>,
    body: &Body<'tcx>,
    types: [Ty<'tcx>; 7],
    projection: Instance<'tcx>,
) -> bool {
    let [matrix_reference, _, _, _, _, reference, output] = types;
    if !original(instance, body, 1, 3, 2)
        || !locals(tcx, instance, body, &[output, reference, matrix_reference])
    {
        return false;
    }
    let entry = &body.basic_blocks[START_BLOCK];
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
    if !entry.statements.is_empty()
        || !matches!(func, Operand::Constant(_))
        || args.len() != 1
        || !copied(&args[0].node, 1)
        || destination.local != Local::from_usize(2)
        || !destination.projection.is_empty()
        || target.is_none_or(|b| b.index() != 1)
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
    let observed = Instance::try_resolve(
        tcx,
        TypingEnv::fully_monomorphized(),
        definition,
        tcx.erase_and_anonymize_regions(arguments),
    )
    .ok()
    .flatten();
    observed == Some(projection) && returned_aggregate(tcx, instance, body, 1, output, &[1], 2)
}

pub(super) fn projection<'tcx>(
    tcx: TyCtxt<'tcx>,
    instance: Instance<'tcx>,
    body: &Body<'tcx>,
    types: [Ty<'tcx>; 7],
) -> bool {
    let [matrix_reference, _, _, _, bound, reference, _] = types;
    if !original(instance, body, 1, 2, 1)
        || !locals(tcx, instance, body, &[matrix_reference, reference])
    {
        return false;
    }
    let block = &body.basic_blocks[START_BLOCK];
    let [statement] = block.statements.as_slice() else {
        return false;
    };
    let StatementKind::Assign(assignment) = &statement.kind else {
        return false;
    };
    let Rvalue::Use(Operand::Copy(place)) = &assignment.1 else {
        return false;
    };
    let [ProjectionElem::Deref, ProjectionElem::Field(field, ty)] = place.projection.as_ref()
    else {
        return false;
    };
    matches!(block.terminator().kind, TerminatorKind::Return)
        && assignment.0 == RETURN_PLACE.into()
        && place.local == Local::from_usize(1)
        && field.as_u32() == 0
        && normalized(tcx, instance, *ty) == Some(matrix_reference)
        && rust_shared_reference_v1(reference) == Some(bound)
}
