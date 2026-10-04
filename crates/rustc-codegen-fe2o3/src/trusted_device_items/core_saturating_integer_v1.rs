//! Source-safety exemption for reviewed primitive wrappers, not call terminals.

use super::*;
use crate::production_rustc_intrinsic_v1::saturating_integer::integer_shape_v1;

#[derive(Clone, Copy, Debug)]
struct WrapperContractV1<'a> {
    item: bool,
    core: bool,
    generic_arguments: usize,
    mir_available: bool,
    path: &'a str,
    safe: bool,
    rust_abi: bool,
    variadic: bool,
    input_count: usize,
    first: Option<&'a str>,
    second: Option<&'a str>,
    result: Option<&'a str>,
    target_integer: bool,
}

fn authenticate_contract_v1(contract: WrapperContractV1<'_>) -> bool {
    let Some(integer) = contract.first else {
        return false;
    };
    if !matches!(
        integer,
        "i8" | "i16" | "i32" | "i64" | "isize" | "u8" | "u16" | "u32" | "u64" | "usize"
    ) {
        return false;
    }
    let Some((owner, method)) = contract.path.rsplit_once("::") else {
        return false;
    };
    contract.item
        && contract.core
        && contract.generic_arguments == 0
        && contract.mir_available
        && matches!(method, "saturating_add" | "saturating_sub")
        && owner == format!("core::num::<impl {integer}>")
        && contract.safe
        && contract.rust_abi
        && !contract.variadic
        && contract.input_count == 2
        && contract.second == Some(integer)
        && contract.result == Some(integer)
        && contract.target_integer
}

/// Uses the existing pinned core/lang-item trust boundary. This discharges only
/// the external-HIR check: real MIR and every nested call are still collected.
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
    let name = |ty: Ty<'tcx>| match ty.kind() {
        TyKind::Int(integer) => Some(integer.name_str()),
        TyKind::Uint(integer) => Some(integer.name_str()),
        _ => None,
    };
    authenticate_contract_v1(WrapperContractV1 {
        item: true,
        core: true,
        generic_arguments: instance.args.len(),
        mir_available: true,
        path: &tcx.def_path_str(instance.def_id()),
        safe: signature.safety == Safety::Safe,
        rust_abi: signature.abi == ExternAbi::Rust,
        variadic: signature.c_variadic,
        input_count: signature.inputs().len(),
        first: signature.inputs().first().copied().and_then(name),
        second: signature.inputs().get(1).copied().and_then(name),
        result: name(signature.output()),
        target_integer: integer_shape_v1(signature.output(), tcx.data_layout.pointer_size().bits())
            .is_some(),
    })
}

#[cfg(test)]
#[path = "core_saturating_integer_v1_tests.rs"]
mod tests;
