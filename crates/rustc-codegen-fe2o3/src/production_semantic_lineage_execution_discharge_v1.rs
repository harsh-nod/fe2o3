//! Retain complete formal-memory custody without reinterpreting legacy receipts.

use fe2o3_kernel_ir::InertFormalMemoryReceiptFormatV4;
use fe2o3_lower_mir_kernel::{
    InertCanonicalFormalMemoryAdmissionEvidenceV5, InertFormalMemoryAdmissionEvidenceFormatV5,
    ProductionCanonicalKernelIrIdentityV1, ProductionFormalMemoryOwnerV1,
};

use super::{LineageNeutralKirIdentityV1, ProductionSemanticLineageErrorV3};

fn live_error(error: impl std::fmt::Display) -> ProductionSemanticLineageErrorV3 {
    ProductionSemanticLineageErrorV3::LiveOwner(error.to_string())
}

pub(super) fn prepare_root(
    owner: &ProductionFormalMemoryOwnerV1,
    ordinal: usize,
) -> Result<Box<[u8]>, ProductionSemanticLineageErrorV3> {
    let kernel =
        owner
            .kernels()
            .get(ordinal)
            .ok_or(ProductionSemanticLineageErrorV3::AxisMismatch(
                "formal lineage root is out of range",
            ))?;
    if kernel.execution_discharges().is_empty() {
        let receipt =
            InertFormalMemoryReceiptFormatV4::from_current_obligations(kernel.obligations())
                .map_err(live_error)?;
        return Ok(receipt.into_canonical_bytes().into_boxed_slice());
    }
    let evidence =
        InertCanonicalFormalMemoryAdmissionEvidenceV5::from_live_owner_kernel(owner, ordinal)
            .map_err(live_error)?;
    validate_root(
        evidence.canonical_bytes(),
        owner.semantic_kir().canonical_kernel_ir_identity().into(),
        ordinal,
        kernel.obligations().kernel().as_str(),
    )?;
    Ok(evidence.canonical_bytes().to_vec().into_boxed_slice())
}

pub(super) fn prepare_singleton(
    owner: &ProductionFormalMemoryOwnerV1,
    neutral: ProductionCanonicalKernelIrIdentityV1,
    workgroups: &[(String, [u32; 3])],
) -> Result<InertFormalMemoryAdmissionEvidenceFormatV5, ProductionSemanticLineageErrorV3> {
    let evidence =
        InertFormalMemoryAdmissionEvidenceFormatV5::from_live_owner(owner).map_err(live_error)?;
    validate_singleton(evidence.canonical_bytes(), neutral, workgroups)?;
    Ok(evidence)
}

pub(super) fn validate_singleton(
    bytes: &[u8],
    neutral: ProductionCanonicalKernelIrIdentityV1,
    workgroups: &[(String, [u32; 3])],
) -> Result<(), ProductionSemanticLineageErrorV3> {
    // This checks inert custody from the revalidated live owner, not admission.
    // The protected proof-input verifier separately replays the current graph.
    let [(kernel, _)] = workgroups else {
        return Err(ProductionSemanticLineageErrorV3::AxisMismatch(
            "singleton formal lineage requires one exact root",
        ));
    };
    let evidence =
        InertFormalMemoryAdmissionEvidenceFormatV5::decode_current(bytes).map_err(live_error)?;
    if evidence.canonical_kernel_ir_identity() != neutral || evidence.grants_authority() {
        return Err(ProductionSemanticLineageErrorV3::AxisMismatch(
            "singleton formal lineage names a different neutral KIR or grants authority",
        ));
    }
    if let Some(discharged) = evidence.execution_discharged_v5() {
        return validate_discharged(discharged, neutral.into(), 0, kernel);
    }
    validate_raw(evidence.formal_obligation_receipt_bytes(), kernel)
}

pub(super) fn validate_root(
    bytes: &[u8],
    neutral: LineageNeutralKirIdentityV1,
    ordinal: usize,
    kernel: &str,
) -> Result<(), ProductionSemanticLineageErrorV3> {
    if bytes.starts_with(b"F2FMA5\0\0") {
        let evidence =
            InertCanonicalFormalMemoryAdmissionEvidenceV5::decode(bytes).map_err(live_error)?;
        validate_discharged(&evidence, neutral, ordinal, kernel)
    } else {
        // Per-root legacy payloads remain raw receipts, not singleton envelopes.
        validate_raw(bytes, kernel)
    }
}

fn validate_discharged(
    evidence: &InertCanonicalFormalMemoryAdmissionEvidenceV5,
    neutral: LineageNeutralKirIdentityV1,
    ordinal: usize,
    kernel: &str,
) -> Result<(), ProductionSemanticLineageErrorV3> {
    let kir = evidence.canonical_kernel_ir_identity();
    if kir.version() != neutral.version
        || kir.canonical_length() != neutral.canonical_length
        || kir.digest() != &neutral.digest
        || evidence.kernel_ordinal() != ordinal
        || evidence.kernel_id() != kernel
        || evidence.entry_id() != kernel
        || evidence.discharges().is_empty()
        || evidence.grants_authority()
    {
        return Err(ProductionSemanticLineageErrorV3::AxisMismatch(
            "execution-discharged formal lineage changed its current KIR or exact root",
        ));
    }
    validate_raw(evidence.formal_obligation_receipt_bytes(), kernel)
}

fn validate_raw(bytes: &[u8], kernel: &str) -> Result<(), ProductionSemanticLineageErrorV3> {
    let receipt =
        InertFormalMemoryReceiptFormatV4::decode_current(bytes.to_vec()).map_err(live_error)?;
    if receipt.kernel_id() != kernel || receipt.entry_id() != kernel || receipt.grants_authority() {
        return Err(ProductionSemanticLineageErrorV3::AxisMismatch(
            "formal lineage obligation receipt names a different kernel or entry",
        ));
    }
    Ok(())
}

#[cfg(test)]
#[path = "production_semantic_lineage_execution_discharge_v1_tests.rs"]
mod tests;
