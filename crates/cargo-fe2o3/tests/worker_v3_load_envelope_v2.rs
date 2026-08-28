#![cfg(all(
    target_os = "linux",
    feature = "worker-v3-envelope-integration-test-only"
))]

use ed25519_dalek::SigningKey;
use fe2o3_artifact_transaction::{
    CompilerModuleHandoffSlotV3, InertCompilerExecutionSubjectV1,
    retire_worker_v3_publication_intent_after_load_readiness_v1,
};
use fe2o3_runtime_protocol::{
    COMPILER_EXECUTION_RECEIPT_CARRIAGE_BYTES_V1, CompilerExecutionAttestationChallengeV1,
    CompilerExecutionAttestationReceiptV1, CompilerExecutionAttestationRequestV1,
    CompilerExecutionIssuerMeasurementV1, CompilerExecutionIssuerPolicyV1,
    CompilerExecutionReceiptCarriageV1, CompilerExecutionReceiptPublicationAckV1,
    CompilerExecutionReceiptPublicationV1, MAX_WORKER_V3_LOAD_ENVELOPE_BYTES_V1,
    MAX_WORKER_V3_LOAD_ENVELOPE_BYTES_V2, MAX_WORKER_V3_LOAD_ENVELOPE_REPLAY_BYTES_V2,
    WORKER_V3_LOAD_ENVELOPE_MAGIC_V2, WorkerV3LoadEnvelopeBindingFieldV2,
    WorkerV3LoadEnvelopeCodecBudgetV2, WorkerV3LoadEnvelopeErrorV1, WorkerV3LoadEnvelopeErrorV2,
    WorkerV3LoadEnvelopeV2, WorkerV3LoadEnvelopeWireV1, WorkerV3LoadEnvelopeWireV2,
    recover_worker_v3_load_envelope_v1, recover_worker_v3_load_envelope_v2,
};

#[path = "fixtures/worker_v3_hsaco_admission.rs"]
mod worker_v3_fixture;

const V2_HEADER_BYTES: usize = 24;
const V2_CHECKSUM_BYTES: usize = 32;
const BODY_MUTATION_SAMPLES: usize = 384;

fn carriage_for_subject(
    subject: &InertCompilerExecutionSubjectV1,
    seed: u8,
) -> CompilerExecutionReceiptCarriageV1 {
    let signing_key = SigningKey::from_bytes(&[seed; 32]);
    let policy = CompilerExecutionIssuerPolicyV1::new(
        u64::from(seed),
        CompilerExecutionIssuerMeasurementV1::new([seed + 1; 32], 12_345).unwrap(),
        CompilerExecutionIssuerMeasurementV1::new([seed + 2; 32], 67_890).unwrap(),
        signing_key.verifying_key().to_bytes(),
    )
    .unwrap();
    let challenge =
        CompilerExecutionAttestationChallengeV1::new(&policy, subject, [seed + 3; 32], 1, [0; 32])
            .unwrap();
    let request = CompilerExecutionAttestationRequestV1::new(challenge, subject.clone()).unwrap();
    let receipt =
        CompilerExecutionAttestationReceiptV1::issue(&policy, &request, &signing_key).unwrap();
    let publication =
        CompilerExecutionReceiptPublicationV1::new([seed + 4; 32], [seed + 5; 32], receipt)
            .unwrap();
    let acknowledgment =
        CompilerExecutionReceiptPublicationAckV1::new(&publication, [seed + 6; 32]).unwrap();
    CompilerExecutionReceiptCarriageV1::new(policy, request, publication, acknowledgment).unwrap()
}

fn assert_complete_subject_association(
    expected: &InertCompilerExecutionSubjectV1,
    actual: &InertCompilerExecutionSubjectV1,
) {
    assert_eq!(actual.attempt(), expected.attempt());
    assert_eq!(actual.slot(), expected.slot());
    assert_eq!(
        actual.transaction_identity(),
        expected.transaction_identity()
    );
    assert_eq!(
        actual.rustc_invocation_sha256(),
        expected.rustc_invocation_sha256()
    );
    assert_eq!(actual.compiler_closure(), expected.compiler_closure());
    assert_eq!(
        actual.rustc_identity_inventory(),
        expected.rustc_identity_inventory()
    );
    assert_eq!(
        actual.rustc_preflight_plan(),
        expected.rustc_preflight_plan()
    );
    assert_eq!(actual.semantic_capsule(), expected.semantic_capsule());
    assert_eq!(
        actual.final_compiler_module_commitment(),
        expected.final_compiler_module_commitment()
    );
    assert_eq!(
        actual.compiler_module_handoff(),
        expected.compiler_module_handoff()
    );
    assert_eq!(
        actual.compiler_module_pair_binding(),
        expected.compiler_module_pair_binding()
    );
    assert_eq!(actual.outer_handoff(), expected.outer_handoff());
    assert_eq!(actual.identity(), expected.identity());
    assert_eq!(actual.canonical_bytes(), expected.canonical_bytes());
    assert!(
        actual
            .identity()
            .matches_canonical_bytes(actual.canonical_bytes())
    );
    assert_eq!(actual, expected);
}

fn assert_subject_and_carriage_are_authority_free(
    subject: &InertCompilerExecutionSubjectV1,
    carriage: &CompilerExecutionReceiptCarriageV1,
) {
    assert!(!subject.authenticates_compiler_execution());
    assert!(subject.requires_protected_execution_attestation());
    assert!(!subject.grants_compiler_authority());
    assert!(!subject.grants_publication_authority());
    assert!(!subject.grants_load_authority());
    assert!(!subject.grants_launch_authority());

    assert!(carriage.requires_protected_policy_verification());
    assert!(!carriage.grants_compiler_authority());
    assert!(!carriage.grants_load_authority());
    assert!(!carriage.grants_launch_authority());
    assert!(!carriage.publication().proves_durable_publication());
    assert!(!carriage.publication().grants_compiler_authority());
    assert!(!carriage.acknowledgment().proves_durable_publication());
    assert!(!carriage.acknowledgment().grants_compiler_authority());
}

#[test]
fn receipt_bearing_envelope_round_trips_with_complete_subject_association() {
    let worker_v3_fixture::PublishedWorkerV3Fixture {
        directory: _,
        producer: _,
        attempt: _,
        published,
    } = worker_v3_fixture::published_worker_v3_fixture();
    let expected_subject = published.compiler_execution_subject_v1().unwrap();
    assert_eq!(
        expected_subject.slot(),
        CompilerModuleHandoffSlotV3::Production
    );
    let expected_artifact = published
        .recovered_evidence()
        .exact_finalized_hsaco()
        .to_vec();
    let expected_carriage = carriage_for_subject(&expected_subject, 0x51);

    let envelope =
        WorkerV3LoadEnvelopeV2::from_published_hsaco_v1(published, expected_carriage.clone())
            .unwrap();
    assert_eq!(envelope.exact_artifact_bytes(), expected_artifact);
    assert!(!envelope.authenticates_compiler_origin());
    assert!(!envelope.grants_load_authority());
    assert!(!envelope.grants_launch_authority());

    let canonical = envelope.encode_canonical().unwrap();
    assert_eq!(&canonical[..8], &WORKER_V3_LOAD_ENVELOPE_MAGIC_V2);
    let decoded = WorkerV3LoadEnvelopeWireV2::decode_canonical(&canonical).unwrap();
    decoded
        .validate_reacquired_publication_lease_v2(envelope.current_publication_lease())
        .unwrap();
    assert_eq!(decoded.encode_canonical().unwrap(), canonical);
    assert_eq!(decoded.compiler_execution_receipt(), &expected_carriage);
    assert_eq!(
        decoded.compiler_execution_receipt().canonical_bytes(),
        expected_carriage.canonical_bytes()
    );
    let replay_subject = decoded
        .reconstructed_compiler_execution_subject_v1()
        .unwrap();
    assert_complete_subject_association(&expected_subject, &replay_subject);
    assert_complete_subject_association(
        &expected_subject,
        decoded.compiler_execution_receipt().request().subject(),
    );
    assert_subject_and_carriage_are_authority_free(
        &replay_subject,
        decoded.compiler_execution_receipt(),
    );
    assert!(!decoded.authenticates_compiler_origin());
    assert!(decoded.requires_protected_compiler_verification());
    assert!(!decoded.grants_load_authority());
    assert!(!decoded.grants_launch_authority());
    assert!(!decoded.replay().grants_publication_authority());
    assert!(!decoded.replay().grants_load_authority());
    assert!(!decoded.replay().grants_launch_authority());

    let replay_canonical = decoded.replay().encode_canonical().unwrap();
    let declared_replay_length =
        u32::from_le_bytes(canonical[20..V2_HEADER_BYTES].try_into().unwrap()) as usize;
    assert_eq!(declared_replay_length, replay_canonical.len());
    assert_eq!(
        &canonical[V2_HEADER_BYTES..V2_HEADER_BYTES + declared_replay_length],
        replay_canonical.as_slice()
    );
    assert_eq!(
        MAX_WORKER_V3_LOAD_ENVELOPE_REPLAY_BYTES_V2,
        MAX_WORKER_V3_LOAD_ENVELOPE_BYTES_V1
    );
    assert_eq!(
        MAX_WORKER_V3_LOAD_ENVELOPE_BYTES_V2,
        MAX_WORKER_V3_LOAD_ENVELOPE_BYTES_V1 + 2_114
    );
    assert_eq!(
        2_114,
        V2_HEADER_BYTES + COMPILER_EXECUTION_RECEIPT_CARRIAGE_BYTES_V1 + V2_CHECKSUM_BYTES
    );
    assert!(matches!(
        WorkerV3LoadEnvelopeWireV2::decode_canonical(&replay_canonical),
        Err(WorkerV3LoadEnvelopeErrorV2::BadMagic)
    ));
    assert!(matches!(
        WorkerV3LoadEnvelopeWireV1::decode_canonical(&canonical),
        Err(WorkerV3LoadEnvelopeErrorV1::BadMagic)
    ));
}

#[test]
fn receipt_bearing_envelope_persists_retires_and_recovers_exactly() {
    let worker_v3_fixture::PublishedWorkerV3Fixture {
        directory,
        producer,
        attempt,
        published,
    } = worker_v3_fixture::published_worker_v3_fixture();
    let expected_subject = published.compiler_execution_subject_v1().unwrap();
    let expected_artifact = published
        .recovered_evidence()
        .exact_finalized_hsaco()
        .to_vec();
    let expected_carriage = carriage_for_subject(&expected_subject, 0x61);
    let envelope =
        WorkerV3LoadEnvelopeV2::from_published_hsaco_v1(published, expected_carriage.clone())
            .unwrap();
    let intent = envelope
        .wire()
        .replay()
        .publication_intent_record()
        .identity();
    let canonical = envelope.encode_canonical().unwrap();

    let readiness = envelope
        .persist_durable_replay_custody_v2(&directory.0)
        .unwrap();
    let readiness_receipt = readiness.receipt();
    assert_eq!(readiness.exact_envelope_bytes(), canonical);
    assert!(!readiness.authenticates_descriptor_source());
    assert!(!readiness.grants_semantic_load_admission());
    assert!(!readiness.establishes_hsa_readiness());
    assert!(!readiness.grants_load_authority());
    assert!(!readiness.grants_launch_authority());

    retire_worker_v3_publication_intent_after_load_readiness_v1(
        &directory.0,
        &producer,
        attempt,
        intent,
        readiness_receipt,
    )
    .unwrap();
    drop(envelope);

    assert!(matches!(
        recover_worker_v3_load_envelope_v1(&directory.0, attempt),
        Err(WorkerV3LoadEnvelopeErrorV1::BadMagic)
    ));
    let recovered = recover_worker_v3_load_envelope_v2(&directory.0, attempt).unwrap();
    assert_eq!(recovered.receipt(), readiness_receipt);
    assert_eq!(recovered.exact_artifact_bytes(), expected_artifact);
    assert_eq!(recovered.wire().encode_canonical().unwrap(), canonical);
    assert_eq!(
        recovered.wire().compiler_execution_receipt(),
        &expected_carriage
    );
    let recovered_subject = recovered
        .wire()
        .reconstructed_compiler_execution_subject_v1()
        .unwrap();
    assert_complete_subject_association(&expected_subject, &recovered_subject);
    assert_complete_subject_association(
        &expected_subject,
        recovered
            .wire()
            .compiler_execution_receipt()
            .request()
            .subject(),
    );
    assert!(!recovered.authenticates_compiler_origin());
    assert!(!recovered.grants_load_authority());
    assert!(!recovered.grants_launch_authority());
}

#[test]
fn independently_valid_carriage_for_a_different_occurrence_is_rejected() {
    let first_directory = worker_v3_fixture::TestDirectory::new();
    let second_directory = worker_v3_fixture::TestDirectory::new();
    let first = worker_v3_fixture::publish_worker_v3_fixture_in_directory(&first_directory, 0x61);
    let second = worker_v3_fixture::publish_worker_v3_fixture_in_directory(&second_directory, 0x62);
    let first_subject = first.published.compiler_execution_subject_v1().unwrap();
    let second_subject = second.published.compiler_execution_subject_v1().unwrap();
    assert_ne!(first_subject, second_subject);

    let different_carriage = carriage_for_subject(&second_subject, 0x71);
    let independently_decoded =
        CompilerExecutionReceiptCarriageV1::decode(different_carriage.canonical_bytes()).unwrap();
    assert_eq!(independently_decoded, different_carriage);
    assert_complete_subject_association(&second_subject, independently_decoded.request().subject());
    assert_ne!(independently_decoded.request().subject(), &first_subject);

    assert!(matches!(
        WorkerV3LoadEnvelopeV2::from_published_hsaco_v1(first.published, independently_decoded,),
        Err(WorkerV3LoadEnvelopeErrorV2::BindingMismatch {
            field: WorkerV3LoadEnvelopeBindingFieldV2::CompilerExecutionSubject,
        })
    ));
    drop(second);
}

#[test]
fn codec_budget_rejects_too_small_wire_and_allocation_limits() {
    let worker_v3_fixture::PublishedWorkerV3Fixture {
        directory: _,
        producer: _,
        attempt: _,
        published,
    } = worker_v3_fixture::published_worker_v3_fixture();
    let subject = published.compiler_execution_subject_v1().unwrap();
    let envelope = WorkerV3LoadEnvelopeV2::from_published_hsaco_v1(
        published,
        carriage_for_subject(&subject, 0x31),
    )
    .unwrap();
    let canonical = envelope.encode_canonical().unwrap();
    let replay_length =
        u32::from_le_bytes(canonical[20..V2_HEADER_BYTES].try_into().unwrap()) as usize;

    let wire_budget = WorkerV3LoadEnvelopeCodecBudgetV2::new(canonical.len() - 1, usize::MAX);
    match WorkerV3LoadEnvelopeWireV2::decode_canonical_with_budget(&canonical, wire_budget) {
        Err(WorkerV3LoadEnvelopeErrorV2::WireLengthOutOfRange {
            actual, maximum, ..
        }) => {
            assert_eq!(actual, canonical.len());
            assert_eq!(maximum, canonical.len() - 1);
        }
        result => panic!("expected V2 wire-budget rejection, found {result:?}"),
    }

    let allocation_maximum = (replay_length * 2) - 1;
    let allocation_budget =
        WorkerV3LoadEnvelopeCodecBudgetV2::new(canonical.len(), allocation_maximum);
    match WorkerV3LoadEnvelopeWireV2::decode_canonical_with_budget(&canonical, allocation_budget) {
        Err(WorkerV3LoadEnvelopeErrorV2::AllocationBudgetExceeded { required, maximum }) => {
            assert_eq!(required, replay_length * 2);
            assert_eq!(maximum, allocation_maximum);
        }
        result => panic!("expected V2 allocation-budget rejection, found {result:?}"),
    }
}

#[test]
fn malformed_headers_lengths_checksums_and_sampled_byte_mutations_are_rejected() {
    let worker_v3_fixture::PublishedWorkerV3Fixture {
        directory: _,
        producer: _,
        attempt: _,
        published,
    } = worker_v3_fixture::published_worker_v3_fixture();
    let subject = published.compiler_execution_subject_v1().unwrap();
    let envelope = WorkerV3LoadEnvelopeV2::from_published_hsaco_v1(
        published,
        carriage_for_subject(&subject, 0x41),
    )
    .unwrap();
    let canonical = envelope.encode_canonical().unwrap();
    let replay_length = u32::from_le_bytes(canonical[20..24].try_into().unwrap());

    assert!(WorkerV3LoadEnvelopeWireV2::decode_canonical(&[]).is_err());
    assert!(
        WorkerV3LoadEnvelopeWireV2::decode_canonical(&canonical[..canonical.len() - 1]).is_err()
    );
    let mut extended = canonical.clone();
    extended.push(0);
    assert!(WorkerV3LoadEnvelopeWireV2::decode_canonical(&extended).is_err());

    let mut bad_magic = canonical.clone();
    bad_magic[0] ^= 1;
    assert!(matches!(
        WorkerV3LoadEnvelopeWireV2::decode_canonical(&bad_magic),
        Err(WorkerV3LoadEnvelopeErrorV2::BadMagic)
    ));
    let mut bad_version = canonical.clone();
    bad_version[8..10].copy_from_slice(&3_u16.to_le_bytes());
    assert!(matches!(
        WorkerV3LoadEnvelopeWireV2::decode_canonical(&bad_version),
        Err(WorkerV3LoadEnvelopeErrorV2::UnsupportedVersion { actual: 3 })
    ));
    let mut bad_flags = canonical.clone();
    bad_flags[10..12].copy_from_slice(&1_u16.to_le_bytes());
    assert!(matches!(
        WorkerV3LoadEnvelopeWireV2::decode_canonical(&bad_flags),
        Err(WorkerV3LoadEnvelopeErrorV2::UnsupportedFlags { actual: 1 })
    ));
    let mut bad_total = canonical.clone();
    bad_total[12..20].copy_from_slice(&((canonical.len() as u64) + 1).to_le_bytes());
    assert!(matches!(
        WorkerV3LoadEnvelopeWireV2::decode_canonical(&bad_total),
        Err(WorkerV3LoadEnvelopeErrorV2::InvalidTotalLength { .. })
    ));
    let mut zero_replay = canonical.clone();
    zero_replay[20..24].copy_from_slice(&0_u32.to_le_bytes());
    assert!(matches!(
        WorkerV3LoadEnvelopeWireV2::decode_canonical(&zero_replay),
        Err(WorkerV3LoadEnvelopeErrorV2::ReplayLengthOutOfRange { actual: 0, .. })
    ));
    let mut oversized_replay = canonical.clone();
    oversized_replay[20..24].copy_from_slice(
        &(u32::try_from(MAX_WORKER_V3_LOAD_ENVELOPE_REPLAY_BYTES_V2).unwrap() + 1).to_le_bytes(),
    );
    assert!(matches!(
        WorkerV3LoadEnvelopeWireV2::decode_canonical(&oversized_replay),
        Err(WorkerV3LoadEnvelopeErrorV2::ReplayLengthOutOfRange { .. })
    ));
    let mut inconsistent_replay = canonical.clone();
    inconsistent_replay[20..24].copy_from_slice(&(replay_length + 1).to_le_bytes());
    assert!(matches!(
        WorkerV3LoadEnvelopeWireV2::decode_canonical(&inconsistent_replay),
        Err(WorkerV3LoadEnvelopeErrorV2::InvalidTotalLength { .. })
    ));
    let mut bad_checksum = canonical.clone();
    let checksum_offset = bad_checksum.len() - V2_CHECKSUM_BYTES;
    bad_checksum[checksum_offset] ^= 1;
    assert!(matches!(
        WorkerV3LoadEnvelopeWireV2::decode_canonical(&bad_checksum),
        Err(WorkerV3LoadEnvelopeErrorV2::ChecksumMismatch)
    ));

    for offset in 0..V2_HEADER_BYTES {
        let mut mutated = canonical.clone();
        mutated[offset] ^= 1 << (offset % 8);
        assert!(
            WorkerV3LoadEnvelopeWireV2::decode_canonical(&mutated).is_err(),
            "header mutation at byte {offset} was accepted"
        );
    }
    for offset in canonical.len() - V2_CHECKSUM_BYTES..canonical.len() {
        let mut mutated = canonical.clone();
        mutated[offset] ^= 1 << (offset % 8);
        assert!(
            WorkerV3LoadEnvelopeWireV2::decode_canonical(&mutated).is_err(),
            "checksum mutation at byte {offset} was accepted"
        );
    }

    let body_start = V2_HEADER_BYTES;
    let body_end = canonical.len() - V2_CHECKSUM_BYTES;
    let body_len = body_end - body_start;
    let sample_count = BODY_MUTATION_SAMPLES.min(body_len);
    for sample in 0..sample_count {
        let relative = if sample_count == 1 {
            0
        } else {
            sample * (body_len - 1) / (sample_count - 1)
        };
        let offset = body_start + relative;
        let mut mutated = canonical.clone();
        mutated[offset] ^= 1 << (sample % 8);
        assert!(
            WorkerV3LoadEnvelopeWireV2::decode_canonical(&mutated).is_err(),
            "body mutation at byte {offset} was accepted"
        );
    }
}
