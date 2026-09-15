//! Exact source ABI of the reviewed unbranded DeviceMatrix terminal.
//! Scoped MatrixCapability adapters must remain real defined source bodies.

use super::*;

fn unbranded<'tcx>(tcx: TyCtxt<'tcx>, ty: Ty<'tcx>) -> bool {
    rust_exact_reviewed_adt_arguments_v1(tcx, ty, "fe2o3_device::context::UnbrandedCapability")
        .is_some_and(|arguments| arguments.is_empty())
}

fn operand<'tcx>(tcx: TyCtxt<'tcx>, ty: Ty<'tcx>, expected: TrustedDeviceItem) -> bool {
    let Some(arguments) =
        rust_trusted_adt_type_arguments_v1(tcx, ty, TrustedDeviceItem::Bf16MfmaFragment)
    else {
        return false;
    };
    matches!(arguments.as_slice(), [role, profile, distribution, width, brand]
        if rust_is_exact_trusted_marker_v1(tcx, *role, expected)
            && rust_is_exact_trusted_marker_v1(tcx, *profile, TrustedDeviceItem::Bf16MfmaProfile)
            && rust_is_exact_trusted_marker_v1(tcx, *distribution, TrustedDeviceItem::MfmaRegisterTile16x16)
            && rust_is_exact_trusted_marker_v1(tcx, *width, TrustedDeviceItem::Wave64)
            && unbranded(tcx, *brand))
}

pub(super) fn contracts<'tcx>(
    tcx: TyCtxt<'tcx>,
    inputs: &[Ty<'tcx>],
    output: Ty<'tcx>,
) -> Option<(
    SemanticMfmaOperandContractV1,
    SemanticMfmaOperandContractV1,
    SemanticMfmaAccumulatorContractV1,
)> {
    let [context, lhs, rhs, accumulator] = inputs else {
        return None;
    };
    let context = rust_shared_reference_v1(*context)?;
    if !rust_is_trusted_adt_v1(tcx, context, TrustedDeviceItem::DeviceMatrix)
        || !operand(tcx, *lhs, TrustedDeviceItem::MfmaOperandA)
        || !operand(tcx, *rhs, TrustedDeviceItem::MfmaOperandB)
        || *accumulator != output
    {
        return None;
    }
    let arguments = rust_trusted_adt_type_arguments_v1(
        tcx,
        *accumulator,
        TrustedDeviceItem::F32AccumulatorFragment,
    )?;
    if !matches!(arguments.as_slice(), [profile, distribution, width, brand]
        if rust_is_exact_trusted_marker_v1(tcx, *profile, TrustedDeviceItem::Bf16MfmaProfile)
            && rust_is_exact_trusted_marker_v1(tcx, *distribution, TrustedDeviceItem::MfmaAccumulatorRowMajor)
            && rust_is_exact_trusted_marker_v1(tcx, *width, TrustedDeviceItem::Wave64)
            && unbranded(tcx, *brand))
    {
        return None;
    }
    let operand = |role| SemanticMfmaOperandContractV1 {
        role,
        profile: SemanticMfmaProfileV1::Bf16F32M16N16K16,
        register_distribution: SemanticMfmaRegisterDistributionV1::Tile16x16,
        wave_width: 64,
    };
    Some((
        operand(SemanticMfmaOperandRoleV1::A),
        operand(SemanticMfmaOperandRoleV1::B),
        SemanticMfmaAccumulatorContractV1 {
            profile: SemanticMfmaProfileV1::Bf16F32M16N16K16,
            distribution: SemanticMfmaAccumulatorDistributionV1::RowMajor,
            wave_width: 64,
        },
    ))
}
