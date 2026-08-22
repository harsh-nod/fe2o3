#![cfg(all(
    target_os = "linux",
    feature = "worker-v3-envelope-integration-test-only"
))]

use fe2o3_artifact_transaction::retire_worker_v3_publication_intent_after_load_readiness_v1;
use fe2o3_worker_v2_bundle::{
    WorkerV3LoadEnvelopeV1, WorkerV3LoadEnvelopeWireV1, recover_worker_v3_load_envelope_v1,
};

#[path = "../../fe2o3-hsaco-finalize/tests/worker_v3_hsaco_admission.rs"]
mod worker_v3_fixture;

#[test]
fn completed_v3_publication_becomes_restartable_inert_envelope_custody() {
    let worker_v3_fixture::PublishedWorkerV3Fixture {
        directory,
        producer,
        attempt,
        published,
    } = worker_v3_fixture::published_worker_v3_fixture();
    let output_dir = directory.0.clone();
    let exact_artifact = published
        .recovered_evidence()
        .exact_finalized_hsaco()
        .to_vec();

    let envelope = WorkerV3LoadEnvelopeV1::from_published_hsaco_v1(published).unwrap();
    assert_eq!(envelope.exact_artifact_bytes(), exact_artifact);
    assert!(!envelope.grants_load_authority());
    assert!(!envelope.grants_launch_authority());

    let canonical = envelope.encode_canonical().unwrap();
    let inert = WorkerV3LoadEnvelopeWireV1::decode_canonical(&canonical).unwrap();
    inert
        .validate_reacquired_publication_lease_v1(envelope.current_publication_lease())
        .unwrap();
    assert_eq!(inert.encode_canonical().unwrap(), canonical);
    assert!(!inert.grants_publication_authority());
    assert!(!inert.grants_load_authority());
    assert!(!inert.grants_launch_authority());

    let intent = inert.publication_intent_record().identity();
    let readiness = envelope
        .persist_durable_replay_custody_v1(&output_dir)
        .unwrap();
    assert_eq!(readiness.exact_envelope_bytes(), canonical);
    assert!(!readiness.authenticates_descriptor_source());
    assert!(!readiness.establishes_hsa_readiness());
    assert!(!readiness.grants_load_authority());
    assert!(!readiness.grants_launch_authority());

    retire_worker_v3_publication_intent_after_load_readiness_v1(
        &output_dir,
        &producer,
        attempt,
        intent,
        readiness.receipt(),
    )
    .unwrap();
    drop(envelope);

    let recovered = recover_worker_v3_load_envelope_v1(&output_dir, attempt).unwrap();
    assert_eq!(recovered.receipt(), readiness.receipt());
    assert_eq!(recovered.wire().encode_canonical().unwrap(), canonical);
    assert_eq!(recovered.exact_artifact_bytes(), exact_artifact);
    assert!(!recovered.authenticates_descriptor_source());
    assert!(!recovered.grants_load_authority());
    assert!(!recovered.grants_launch_authority());
}
