//! Fresh nominal descriptor for a fully decoded, source-compatible final graph.
use super::*;
use fe2o3_lower_mir_kernel::{
    CanonicalOutputFormalSourceAnchorV1 as Anchor, CheckedDecodedExpandedSourceV1,
};

/// The checked view establishes compatibility. Exact source/file identity is
/// retained by the caller's original custody and compared through fresh bytes.
pub(crate) fn produce(
    checked: &CheckedDecodedExpandedSourceV1<'_>,
    roots: &[TypedDescriptorRootV1],
    target: &crate::production_target_v1::AuthenticatedProductionTargetV1,
    budget: &mut Budget<'_>,
) -> R<Vec<u8>> {
    for_profile(
        checked,
        roots,
        target.profile(),
        target.rustc_layout().default_pointer_width_bits(),
        budget,
    )
}

fn for_profile(
    checked: &CheckedDecodedExpandedSourceV1<'_>,
    roots: &[TypedDescriptorRootV1],
    profile: ProductionAmdTargetProfileV1,
    pointer_width: u16,
    budget: &mut Budget<'_>,
) -> R<Vec<u8>> {
    scoped(
        budget,
        || E::Panicked,
        |budget| {
            budget.reserve_storage(VIEW)?;
            budget.charge_work(2)?;
            let anchor = checked.source_anchor(budget)?;
            let (semantic, source_launch) = match anchor {
                Anchor::Direct(v) => (
                    v.semantic().semantic(),
                    v.source_launch_roster()
                        .ok_or(E::Mismatch("connected source launch"))?,
                ),
                Anchor::Erased(v) => (
                    v.original_source().semantic_ssa().source_semantic(),
                    v.original_source().source_launch(),
                ),
            };
            let view = CheckedDescriptorViewV1 {
                semantic,
                source_launch,
                neutral: checked.pre_bind(budget)?,
                bound: checked.bound_input(budget)?,
                output: checked.output(budget)?,
                kernels: checked.kernels(budget)?,
            };
            let _ = dialect_amdgcn::check_production_target_coordinate_preservation_v1(
                view.neutral,
                view.bound,
                profile,
                budget,
            )
            .map_err(|e| {
                E::Nominal(NominalDescriptorErrorV3::Descriptor(
                    CompilerDescriptorError::CheckedOutputTarget(e),
                ))
            })?;
            nominal_v3::validate_view(roots, &view, profile, budget).map_err(E::Nominal)?;
            let wire = codec::encode_subject(
                roots,
                view.semantic,
                view.output.module(),
                view.output.canonical().canonical_bytes(),
                view.kernels.len(),
                profile,
                pointer_width,
                FINAL_DOMAIN,
                "inert-expanded-production-final-native-v3",
                budget,
            )
            .map_err(E::Nominal)?;
            budget.reserve_storage(
                wire.capacity()
                    .checked_add(size_of::<Vec<u8>>())
                    .ok_or(Resource::Arithmetic)?,
            )?;
            budget.reserve_storage(
                DESCRIPTOR_TABLE_VIEW_STORAGE_V3
                    .checked_add(DESCRIPTOR_READER_SCRATCH_STORAGE_V3)
                    .ok_or(Resource::Arithmetic)?,
            )?;
            let table = decode_device_descriptor_table_v3(&wire, &mut |n| budget.charge_work(n))
                .map_err(|e| E::Nominal(NominalDescriptorErrorV3::Wire(e)))?;
            let (agreement, receipt) =
                fe2o3_verifier::check_nominal_source_abi_v3(anchor, &table, budget)
                    .map_err(|e| E::Nominal(NominalDescriptorErrorV3::Agreement(e)))?;
            budget.reserve_storage(receipt.retained_storage())?;
            agreement
                .verify_equivalence(budget)
                .map_err(|e| E::Nominal(NominalDescriptorErrorV3::Agreement(e)))?;
            if agreement.grants_artifact_or_launch_authority() {
                return Err(E::Mismatch("inert decoded nominal agreement"));
            }
            drop(agreement);
            drop(table);
            Ok(wire)
        },
    )
}

#[cfg(test)]
pub(crate) fn fixture(
    checked: &CheckedDecodedExpandedSourceV1<'_>,
    roots: &[TypedDescriptorRootV1],
    profile: ProductionAmdTargetProfileV1,
    budget: &mut Budget<'_>,
) -> R<Vec<u8>> {
    for_profile(checked, roots, profile, 64, budget)
}

#[cfg(test)]
pub(crate) fn live_fixture(
    owner: &Expanded,
    roots: &[TypedDescriptorRootV1],
    profile: ProductionAmdTargetProfileV1,
    budget: &mut Budget<'_>,
) -> R<Vec<u8>> {
    super::produce_for_profile(owner, roots, profile, 64, budget)
}
