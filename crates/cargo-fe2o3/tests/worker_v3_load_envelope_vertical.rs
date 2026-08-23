#![cfg(all(
    target_os = "linux",
    feature = "worker-v3-envelope-integration-test-only"
))]

use fe2o3_artifact_transaction::retire_worker_v3_publication_intent_after_load_readiness_v1;
use fe2o3_host::{
    __hardware_test::application_handoff_observed_context_fixture_v1,
    RecoveredWorkerV3AdmissionErrorV1, admit_recovered_worker_v3_descriptor_v1,
};
use fe2o3_kernel_descriptor::KernelId;
use fe2o3_worker_v2_bundle::{
    RecoveredWorkerV3LoadEnvelopeV1, WorkerV3LoadEnvelopeV1, WorkerV3LoadEnvelopeWireV1,
    recover_worker_v3_load_envelope_v1,
};

#[path = "../../fe2o3-hsaco-finalize/tests/worker_v3_hsaco_admission.rs"]
mod worker_v3_fixture;

fn recovered_host_fixture() -> (
    worker_v3_fixture::TestDirectory,
    RecoveredWorkerV3LoadEnvelopeV1,
) {
    let worker_v3_fixture::PublishedWorkerV3Fixture {
        directory,
        producer,
        attempt,
        published,
    } = worker_v3_fixture::published_worker_v3_fixture();
    let envelope = WorkerV3LoadEnvelopeV1::from_published_hsaco_v1(published).unwrap();
    let intent = envelope.wire().publication_intent_record().identity();
    let readiness = envelope
        .persist_durable_replay_custody_v1(&directory.0)
        .unwrap();
    retire_worker_v3_publication_intent_after_load_readiness_v1(
        &directory.0,
        &producer,
        attempt,
        intent,
        readiness.receipt(),
    )
    .unwrap();
    drop(envelope);
    let recovered = recover_worker_v3_load_envelope_v1(&directory.0, attempt).unwrap();
    (directory, recovered)
}

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

    let observed = application_handoff_observed_context_fixture_v1("gfx942:xnack-");
    let admitted = admit_recovered_worker_v3_descriptor_v1(
        recovered,
        KernelId::from_bytes([0xa1; 32]),
        &observed,
    )
    .unwrap();
    assert_eq!(admitted.descriptor().entry_name().as_str(), "vecadd");
    assert_eq!(
        admitted.descriptor().descriptor_symbol().as_str(),
        "vecadd.kd"
    );
    assert_eq!(admitted.physical_kernel().name(), "vecadd");
    assert_eq!(admitted.physical_kernel().symbol(), "vecadd.kd");
    assert_eq!(admitted.descriptor_binding().kernel_index(), 0);
    assert_eq!(admitted.target().to_string(), "gfx942:xnack-");
    assert_eq!(admitted.code_object_version().number(), 6);
    assert!(admitted.authenticates_descriptor_source());
    assert!(!admitted.authenticates_compiler_origin());
    assert!(!admitted.authenticates_verification_authority());
    assert!(!admitted.grants_load_authority());
    assert!(!admitted.grants_launch_authority());
    admitted.revalidate_currentness().unwrap();
}

#[test]
fn v3_host_admission_rejects_an_unknown_kernel_identity() {
    let (_directory, recovered) = recovered_host_fixture();
    let observed = application_handoff_observed_context_fixture_v1("gfx942:xnack-");
    assert!(matches!(
        admit_recovered_worker_v3_descriptor_v1(
            recovered,
            KernelId::from_bytes([0xff; 32]),
            &observed,
        ),
        Err(RecoveredWorkerV3AdmissionErrorV1::KernelNotFound)
    ));
}

#[test]
fn v3_host_admission_rejects_incompatible_observed_target_features() {
    let (_directory, recovered) = recovered_host_fixture();
    let observed = application_handoff_observed_context_fixture_v1("gfx942:xnack+");
    assert!(matches!(
        admit_recovered_worker_v3_descriptor_v1(
            recovered,
            KernelId::from_bytes([0xa1; 32]),
            &observed,
        ),
        Err(RecoveredWorkerV3AdmissionErrorV1::ObservedTargetMismatch)
    ));
}
