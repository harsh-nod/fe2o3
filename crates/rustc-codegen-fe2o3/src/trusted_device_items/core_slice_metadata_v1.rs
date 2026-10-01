//! Source-safety identity only; the wrapper and its real MIR callees stay traversed.

use super::*;
use rustc_hir::Mutability;

const REVIEWED_PATH: &str = "core::ptr::metadata";

#[derive(Clone, Copy, Debug)]
struct IdentityContract<'a> {
    item: bool,
    core: bool,
    external: bool,
    free_function: bool,
    path: &'a str,
    declared_generics: usize,
    instance_generics: usize,
    mir: bool,
    intrinsic: bool,
}

fn authenticate_identity(contract: IdentityContract<'_>) -> bool {
    contract.item
        && contract.core
        && contract.external
        && contract.free_function
        && contract.path == REVIEWED_PATH
        && contract.declared_generics == 1
        && contract.instance_generics == 1
        && contract.mir
        && !contract.intrinsic
}

#[derive(Clone, Copy, Debug)]
struct SignatureContract {
    normalized: bool,
    safe: bool,
    rust_abi: bool,
    variadic: bool,
    inputs: usize,
    generic_slice: bool,
    const_slice_pointer: bool,
    matching_generic_pointee: bool,
    usize_output: bool,
}

fn authenticate_signature(contract: SignatureContract) -> bool {
    contract.normalized
        && contract.safe
        && contract.rust_abi
        && !contract.variadic
        && contract.inputs == 1
        && contract.generic_slice
        && contract.const_slice_pointer
        && contract.matching_generic_pointee
        && contract.usize_output
}

pub(super) fn authenticate_v1<'tcx>(tcx: TyCtxt<'tcx>, instance: Instance<'tcx>) -> bool {
    let def_id = instance.def_id();
    if !matches!(instance.def, InstanceKind::Item(_))
        || def_id.is_local()
        || !crate::production_rustc_intrinsic_v1::is_reviewed_core_function_v1(tcx, instance)
    {
        return false;
    }
    // The pinned core wrapper has no dedicated lang/diagnostic item. Bind its
    // exact path inside the authenticated core crate, not a user-visible name.
    if !authenticate_identity(IdentityContract {
        item: true,
        core: true,
        external: true,
        free_function: tcx.def_kind(def_id) == rustc_hir::def::DefKind::Fn
            && tcx.opt_associated_item(def_id).is_none(),
        path: &tcx.def_path_str(def_id),
        declared_generics: tcx.generics_of(def_id).count(),
        instance_generics: instance.args.len(),
        mir: tcx.is_mir_available(def_id),
        intrinsic: tcx.intrinsic(def_id).is_some(),
    }) {
        return false;
    }
    let Some(pointee) = instance.args[0].as_type() else {
        return false;
    };
    let Ok(pointee) = tcx.try_normalize_erasing_regions(TypingEnv::fully_monomorphized(), pointee)
    else {
        return false;
    };
    let signature = tcx
        .instantiate_bound_regions_with_erased(tcx.fn_sig(def_id).instantiate(tcx, instance.args));
    let Ok(signature) =
        tcx.try_normalize_erasing_regions(TypingEnv::fully_monomorphized(), signature)
    else {
        return false;
    };
    let [input] = signature.inputs() else {
        return false;
    };
    let (const_slice_pointer, matching_generic_pointee) = match input.kind() {
        TyKind::RawPtr(input_pointee, Mutability::Not) => (
            matches!(input_pointee.kind(), TyKind::Slice(_)),
            *input_pointee == pointee,
        ),
        _ => (false, false),
    };
    authenticate_signature(SignatureContract {
        normalized: true,
        safe: signature.safety == Safety::Safe,
        rust_abi: signature.abi == ExternAbi::Rust,
        variadic: signature.c_variadic,
        inputs: signature.inputs().len(),
        generic_slice: matches!(pointee.kind(), TyKind::Slice(_)),
        const_slice_pointer,
        matching_generic_pointee,
        usize_output: signature.output() == tcx.types.usize,
    })
}

#[cfg(test)]
#[path = "core_slice_metadata_v1_tests.rs"]
mod tests;
