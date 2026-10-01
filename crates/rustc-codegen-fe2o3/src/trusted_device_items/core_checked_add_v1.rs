//! Source-safety recognition only; the compiler still imports the actual core MIR.

use super::*;
use crate::production_rustc_intrinsic_v1::saturating_integer::integer_shape_v1;

#[derive(Clone, Copy, Debug)]
struct Contract<'a> {
    item: bool,
    core: bool,
    generics: usize,
    mir: bool,
    path: &'a str,
    safe: bool,
    rust_abi: bool,
    variadic: bool,
    inputs: usize,
    first: Option<&'a str>,
    second: Option<&'a str>,
    core_option: bool,
    payload: Option<&'a str>,
    target_unsigned: bool,
}

fn authenticate_contract(contract: Contract<'_>) -> bool {
    let Some(integer) = contract.first else {
        return false;
    };
    matches!(integer, "u8" | "u16" | "u32" | "u64" | "usize")
        && contract.item
        && contract.core
        && contract.generics == 0
        && contract.mir
        && contract.path == format!("core::num::<impl {integer}>::checked_add")
        && contract.safe
        && contract.rust_abi
        && !contract.variadic
        && contract.inputs == 2
        && contract.second == Some(integer)
        && contract.core_option
        && contract.payload == Some(integer)
        && contract.target_unsigned
}

/// Uses the existing pinned core lang-item boundary, not a name-only sysroot
/// assertion. Only the external-HIR check is discharged; this is not a terminal.
pub(super) fn authenticate_v1<'tcx>(tcx: TyCtxt<'tcx>, instance: Instance<'tcx>) -> bool {
    if !matches!(instance.def, InstanceKind::Item(_))
        || !crate::production_rustc_intrinsic_v1::is_reviewed_core_function_v1(tcx, instance)
        || !instance.args.is_empty()
        || !tcx.is_mir_available(instance.def_id())
    {
        return false;
    }
    let signature = tcx.instantiate_bound_regions_with_erased(
        tcx.fn_sig(instance.def_id())
            .instantiate(tcx, instance.args),
    );
    let unsigned = |ty: Ty<'tcx>| match ty.kind() {
        TyKind::Uint(integer) => Some(integer.name_str()),
        _ => None,
    };
    let (core_option, payload) = match signature.output().kind() {
        TyKind::Adt(definition, arguments)
            if definition.did().krate == instance.def_id().krate
                && tcx.is_diagnostic_item(rustc_span::sym::Option, definition.did())
                && arguments.len() == 1 =>
        {
            (true, arguments[0].as_type().and_then(unsigned))
        }
        _ => (false, None),
    };
    authenticate_contract(Contract {
        item: true,
        core: true,
        generics: instance.args.len(),
        mir: true,
        path: &tcx.def_path_str(instance.def_id()),
        safe: signature.safety == Safety::Safe,
        rust_abi: signature.abi == ExternAbi::Rust,
        variadic: signature.c_variadic,
        inputs: signature.inputs().len(),
        first: signature.inputs().first().copied().and_then(unsigned),
        second: signature.inputs().get(1).copied().and_then(unsigned),
        core_option,
        payload,
        target_unsigned: signature.inputs().first().is_some_and(|input| {
            matches!(
                integer_shape_v1(*input, tcx.data_layout.pointer_size().bits()),
                Some((false, _))
            )
        }),
    })
}

#[cfg(test)]
#[path = "core_checked_add_v1_tests.rs"]
mod tests;
