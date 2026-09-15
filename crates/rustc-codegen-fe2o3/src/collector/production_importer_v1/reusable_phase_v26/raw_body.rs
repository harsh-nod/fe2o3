//! Closed original-rustc body recipes. Never canonical-index assumptions and
//! never an independent issuer: the live source plan also checks nominal types,
//! complete incoming sites, HIR/reference custody and the canonical ABI roster.
use super::*;
use rustc_middle::mir::{
    self, BasicBlock, Body, Local, Operand, Place, Rvalue, StatementKind, TerminatorKind,
    UnwindAction,
};
use rustc_middle::ty::{EarlyBinder, TypeFoldable, TypingEnv};

/// Original operand spelling, not an ownership receipt. Only the enclosing
/// reviewed definition plus live source/occurrence custody can justify Copy.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum OriginalOperand {
    Move,
    Copy,
}

#[derive(Clone, Copy)]
pub(crate) enum BodyRecipe<'tcx> {
    OwnerConvert,
    Issue,
    Bind,
    WithPhase {
        issue: Instance<'tcx>,
        invoke: Instance<'tcx>,
        drop: Instance<'tcx>,
        issue_block: BasicBlock,
        invoke_block: BasicBlock,
        drop_block: BasicBlock,
        return_block: BasicBlock,
        phase: Local,
        tuple: Local,
        result_pair: Local,
        drop_result: Local,
        closure_operand: OriginalOperand,
    },
    Finish {
        barrier: Instance<'tcx>,
        barrier_block: BasicBlock,
        return_block: BasicBlock,
        advanced: Local,
        barrier_operand: OriginalOperand,
    },
}

pub(super) fn normalize<'tcx, T: TypeFoldable<TyCtxt<'tcx>>>(
    tcx: TyCtxt<'tcx>,
    instance: Instance<'tcx>,
    value: T,
) -> Result<T> {
    instance
        .try_instantiate_mir_and_normalize_erasing_regions(
            tcx,
            TypingEnv::fully_monomorphized(),
            EarlyBinder::bind(value),
        )
        .map_err(|_| Error::Body("phase source normalization failed"))
}

pub(super) fn local_type<'tcx>(
    tcx: TyCtxt<'tcx>,
    instance: Instance<'tcx>,
    body: &Body<'tcx>,
    local: Local,
) -> Result<Ty<'tcx>> {
    normalize(
        tcx,
        instance,
        body.local_decls
            .get(local)
            .ok_or(Error::Body("phase original local is absent"))?
            .ty,
    )
}

fn whole(place: Place<'_>, local: Local) -> bool {
    place.local == local && place.projection.is_empty()
}
fn move_local(operand: &Operand<'_>, local: Local) -> bool {
    matches!(operand, Operand::Move(place) if whole(*place, local))
}
fn copy_local(operand: &Operand<'_>, local: Local) -> bool {
    matches!(operand, Operand::Copy(place) if whole(*place, local))
}

fn original_operand(operand: &Operand<'_>, local: Local) -> Option<OriginalOperand> {
    match operand {
        Operand::Move(place) if whole(*place, local) => Some(OriginalOperand::Move),
        Operand::Copy(place) if whole(*place, local) => Some(OriginalOperand::Copy),
        _ => None,
    }
}

struct Call<'a, 'tcx> {
    instance: Instance<'tcx>,
    arguments: &'a [rustc_span::Spanned<Operand<'tcx>>],
    destination: Place<'tcx>,
    target: BasicBlock,
}

fn call<'a, 'tcx>(
    tcx: TyCtxt<'tcx>,
    instance: Instance<'tcx>,
    body: &'a Body<'tcx>,
    block: BasicBlock,
    arguments: usize,
    work: &mut usize,
) -> Result<Call<'a, 'tcx>> {
    charge(work, 1 + arguments)?;
    let block = body
        .basic_blocks
        .get(block)
        .ok_or(Error::Body("phase call block missing"))?;
    let TerminatorKind::Call {
        func,
        args,
        destination,
        target: Some(target),
        unwind: UnwindAction::Unreachable,
        ..
    } = &block.terminator().kind
    else {
        return Err(Error::Body("phase call is not an exact normal-return call"));
    };
    if args.len() != arguments
        || !destination.projection.is_empty()
        || !matches!(func, Operand::Constant(_))
    {
        return Err(Error::Body(
            "phase call arity, destination or static callee changed",
        ));
    }
    let function = normalize(tcx, instance, func.ty(&body.local_decls, tcx))?;
    let TyKind::FnDef(definition, args) = *function.kind() else {
        return Err(Error::Body("phase callee is not an original function item"));
    };
    let resolved = Instance::try_resolve(
        tcx,
        TypingEnv::fully_monomorphized(),
        definition,
        tcx.erase_and_anonymize_regions(args),
    )
    .ok()
    .flatten()
    .ok_or(Error::Body("phase original callee did not resolve"))?;
    // Re-read the original args after resolving the function's generic args.
    let TerminatorKind::Call { args, .. } = &block.terminator().kind else {
        unreachable!()
    };
    Ok(Call {
        instance: resolved,
        arguments: args,
        destination: *destination,
        target: *target,
    })
}

fn returning(body: &Body<'_>, block: BasicBlock) -> bool {
    body.basic_blocks.get(block).is_some_and(|b| {
        b.statements.is_empty() && matches!(b.terminator().kind, TerminatorKind::Return)
    })
}

fn chain(body: &Body<'_>, path: &[BasicBlock], work: &mut usize) -> Result<()> {
    charge(work, path.len())?;
    if path.first() != Some(&mir::START_BLOCK) || path.len() != body.basic_blocks.len() {
        return Err(Error::Body(
            "phase definition is not the complete acyclic body",
        ));
    }
    for (index, block) in path.iter().enumerate() {
        for previous in &path[..index] {
            charge(work, 1)?;
            if previous == block {
                return Err(Error::Body("phase body repeats an original block"));
            }
        }
        for (from, data) in body.basic_blocks.iter_enumerated() {
            // Account scans of Return/Unreachable blocks too, not only edges.
            charge(work, 1)?;
            for successor in data.terminator().successors() {
                charge(work, 1)?;
                if successor == *block && (index == 0 || from != path[index - 1]) {
                    return Err(Error::Body("phase body has a bypass or reentry edge"));
                }
            }
        }
    }
    Ok(())
}

fn zst_operand<'tcx>(
    tcx: TyCtxt<'tcx>,
    instance: Instance<'tcx>,
    operand: &Operand<'tcx>,
    expected: Ty<'tcx>,
) -> Result<bool> {
    let Operand::Constant(constant) = operand else {
        return Ok(false);
    };
    let value = normalize(tcx, instance, constant.const_)?;
    Ok(normalize(tcx, instance, value.ty())? == expected
        && value
            .eval(tcx, TypingEnv::fully_monomorphized(), constant.span)
            .ok()
            == Some(mir::ConstValue::ZeroSized))
}

fn constructor<'tcx>(
    tcx: TyCtxt<'tcx>,
    instance: Instance<'tcx>,
    body: &Body<'tcx>,
    borrowed: bool,
    work: &mut usize,
) -> Result<()> {
    if body.arg_count != 1 || body.local_decls.len() != 4 || body.basic_blocks.len() != 1 {
        return Err(Error::Body(
            "phase physical constructor local/block roster changed",
        ));
    }
    let data = &body.basic_blocks[mir::START_BLOCK];
    if data.statements.len() != 3 || !matches!(data.terminator().kind, TerminatorKind::Return) {
        return Err(Error::Body("phase physical constructor body changed"));
    }
    let argument = Local::from_u32(1);
    let mut fields = [Local::from_u32(0); 2];
    for (index, statement) in data.statements[..2].iter().enumerate() {
        charge(work, 1)?;
        let StatementKind::Assign(assignment) = &statement.kind else {
            return Err(Error::Body("phase observation is not assigned"));
        };
        let Rvalue::Use(Operand::Copy(place)) = &assignment.1 else {
            return Err(Error::Body("phase observation is not a source field read"));
        };
        if !assignment.0.projection.is_empty()
            || assignment.0.local.index() < 2
            || place.local != argument
            || local_type(tcx, instance, body, assignment.0.local)? != tcx.types.u64
        {
            return Err(Error::Body("phase size/rank transport changed"));
        }
        let projection = match place.projection.as_slice() {
            [mir::ProjectionElem::Deref, field] if borrowed => field,
            [field] if !borrowed => field,
            _ => return Err(Error::Body("phase constructor receiver projection changed")),
        };
        if !matches!(projection, mir::ProjectionElem::Field(field, ty)
            if field.as_u32() == index as u32 && normalize(tcx, instance, *ty)? == tcx.types.u64)
        {
            return Err(Error::Body("phase size/rank field order changed"));
        }
        fields[index] = assignment.0.local;
    }
    if fields[0] == fields[1] {
        return Err(Error::Body("phase size/rank locals collapsed"));
    }
    let StatementKind::Assign(assignment) = &data.statements[2].kind else {
        return Err(Error::Body("phase constructor has no result assignment"));
    };
    let Rvalue::Aggregate(kind, operands) = &assignment.1 else {
        return Err(Error::Body(
            "phase constructor result is not its source aggregate",
        ));
    };
    let mir::AggregateKind::Adt(definition, variant, arguments, annotation, field) = &**kind else {
        return Err(Error::Body("phase constructor substituted aggregate kind"));
    };
    let output = local_type(tcx, instance, body, mir::RETURN_PLACE)?;
    let TyKind::Adt(actual, args) = *output.kind() else {
        return Err(Error::Body("phase constructor result is not an ADT"));
    };
    if !whole(assignment.0, mir::RETURN_PLACE)
        || *definition != actual.did()
        || variant.as_u32() != 0
        || annotation.is_some()
        || field.is_some()
        || normalize(tcx, instance, *arguments)? != args
        || operands.len() != 4
        || actual.non_enum_variant().fields.len() != 4
        || !move_local(&operands[rustc_abi::FieldIdx::from_usize(0)], fields[0])
        || !move_local(&operands[rustc_abi::FieldIdx::from_usize(1)], fields[1])
    {
        return Err(Error::Body(
            "phase constructor did not retain both actual observations",
        ));
    }
    for index in 2..4 {
        charge(work, 1)?;
        let ty = normalize(
            tcx,
            instance,
            actual.non_enum_variant().fields[rustc_abi::FieldIdx::from_usize(index)].ty(tcx, args),
        )?;
        if !zst_operand(tcx, instance, &operands[rustc_abi::FieldIdx::from_usize(index)], ty)? {
            return Err(Error::Body("phase constructor marker operand changed"));
        }
    }
    Ok(())
}

pub(super) fn observe<'tcx>(
    tcx: TyCtxt<'tcx>,
    instance: Instance<'tcx>,
    body: &Body<'tcx>,
    role: Role,
    work: &mut usize,
) -> Result<BodyRecipe<'tcx>> {
    charge(
        work,
        body.basic_blocks
            .len()
            .checked_add(body.local_decls.len())
            .ok_or(Error::Work)?,
    )?;
    if !std::ptr::eq(body, tcx.instance_mir(instance.def))
        || body.source.instance != instance.def
        || body.source.promoted.is_some()
        || body.phase != mir::MirPhase::Runtime(mir::RuntimePhase::Optimized)
        || body.injection_phase.is_some()
        || body.tainted_by_errors.is_some()
        || body.coroutine.is_some()
        || body.spread_arg.is_some()
        || !body.user_type_annotations.is_empty()
        || body.basic_blocks.iter().any(|b| b.is_cleanup)
    {
        return Err(Error::Body(
            "phase helper is not the retained original optimized definition",
        ));
    }
    match role {
        Role::OwnerConvert => {
            constructor(tcx, instance, body, false, work)?;
            Ok(BodyRecipe::OwnerConvert)
        }
        Role::Issue => {
            constructor(tcx, instance, body, true, work)?;
            Ok(BodyRecipe::Issue)
        }
        Role::Bind => {
            if body.arg_count != 2
                || body.local_decls.len() != 3
                || body.basic_blocks.len() != 1
                || !returning(body, mir::START_BLOCK)
            {
                return Err(Error::Body(
                    "phase Bind is not its exact two-reference erased body",
                ));
            }
            Ok(BodyRecipe::Bind)
        }
        Role::Finish => {
            if body.arg_count != 1
                || body.local_decls.len() != 3
                || body.basic_blocks.len() != 2
                || !body.basic_blocks[mir::START_BLOCK].statements.is_empty()
            {
                return Err(Error::Body("phase Finish body roster changed"));
            }
            let barrier = call(tcx, instance, body, mir::START_BLOCK, 1, work)?;
            let barrier_operand = original_operand(&barrier.arguments[0].node, Local::from_u32(1))
                .ok_or(Error::Body(
                    "phase Finish changed its original workgroup operand",
                ))?;
            if barrier.destination.local.index() != 2 || !returning(body, barrier.target) {
                return Err(Error::Body(
                    "phase Finish did not consume its exact workgroup and barrier result",
                ));
            }
            chain(body, &[mir::START_BLOCK, barrier.target], work)?;
            Ok(BodyRecipe::Finish {
                barrier: barrier.instance,
                barrier_block: mir::START_BLOCK,
                return_block: barrier.target,
                advanced: barrier.destination.local,
                barrier_operand,
            })
        }
        Role::WithPhase => {
            if body.arg_count != 2
                || body.local_decls.len() != 7
                || body.basic_blocks.len() != 4
                || !body.basic_blocks[mir::START_BLOCK].statements.is_empty()
            {
                return Err(Error::Body("with_phase body roster changed"));
            }
            let issue = call(tcx, instance, body, mir::START_BLOCK, 1, work)?;
            let invoke = call(tcx, instance, body, issue.target, 2, work)?;
            let drop = call(tcx, instance, body, invoke.target, 1, work)?;
            let closure_operand = original_operand(&invoke.arguments[0].node, Local::from_u32(2))
                .ok_or(Error::Body(
                "with_phase changed its original closure operand",
            ))?;
            if !copy_local(&issue.arguments[0].node, Local::from_u32(1))
                || !returning(body, drop.target)
            {
                return Err(Error::Body(
                    "with_phase owner/closure/return transport changed",
                ));
            }
            let tuple_block = &body.basic_blocks[issue.target];
            let result_block = &body.basic_blocks[invoke.target];
            let [tuple_statement] = tuple_block.statements.as_slice() else {
                return Err(Error::Body("with_phase argument tuple changed"));
            };
            let StatementKind::Assign(tuple) = &tuple_statement.kind else {
                return Err(Error::Body("with_phase tuple is not assigned"));
            };
            let Rvalue::Aggregate(kind, args) = &tuple.1 else {
                return Err(Error::Body("with_phase tuple is not an aggregate"));
            };
            if !matches!(**kind, mir::AggregateKind::Tuple)
                || !tuple.0.projection.is_empty()
                || args.len() != 1
                || !move_local(&args[rustc_abi::FieldIdx::from_usize(0)], issue.destination.local)
                || !move_local(&invoke.arguments[1].node, tuple.0.local)
            {
                return Err(Error::Body("with_phase tuple lost the actual issue result"));
            }
            let [result_statement] = result_block.statements.as_slice() else {
                return Err(Error::Body("with_phase scalar result relay changed"));
            };
            let StatementKind::Assign(result) = &result_statement.kind else {
                return Err(Error::Body("with_phase result is not assigned"));
            };
            let Rvalue::Use(Operand::Move(place)) = &result.1 else {
                return Err(Error::Body("with_phase result is not retained field1"));
            };
            if !whole(result.0, mir::RETURN_PLACE)
                || place.local != invoke.destination.local
                || !matches!(place.projection.as_slice(), [mir::ProjectionElem::Field(field, _)] if field.as_u32() == 1)
                || local_type(tcx, instance, body, drop.destination.local)? != tcx.types.unit
            {
                return Err(Error::Body("with_phase result/drop identity changed"));
            }
            let result_pair = local_type(tcx, instance, body, invoke.destination.local)?;
            let TyKind::Tuple(fields) = result_pair.kind() else {
                return Err(Error::Body("with_phase closure did not return a pair"));
            };
            if fields.len() != 2 || fields[1] != local_type(tcx, instance, body, mir::RETURN_PLACE)?
            {
                return Err(Error::Body("with_phase pair result type changed"));
            }
            let exact_drop = match &drop.arguments[0].node {
                Operand::Move(place) => {
                    place.local == invoke.destination.local
                        && matches!(place.projection.as_slice(), [mir::ProjectionElem::Field(field, _)] if field.as_u32() == 0)
                }
                constant @ Operand::Constant(_) => zst_operand(tcx, instance, constant, fields[0])?,
                Operand::Copy(_) => false,
                _ => false,
            };
            if !exact_drop {
                return Err(Error::Body("with_phase drop lost completion field0"));
            }
            let temporaries = [
                issue.destination.local,
                tuple.0.local,
                invoke.destination.local,
                drop.destination.local,
            ];
            for (index, local) in temporaries.iter().enumerate() {
                charge(work, index + 1)?;
                if local.index() < 3 || temporaries[..index].contains(local) {
                    return Err(Error::Body(
                        "with_phase substituted an original temporary role",
                    ));
                }
            }
            chain(
                body,
                &[mir::START_BLOCK, issue.target, invoke.target, drop.target],
                work,
            )?;
            Ok(BodyRecipe::WithPhase {
                issue: issue.instance,
                invoke: invoke.instance,
                drop: drop.instance,
                issue_block: mir::START_BLOCK,
                invoke_block: issue.target,
                drop_block: invoke.target,
                return_block: drop.target,
                phase: issue.destination.local,
                tuple: tuple.0.local,
                result_pair: invoke.destination.local,
                drop_result: drop.destination.local,
                closure_operand,
            })
        }
    }
}
