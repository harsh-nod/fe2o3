//! Describes actual native proof owners without certifying protected currentness.
use crate::{other, require};
use fe2o3_compiler_execution_protocol::CompilerExecutionReceiptCarriageV3 as Carriage;
use fe2o3_functional_proof::FunctionalRefinementBoundaryV2 as Boundary;
use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as Budget;
use fe2o3_runtime_protocol::{
    NativeApplicationProofEvidencePartsV1 as Parts, NativeApplicationProofEvidenceV1 as Evidence,
    NativeApplicationProofInputsV1 as Inputs, NativeApplicationProofStorageV1 as Storage,
};
use fe2o3_verifier::NativeConditionalFillRefinementExecutionV1 as Proof;
use sha2::{Digest, Sha256};
use std::io;

/// Caller retains the original recovered V5 owner, authenticated analyzer execution,
/// imported proof, request inputs and decoded native carriage on this account.
/// The returned inert record cannot replace any of those original owners.
pub(crate) fn describe(
    inputs: &Inputs,
    carriage: &Carriage,
    proof: &Proof<'_>,
    budget: &mut Budget<'_>,
) -> io::Result<(Evidence, Storage)> {
    let analysis = proof.analysis_execution();
    let components = [
        proof.owner().handoff().canonical_bytes(),
        proof.owner().output().canonical().canonical_bytes(),
        analysis.request().canonical_bytes(),
        analysis.analysis().canonical_bytes(),
        analysis.canonical_receipt_bytes(),
        proof.generated_source(),
        proof.obligation_preimage(),
        proof.signed_receipt_wire(),
    ];
    let payload = analysis.request().exact_payload_bytes();
    let work = components
        .iter()
        .try_fold(payload.len(), |n, bytes| n.checked_add(bytes.len()))
        .and_then(|n| n.checked_mul(4))
        .and_then(|n| n.checked_add(4096))
        .ok_or_else(|| io::Error::other("native proof evidence work overflow"))?;
    budget.charge_work(work).map_err(other)?;
    require(
        proof.boundary() == Boundary::FinalKernelIrToGfx942FillDispatchConditional
            && proof.retains_strictly_imported_signed_receipt()
            && analysis.authenticates_analyzer_execution()
            && !proof.authenticates_currentness()
            && !proof.grants_launch_authority()
            && measured(payload) == inputs.payload(),
        "native retained proof boundary or payload differs",
    )?;
    let [
        native_handoff,
        final_kernel_ir,
        analysis_request,
        analysis_bundle,
        analysis_receipt,
        generated_source,
        obligation,
        signed_receipt,
    ] = components.map(measured);
    Evidence::new(
        inputs,
        Parts {
            native_handoff,
            final_kernel_ir,
            analysis_request,
            analysis_bundle,
            analysis_receipt,
            generated_source,
            obligation,
            signed_receipt,
            analysis_execution_identity: analysis.identity().sha256(),
            analysis_challenge: analysis.execution_challenge().as_bytes(),
            receipt_verifying_key: *proof.receipt_verifying_key(),
            carriage_identity: *carriage.identity().as_bytes(),
            subject_identity: *carriage.request().subject().identity().sha256(),
            policy_identity: *carriage.policy().identity().as_bytes(),
        },
        budget,
    )
    .map_err(other)
}
fn measured(bytes: &[u8]) -> ([u8; 32], u64) {
    (Sha256::digest(bytes).into(), bytes.len() as u64)
}
