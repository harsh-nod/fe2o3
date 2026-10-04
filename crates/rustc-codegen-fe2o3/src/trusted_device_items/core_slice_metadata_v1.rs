//! Source-safety admission for the compiler-identified slice metadata operation.

use rustc_abi::ExternAbi;
use rustc_hir::{Mutability, Safety};
use rustc_middle::mir::{Body, Operand, Rvalue, StatementKind, TerminatorKind, UnOp};
use rustc_middle::ty::{EarlyBinder, Instance, InstanceKind, TyCtxt, TyKind, TypingEnv};

/// Discharges only the external-HIR check for rustc's exact `slice_len_fn`.
/// The generic helper's MIR remains collected and imported normally. This uses
/// the existing pinned-core trust boundary, not a proof of arbitrary core code
/// or authentication of a replacement sysroot.
pub(crate) fn authenticate_v1<'tcx>(tcx: TyCtxt<'tcx>, instance: Instance<'tcx>) -> bool {
    let Some(core) = tcx.lang_items().sized_trait() else {
        return false;
    };
    if !matches!(instance.def, InstanceKind::Item(_))
        || tcx.lang_items().slice_len_fn() != Some(instance.def_id())
        || instance.def_id().is_local()
        || instance.def_id().krate != core.krate
        || tcx.crate_name(core.krate).as_str() != "core"
        || !tcx.is_mir_available(instance.def_id())
    {
        return false;
    }
    let [element] = instance.args.as_slice() else {
        return false;
    };
    let Some(element) = element.as_type() else {
        return false;
    };
    let signature = tcx.instantiate_bound_regions_with_erased(
        tcx.fn_sig(instance.def_id())
            .instantiate(tcx, instance.args),
    );
    if signature.safety != Safety::Safe
        || signature.abi != ExternAbi::Rust
        || signature.c_variadic
        || signature.output() != tcx.types.usize
        || !matches!(signature.inputs(), [input]
            if matches!(input.kind(), TyKind::Ref(_, pointee, Mutability::Not)
                if matches!(pointee.kind(), TyKind::Slice(actual) if *actual == element)))
    {
        return false;
    }
    metadata_body_v1(tcx, instance, tcx.instance_mir(instance.def))
}

fn metadata_body_v1<'tcx>(tcx: TyCtxt<'tcx>, instance: Instance<'tcx>, body: &Body<'tcx>) -> bool {
    if body.source.instance != instance.def
        || body.source.promoted.is_some()
        || body.arg_count != 1
        || body.local_decls.len() != 2
        || body.basic_blocks.len() != 1
    {
        return false;
    }
    let [element] = instance.args.as_slice() else {
        return false;
    };
    let Some(element) = element.as_type() else {
        return false;
    };
    let mut locals = body.local_decls.iter();
    let Some(result) = locals.next() else {
        return false;
    };
    let Some(receiver) = locals.next() else {
        return false;
    };
    let Ok(receiver) = instance.try_instantiate_mir_and_normalize_erasing_regions(
        tcx,
        TypingEnv::fully_monomorphized(),
        EarlyBinder::bind(receiver.ty),
    ) else {
        return false;
    };
    if result.ty != tcx.types.usize
        || !matches!(receiver.kind(), TyKind::Ref(_, pointee, Mutability::Not)
            if matches!(pointee.kind(), TyKind::Slice(actual) if *actual == element))
    {
        return false;
    }
    let Some(block) = body.basic_blocks.iter().next() else {
        return false;
    };
    let [statement] = block.statements.as_slice() else {
        return false;
    };
    let StatementKind::Assign(assignment) = &statement.kind else {
        return false;
    };
    let (destination, value) = &**assignment;
    !block.is_cleanup
        && destination.local.index() == 0
        && destination.projection.is_empty()
        && matches!(value, Rvalue::UnaryOp(UnOp::PtrMetadata, Operand::Copy(input))
            if input.local.index() == 1 && input.projection.is_empty())
        && matches!(
            block.terminator.as_ref().map(|term| &term.kind),
            Some(TerminatorKind::Return)
        )
}

#[cfg(test)]
#[path = "core_slice_metadata_v1/tests.rs"]
mod tests;
