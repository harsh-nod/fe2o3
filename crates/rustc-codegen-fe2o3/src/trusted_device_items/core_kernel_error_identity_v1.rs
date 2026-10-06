//! Closed source-origin authentication for the captured core KernelError identity.

use rustc_abi::ExternAbi;
use rustc_hir::{Safety, def::DefKind};
use rustc_middle::mir::{
    BasicBlock, Body, ClearCrossCrate, Operand, Place, Rvalue, SourceScope,
    StatementKind, TerminatorKind,
};
use rustc_middle::ty::{
    EarlyBinder, Instance, InstanceKind, Ty, TyCtxt, TyKind, TypeVisitableExt, TypingEnv,
};
use rustc_span::Symbol;

fn identity_signature_v1<'tcx>(tcx: TyCtxt<'tcx>, instance: Instance<'tcx>) -> Option<Ty<'tcx>> {
    if !matches!(instance.def, InstanceKind::Item(_))
        || !crate::production_rustc_intrinsic_v1::is_reviewed_core_function_v1(tcx, instance)
        || tcx.def_kind(instance.def_id()) != DefKind::AssocFn
        || !tcx.is_mir_available(instance.def_id())
        || instance.args.len() != 1
        || instance.args.has_param() || instance.args.has_escaping_bound_vars()
    { return None; }
    let from = tcx.get_diagnostic_item(Symbol::intern("from_fn"))?;
    let from_trait = tcx.get_diagnostic_item(Symbol::intern("From"))?;
    if from.krate != instance.def_id().krate || from_trait.krate != instance.def_id().krate
        || tcx.trait_of_assoc(from) != Some(from_trait)
        || tcx.associated_item(instance.def_id()).trait_item_def_id() != Some(from)
    { return None; }
    let argument = tcx.try_normalize_erasing_regions(
        TypingEnv::fully_monomorphized(), instance.args[0].as_type()?,
    ).ok()?;
    let TyKind::Adt(adt, args) = *argument.kind() else { return None; };
    if !adt.is_enum() || !args.is_empty()
        || super::classify(tcx, adt.did()) != Some(super::TrustedDeviceItem::KernelError)
    { return None; }
    let signature = tcx.instantiate_bound_regions_with_erased(
        tcx.fn_sig(instance.def_id()).instantiate(tcx, instance.args),
    );
    let signature = tcx.try_normalize_erasing_regions(TypingEnv::fully_monomorphized(), signature).ok()?;
    (signature.safety == Safety::Safe && signature.abi == ExternAbi::Rust
        && !signature.c_variadic && signature.inputs() == [argument]
        && signature.output() == argument).then_some(argument)
}

fn plain_local_v1(place: &Place<'_>, index: usize) -> bool {
    place.local.as_usize() == index && place.projection.is_empty()
}

fn identity_body_v1<'tcx>(tcx: TyCtxt<'tcx>, instance: Instance<'tcx>, body: &Body<'tcx>) -> bool {
    let Some(argument) = identity_signature_v1(tcx, instance) else { return false; };
    if body.arg_count != 1 || body.local_decls.len() != 2
        || body.basic_blocks.len() != 1 || body.source_scopes.len() != 1
    { return false; }
    let root = SourceScope::from_u32(0);
    for local in body.local_decls.iter() {
        if instance.try_instantiate_mir_and_normalize_erasing_regions(
            tcx, TypingEnv::fully_monomorphized(), EarlyBinder::bind(local.ty),
        ).ok() != Some(argument) || local.source_info.scope != root
        { return false; }
    }
    let scope = &body.source_scopes[root];
    if scope.parent_scope.is_some() || scope.inlined.is_some()
        || scope.inlined_parent_scope.is_some()
        || !matches!(&scope.local_data, ClearCrossCrate::Clear)
    { return false; }
    let block = &body.basic_blocks[BasicBlock::from_u32(0)];
    if block.is_cleanup || block.statements.len() != 1 { return false; }
    let statement = &block.statements[0];
    let Some(terminator) = &block.terminator else { return false; };
    if statement.source_info.scope != root || terminator.source_info.scope != root
        || !matches!(&terminator.kind, TerminatorKind::Return)
    { return false; }
    let StatementKind::Assign(assignment) = &statement.kind else { return false; };
    let Rvalue::Use(Operand::Move(input)) = &assignment.1 else { return false; };
    plain_local_v1(&assignment.0, 0) && plain_local_v1(input, 1)
}

/// Only discharges this wrapper's external-HIR source-origin check. Ordinary
/// MIR import and independent callee, inline-origin and unsafe audits remain.
pub(crate) fn authenticate_reviewed_safe_core_kernel_error_identity_v1<'tcx>(
    tcx: TyCtxt<'tcx>, instance: Instance<'tcx>,
) -> bool {
    identity_signature_v1(tcx, instance).is_some()
        && identity_body_v1(tcx, instance, tcx.instance_mir(instance.def))
}

#[cfg(test)]
#[path = "core_kernel_error_identity_v1_tests.rs"]
mod tests;
