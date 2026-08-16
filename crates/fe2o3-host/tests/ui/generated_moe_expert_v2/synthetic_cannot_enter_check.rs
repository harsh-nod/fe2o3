use fe2o3_host::{
    MoeRoutingOutputCandidateV1, check_completed_moe_routing_readback_v2,
};

struct SyntheticEvidence;

fn counterfeit(value: SyntheticEvidence, candidate: MoeRoutingOutputCandidateV1) {
    let _ = check_completed_moe_routing_readback_v2(value, candidate);
}

fn main() {}
