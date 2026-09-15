//! Branded FP4/FP8 zero construction, not lane issuance or numerical authority.
use super::*;

fn profile(
    expansion: Expansion,
) -> Option<(TrustedDeviceItem, TrustedDeviceItem, SemanticMfmaProfileV1)> {
    Some(match expansion {
        Expansion::Gfx950Fp4AccumulatorZero => (
            TrustedDeviceItem::Gfx950Fp4F32AccumulatorFragmentZero,
            TrustedDeviceItem::Gfx950Fp4E2M1Format,
            SemanticMfmaProfileV1::Fp4E2M1F32M16N16K128,
        ),
        Expansion::Gfx950Fp8AccumulatorZero => (
            TrustedDeviceItem::Gfx950F32AccumulatorFragmentZero,
            TrustedDeviceItem::Gfx950Fp8E4M3Format,
            SemanticMfmaProfileV1::Fp8E4M3F32M16N16K128,
        ),
        _ => return None,
    })
}

fn brand<'tcx>(
    tcx: TyCtxt<'tcx>,
    expansion: Expansion,
    inputs: &[Ty<'tcx>],
    output: Ty<'tcx>,
) -> Option<Ty<'tcx>> {
    let (_, format, _) = profile(expansion)?;
    let [reference] = inputs else {
        return None;
    };
    let lane = rust_shared_reference_v1(*reference)?;
    let lane_args = rust_trusted_adt_type_arguments_v1(tcx, lane, TrustedDeviceItem::WaveLane)?;
    let [width, lane_brand] = lane_args.as_slice() else {
        return None;
    };
    if !rust_is_exact_trusted_marker_v1(tcx, *width, TrustedDeviceItem::Wave64) {
        return None;
    }
    let subgroup = rust_exact_reviewed_adt_arguments_v1(
        tcx,
        *lane_brand,
        "fe2o3_device::execution::SubgroupBrand",
    )?
    .types()
    .collect::<Vec<_>>();
    let [subgroup_width, execution_brand, _epoch] = subgroup.as_slice() else {
        return None;
    };
    if rust_subgroup_width_v1(tcx, *subgroup_width) != Some(64)
        || rust_execution_brand_v1(tcx, *execution_brand).is_none()
        || accumulator(tcx, output, format)?.0 != *lane_brand
    {
        return None;
    }
    // The complete nominal brand, including epoch, must match the output.
    // Live lane provenance is checked separately by scoped source/SSA custody.
    Some(*lane_brand)
}

pub(in crate::collector::production_importer_v1) fn source_matches<'tcx>(
    tcx: TyCtxt<'tcx>,
    expansion: Expansion,
    inputs: &[Ty<'tcx>],
    output: Ty<'tcx>,
) -> bool {
    brand(tcx, expansion, inputs, output).is_some()
}

pub(in crate::collector::production_importer_v1) fn operation<'tcx>(
    tcx: TyCtxt<'tcx>,
    instance: Instance<'tcx>,
    expansion: Expansion,
    abi: &SemanticFunctionAbiV1,
    types: &[SemanticTypeDeclV1],
) -> Result<SemanticCompilerIntrinsicOperationV1, ProductionSemanticImportErrorV1> {
    let (item, _, profile) =
        profile(expansion).ok_or_else(|| rejected("branded zero exact format family"))?;
    if trusted_device_items::classify(tcx, instance.def_id()) != Some(item) {
        return Err(rejected("branded zero exact original source"));
    }
    let signature = source::signature(tcx, instance)?;
    let brand = brand(tcx, expansion, signature.inputs(), signature.output()).ok_or_else(|| {
        rejected("branded zero shared lane, matching full brand and accumulator layout")
    })?;
    if instance.args.types().collect::<Vec<_>>() != [brand] {
        return Err(rejected("branded zero original generic arguments"));
    }
    require_capability_memory_terminal_abi_v1(
        tcx,
        abi,
        types,
        signature.inputs(),
        signature.output(),
        &[SemanticSourceArgumentOwnershipV1::SharedBorrow],
    )?;
    Ok(
        SemanticCompilerIntrinsicOperationV1::F32MatrixAccumulatorZero {
            lane: pointer_pointee_v1(types, abi.source_input_types()[0])?,
            fragment: abi.source_output_type(),
            contract: SemanticMfmaAccumulatorContractV1 {
                profile,
                distribution: SemanticMfmaAccumulatorDistributionV1::RowMajor,
                wave_width: 64,
            },
        },
    )
}

pub(super) fn validate_carriage<'tcx>(
    tcx: TyCtxt<'tcx>,
    plan: &ProductionSemanticPreflightPlanV1<'tcx>,
    types: &[SemanticTypeDeclV1],
    callables: &[SemanticCallableDeclV1],
) -> Result<(), ProductionSemanticImportErrorV1> {
    for (index, terminal) in plan.terminal_producers().iter().enumerate() {
        if profile(terminal.expansion).is_none() {
            continue;
        }
        let signature = source::signature(tcx, terminal.instance)?;
        if !source_matches(
            tcx,
            terminal.expansion,
            signature.inputs(),
            signature.output(),
        ) {
            // Preserve the pre-existing legacy operation path unchanged.
            continue;
        }
        let Some(SemanticCallableDeclV1::CompilerIntrinsic {
            binding,
            operation: retained,
            ..
        }) = callables.get(plan.function_producers().len() + index)
        else {
            return Err(rejected("branded zero canonical callable"));
        };
        if binding.identity() != terminal.identities.function()
            || binding.abi().identity() != terminal.abi.identity
            || *retained
                != operation(
                    tcx,
                    terminal.instance,
                    terminal.expansion,
                    binding.abi(),
                    types,
                )?
        {
            return Err(rejected("branded zero original source/ABI carriage"));
        }
    }
    Ok(())
}

#[test]
fn branded_zero_adapter_excludes_matrix_issuance_and_other_formats() {
    assert!(profile(Expansion::Gfx950Fp4AccumulatorZero).is_some());
    assert!(profile(Expansion::Gfx950Fp8AccumulatorZero).is_some());
    for expansion in [
        Expansion::Gfx950MatrixContextCurrent,
        Expansion::Gfx950LdsTransposeTileCurrent,
        Expansion::F32MatrixAccumulatorZero,
        Expansion::Gfx950Fp4AccumulatorIntoValues,
        Expansion::Gfx950Fp8AccumulatorIntoValues,
        Expansion::Gfx950Fp4MultiplyAccumulate,
        Expansion::Gfx950Fp8MultiplyAccumulate,
        Expansion::Gfx950Fp4Fp8MultiplyAccumulate,
    ] {
        assert!(profile(expansion).is_none());
    }
}
