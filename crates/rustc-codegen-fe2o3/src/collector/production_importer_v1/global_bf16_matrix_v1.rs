use super::*;
use fe2o3_mir_model::semantic_mir_v1::{
    SemanticGlobalBf16MatrixLoadV1, SemanticGlobalBf16MatrixTypesV1,
};

#[cfg(test)]
mod tests;

#[cfg(test)]
pub(super) mod layout_diagnostics;

pub(super) fn validate_carriage<'tcx>(
    tcx: TyCtxt<'tcx>,
    plan: &ProductionSemanticPreflightPlanV1<'tcx>,
    contexts: &AuthenticatedProductionKernelContextsV1,
    mir: &AdmittedInertSemanticMirV1,
) -> Result<(), ProductionSemanticImportErrorV1> {
    use crate::production_semantic_terminal_v1::ProductionTerminalExpansionV1 as E;
    for (index, terminal) in plan.terminal_producers().iter().enumerate() {
        if !matches!(
            terminal.expansion,
            E::GlobalBf16MatrixALoadZeroFilled | E::GlobalBf16MatrixBLoadZeroFilled
        ) {
            continue;
        }
        let Some(SemanticCallableDeclV1::CompilerIntrinsic {
            binding, operation, ..
        }) = mir.callables().get(plan.function_producers().len() + index)
        else {
            return Err(body_owner_table_mismatch_v1(
                "global BF16 canonical callable carriage",
            ));
        };
        let root = capability_memory_root_for_terminal_v1(
            tcx,
            plan,
            contexts,
            u32::try_from(index)
                .map_err(|_| ProductionSemanticImportErrorV1::RootIdentityMismatch)?,
            terminal.expansion,
        )?;
        if mir.wire_version() < SemanticMirWireVersionV1::V22
            || binding.identity() != terminal.identities.function()
            || binding.abi().identity() != terminal.abi.identity
            || *operation
                != terminal_operation_v1(
                    tcx,
                    terminal.instance,
                    terminal.expansion,
                    binding.abi(),
                    mir.types(),
                    root,
                    terminal.identities.function(),
                    contexts,
                )?
        {
            return Err(body_owner_table_mismatch_v1(
                "global BF16 complete source contract carriage",
            ));
        }
    }
    Ok(())
}

struct RustGlobalBf16LoadV1<'tcx> {
    matrix: Ty<'tcx>,
    global: Ty<'tcx>,
    lane: Ty<'tcx>,
    element: Ty<'tcx>,
    matrix_brand: Ty<'tcx>,
    global_brand: Ty<'tcx>,
    kernel_brand: RustKernelBrandV1<'tcx>,
}

#[allow(clippy::too_many_arguments)]
pub(super) fn operation<'tcx>(
    tcx: TyCtxt<'tcx>,
    instance: Instance<'tcx>,
    role: SemanticMfmaOperandRoleV1,
    abi: &SemanticFunctionAbiV1,
    types: &[SemanticTypeDeclV1],
    rust_inputs: &[Ty<'tcx>],
    rust_output: Ty<'tcx>,
    root: Option<&AuthenticatedProductionKernelContextRootV1>,
    contexts: &AuthenticatedProductionKernelContextsV1,
    source_identity: SemanticFunctionIdentityV1,
) -> Result<SemanticCompilerIntrinsicOperationV1, ProductionSemanticImportErrorV1> {
    let expected = match role {
        SemanticMfmaOperandRoleV1::A => TrustedDeviceItem::Bf16MfmaGlobalMatrixALoadZeroFilled,
        SemanticMfmaOperandRoleV1::B => TrustedDeviceItem::Bf16MfmaGlobalMatrixBLoadZeroFilled,
    };
    if trusted_device_items::classify(tcx, instance.def_id()) != Some(expected) {
        return Err(body_owner_table_mismatch_v1(
            "global BF16 matrix terminal identity",
        ));
    }
    require_capability_memory_terminal_abi_v1(
        tcx,
        abi,
        types,
        rust_inputs,
        rust_output,
        &[
            SemanticSourceArgumentOwnershipV1::SharedBorrow,
            SemanticSourceArgumentOwnershipV1::SharedBorrow,
            SemanticSourceArgumentOwnershipV1::ByValue,
            SemanticSourceArgumentOwnershipV1::ByValue,
        ],
    )?;
    let root = root.ok_or(ProductionSemanticImportErrorV1::KernelContextBinding(
        "global BF16 matrix load lacks authenticated root custody",
    ))?;
    let rust = validate_source(tcx, role, rust_inputs, rust_output)?;
    if !rust_kernel_brand_matches_root_v1(tcx, rust.kernel_brand, root) {
        return Err(body_owner_table_mismatch_v1(
            "global BF16 matrix root brand",
        ));
    }
    let t = SemanticGlobalBf16MatrixTypesV1 {
        matrix: semantic_type_for_rust_v1(tcx, types, rust.matrix)?,
        global: semantic_type_for_rust_v1(tcx, types, rust.global)?,
        lane: semantic_type_for_rust_v1(tcx, types, rust.lane)?,
        fragment: abi.source_output_type(),
        element: semantic_type_for_rust_v1(tcx, types, rust.element)?,
        index: abi.source_input_types()[2],
    };
    if !fe2o3_mir_model::semantic_mir_v1::semantic_global_bf16_matrix_layout_matches_v1(types, t) {
        return Err(body_owner_table_mismatch_v1(
            "global BF16 matrix retained layout",
        ));
    }
    let contract = SemanticGlobalBf16MatrixLoadV1::new(
        t,
        SemanticMfmaOperandContractV1 {
            role,
            profile: SemanticMfmaProfileV1::Bf16F32M16N16K16,
            register_distribution: SemanticMfmaRegisterDistributionV1::Tile16x16,
            wave_width: 64,
        },
        rustc_type_identity_v1(tcx, rust.matrix_brand),
        rustc_type_identity_v1(tcx, rust.global_brand),
        capability_memory_provenance_v1(root, contexts)?,
        source_identity,
    )
    .map_err(ProductionSemanticImportErrorV1::SemanticSchema)?;
    Ok(SemanticCompilerIntrinsicOperationV1::GlobalBf16MatrixLoad { contract })
}

fn validate_source<'tcx>(
    tcx: TyCtxt<'tcx>,
    role: SemanticMfmaOperandRoleV1,
    rust_inputs: &[Ty<'tcx>],
    rust_output: Ty<'tcx>,
) -> Result<RustGlobalBf16LoadV1<'tcx>, ProductionSemanticImportErrorV1> {
    let invalid = || {
        body_owner_table_mismatch_v1(
            "global BF16 matrix load type, brand, layout, or memory contract",
        )
    };
    if rust_inputs.len() != 4
        || !rust_inputs[2..]
            .iter()
            .all(|ty| matches!(ty.kind(), TyKind::Uint(UintTy::Usize)))
    {
        return Err(invalid());
    }
    let matrix_ty = rust_shared_reference_v1(rust_inputs[0]).ok_or_else(invalid)?;
    let lane_ty = rust_shared_reference_v1(rust_inputs[1]).ok_or_else(invalid)?;
    let matrix_args = rust_trusted_adt_type_arguments_v1(
        tcx,
        matrix_ty,
        TrustedDeviceItem::Bf16MfmaGlobalMatrixView,
    )
    .ok_or_else(invalid)?;
    let [matrix_role, matrix_brand, global_brand] = matrix_args.as_slice() else {
        return Err(invalid());
    };
    let expected_role = match role {
        SemanticMfmaOperandRoleV1::A => TrustedDeviceItem::MfmaOperandA,
        SemanticMfmaOperandRoleV1::B => TrustedDeviceItem::MfmaOperandB,
    };
    let lane_args = rust_trusted_adt_type_arguments_v1(tcx, lane_ty, TrustedDeviceItem::WaveLane)
        .ok_or_else(invalid)?;
    let [lane_width, lane_brand] = lane_args.as_slice() else {
        return Err(invalid());
    };
    let fragment_args =
        rust_trusted_adt_type_arguments_v1(tcx, rust_output, TrustedDeviceItem::Bf16MfmaFragment)
            .ok_or_else(invalid)?;
    let [
        fragment_role,
        profile,
        distribution,
        fragment_width,
        fragment_brand,
    ] = fragment_args.as_slice()
    else {
        return Err(invalid());
    };
    let kernel_brand = matrix_kernel_brand(tcx, *matrix_brand, 0).ok_or_else(invalid)?;
    let global_kernel_brand = rust_kernel_brand_v1(tcx, *global_brand).ok_or_else(invalid)?;
    if !rust_is_exact_trusted_marker_v1(tcx, *matrix_role, expected_role)
        || fragment_role != matrix_role
        || fragment_brand != matrix_brand
        || lane_brand != matrix_brand
        || lane_width != fragment_width
        || !rust_is_exact_trusted_marker_v1(tcx, *lane_width, TrustedDeviceItem::Wave64)
        || !rust_is_exact_trusted_marker_v1(tcx, *profile, TrustedDeviceItem::Bf16MfmaProfile)
        || !rust_is_exact_trusted_marker_v1(
            tcx,
            *distribution,
            TrustedDeviceItem::MfmaRegisterTile16x16,
        )
        || !rust_same_kernel_brand_v1(kernel_brand, global_kernel_brand)
    {
        return Err(invalid());
    }
    for (axis, path) in [
        (kernel_brand.target, "fe2o3_device::context::CurrentTarget"),
        (
            kernel_brand.launch,
            "fe2o3_device::context::RegisteredLaunch",
        ),
    ] {
        if !rust_exact_reviewed_adt_arguments_v1(tcx, axis, path)
            .is_some_and(|arguments| arguments.is_empty())
        {
            return Err(invalid());
        }
    }
    let TyKind::Adt(definition, arguments) = *matrix_ty.kind() else {
        return Err(invalid());
    };
    let storage_reference = definition
        .non_enum_variant()
        .fields
        .iter()
        .next()
        .ok_or_else(invalid)?
        .ty(tcx, arguments);
    let global_ty = rust_shared_reference_v1(storage_reference).ok_or_else(invalid)?;
    let global = rust_capability_memory_view_v1(tcx, global_ty).ok_or_else(invalid)?;
    if global.role != RustCapabilityMemoryRoleV1::ReadOnly
        || !matches!(global.element.kind(), TyKind::Uint(UintTy::U16))
        || global.brand.ty != *global_brand
    {
        return Err(invalid());
    }
    Ok(RustGlobalBf16LoadV1 {
        matrix: matrix_ty,
        global: global_ty,
        lane: lane_ty,
        element: global.element,
        matrix_brand: *matrix_brand,
        global_brand: *global_brand,
        kernel_brand,
    })
}

fn matrix_kernel_brand<'tcx>(
    tcx: TyCtxt<'tcx>,
    brand: Ty<'tcx>,
    depth: usize,
) -> Option<RustKernelBrandV1<'tcx>> {
    if depth > 4 {
        return None;
    }
    if let Some(root) = rust_kernel_brand_v1(tcx, brand) {
        return Some(root);
    }
    if let Some(arguments) =
        rust_exact_reviewed_adt_arguments_v1(tcx, brand, "fe2o3_device::execution::SubgroupBrand")
    {
        let types = arguments.types().collect::<Vec<_>>();
        let [width, parent, _epoch] = types.as_slice() else {
            return None;
        };
        if rust_subgroup_width_v1(tcx, *width) != Some(64) {
            return None;
        }
        return matrix_kernel_brand(tcx, *parent, depth + 1);
    }
    let arguments = rust_exact_reviewed_adt_arguments_v1(
        tcx,
        brand,
        "fe2o3_device::execution::ReusableWorkgroupBrand",
    )?;
    let types = arguments.types().collect::<Vec<_>>();
    let [parent] = types.as_slice() else {
        return None;
    };
    matrix_kernel_brand(tcx, *parent, depth + 1)
}
