//! Closed source-safety authentication for reviewed unsigned arithmetic and its
//! retained core helpers. This is not a terminal: their MIR is still collected
//! and lowered independently, including the existing `cold_path` expansion.

use super::{
    BinOp, Body, ExternAbi, Instance, InstanceKind, Operand, Rvalue, Safety, StatementKind,
    TerminatorKind, Ty, TyCtxt, TyKind, TypingEnv, UnwindAction,
};
use rustc_hir::def::DefKind;
use rustc_middle::mir::{
    AggregateKind, BasicBlock, CastKind, Const, ConstValue, Local, Place, ProjectionElem,
    RETURN_PLACE,
};
use rustc_middle::ty::{ConstKind, FnSig};

const MAX_LOCALS: usize = 16;
const MAX_BLOCKS: usize = 8;
const MAX_STATEMENTS: usize = 64;
const MAX_SCOPES: usize = 8;

#[path = "core_checked_mul_v1/checked_div.rs"]
mod checked_div;
pub(crate) use checked_div::authenticate_reviewed_safe_core_checked_div_v1;

#[path = "core_checked_mul_v1/div_ceil.rs"]
mod div_ceil;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Operation {
    Add,
    Sub,
    Mul,
}

impl Operation {
    fn with_overflow(self) -> BinOp {
        match self {
            Self::Add => BinOp::AddWithOverflow,
            Self::Sub => BinOp::SubWithOverflow,
            Self::Mul => BinOp::MulWithOverflow,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Word {
    Usize,
    U32,
    U64,
}

impl Word {
    fn ty<'tcx>(self, tcx: TyCtxt<'tcx>) -> Ty<'tcx> {
        match self {
            Self::Usize => tcx.types.usize,
            Self::U32 => tcx.types.u32,
            Self::U64 => tcx.types.u64,
        }
    }

    fn from_ty<'tcx>(tcx: TyCtxt<'tcx>, ty: Ty<'tcx>) -> Option<Self> {
        [Self::Usize, Self::U32, Self::U64]
            .into_iter()
            .find(|word| word.ty(tcx) == ty)
    }

    fn bits(self, tcx: TyCtxt<'_>) -> u64 {
        match self {
            Self::Usize => tcx.data_layout.pointer_size().bits(),
            Self::U32 => 32,
            Self::U64 => 64,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Helper {
    Checked(Operation, Word),
    Overflowing(Operation, Word),
    WrappingSub,
    DivCeil,
    IsMultipleOf,
    Unlikely,
}

impl Helper {
    fn may_call(self, callee: Self) -> bool {
        match (self, callee) {
            (Self::Checked(operation, word), Self::Overflowing(callee, callee_word)) => {
                operation == callee && word == callee_word
            }
            (Self::Checked(..), Self::Unlikely) => true,
            _ => false,
        }
    }

    fn operation(self) -> Option<Operation> {
        match self {
            Self::Checked(operation, _) | Self::Overflowing(operation, _) => Some(operation),
            Self::WrappingSub => Some(Operation::Sub),
            Self::Unlikely | Self::DivCeil | Self::IsMultipleOf => None,
        }
    }

    fn may_call_cold_path(self) -> bool {
        matches!(self, Self::Checked(..) | Self::Unlikely)
    }

    fn word(self) -> Word {
        match self {
            Self::Checked(_, word) | Self::Overflowing(_, word) => word,
            Self::IsMultipleOf => Word::U32,
            _ => Word::Usize,
        }
    }

    fn reviewed_width(self) -> bool {
        self.word() == Word::Usize
            || matches!(
                self,
                Self::Checked(Operation::Mul, _)
                    | Self::Overflowing(Operation::Mul, _)
                    | Self::Checked(Operation::Add, Word::U64)
                    | Self::Overflowing(Operation::Add, Word::U64)
                    | Self::IsMultipleOf
            )
    }
}

/// Discharges only the unavailable external HIR observation, never an unsafe
/// call or downstream MIR/effect/lowering obligation.
pub(crate) fn authenticate_reviewed_safe_core_arithmetic_helper_v1<'tcx>(
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
    let signature = tcx.instantiate_bound_regions_with_erased(
        tcx.fn_sig(definition).instantiate(tcx, instance.args),
    );
    let name = tcx.item_name(definition);
    let word = if name.as_str() == "unlikely" {
        Word::Usize
    } else {
        Word::from_ty(tcx, *signature.inputs().first()?)?
    };
    let helper = match name.as_str() {
        "checked_add" => Helper::Checked(Operation::Add, word),
        "checked_sub" => Helper::Checked(Operation::Sub, word),
        "checked_mul" => Helper::Checked(Operation::Mul, word),
        "overflowing_add" => Helper::Overflowing(Operation::Add, word),
        "overflowing_sub" => Helper::Overflowing(Operation::Sub, word),
        "overflowing_mul" => Helper::Overflowing(Operation::Mul, word),
        "wrapping_sub" => Helper::WrappingSub,
        "div_ceil" => Helper::DivCeil,
        "is_multiple_of" => Helper::IsMultipleOf,
        "unlikely" => Helper::Unlikely,
        _ => return None,
    };
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
        && tcx.type_of(implementation).instantiate_identity() == helper.word().ty(tcx))
    .then_some(helper)
}

fn signature_matches<'tcx>(tcx: TyCtxt<'tcx>, helper: Helper, signature: FnSig<'tcx>) -> bool {
    let word = helper.word();
    helper.reviewed_width()
        && signature.safety == Safety::Safe
        && signature.abi == ExternAbi::Rust
        && !signature.c_variadic
        && match helper {
            Helper::Checked(..) => {
                signature.inputs() == [word.ty(tcx), word.ty(tcx)]
                    && is_option(tcx, word, signature.output())
            }
            Helper::Overflowing(..) => {
                signature.inputs() == [word.ty(tcx), word.ty(tcx)]
                    && signature.output() == Ty::new_tup(tcx, &[word.ty(tcx), tcx.types.bool])
            }
            Helper::WrappingSub | Helper::DivCeil => {
                signature.inputs() == [tcx.types.usize, tcx.types.usize]
                    && signature.output() == tcx.types.usize
            }
            Helper::Unlikely => {
                signature.inputs() == [tcx.types.bool] && signature.output() == tcx.types.bool
            }
            Helper::IsMultipleOf => {
                signature.inputs() == [tcx.types.u32, tcx.types.u32]
                    && signature.output() == tcx.types.bool
            }
        }
}

fn is_option<'tcx>(tcx: TyCtxt<'tcx>, word: Word, ty: Ty<'tcx>) -> bool {
    matches!(ty.kind(), TyKind::Adt(adt, args)
        if Some(adt.did()) == tcx.lang_items().option_type()
            && args.as_slice().len() == 1
            && args.as_slice()[0].as_type() == Some(word.ty(tcx)))
}

fn is_word<'tcx>(tcx: TyCtxt<'tcx>, word: Word, ty: Ty<'tcx>) -> bool {
    let TyKind::Uint(integer) = ty.kind() else {
        return false;
    };
    // Core uses same-width unsigned casts for its target-dependent ActualT.
    // Widening or narrowing would change the multiplication overflow predicate.
    integer
        .bit_width()
        .unwrap_or(tcx.data_layout.pointer_size().bits())
        == word.bits(tcx)
}

fn is_pair<'tcx>(tcx: TyCtxt<'tcx>, word: Word, ty: Ty<'tcx>) -> bool {
    matches!(ty.kind(), TyKind::Tuple(fields)
        if matches!(fields.as_slice(), [value, flag] if is_word(tcx, word, *value) && *flag == tcx.types.bool))
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Value {
    Left,
    Right,
    Arithmetic(Operation),
    OverflowPair(Operation),
    Bool(bool),
    SomeArithmetic(Operation),
    None,
    Unit,
}

fn value_has_type<'tcx>(tcx: TyCtxt<'tcx>, word: Word, value: Value, ty: Ty<'tcx>) -> bool {
    match value {
        Value::Left | Value::Right | Value::Arithmetic(_) => is_word(tcx, word, ty),
        Value::OverflowPair(_) => is_pair(tcx, word, ty),
        Value::Bool(_) => ty == tcx.types.bool,
        Value::SomeArithmetic(_) | Value::None => is_option(tcx, word, ty),
        Value::Unit => ty == tcx.types.unit,
    }
}

#[derive(Clone)]
struct LocalState<V> {
    values: [Option<V>; MAX_LOCALS],
    storage_live: [bool; MAX_LOCALS],
    moved_fields: [u8; MAX_LOCALS],
}

impl<V: Copy> LocalState<V> {
    fn new(body: &Body<'_>) -> Option<Self> {
        let mut state = Self {
            values: [None; MAX_LOCALS],
            storage_live: [true; MAX_LOCALS],
            moved_fields: [0; MAX_LOCALS],
        };
        // Locals without storage markers are always live. All explicitly
        // managed locals start dead, independently of value initialization.
        for block in body.basic_blocks.iter() {
            for statement in &block.statements {
                if let StatementKind::StorageLive(local) | StatementKind::StorageDead(local) =
                    statement.kind
                {
                    let index = local.as_usize();
                    if index <= body.arg_count || index >= body.local_decls.len() {
                        return None;
                    }
                    *state.storage_live.get_mut(index)? = false;
                }
            }
        }
        Some(state)
    }

    fn storage(&mut self, local: Local, live: bool) -> Option<()> {
        let index = local.as_usize();
        let previous = self.storage_live.get_mut(index)?;
        if *previous == live {
            return None;
        }
        *previous = live;
        self.values[index] = None;
        self.moved_fields[index] = 0;
        Some(())
    }

    fn write(&mut self, place: Place<'_>, value: V) -> Option<()> {
        let index = place.local.as_usize();
        if !place.projection.is_empty() || !*self.storage_live.get(index)? {
            return None;
        }
        self.values[index] = Some(value);
        self.moved_fields[index] = 0;
        Some(())
    }

    fn read(&mut self, place: Place<'_>, consume: bool) -> Option<V> {
        let index = place.local.as_usize();
        if !*self.storage_live.get(index)? {
            return None;
        }
        let fields = match place.projection.as_ref() {
            [] => 0b11,
            [ProjectionElem::Field(field, _)] if field.as_usize() < 2 => 1 << field.as_usize(),
            _ => return None,
        };
        if self.moved_fields[index] & fields != 0 {
            return None;
        }
        let value = self.values[index]?;
        if consume {
            if place.projection.is_empty() {
                self.values[index] = None;
            } else {
                // Moving one tuple field leaves the other field available,
                // but prevents a subsequent read of the whole tuple.
                self.moved_fields[index] |= fields;
            }
        }
        Some(value)
    }
}

fn reviewed_body<'tcx>(tcx: TyCtxt<'tcx>, body: &Body<'tcx>, helper: Helper) -> bool {
    let word = helper.word();
    let arguments = if helper == Helper::Unlikely { 1 } else { 2 };
    if !helper.reviewed_width()
        || body.arg_count != arguments
        || body.spread_arg.is_some()
        || body.coroutine.is_some()
        || body.source.promoted.is_some()
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
            !is_word(tcx, word, local.ty)
                && local.ty != tcx.types.bool
                && local.ty != tcx.types.unit
                && !is_pair(tcx, word, local.ty)
                && !is_option(tcx, word, local.ty)
        })
        || body.source_scopes.iter().any(|scope| {
            scope.inlined.is_some_and(|(instance, _)| {
                !helper_identity(tcx, instance).is_some_and(|callee| helper.may_call(callee))
                    || !authenticate_reviewed_safe_core_arithmetic_helper_v1(tcx, instance)
            })
        })
    {
        return false;
    }
    let expected_output = match helper {
        Helper::Checked(..) => is_option(tcx, word, body.local_decls[RETURN_PLACE].ty),
        Helper::Overflowing(..) => {
            body.local_decls[RETURN_PLACE].ty == Ty::new_tup(tcx, &[word.ty(tcx), tcx.types.bool])
        }
        Helper::WrappingSub | Helper::DivCeil => {
            body.local_decls[RETURN_PLACE].ty == tcx.types.usize
        }
        Helper::Unlikely | Helper::IsMultipleOf => {
            body.local_decls[RETURN_PLACE].ty == tcx.types.bool
        }
    };
    let input = if helper == Helper::Unlikely {
        tcx.types.bool
    } else {
        word.ty(tcx)
    };
    if !expected_output
        || body
            .args_iter()
            .any(|local| body.local_decls[local].ty != input)
    {
        return false;
    }
    let mut covered = [false; MAX_BLOCKS];
    // The only symbolic branch is this operation's exact overflow predicate
    // (unsigned lhs < rhs for subtraction). Exhaust both values, with no
    // concrete operand samples. Unchecked operations must be safe on each path.
    for overflow in [false, true] {
        let expected = match helper {
            Helper::Checked(..) if overflow => Value::None,
            Helper::Checked(operation, _) => Value::SomeArithmetic(operation),
            Helper::Overflowing(operation, _) => Value::OverflowPair(operation),
            Helper::WrappingSub => Value::Arithmetic(Operation::Sub),
            Helper::Unlikely => Value::Bool(overflow),
            Helper::DivCeil | Helper::IsMultipleOf => {
                return div_ceil::reviewed_body(tcx, body, helper);
            }
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
    let word = helper.word();
    let mut locals = LocalState::new(body)?;
    locals.write(
        Local::from_usize(1).into(),
        if helper == Helper::Unlikely {
            Value::Bool(overflow)
        } else {
            Value::Left
        },
    )?;
    if helper != Helper::Unlikely {
        locals.write(Local::from_usize(2).into(), Value::Right)?;
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
                    let value = read_rvalue(tcx, body, &mut locals, helper, overflow, rvalue)?;
                    if rvalue.ty(&body.local_decls, tcx)
                        != body.local_decls.get(destination.local)?.ty
                    {
                        return None;
                    }
                    write_local(tcx, body, word, &mut locals, *destination, value)?;
                }
                StatementKind::StorageLive(local) => locals.storage(*local, true)?,
                StatementKind::StorageDead(local) => locals.storage(*local, false)?,
                _ => return None,
            }
        }
        next = match &block.terminator.as_ref()?.kind {
            TerminatorKind::Return => return locals.read(RETURN_PLACE.into(), false),
            TerminatorKind::Goto { target } => *target,
            TerminatorKind::SwitchInt { discr, targets } => {
                let Value::Bool(flag) =
                    read_operand(tcx, body, word, &mut locals, overflow, discr)?
                else {
                    return None;
                };
                if targets.all_targets().len() > 3
                    || targets.iter().any(|(value, _)| value > 1)
                    || targets
                        .all_targets()
                        .iter()
                        .any(|target| target.as_usize() >= body.basic_blocks.len())
                {
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
                let value = if helper.may_call_cold_path() && is_cold_path(tcx, callee) {
                    if !args.is_empty() || !matches!(unwind, UnwindAction::Unreachable) {
                        return None;
                    }
                    Value::Unit
                } else if helper == Helper::WrappingSub && is_wrapping_sub(tcx, callee) {
                    let [left, right] = &args[..] else {
                        return None;
                    };
                    if !matches!(unwind, UnwindAction::Unreachable)
                        || read_operand(tcx, body, word, &mut locals, overflow, &left.node)
                            != Some(Value::Left)
                        || read_operand(tcx, body, word, &mut locals, overflow, &right.node)
                            != Some(Value::Right)
                        || left.node.ty(&body.local_decls, tcx) != tcx.types.usize
                        || right.node.ty(&body.local_decls, tcx) != tcx.types.usize
                    {
                        return None;
                    }
                    Value::Arithmetic(Operation::Sub)
                } else {
                    let kind = helper_identity(tcx, callee)?;
                    if !helper.may_call(kind)
                        || !authenticate_reviewed_safe_core_arithmetic_helper_v1(tcx, callee)
                    {
                        return None;
                    }
                    match (kind, &args[..]) {
                        (Helper::Overflowing(operation, _), [left, right])
                            if read_operand(tcx, body, word, &mut locals, overflow, &left.node)
                                == Some(Value::Left)
                                && read_operand(
                                    tcx,
                                    body,
                                    word,
                                    &mut locals,
                                    overflow,
                                    &right.node,
                                ) == Some(Value::Right)
                                && left.node.ty(&body.local_decls, tcx) == word.ty(tcx)
                                && right.node.ty(&body.local_decls, tcx) == word.ty(tcx) =>
                        {
                            Value::OverflowPair(operation)
                        }
                        (Helper::Unlikely, [flag]) => {
                            let value =
                                read_operand(tcx, body, word, &mut locals, overflow, &flag.node)?;
                            if !matches!(value, Value::Bool(_)) {
                                return None;
                            }
                            value
                        }
                        _ => return None,
                    }
                };
                let signature = tcx.instantiate_bound_regions_with_erased(
                    tcx.fn_sig(callee.def_id()).instantiate(tcx, callee.args),
                );
                if signature.output() != body.local_decls.get(destination.local)?.ty {
                    return None;
                }
                write_local(tcx, body, word, &mut locals, *destination, value)?;
                (*target)?
            }
            _ => return None,
        };
    }
}

fn write_local<'tcx>(
    tcx: TyCtxt<'tcx>,
    body: &Body<'tcx>,
    word: Word,
    locals: &mut LocalState<Value>,
    place: Place<'tcx>,
    value: Value,
) -> Option<()> {
    if !place.projection.is_empty()
        || !value_has_type(tcx, word, value, body.local_decls.get(place.local)?.ty)
    {
        return None;
    }
    locals.write(place, value)
}

fn read_operand<'tcx>(
    tcx: TyCtxt<'tcx>,
    body: &Body<'tcx>,
    word: Word,
    locals: &mut LocalState<Value>,
    overflow: bool,
    operand: &Operand<'tcx>,
) -> Option<Value> {
    match operand {
        Operand::Copy(place) | Operand::Move(place) => {
            let value = locals.read(*place, matches!(operand, Operand::Move(_)))?;
            match place.projection.as_ref() {
                [] => Some(value),
                [ProjectionElem::Field(field, ty)] => {
                    let Value::OverflowPair(operation) = value else {
                        return None;
                    };
                    let TyKind::Tuple(fields) = body.local_decls.get(place.local)?.ty.kind() else {
                        return None;
                    };
                    if fields.get(field.as_usize()) != Some(ty) {
                        return None;
                    }
                    match field.as_usize() {
                        0 if is_word(tcx, word, *ty) => Some(Value::Arithmetic(operation)),
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
            if !is_option(tcx, word, ty) {
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
    locals: &mut LocalState<Value>,
    helper: Helper,
    overflow: bool,
    rvalue: &Rvalue<'tcx>,
) -> Option<Value> {
    let word = helper.word();
    let mut read = |operand| read_operand(tcx, body, word, locals, overflow, operand);
    match rvalue {
        Rvalue::Use(operand) => read(operand),
        Rvalue::Cast(CastKind::IntToInt, operand, target)
            if is_word(tcx, word, operand.ty(&body.local_decls, tcx))
                && is_word(tcx, word, *target) =>
        {
            read(operand)
        }
        Rvalue::BinaryOp(binary, operands)
            if read(&operands.0) == Some(Value::Left)
                && read(&operands.1) == Some(Value::Right)
                && is_word(tcx, word, operands.0.ty(&body.local_decls, tcx))
                && operands.0.ty(&body.local_decls, tcx)
                    == operands.1.ty(&body.local_decls, tcx) =>
        {
            let operation = helper.operation()?;
            if *binary == operation.with_overflow()
                && matches!(helper, Helper::Checked(..) | Helper::Overflowing(..))
            {
                Some(Value::OverflowPair(operation))
            } else {
                match (helper, binary) {
                    (Helper::Checked(Operation::Sub, _), BinOp::Lt) => Some(Value::Bool(overflow)),
                    (Helper::Checked(Operation::Add, _), BinOp::AddUnchecked)
                    | (Helper::Checked(Operation::Sub, _), BinOp::SubUnchecked)
                        if !overflow =>
                    {
                        Some(Value::Arithmetic(operation))
                    }
                    (Helper::WrappingSub, BinOp::Sub) => Some(Value::Arithmetic(operation)),
                    _ => None,
                }
            }
        }
        Rvalue::Aggregate(kind, operands) => match (&**kind, operands.raw.as_slice()) {
            (AggregateKind::Tuple, [result, flag]) => {
                let operation = helper.operation()?;
                (read(result) == Some(Value::Arithmetic(operation))
                    && read(flag) == Some(Value::Bool(overflow)))
                .then_some(Value::OverflowPair(operation))
            }
            (AggregateKind::Adt(adt, variant, args, user_ty, field), operands)
                if Some(*adt) == tcx.lang_items().option_type()
                    && args.as_slice().len() == 1
                    && args.as_slice()[0].as_type() == Some(word.ty(tcx))
                    && user_ty.is_none()
                    && field.is_none() =>
            {
                let variant = tcx.adt_def(*adt).variants().get(*variant)?.def_id;
                match operands {
                    [] if Some(variant) == tcx.lang_items().option_none_variant() => {
                        Some(Value::None)
                    }
                    [result]
                        if Some(variant) == tcx.lang_items().option_some_variant()
                            && read(result) == Some(Value::Arithmetic(helper.operation()?)) =>
                    {
                        Some(Value::SomeArithmetic(helper.operation()?))
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
    let Const::Val(ConstValue::ZeroSized, ty) = callee.const_ else {
        return None;
    };
    let TyKind::FnDef(definition, arguments) = ty.kind() else {
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

fn is_wrapping_sub<'tcx>(tcx: TyCtxt<'tcx>, instance: Instance<'tcx>) -> bool {
    let Some(core) = tcx.lang_items().sized_trait() else {
        return false;
    };
    let InstanceKind::Intrinsic(definition) = instance.def else {
        return false;
    };
    if definition.krate != core.krate
        || tcx.crate_name(core.krate).as_str() != "core"
        || !matches!(instance.args.as_slice(), [argument] if argument.as_type() == Some(tcx.types.usize))
        || tcx
            .intrinsic(definition)
            .is_none_or(|intrinsic| intrinsic.name.as_str() != "wrapping_sub")
    {
        return false;
    }
    let signature = tcx.instantiate_bound_regions_with_erased(
        tcx.fn_sig(definition).instantiate(tcx, instance.args),
    );
    signature.safety == Safety::Safe
        && signature.abi == ExternAbi::Rust
        && !signature.c_variadic
        && signature.inputs() == [tcx.types.usize, tcx.types.usize]
        && signature.output() == tcx.types.usize
}

#[cfg(test)]
#[path = "core_checked_mul_v1_tests.rs"]
mod tests;
