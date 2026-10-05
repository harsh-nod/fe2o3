//! Closed source-origin authentication for the observed core u32 widening.

use rustc_abi::ExternAbi;
use rustc_hir::{Safety, def::DefKind};
use rustc_middle::mir::{
    BasicBlock, Body, CastKind, ClearCrossCrate, Operand, Place, Rvalue, SourceScope,
    StatementKind, TerminatorKind,
};
use rustc_middle::ty::{EarlyBinder, Instance, InstanceKind, TyCtxt, TypingEnv};
use rustc_span::Symbol;

fn widening_signature_v1<'tcx>(tcx: TyCtxt<'tcx>, instance: Instance<'tcx>) -> bool {
    if !matches!(instance.def, InstanceKind::Item(_))
        || !crate::production_rustc_intrinsic_v1::is_reviewed_core_function_v1(tcx, instance)
        || tcx.def_kind(instance.def_id()) != DefKind::AssocFn
        || !tcx.is_mir_available(instance.def_id())
        || !instance.args.is_empty()
    {
        return false;
    }
    let Some(from) = tcx.get_diagnostic_item(Symbol::intern("from_fn")) else {
        return false;
    };
    let Some(from_trait) = tcx.get_diagnostic_item(Symbol::intern("From")) else {
        return false;
    };
    if from.krate != instance.def_id().krate
        || from_trait.krate != instance.def_id().krate
        || tcx.trait_of_assoc(from) != Some(from_trait)
        || tcx.associated_item(instance.def_id()).trait_item_def_id() != Some(from)
    {
        return false;
    }
    let signature = tcx.instantiate_bound_regions_with_erased(
        tcx.fn_sig(instance.def_id()).instantiate(tcx, instance.args),
    );
    let Ok(signature) =
        tcx.try_normalize_erasing_regions(TypingEnv::fully_monomorphized(), signature)
    else {
        return false;
    };
    signature.safety == Safety::Safe
        && signature.abi == ExternAbi::Rust
        && !signature.c_variadic
        && signature.inputs() == [tcx.types.u32]
        && signature.output() == tcx.types.u64
}

fn plain_local_v1(place: &Place<'_>, index: usize) -> bool {
    place.local.as_usize() == index && place.projection.is_empty()
}

fn widening_body_v1<'tcx>(
    tcx: TyCtxt<'tcx>,
    instance: Instance<'tcx>,
    body: &Body<'tcx>,
) -> bool {
    if !widening_signature_v1(tcx, instance)
        || body.arg_count != 1
        || body.local_decls.len() != 2
        || body.basic_blocks.len() != 1
        || body.source_scopes.len() != 1
    {
        return false;
    }
    let root_scope = SourceScope::from_u32(0);
    for (local, expected) in body.local_decls.iter().zip([tcx.types.u64, tcx.types.u32]) {
        if instance
            .try_instantiate_mir_and_normalize_erasing_regions(
                tcx,
                TypingEnv::fully_monomorphized(),
                EarlyBinder::bind(local.ty),
            )
            .ok()
            != Some(expected)
            || local.source_info.scope != root_scope
        {
            return false;
        }
    }
    let scope = &body.source_scopes[root_scope];
    if scope.parent_scope.is_some()
        || scope.inlined.is_some()
        || scope.inlined_parent_scope.is_some()
        || !matches!(&scope.local_data, ClearCrossCrate::Clear)
    {
        return false;
    }
    let block = &body.basic_blocks[BasicBlock::from_u32(0)];
    if block.is_cleanup || block.statements.len() != 1 {
        return false;
    }
    let statement = &block.statements[0];
    let Some(terminator) = &block.terminator else {
        return false;
    };
    if statement.source_info.scope != root_scope
        || terminator.source_info.scope != root_scope
        || !matches!(&terminator.kind, TerminatorKind::Return)
    {
        return false;
    }
    let StatementKind::Assign(assignment) = &statement.kind else {
        return false;
    };
    let Rvalue::Cast(CastKind::IntToInt, Operand::Copy(input), output) = &assignment.1 else {
        return false;
    };
    plain_local_v1(&assignment.0, 0)
        && plain_local_v1(input, 1)
        && instance
            .try_instantiate_mir_and_normalize_erasing_regions(
                tcx,
                TypingEnv::fully_monomorphized(),
                EarlyBinder::bind(*output),
            )
            .ok()
            == Some(tcx.types.u64)
}

/// Only discharges the external-HIR source-origin check. Ordinary MIR import
/// and the independent inline-origin audit remain mandatory and unchanged.
pub(crate) fn authenticate_reviewed_safe_core_u32_widening_v1<'tcx>(
    tcx: TyCtxt<'tcx>,
    instance: Instance<'tcx>,
) -> bool {
    widening_signature_v1(tcx, instance)
        && widening_body_v1(tcx, instance, tcx.instance_mir(instance.def))
}

#[cfg(test)]
#[path = "core_u32_widening_v1_tests.rs"]
mod tests;
