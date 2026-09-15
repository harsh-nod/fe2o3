//! The retained-call form of the same closed masked-shift proof. Every block
//! is matched, including constant-overflow assertions and the failure branch.

use super::*;
use rustc_middle::mir::{AssertMessage, MirPhase, ProjectionElem, RuntimePhase, UnOp};

pub(super) fn check<'tcx, 'a>(
    tcx: TyCtxt<'tcx>,
    body: &Body<'tcx>,
    helper: Helper,
    fetch: &impl Fn(Instance<'tcx>) -> &'a Body<'tcx>,
    budget: &mut usize,
) -> Option<bool>
where
    'tcx: 'a,
{
    match helper {
        Helper::Wrapping if body.source_scopes.len() == 1 => {
            Some(wrapping(tcx, body, fetch, budget))
        }
        Helper::Unchecked if body.source_scopes.len() == 1 => {
            Some(unchecked(tcx, body, fetch, budget))
        }
        Helper::LanguageUb if body.source_scopes.len() == 1 => {
            Some(language_ub(tcx, body, fetch, budget))
        }
        Helper::Runtime
            if matches!(
                body.basic_blocks
                    .raw
                    .first()?
                    .statements
                    .first()
                    .and_then(|statement| assignment(statement, 0)),
                Some(Rvalue::UnaryOp(..))
            ) =>
        {
            Some(
                shape(body, 0, &[tcx.types.bool], &[1], 1)
                    && no_required_consts(body)
                    && matches!(assignment(&bb(body, 0).statements[0], 0),
                    Some(Rvalue::UnaryOp(UnOp::Not, operand))
                        if scalar(tcx, operand, tcx.types.bool) == Some(0))
                    && returns(body, 0),
            )
        }
        Helper::Precondition if body.source_scopes.len() == 2 => {
            Some(formatting::precondition(tcx, body, fetch, budget))
        }
        _ => None,
    }
}

fn wrapping<'tcx, 'a>(
    tcx: TyCtxt<'tcx>,
    body: &Body<'tcx>,
    fetch: &impl Fn(Instance<'tcx>) -> &'a Body<'tcx>,
    budget: &mut usize,
) -> bool
where
    'tcx: 'a,
{
    if !shape(
        body,
        2,
        &[
            tcx.types.u32,
            tcx.types.u32,
            tcx.types.u32,
            tcx.types.u32,
            tcx.types.u32,
            Ty::new_tup(tcx, &[tcx.types.u32, tcx.types.bool]),
        ],
        &[1, 2, 0],
        1,
    ) {
        return false;
    }
    let Some(Rvalue::BinaryOp(BinOp::SubWithOverflow, operands)) =
        assignment(&bb(body, 0).statements[0], 5)
    else {
        return false;
    };
    if !bits(tcx, &operands.0)
        || scalar(tcx, &operands.1, tcx.types.u32) != Some(1)
        || !required_bits(tcx, body, 2)
    {
        return false;
    }
    let TerminatorKind::Assert {
        cond,
        expected,
        msg,
        target,
        unwind,
    } = &bb(body, 0).terminator().kind
    else {
        return false;
    };
    // BITS is authenticated and evaluated to 32, so 32 - 1 cannot overflow.
    // The original assertion, including its failure operands, must still match.
    if *expected
        || !moved_field(cond, 5, 1, tcx.types.bool)
        || target.as_usize() != 1
        || !matches!(unwind, UnwindAction::Unreachable)
        || !matches!(&**msg, AssertMessage::Overflow(BinOp::Sub, left, right)
            if left == &operands.0 && right == &operands.1)
        || !matches!(assignment(&bb(body, 1).statements[0], 4), Some(Rvalue::Use(operand))
            if moved_field(operand, 5, 0, tcx.types.u32))
        || !binary(body, 1, 1, 3, BinOp::BitAnd, 2, false, 4, true)
        || !returns(body, 2)
    {
        return false;
    }
    // Only this safe entry establishes count = rhs & 31. Neither unchecked_shr
    // nor its precondition is a public semantic terminal in its own right.
    call(
        tcx,
        body,
        1,
        0,
        2,
        &[(1, false), (3, true)],
        Helper::Unchecked,
        fetch,
        budget,
    )
}

fn unchecked<'tcx, 'a>(
    tcx: TyCtxt<'tcx>,
    body: &Body<'tcx>,
    fetch: &impl Fn(Instance<'tcx>) -> &'a Body<'tcx>,
    budget: &mut usize,
) -> bool
where
    'tcx: 'a,
{
    shape(
        body,
        2,
        &[
            tcx.types.u32,
            tcx.types.u32,
            tcx.types.u32,
            tcx.types.bool,
            tcx.types.unit,
        ],
        &[0, 0, 0, 1],
        1,
    ) && no_required_consts(body)
        && switch_local(body, 1, 3, 3, 2)
        && binary(body, 3, 0, 0, BinOp::ShrUnchecked, 1, false, 2, false)
        && returns(body, 3)
        && call(tcx, body, 0, 3, 1, &[], Helper::LanguageUb, fetch, budget)
        && call(
            tcx,
            body,
            2,
            4,
            3,
            &[(2, false)],
            Helper::Precondition,
            fetch,
            budget,
        )
}

fn language_ub<'tcx, 'a>(
    tcx: TyCtxt<'tcx>,
    body: &Body<'tcx>,
    fetch: &impl Fn(Instance<'tcx>) -> &'a Body<'tcx>,
    budget: &mut usize,
) -> bool
where
    'tcx: 'a,
{
    shape(
        body,
        0,
        &[tcx.types.bool, tcx.types.bool],
        &[0, 0, 1, 1, 0],
        1,
    ) && no_required_consts(body)
        && switch_local(body, 1, 1, 3, 2)
        && matches!(assignment(&bb(body, 2).statements[0], 0), Some(Rvalue::Use(operand)) if ub_checks(operand))
        && matches!(assignment(&bb(body, 3).statements[0], 0), Some(Rvalue::Use(operand))
            if scalar(tcx, operand, tcx.types.bool) == Some(0))
        && goto(body, 2, 4)
        && goto(body, 3, 4)
        && returns(body, 4)
        && call(tcx, body, 0, 1, 1, &[], Helper::Runtime, fetch, budget)
}

pub(super) fn shape<'tcx>(
    body: &Body<'tcx>,
    args: usize,
    types: &[Ty<'tcx>],
    statements: &[usize],
    scopes: usize,
) -> bool {
    body.phase == MirPhase::Runtime(RuntimePhase::Optimized)
        && body.arg_count == args
        && body
            .local_decls
            .iter()
            .map(|decl| decl.ty)
            .eq(types.iter().copied())
        && body.basic_blocks.len() == statements.len()
        && body
            .basic_blocks
            .iter()
            .zip(statements)
            .all(|(block, count)| block.statements.len() == *count)
        && body.source_scopes.len() == scopes
        && body.source_scopes.iter().enumerate().all(|(index, scope)| {
            scope.inlined.is_none()
                && scope.inlined_parent_scope.is_none()
                && scope.parent_scope == (index != 0).then(|| SourceScope::from_usize(0))
        })
        && body
            .local_decls
            .iter()
            .all(|decl| decl.source_info.scope.as_usize() < scopes)
        && body.basic_blocks.iter().all(|block| {
            block.terminator().source_info.scope.as_usize() < scopes
                && block
                    .statements
                    .iter()
                    .all(|statement| statement.source_info.scope.as_usize() < scopes)
        })
}

pub(super) fn bb<'a, 'tcx>(body: &'a Body<'tcx>, index: usize) -> &'a BasicBlockData<'tcx> {
    &body.basic_blocks[BasicBlock::from_usize(index)]
}

pub(super) fn operand(operand: &Operand<'_>, index: usize, moved: bool) -> bool {
    match operand {
        Operand::Copy(place) if !moved => local(*place, index),
        Operand::Move(place) if moved => local(*place, index),
        _ => false,
    }
}

fn moved_field<'tcx>(operand: &Operand<'tcx>, index: usize, field: usize, ty: Ty<'tcx>) -> bool {
    matches!(operand, Operand::Move(place) if place.local.as_usize() == index
        && matches!(place.projection.as_ref(), [ProjectionElem::Field(actual, actual_ty)]
            if actual.as_usize() == field && *actual_ty == ty))
}

#[allow(clippy::too_many_arguments)]
pub(super) fn binary(
    body: &Body<'_>,
    block: usize,
    statement: usize,
    dest: usize,
    op: BinOp,
    left: usize,
    left_moved: bool,
    right: usize,
    right_moved: bool,
) -> bool {
    matches!(assignment(&bb(body, block).statements[statement], dest), Some(Rvalue::BinaryOp(actual, operands))
        if *actual == op && operand(&operands.0, left, left_moved) && operand(&operands.1, right, right_moved))
}

pub(super) fn returns(body: &Body<'_>, block: usize) -> bool {
    matches!(bb(body, block).terminator().kind, TerminatorKind::Return)
}

fn goto(body: &Body<'_>, block: usize, expected: usize) -> bool {
    matches!(bb(body, block).terminator().kind, TerminatorKind::Goto { target } if target.as_usize() == expected)
}

pub(super) fn switch_local(
    body: &Body<'_>,
    block: usize,
    local: usize,
    zero: usize,
    otherwise: usize,
) -> bool {
    matches!(&bb(body, block).terminator().kind, TerminatorKind::SwitchInt { discr, targets }
        if operand(discr, local, true) && switch(targets, zero, otherwise))
}

#[allow(clippy::too_many_arguments)]
pub(super) fn call<'tcx, 'a>(
    tcx: TyCtxt<'tcx>,
    body: &Body<'tcx>,
    block: usize,
    dest: usize,
    next: usize,
    operands: &[(usize, bool)],
    helper: Helper,
    fetch: &impl Fn(Instance<'tcx>) -> &'a Body<'tcx>,
    budget: &mut usize,
) -> bool
where
    'tcx: 'a,
{
    let TerminatorKind::Call {
        func,
        args,
        destination,
        target,
        unwind,
        ..
    } = &bb(body, block).terminator().kind
    else {
        return false;
    };
    let Some(callee) = resolve(tcx, func) else {
        return false;
    };
    args.len() == operands.len()
        && args
            .iter()
            .zip(operands)
            .all(|(arg, &(index, moved))| operand(&arg.node, index, moved))
        && local(*destination, dest)
        && target.map(|block| block.as_usize()) == Some(next)
        && matches!(unwind, UnwindAction::Unreachable)
        && check_instance(tcx, callee, helper, fetch, budget)
}

pub(super) fn bits<'tcx>(tcx: TyCtxt<'tcx>, operand: &Operand<'tcx>) -> bool {
    let Operand::Constant(constant) = operand else {
        return false;
    };
    bits_const(tcx, constant.const_)
}

fn bits_const<'tcx>(tcx: TyCtxt<'tcx>, constant: Const<'tcx>) -> bool {
    let Const::Unevaluated(value, ty) = constant else {
        return false;
    };
    let Some(core) = tcx.lang_items().sized_trait() else {
        return false;
    };
    let Some(implementation) = tcx.impl_of_assoc(value.def) else {
        return false;
    };
    ty == tcx.types.u32 && value.def.krate == core.krate
        && tcx.crate_name(core.krate).as_str() == "core"
        && matches!(tcx.def_kind(value.def), DefKind::AssocConst { is_type_const: false })
        && value.args.is_empty() && value.promoted.is_none()
        && tcx.item_name(value.def).as_str() == "BITS"
        && implementation.krate == core.krate && !tcx.impl_is_of_trait(implementation)
        && tcx.type_of(implementation).instantiate_identity() == tcx.types.u32
        && tcx.type_of(value.def).instantiate_identity() == tcx.types.u32
        // This is exact CTFE of an authenticated constant, not execution of a
        // test input or admission of any const-evaluator callee as device MIR.
        && constant.try_eval_bits(tcx, TypingEnv::fully_monomorphized()) == Some(32)
}

pub(super) fn required_bits<'tcx>(tcx: TyCtxt<'tcx>, body: &Body<'tcx>, count: usize) -> bool {
    body.required_consts.as_ref().is_some_and(|constants| {
        constants.len() == count
            && constants
                .iter()
                .all(|constant| bits_const(tcx, constant.const_))
    })
}

pub(super) fn no_required_consts(body: &Body<'_>) -> bool {
    body.required_consts
        .as_ref()
        .is_some_and(|constants| constants.is_empty())
}
