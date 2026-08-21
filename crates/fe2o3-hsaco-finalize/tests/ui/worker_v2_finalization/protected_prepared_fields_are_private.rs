use fe2o3_hsaco_finalize::{
    MissingAuthenticatedProtectedDescriptorSourceEvidenceV2,
    PreparedFinalizedProtectedWorkerV2HsacoV2,
};

fn inspect_finalized_fields(prepared: &PreparedFinalizedProtectedWorkerV2HsacoV2) {
    let _ = prepared.identity;
    let _ = &prepared.raw;
    let _ = &prepared.finalized;
    let _ = prepared.finalized_output;
}

fn inspect_blocker_fields(blocker: &MissingAuthenticatedProtectedDescriptorSourceEvidenceV2) {
    let _ = &blocker.raw;
}

fn main() {}
