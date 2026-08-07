use fe2o3_hsaco_finalize::PreparedFinalizedWorkerV2HsacoV1;

fn inspect_private_fields(prepared: &PreparedFinalizedWorkerV2HsacoV1) {
    let _ = &prepared.raw;
    let _ = &prepared.finalized;
    let _ = prepared.finalized_output;
}

fn main() {}
