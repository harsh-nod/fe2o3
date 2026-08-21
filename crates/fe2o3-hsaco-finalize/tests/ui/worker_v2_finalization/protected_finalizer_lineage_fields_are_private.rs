use fe2o3_hsaco_finalize::ProtectedWorkerV2FinalizerLineageV2;

fn inspect_fields(lineage: &ProtectedWorkerV2FinalizerLineageV2) {
    let _ = lineage.route;
    let _ = lineage.source_evidence_identity;
    let _ = lineage.raw_inspection_identity;
    let _ = lineage.canonical_finalization_identity;
    let _ = &lineage.compiler_envelope_bytes;
    let _ = &lineage.symbol_manifest_bytes;
    let _ = &lineage.link_plan_bytes;
    let _ = &lineage.bootstrap_request_bytes;
    let _ = &lineage.bootstrap_response_bytes;
    let _ = &lineage.authorized_request_bytes;
    let _ = &lineage.authorized_response_bytes;
    let _ = &lineage.descriptor_observation_preimage;
    let _ = &lineage.abi_observation_preimage;
    let _ = &lineage.resource_observation_preimage;
}

fn main() {}
