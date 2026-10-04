//! Byte-only validation must agree with admission without acquiring its publication lock.
use super::*;
use fe2o3_host::{CheckedWorkerV3CompilerClosureV1, check_worker_v3_compiler_closure_v1};
use fe2o3_runtime_protocol::{WorkerV3LoadEnvelopeBindingFieldV2, WorkerV3LoadEnvelopeErrorV2};

pub(super) fn check_request<'request, K: CompilerGeneratedKernelExpectationV1>(
    request: &'request WorkerV3VerificationRequestV1<'_, K>,
) -> CheckedWorkerV3CompilerClosureV1<'request> {
    let envelope = request.load_envelope_evidence_view();
    let closure = check_worker_v3_compiler_closure_v1(
        envelope.exact_canonical_bytes(),
        request.finalized_hsaco_bytes(),
        request.descriptor().kernel_id(),
    )
    .unwrap();
    assert!(std::ptr::eq(
        closure.exact_canonical_envelope_bytes(),
        envelope.exact_canonical_bytes()
    ));
    assert!(std::ptr::eq(
        closure.finalized_hsaco_bytes(),
        request.finalized_hsaco_bytes()
    ));
    assert_eq!(closure.lineage_identity(), request.lineage_identity());
    assert_eq!(
        closure.finalizer_derivation(),
        request.finalizer_derivation()
    );
    assert!(!std::ptr::eq(
        closure.finalizer_derivation(),
        request.finalizer_derivation()
    ));
    assert_eq!(
        closure.semantic_compiler_handoff(),
        request.semantic_compiler_handoff()
    );
    assert_eq!(
        closure.compiler_execution_subject(),
        request.compiler_execution_subject()
    );
    assert_eq!(
        closure.compiler_execution_receipt_carriage(),
        request.compiler_execution_receipt_carriage()
    );
    assert_eq!(closure.descriptor(), request.descriptor());
    assert_eq!(closure.descriptor_binding(), request.descriptor_binding());
    assert_eq!(closure.target(), request.target());
    assert_eq!(closure.code_object_version(), request.code_object_version());
    assert!(!closure.grants_currentness_authority());
    assert!(!closure.authenticates_compiler_origin());
    assert!(!closure.grants_load_authority());
    assert!(!closure.grants_launch_authority());

    // Preserve the pre-extraction marker challenge encoding, not just two new callers' agreement.
    let mut challenge = Sha256::new();
    challenge.update(b"fe2o3.host.worker-v3-verification-challenge.v1\0");
    challenge.update(request.lineage_identity().as_bytes());
    challenge.update(K::KERNEL_BINDING_ID_V1);
    for name in [K::LOGICAL_NAME, K::EXPORT_NAME] {
        challenge.update((name.len() as u64).to_le_bytes());
        challenge.update(name.as_bytes());
    }
    challenge.update(K::PROFILE.generated_host_contract_identity());
    assert_eq!(
        challenge.finalize().as_slice(),
        request.challenge_identity().as_bytes()
    );
    closure
}

#[test]
fn copied_compiler_closure_remains_inert_after_publication_is_gone() {
    let (envelope, payload, directory) = {
        let (directory, recovered) = recovered_host_fixture();
        let current = recovered
            .current_publication_lease()
            .acquire_current_token()
            .unwrap();
        let evidence = recovered.canonical_evidence_view();
        let checked = check_worker_v3_compiler_closure_v1(
            evidence.exact_canonical_bytes(),
            current.exact_artifact_bytes(),
            KernelId::from_bytes(TEST_MARKER_BINDING),
        )
        .unwrap();
        current.revalidate_locked_currentness().unwrap();
        (
            checked.exact_canonical_envelope_bytes().to_vec(),
            checked.finalized_hsaco_bytes().to_vec(),
            directory.0.clone(),
        )
    };
    assert!(!directory.exists());
    let copied = check_worker_v3_compiler_closure_v1(
        &envelope,
        &payload,
        KernelId::from_bytes(TEST_MARKER_BINDING),
    )
    .unwrap();
    assert!(!copied.grants_currentness_authority());
    assert!(!copied.grants_launch_authority());
}

#[test]
fn compiler_closure_rejects_wrong_payload_size_bytes_and_selection() {
    let (_directory, recovered) = recovered_host_fixture();
    let evidence = recovered.canonical_evidence_view();
    let canonical = evidence.exact_canonical_bytes();
    let payload = recovered.exact_artifact_bytes();
    let kernel = KernelId::from_bytes(TEST_MARKER_BINDING);
    assert!(matches!(
        check_worker_v3_compiler_closure_v1(canonical, &payload[..payload.len() - 1], kernel),
        Err(RecoveredWorkerV3AdmissionErrorV1::FinalizedLengthMismatch)
    ));
    let mut altered = payload.to_vec();
    altered.push(0);
    assert!(matches!(
        check_worker_v3_compiler_closure_v1(canonical, &altered, kernel),
        Err(RecoveredWorkerV3AdmissionErrorV1::FinalizedLengthMismatch)
    ));
    altered.pop();
    altered[0] ^= 1;
    assert!(check_worker_v3_compiler_closure_v1(canonical, &altered, kernel).is_err());
    assert!(matches!(
        check_worker_v3_compiler_closure_v1(canonical, payload, KernelId::from_bytes([0x37; 32])),
        Err(RecoveredWorkerV3AdmissionErrorV1::KernelNotFound)
    ));
    let v1 = recovered.wire().replay().encode_canonical().unwrap();
    assert!(matches!(
        check_worker_v3_compiler_closure_v1(&v1, payload, kernel),
        Err(RecoveredWorkerV3AdmissionErrorV1::Envelope(
            WorkerV3LoadEnvelopeErrorV2::BadMagic
        ))
    ));
}

#[test]
fn compiler_closure_rejects_foreign_carriage_after_checksum_resealing() {
    let (_first_directory, first) = recovered_host_fixture();
    let second_directory = worker_v3_fixture::TestDirectory::new();
    let second = worker_v3_fixture::publish_worker_v3_fixture_in_directory(&second_directory, 0x62);
    let second_subject = second.published.compiler_execution_subject_v1().unwrap();
    assert_ne!(
        first
            .wire()
            .compiler_execution_receipt()
            .request()
            .subject(),
        &second_subject
    );
    let second_carriage = carriage_for_subject(&second_subject, 0x71);
    let mut canonical = first
        .canonical_evidence_view()
        .exact_canonical_bytes()
        .to_vec();
    let carriage = second_carriage.canonical_bytes();
    let checksum_offset = canonical.len() - 32;
    let carriage_offset = checksum_offset - carriage.len();
    canonical[carriage_offset..checksum_offset].copy_from_slice(carriage);
    let mut checksum = Sha256::new();
    checksum.update(b"FE2O3/WORKER-V3/LOAD-ENVELOPE-CHECKSUM/V2\0");
    checksum.update(&canonical[..checksum_offset]);
    canonical[checksum_offset..].copy_from_slice(&checksum.finalize());
    assert!(matches!(
        check_worker_v3_compiler_closure_v1(
            &canonical,
            first.exact_artifact_bytes(),
            KernelId::from_bytes(TEST_MARKER_BINDING),
        ),
        Err(RecoveredWorkerV3AdmissionErrorV1::Envelope(
            WorkerV3LoadEnvelopeErrorV2::BindingMismatch {
                field: WorkerV3LoadEnvelopeBindingFieldV2::CompilerExecutionSubject,
            }
        ))
    ));
}

#[test]
fn equal_hsaco_does_not_identify_the_complete_compiler_closure() {
    let (_primary_directory, primary) = recovered_host_fixture();
    let (_foreign_directory, foreign) = recover_published_worker_v3_fixture(
        worker_v3_fixture::published_worker_v3_fixture_with_llvm_build_identity("foreign-closure"),
    );
    assert_eq!(
        primary.exact_artifact_bytes(),
        foreign.exact_artifact_bytes()
    );
    let primary_evidence = primary.canonical_evidence_view();
    let foreign_evidence = foreign.canonical_evidence_view();
    let primary_checked = check_worker_v3_compiler_closure_v1(
        primary_evidence.exact_canonical_bytes(),
        primary.exact_artifact_bytes(),
        KernelId::from_bytes(TEST_MARKER_BINDING),
    )
    .unwrap();
    let foreign_checked = check_worker_v3_compiler_closure_v1(
        foreign_evidence.exact_canonical_bytes(),
        foreign.exact_artifact_bytes(),
        KernelId::from_bytes(TEST_MARKER_BINDING),
    )
    .unwrap();
    assert_ne!(
        primary_checked.lineage_identity(),
        foreign_checked.lineage_identity()
    );
    assert_ne!(
        primary_checked.finalizer_derivation(),
        foreign_checked.finalizer_derivation()
    );
}
