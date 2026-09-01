//! Live formal-memory conversion into the canonical V4 evidence contract.

use fe2o3_kernel_ir::InertCanonicalFormalMemoryObligationReceiptV1;
use fe2o3_mir_kir_contracts::InertCanonicalFormalMemoryAdmissionEvidenceV4;

use crate::{ProductionEvidenceConstructionErrorV1, ProductionFormalMemoryOwnerV1};

/// Replays one live formal owner and constructs exact, authority-free V4 evidence.
pub fn produce_formal_memory_admission_evidence_v4(
    owner: &ProductionFormalMemoryOwnerV1,
) -> Result<InertCanonicalFormalMemoryAdmissionEvidenceV4, ProductionEvidenceConstructionErrorV1> {
    owner
        .verify_equivalence()
        .map_err(ProductionEvidenceConstructionErrorV1::FormalMemory)?;
    let [kernel] = owner.kernels() else {
        return Err(ProductionEvidenceConstructionErrorV1::InvalidOwner(
            "V4 evidence requires one formal kernel",
        ));
    };
    if !kernel.obligations().inter_invocation_conflicts().is_empty() {
        return Err(ProductionEvidenceConstructionErrorV1::InvalidOwner(
            "formal owner retains inter-invocation conflicts",
        ));
    }
    let receipt =
        InertCanonicalFormalMemoryObligationReceiptV1::from_obligations(kernel.obligations())
            .map_err(ProductionEvidenceConstructionErrorV1::FormalReceipt)?;
    receipt
        .revalidate()
        .map_err(ProductionEvidenceConstructionErrorV1::FormalReceipt)?;
    let witness_invocation_count = kernel.witness_invocation_count();
    if witness_invocation_count == 0
        || kernel
            .witness_extents(owner.semantic_kir().module())
            .is_none_or(|extents| extents.contains(&0))
    {
        return Err(ProductionEvidenceConstructionErrorV1::InvalidOwner(
            "formal owner has an invalid structural witness",
        ));
    }
    InertCanonicalFormalMemoryAdmissionEvidenceV4::from_canonical_parts(
        owner.semantic_kir().canonical_kernel_ir_identity(),
        *receipt.identity().digest(),
        witness_invocation_count,
        receipt.canonical_bytes(),
    )
    .map_err(Into::into)
}
