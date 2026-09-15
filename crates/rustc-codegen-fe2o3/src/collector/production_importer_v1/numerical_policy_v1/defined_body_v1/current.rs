//! The legacy unbranded Current leaf, never a branded or policy Math issuer.

use super::*;

pub(super) fn matches_current<'tcx>(
    tcx: TyCtxt<'tcx>,
    instance: Instance<'tcx>,
    abi: &SemanticFunctionAbiV1,
    types: &[SemanticTypeDeclV1],
) -> bool {
    use dialect_amdgcn::DeviceMathDiagnosticItem as Math;
    if !matches!(instance.def, rustc_middle::ty::InstanceKind::Item(_))
        || !instance.args.is_empty()
        || trusted_device_items::classify(tcx, instance.def_id())
            != Some(TrustedDeviceItem::DeviceMath(Math::ContextFromCompiler))
        || !current_abi(abi, types)
    {
        return false;
    }
    let signature = tcx.instantiate_bound_regions_with_erased(
        tcx.fn_sig(instance.def_id())
            .instantiate(tcx, instance.args),
    );
    if signature.safety != rustc_hir::Safety::Safe
        || signature.abi != rustc_abi::ExternAbi::Rust
        || signature.c_variadic
        || !signature.inputs().is_empty()
    {
        return false;
    }
    let output = signature.output();
    let TyKind::Adt(definition, arguments) = *output.kind() else {
        return false;
    };
    if !definition.is_struct()
        || arguments.len() != 1
        || trusted_device_items::classify(tcx, definition.did())
            != Some(TrustedDeviceItem::DeviceMath(Math::Context))
    {
        return false;
    }
    let Some(brand) = arguments[0].as_type() else {
        return false;
    };
    let TyKind::Adt(marker, marker_arguments) = *brand.kind() else {
        return false;
    };
    marker.is_enum()
        && marker.variants().is_empty()
        && marker_arguments.is_empty()
        && trusted_device_items::is_exact_reviewed_provider_definition_v1(
            tcx,
            marker.did(),
            "fe2o3_device::context::UnbrandedCapability",
        )
        && semantic_type_for_rust_v1(tcx, types, output)
            .is_ok_and(|output| output == abi.source_output_type())
}

fn current_abi(abi: &SemanticFunctionAbiV1, types: &[SemanticTypeDeclV1]) -> bool {
    abi.canon_abi() == SemanticCanonAbiV1::Rust
        && abi.extern_abi() == SemanticExternAbiV1::Rust
        && !abi.can_unwind()
        && !abi.c_variadic()
        && abi.fixed_count() == 0
        && abi.source_input_types().is_empty()
        && abi.source_argument_ownership().is_empty()
        && abi.arguments().is_empty()
        && abi.adjusted_arguments().is_empty()
        && abi.hidden_arguments().is_empty()
        && abi.return_value().ty() == abi.source_output_type()
        && matches!(abi.return_value().mode(), SemanticAbiPassModeV1::Ignore)
        && abi.return_value().adjusted().is_none()
        && abi.return_value().pointee_override().is_none()
        && semantic_exact_inhabited_aggregate_zst_v1(types, abi.source_output_type())
        && types[abi.source_output_type().index() as usize]
            .layout()
            .alignment_bytes()
            == 1
}

#[cfg(test)]
#[path = "current_tests.rs"]
mod tests;
