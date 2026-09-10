//! Closed source-safety authentication for `usize::checked_mul` and its two
//! retained core helpers. This is not a terminal: their MIR is still collected
//! and lowered independently, including the existing `cold_path` expansion.

use super::{
    BinOp, Body, ExternAbi, Instance, InstanceKind, Operand, Rvalue, Safety, StatementKind,
    TerminatorKind, Ty, TyCtxt, TyKind, TypingEnv, UnwindAction,
};
use rustc_hir::def::DefKind;
use rustc_middle::mir::{
    AggregateKind, BasicBlock, CastKind, Const, Place, ProjectionElem, RETURN_PLACE,
};
use rustc_middle::ty::{ConstKind, FnSig};

const MAX_LOCALS: usize = 16;
const MAX_BLOCKS: usize = 8;
const MAX_STATEMENTS: usize = 64;
const MAX_SCOPES: usize = 8;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Helper {
    CheckedMul,
    OverflowingMul,
    Unlikely,
}

impl Helper {
    fn may_call(self, callee: Self) -> bool {
        self == Self::CheckedMul && matches!(callee, Self::OverflowingMul | Self::Unlikely)
    }
}

/// Discharges only the unavailable external HIR observation, never an unsafe
/// call or downstream MIR/effect/lowering obligation.
pub(crate) fn authenticate_reviewed_safe_core_checked_mul_helper_v1<'tcx>(
    tcx: TyCtxt<'tcx>,
    instance: Instance<'tcx>,
) -> bool {
    let Some(helper) = helper_identity(tcx, instance) else {
        return false;
    };
    reviewed_body(tcx, tcx.instance_mir(instance.def), helper)
}

fn helper_identity<'tcx>(tcx: TyCtxt<'tcx>, instance: Instance<'tcx>) -> Option<Helper> {
    let core = tcx.lang_items().sized_trait()?.krate;
    let definition = instance.def_id();
    if !matches!(instance.def, InstanceKind::Item(_))
        || definition.krate != core
        || tcx.crate_name(core).as_str() != "core"
        || !instance.args.is_empty()
        || !matches!(tcx.def_kind(definition), DefKind::Fn | DefKind::AssocFn)
        || !tcx.is_mir_available(definition)
    {
        return None;
    }
    let helper = match tcx.item_name(definition).as_str() {
        "checked_mul" => Helper::CheckedMul,
        "overflowing_mul" => Helper::OverflowingMul,
        "unlikely" => Helper::Unlikely,
        _ => return None,
    };
    let signature = tcx.instantiate_bound_regions_with_erased(
        tcx.fn_sig(definition).instantiate(tcx, instance.args),
    );
    if !signature_matches(tcx, helper, signature) {
        return None;
    }
    if helper == Helper::Unlikely {
        return (tcx.def_kind(definition) == DefKind::Fn
            && tcx.def_path_str(definition) == "core::intrinsics::unlikely")
            .then_some(helper);
    }
    let associated = tcx.opt_associated_item(definition)?;
    let implementation = tcx.impl_of_assoc(definition)?;
    (associated.is_fn()
        && implementation.krate == core
        && !tcx.impl_is_of_trait(implementation)
        && tcx.type_of(implementation).instantiate_identity() == tcx.types.usize)
        .then_some(helper)
}

fn signature_matches<'tcx>(tcx: TyCtxt<'tcx>, helper: Helper, signature: FnSig<'tcx>) -> bool {
    signature.safety == Safety::Safe
        && signature.abi == ExternAbi::Rust
        && !signature.c_variadic
        && match helper {
            Helper::CheckedMul => {
                signature.inputs() == [tcx.types.usize, tcx.types.usize]
                    && is_option_usize(tcx, signature.output())
            }
            Helper::OverflowingMul => {
                signature.inputs() == [tcx.types.usize, tcx.types.usize]
                    && signature.output() == Ty::new_tup(tcx, &[tcx.types.usize, tcx.types.bool])
            }
            Helper::Unlikely => {
                signature.inputs() == [tcx.types.bool] && signature.output() == tcx.types.bool
            }
        }
}

fn is_option_usize<'tcx>(tcx: TyCtxt<'tcx>, ty: Ty<'tcx>) -> bool {
    matches!(ty.kind(), TyKind::Adt(adt, args)
        if Some(adt.did()) == tcx.lang_items().option_type()
            && args.as_slice().len() == 1
            && args.as_slice()[0].as_type() == Some(tcx.types.usize))
}

fn is_word<'tcx>(tcx: TyCtxt<'tcx>, ty: Ty<'tcx>) -> bool {
    let TyKind::Uint(integer) = ty.kind() else {
        return false;
    };
    integer
        .bit_width()
        .is_none_or(|width| width == tcx.data_layout.pointer_size().bits())
}

fn is_pair<'tcx>(tcx: TyCtxt<'tcx>, ty: Ty<'tcx>) -> bool {
    matches!(ty.kind(), TyKind::Tuple(fields)
        if matches!(fields.as_slice(), [word, flag] if is_word(tcx, *word) && *flag == tcx.types.bool))
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Value {
    Left,
    Right,
    Product,
    OverflowPair,
    Bool(bool),
    SomeProduct,
    None,
    Unit,
}

fn value_has_type<'tcx>(tcx: TyCtxt<'tcx>, value: Value, ty: Ty<'tcx>) -> bool {
    match value {
        Value::Left | Value::Right | Value::Product => is_word(tcx, ty),
        Value::OverflowPair => is_pair(tcx, ty),
        Value::Bool(_) => ty == tcx.types.bool,
        Value::SomeProduct | Value::None => is_option_usize(tcx, ty),
        Value::Unit => ty == tcx.types.unit,
    }
}

fn reviewed_body<'tcx>(tcx: TyCtxt<'tcx>, body: &Body<'tcx>, helper: Helper) -> bool {
    let arguments = if helper == Helper::Unlikely { 1 } else { 2 };
    if body.arg_count != arguments
        || !(arguments + 1..=MAX_LOCALS).contains(&body.local_decls.len())
        || !(1..=MAX_BLOCKS).contains(&body.basic_blocks.len())
        || !(1..=MAX_SCOPES).contains(&body.source_scopes.len())
        || body.basic_blocks.iter().any(|block| block.is_cleanup)
        || body
            .basic_blocks
            .iter()
            .map(|block| block.statements.len())
            .sum::<usize>()
            > MAX_STATEMENTS
        || body.local_decls.iter().any(|local| {
            !is_word(tcx, local.ty)
                && local.ty != tcx.types.bool
                && local.ty != tcx.types.unit
                && !is_pair(tcx, local.ty)
                && !is_option_usize(tcx, local.ty)
        })
        || body.source_scopes.iter().any(|scope| {
            scope.inlined.is_some_and(|(instance, _)| {
                !helper_identity(tcx, instance).is_some_and(|callee| helper.may_call(callee))
                    || !authenticate_reviewed_safe_core_checked_mul_helper_v1(tcx, instance)
            })
        })
    {
        return false;
    }
    let expected_output = match helper {
        Helper::CheckedMul => is_option_usize(tcx, body.local_decls[RETURN_PLACE].ty),
        Helper::OverflowingMul => {
            body.local_decls[RETURN_PLACE].ty
                == Ty::new_tup(tcx, &[tcx.types.usize, tcx.types.bool])
        }
        Helper::Unlikely => body.local_decls[RETURN_PLACE].ty == tcx.types.bool,
    };
    let input = if helper == Helper::Unlikely {
        tcx.types.bool
    } else {
        tcx.types.usize
    };
    if !expected_output
        || body
            .args_iter()
            .any(|local| body.local_decls[local].ty != input)
    {
        return false;
    }
    let mut covered = [false; MAX_BLOCKS];
    // The only symbolic branch is the exact overflow bit. Check both values;
    // no concrete operand samples or unproved arithmetic identities are used.
    for overflow in [false, true] {
        let expected = match helper {
            Helper::CheckedMul if overflow => Value::None,
            Helper::CheckedMul => Value::SomeProduct,
            Helper::OverflowingMul => Value::OverflowPair,
            Helper::Unlikely => Value::Bool(overflow),
        };
        if evaluate(tcx, body, helper, overflow, &mut covered) != Some(expected) {
            return false;
        }
    }
    covered[..body.basic_blocks.len()]
        .iter()
        .all(|covered| *covered)
}

fn evaluate<'tcx>(
    tcx: TyCtxt<'tcx>,
    body: &Body<'tcx>,
    helper: Helper,
    overflow: bool,
    covered: &mut [bool; MAX_BLOCKS],
) -> Option<Value> {
    let mut locals = [None; MAX_LOCALS];
    locals[1] = Some(if helper == Helper::Unlikely {
        Value::Bool(overflow)
    } else {
        Value::Left
    });
    if helper != Helper::Unlikely {
        locals[2] = Some(Value::Right);
    }
    let mut visited = [false; MAX_BLOCKS];
    let mut next = BasicBlock::from_usize(0);
    loop {
        let block = body.basic_blocks.get(next)?;
        if std::mem::replace(&mut visited[next.as_usize()], true) {
            return None;
        }
        covered[next.as_usize()] = true;
        for statement in &block.statements {
            match &statement.kind {
                StatementKind::Assign(assignment) => {
                    let (destination, rvalue) = &**assignment;
                    let value = read_rvalue(tcx, body, &locals, overflow, rvalue)?;
                    write_local(tcx, body, &mut locals, *destination, value)?;
                }
                StatementKind::StorageLive(local) | StatementKind::StorageDead(local)
                    if local.as_usize() > body.arg_count
                        && local.as_usize() < body.local_decls.len() =>
                {
                    locals[local.as_usize()] = None;
                }
                _ => return None,
            }
        }
        next = match &block.terminator.as_ref()?.kind {
            TerminatorKind::Return => return locals[0],
            TerminatorKind::Goto { target } => *target,
            TerminatorKind::SwitchInt { discr, targets } => {
                let Value::Bool(flag) = read_operand(tcx, body, &locals, overflow, discr)? else {
                    return None;
                };
                if targets.all_targets().len() > 3 || targets.iter().any(|(value, _)| value > 1) {
                    return None;
                }
                targets.target_for_value(u128::from(flag))
            }
            TerminatorKind::Call {
                func,
                args,
                destination,
                target,
                unwind,
                ..
            } if matches!(unwind, UnwindAction::Unreachable | UnwindAction::Continue) => {
                let callee = resolve_callee(tcx, func)?;
                let value = if helper != Helper::OverflowingMul && is_cold_path(tcx, callee) {
                    if !args.is_empty() || !matches!(unwind, UnwindAction::Unreachable) {
                        return None;
                    }
                    Value::Unit
                } else {
                    let kind = helper_identity(tcx, callee)?;
                    if !helper.may_call(kind)
                        || !authenticate_reviewed_safe_core_checked_mul_helper_v1(tcx, callee)
                    {
                        return None;
                    }
                    match (kind, &args[..]) {
                        (Helper::OverflowingMul, [left, right])
                            if read_operand(tcx, body, &locals, overflow, &left.node)
                                == Some(Value::Left)
                                && read_operand(tcx, body, &locals, overflow, &right.node)
                                    == Some(Value::Right) =>
                        {
                            Value::OverflowPair
                        }
                        (Helper::Unlikely, [flag]) => {
                            let value = read_operand(tcx, body, &locals, overflow, &flag.node)?;
                            if !matches!(value, Value::Bool(_)) {
                                return None;
                            }
                            value
                        }
                        _ => return None,
                    }
                };
                write_local(tcx, body, &mut locals, *destination, value)?;
                (*target)?
            }
            _ => return None,
        };
    }
}

fn write_local<'tcx>(
    tcx: TyCtxt<'tcx>,
    body: &Body<'tcx>,
    locals: &mut [Option<Value>; MAX_LOCALS],
    place: Place<'tcx>,
    value: Value,
) -> Option<()> {
    if !place.projection.is_empty()
        || !value_has_type(tcx, value, body.local_decls.get(place.local)?.ty)
    {
        return None;
    }
    *locals.get_mut(place.local.as_usize())? = Some(value);
    Some(())
}

fn read_operand<'tcx>(
    tcx: TyCtxt<'tcx>,
    body: &Body<'tcx>,
    locals: &[Option<Value>; MAX_LOCALS],
    overflow: bool,
    operand: &Operand<'tcx>,
) -> Option<Value> {
    match operand {
        Operand::Copy(place) | Operand::Move(place) => {
            let value = (*locals.get(place.local.as_usize())?)?;
            match place.projection.as_ref() {
                [] => Some(value),
                [ProjectionElem::Field(field, ty)] if value == Value::OverflowPair => {
                    let TyKind::Tuple(fields) = body.local_decls.get(place.local)?.ty.kind() else {
                        return None;
                    };
                    if fields.get(field.as_usize()) != Some(ty) {
                        return None;
                    }
                    match field.as_usize() {
                        0 if is_word(tcx, *ty) => Some(Value::Product),
                        1 if *ty == tcx.types.bool => Some(Value::Bool(overflow)),
                        _ => None,
                    }
                }
                _ => None,
            }
        }
        Operand::Constant(constant) => {
            // Only already evaluated scalar/ADT values; never execute an
            // arbitrary unevaluated constant while authenticating a helper.
            let value = match constant.const_ {
                Const::Val(value, _) => value,
                Const::Ty(_, constant) => match constant.kind() {
                    ConstKind::Value(value) => tcx.valtree_to_const_val(value),
                    _ => return None,
                },
                _ => return None,
            };
            let ty = constant.const_.ty();
            if ty == tcx.types.bool {
                return value.try_to_scalar_int()?.try_into().ok().map(Value::Bool);
            }
            if !is_option_usize(tcx, ty) {
                return None;
            }
            let contents = tcx.try_destructure_mir_constant_for_user_output(value, ty)?;
            let TyKind::Adt(adt, _) = ty.kind() else {
                return None;
            };
            (contents.fields.is_empty()
                && adt.variants().get(contents.variant?)?.def_id
                    == tcx.lang_items().option_none_variant()?)
            .then_some(Value::None)
        }
        _ => None,
    }
}

fn read_rvalue<'tcx>(
    tcx: TyCtxt<'tcx>,
    body: &Body<'tcx>,
    locals: &[Option<Value>; MAX_LOCALS],
    overflow: bool,
    rvalue: &Rvalue<'tcx>,
) -> Option<Value> {
    let read = |operand| read_operand(tcx, body, locals, overflow, operand);
    match rvalue {
        Rvalue::Use(operand) => read(operand),
        Rvalue::Cast(CastKind::IntToInt, operand, target)
            if is_word(tcx, operand.ty(&body.local_decls, tcx)) && is_word(tcx, *target) =>
        {
            read(operand)
        }
        Rvalue::BinaryOp(BinOp::MulWithOverflow, operands)
            if read(&operands.0) == Some(Value::Left)
                && read(&operands.1) == Some(Value::Right) =>
        {
            Some(Value::OverflowPair)
        }
        Rvalue::Aggregate(kind, operands) => match (&**kind, operands.raw.as_slice()) {
            (AggregateKind::Tuple, [product, flag])
                if read(product) == Some(Value::Product)
                    && read(flag) == Some(Value::Bool(overflow)) =>
            {
                Some(Value::OverflowPair)
            }
            (AggregateKind::Adt(adt, variant, args, user_ty, field), operands)
                if Some(*adt) == tcx.lang_items().option_type()
                    && args.as_slice().len() == 1
                    && args.as_slice()[0].as_type() == Some(tcx.types.usize)
                    && user_ty.is_none()
                    && field.is_none() =>
            {
                let variant = tcx.adt_def(*adt).variants().get(*variant)?.def_id;
                match operands {
                    [] if Some(variant) == tcx.lang_items().option_none_variant() => {
                        Some(Value::None)
                    }
                    [product]
                        if Some(variant) == tcx.lang_items().option_some_variant()
                            && read(product) == Some(Value::Product) =>
                    {
                        Some(Value::SomeProduct)
                    }
                    _ => None,
                }
            }
            _ => None,
        },
        _ => None,
    }
}

fn resolve_callee<'tcx>(tcx: TyCtxt<'tcx>, operand: &Operand<'tcx>) -> Option<Instance<'tcx>> {
    let Operand::Constant(callee) = operand else {
        return None;
    };
    let TyKind::FnDef(definition, arguments) = callee.const_.ty().kind() else {
        return None;
    };
    Instance::try_resolve(
        tcx,
        TypingEnv::fully_monomorphized(),
        *definition,
        arguments,
    )
    .ok()
    .flatten()
}

fn is_cold_path<'tcx>(tcx: TyCtxt<'tcx>, instance: Instance<'tcx>) -> bool {
    let Some(core) = tcx.lang_items().sized_trait() else {
        return false;
    };
    let InstanceKind::Intrinsic(definition) = instance.def else {
        return false;
    };
    if definition.krate != core.krate
        || !instance.args.is_empty()
        || tcx
            .intrinsic(definition)
            .is_none_or(|intrinsic| intrinsic.name.as_str() != "cold_path")
    {
        return false;
    }
    let signature = tcx.instantiate_bound_regions_with_erased(
        tcx.fn_sig(definition).instantiate(tcx, instance.args),
    );
    signature.safety == Safety::Safe
        && signature.abi == ExternAbi::Rust
        && !signature.c_variadic
        && signature.inputs().is_empty()
        && signature.output() == tcx.types.unit
}

#[cfg(test)]
#[path = "core_checked_mul_v1_tests.rs"]
mod tests;
