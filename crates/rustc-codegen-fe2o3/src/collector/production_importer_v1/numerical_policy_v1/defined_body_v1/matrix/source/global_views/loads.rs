//! Defined byte-load/packing helpers. No opaque load or fragment authority is issued.
use super::*;

mod body;
mod carriage;
#[cfg(test)]
mod tests;

pub(super) fn kind(
    tcx: TyCtxt<'_>,
    definition: rustc_hir::def_id::DefId,
) -> Option<(Format, Role)> {
    match trusted_device_items::classify(tcx, definition) {
        Some(TrustedDeviceItem::Gfx950MfmaGlobalMatrixAFp4LoadM16K128) => {
            Some((Format::Fp4E2M1, Role::A))
        }
        Some(TrustedDeviceItem::Gfx950MfmaGlobalMatrixBFp4LoadK128N16) => {
            Some((Format::Fp4E2M1, Role::B))
        }
        Some(TrustedDeviceItem::Gfx950MfmaGlobalMatrixAFp8LoadM16K128) => {
            Some((Format::Fp8E4M3, Role::A))
        }
        Some(TrustedDeviceItem::Gfx950MfmaGlobalMatrixBFp8LoadK128N16) => {
            Some((Format::Fp8E4M3, Role::B))
        }
        _ => None,
    }
}

pub(super) struct Load<'tcx> {
    root: RustKernelBrandV1<'tcx>,
    matrix_brand: Ty<'tcx>,
    epoch: Ty<'tcx>,
    inputs: [Ty<'tcx>; 4],
    view: Ty<'tcx>,
    lane: Ty<'tcx>,
    global_reference: Ty<'tcx>,
    global: Ty<'tcx>,
    output: Ty<'tcx>,
    registers: Ty<'tcx>,
    helpers: [Instance<'tcx>; 3],
}

pub(super) fn validate_source<'tcx>(
    tcx: TyCtxt<'tcx>,
    instance: Instance<'tcx>,
) -> Result<Option<Load<'tcx>>, ProductionSemanticImportErrorV1> {
    let Some((format, role)) = kind(tcx, instance.def_id()) else {
        return Ok(None);
    };
    let signature = super::super::signature(tcx, instance)?;
    let inputs: [Ty<'tcx>; 4] = signature
        .inputs()
        .try_into()
        .map_err(|_| rejected("Global FP4/8 load retains view, lane and two bases"))?;
    if inputs[2..] != [tcx.types.usize; 2] {
        return Err(rejected("Global FP4/8 load exact usize bases"));
    }
    let view = rust_shared_reference_v1(inputs[0])
        .ok_or_else(|| rejected("Global FP4/8 load shared view"))?;
    let arguments = rust_trusted_adt_type_arguments_v1(
        tcx,
        view,
        TrustedDeviceItem::Gfx950MfmaGlobalMatrixView,
    )
    .ok_or_else(|| rejected("Global FP4/8 load exact Global view"))?;
    let [format_ty, role_ty, matrix_brand, global_brand] = arguments.as_slice() else {
        return Err(rejected("Global FP4/8 load format role and two brands"));
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
        || instance.args.types().collect::<Vec<_>>() != [*matrix_brand, *global_brand]
    {
        return Err(rejected(
            "Global FP4/8 load original format role and instance",
        ));
    }
    let subgroup = rust_exact_reviewed_adt_arguments_v1(
        tcx,
        *matrix_brand,
        "fe2o3_device::execution::SubgroupBrand",
    )
    .ok_or_else(|| rejected("Global FP4/8 load retains exact subgroup brand"))?
    .types()
    .collect::<Vec<_>>();
    let [width, execution_brand, epoch] = subgroup.as_slice() else {
        return Err(rejected("Global FP4/8 load subgroup width brand epoch"));
    };
    let root = rust_kernel_brand_v1(tcx, *global_brand)
        .ok_or_else(|| rejected("Global FP4/8 load exact allocation root"))?;
    let execution = rust_execution_brand_v1(tcx, *execution_brand)
        .ok_or_else(|| rejected("Global FP4/8 load exact execution brand"))?;
    if rust_subgroup_width_v1(tcx, *width) != Some(64)
        || super::super::super::super::super::bind::matrix_global_brand(tcx, *matrix_brand, 0)
            != Some(root.ty)
        || execution.kernel != root.kernel
        || execution.target != root.target
        || execution.launch != root.launch
    {
        return Err(rejected(
            "Global FP4/8 load exact phase to allocation root relation",
        ));
    }
    let lane = rust_shared_reference_v1(inputs[1])
        .ok_or_else(|| rejected("Global FP4/8 load shared lane witness"))?;
    let lane_arguments = rust_trusted_adt_type_arguments_v1(tcx, lane, TrustedDeviceItem::WaveLane)
        .ok_or_else(|| rejected("Global FP4/8 load exact lane ADT"))?;
    if !matches!(lane_arguments.as_slice(), [lane_width, brand]
        if rust_is_exact_trusted_marker_v1(tcx, *lane_width, TrustedDeviceItem::Wave64) && brand == matrix_brand)
    {
        return Err(rejected(
            "Global FP4/8 load lane full subgroup brand and epoch",
        ));
    }
    let output = signature.output();
    let fragment =
        rust_trusted_adt_type_arguments_v1(tcx, output, TrustedDeviceItem::Gfx950MfmaFragment)
            .ok_or_else(|| rejected("Global FP4/8 load exact fragment ADT"))?;
    if fragment != [*format_ty, *role_ty, *matrix_brand] {
        return Err(rejected(
            "Global FP4/8 load fragment format role and full brand",
        ));
    }
    let TyKind::Adt(definition, view_arguments) = *view.kind() else {
        unreachable!()
    };
    let global_reference = definition
        .non_enum_variant()
        .fields
        .iter()
        .next()
        .map(|field| {
            tcx.try_normalize_erasing_regions(
                TypingEnv::fully_monomorphized(),
                field.ty(tcx, view_arguments),
            )
        })
        .transpose()
        .map_err(|_| rejected("Global FP4/8 load storage field normalization"))?
        .ok_or_else(|| rejected("Global FP4/8 load storage field"))?;
    let global = rust_shared_reference_v1(global_reference)
        .ok_or_else(|| rejected("Global FP4/8 load retained Global reference"))?;
    let memory = rust_capability_memory_view_v1(tcx, global)
        .ok_or_else(|| rejected("Global FP4/8 load branded allocation"))?;
    if memory.element != tcx.types.u8
        || memory.role != RustCapabilityMemoryRoleV1::ReadOnly
        || memory.brand.ty != root.ty
        || !layout::view(
            tcx,
            view,
            global_reference,
            *format_ty,
            *role_ty,
            *matrix_brand,
        )
    {
        return Err(rejected(
            "Global FP4/8 load one source byte per logical value and exact layout",
        ));
    }
    let registers = layout::fragment(tcx, output, *format_ty, *role_ty, *matrix_brand)
        .ok_or_else(|| rejected("Global FP4/8 load eight u32 registers and invariant fields"))?;
    let helpers = body::wrapper(
        tcx,
        instance,
        tcx.instance_mir(instance.def),
        inputs,
        output,
        registers,
        format,
        role,
    )
    .ok_or_else(|| rejected("Global FP4/8 load original lane pack fragment body"))?;
    Ok(Some(Load {
        root,
        matrix_brand: *matrix_brand,
        epoch: *epoch,
        inputs,
        view,
        lane,
        global_reference,
        global,
        output,
        registers,
        helpers,
    }))
}

pub(super) use carriage::validate as validate_canonical;

#[cfg(test)]
pub(in crate::collector::production_importer_v1) fn check_import<'tcx>(
    tcx: TyCtxt<'tcx>,
    plan: &ProductionSemanticPreflightPlanV1<'tcx>,
    imported: &ConstructedProductionSemanticMirV1,
) {
    tests::check_import(tcx, plan, imported);
}
