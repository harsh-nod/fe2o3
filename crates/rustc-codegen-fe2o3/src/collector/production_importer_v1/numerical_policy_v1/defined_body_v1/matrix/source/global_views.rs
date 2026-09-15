//! A defined constructor validator, not an execution terminal or SSA issuer.

use super::*;

pub(super) mod body;
pub(in crate::collector::production_importer_v1::numerical_policy_v1::defined_body_v1::matrix) mod layout;
pub(in crate::collector::production_importer_v1) mod loads;
mod logical_values;
pub(super) mod replay;

#[cfg(test)]
mod body_tests;
#[cfg(test)]
mod tests;

use logical_values::{Format, Role};

const CHECKED: &str = "fe2o3_device::gfx950::GlobalGfx950MfmaMatrix::checked";

pub(in crate::collector::production_importer_v1) fn kind(
    tcx: TyCtxt<'_>,
    definition: rustc_hir::def_id::DefId,
) -> Option<(Format, Role)> {
    match trusted_device_items::classify(tcx, definition) {
        Some(TrustedDeviceItem::Gfx950MfmaGlobalMatrixAFp4RowMajor) => {
            Some((Format::Fp4E2M1, Role::A))
        }
        Some(TrustedDeviceItem::Gfx950MfmaGlobalMatrixBFp4RowMajor) => {
            Some((Format::Fp4E2M1, Role::B))
        }
        Some(TrustedDeviceItem::Gfx950MfmaGlobalMatrixAFp8RowMajor) => {
            Some((Format::Fp8E4M3, Role::A))
        }
        Some(TrustedDeviceItem::Gfx950MfmaGlobalMatrixBFp8RowMajor) => {
            Some((Format::Fp8E4M3, Role::B))
        }
        _ => None,
    }
}

/// Ephemeral source facts. Nothing here is a public or serialized authority.
pub(in crate::collector::production_importer_v1) struct Constructor<'tcx> {
    identity: super::Identity<'tcx>,
    // Matrix reference/value, policy reference/value, bound pair.
    bound_types: [Ty<'tcx>; 5],
    bound_reference: Ty<'tcx>,
    receiver: Ty<'tcx>,
    inputs: [Ty<'tcx>; 6],
    global: Ty<'tcx>,
    view: Ty<'tcx>,
    error: Ty<'tcx>,
    output: Ty<'tcx>,
    checked: Instance<'tcx>,
}

pub(in crate::collector::production_importer_v1) fn validate_source<'tcx>(
    tcx: TyCtxt<'tcx>,
    instance: Instance<'tcx>,
) -> Result<Option<Constructor<'tcx>>, ProductionSemanticImportErrorV1> {
    loads::validate_source(tcx, instance)?;
    let Some((format, role)) = kind(tcx, instance.def_id()) else {
        return Ok(None);
    };
    let signature = super::signature(tcx, instance)?;
    let inputs: [Ty<'tcx>; 6] = signature
        .inputs()
        .try_into()
        .map_err(|_| rejected("global matrix constructor six original arguments"))?;
    if inputs[2..] != [tcx.types.usize; 4] {
        return Err(rejected("global matrix constructor exact usize dimensions"));
    }
    let receiver = rust_shared_reference_v1(inputs[0])
        .ok_or_else(|| rejected("global matrix constructor shared policy receiver"))?;
    let receiver_arguments = rust_trusted_adt_type_arguments_v1(
        tcx,
        receiver,
        TrustedDeviceItem::PolicyGfx950MatrixCapability,
    )
    .ok_or_else(|| rejected("global matrix constructor exact PolicyGfx950Matrix"))?;
    let receiver_fields = super::fields(tcx, receiver)?;
    let [bound_reference, private] = receiver_fields.as_slice() else {
        return Err(rejected(
            "global matrix constructor retained bound pair reference",
        ));
    };
    if !super::phantom(tcx, *private) {
        return Err(rejected("global matrix constructor private receiver field"));
    }
    let bound = rust_shared_reference_v1(*bound_reference)
        .ok_or_else(|| rejected("global matrix constructor shared bound reference"))?;
    let pair = super::pair(tcx, bound)?;
    let expected = [
        pair.identity.matrix_brand,
        pair.identity.root.ty,
        pair.identity.policy,
    ];
    if receiver_arguments != expected || instance.args.types().collect::<Vec<_>>() != expected {
        return Err(rejected(
            "global matrix constructor source receiver instance identity",
        ));
    }
    let global = rust_shared_reference_v1(inputs[1])
        .ok_or_else(|| rejected("global matrix constructor shared Global reference"))?;
    let memory = rust_capability_memory_view_v1(tcx, global)
        .ok_or_else(|| rejected("global matrix constructor exact Global source"))?;
    if memory.element != tcx.types.u8
        || !matches!(memory.role, RustCapabilityMemoryRoleV1::ReadOnly)
        || memory.brand.ty != pair.identity.root.ty
    {
        return Err(rejected("global matrix constructor u8 ReadOnly exact root"));
    }
    let output = signature.output();
    let (view, error) = rust_result_payloads_v1(tcx, output)
        .ok_or_else(|| rejected("global matrix constructor retains Result and both payloads"))?;
    if !rust_is_exact_trusted_marker_v1(tcx, error, TrustedDeviceItem::Gfx950MfmaMatrixViewError) {
        return Err(rejected("global matrix constructor exact checked error"));
    }
    let view_arguments = rust_trusted_adt_type_arguments_v1(
        tcx,
        view,
        TrustedDeviceItem::Gfx950MfmaGlobalMatrixView,
    )
    .ok_or_else(|| rejected("global matrix constructor exact Global view"))?;
    let [format_ty, role_ty, matrix_brand, global_brand] = view_arguments.as_slice() else {
        return Err(rejected(
            "global matrix constructor four view type arguments",
        ));
    };
    let format_marker = match format {
        Format::Fp4E2M1 => TrustedDeviceItem::Gfx950Fp4E2M1Format,
        Format::Fp8E4M3 => TrustedDeviceItem::Gfx950Fp8E4M3Format,
    };
    let role_marker = match role {
        Role::A => TrustedDeviceItem::Gfx950MfmaOperandA,
        Role::B => TrustedDeviceItem::Gfx950MfmaOperandB,
    };
    if !rust_is_exact_trusted_marker_v1(tcx, *format_ty, format_marker)
        || !rust_is_exact_trusted_marker_v1(tcx, *role_ty, role_marker)
        || *matrix_brand != pair.identity.matrix_brand
        || *global_brand != pair.identity.root.ty
        || !layout::view(tcx, view, inputs[1], *format_ty, *role_ty, *matrix_brand)
        || !layout::result(tcx, output)
    {
        return Err(rejected(
            "global matrix constructor exact format role brands fields and layout",
        ));
    }
    let original = tcx.instance_mir(instance.def);
    let checked = super::super::super::source::forwarding_callee(tcx, instance, original)?;
    if !trusted_device_items::is_exact_reviewed_provider_definition_v1(
        tcx,
        checked.def_id(),
        CHECKED,
    ) || !trusted_device_items::authenticate_reviewed_safe_external_helper_v1(
        tcx,
        checked.def_id(),
    )
    .map_err(|_| rejected("global matrix checked provider authentication"))?
    {
        return Err(rejected("global matrix original checked provider body"));
    }
    let checked_signature = super::signature(tcx, checked)?;
    if checked_signature.inputs() != &inputs[1..]
        || checked_signature.output() != output
        || checked.args.types().collect::<Vec<_>>() != view_arguments
        || !body::constructor(tcx, instance, original, inputs, output, checked)
    {
        return Err(rejected(
            "global matrix exact original forwarding body and checked ABI",
        ));
    }
    Ok(Some(Constructor {
        identity: pair.identity,
        bound_types: pair.types,
        bound_reference: *bound_reference,
        receiver,
        inputs,
        global,
        view,
        error,
        output,
        checked,
    }))
}

pub(in crate::collector::production_importer_v1) fn validate_canonical<'tcx>(
    tcx: TyCtxt<'tcx>,
    plan: &ProductionSemanticPreflightPlanV1<'tcx>,
    types: &[SemanticTypeDeclV1],
    functions: &[SemanticFunctionDeclV1],
    callables: &[SemanticCallableDeclV1],
    contexts: &AuthenticatedProductionKernelContextsV1,
) -> Result<(), ProductionSemanticImportErrorV1> {
    loads::validate_canonical(tcx, plan, types, functions, callables, contexts)?;
    let candidates = plan
        .function_producers()
        .iter()
        .enumerate()
        .filter_map(|(index, producer)| {
            kind(tcx, producer.instance.def_id())
                .map(|_| SemanticFunctionIdV1::from_index(index as u32))
        })
        .collect::<Vec<_>>();
    if candidates.is_empty() {
        return Ok(());
    }
    let roster = roster::Roster::new(tcx, plan, types, functions, callables)?;
    let call_edges = plan
        .direct_call_producers()
        .iter()
        .map(|call| (call.caller, call.callee))
        .collect::<Vec<_>>();
    let mut selected = BTreeSet::new();
    for function in candidates {
        let instance = roster.defined(function)?;
        let facts = validate_source(tcx, instance)?
            .ok_or_else(|| rejected("global matrix missing original constructor marker"))?;
        let checked = roster.defined_instance(facts.checked)?;
        let root = authenticate_capability_memory_root_v1(
            contexts,
            &BTreeSet::from([function]),
            &call_edges,
            false,
        )?;
        roster.root(root, contexts)?;
        super::super::identity(tcx, &facts.identity, root, contexts)?;
        roster.types(facts.bound_types)?;
        roster.types(facts.inputs)?;
        roster.types([
            facts.bound_reference,
            facts.receiver,
            facts.global,
            facts.view,
            facts.error,
            facts.output,
        ])?;
        let calls = plan
            .direct_call_producers()
            .iter()
            .filter(|call| call.caller == function)
            .collect::<Vec<_>>();
        if !matches!(calls.as_slice(), [call] if call.block == 0 && call.callee == checked)
            || plan
                .terminal_expansion_producers()
                .iter()
                .any(|call| call.caller == function)
            || plan
                .normalized_intrinsic_producers()
                .iter()
                .any(|call| call.caller == function)
        {
            return Err(rejected(
                "global matrix exact original checked forwarding edge",
            ));
        }
        require_capability_memory_terminal_abi_v1(
            tcx,
            functions[function.index() as usize].abi(),
            types,
            &facts.inputs,
            facts.output,
            &[
                SemanticSourceArgumentOwnershipV1::SharedBorrow,
                SemanticSourceArgumentOwnershipV1::SharedBorrow,
                SemanticSourceArgumentOwnershipV1::ByValue,
                SemanticSourceArgumentOwnershipV1::ByValue,
                SemanticSourceArgumentOwnershipV1::ByValue,
                SemanticSourceArgumentOwnershipV1::ByValue,
            ],
        )?;
        require_capability_memory_terminal_abi_v1(
            tcx,
            functions[checked.index() as usize].abi(),
            types,
            &facts.inputs[1..],
            facts.output,
            &[
                SemanticSourceArgumentOwnershipV1::SharedBorrow,
                SemanticSourceArgumentOwnershipV1::ByValue,
                SemanticSourceArgumentOwnershipV1::ByValue,
                SemanticSourceArgumentOwnershipV1::ByValue,
                SemanticSourceArgumentOwnershipV1::ByValue,
            ],
        )?;
        selected.insert(function);
        selected.insert(checked);
    }
    replay::validate(tcx, plan, types, functions, selected)
}

#[cfg(test)]
pub(in crate::collector::production_importer_v1) fn check_import<'tcx>(
    tcx: TyCtxt<'tcx>,
    plan: &ProductionSemanticPreflightPlanV1<'tcx>,
    imported: &ConstructedProductionSemanticMirV1,
) {
    tests::check_import(tcx, plan, imported);
}
