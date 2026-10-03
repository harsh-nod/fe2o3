//! Synthetic source transport fixture, not compiler-origin or application authority.

#[allow(
    dead_code,
    reason = "the shared fixture supports additional unrelated source families"
)]
#[path = "../../../../tests/support/compiler_proof_inputs_v3.rs"]
mod compiler_proof_inputs_v3;

use fe2o3_compiler_lineage::{
    InertCanonicalSemanticMirReceiptV3, InertFormalMemoryReceiptV3, InertKernelIrReceiptV3,
    InertLineageContentIdentityV3, InertMiddleEndReceiptV3, InertMirToKirCorrespondenceReceiptV3,
    InertProofBindingAssociationInputsV4, InertProofBindingAssociationV4,
    InertProofBindingReceiptV3,
};
use fe2o3_verifier::{ValidatedCompilerProofInputsV4, validate_compiler_proof_inputs_v4};

pub fn source_inputs() -> ValidatedCompilerProofInputsV4 {
    let source = compiler_proof_inputs_v3::canonical_compiler_proof_inputs_v4_with_induction(0);
    validate_source_inputs(source)
}

#[allow(
    dead_code,
    reason = "capture is used by the public integration harness"
)]
pub fn captured_source_inputs(
    seed: u8,
) -> (
    ValidatedCompilerProofInputsV4,
    fe2o3_lower_mir_kernel::ProductionFormalMemoryOwnerV1,
) {
    let (source, owner) =
        compiler_proof_inputs_v3::canonical_compiler_proof_inputs_v4_with_captured_induction(seed);
    (validate_source_inputs(source), owner)
}

fn validate_source_inputs(
    source: compiler_proof_inputs_v3::CanonicalCompilerProofInputsV3,
) -> ValidatedCompilerProofInputsV4 {
    let evidence =
        compiler_proof_inputs_v3::canonical_verus_execution_evidence_v1(source.middle_end(), 0);
    let mir =
        InertCanonicalSemanticMirReceiptV3::from_canonical_preimage(source.semantic_mir().to_vec())
            .unwrap();
    let middle =
        InertMiddleEndReceiptV3::from_canonical_preimage(source.middle_end().to_vec()).unwrap();
    let kir = InertKernelIrReceiptV3::from_canonical_preimage(source.kernel_ir().to_vec()).unwrap();
    let correspondence = InertMirToKirCorrespondenceReceiptV3::from_canonical_preimage(
        source.correspondence().to_vec(),
    )
    .unwrap();
    let memory =
        InertFormalMemoryReceiptV3::from_canonical_preimage(source.formal_memory().to_vec())
            .unwrap();
    let identity = |sha: &[u8; 32], len| InertLineageContentIdentityV3::new(*sha, len).unwrap();
    let association = InertProofBindingAssociationV4::new(
        InertProofBindingAssociationInputsV4::new(
            identity(mir.identity().sha256(), mir.identity().byte_len()),
            identity(middle.identity().sha256(), middle.identity().byte_len()),
            identity(kir.identity().sha256(), kir.identity().byte_len()),
            identity(
                correspondence.identity().sha256(),
                correspondence.identity().byte_len(),
            ),
            identity(memory.identity().sha256(), memory.identity().byte_len()),
        ),
        &evidence,
    )
    .unwrap();
    let binding =
        InertProofBindingReceiptV3::from_canonical_preimage(association.canonical_bytes()).unwrap();
    validate_compiler_proof_inputs_v4(&binding, &mir, &middle, &kir, &correspondence, &memory)
        .unwrap()
}
