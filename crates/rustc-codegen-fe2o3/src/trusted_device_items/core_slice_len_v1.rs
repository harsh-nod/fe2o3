//! Source-safety recognition only; the exact core len body and callees stay traversed.

use super::*;
use rustc_hir::Mutability;

#[derive(Clone, Copy, Debug)]
struct IdentityContract {
    item: bool,
    core: bool,
    external: bool,
    lang_item: bool,
    mir: bool,
    intrinsic: bool,
    generic_arity_matches: bool,
}

fn authenticate_identity(contract: IdentityContract) -> bool {
    contract.item
        && contract.core
        && contract.external
        && contract.lang_item
        && contract.mir
        && !contract.intrinsic
        && contract.generic_arity_matches
}

#[derive(Clone, Copy, Debug)]
struct SignatureContract {
    method: bool,
    inherent: bool,
    core_owner: bool,
    associated_owner: bool,
    normalized: bool,
    safe: bool,
    rust_abi: bool,
    variadic: bool,
    inputs: usize,
    shared_slice: bool,
    usize_output: bool,
    matching_self_type: bool,
}

fn authenticate_signature(contract: SignatureContract) -> bool {
    contract.method
        && contract.inherent
        && contract.core_owner
        && contract.associated_owner
        && contract.normalized
        && contract.safe
        && contract.rust_abi
        && !contract.variadic
        && contract.inputs == 1
        && contract.shared_slice
        && contract.usize_output
        && contract.matching_self_type
}

pub(super) fn authenticate_v1<'tcx>(tcx: TyCtxt<'tcx>, instance: Instance<'tcx>) -> bool {
    let def_id = instance.def_id();
    if !matches!(instance.def, InstanceKind::Item(_))
        || def_id.is_local()
        || !crate::production_rustc_intrinsic_v1::is_reviewed_core_function_v1(tcx, instance)
        || tcx.lang_items().slice_len_fn() != Some(def_id)
    {
        return false;
    }
    if !authenticate_identity(IdentityContract {
        item: true,
        core: true,
        external: true,
        lang_item: true,
        mir: tcx.is_mir_available(def_id),
        intrinsic: tcx.intrinsic(def_id).is_some(),
        generic_arity_matches: tcx.generics_of(def_id).count() == instance.args.len(),
    }) {
        return false;
    }
    let Some(associated) = tcx.opt_associated_item(def_id) else {
        return false;
    };
    let Some(implementation) = tcx.impl_of_assoc(def_id) else {
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
    let TyKind::Ref(_, slice, Mutability::Not) = input.kind() else {
        return false;
    };
    if !matches!(slice.kind(), TyKind::Slice(_)) {
        return false;
    }
    let impl_args = instance
        .args
        .truncate_to(tcx, tcx.generics_of(implementation));
    let Ok(self_type) = tcx.try_normalize_erasing_regions(
        TypingEnv::fully_monomorphized(),
        tcx.type_of(implementation).instantiate(tcx, impl_args),
    ) else {
        return false;
    };
    authenticate_signature(SignatureContract {
        method: associated.is_fn() && associated.is_method(),
        inherent: !tcx.impl_is_of_trait(implementation) && associated.trait_item_def_id().is_none(),
        core_owner: implementation.krate == def_id.krate,
        associated_owner: associated.impl_container(tcx) == Some(implementation),
        normalized: true,
        safe: signature.safety == Safety::Safe,
        rust_abi: signature.abi == ExternAbi::Rust,
        variadic: signature.c_variadic,
        inputs: signature.inputs().len(),
        shared_slice: true,
        usize_output: signature.output() == tcx.types.usize,
        matching_self_type: self_type == *slice,
    })
}

#[cfg(test)]
#[path = "core_slice_len_v1_tests.rs"]
mod tests;
