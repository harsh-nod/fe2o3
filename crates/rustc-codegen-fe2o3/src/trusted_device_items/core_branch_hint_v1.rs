//! Source-safety recognition only; actual branch-hint MIR and callees stay traversed.

use super::*;

#[derive(Clone, Copy, Debug)]
struct Contract<'a> {
    item: bool,
    core: bool,
    generics: usize,
    mir: bool,
    intrinsic: bool,
    path: &'a str,
    safe: bool,
    rust_abi: bool,
    variadic: bool,
    inputs: usize,
    boolean_input: bool,
    boolean_output: bool,
}

fn authenticate_contract(contract: Contract<'_>) -> bool {
    contract.item
        && contract.core
        && contract.generics == 0
        && contract.mir
        && !contract.intrinsic
        && matches!(
            contract.path,
            "core::intrinsics::likely" | "core::intrinsics::unlikely"
        )
        && contract.safe
        && contract.rust_abi
        && !contract.variadic
        && contract.inputs == 1
        && contract.boolean_input
        && contract.boolean_output
}

/// The core lang-item crate and exact signature authenticate the external-HIR
/// exemption, not a replacement body or a semantic terminal.
pub(super) fn authenticate_v1<'tcx>(tcx: TyCtxt<'tcx>, instance: Instance<'tcx>) -> bool {
    if !matches!(instance.def, InstanceKind::Item(_))
        || !crate::production_rustc_intrinsic_v1::is_reviewed_core_function_v1(tcx, instance)
        || !instance.args.is_empty()
        || !tcx.is_mir_available(instance.def_id())
        || tcx.intrinsic(instance.def_id()).is_some()
    {
        return false;
    }
    let signature = tcx.instantiate_bound_regions_with_erased(
        tcx.fn_sig(instance.def_id())
            .instantiate(tcx, instance.args),
    );
    authenticate_contract(Contract {
        item: true,
        core: true,
        generics: instance.args.len(),
        mir: true,
        intrinsic: false,
        path: &tcx.def_path_str(instance.def_id()),
        safe: signature.safety == Safety::Safe,
        rust_abi: signature.abi == ExternAbi::Rust,
        variadic: signature.c_variadic,
        inputs: signature.inputs().len(),
        boolean_input: signature
            .inputs()
            .first()
            .is_some_and(|input| matches!(input.kind(), TyKind::Bool)),
        boolean_output: matches!(signature.output().kind(), TyKind::Bool),
    })
}

#[cfg(test)]
#[path = "core_branch_hint_v1_tests.rs"]
mod tests;
