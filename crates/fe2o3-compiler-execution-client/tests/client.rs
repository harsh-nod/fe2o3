use std::mem;
use std::os::fd::{AsFd, AsRawFd, FromRawFd, OwnedFd};
use std::thread;
use std::time::Duration;

use ed25519_dalek::{Signer, SigningKey};
use fe2o3_artifact_transaction::{
    INERT_COMPILER_EXECUTION_SUBJECT_BYTES_V1, INERT_COMPILER_EXECUTION_SUBJECT_MAGIC_V1,
    INERT_COMPILER_EXECUTION_SUBJECT_VERSION_V1, InertCompilerExecutionSubjectV1,
};
use fe2o3_compiler_execution_client::{
    CompilerExecutionClientErrorV1, CompilerExecutionClientV1,
    CompilerExecutionCurrentRecordChallengeV1, CompilerExecutionReceiptRecoveryV1,
};
use fe2o3_compiler_execution_protocol::{
    CompilerExecutionAttestationChallengeV1, CompilerExecutionAttestationReceiptV1,
    CompilerExecutionAttestationRequestV1, CompilerExecutionCurrentRecordAttestationV3,
    CompilerExecutionCurrentRecordVerificationErrorV3,
    CompilerExecutionCurrentRecordVerificationV3, CompilerExecutionExternalAnchorTransactionV1,
    CompilerExecutionIssuerMeasurementV1, CompilerExecutionIssuerPolicyV1,
    CompilerExecutionReceiptCarriageV1, CompilerExecutionReceiptPublicationAckV1,
    CompilerExecutionReceiptPublicationV1, CompilerExecutionServicePublishDispositionV1,
    CompilerExecutionServiceRequestKindV1, CompilerExecutionServiceRequestV1,
    CompilerExecutionServiceResponseV1, MAX_COMPILER_EXECUTION_SERVICE_REQUEST_BYTES_V1,
    MAX_COMPILER_EXECUTION_SERVICE_RESPONSE_BYTES_V1,
};
use fe2o3_external_anchor_protocol::{
    AnchorPositionV1, AnchorTransitionReceiptV1, AnchoredStateV1, CallerNonceV1, HashChainHeadV1,
    PinnedAnchorKeyV1, UnsignedAnchorObservationV1,
};
use sha2::{Digest, Sha256};

const SUBJECT_IDENTITY_DOMAIN: &[u8] = b"FE2O3/INERT-COMPILER-EXECUTION-SUBJECT/V1\0";
const COMPILER_CLOSURE_IDENTITY_DOMAIN: &[u8] = b"fe2o3-compiler-closure-identity-v2\0";

#[derive(Clone)]
struct Fixture {
    signing_key: SigningKey,
    anchor_signing_key: SigningKey,
    policy: CompilerExecutionIssuerPolicyV1,
    subject: InertCompilerExecutionSubjectV1,
    challenge: CompilerExecutionAttestationChallengeV1,
    request: CompilerExecutionAttestationRequestV1,
    publication: CompilerExecutionReceiptPublicationV1,
    acknowledgment: CompilerExecutionReceiptPublicationAckV1,
    carriage: CompilerExecutionReceiptCarriageV1,
}

impl Fixture {
    fn new() -> Self {
        Self::with_subject(0x20)
    }

    fn with_subject(subject_seed: u8) -> Self {
        let key = SigningKey::from_bytes(&[0x51; 32]);
        let anchor_signing_key = SigningKey::from_bytes(&[0x52; 32]);
        let policy = CompilerExecutionIssuerPolicyV1::new(
            7,
            CompilerExecutionIssuerMeasurementV1::new([0x61; 32], 123).unwrap(),
            CompilerExecutionIssuerMeasurementV1::new([0x62; 32], 456).unwrap(),
            key.verifying_key().to_bytes(),
            anchor_signing_key.verifying_key().to_bytes(),
        )
        .unwrap();
        let subject = subject(subject_seed);
        let challenge =
            CompilerExecutionAttestationChallengeV1::new(&policy, &subject, [0x63; 32], 1, [0; 32])
                .unwrap();
        let request =
            CompilerExecutionAttestationRequestV1::new(challenge.clone(), subject.clone()).unwrap();
        let receipt =
            CompilerExecutionAttestationReceiptV1::issue(&policy, &request, &key).unwrap();
        let publication =
            CompilerExecutionReceiptPublicationV1::new([0x64; 32], [0x65; 32], receipt).unwrap();
        let acknowledgment =
            CompilerExecutionReceiptPublicationAckV1::new(&publication, [0x66; 32]).unwrap();
        let carriage = CompilerExecutionReceiptCarriageV1::new(
            policy.clone(),
            request.clone(),
            publication.clone(),
            acknowledgment.clone(),
        )
        .unwrap();
        Self {
            signing_key: key,
            anchor_signing_key,
            policy,
            subject,
            challenge,
            request,
            publication,
            acknowledgment,
            carriage,
        }
    }

    fn anchor_receipt(&self) -> AnchorTransitionReceiptV1 {
        let transaction = CompilerExecutionExternalAnchorTransactionV1::new(
            self.policy.clone(),
            self.request.clone(),
            self.publication.clone(),
        )
        .unwrap();
        let key = PinnedAnchorKeyV1::from_bytes(self.anchor_signing_key.verifying_key().to_bytes())
            .unwrap();
        let pending = AnchoredStateV1::from_local_state(0, HashChainHeadV1::from_bytes([0; 32]))
            .prepare(transaction.external_anchor_digest(), &key)
            .unwrap()
            .begin_advance(CallerNonceV1::from_bytes([0x67; 32]), &key)
            .unwrap();
        let unsigned = UnsignedAnchorObservationV1::from_challenge(
            pending.challenge(),
            AnchorPositionV1::Proposed,
        );
        let signature = self.anchor_signing_key.sign(&unsigned.signing_bytes());
        AnchorTransitionReceiptV1::new(
            pending.challenge().clone(),
            &unsigned.attach_signature(signature.to_bytes()),
            &key,
        )
        .unwrap()
    }

    fn currentness_receipt(
        &self,
        carriage: &CompilerExecutionReceiptCarriageV1,
        commit_receipt: &AnchorTransitionReceiptV1,
        verification_challenge: [u8; 32],
        position: AnchorPositionV1,
    ) -> AnchorTransitionReceiptV1 {
        let challenge =
            CompilerExecutionCurrentRecordVerificationV3::external_anchor_currentness_challenge(
                carriage,
                commit_receipt,
                verification_challenge,
            )
            .unwrap();
        let key = PinnedAnchorKeyV1::from_bytes(self.anchor_signing_key.verifying_key().to_bytes())
            .unwrap();
        let unsigned = UnsignedAnchorObservationV1::from_challenge(&challenge, position);
        let signature = self.anchor_signing_key.sign(&unsigned.signing_bytes());
        AnchorTransitionReceiptV1::new(
            challenge,
            &unsigned.attach_signature(signature.to_bytes()),
            &key,
        )
        .unwrap()
    }
}

#[derive(Clone, Copy)]
enum DurableStage {
    Ready,
    Prepared,
    Issued,
    Published,
}

#[test]
fn fresh_acquisition_runs_the_complete_bounded_lifecycle() {
    let fixture = Fixture::new();
    let (client, service) = socket_pair(libc::SOCK_SEQPACKET);
    let handle = spawn_service(service, fixture.clone(), DurableStage::Ready);
    let carriage = CompilerExecutionClientV1::admit(client, Duration::from_secs(1))
        .unwrap()
        .acquire(&fixture.policy, fixture.subject.clone())
        .unwrap();
    assert_eq!(carriage, fixture.carriage);
    assert_eq!(handle.join().unwrap(), 5);
}

#[test]
fn acquisition_resumes_prepared_and_issued_journal_states() {
    for (stage, packets) in [(DurableStage::Prepared, 4), (DurableStage::Issued, 3)] {
        let fixture = Fixture::new();
        let (client, service) = socket_pair(libc::SOCK_SEQPACKET);
        let handle = spawn_service(service, fixture.clone(), stage);
        let carriage = CompilerExecutionClientV1::admit(client, Duration::from_secs(1))
            .unwrap()
            .acquire(&fixture.policy, fixture.subject.clone())
            .unwrap();
        assert_eq!(carriage, fixture.carriage);
        assert_eq!(handle.join().unwrap(), packets);
    }
}

#[test]
fn exact_current_recovery_is_one_terminal_packet() {
    let fixture = Fixture::new();
    let (client, service) = socket_pair(libc::SOCK_SEQPACKET);
    let handle = spawn_service(service, fixture.clone(), DurableStage::Published);
    let carriage = CompilerExecutionClientV1::admit(client, Duration::from_secs(1))
        .unwrap()
        .acquire(&fixture.policy, fixture.subject.clone())
        .unwrap();
    assert_eq!(carriage, fixture.carriage);
    assert_eq!(handle.join().unwrap(), 1);
}

#[test]
fn recovery_only_cancels_cleanly_after_canonical_absence() {
    let fixture = Fixture::new();
    let (client, service) = socket_pair(libc::SOCK_SEQPACKET);
    let handle = spawn_service(service, fixture.clone(), DurableStage::Ready);
    let result = CompilerExecutionClientV1::admit(client, Duration::from_secs(1))
        .unwrap()
        .recover_only(&fixture.policy, fixture.subject.clone())
        .unwrap();
    match result {
        CompilerExecutionReceiptRecoveryV1::Absent {
            sequence,
            rollback_anchor,
        } => {
            assert_eq!(sequence, 1);
            assert_eq!(rollback_anchor, [0; 32]);
        }
        CompilerExecutionReceiptRecoveryV1::Recovered(_) => {
            panic!("unexpected recovered receipt")
        }
    }
    assert_eq!(handle.join().unwrap(), 2);
}

#[test]
fn recovery_only_returns_the_complete_current_carriage() {
    let fixture = Fixture::new();
    let (client, service) = socket_pair(libc::SOCK_SEQPACKET);
    let handle = spawn_service(service, fixture.clone(), DurableStage::Published);
    let result = CompilerExecutionClientV1::admit(client, Duration::from_secs(1))
        .unwrap()
        .recover_only(&fixture.policy, fixture.subject.clone())
        .unwrap();
    assert_eq!(result.into_carriage().unwrap(), fixture.carriage);
    assert_eq!(handle.join().unwrap(), 1);
}

#[test]
fn exact_current_verification_is_one_terminal_packet() {
    let fixture = Fixture::new();
    let (client, service) = socket_pair(libc::SOCK_SEQPACKET);
    let handle = spawn_service(service, fixture.clone(), DurableStage::Published);
    let verification = CompilerExecutionClientV1::admit(client, Duration::from_secs(1))
        .unwrap()
        .verify_current_only(&fixture.policy, fixture.carriage.clone())
        .unwrap();
    assert!(verification.authenticates_pinned_signing_key());
    assert!(verification.authenticates_expected_challenge());
    assert_eq!(
        verification.verification().carriage_identity(),
        *fixture.carriage.identity().as_bytes()
    );
    assert_eq!(
        verification
            .verification()
            .protected_policy_verification_identity(),
        [0x91; 32]
    );
    assert_eq!(
        verification
            .verification()
            .protected_worker_ledger_verification_identity(),
        [0x92; 32]
    );
    assert!(!verification.authenticates_protected_current_record());
    assert!(verification.authenticates_external_anchor_commit());
    assert_ne!(
        verification.external_rollback_verification_identity(),
        [0; 32]
    );
    assert!(verification.authenticates_external_rollback_currentness());
    assert!(!verification.grants_authority());
    assert_eq!(handle.join().unwrap(), 1);
}

#[test]
fn caller_owned_current_verification_challenge_is_bound_exactly() {
    let fixture = Fixture::new();
    let (client, service) = socket_pair(libc::SOCK_SEQPACKET);
    let handle = spawn_service(service, fixture.clone(), DurableStage::Published);
    let expected_challenge_bytes = [0xa7; 32];
    let expected_challenge =
        CompilerExecutionCurrentRecordChallengeV1::from_bytes(expected_challenge_bytes).unwrap();
    assert_eq!(expected_challenge.as_bytes(), &expected_challenge_bytes);
    assert!(!expected_challenge.grants_authority());
    let verification = CompilerExecutionClientV1::admit(client, Duration::from_secs(1))
        .unwrap()
        .verify_current_only_with_challenge(
            &fixture.policy,
            fixture.carriage.clone(),
            expected_challenge,
        )
        .unwrap();
    assert_eq!(
        verification.attestation().challenge(),
        expected_challenge_bytes
    );
    assert!(verification.authenticates_expected_challenge());
    assert!(!verification.grants_authority());
    assert_eq!(handle.join().unwrap(), 1);
}

#[test]
fn caller_owned_current_verification_challenge_rejects_zero() {
    assert!(matches!(
        CompilerExecutionCurrentRecordChallengeV1::from_bytes([0; 32]),
        Err(CompilerExecutionCurrentRecordVerificationErrorV3::ZeroChallenge)
    ));
}

#[test]
fn current_verification_carriage_substitution_fails_closed() {
    let fixture = Fixture::new();
    let substituted = Fixture::with_subject(0x21);
    let (client, service) = socket_pair(libc::SOCK_SEQPACKET);
    let policy = fixture.policy.clone();
    let handle = thread::spawn(move || {
        let request = receive_request(&service);
        assert_eq!(
            request.kind(),
            CompilerExecutionServiceRequestKindV1::VerifyCurrent
        );
        let verification_challenge = request.verification_challenge().unwrap();
        let commit_receipt = substituted.anchor_receipt();
        let currentness_receipt = substituted.currentness_receipt(
            &substituted.carriage,
            &commit_receipt,
            verification_challenge,
            AnchorPositionV1::Proposed,
        );
        let verification = CompilerExecutionCurrentRecordVerificationV3::new(
            &substituted.carriage,
            commit_receipt,
            currentness_receipt,
            verification_challenge,
            [0x91; 32],
            [0x92; 32],
        )
        .unwrap();
        let attestation = CompilerExecutionCurrentRecordAttestationV3::issue(
            &substituted.policy,
            &substituted.carriage,
            verification,
            verification_challenge,
            &substituted.signing_key,
        )
        .unwrap();
        let response =
            CompilerExecutionServiceResponseV1::verified_current(request.identity(), attestation)
                .unwrap();
        send_raw(&service, response.canonical_bytes());
    });
    let error = CompilerExecutionClientV1::admit(client, Duration::from_secs(1))
        .unwrap()
        .verify_current_only(&policy, fixture.carriage)
        .unwrap_err();
    assert!(matches!(
        error,
        CompilerExecutionClientErrorV1::CurrentRecord(
            CompilerExecutionCurrentRecordVerificationErrorV3::VerificationMismatch
        )
    ));
    handle.join().unwrap();
}

#[test]
fn stale_current_verification_challenge_fails_closed() {
    let fixture = Fixture::new();
    let (client, service) = socket_pair(libc::SOCK_SEQPACKET);
    let service_fixture = fixture.clone();
    let expected_challenge_bytes = [0xa8; 32];
    let handle = thread::spawn(move || {
        let request = receive_request(&service);
        let mut stale_challenge = request.verification_challenge().unwrap();
        assert_eq!(stale_challenge, expected_challenge_bytes);
        stale_challenge[0] ^= 0x80;
        let commit_receipt = service_fixture.anchor_receipt();
        let currentness_receipt = service_fixture.currentness_receipt(
            &service_fixture.carriage,
            &commit_receipt,
            stale_challenge,
            AnchorPositionV1::Proposed,
        );
        let verification = CompilerExecutionCurrentRecordVerificationV3::new(
            &service_fixture.carriage,
            commit_receipt,
            currentness_receipt,
            stale_challenge,
            [0x91; 32],
            [0x92; 32],
        )
        .unwrap();
        let attestation = CompilerExecutionCurrentRecordAttestationV3::issue(
            &service_fixture.policy,
            &service_fixture.carriage,
            verification,
            stale_challenge,
            &service_fixture.signing_key,
        )
        .unwrap();
        let response =
            CompilerExecutionServiceResponseV1::verified_current(request.identity(), attestation)
                .unwrap();
        send_raw(&service, response.canonical_bytes());
    });
    let error = CompilerExecutionClientV1::admit(client, Duration::from_secs(1))
        .unwrap()
        .verify_current_only_with_challenge(
            &fixture.policy,
            fixture.carriage,
            CompilerExecutionCurrentRecordChallengeV1::from_bytes(expected_challenge_bytes)
                .unwrap(),
        )
        .unwrap_err();
    assert!(matches!(
        error,
        CompilerExecutionClientErrorV1::CurrentRecord(
            CompilerExecutionCurrentRecordVerificationErrorV3::ChallengeMismatch
        )
    ));
    handle.join().unwrap();
}

#[test]
fn response_request_identity_substitution_fails_closed() {
    let fixture = Fixture::new();
    let (client, service) = socket_pair(libc::SOCK_SEQPACKET);
    let policy = fixture.policy.clone();
    let handle = thread::spawn(move || {
        let request = receive_request(&service);
        assert_eq!(
            request.kind(),
            CompilerExecutionServiceRequestKindV1::Recover
        );
        let unrelated = CompilerExecutionServiceRequestV1::inspect(&policy);
        let response = CompilerExecutionServiceResponseV1::receipt_absent(
            unrelated.identity(),
            &policy,
            1,
            [0; 32],
        )
        .unwrap();
        send_raw(&service, response.canonical_bytes());
    });
    let error = CompilerExecutionClientV1::admit(client, Duration::from_secs(1))
        .unwrap()
        .acquire(&fixture.policy, fixture.subject.clone())
        .unwrap_err();
    assert!(matches!(
        error,
        CompilerExecutionClientErrorV1::RequestIdentityMismatch
    ));
    handle.join().unwrap();
}

#[test]
fn recovered_subject_substitution_fails_closed() {
    let fixture = Fixture::new();
    let substituted = Fixture::with_subject(0x21);
    let (client, service) = socket_pair(libc::SOCK_SEQPACKET);
    let carriage = substituted.carriage.clone();
    let handle = thread::spawn(move || {
        let request = receive_request(&service);
        let response =
            CompilerExecutionServiceResponseV1::recovered(request.identity(), carriage).unwrap();
        send_raw(&service, response.canonical_bytes());
    });
    let error = CompilerExecutionClientV1::admit(client, Duration::from_secs(1))
        .unwrap()
        .acquire(&fixture.policy, fixture.subject.clone())
        .unwrap_err();
    assert!(matches!(
        error,
        CompilerExecutionClientErrorV1::SubjectOrPolicyMismatch
    ));
    handle.join().unwrap();
}

#[test]
fn durable_position_change_between_absence_and_inspection_fails_closed() {
    let fixture = Fixture::new();
    let (client, service) = socket_pair(libc::SOCK_SEQPACKET);
    let policy = fixture.policy.clone();
    let handle = thread::spawn(move || {
        let recover = receive_request(&service);
        let absent = CompilerExecutionServiceResponseV1::receipt_absent(
            recover.identity(),
            &policy,
            1,
            [0; 32],
        )
        .unwrap();
        send_raw(&service, absent.canonical_bytes());
        let inspect = receive_request(&service);
        let changed =
            CompilerExecutionServiceResponseV1::ready(inspect.identity(), &policy, 2, [0x71; 32])
                .unwrap();
        send_raw(&service, changed.canonical_bytes());
    });
    let error = CompilerExecutionClientV1::admit(client, Duration::from_secs(1))
        .unwrap()
        .acquire(&fixture.policy, fixture.subject.clone())
        .unwrap_err();
    assert!(matches!(
        error,
        CompilerExecutionClientErrorV1::DurableStateChanged
    ));
    handle.join().unwrap();
}

#[test]
fn absolute_deadline_applies_to_the_complete_session() {
    let fixture = Fixture::new();
    let (client, service) = socket_pair(libc::SOCK_SEQPACKET);
    let handle = thread::spawn(move || {
        let _request = receive_request(&service);
        thread::sleep(Duration::from_millis(50));
    });
    let error = CompilerExecutionClientV1::admit(client, Duration::from_millis(5))
        .unwrap()
        .acquire(&fixture.policy, fixture.subject.clone())
        .unwrap_err();
    assert!(matches!(error, CompilerExecutionClientErrorV1::Timeout));
    handle.join().unwrap();
}

#[test]
fn oversized_and_ancillary_responses_fail_closed() {
    let fixture = Fixture::new();
    let (client, service) = socket_pair(libc::SOCK_SEQPACKET);
    let handle = thread::spawn(move || {
        let _request = receive_request(&service);
        send_raw(
            &service,
            &vec![0_u8; MAX_COMPILER_EXECUTION_SERVICE_RESPONSE_BYTES_V1 + 1],
        );
    });
    let error = CompilerExecutionClientV1::admit(client, Duration::from_secs(1))
        .unwrap()
        .acquire(&fixture.policy, fixture.subject.clone())
        .unwrap_err();
    assert!(matches!(
        error,
        CompilerExecutionClientErrorV1::PacketTruncated
    ));
    handle.join().unwrap();

    let (client, service) = socket_pair(libc::SOCK_SEQPACKET);
    let policy = fixture.policy.clone();
    let handle = thread::spawn(move || {
        let request = receive_request(&service);
        let response = CompilerExecutionServiceResponseV1::receipt_absent(
            request.identity(),
            &policy,
            1,
            [0; 32],
        )
        .unwrap();
        send_with_descriptor(&service, response.canonical_bytes());
    });
    let error = CompilerExecutionClientV1::admit(client, Duration::from_secs(1))
        .unwrap()
        .acquire(&fixture.policy, fixture.subject.clone())
        .unwrap_err();
    assert!(matches!(
        error,
        CompilerExecutionClientErrorV1::AncillaryData
    ));
    handle.join().unwrap();
}

#[test]
fn admission_rejects_zero_timeout_stream_and_unconnected_sockets() {
    let (client, _service) = socket_pair(libc::SOCK_SEQPACKET);
    let raw = client.as_raw_fd();
    let admitted = CompilerExecutionClientV1::admit(client, Duration::from_secs(1)).unwrap();
    let flags = unsafe { libc::fcntl(raw, libc::F_GETFD) };
    assert_ne!(flags & libc::FD_CLOEXEC, 0);
    drop(admitted);

    let (client, _service) = socket_pair(libc::SOCK_SEQPACKET);
    assert!(matches!(
        CompilerExecutionClientV1::admit(client, Duration::ZERO),
        Err(CompilerExecutionClientErrorV1::InvalidTimeout)
    ));

    let (stream, _peer) = socket_pair(libc::SOCK_STREAM);
    assert!(matches!(
        CompilerExecutionClientV1::admit(stream, Duration::from_secs(1)),
        Err(CompilerExecutionClientErrorV1::NotSeqpacket)
    ));

    let socket = unsafe {
        let raw = libc::socket(libc::AF_UNIX, libc::SOCK_SEQPACKET | libc::SOCK_CLOEXEC, 0);
        assert!(raw >= 0);
        OwnedFd::from_raw_fd(raw)
    };
    assert!(matches!(
        CompilerExecutionClientV1::admit(socket, Duration::from_secs(1)),
        Err(CompilerExecutionClientErrorV1::NamedOrNonUnixPeer)
    ));
}

fn spawn_service(
    service: OwnedFd,
    fixture: Fixture,
    initial: DurableStage,
) -> thread::JoinHandle<usize> {
    thread::spawn(move || {
        let mut stage = initial;
        let mut packets = 0;
        loop {
            let request = receive_request(&service);
            packets += 1;
            let (response, terminal) = match request.kind() {
                CompilerExecutionServiceRequestKindV1::Recover => {
                    assert_eq!(request.subject(), Some(&fixture.subject));
                    if matches!(stage, DurableStage::Published) {
                        (
                            CompilerExecutionServiceResponseV1::recovered(
                                request.identity(),
                                fixture.carriage.clone(),
                            )
                            .unwrap(),
                            true,
                        )
                    } else {
                        let (sequence, anchor) = stage_position(stage, &fixture);
                        (
                            CompilerExecutionServiceResponseV1::receipt_absent(
                                request.identity(),
                                &fixture.policy,
                                sequence,
                                anchor,
                            )
                            .unwrap(),
                            false,
                        )
                    }
                }
                CompilerExecutionServiceRequestKindV1::Inspect => {
                    let response = match stage {
                        DurableStage::Ready => CompilerExecutionServiceResponseV1::ready(
                            request.identity(),
                            &fixture.policy,
                            1,
                            [0; 32],
                        ),
                        DurableStage::Prepared => CompilerExecutionServiceResponseV1::prepared(
                            request.identity(),
                            &fixture.policy,
                            fixture.challenge.clone(),
                        ),
                        DurableStage::Issued => CompilerExecutionServiceResponseV1::issued(
                            request.identity(),
                            &fixture.policy,
                            fixture.publication.clone(),
                        ),
                        DurableStage::Published => panic!("published state recovered terminally"),
                    }
                    .unwrap();
                    (response, false)
                }
                CompilerExecutionServiceRequestKindV1::Prepare => {
                    assert!(matches!(stage, DurableStage::Ready));
                    stage = DurableStage::Prepared;
                    (
                        CompilerExecutionServiceResponseV1::prepared(
                            request.identity(),
                            &fixture.policy,
                            fixture.challenge.clone(),
                        )
                        .unwrap(),
                        false,
                    )
                }
                CompilerExecutionServiceRequestKindV1::Issue => {
                    assert!(matches!(stage, DurableStage::Prepared));
                    assert_eq!(request.request(), Some(&fixture.request));
                    stage = DurableStage::Issued;
                    (
                        CompilerExecutionServiceResponseV1::issued(
                            request.identity(),
                            &fixture.policy,
                            fixture.publication.clone(),
                        )
                        .unwrap(),
                        false,
                    )
                }
                CompilerExecutionServiceRequestKindV1::Publish => {
                    assert!(matches!(stage, DurableStage::Issued));
                    assert_eq!(request.request(), Some(&fixture.request));
                    assert_eq!(request.publication(), Some(&fixture.publication));
                    stage = DurableStage::Published;
                    (
                        CompilerExecutionServiceResponseV1::published(
                            request.identity(),
                            &fixture.policy,
                            fixture.acknowledgment.clone(),
                            CompilerExecutionServicePublishDispositionV1::Advanced,
                        )
                        .unwrap(),
                        true,
                    )
                }
                CompilerExecutionServiceRequestKindV1::Cancel => {
                    let (sequence, anchor) = stage_position(stage, &fixture);
                    (
                        CompilerExecutionServiceResponseV1::cancelled(
                            request.identity(),
                            &fixture.policy,
                            sequence,
                            anchor,
                        )
                        .unwrap(),
                        true,
                    )
                }
                CompilerExecutionServiceRequestKindV1::VerifyCurrent => {
                    assert!(matches!(stage, DurableStage::Published));
                    assert_eq!(request.carriage(), Some(&fixture.carriage));
                    let verification_challenge = request.verification_challenge().unwrap();
                    let commit_receipt = fixture.anchor_receipt();
                    let currentness_receipt = fixture.currentness_receipt(
                        &fixture.carriage,
                        &commit_receipt,
                        verification_challenge,
                        AnchorPositionV1::Proposed,
                    );
                    let verification = CompilerExecutionCurrentRecordVerificationV3::new(
                        &fixture.carriage,
                        commit_receipt,
                        currentness_receipt,
                        verification_challenge,
                        [0x91; 32],
                        [0x92; 32],
                    )
                    .unwrap();
                    let attestation = CompilerExecutionCurrentRecordAttestationV3::issue(
                        &fixture.policy,
                        &fixture.carriage,
                        verification,
                        verification_challenge,
                        &fixture.signing_key,
                    )
                    .unwrap();
                    (
                        CompilerExecutionServiceResponseV1::verified_current(
                            request.identity(),
                            attestation,
                        )
                        .unwrap(),
                        true,
                    )
                }
            };
            send_raw(&service, response.canonical_bytes());
            if terminal {
                return packets;
            }
        }
    })
}

fn stage_position(stage: DurableStage, fixture: &Fixture) -> (u64, [u8; 32]) {
    match stage {
        DurableStage::Ready => (1, [0; 32]),
        DurableStage::Prepared => (
            fixture.challenge.sequence(),
            fixture.challenge.prior_rollback_anchor(),
        ),
        DurableStage::Issued => (
            fixture.publication.receipt().sequence(),
            fixture.publication.receipt().prior_rollback_anchor(),
        ),
        DurableStage::Published => (
            fixture.acknowledgment.sequence() + 1,
            fixture.acknowledgment.current_rollback_anchor(),
        ),
    }
}

fn socket_pair(socket_type: i32) -> (OwnedFd, OwnedFd) {
    let mut descriptors = [-1; 2];
    let result = unsafe {
        libc::socketpair(
            libc::AF_UNIX,
            socket_type | libc::SOCK_CLOEXEC,
            0,
            descriptors.as_mut_ptr(),
        )
    };
    assert_eq!(result, 0);
    unsafe {
        (
            OwnedFd::from_raw_fd(descriptors[0]),
            OwnedFd::from_raw_fd(descriptors[1]),
        )
    }
}

fn receive_request(service: &OwnedFd) -> CompilerExecutionServiceRequestV1 {
    let mut bytes = [0_u8; MAX_COMPILER_EXECUTION_SERVICE_REQUEST_BYTES_V1];
    let received = unsafe {
        libc::recv(
            service.as_raw_fd(),
            bytes.as_mut_ptr().cast(),
            bytes.len(),
            0,
        )
    };
    assert!(received > 0);
    CompilerExecutionServiceRequestV1::decode(&bytes[..received as usize]).unwrap()
}

fn send_raw(service: &OwnedFd, bytes: &[u8]) {
    let sent = unsafe {
        libc::send(
            service.as_raw_fd(),
            bytes.as_ptr().cast(),
            bytes.len(),
            libc::MSG_NOSIGNAL,
        )
    };
    assert_eq!(sent, bytes.len() as isize);
}

fn send_with_descriptor(service: &OwnedFd, bytes: &[u8]) {
    let pipe = rustix::pipe::pipe_with(rustix::pipe::PipeFlags::CLOEXEC).unwrap();
    let mut vector = libc::iovec {
        iov_base: bytes.as_ptr().cast_mut().cast(),
        iov_len: bytes.len(),
    };
    let control_len = unsafe { libc::CMSG_SPACE(mem::size_of::<i32>() as u32) } as usize;
    let mut control = vec![0_u8; control_len];
    let mut header = unsafe { mem::zeroed::<libc::msghdr>() };
    header.msg_iov = &mut vector;
    header.msg_iovlen = 1;
    header.msg_control = control.as_mut_ptr().cast();
    header.msg_controllen = control.len();
    unsafe {
        let message = libc::CMSG_FIRSTHDR(&header);
        assert!(!message.is_null());
        (*message).cmsg_level = libc::SOL_SOCKET;
        (*message).cmsg_type = libc::SCM_RIGHTS;
        (*message).cmsg_len = libc::CMSG_LEN(mem::size_of::<i32>() as u32) as usize;
        libc::CMSG_DATA(message)
            .cast::<i32>()
            .write_unaligned(pipe.0.as_fd().as_raw_fd());
        let sent = libc::sendmsg(service.as_raw_fd(), &header, libc::MSG_NOSIGNAL);
        assert_eq!(sent, bytes.len() as isize);
    }
}

fn subject(seed: u8) -> InertCompilerExecutionSubjectV1 {
    let closure_pins = [
        [seed; 32],
        [seed + 1; 32],
        [seed + 2; 32],
        [seed + 3; 32],
        [seed + 4; 32],
        [seed + 5; 32],
    ];
    let mut closure_digest = Sha256::new();
    closure_digest.update(COMPILER_CLOSURE_IDENTITY_DOMAIN);
    closure_digest.update(1_u16.to_le_bytes());
    for pin in closure_pins {
        closure_digest.update(pin);
    }
    let closure_identity: [u8; 32] = closure_digest.finalize().into();
    let mut bytes = [0_u8; INERT_COMPILER_EXECUTION_SUBJECT_BYTES_V1];
    let mut offset = 0;
    put(
        &mut bytes,
        &mut offset,
        &INERT_COMPILER_EXECUTION_SUBJECT_MAGIC_V1,
    );
    put(
        &mut bytes,
        &mut offset,
        &INERT_COMPILER_EXECUTION_SUBJECT_VERSION_V1.to_le_bytes(),
    );
    put(&mut bytes, &mut offset, &0_u16.to_le_bytes());
    put(
        &mut bytes,
        &mut offset,
        &(INERT_COMPILER_EXECUTION_SUBJECT_BYTES_V1 as u64).to_le_bytes(),
    );
    put(&mut bytes, &mut offset, &0_u32.to_le_bytes());
    put(&mut bytes, &mut offset, &9_u64.to_le_bytes());
    put(&mut bytes, &mut offset, &[seed + 6; 16]);
    put(&mut bytes, &mut offset, &[seed + 7; 32]);
    bytes[offset] = 0;
    offset += 8;
    put(&mut bytes, &mut offset, &[seed + 8; 32]);
    put(&mut bytes, &mut offset, &[seed + 9; 32]);
    for pin in closure_pins {
        put(&mut bytes, &mut offset, &pin);
    }
    put(&mut bytes, &mut offset, &1_u16.to_le_bytes());
    put(&mut bytes, &mut offset, &closure_identity);
    for axis in 0_u8..7 {
        put(&mut bytes, &mut offset, &[seed + 10 + axis; 32]);
        put(
            &mut bytes,
            &mut offset,
            &(1_000_u64 + u64::from(axis)).to_le_bytes(),
        );
    }
    let identity = digest(SUBJECT_IDENTITY_DOMAIN, &bytes[..offset]);
    put(&mut bytes, &mut offset, &identity);
    assert_eq!(offset, bytes.len());
    InertCompilerExecutionSubjectV1::decode(&bytes).unwrap()
}

fn digest(domain: &[u8], bytes: &[u8]) -> [u8; 32] {
    let mut digest = Sha256::new();
    digest.update(domain);
    digest.update((bytes.len() as u64).to_le_bytes());
    digest.update(bytes);
    digest.finalize().into()
}

fn put(output: &mut [u8], offset: &mut usize, value: &[u8]) {
    let end = *offset + value.len();
    output[*offset..end].copy_from_slice(value);
    *offset = end;
}
