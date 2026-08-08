use fe2o3_hsaco_finalize::PreparedFinalizedWorkerV2HsacoPublicationV1;

fn substitute_private_fields(prepared: &PreparedFinalizedWorkerV2HsacoPublicationV1) {
    let _ = &prepared.finalized;
    let _ = prepared.producer_package;
    let _ = prepared.plan;
    let _ = prepared.upstream;
    let intent = prepared.publication_intent();
    let _ = intent.route;
    let _ = intent.plan;
    let _ = intent.upstream;
    let _ = intent.raw_inspection;
    let _ = intent.canonical_finalization;
    let _ = intent.raw_snapshot;
    let _ = intent.finalized_snapshot;
}

fn main() {}
