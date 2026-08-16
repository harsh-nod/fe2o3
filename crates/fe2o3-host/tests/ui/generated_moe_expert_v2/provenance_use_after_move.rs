use fe2o3_host::{
    MoeRoutingCompletionReadbackProvenanceV2, MoeRoutingOutputCandidateV1,
    check_completed_moe_routing_readback_v2,
};

fn replay(
    provenance: MoeRoutingCompletionReadbackProvenanceV2,
    candidate: MoeRoutingOutputCandidateV1,
) {
    let _ = check_completed_moe_routing_readback_v2(provenance, candidate);
    let _ = check_completed_moe_routing_readback_v2(provenance, candidate);
}

fn main() {}
