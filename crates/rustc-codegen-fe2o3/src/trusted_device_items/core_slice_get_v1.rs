//! Source-safety identity only; both safe owners retain their actual core MIR.

use super::*;
use rustc_hir::Mutability;

#[derive(Clone, Copy, Debug)]
struct SignatureContract {
    safe: bool,
    rust_abi: bool,
    variadic: bool,
    inputs: usize,
    shared_slice: bool,
    usize_index: bool,
    core_option: bool,
    shared_element: bool,
}

fn authenticate_signature(contract: SignatureContract) -> bool {
    contract.safe
        && contract.rust_abi
        && !contract.variadic
        && contract.inputs == 2
        && contract.shared_slice
        && contract.usize_index
        && contract.core_option
        && contract.shared_element
}

pub(super) fn authenticate_v1<'tcx>(tcx: TyCtxt<'tcx>, instance: Instance<'tcx>) -> bool {
    if !matches!(instance.def, InstanceKind::Item(_))
        || !crate::production_rustc_intrinsic_v1::is_reviewed_core_function_v1(tcx, instance)
        || !tcx.is_mir_available(instance.def_id())
        || tcx.intrinsic(instance.def_id()).is_some()
        || tcx.generics_of(instance.def_id()).count() != instance.args.len()
    {
        return false;
    }
    let Some(associated) = tcx.opt_associated_item(instance.def_id()) else {
        return false;
    };
    let Some(implementation) = tcx.impl_of_assoc(instance.def_id()) else {
        return false;
    };
    let core = instance.def_id().krate;
    if implementation.krate != core
        || !associated.is_fn()
        || !associated.is_method()
        || associated.impl_container(tcx) != Some(implementation)
        || tcx.item_name(instance.def_id()) != Symbol::intern("get")
    {
        return false;
    }
    let trait_owner = tcx.impl_is_of_trait(implementation);
    let signature = tcx.instantiate_bound_regions_with_erased(
        tcx.fn_sig(instance.def_id())
            .instantiate(tcx, instance.args),
    );
    let Ok(signature) =
        tcx.try_normalize_erasing_regions(TypingEnv::fully_monomorphized(), signature)
    else {
        return false;
    };
    let [first, second] = signature.inputs() else {
        return false;
    };
    let (slice_ref, index) = if trait_owner {
        (*second, *first)
    } else {
        (*first, *second)
    };
    let TyKind::Ref(_, slice, Mutability::Not) = slice_ref.kind() else {
        return false;
    };
    let TyKind::Slice(element) = slice.kind() else {
        return false;
    };
    let (core_option, shared_element) = match signature.output().kind() {
        TyKind::Adt(definition, arguments)
            if definition.did().krate == core
                && tcx.is_diagnostic_item(rustc_span::sym::Option, definition.did())
                && arguments.len() == 1 =>
        {
            (true, arguments[0].as_type().is_some_and(|payload| {
                matches!(payload.kind(), TyKind::Ref(_, output, Mutability::Not) if output == element)
            }))
        }
        _ => (false, false),
    };
    if !authenticate_signature(SignatureContract {
        safe: signature.safety == Safety::Safe,
        rust_abi: signature.abi == ExternAbi::Rust,
        variadic: signature.c_variadic,
        inputs: signature.inputs().len(),
        shared_slice: true,
        usize_index: matches!(index.kind(), TyKind::Uint(UintTy::Usize)),
        core_option,
        shared_element,
    }) {
        return false;
    }
    let impl_args = instance
        .args
        .truncate_to(tcx, tcx.generics_of(implementation));
    if !trait_owner {
        return associated.trait_item_def_id().is_none()
            && tcx.type_of(implementation).instantiate(tcx, impl_args) == *slice;
    }
    let Some(slice_index) = tcx.get_diagnostic_item(Symbol::intern("SliceIndex")) else {
        return false;
    };
    let Some(trait_method) = associated.trait_item_def_id() else {
        return false;
    };
    let Some(trait_item) = tcx.opt_associated_item(trait_method) else {
        return false;
    };
    if slice_index.krate != core
        || trait_method.krate != core
        || tcx.impl_trait_id(implementation) != slice_index
        || tcx.parent(trait_method) != slice_index
        || !trait_item.is_fn()
        || !trait_item.is_method()
        || tcx.item_name(trait_method) != Symbol::intern("get")
    {
        return false;
    }
    let trait_ref = tcx
        .impl_trait_ref(implementation)
        .instantiate(tcx, impl_args);
    trait_ref.args.len() == 2 && trait_ref.self_ty() == index && trait_ref.args.type_at(1) == *slice
}

#[cfg(test)]
#[path = "core_slice_get_v1_tests.rs"]
mod tests;
