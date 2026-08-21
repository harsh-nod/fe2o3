use fe2o3_worker_v2_bundle::{WorkerV2FinalArtifactEvidenceV2, WorkerV2LoadEnvelopeV2};

fn inspect_final_artifact(evidence: &WorkerV2FinalArtifactEvidenceV2) {
    let _ = evidence.compiler_closure;
    let _ = evidence.publication_intent;
    let _ = evidence.backend_receipt;
    let _ = &evidence.published_claim;
    let _ = evidence.final_bytes;
    let _ = evidence.target;
    let _ = evidence.code_object_version;
    let _ = evidence.target_identity;
    let _ = evidence.abi_identity;
    let _ = evidence.descriptor_identity;
    let _ = evidence.symbol_identity;
    let _ = evidence.resource_identity;
    let _ = evidence.proof_or_inspection_identity;
}

fn inspect_load_envelope(envelope: &WorkerV2LoadEnvelopeV2) {
    let _ = &envelope.components;
    let _ = &envelope.final_artifact_evidence;
}

fn main() {}
