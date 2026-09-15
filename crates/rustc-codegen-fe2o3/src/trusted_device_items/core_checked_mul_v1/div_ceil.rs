//! Closed quotient/remainder source proof. This grants only external-HIR
//! authentication: division and overflow assertions remain in the original MIR.
//! The pinned sysroot has one division assertion and a plain Add. The checked
//! source representation also retains remainder and AddWithOverflow assertions.
//! `u32::is_multiple_of` shares the typed remainder/state machine, with a
//! separate zero-divisor result and a mandatory retained remainder assertion.

use super::*;
use rustc_middle::mir::{AssertKind, MirPhase, RuntimePhase};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Symbolic {
    Left,
    Right,
    Zero,
    One,
    Quotient,
    Remainder,
    Incremented,
    IncrementPair,
    DivisorZero,
    RemainderPositive,
    IncrementOverflow,
    LeftZero,
    RemainderZero,
}

impl Symbolic {
    fn ty<'tcx>(self, tcx: TyCtxt<'tcx>, word: Word) -> Ty<'tcx> {
        match self {
            Self::IncrementPair => Ty::new_tup(tcx, &[tcx.types.usize, tcx.types.bool]),
            Self::DivisorZero
            | Self::RemainderPositive
            | Self::IncrementOverflow
            | Self::LeftZero
            | Self::RemainderZero => tcx.types.bool,
            _ => word.ty(tcx),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Case {
    ZeroDivisor,
    Exact,
    Rounded,
}

#[derive(Clone)]
struct State {
    helper: Helper,
    locals: LocalState<Symbolic>,
    divisor_branch: bool,
    nonzero_asserted: bool,
    divided: bool,
    remainder: bool,
    remainder_branch: bool,
    increment_asserted: bool,
}

impl State {
    fn ready_to_return(&self, case: Case) -> bool {
        match self.helper {
            Helper::DivCeil => {
                self.nonzero_asserted && self.divided && self.remainder && self.remainder_branch
            }
            Helper::IsMultipleOf => {
                self.divisor_branch
                    && if case == Case::ZeroDivisor {
                        !self.nonzero_asserted && !self.remainder
                    } else {
                        self.nonzero_asserted && self.remainder
                    }
            }
            _ => false,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Outcome {
    DivisionByZero,
    Return(Symbolic),
}

// The parent applies its unchanged shared header/type/graph budgets first.
pub(super) fn reviewed_body<'tcx>(tcx: TyCtxt<'tcx>, body: &Body<'tcx>, helper: Helper) -> bool {
    let word = helper.word();
    if !matches!(helper, Helper::DivCeil | Helper::IsMultipleOf)
        || body.phase != MirPhase::Runtime(RuntimePhase::Optimized)
        || !matches!(body.source.instance, InstanceKind::Item(_))
        || body.is_polymorphic
        || body.injection_phase.is_some()
        || body.tainted_by_errors.is_some()
        || body
            .required_consts
            .as_ref()
            .is_none_or(|constants| !constants.is_empty())
        || !body.user_type_annotations.is_empty()
        || body.var_debug_info.len() > MAX_LOCALS
        || body.source_scopes.len() != if helper == Helper::DivCeil { 3 } else { 1 }
        || body.source_scopes.iter().enumerate().any(|(index, scope)| {
            scope.inlined.is_some()
                || scope.inlined_parent_scope.is_some()
                || scope.parent_scope.map(|parent| parent.as_usize()) != index.checked_sub(1)
        })
        || body.local_decls.iter().any(|decl| {
            decl.source_info.scope.as_usize() >= body.source_scopes.len()
                || (decl.ty != word.ty(tcx)
                    && decl.ty != tcx.types.bool
                    && !(helper == Helper::DivCeil
                        && decl.ty == Ty::new_tup(tcx, &[tcx.types.usize, tcx.types.bool])))
        })
    {
        return false;
    }
    let mut covered = [false; MAX_BLOCKS];
    // These are symbolic partitions, not integer samples. For is_multiple_of
    // the nonzero-divisor return retains the symbolic remainder-equals-zero
    // predicate, so no partition of the remainder itself is needed.
    let cases: &[(Case, Outcome)] = if helper == Helper::DivCeil {
        &[
            (Case::ZeroDivisor, Outcome::DivisionByZero),
            (Case::Exact, Outcome::Return(Symbolic::Quotient)),
            (Case::Rounded, Outcome::Return(Symbolic::Incremented)),
        ]
    } else {
        &[
            (Case::ZeroDivisor, Outcome::Return(Symbolic::LeftZero)),
            (Case::Exact, Outcome::Return(Symbolic::RemainderZero)),
        ]
    };
    for &(case, expected) in cases {
        if evaluate(tcx, body, helper, case, &mut covered) != Some(expected) {
            return false;
        }
    }
    covered[..body.basic_blocks.len()].iter().all(|seen| *seen)
}

fn read<'tcx>(
    tcx: TyCtxt<'tcx>,
    body: &Body<'tcx>,
    state: &mut State,
    operand: &Operand<'tcx>,
) -> Option<Symbolic> {
    match operand {
        Operand::Copy(place) | Operand::Move(place) => {
            let value = state
                .locals
                .read(*place, matches!(operand, Operand::Move(_)))?;
            if body.local_decls.get(place.local)?.ty != value.ty(tcx, state.helper.word()) {
                return None;
            }
            match place.projection.as_ref() {
                [] => Some(value),
                [ProjectionElem::Field(field, ty)] if value == Symbolic::IncrementPair => {
                    match field.as_usize() {
                        0 if *ty == tcx.types.usize && state.increment_asserted => {
                            Some(Symbolic::Incremented)
                        }
                        1 if *ty == tcx.types.bool => Some(Symbolic::IncrementOverflow),
                        _ => None,
                    }
                }
                _ => None,
            }
        }
        Operand::Constant(constant) => {
            let Const::Val(value, ty) = constant.const_ else {
                return None;
            };
            if ty != state.helper.word().ty(tcx) {
                return None;
            }
            match value
                .try_to_scalar_int()?
                .try_to_bits(rustc_abi::Size::from_bits(state.helper.word().bits(tcx)))
                .ok()?
            {
                0 => Some(Symbolic::Zero),
                1 => Some(Symbolic::One),
                _ => None,
            }
        }
        _ => None,
    }
}

fn rvalue<'tcx>(
    tcx: TyCtxt<'tcx>,
    body: &Body<'tcx>,
    state: &mut State,
    case: Case,
    value: &Rvalue<'tcx>,
) -> Option<Symbolic> {
    use Symbolic::*;
    match value {
        Rvalue::Use(operand) => read(tcx, body, state, operand),
        Rvalue::BinaryOp(op, operands) => {
            let left = read(tcx, body, state, &operands.0)?;
            let right = read(tcx, body, state, &operands.1)?;
            match (*op, left, right) {
                (BinOp::Eq, Right, Zero) => Some(DivisorZero),
                (BinOp::Div, Left, Right)
                    if state.helper == Helper::DivCeil && state.nonzero_asserted =>
                {
                    state.divided = true;
                    Some(Quotient)
                }
                (BinOp::Rem, Left, Right)
                    if state.nonzero_asserted
                        && match state.helper {
                            Helper::DivCeil => state.divided,
                            Helper::IsMultipleOf => {
                                state.divisor_branch && case != Case::ZeroDivisor
                            }
                            _ => false,
                        } =>
                {
                    state.remainder = true;
                    Some(Remainder)
                }
                (BinOp::Gt, Remainder, Zero) if state.helper == Helper::DivCeil => {
                    Some(RemainderPositive)
                }
                (BinOp::Eq, Left, Zero)
                    if state.helper == Helper::IsMultipleOf
                        && state.divisor_branch
                        && case == Case::ZeroDivisor =>
                {
                    Some(LeftZero)
                }
                (BinOp::Eq, Remainder, Zero)
                    if state.helper == Helper::IsMultipleOf && state.remainder =>
                {
                    Some(RemainderZero)
                }
                (BinOp::Add | BinOp::AddWithOverflow, Quotient, One)
                    if state.helper == Helper::DivCeil
                        && state.remainder_branch
                        && case == Case::Rounded =>
                {
                    if *op == BinOp::AddWithOverflow {
                        state.increment_asserted = false;
                        Some(IncrementPair)
                    } else {
                        // A positive remainder implies rhs >= 2, hence
                        // quotient <= MAX/2 and quotient + 1 cannot overflow.
                        Some(Incremented)
                    }
                }
                _ => None,
            }
        }
        _ => None,
    }
}

fn evaluate<'tcx>(
    tcx: TyCtxt<'tcx>,
    body: &Body<'tcx>,
    helper: Helper,
    case: Case,
    covered: &mut [bool; MAX_BLOCKS],
) -> Option<Outcome> {
    let mut state = State {
        helper,
        locals: LocalState::new(body)?,
        divisor_branch: false,
        nonzero_asserted: false,
        divided: false,
        remainder: false,
        remainder_branch: false,
        increment_asserted: false,
    };
    state
        .locals
        .write(Local::from_usize(1).into(), Symbolic::Left)?;
    state
        .locals
        .write(Local::from_usize(2).into(), Symbolic::Right)?;
    let mut visited = [false; MAX_BLOCKS];
    let mut next = BasicBlock::from_usize(0);
    loop {
        let block = body.basic_blocks.get(next)?;
        if std::mem::replace(visited.get_mut(next.as_usize())?, true) {
            return None;
        }
        covered[next.as_usize()] = true;
        for statement in &block.statements {
            if statement.source_info.scope.as_usize() >= body.source_scopes.len() {
                return None;
            }
            match &statement.kind {
                StatementKind::Assign(assignment) => {
                    let (destination, expression) = &**assignment;
                    if !destination.projection.is_empty()
                        || (1..=2).contains(&destination.local.as_usize())
                    {
                        return None;
                    }
                    let value = rvalue(tcx, body, &mut state, case, expression)?;
                    if body.local_decls.get(destination.local)?.ty != value.ty(tcx, helper.word()) {
                        return None;
                    }
                    state.locals.write(*destination, value)?;
                }
                StatementKind::StorageLive(local) => state.locals.storage(*local, true)?,
                StatementKind::StorageDead(local) => state.locals.storage(*local, false)?,
                _ => return None,
            }
        }
        let terminator = block.terminator.as_ref()?;
        if terminator.source_info.scope.as_usize() >= body.source_scopes.len() {
            return None;
        }
        next = match &terminator.kind {
            TerminatorKind::Return if state.ready_to_return(case) => {
                return Some(Outcome::Return(
                    state.locals.read(RETURN_PLACE.into(), false)?,
                ));
            }
            TerminatorKind::Goto { target } => *target,
            TerminatorKind::SwitchInt { discr, targets }
                if targets.iter().map(|(value, _)| value).eq([0])
                    && targets
                        .all_targets()
                        .iter()
                        .all(|target| target.as_usize() < body.basic_blocks.len()) =>
            {
                let nonzero = match (helper, read(tcx, body, &mut state, discr)?) {
                    (Helper::DivCeil, Symbolic::RemainderPositive) => {
                        state.remainder_branch = true;
                        case == Case::Rounded
                    }
                    (Helper::IsMultipleOf, Symbolic::Right) if !state.divisor_branch => {
                        state.divisor_branch = true;
                        case != Case::ZeroDivisor
                    }
                    _ => return None,
                };
                if nonzero {
                    targets.otherwise()
                } else {
                    targets.target_for_value(0)
                }
            }
            TerminatorKind::Assert {
                cond,
                expected: false,
                msg,
                target,
                unwind,
            } if matches!(unwind, UnwindAction::Continue | UnwindAction::Unreachable)
                && target.as_usize() < body.basic_blocks.len() =>
            {
                let condition = read(tcx, body, &mut state, cond)?;
                // Diagnostic operands execute only on the failing edge. Check
                // their moves in isolation, not in the surviving state.
                let mut failure = state.clone();
                match (&**msg, condition) {
                    (AssertKind::DivisionByZero(left), Symbolic::DivisorZero)
                        if helper == Helper::DivCeil
                            && !state.nonzero_asserted
                            && read(tcx, body, &mut failure, left) == Some(Symbolic::Left) =>
                    {
                        if case == Case::ZeroDivisor {
                            return Some(Outcome::DivisionByZero);
                        }
                        state.nonzero_asserted = true;
                    }
                    (AssertKind::RemainderByZero(left), Symbolic::DivisorZero)
                        if helper == Helper::DivCeil
                            && state.nonzero_asserted
                            && state.divided
                            && !state.remainder
                            && read(tcx, body, &mut failure, left) == Some(Symbolic::Left) => {}
                    (AssertKind::RemainderByZero(left), Symbolic::DivisorZero)
                        if helper == Helper::IsMultipleOf
                            && case != Case::ZeroDivisor
                            && state.divisor_branch
                            && !state.nonzero_asserted
                            && !state.remainder
                            && read(tcx, body, &mut failure, left) == Some(Symbolic::Left) =>
                    {
                        state.nonzero_asserted = true;
                    }
                    (
                        AssertKind::Overflow(BinOp::Add, left, right),
                        Symbolic::IncrementOverflow,
                    ) if case == Case::Rounded
                        && state.remainder_branch
                        && read(tcx, body, &mut failure, left) == Some(Symbolic::Quotient)
                        && read(tcx, body, &mut failure, right) == Some(Symbolic::One) =>
                    {
                        state.increment_asserted = true;
                    }
                    _ => return None,
                }
                *target
            }
            _ => return None,
        };
    }
}

#[cfg(test)]
#[path = "div_ceil/tests.rs"]
mod tests;
