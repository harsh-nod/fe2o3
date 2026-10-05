//! Closed source-origin checks for the two observed core Option wrappers.

use rustc_abi::ExternAbi;
use rustc_hir::{Safety, def::DefKind, def_id::DefId};
use rustc_middle::mir::{
    AggregateKind, BasicBlock, Body, ClearCrossCrate, Operand, Place, ProjectionElem,
    Rvalue, SourceScope, Statement, StatementKind, TerminatorKind, UnwindAction,
};
use rustc_middle::ty::{
    ClosureKind, EarlyBinder, Instance, InstanceKind, Ty, TyCtxt, TyKind,
    TypeVisitableExt, TypingEnv,
};
use rustc_span::sym;

use super::{TrustedDeviceItem, classify, definition};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Wrapper { OkOr, AndThen }

struct Types<'tcx> {
    wrapper: Wrapper,
    input: Ty<'tcx>,
    other: Ty<'tcx>,
    output: Ty<'tcx>,
    payload: Ty<'tcx>,
}

fn normalized<'tcx>(tcx: TyCtxt<'tcx>, instance: Instance<'tcx>, ty: Ty<'tcx>) -> Option<Ty<'tcx>> {
    instance.try_instantiate_mir_and_normalize_erasing_regions(
        tcx, TypingEnv::fully_monomorphized(), EarlyBinder::bind(ty),
    ).ok()
}

fn option_payload<'tcx>(tcx: TyCtxt<'tcx>, ty: Ty<'tcx>) -> Option<Ty<'tcx>> {
    let TyKind::Adt(adt, args) = *ty.kind() else { return None; };
    let core = tcx.lang_items().sized_trait()?.krate;
    if adt.did().krate != core || tcx.crate_name(core).as_str() != "core"
        || !tcx.is_diagnostic_item(sym::Option, adt.did()) || !adt.is_enum() || args.len() != 1
    { return None; }
    let mut variants = adt.variants().iter();
    if variants.next()?.def_id != tcx.lang_items().option_none_variant()?
        || variants.next()?.def_id != tcx.lang_items().option_some_variant()?
        || variants.next().is_some()
    { return None; }
    args[0].as_type()
}

fn result_arguments<'tcx>(tcx: TyCtxt<'tcx>, ty: Ty<'tcx>) -> Option<(Ty<'tcx>, Ty<'tcx>)> {
    let TyKind::Adt(adt, args) = *ty.kind() else { return None; };
    let core = tcx.lang_items().sized_trait()?.krate;
    if adt.did().krate != core || tcx.crate_name(core).as_str() != "core"
        || !tcx.is_diagnostic_item(sym::Result, adt.did()) || !adt.is_enum() || args.len() != 2
    { return None; }
    let mut variants = adt.variants().iter();
    if variants.next()?.def_id != tcx.lang_items().result_ok_variant()?
        || variants.next()?.def_id != tcx.lang_items().result_err_variant()?
        || variants.next().is_some()
    { return None; }
    Some((args[0].as_type()?, args[1].as_type()?))
}

fn payload_allowed<'tcx>(tcx: TyCtxt<'tcx>, payload: Ty<'tcx>) -> bool {
    if payload == tcx.types.usize || payload == tcx.types.u32 { return true; }
    let TyKind::Adt(adt, args) = *payload.kind() else { return false; };
    if classify(tcx, adt.did()) != Some(TrustedDeviceItem::DisjointTile2D) || args.len() != 5 {
        return false;
    }
    let Some(index_fn) = definition(tcx, TrustedDeviceItem::ThreadIndex1d) else { return false; };
    let signature = tcx.instantiate_bound_regions_with_erased(tcx.fn_sig(index_fn).instantiate_identity());
    let TyKind::Adt(index, index_args) = *signature.output().kind() else { return false; };
    if classify(tcx, index.did()) != Some(TrustedDeviceItem::ThreadIndex) || index_args.len() != 1
        || args[0].as_type() != index_args[0].as_type()
    { return false; }
    args.iter().skip(1).zip([64, 16, 16, 4]).all(|(arg, expected)| {
        arg.as_const().and_then(|value| value.try_to_target_usize(tcx)) == Some(expected)
    })
}

fn kernel_error<'tcx>(tcx: TyCtxt<'tcx>, error: Ty<'tcx>) -> bool {
    matches!(error.kind(), TyKind::Adt(adt, args)
        if adt.is_enum() && args.is_empty()
            && classify(tcx, adt.did()) == Some(TrustedDeviceItem::KernelError))
        && !error.needs_drop(tcx, TypingEnv::fully_monomorphized())
}

fn observed_callback<'tcx>(tcx: TyCtxt<'tcx>, callback: Ty<'tcx>, output: Ty<'tcx>) -> bool {
    let TyKind::Closure(def_id, args) = *callback.kind() else { return false; };
    if !def_id.is_local() || tcx.def_kind(def_id) != DefKind::Closure
        || args.has_param() || args.has_escaping_bound_vars()
        || callback.needs_drop(tcx, TypingEnv::fully_monomorphized())
    { return false; }
    let closure = args.as_closure();
    if closure.kind_ty().to_opt_closure_kind() != Some(ClosureKind::Fn) { return false; }
    let captures = closure.upvar_tys();
    if captures.len() != 1
        || !matches!(captures[0].kind(), TyKind::Ref(_, ty, mutability)
            if *ty == tcx.types.u32 && *mutability == rustc_hir::Mutability::Not)
    { return false; }
    let signature = tcx.instantiate_bound_regions_with_erased(closure.sig());
    let Ok(signature) = tcx.try_normalize_erasing_regions(TypingEnv::fully_monomorphized(), signature)
    else { return false; };
    signature.safety == Safety::Safe && signature.abi == ExternAbi::RustCall
        && !signature.c_variadic && signature.inputs() == [Ty::new_tup(tcx, &[tcx.types.usize])]
        && signature.output() == output
        && tcx.hir_maybe_body_owned_by(def_id.expect_local()).is_some()
}

fn signature<'tcx>(tcx: TyCtxt<'tcx>, instance: Instance<'tcx>) -> Option<Types<'tcx>> {
    if !matches!(instance.def, InstanceKind::Item(_))
        || !crate::production_rustc_intrinsic_v1::is_reviewed_core_function_v1(tcx, instance)
        || tcx.def_kind(instance.def_id()) != DefKind::AssocFn
        || !tcx.is_mir_available(instance.def_id())
        || !matches!(instance.args.len(), 2 | 3)
        || instance.args.iter().any(|arg| arg.as_type().is_none() || arg.has_param() || arg.has_escaping_bound_vars())
    { return None; }
    let implementation = tcx.impl_of_assoc(instance.def_id())?;
    if implementation.krate != instance.def_id().krate || tcx.impl_is_of_trait(implementation) {
        return None;
    }
    let receiver = tcx.try_normalize_erasing_regions(
        TypingEnv::fully_monomorphized(), tcx.type_of(implementation).instantiate(tcx, instance.args),
    ).ok()?;
    let payload = option_payload(tcx, receiver)?;
    let function = tcx.instantiate_bound_regions_with_erased(
        tcx.fn_sig(instance.def_id()).instantiate(tcx, instance.args),
    );
    let function = tcx.try_normalize_erasing_regions(TypingEnv::fully_monomorphized(), function).ok()?;
    let [input, other] = function.inputs() else { return None; };
    if function.safety != Safety::Safe || function.abi != ExternAbi::Rust || function.c_variadic
        || *input != receiver || instance.args[0].as_type() != Some(payload)
    { return None; }
    let output = function.output();
    let wrapper = match tcx.item_name(instance.def_id()).as_str() {
        "ok_or" if instance.args.len() == 2 && instance.args[1].as_type() == Some(*other)
            && result_arguments(tcx, output) == Some((payload, *other)) => Wrapper::OkOr,
        "and_then" if instance.args.len() == 3 && payload == tcx.types.usize
            && instance.args[1].as_type() == Some(tcx.types.usize)
            && instance.args[2].as_type() == Some(*other)
            && option_payload(tcx, output) == Some(tcx.types.usize)
            && observed_callback(tcx, *other, output) => Wrapper::AndThen,
        _ => return None,
    };
    Some(Types { wrapper, input: *input, other: *other, output, payload })
}

fn local(place: &Place<'_>, index: usize) -> bool {
    place.local.as_usize() == index && place.projection.is_empty()
}

fn operand(operand: &Operand<'_>, index: usize, copied: bool) -> bool {
    match (copied, operand) {
        (true, Operand::Copy(place)) | (false, Operand::Move(place)) => local(place, index),
        _ => false,
    }
}

fn assign_use(statement: &Statement<'_>, destination: usize, source: usize, copied: bool) -> bool {
    matches!(&statement.kind, StatementKind::Assign(assignment)
        if local(&assignment.0, destination)
            && matches!(&assignment.1, Rvalue::Use(value) if operand(value, source, copied)))
}

fn aggregate<'tcx>(
    tcx: TyCtxt<'tcx>, instance: Instance<'tcx>, statement: &Statement<'tcx>,
    expected_ty: Ty<'tcx>, variant_def: DefId, source: Option<(usize, bool)>,
) -> bool {
    let StatementKind::Assign(assignment) = &statement.kind else { return false; };
    let Rvalue::Aggregate(kind, operands) = &assignment.1 else { return false; };
    let AggregateKind::Adt(def_id, variant, args, None, None) = &**kind else { return false; };
    let TyKind::Adt(expected, expected_args) = *expected_ty.kind() else { return false; };
    let mut values = operands.iter();
    let operands_match = match source {
        Some((index, copied)) => values.next().is_some_and(|value| operand(value, index, copied))
            && values.next().is_none(),
        None => values.next().is_none(),
    };
    local(&assignment.0, 0) && *def_id == expected.did()
        && expected.variants().get(*variant).map(|row| row.def_id) == Some(variant_def)
        && instance.try_instantiate_mir_and_normalize_erasing_regions(
            tcx, TypingEnv::fully_monomorphized(), EarlyBinder::bind(*args),
        ).ok() == Some(expected_args) && operands_match
}

fn some_payload<'tcx>(tcx: TyCtxt<'tcx>, instance: Instance<'tcx>, statement: &Statement<'tcx>, payload: Ty<'tcx>) -> bool {
    let StatementKind::Assign(assignment) = &statement.kind else { return false; };
    let Rvalue::Use(Operand::Move(place)) = &assignment.1 else { return false; };
    let [ProjectionElem::Downcast(_, variant), ProjectionElem::Field(field, ty)] = place.projection.as_slice()
    else { return false; };
    local(&assignment.0, 4) && place.local.as_usize() == 1 && variant.as_usize() == 1
        && field.as_usize() == 0 && normalized(tcx, instance, *ty) == Some(payload)
}

fn drop_other(kind: &TerminatorKind<'_>, target_block: u32) -> bool {
    matches!(kind, TerminatorKind::Drop { place, target, unwind, replace: false, drop: None, async_fut: None }
        if local(place, 2) && *target == BasicBlock::from_u32(target_block)
            && matches!(unwind, UnwindAction::Unreachable))
}

fn callback_call<'tcx>(tcx: TyCtxt<'tcx>, instance: Instance<'tcx>, types: &Types<'tcx>, kind: &TerminatorKind<'tcx>) -> bool {
    let TerminatorKind::Call { func: Operand::Constant(function), args, destination, target, unwind, .. } = kind
    else { return false; };
    let TyKind::FnDef(def_id, raw_args) = *function.const_.ty().kind() else { return false; };
    let Some(fn_once) = tcx.lang_items().fn_once_trait() else { return false; };
    if def_id.krate != instance.def_id().krate || fn_once.krate != def_id.krate
        || tcx.trait_of_assoc(def_id) != Some(fn_once) || tcx.item_name(def_id).as_str() != "call_once"
        || !matches!(&args[..], [closure, tuple] if operand(&closure.node, 5, false) && operand(&tuple.node, 6, false))
        || !local(destination, 0) || *target != Some(BasicBlock::from_u32(4))
        || !matches!(unwind, UnwindAction::Unreachable)
    { return false; }
    let Ok(arguments) = instance.try_instantiate_mir_and_normalize_erasing_regions(
        tcx, TypingEnv::fully_monomorphized(), EarlyBinder::bind(raw_args),
    ) else { return false; };
    let tuple = Ty::new_tup(tcx, &[tcx.types.usize]);
    if arguments.len() != 2 || arguments[0].as_type() != Some(types.other)
        || arguments[1].as_type() != Some(tuple)
    { return false; }
    let function = tcx.instantiate_bound_regions_with_erased(tcx.fn_sig(def_id).instantiate(tcx, arguments));
    let Ok(function) = tcx.try_normalize_erasing_regions(TypingEnv::fully_monomorphized(), function)
    else { return false; };
    if function.safety != Safety::Safe || function.abi != ExternAbi::RustCall || function.c_variadic
        || function.inputs() != [types.other, tuple] || function.output() != types.output
    { return false; }
    let Ok(Some(resolved)) = Instance::try_resolve(tcx, TypingEnv::fully_monomorphized(), def_id, arguments)
    else { return false; };
    // Resolution must retain this exact closure. No callback or shim body is
    // exempted from the collector's existing independent traversal.
    matches!(resolved.def, InstanceKind::ClosureOnceShim { call_once, track_caller: false }
        if call_once == def_id && resolved.args == arguments)
}

fn body_matches<'tcx>(tcx: TyCtxt<'tcx>, instance: Instance<'tcx>, body: &Body<'tcx>) -> bool {
    let Some(types) = signature(tcx, instance) else { return false; };
    let and_then = types.wrapper == Wrapper::AndThen;
    if body.arg_count != 2 || body.local_decls.len() != (if and_then { 7 } else { 6 })
        || body.basic_blocks.len() != (if and_then { 6 } else { 5 })
        || body.source_scopes.len() != 2
    { return false; }
    let expected = [types.output, types.input, types.other, tcx.types.isize, types.payload, types.other];
    for (index, declaration) in body.local_decls.iter().enumerate() {
        let ty = if index < 6 { expected[index] } else { Ty::new_tup(tcx, &[tcx.types.usize]) };
        if normalized(tcx, instance, declaration.ty) != Some(ty)
            || declaration.source_info.scope != SourceScope::from_u32(0)
        { return false; }
    }
    for (index, scope) in body.source_scopes.iter().enumerate() {
        if scope.parent_scope != (index == 1).then_some(SourceScope::from_u32(0))
            || scope.inlined.is_some() || scope.inlined_parent_scope.is_some()
            || !matches!(&scope.local_data, ClearCrossCrate::Clear)
        { return false; }
    }
    let scopes: &[&[u32]] = if and_then {
        &[&[0], &[], &[0], &[0, 1, 1, 1, 1], &[1, 1], &[]]
    } else { &[&[0], &[], &[0, 0, 0, 0], &[0, 1], &[]] };
    for (index, (block, scopes)) in body.basic_blocks.iter().zip(scopes).enumerate() {
        if block.is_cleanup || block.statements.len() != scopes.len()
            || block.statements.iter().zip(*scopes).any(|(s, scope)| s.source_info.scope != SourceScope::from_u32(*scope))
            || block.terminator.as_ref().is_none_or(|term|
                term.source_info.scope != SourceScope::from_u32(if and_then && index == 3 { 1 } else { 0 }))
        { return false; }
    }
    let block = |index| &body.basic_blocks[BasicBlock::from_u32(index)];
    let StatementKind::Assign(discriminant) = &block(0).statements[0].kind else { return false; };
    let TerminatorKind::SwitchInt { discr, targets } = &block(0).terminator().kind else { return false; };
    if !local(&discriminant.0, 3)
        || !matches!(&discriminant.1, Rvalue::Discriminant(place) if local(place, 1))
        || !operand(discr, 3, false)
        || !targets.iter().eq([(0, BasicBlock::from_u32(2)), (1, BasicBlock::from_u32(3))])
        || targets.otherwise() != BasicBlock::from_u32(1)
        || !matches!(block(1).terminator().kind, TerminatorKind::Unreachable)
        || !some_payload(tcx, instance, &block(3).statements[0], types.payload)
    { return false; }
    if !and_then {
        let (Some(err), Some(ok)) = (tcx.lang_items().result_err_variant(), tcx.lang_items().result_ok_variant())
        else { return false; };
        let none = &block(2).statements;
        matches!(none[0].kind, StatementKind::StorageLive(value) if value.as_usize() == 5)
            && assign_use(&none[1], 5, 2, false)
            && aggregate(tcx, instance, &none[2], types.output, err, Some((5, false)))
            && matches!(none[3].kind, StatementKind::StorageDead(value) if value.as_usize() == 5)
            && matches!(block(2).terminator().kind, TerminatorKind::Goto { target } if target == BasicBlock::from_u32(4))
            && aggregate(tcx, instance, &block(3).statements[1], types.output, ok, Some((4, true)))
            && drop_other(&block(3).terminator().kind, 4)
            && matches!(block(4).terminator().kind, TerminatorKind::Return)
    } else {
        let Some(none) = tcx.lang_items().option_none_variant() else { return false; };
        let some = &block(3).statements;
        let StatementKind::Assign(tuple) = &some[4].kind else { return false; };
        let Rvalue::Aggregate(kind, operands) = &tuple.1 else { return false; };
        let mut operands = operands.iter();
        aggregate(tcx, instance, &block(2).statements[0], types.output, none, None)
            && drop_other(&block(2).terminator().kind, 5)
            && matches!(some[1].kind, StatementKind::StorageLive(value) if value.as_usize() == 5)
            && assign_use(&some[2], 5, 2, false)
            && matches!(some[3].kind, StatementKind::StorageLive(value) if value.as_usize() == 6)
            && local(&tuple.0, 6) && matches!(&**kind, AggregateKind::Tuple)
            && operands.next().is_some_and(|value| operand(value, 4, true)) && operands.next().is_none()
            && callback_call(tcx, instance, &types, &block(3).terminator().kind)
            && matches!(block(4).statements[0].kind, StatementKind::StorageDead(value) if value.as_usize() == 6)
            && matches!(block(4).statements[1].kind, StatementKind::StorageDead(value) if value.as_usize() == 5)
            && matches!(block(4).terminator().kind, TerminatorKind::Goto { target } if target == BasicBlock::from_u32(5))
            && matches!(block(5).terminator().kind, TerminatorKind::Return)
    }
}

/// Discharges only the external-HIR check for the exact wrapper. Callback,
/// shim, inline-origin and local unsafe-block audits are not transferred.
pub(crate) fn authenticate_reviewed_safe_core_attention_option_v1<'tcx>(tcx: TyCtxt<'tcx>, instance: Instance<'tcx>) -> bool {
    let Some(types) = signature(tcx, instance) else { return false; };
    (types.wrapper == Wrapper::AndThen || (payload_allowed(tcx, types.payload) && kernel_error(tcx, types.other)))
        && body_matches(tcx, instance, tcx.instance_mir(instance.def))
}

#[cfg(test)]
#[path = "core_attention_option_v1_tests.rs"]
mod tests;
