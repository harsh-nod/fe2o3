//! Owned proof payload shared by consuming-roster and retained-roster stages.
use super::*;
use fe2o3_lower_mir_kernel::ProductionUnitLocalErasedSourceOwnerV1 as ErasedSource;
use fe2o3_verifier::{
    NativeCompilerUnitLocalErasedSourceProofInputsV1,
    ValidatedNativeCompilerUnitLocalErasedSourceProofV1 as ErasedProof,
    validate_native_compiler_unit_local_erased_source_proof_v1,
};
use std::mem::size_of;

#[cfg(test)]
#[path = "production_native_owned_source_packet_v1_tests.rs"]
mod tests;

/// No ranked-roster clone: the caller either retains or separately transfers it.
/// The proof retains independently reconstructed original source/N/(E), never O.
pub(crate) struct PreparedNativeSourceProofPacketV1<P> {
    proof: P,
    original_native_module: Vec<u8>,
    proof_storage: usize,
}
impl<P> PreparedNativeSourceProofPacketV1<P> {
    pub(crate) fn proof(&self) -> &P {
        &self.proof
    }
    pub(crate) fn original_native_module(&self) -> &[u8] {
        &self.original_native_module
    }
    pub(crate) fn retained_storage(&self) -> Result<usize, E> {
        if self.proof_storage < size_of::<P>() {
            return Err(Resource::Accounting.into());
        }
        packet_header::<P>()?
            .checked_add(self.proof_storage)
            .and_then(|n| n.checked_add(self.original_native_module.capacity()))
            .ok_or(Resource::Arithmetic.into())
    }
    fn from_parts(parts: packet::NativeSourcePacketPartsV1<P>) -> Result<Self, E> {
        let proof_storage = parts
            .retained
            .checked_sub(packet_header::<P>()?)
            .and_then(|n| n.checked_sub(parts.native_module.capacity()))
            .ok_or(Resource::Accounting)?;
        let value = Self {
            proof: parts.proof,
            original_native_module: parts.native_module,
            proof_storage,
        };
        if value.retained_storage()? != parts.retained {
            return Err(Resource::Accounting.into());
        }
        Ok(value)
    }
}

fn packet_header<P>() -> Result<usize, E> {
    size_of::<PreparedNativeSourceProofPacketV1<P>>()
        .checked_sub(size_of::<P>())
        .ok_or(Resource::Accounting.into())
}

/// The returned packet's complete logical receipt is unreserved. The original
/// source/ranked owners remain live and paid; inherited proof engines retain
/// their existing bounded domains, not a complete heap/RSS accounting claim.
pub(crate) fn prepare_borrowed_native_source_packet_v1(
    source: &ProductionSemanticKirOwnerV1,
    ranked: &AuthenticatedRankedVerificationRosterV1,
    budget: &mut Budget<'_>,
) -> Result<PreparedNativeSourceProofPacketV1<ValidatedNativeCompilerRankedSourceProofV1>, E> {
    budget.charge_work(6)?;
    let minimum = source
        .pre_ranked_retained_analysis_storage_v1()
        .ok_or(E::Mismatch("missing native source owner"))?;
    if budget.storage() < minimum {
        return Err(Resource::Accounting.into());
    }
    packet::with_native_lineage_transfer_v1(budget, |budget| {
        let parts = prepare_native_source_packet_v1(
            NativeSourceRefV1::Direct(source),
            ranked,
            packet_header::<ValidatedNativeCompilerRankedSourceProofV1>,
            budget,
            |inputs, budget| {
                let (proof, receipt) =
                    validate_native_compiler_ranked_source_proof_v1(inputs, budget)
                        .map_err(E::Replay)?;
                Ok((proof, receipt.retained_storage()))
            },
        )?;
        PreparedNativeSourceProofPacketV1::from_parts(parts)
    })
}

/// Original N signs the ranked packet; E is independently reconstructed by the
/// erased validator. The same ranked roster is only borrowed, not re-created.
pub(crate) fn prepare_borrowed_erased_native_source_packet_v1(
    source: &ErasedSource,
    ranked: &AuthenticatedRankedVerificationRosterV1,
    budget: &mut Budget<'_>,
) -> Result<PreparedNativeSourceProofPacketV1<ErasedProof>, E> {
    budget.charge_work(6)?;
    if budget.storage() < source.retained_storage_floor_v1() {
        return Err(Resource::Accounting.into());
    }
    packet::with_native_lineage_transfer_v1(budget, |budget| {
        let parts = prepare_native_source_packet_v1(
            NativeSourceRefV1::Erased(source),
            ranked,
            packet_header::<ErasedProof>,
            budget,
            |inputs, budget| {
                let (proof, receipt) = validate_native_compiler_unit_local_erased_source_proof_v1(
                    NativeCompilerUnitLocalErasedSourceProofInputsV1 {
                        original: inputs.source,
                        ranked_roots: inputs.ranked_roots,
                        erased: source.erased(),
                    },
                    budget,
                )
                .map_err(E::Replay)?;
                Ok((proof, receipt.retained_storage()))
            },
        )?;
        PreparedNativeSourceProofPacketV1::from_parts(parts)
    })
}

pub(super) fn roster_wrapper_storage<P, W>(
    packet: &PreparedNativeSourceProofPacketV1<P>,
) -> Result<usize, E> {
    let retained = packet.retained_storage()?;
    size_of::<W>()
        .checked_sub(size_of::<AuthenticatedRankedVerificationRosterV1>())
        .and_then(|n| n.checked_sub(size_of::<PreparedNativeSourceProofPacketV1<P>>()))
        .and_then(|n| n.checked_add(retained))
        .ok_or(Resource::Accounting.into())
}
