use fe2o3_artifact_transaction::ProducerIdentity;
use fe2o3_hsaco_finalize::{
    InspectedProtectedRawWorkerV2HsacoV1, InspectedRawWorkerV2HsacoV1,
    PreparedFinalizedProtectedWorkerV2HsacoV2, PreparedFinalizedWorkerV2HsacoV1,
    finalize_inspected_protected_worker_v2_hsaco_v2, finalize_inspected_worker_v2_hsaco_v1,
    prepare_finalized_protected_worker_v2_hsaco_publication_v2,
    prepare_finalized_worker_v2_hsaco_publication_v1,
    prepare_protected_worker_v2_hsaco_publication_v2, prepare_worker_v2_hsaco_publication_v1,
};

fn protected_into_v1_finalization(raw: InspectedProtectedRawWorkerV2HsacoV1) {
    let _ = finalize_inspected_worker_v2_hsaco_v1(raw);
}

fn v1_into_protected_finalization(raw: InspectedRawWorkerV2HsacoV1) {
    let _ = finalize_inspected_protected_worker_v2_hsaco_v2(raw);
}

fn protected_into_v1_publication(
    producer: &ProducerIdentity,
    raw: InspectedProtectedRawWorkerV2HsacoV1,
) {
    let _ = prepare_worker_v2_hsaco_publication_v1(producer, raw);
}

fn v1_into_protected_publication(producer: &ProducerIdentity, raw: InspectedRawWorkerV2HsacoV1) {
    let _ = prepare_protected_worker_v2_hsaco_publication_v2(producer, raw);
}

fn protected_finalized_into_v1_publication(
    producer: &ProducerIdentity,
    finalized: PreparedFinalizedProtectedWorkerV2HsacoV2,
) {
    let _ = prepare_finalized_worker_v2_hsaco_publication_v1(producer, finalized);
}

fn v1_finalized_into_protected_publication(
    producer: &ProducerIdentity,
    finalized: PreparedFinalizedWorkerV2HsacoV1,
) {
    let _ = prepare_finalized_protected_worker_v2_hsaco_publication_v2(producer, finalized);
}

fn main() {}
