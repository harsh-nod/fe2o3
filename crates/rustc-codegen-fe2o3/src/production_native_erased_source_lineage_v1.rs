//! Original-N signed custody and independently replayed E, never N relabeling.
use super::*;
use fe2o3_lower_mir_kernel::ProductionUnitLocalErasedSourceOwnerV1 as ErasedSource;
use fe2o3_verifier::{
    NativeCompilerUnitLocalErasedSourceProofInputsV1,
    ValidatedNativeCompilerUnitLocalErasedSourceProofV1 as Proof,
    validate_native_compiler_unit_local_erased_source_proof_v1,
};

/// Move-only retained original ranked custody plus fresh source/N/E replay.
/// The actual-O join belongs to the consuming backend stage, not this packet.
#[allow(
    dead_code,
    reason = "retained source custody for a future protected-native consumer"
)]
pub(crate) struct PreparedErasedNativeSourceLineageV1 {
    ranked: AuthenticatedRankedVerificationRosterV1,
    proof: Proof,
    original_native_module: Vec<u8>,
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
        &self.proof
    }
    pub(crate) fn original_native_module(&self) -> &[u8] {
        &self.original_native_module
    }
    pub(crate) const fn grants_artifact_or_launch_authority(&self) -> bool {
        false
    }

    pub(crate) fn check_output_catalog_v1(
        &self,
        actual_output: &Catalog,
        budget: &mut Budget<'_>,
    ) -> Result<(), E> {
        check_catalog_v1(self.proof.source().catalog(), actual_output, budget)
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
    budget.charge_work(6)?;
    if budget.storage() < source.retained_storage_floor_v1() {
        return Err(Resource::Accounting.into());
    }
    packet::with_native_lineage_transfer_v1(budget, |budget| {
        let parts = prepare_native_source_packet_v1(
            NativeSourceRefV1::Erased(source),
            &ranked,
            || {
                std::mem::size_of::<PreparedErasedNativeSourceLineageV1>()
                    .checked_sub(std::mem::size_of::<Proof>())
                    .and_then(|n| {
                        n.checked_sub(
                            std::mem::size_of::<AuthenticatedRankedVerificationRosterV1>(),
                        )
                    })
                    .ok_or(E::Resource(Resource::Arithmetic))
            },
            budget,
            |inputs, budget| {
                let (proof, storage) = validate_native_compiler_unit_local_erased_source_proof_v1(
                    NativeCompilerUnitLocalErasedSourceProofInputsV1 {
                        original: inputs.source,
                        ranked_roots: inputs.ranked_roots,
                        erased: source.erased(),
                    },
                    budget,
                )
                .map_err(E::Replay)?;
                Ok((proof, storage.retained_storage()))
            },
        )?;
        Ok((
            PreparedErasedNativeSourceLineageV1 {
                ranked,
                proof: parts.proof,
                original_native_module: parts.native_module,
            },
            ErasedNativeSourceLineageStorageV1(parts.retained),
        ))
    })
}

#[cfg(test)]
#[path = "production_native_erased_source_lineage_v1_tests.rs"]
mod tests;
