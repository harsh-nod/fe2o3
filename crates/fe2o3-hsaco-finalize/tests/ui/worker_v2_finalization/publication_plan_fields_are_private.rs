use fe2o3_hsaco_finalize::PreparedFinalizedWorkerV2HsacoPublicationV1;

fn substitute_private_fields(prepared: &PreparedFinalizedWorkerV2HsacoPublicationV1) {
    let _ = &prepared.finalized;
    let _ = prepared.producer_package;
    let _ = prepared.plan;
    let _ = prepared.upstream;
}

fn main() {}
