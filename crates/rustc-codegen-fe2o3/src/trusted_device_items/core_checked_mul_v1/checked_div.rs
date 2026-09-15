//! Source-only authentication of unsigned checked division. Both symbolic
//! divisor partitions must retain the exact zero test and quotient operation.
//! This does not replace MIR, authenticate user code, or discharge lowering.

use super::*;
use rustc_middle::mir::{MirPhase, RuntimePhase};

/// Discharges only the unavailable external HIR observation. The original
/// body and its recursively admitted callees remain in the execution closure.
pub(crate) fn authenticate_reviewed_safe_core_checked_div_v1<'tcx>(
    tcx: TyCtxt<'tcx>,
    instance: Instance<'tcx>,
) -> bool {
    let Some(word) = identity(tcx, instance) else {
        return false;
    };
    reviewed_body(tcx, instance, word, tcx.instance_mir(instance.def))
}

fn identity<'tcx>(tcx: TyCtxt<'tcx>, instance: Instance<'tcx>) -> Option<Ty<'tcx>> {
    let core = tcx.lang_items().sized_trait()?.krate;
    let definition = instance.def_id();
    if !matches!(instance.def, InstanceKind::Item(_))
        || definition.krate != core
        || tcx.crate_name(core).as_str() != "core"
        || !instance.args.is_empty()
        || tcx.def_kind(definition) != DefKind::AssocFn
        || tcx.item_name(definition).as_str() != "checked_div"
        || !tcx.is_mir_available(definition)
    {
        return None;
    }
    let implementation = tcx.impl_of_assoc(definition)?;
    let word = tcx.type_of(implementation).instantiate_identity();
    if implementation.krate != core
        || tcx.impl_is_of_trait(implementation)
        || !tcx.opt_associated_item(definition)?.is_fn()
        || !matches!(word.kind(), TyKind::Uint(_))
    {
        return None;
    }
    let signature = tcx.instantiate_bound_regions_with_erased(
        tcx.fn_sig(definition).instantiate(tcx, instance.args),
    );
    (signature.safety == Safety::Safe
        && signature.abi == ExternAbi::Rust
        && !signature.c_variadic
        && signature.inputs() == [word, word]
        && option_type(tcx, word, signature.output()))
    .then_some(word)
}

fn option_type<'tcx>(tcx: TyCtxt<'tcx>, word: Ty<'tcx>, ty: Ty<'tcx>) -> bool {
    matches!(ty.kind(), TyKind::Adt(adt, args)
        if Some(adt.did()) == tcx.lang_items().option_type()
            && matches!(args.as_slice(), [argument] if argument.as_type() == Some(word)))
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Symbol {
    Left,
    Right,
    Zero,
    DivisorZero,
    Bool(bool),
    Quotient,
    None,
    SomeQuotient,
    Unit,
}

impl Symbol {
    fn has_type<'tcx>(self, tcx: TyCtxt<'tcx>, word: Ty<'tcx>, ty: Ty<'tcx>) -> bool {
        match self {
            Self::Left | Self::Right | Self::Zero | Self::Quotient => ty == word,
            Self::DivisorZero | Self::Bool(_) => ty == tcx.types.bool,
            Self::None | Self::SomeQuotient => option_type(tcx, word, ty),
            Self::Unit => ty == tcx.types.unit,
        }
    }
}

struct State {
    locals: LocalState<Symbol>,
    divisor_branch: bool,
    divided: bool,
}

fn reviewed_body<'tcx>(
    tcx: TyCtxt<'tcx>,
    instance: Instance<'tcx>,
    word: Ty<'tcx>,
    body: &Body<'tcx>,
) -> bool {
    if identity(tcx, instance) != Some(word)
        || body.source.instance != instance.def
        || body.phase != MirPhase::Runtime(RuntimePhase::Optimized)
        || body.arg_count != 2
        || body.spread_arg.is_some()
        || body.coroutine.is_some()
        || body.source.promoted.is_some()
        || body.is_polymorphic
        || body.injection_phase.is_some()
        || body.tainted_by_errors.is_some()
        || body
            .required_consts
            .as_ref()
            .is_none_or(|items| !items.is_empty())
        || !body.user_type_annotations.is_empty()
        || body.var_debug_info.len() > MAX_LOCALS
        || !(3..=MAX_LOCALS).contains(&body.local_decls.len())
        || !(1..=MAX_BLOCKS).contains(&body.basic_blocks.len())
        || !(1..=MAX_SCOPES).contains(&body.source_scopes.len())
        || body.basic_blocks.iter().any(|block| block.is_cleanup)
        || body
            .basic_blocks
            .iter()
            .map(|block| block.statements.len())
            .sum::<usize>()
            > MAX_STATEMENTS
        || !option_type(tcx, word, body.local_decls[RETURN_PLACE].ty)
        || body
            .args_iter()
            .any(|local| body.local_decls[local].ty != word)
        || body.local_decls.iter().any(|decl| {
            decl.source_info.scope.as_usize() >= body.source_scopes.len()
                || decl.user_ty.is_some()
                || (decl.ty != word
                    && decl.ty != tcx.types.bool
                    && decl.ty != tcx.types.unit
                    && !option_type(tcx, word, decl.ty))
        })
        || body.source_scopes.iter().enumerate().any(|(index, scope)| {
            (index == 0 && scope.parent_scope.is_some())
                || (index != 0
                    && scope
                        .parent_scope
                        .is_none_or(|parent| parent.as_usize() >= index))
                || scope
                    .inlined_parent_scope
                    .is_some_and(|parent| parent.as_usize() >= index)
                || scope
                    .inlined
                    .is_some_and(|(callee, _)| !closed_hint(tcx, callee))
        })
        || body.basic_blocks.iter().any(|block| {
            block
                .statements
                .iter()
                .any(|statement| statement.source_info.scope.as_usize() >= body.source_scopes.len())
                || block.terminator.as_ref().is_none_or(|term| {
                    term.source_info.scope.as_usize() >= body.source_scopes.len()
                })
        })
    {
        return false;
    }
    let mut covered = [false; MAX_BLOCKS];
    // These are exhaustive symbolic partitions of the original divisor, not
    // sampled integers. The flag alone grants nothing without its real edge.
    for zero in [false, true] {
        if evaluate(tcx, word, body, zero, &mut covered)
            != Some(if zero {
                Symbol::None
            } else {
                Symbol::SomeQuotient
            })
        {
            return false;
        }
    }
    covered[..body.basic_blocks.len()].iter().all(|seen| *seen)
}

fn closed_hint<'tcx>(tcx: TyCtxt<'tcx>, instance: Instance<'tcx>) -> bool {
    helper_identity(tcx, instance) == Some(Helper::Unlikely)
        && authenticate_reviewed_safe_core_arithmetic_helper_v1(tcx, instance)
}

fn read<'tcx>(
    tcx: TyCtxt<'tcx>,
    word: Ty<'tcx>,
    body: &Body<'tcx>,
    state: &mut State,
    operand: &Operand<'tcx>,
) -> Option<Symbol> {
    match operand {
        Operand::Copy(place) | Operand::Move(place) if place.projection.is_empty() => {
            let value = state
                .locals
                .read(*place, matches!(operand, Operand::Move(_)))?;
            value
                .has_type(tcx, word, body.local_decls.get(place.local)?.ty)
                .then_some(value)
        }
        Operand::Constant(constant) if constant.user_ty.is_none() => {
            let value = match constant.const_ {
                Const::Val(value, _) => value,
                Const::Ty(declared, constant) => match constant.kind() {
                    ConstKind::Value(value) if value.ty == declared => {
                        tcx.valtree_to_const_val(value)
                    }
                    _ => return None,
                },
                _ => return None,
            };
            let ty = constant.const_.ty();
            if ty == word {
                let TyKind::Uint(integer) = word.kind() else {
                    return None;
                };
                let bits = integer
                    .bit_width()
                    .unwrap_or(tcx.data_layout.pointer_size().bits());
                return (value
                    .try_to_scalar_int()?
                    .try_to_bits(rustc_abi::Size::from_bits(bits))
                    .ok()?
                    == 0)
                    .then_some(Symbol::Zero);
            }
            if ty == tcx.types.bool {
                return value.try_to_scalar_int()?.try_into().ok().map(Symbol::Bool);
            }
            if ty == tcx.types.unit && value == ConstValue::ZeroSized {
                return Some(Symbol::Unit);
            }
            if !option_type(tcx, word, ty) {
                return None;
            }
            let contents = tcx.try_destructure_mir_constant_for_user_output(value, ty)?;
            let TyKind::Adt(adt, _) = ty.kind() else {
                return None;
            };
            (contents.fields.is_empty()
                && adt.variants().get(contents.variant?)?.def_id
                    == tcx.lang_items().option_none_variant()?)
            .then_some(Symbol::None)
        }
        _ => None,
    }
}

fn write<'tcx>(
    tcx: TyCtxt<'tcx>,
    word: Ty<'tcx>,
    body: &Body<'tcx>,
    state: &mut State,
    place: Place<'tcx>,
    value: Symbol,
) -> Option<()> {
    if !place.projection.is_empty()
        || !value.has_type(tcx, word, body.local_decls.get(place.local)?.ty)
    {
        return None;
    }
    state.locals.write(place, value)
}

fn rvalue<'tcx>(
    tcx: TyCtxt<'tcx>,
    word: Ty<'tcx>,
    body: &Body<'tcx>,
    state: &mut State,
    zero: bool,
    value: &Rvalue<'tcx>,
) -> Option<Symbol> {
    match value {
        Rvalue::Use(operand) => read(tcx, word, body, state, operand),
        Rvalue::BinaryOp(operation, operands) => {
            let left = read(tcx, word, body, state, &operands.0)?;
            let right = read(tcx, word, body, state, &operands.1)?;
            match (*operation, left, right) {
                (BinOp::Eq, Symbol::Right, Symbol::Zero) => Some(Symbol::DivisorZero),
                (BinOp::Div, Symbol::Left, Symbol::Right)
                    if !zero && state.divisor_branch && !state.divided =>
                {
                    state.divided = true;
                    Some(Symbol::Quotient)
                }
                _ => None,
            }
        }
        Rvalue::Aggregate(kind, operands) => {
            let AggregateKind::Adt(adt, variant, args, user_ty, field) = &**kind else {
                return None;
            };
            if Some(*adt) != tcx.lang_items().option_type()
                || !matches!(args.as_slice(), [argument] if argument.as_type() == Some(word))
                || user_ty.is_some()
                || field.is_some()
            {
                return None;
            }
            let variant = tcx.adt_def(*adt).variants().get(*variant)?.def_id;
            match operands.raw.as_slice() {
                [] if Some(variant) == tcx.lang_items().option_none_variant() => Some(Symbol::None),
                [result]
                    if Some(variant) == tcx.lang_items().option_some_variant()
                        && read(tcx, word, body, state, result) == Some(Symbol::Quotient) =>
                {
                    Some(Symbol::SomeQuotient)
                }
                _ => None,
            }
        }
        _ => None,
    }
}

fn evaluate<'tcx>(
    tcx: TyCtxt<'tcx>,
    word: Ty<'tcx>,
    body: &Body<'tcx>,
    zero: bool,
    covered: &mut [bool; MAX_BLOCKS],
) -> Option<Symbol> {
    let mut state = State {
        locals: LocalState::new(body)?,
        divisor_branch: false,
        divided: false,
    };
    state
        .locals
        .write(Local::from_usize(1).into(), Symbol::Left)?;
    state
        .locals
        .write(Local::from_usize(2).into(), Symbol::Right)?;
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
                    let value = rvalue(tcx, word, body, &mut state, zero, &assignment.1)?;
                    write(tcx, word, body, &mut state, assignment.0, value)?;
                }
                StatementKind::StorageLive(local) => state.locals.storage(*local, true)?,
                StatementKind::StorageDead(local) => state.locals.storage(*local, false)?,
                _ => return None,
            }
        }
        next = match &block.terminator.as_ref()?.kind {
            TerminatorKind::Return if state.divisor_branch && state.divided != zero => {
                return state.locals.read(RETURN_PLACE.into(), false);
            }
            TerminatorKind::Goto { target } => *target,
            TerminatorKind::SwitchInt { discr, targets } => {
                if targets.all_targets().len() > 3
                    || targets
                        .all_targets()
                        .iter()
                        .any(|target| target.as_usize() >= body.basic_blocks.len())
                {
                    return None;
                }
                match read(tcx, word, body, &mut state, discr)? {
                    // Host optimization switches on rhs directly. Exactly
                    // one zero entry leaves every nonzero value on otherwise;
                    // selecting a sample nonzero integer would be insufficient.
                    Symbol::Right
                        if targets.all_targets().len() == 2
                            && targets.iter().map(|(value, _)| value).eq([0]) =>
                    {
                        state.divisor_branch = true;
                        if zero {
                            targets.target_for_value(0)
                        } else {
                            targets.otherwise()
                        }
                    }
                    Symbol::DivisorZero if targets.iter().all(|(value, _)| value <= 1) => {
                        state.divisor_branch = true;
                        targets.target_for_value(u128::from(zero))
                    }
                    Symbol::Bool(flag) if targets.iter().all(|(value, _)| value <= 1) => {
                        targets.target_for_value(u128::from(flag))
                    }
                    _ => return None,
                }
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
                let value = if closed_hint(tcx, callee) {
                    let [flag] = &args[..] else { return None };
                    let value = read(tcx, word, body, &mut state, &flag.node)?;
                    if !matches!(value, Symbol::DivisorZero | Symbol::Bool(_)) {
                        return None;
                    }
                    value
                } else if is_cold_path(tcx, callee)
                    && args.is_empty()
                    && matches!(unwind, UnwindAction::Unreachable)
                {
                    Symbol::Unit
                } else {
                    return None;
                };
                write(tcx, word, body, &mut state, *destination, value)?;
                (*target)?
            }
            _ => return None,
        };
    }
}

#[cfg(test)]
#[path = "checked_div/tests.rs"]
mod tests;
