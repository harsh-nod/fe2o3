//! Exact primitive core source-safety checks. Accepted MIR remains recursive.
//!
//! The safety observation alone never authorizes omitting a block. The separate
//! production proof supplies a whole-body cast expansion for the two reviewed
//! conversions; collection and semantic construction must consume that same
//! expansion while retaining the original source and ABI identities.

use rustc_abi::{ExternAbi, Size};
use rustc_hir::{Mutability, Safety, def::DefKind};
use rustc_middle::mir::{
    BasicBlock, BinOp, Body, CastKind, Const, ConstOperand, ConstValue, Local, Operand,
    ProjectionElem, Rvalue, StatementKind, TerminatorKind, UnwindAction,
};
use rustc_middle::ty::{
    ConstKind, FloatTy, FnSig, Instance, InstanceKind, Ty, TyCtxt, TyKind, TypeVisitableExt,
    TypingEnv,
};
use rustc_span::Symbol;

#[path = "core_primitive_value_v1/production.rs"]
mod production;
pub(crate) use production::{ReviewedCorePrimitiveCastV1, prove_core_primitive_cast_v1};

const MAX_LOCALS: usize = 24;
const MAX_BLOCKS: usize = 16;
const MAX_SCOPES: usize = 8;
const MAX_STATEMENTS: usize = 96;
const MAX_WORK: usize = 1024;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Helper<'tcx> {
    Ne(Ty<'tcx>),
    U32ToU64,
    U8ToF32,
}

impl<'tcx> Helper<'tcx> {
    fn input(self, tcx: TyCtxt<'tcx>) -> Ty<'tcx> {
        match self {
            Self::Ne(ty) => Ty::new_imm_ref(tcx, tcx.lifetimes.re_erased, ty),
            Self::U32ToU64 => tcx.types.u32,
            Self::U8ToF32 => tcx.types.u8,
        }
    }

    fn output(self, tcx: TyCtxt<'tcx>) -> Ty<'tcx> {
        match self {
            Self::Ne(_) => tcx.types.bool,
            Self::U32ToU64 => tcx.types.u64,
            Self::U8ToF32 => tcx.types.f32,
        }
    }

    fn arguments(self) -> usize {
        if matches!(self, Self::Ne(_)) { 2 } else { 1 }
    }
}

/// Authentication is neither a semantic terminal nor a panic exemption.
pub(crate) fn authenticate_reviewed_safe_core_primitive_value_helper_v1<'tcx>(
    tcx: TyCtxt<'tcx>,
    instance: Instance<'tcx>,
) -> bool {
    if let Some(element) = primitive_eq_identity(tcx, instance) {
        return reviewed_primitive_eq_body(tcx, instance, tcx.instance_mir(instance.def), element);
    }
    helper_identity(tcx, instance)
        .is_some_and(|helper| reviewed_body(tcx, tcx.instance_mir(instance.def), helper))
}

// Eq is intentionally outside Helper and the cast proof/expansion path. This
// authenticates its own nominal method and source, independently of Option.
fn primitive_eq_identity<'tcx>(tcx: TyCtxt<'tcx>, instance: Instance<'tcx>) -> Option<Ty<'tcx>> {
    let core = tcx.lang_items().sized_trait()?.krate;
    let partial_eq = tcx.lang_items().eq_trait()?;
    let eq = tcx.get_diagnostic_item(Symbol::intern("cmp_partialeq_eq"))?;
    let definition = instance.def_id();
    if !matches!(instance.def, InstanceKind::Item(_))
        || definition.krate != core
        || tcx.crate_name(core).as_str() != "core"
        || !instance.args.is_empty()
        || tcx.def_kind(definition) != DefKind::AssocFn
        || !tcx.is_mir_available(definition)
        || partial_eq.krate != core
        || eq.krate != core
        || tcx.get_diagnostic_item(Symbol::intern("PartialEq")) != Some(partial_eq)
        || tcx.trait_of_assoc(eq) != Some(partial_eq)
        || tcx.generics_of(definition).count() != 0
    {
        return None;
    }
    let item = tcx.opt_associated_item(definition)?;
    let implementation = tcx.impl_of_assoc(definition)?;
    if !item.is_fn()
        || item.trait_item_def_id() != Some(eq)
        || implementation.krate != core
        || !tcx.impl_is_of_trait(implementation)
        || tcx.generics_of(implementation).count() != 0
    {
        return None;
    }
    let env = TypingEnv::fully_monomorphized();
    let element = tcx
        .try_normalize_erasing_regions(
            env,
            tcx.type_of(implementation).instantiate(tcx, instance.args),
        )
        .ok()?;
    if !primitive(element) {
        return None;
    }
    let trait_ref = tcx
        .try_normalize_erasing_regions(
            env,
            tcx.impl_trait_ref(implementation)
                .instantiate(tcx, instance.args),
        )
        .ok()?;
    let arguments = tcx.mk_args(&[element.into(), element.into()]);
    if trait_ref.def_id != partial_eq
        || trait_ref.args != arguments
        || Instance::try_resolve(tcx, env, eq, arguments).ok()?? != instance
    {
        return None;
    }
    let signature = tcx.instantiate_bound_regions_with_erased(
        tcx.fn_sig(definition).instantiate(tcx, instance.args),
    );
    primitive_eq_signature(tcx, element, signature).then_some(element)
}

fn primitive_eq_signature<'tcx>(
    tcx: TyCtxt<'tcx>,
    element: Ty<'tcx>,
    signature: FnSig<'tcx>,
) -> bool {
    let Ok(signature) =
        tcx.try_normalize_erasing_regions(TypingEnv::fully_monomorphized(), signature)
    else {
        return false;
    };
    let reference = Ty::new_imm_ref(tcx, tcx.lifetimes.re_erased, element);
    signature.safety == Safety::Safe
        && signature.abi == ExternAbi::Rust
        && !signature.c_variadic
        && !signature.has_non_region_param()
        && !signature.has_infer()
        && !signature.has_aliases()
        && !signature.has_escaping_bound_vars()
        && signature.inputs() == [reference, reference]
        && signature.output() == tcx.types.bool
}

fn primitive_eq_load(statement: &StatementKind<'_>, destination: usize, input: usize) -> bool {
    matches!(statement, StatementKind::Assign(a)
        if a.0.local.as_usize() == destination && a.0.projection.is_empty()
        && matches!(&a.1, Rvalue::Use(Operand::Copy(place))
            if place.local.as_usize() == input
                && matches!(place.projection.as_ref(), [ProjectionElem::Deref])))
}

fn primitive_eq_operand(operand: &Operand<'_>, local: usize) -> bool {
    matches!(operand, Operand::Copy(place) | Operand::Move(place)
        if place.local.as_usize() == local && place.projection.is_empty())
}

fn primitive_eq_result(statement: &StatementKind<'_>) -> bool {
    matches!(statement, StatementKind::Assign(a)
        if a.0.local.as_usize() == 0 && a.0.projection.is_empty()
        && matches!(&a.1, Rvalue::BinaryOp(BinOp::Eq, inputs)
            if primitive_eq_operand(&inputs.0, 3) && primitive_eq_operand(&inputs.1, 4)))
}

// Fixed work: one block, five locals, one scope, three/seven statements. Both
// forms load each shared operand exactly once; all locals have primitive types.
// Eq keeps the primitive's semantics, including floating NaNs, without folding.
fn reviewed_primitive_eq_body<'tcx>(
    tcx: TyCtxt<'tcx>,
    instance: Instance<'tcx>,
    body: &Body<'tcx>,
    element: Ty<'tcx>,
) -> bool {
    if !primitive(element)
        || body.arg_count != 2
        || body.local_decls.len() != 5
        || body.basic_blocks.len() != 1
        || body.source_scopes.len() != 1
        || body.source.instance != instance.def
        || body.source.promoted.is_some()
        || body.tainted_by_errors.is_some()
        || body.coroutine.is_some()
        || body.spread_arg.is_some()
        || !body.user_type_annotations.is_empty()
        || body
            .required_consts
            .as_deref()
            .is_none_or(|items| !items.is_empty())
        || body
            .mentioned_items
            .as_deref()
            .is_none_or(|items| !items.is_empty())
        || body.source_scopes.iter().any(|s| {
            s.parent_scope.is_some() || s.inlined.is_some() || s.inlined_parent_scope.is_some()
        })
    {
        return false;
    }
    let reference = Ty::new_imm_ref(tcx, tcx.lifetimes.re_erased, element);
    let expected = [tcx.types.bool, reference, reference, element, element];
    if body.local_decls.iter().zip(expected).any(|(decl, ty)| {
        tcx.try_normalize_erasing_regions(TypingEnv::fully_monomorphized(), decl.ty)
            .ok()
            != Some(ty)
            || decl.user_ty.is_some()
            || decl.source_info.scope.as_usize() != 0
    }) {
        return false;
    }
    let block = &body.basic_blocks[BasicBlock::from_usize(0)];
    if block.is_cleanup
        || ![3, 7].contains(&block.statements.len())
        || block
            .statements
            .iter()
            .any(|s| s.source_info.scope.as_usize() != 0)
        || !block.terminator.as_ref().is_some_and(|t| {
            matches!(t.kind, TerminatorKind::Return) && t.source_info.scope.as_usize() == 0
        })
    {
        return false;
    }
    match block.statements.as_slice() {
        [left, right, result] => {
            primitive_eq_load(&left.kind, 3, 1)
                && primitive_eq_load(&right.kind, 4, 2)
                && primitive_eq_result(&result.kind)
        }
        [
            live_left,
            left,
            live_right,
            right,
            result,
            dead_right,
            dead_left,
        ] => {
            matches!(live_left.kind, StatementKind::StorageLive(l) if l.as_usize() == 3)
                && matches!(live_right.kind, StatementKind::StorageLive(l) if l.as_usize() == 4)
                && primitive_eq_load(&left.kind, 3, 1)
                && primitive_eq_load(&right.kind, 4, 2)
                && primitive_eq_result(&result.kind)
                && matches!(dead_right.kind, StatementKind::StorageDead(l) if l.as_usize() == 4)
                && matches!(dead_left.kind, StatementKind::StorageDead(l) if l.as_usize() == 3)
        }
        _ => false,
    }
}

fn primitive(ty: Ty<'_>) -> bool {
    matches!(
        ty.kind(),
        TyKind::Bool
            | TyKind::Char
            | TyKind::Int(_)
            | TyKind::Uint(_)
            | TyKind::Float(FloatTy::F32 | FloatTy::F64)
    )
}

fn helper_identity<'tcx>(tcx: TyCtxt<'tcx>, instance: Instance<'tcx>) -> Option<Helper<'tcx>> {
    let core = tcx.lang_items().sized_trait()?.krate;
    let definition = instance.def_id();
    if !matches!(instance.def, InstanceKind::Item(_))
        || definition.krate != core
        || tcx.crate_name(core).as_str() != "core"
        || !instance.args.is_empty()
        || tcx.def_kind(definition) != DefKind::AssocFn
        || !tcx.is_mir_available(definition)
    {
        return None;
    }
    let item = tcx.opt_associated_item(definition)?;
    let trait_item = item.trait_item_def_id()?;
    let implementation = tcx.impl_of_assoc(definition)?;
    if !item.is_fn() || implementation.krate != core || !tcx.impl_is_of_trait(implementation) {
        return None;
    }
    let trait_ref = tcx
        .impl_trait_ref(implementation)
        .instantiate(tcx, instance.args);
    if trait_ref.def_id.krate != core
        || tcx.trait_of_assoc(trait_item) != Some(trait_ref.def_id)
        || trait_item.krate != core
    {
        return None;
    }
    let self_ty = tcx.type_of(implementation).instantiate(tcx, instance.args);
    let helper = match tcx.item_name(definition).as_str() {
        "ne" if Some(trait_ref.def_id) == tcx.lang_items().eq_trait()
            && tcx.item_name(trait_item).as_str() == "ne"
            && primitive(self_ty)
            && trait_ref.args.as_slice()
                == tcx.mk_args(&[self_ty.into(), self_ty.into()]).as_slice() =>
        {
            Helper::Ne(self_ty)
        }
        "from"
            if Some(trait_ref.def_id) == tcx.get_diagnostic_item(Symbol::intern("From"))
                && tcx.item_name(trait_item).as_str() == "from" =>
        {
            let [target, source] = trait_ref.args.as_slice() else {
                return None;
            };
            match (source.as_type()?, target.as_type()?) {
                (source, target)
                    if source == tcx.types.u32 && target == tcx.types.u64 && self_ty == target =>
                {
                    Helper::U32ToU64
                }
                (source, target)
                    if source == tcx.types.u8 && target == tcx.types.f32 && self_ty == target =>
                {
                    Helper::U8ToF32
                }
                _ => return None,
            }
        }
        _ => return None,
    };
    let signature = tcx.instantiate_bound_regions_with_erased(
        tcx.fn_sig(definition).instantiate(tcx, instance.args),
    );
    signature_matches(tcx, helper, signature).then_some(helper)
}

fn signature_matches<'tcx>(
    tcx: TyCtxt<'tcx>,
    helper: Helper<'tcx>,
    signature: FnSig<'tcx>,
) -> bool {
    signature.safety == Safety::Safe
        && signature.abi == ExternAbi::Rust
        && !signature.c_variadic
        && signature.inputs().len() == helper.arguments()
        && signature.inputs().iter().all(|ty| *ty == helper.input(tcx))
        && signature.output() == helper.output(tcx)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Kind {
    Reference(u8),
    Input(u8),
    Ne,
    Converted,
    Bits(u128),
    Unit,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Value<'tcx> {
    ty: Ty<'tcx>,
    kind: Kind,
}

#[derive(Clone)]
struct State<'tcx> {
    block: usize,
    path: [bool; MAX_BLOCKS],
    live: [bool; MAX_LOCALS],
    values: [Option<Value<'tcx>>; MAX_LOCALS],
    feasible: bool,
}

fn reviewed_body<'tcx>(tcx: TyCtxt<'tcx>, body: &Body<'tcx>, helper: Helper<'tcx>) -> bool {
    let arguments = helper.arguments();
    if body.arg_count != arguments
        || !(arguments + 1..=MAX_LOCALS).contains(&body.local_decls.len())
        || !(1..=MAX_BLOCKS).contains(&body.basic_blocks.len())
        || !(1..=MAX_SCOPES).contains(&body.source_scopes.len())
        || body.spread_arg.is_some()
        || body.coroutine.is_some()
        || body.basic_blocks.iter().any(|block| block.is_cleanup)
        || body
            .basic_blocks
            .iter()
            .map(|block| block.statements.len())
            .sum::<usize>()
            > MAX_STATEMENTS
        || body
            .source_scopes
            .iter()
            .any(|scope| scope.inlined.is_some() || scope.inlined_parent_scope.is_some())
        || body
            .local_decls
            .iter()
            .any(|local| !local_type(tcx, helper, tcx.erase_and_anonymize_regions(local.ty)))
        || body.local_decls[Local::from_usize(0)].ty != helper.output(tcx)
        || body.args_iter().any(|local| {
            tcx.erase_and_anonymize_regions(body.local_decls[local].ty) != helper.input(tcx)
        })
    {
        return false;
    }
    let mut initial = State {
        block: 0,
        path: [false; MAX_BLOCKS],
        live: [true; MAX_LOCALS],
        values: [None; MAX_LOCALS],
        feasible: true,
    };
    for block in body.basic_blocks.iter() {
        for statement in &block.statements {
            if let StatementKind::StorageLive(local) | StatementKind::StorageDead(local) =
                statement.kind
            {
                let index = local.as_usize();
                if index <= arguments || index >= body.local_decls.len() {
                    return false;
                }
                initial.live[index] = false;
            }
        }
    }
    for index in 1..=arguments {
        initial.values[index] = Some(Value {
            ty: helper.input(tcx),
            kind: if matches!(helper, Helper::Ne(_)) {
                Kind::Reference(index as u8)
            } else {
                Kind::Input(index as u8)
            },
        });
    }
    let mut pending = vec![initial];
    let mut covered = [false; MAX_BLOCKS];
    let mut work = 0;
    let mut returns = 0;
    // Explore infeasible branches too: every node must have closed local-only
    // semantics. Only constant evaluation can make a path infeasible; symbolic
    // inputs are never sampled or used to assume away an assertion.
    while let Some(mut state) = pending.pop() {
        work += 1;
        if work > MAX_WORK || state.block >= body.basic_blocks.len() || state.path[state.block] {
            return false;
        }
        state.path[state.block] = true;
        covered[state.block] = true;
        let block = &body.basic_blocks[BasicBlock::from_usize(state.block)];
        for statement in &block.statements {
            work += 1;
            if work > MAX_WORK {
                return false;
            }
            match &statement.kind {
                StatementKind::StorageLive(local) => {
                    let index = local.as_usize();
                    if state.live[index] {
                        return false;
                    }
                    state.live[index] = true;
                    state.values[index] = None;
                }
                StatementKind::StorageDead(local) => {
                    let index = local.as_usize();
                    if !state.live[index] {
                        return false;
                    }
                    state.live[index] = false;
                    state.values[index] = None;
                }
                StatementKind::Assign(assignment) => {
                    let (destination, value) = &**assignment;
                    let index = destination.local.as_usize();
                    if !destination.projection.is_empty()
                        || index >= body.local_decls.len()
                        || (1..=arguments).contains(&index)
                        || !state.live[index]
                    {
                        return false;
                    }
                    let Some(value) = rvalue(tcx, body, helper, &mut state, value) else {
                        return false;
                    };
                    if value.ty
                        != tcx.erase_and_anonymize_regions(body.local_decls[destination.local].ty)
                    {
                        return false;
                    }
                    state.values[index] = Some(value);
                }
                _ => return false,
            }
        }
        let Some(terminator) = &block.terminator else {
            return false;
        };
        match &terminator.kind {
            TerminatorKind::Return => {
                let expected = Value {
                    ty: helper.output(tcx),
                    kind: if matches!(helper, Helper::Ne(_)) {
                        Kind::Ne
                    } else {
                        Kind::Converted
                    },
                };
                if state.values[0] != Some(expected) {
                    return false;
                }
                returns += usize::from(state.feasible);
            }
            TerminatorKind::Goto { target } => {
                state.block = target.as_usize();
                pending.push(state);
            }
            TerminatorKind::SwitchInt { discr, targets } if !matches!(helper, Helper::Ne(_)) => {
                let Some(Value {
                    ty,
                    kind: Kind::Bits(bit),
                }) = operand(tcx, body, &mut state, discr)
                else {
                    return false;
                };
                let mut explicit = targets.iter();
                let Some((0, false_block)) = explicit.next() else {
                    return false;
                };
                if explicit.next().is_some() || ty != tcx.types.bool || bit > 1 {
                    return false;
                }
                for (value, target) in [(0, false_block), (1, targets.otherwise())] {
                    let mut next = state.clone();
                    next.block = target.as_usize();
                    next.feasible &= bit == value;
                    pending.push(next);
                }
            }
            TerminatorKind::Call { .. }
                if !state.feasible
                    && !matches!(helper, Helper::Ne(_))
                    && exact_assertion_panic(tcx, body, &terminator.kind) => {}
            _ => return false,
        }
    }
    returns > 0
        && covered[..body.basic_blocks.len()]
            .iter()
            .all(|covered| *covered)
}

fn local_type<'tcx>(tcx: TyCtxt<'tcx>, helper: Helper<'tcx>, ty: Ty<'tcx>) -> bool {
    ty == helper.input(tcx)
        || ty == helper.output(tcx)
        || match helper {
            Helper::Ne(element) => ty == element,
            _ => [
                tcx.types.bool,
                tcx.types.unit,
                tcx.types.never,
                tcx.types.i128,
                tcx.types.u128,
            ]
            .contains(&ty),
        }
}

fn operand<'tcx>(
    tcx: TyCtxt<'tcx>,
    body: &Body<'tcx>,
    state: &mut State<'tcx>,
    operand: &Operand<'tcx>,
) -> Option<Value<'tcx>> {
    let (place, moved) = match operand {
        Operand::Constant(value) => return constant(tcx, value),
        Operand::Copy(place) => (place, false),
        Operand::Move(place) => (place, true),
        _ => return None,
    };
    let index = place.local.as_usize();
    if index >= body.local_decls.len() || !state.live[index] {
        return None;
    }
    let value = state.values[index]?;
    match place.projection.as_ref() {
        [] => {
            if moved {
                state.values[index] = None;
            }
            Some(value)
        }
        [ProjectionElem::Deref] if !moved => {
            let Kind::Reference(argument) = value.kind else {
                return None;
            };
            let TyKind::Ref(_, pointee, Mutability::Not) = *value.ty.kind() else {
                return None;
            };
            Some(Value {
                ty: pointee,
                kind: Kind::Input(argument),
            })
        }
        _ => None,
    }
}

fn rvalue<'tcx>(
    tcx: TyCtxt<'tcx>,
    body: &Body<'tcx>,
    helper: Helper<'tcx>,
    state: &mut State<'tcx>,
    value: &Rvalue<'tcx>,
) -> Option<Value<'tcx>> {
    match value {
        Rvalue::Use(value) => operand(tcx, body, state, value),
        Rvalue::BinaryOp(operation, inputs) => {
            let left = operand(tcx, body, state, &inputs.0)?;
            let right = operand(tcx, body, state, &inputs.1)?;
            if left.ty != right.ty {
                return None;
            }
            let kind = match (*operation, left.kind, right.kind) {
                (BinOp::Ne, Kind::Input(1), Kind::Input(2)) if helper == Helper::Ne(left.ty) => {
                    Kind::Ne
                }
                (BinOp::Le, Kind::Bits(left_bits), Kind::Bits(right_bits))
                    if !matches!(helper, Helper::Ne(_)) =>
                {
                    let result = if left.ty == tcx.types.i128 {
                        (left_bits as i128) <= (right_bits as i128)
                    } else if left.ty == tcx.types.u128 {
                        left_bits <= right_bits
                    } else {
                        return None;
                    };
                    Kind::Bits(u128::from(result))
                }
                _ => return None,
            };
            Some(Value {
                ty: tcx.types.bool,
                kind,
            })
        }
        Rvalue::Cast(kind, input, target) if !matches!(helper, Helper::Ne(_)) => {
            let value = operand(tcx, body, state, input)?;
            let result = match value.kind {
                Kind::Input(1)
                    if value.ty == helper.input(tcx)
                        && *target == helper.output(tcx)
                        && *kind
                            == if helper == Helper::U32ToU64 {
                                CastKind::IntToInt
                            } else {
                                CastKind::IntToFloat
                            } =>
                {
                    Kind::Converted
                }
                Kind::Bits(bits) => Kind::Bits(constant_cast(tcx, *kind, value.ty, bits, *target)?),
                _ => return None,
            };
            Some(Value {
                ty: *target,
                kind: result,
            })
        }
        _ => None,
    }
}

fn constant_cast<'tcx>(
    tcx: TyCtxt<'tcx>,
    kind: CastKind,
    source: Ty<'tcx>,
    bits: u128,
    target: Ty<'tcx>,
) -> Option<u128> {
    if kind == CastKind::IntToInt
        && [tcx.types.u8, tcx.types.u32, tcx.types.u64].contains(&source)
        && [tcx.types.i128, tcx.types.u128].contains(&target)
    {
        return Some(bits);
    }
    // IEEE f32::MIN saturates below i128::MIN. f32::MAX is exactly 2^128 - 2^104,
    // which fits u128; replacing it with u128::MAX would be an incorrect proof.
    match (kind, source, bits, target) {
        (CastKind::FloatToInt, source, 0xff7fffff, target)
            if source == tcx.types.f32 && target == tcx.types.i128 =>
        {
            Some(1 << 127)
        }
        (CastKind::FloatToInt, source, 0x7f7fffff, target)
            if source == tcx.types.f32 && target == tcx.types.u128 =>
        {
            Some(u128::MAX - ((1 << 104) - 1))
        }
        _ => None,
    }
}

fn constant<'tcx>(tcx: TyCtxt<'tcx>, constant: &ConstOperand<'tcx>) -> Option<Value<'tcx>> {
    let ty = constant.const_.ty();
    let bytes = if ty == tcx.types.bool || ty == tcx.types.u8 {
        1
    } else if ty == tcx.types.u32 || ty == tcx.types.f32 {
        4
    } else if ty == tcx.types.u64 {
        8
    } else if ty == tcx.types.i128 || ty == tcx.types.u128 {
        16
    } else if ty == tcx.types.unit {
        0
    } else {
        return None;
    };
    let value = match constant.const_ {
        Const::Val(value, _) => value,
        Const::Ty(_, constant) => match constant.kind() {
            ConstKind::Value(value) => tcx.valtree_to_const_val(value),
            _ => return None,
        },
        Const::Unevaluated(value, _) => {
            let core = tcx.lang_items().sized_trait()?.krate;
            let implementation = tcx.impl_of_assoc(value.def)?;
            if value.def.krate != core
                || !value.args.is_empty()
                || !matches!(tcx.def_kind(value.def), DefKind::AssocConst { .. })
                || !matches!(tcx.item_name(value.def).as_str(), "MIN" | "MAX")
                || implementation.krate != core
                || tcx.impl_is_of_trait(implementation)
                || tcx.type_of(implementation).instantiate_identity() != ty
                || ![tcx.types.u8, tcx.types.u32, tcx.types.u64, tcx.types.f32].contains(&ty)
            {
                return None;
            }
            constant
                .const_
                .eval(tcx, TypingEnv::fully_monomorphized(), constant.span)
                .ok()?
        }
    };
    let kind = if bytes == 0 {
        if value != ConstValue::ZeroSized {
            return None;
        }
        Kind::Unit
    } else {
        let bits = value
            .try_to_scalar_int()?
            .try_to_bits(Size::from_bytes(bytes))
            .ok()?;
        if ty == tcx.types.bool && bits > 1 {
            return None;
        }
        Kind::Bits(bits)
    };
    Some(Value { ty, kind })
}

fn exact_assertion_panic<'tcx>(
    tcx: TyCtxt<'tcx>,
    body: &Body<'tcx>,
    terminator: &TerminatorKind<'tcx>,
) -> bool {
    let TerminatorKind::Call {
        func: Operand::Constant(function),
        args,
        destination,
        target: None,
        unwind: UnwindAction::Unreachable,
        ..
    } = terminator
    else {
        return false;
    };
    let Const::Val(ConstValue::ZeroSized, ty) = function.const_ else {
        return false;
    };
    let TyKind::FnDef(definition, generics) = *ty.kind() else {
        return false;
    };
    let Some(core) = tcx.lang_items().sized_trait() else {
        return false;
    };
    let [argument] = args.as_ref() else {
        return false;
    };
    let Operand::Constant(message) = &argument.node else {
        return false;
    };
    let Const::Val(value, message_ty) = message.const_ else {
        return false;
    };
    definition.krate == core.krate
        && Some(definition) == tcx.lang_items().panic_fn()
        && generics.is_empty()
        && destination.projection.is_empty()
        && body
            .local_decls
            .get(destination.local)
            .is_some_and(|local| local.ty == tcx.types.never)
        && matches!(message_ty.kind(), TyKind::Ref(_, ty, Mutability::Not) if *ty == tcx.types.str_)
        && value
            .try_get_slice_bytes_for_diagnostics(tcx)
            .is_some_and(|bytes| bytes.len() <= 256 && bytes.starts_with(b"assertion failed: "))
}

#[cfg(test)]
#[path = "core_primitive_value_v1/tests.rs"]
mod tests;
