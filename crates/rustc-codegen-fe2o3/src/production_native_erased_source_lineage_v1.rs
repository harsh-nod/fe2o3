//! Original-N signed custody and independently replayed E, never N relabeling.
use super::*;
use fe2o3_lower_mir_kernel::ProductionUnitLocalErasedSourceOwnerV1 as ErasedSource;
use fe2o3_verifier::ValidatedNativeCompilerUnitLocalErasedSourceProofV1 as Proof;

/// Move-only retained original ranked custody plus fresh source/N/E replay.
/// The actual-O join belongs to the consuming backend stage, not this packet.
#[allow(
    dead_code,
    reason = "retained source custody for a future protected-native consumer"
)]
pub(crate) struct PreparedErasedNativeSourceLineageV1 {
    ranked: AuthenticatedRankedVerificationRosterV1,
    packet: PreparedNativeSourceProofPacketV1<Proof>,
}

#[allow(
    dead_code,
    reason = "read-only custody getters, not publication authority"
)]
impl PreparedErasedNativeSourceLineageV1 {
    pub(crate) fn ranked(&self) -> &AuthenticatedRankedVerificationRosterV1 {
        &self.ranked
    }
    pub(crate) fn proof(&self) -> &Proof {
        self.packet.proof()
    }
    pub(crate) fn original_native_module(&self) -> &[u8] {
        self.packet.original_native_module()
    }
    pub(crate) const fn grants_artifact_or_launch_authority(&self) -> bool {
        false
    }

    pub(crate) fn check_output_catalog_v1(
        &self,
        actual_output: &Catalog,
        budget: &mut Budget<'_>,
    ) -> Result<(), E> {
        check_catalog_v1(
            self.packet.proof().source().catalog(),
            actual_output,
            budget,
        )
    }
}

fn check_catalog_v1(
    original_and_erased: &Catalog,
    output: &Catalog,
    budget: &mut Budget<'_>,
) -> Result<(), E> {
    budget.charge_work(
        65usize
            .checked_add(original_and_erased.canonical_bytes().len())
            .and_then(|n| n.checked_add(output.canonical_bytes().len()))
            .ok_or(Resource::Arithmetic)?,
    )?;
    if original_and_erased.semantic_source() != output.semantic_source()
        || original_and_erased.digest() != output.digest()
        || original_and_erased.canonical_bytes() != output.canonical_bytes()
    {
        return Err(E::Mismatch("original N/erased E/actual O catalog"));
    }
    Ok(())
}

/// Additional payload only; original N/E and the transferred roster stay live
/// under their caller-reserved receipts. Reserve before the next allocation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ErasedNativeSourceLineageStorageV1(usize);
impl ErasedNativeSourceLineageStorageV1 {
    pub(crate) const fn retained_storage(self) -> usize {
        self.0
    }
}

pub(crate) fn try_prepare_erased_native_source_lineage_v1(
    source: &ErasedSource,
    ranked: AuthenticatedRankedVerificationRosterV1,
    budget: &mut Budget<'_>,
) -> Result<
    (
        PreparedErasedNativeSourceLineageV1,
        ErasedNativeSourceLineageStorageV1,
    ),
    E,
> {
    packet::with_native_lineage_transfer_v1(budget, move |budget| {
        let packet = prepare_borrowed_erased_native_source_packet_v1(source, &ranked, budget)?;
        let retained = owned_packet::roster_wrapper_storage::<
            _,
            PreparedErasedNativeSourceLineageV1,
        >(&packet)?;
        budget.reserve_storage(retained)?;
        Ok((
            PreparedErasedNativeSourceLineageV1 { ranked, packet },
            ErasedNativeSourceLineageStorageV1(retained),
        ))
    })
}

#[cfg(test)]
#[path = "production_native_erased_source_lineage_v1_tests.rs"]
mod tests;
