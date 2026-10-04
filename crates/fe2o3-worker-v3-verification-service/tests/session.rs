use std::collections::HashSet;
use std::fs::File;
use std::io::{IoSlice, Write};
use std::mem::MaybeUninit;
use std::os::fd::{AsFd, AsRawFd, OwnedFd};
use std::time::Duration;

use fe2o3_worker_v3_verification_protocol::{
    WORKER_V3_VERIFICATION_RESPONSE_BYTES_V1, WorkerV3VerificationEntryCoordinateV1,
    WorkerV3VerificationFdPayloadDescriptorV1, WorkerV3VerificationFdPayloadKindV1,
    WorkerV3VerificationFreshChallengeV1, WorkerV3VerificationMeasurementIdentityV1,
    WorkerV3VerificationPolicyIdentityV1, WorkerV3VerificationRequestV1,
    WorkerV3VerificationResponseDispositionV1, WorkerV3VerificationResponseV1,
    WorkerV3VerificationRosterIdentityV1,
};
use fe2o3_worker_v3_verification_service::{
    WorkerV3VerificationCallerV1, WorkerV3VerificationChallengeReplayGuardV1,
    WorkerV3VerificationMeasurementResolverV1, WorkerV3VerificationPolicyResolverV1,
    WorkerV3VerificationRejectionReasonV1, WorkerV3VerificationServiceErrorV1,
    WorkerV3VerificationSessionOutcomeV1, prepare_worker_v3_verification_receiver_v1,
    serve_worker_v3_verification_session_v1,
};
use rustix::fs::{MemfdFlags, Mode, OFlags, SealFlags};
use rustix::net::{
    AddressFamily, RecvFlags, SendAncillaryBuffer, SendAncillaryMessage, SendFlags, Shutdown,
    SocketFlags, SocketType, recv, sendmsg, shutdown, socketpair,
};
use sha2::{Digest, Sha256};

const SEALS: SealFlags = SealFlags::WRITE
    .union(SealFlags::GROW)
    .union(SealFlags::SHRINK)
    .union(SealFlags::SEAL);

#[repr(align(16))]
struct AlignedAncillaryStorage<const N: usize>([MaybeUninit<u8>; N]);

struct FixedPolicy(Option<WorkerV3VerificationPolicyIdentityV1>);

impl WorkerV3VerificationPolicyResolverV1 for FixedPolicy {
    fn resolve_expected_policy(
        &mut self,
        _caller: WorkerV3VerificationCallerV1,
        _request: &WorkerV3VerificationRequestV1,
    ) -> Option<WorkerV3VerificationPolicyIdentityV1> {
        self.0
    }
}

struct FixedMeasurement(Option<WorkerV3VerificationMeasurementIdentityV1>);

impl WorkerV3VerificationMeasurementResolverV1 for FixedMeasurement {
    fn resolve_expected_measurement(
        &mut self,
        _caller: WorkerV3VerificationCallerV1,
        _policy: WorkerV3VerificationPolicyIdentityV1,
        _request: &WorkerV3VerificationRequestV1,
    ) -> Option<WorkerV3VerificationMeasurementIdentityV1> {
        self.0
    }
}

type ReplayKey = (u32, u32, u32, [u8; 32], [u8; 32]);

#[derive(Default)]
struct ReplayGuard(HashSet<ReplayKey>);

impl WorkerV3VerificationChallengeReplayGuardV1 for ReplayGuard {
    fn admit_fresh_challenge(
        &mut self,
        caller: WorkerV3VerificationCallerV1,
        policy: WorkerV3VerificationPolicyIdentityV1,
        challenge: WorkerV3VerificationFreshChallengeV1,
    ) -> bool {
        self.0.insert((
            caller.pid(),
            caller.uid(),
            caller.gid(),
            *policy.as_bytes(),
            *challenge.as_bytes(),
        ))
    }
}

fn pair() -> (OwnedFd, OwnedFd) {
    let pair = socketpair(
        AddressFamily::UNIX,
        SocketType::SEQPACKET,
        SocketFlags::CLOEXEC | SocketFlags::NONBLOCK,
        None,
    )
    .unwrap();
    prepare_worker_v3_verification_receiver_v1(&pair.0).unwrap();
    pair
}

fn sealed_read_only(bytes: &[u8]) -> File {
    let descriptor = rustix::fs::memfd_create(
        "fe2o3-worker-v3-service-test",
        MemfdFlags::CLOEXEC | MemfdFlags::ALLOW_SEALING,
    )
    .unwrap();
    let mut writer = File::from(descriptor);
    rustix::fs::fchmod(&writer, Mode::RUSR).unwrap();
    writer.write_all(bytes).unwrap();
    writer.flush().unwrap();
    rustix::fs::fcntl_add_seals(&writer, SEALS).unwrap();
    let path = format!("/proc/self/fd/{}", writer.as_raw_fd());
    let read_only = File::from(
        rustix::fs::open(path, OFlags::RDONLY | OFlags::CLOEXEC, Mode::empty()).unwrap(),
    );
    drop(writer);
    read_only
}

fn unsealed_read_only(bytes: &[u8]) -> File {
    let descriptor = rustix::fs::memfd_create(
        "fe2o3-worker-v3-service-unsealed-test",
        MemfdFlags::CLOEXEC | MemfdFlags::ALLOW_SEALING,
    )
    .unwrap();
    let mut writer = File::from(descriptor);
    writer.write_all(bytes).unwrap();
    let path = format!("/proc/self/fd/{}", writer.as_raw_fd());
    let read_only = File::from(
        rustix::fs::open(path, OFlags::RDONLY | OFlags::CLOEXEC, Mode::empty()).unwrap(),
    );
    drop(writer);
    read_only
}

fn sealed_writable(bytes: &[u8]) -> File {
    let descriptor = rustix::fs::memfd_create(
        "fe2o3-worker-v3-service-writable-test",
        MemfdFlags::CLOEXEC | MemfdFlags::ALLOW_SEALING,
    )
    .unwrap();
    let mut file = File::from(descriptor);
    file.write_all(bytes).unwrap();
    rustix::fs::fcntl_add_seals(&file, SEALS).unwrap();
    file
}

fn sha256(bytes: &[u8]) -> [u8; 32] {
    Sha256::digest(bytes).into()
}

fn request(
    challenge: u8,
    policy: u8,
    measurement: u8,
    envelope: &[u8],
    hsaco: &[u8],
) -> WorkerV3VerificationRequestV1 {
    WorkerV3VerificationRequestV1::new(
        WorkerV3VerificationFreshChallengeV1::new([challenge; 32]).unwrap(),
        WorkerV3VerificationRosterIdentityV1::new([0x22; 32]).unwrap(),
        WorkerV3VerificationPolicyIdentityV1::new([policy; 32]).unwrap(),
        WorkerV3VerificationMeasurementIdentityV1::new([measurement; 32]).unwrap(),
        WorkerV3VerificationFdPayloadDescriptorV1::load_envelope_v2(
            envelope.len() as u64,
            sha256(envelope),
        )
        .unwrap(),
        WorkerV3VerificationFdPayloadDescriptorV1::finalized_hsaco(
            hsaco.len() as u64,
            sha256(hsaco),
        )
        .unwrap(),
        vec![
            WorkerV3VerificationEntryCoordinateV1::new(
                0,
                "kernel",
                "kernel_export",
                [0x31; 32],
                [0x32; 32],
                [0x33; 32],
            )
            .unwrap(),
        ],
    )
    .unwrap()
}

fn send_packet(peer: &OwnedFd, bytes: &[u8], descriptors: &[std::os::fd::BorrowedFd<'_>]) {
    send_packet_open(peer, bytes, descriptors);
    shutdown(peer, Shutdown::Write).unwrap();
}

fn send_packet_open(peer: &OwnedFd, bytes: &[u8], descriptors: &[std::os::fd::BorrowedFd<'_>]) {
    let mut space =
        AlignedAncillaryStorage([MaybeUninit::uninit(); rustix::cmsg_space!(ScmRights(4))]);
    let mut ancillary = SendAncillaryBuffer::new(&mut space.0);
    if !descriptors.is_empty() {
        assert!(ancillary.push(SendAncillaryMessage::ScmRights(descriptors)));
    }
    assert_eq!(
        sendmsg(
            peer,
            &[IoSlice::new(bytes)],
            &mut ancillary,
            SendFlags::NOSIGNAL,
        )
        .unwrap(),
        bytes.len()
    );
}

fn receive_response(peer: &OwnedFd) -> WorkerV3VerificationResponseV1 {
    let mut bytes = [0_u8; WORKER_V3_VERIFICATION_RESPONSE_BYTES_V1];
    assert_eq!(
        recv(peer, &mut bytes, RecvFlags::empty()).unwrap().0,
        bytes.len()
    );
    WorkerV3VerificationResponseV1::decode_canonical(&bytes).unwrap()
}

fn run(
    request: &WorkerV3VerificationRequestV1,
    descriptors: &[std::os::fd::BorrowedFd<'_>],
    policy: &mut FixedPolicy,
    measurement: &mut FixedMeasurement,
    replay: &mut ReplayGuard,
) -> (
    WorkerV3VerificationSessionOutcomeV1,
    WorkerV3VerificationResponseV1,
) {
    let (service, peer) = pair();
    send_packet(&peer, request.encode_canonical(), descriptors);
    let outcome = serve_worker_v3_verification_session_v1(
        service,
        Duration::from_secs(1),
        policy,
        measurement,
        replay,
    )
    .unwrap();
    let response = receive_response(&peer);
    (outcome, response)
}

#[test]
fn exact_session_captures_receiver_owned_immutable_payloads_without_authority() {
    let envelope_bytes = b"canonical envelope";
    let hsaco_bytes = b"canonical finalized hsaco";
    let request = request(1, 2, 3, envelope_bytes, hsaco_bytes);
    let envelope = sealed_read_only(envelope_bytes);
    let hsaco = sealed_read_only(hsaco_bytes);
    let envelope_source = rustix::fs::fstat(&envelope).unwrap();
    let hsaco_source = rustix::fs::fstat(&hsaco).unwrap();
    let mut policy = FixedPolicy(Some(request.policy_identity()));
    let mut measurement = FixedMeasurement(Some(request.measurement_identity()));
    let mut replay = ReplayGuard::default();
    let (outcome, wire_response) = run(
        &request,
        &[envelope.as_fd(), hsaco.as_fd()],
        &mut policy,
        &mut measurement,
        &mut replay,
    );
    let WorkerV3VerificationSessionOutcomeV1::Framed(framed) = outcome else {
        panic!("exact request was not framed");
    };
    assert_eq!(
        wire_response.disposition(),
        WorkerV3VerificationResponseDispositionV1::RequestFramed
    );
    assert_eq!(wire_response, *framed.response());
    assert!(wire_response.matches_request(&request));
    assert!(!wire_response.grants_authority());
    assert!(!framed.grants_authority());
    assert!(!framed.verification_performed());
    for (kind, expected, source) in [
        (
            WorkerV3VerificationFdPayloadKindV1::LoadEnvelopeV2,
            envelope_bytes.as_slice(),
            envelope_source,
        ),
        (
            WorkerV3VerificationFdPayloadKindV1::FinalizedHsaco,
            hsaco_bytes.as_slice(),
            hsaco_source,
        ),
    ] {
        let payload = framed.payload(kind);
        assert_eq!(payload.byte_len(), expected.len() as u64);
        assert_eq!(payload.sha256(), &sha256(expected));
        assert!(!payload.grants_authority());
        let retained = rustix::fs::fstat(payload).unwrap();
        assert_ne!(
            (retained.st_dev, retained.st_ino),
            (source.st_dev, source.st_ino)
        );
        let mut observed = vec![0_u8; expected.len()];
        assert_eq!(
            rustix::io::pread(payload, &mut observed, 0).unwrap(),
            expected.len()
        );
        assert_eq!(observed, expected);
        assert_eq!(rustix::fs::fcntl_get_seals(payload).unwrap(), SEALS);
        assert_eq!(
            rustix::fs::fcntl_getfl(payload).unwrap() & OFlags::ACCMODE,
            OFlags::RDONLY
        );
    }
}

#[test]
fn duplicate_challenge_is_rejected_after_one_framed_session() {
    let envelope = b"envelope";
    let hsaco = b"hsaco";
    let request = request(4, 5, 6, envelope, hsaco);
    let envelope_file = sealed_read_only(envelope);
    let hsaco_file = sealed_read_only(hsaco);
    let mut policy = FixedPolicy(Some(request.policy_identity()));
    let mut measurement = FixedMeasurement(Some(request.measurement_identity()));
    let mut replay = ReplayGuard::default();
    let first = run(
        &request,
        &[envelope_file.as_fd(), hsaco_file.as_fd()],
        &mut policy,
        &mut measurement,
        &mut replay,
    );
    assert!(matches!(
        first.0,
        WorkerV3VerificationSessionOutcomeV1::Framed(_)
    ));
    let second = run(
        &request,
        &[envelope_file.as_fd(), hsaco_file.as_fd()],
        &mut policy,
        &mut measurement,
        &mut replay,
    );
    let WorkerV3VerificationSessionOutcomeV1::Rejected(rejected) = second.0 else {
        panic!("replayed challenge was framed");
    };
    assert_eq!(
        rejected.reason(),
        WorkerV3VerificationRejectionReasonV1::ChallengeReplay
    );
    assert_eq!(
        second.1.disposition(),
        WorkerV3VerificationResponseDispositionV1::RequestRejected
    );
    assert!(!rejected.grants_authority());
}

#[test]
fn unresolved_and_substituted_policy_or_measurement_fail_closed() {
    let envelope = sealed_read_only(b"envelope");
    let hsaco = sealed_read_only(b"hsaco");
    for (policy, measurement, expected) in [
        (
            FixedPolicy(None),
            FixedMeasurement(Some(
                WorkerV3VerificationMeasurementIdentityV1::new([3; 32]).unwrap(),
            )),
            WorkerV3VerificationRejectionReasonV1::PolicyUnresolved,
        ),
        (
            FixedPolicy(Some(
                WorkerV3VerificationPolicyIdentityV1::new([9; 32]).unwrap(),
            )),
            FixedMeasurement(Some(
                WorkerV3VerificationMeasurementIdentityV1::new([3; 32]).unwrap(),
            )),
            WorkerV3VerificationRejectionReasonV1::PolicyMismatch,
        ),
        (
            FixedPolicy(Some(
                WorkerV3VerificationPolicyIdentityV1::new([2; 32]).unwrap(),
            )),
            FixedMeasurement(None),
            WorkerV3VerificationRejectionReasonV1::MeasurementUnresolved,
        ),
        (
            FixedPolicy(Some(
                WorkerV3VerificationPolicyIdentityV1::new([2; 32]).unwrap(),
            )),
            FixedMeasurement(Some(
                WorkerV3VerificationMeasurementIdentityV1::new([9; 32]).unwrap(),
            )),
            WorkerV3VerificationRejectionReasonV1::MeasurementMismatch,
        ),
    ] {
        let request = request(7, 2, 3, b"envelope", b"hsaco");
        let mut policy = policy;
        let mut measurement = measurement;
        let mut replay = ReplayGuard::default();
        let (outcome, response) = run(
            &request,
            &[envelope.as_fd(), hsaco.as_fd()],
            &mut policy,
            &mut measurement,
            &mut replay,
        );
        let WorkerV3VerificationSessionOutcomeV1::Rejected(rejected) = outcome else {
            panic!("invalid selection was framed");
        };
        assert_eq!(rejected.reason(), expected);
        assert_eq!(
            response.disposition(),
            WorkerV3VerificationResponseDispositionV1::RequestRejected
        );
    }
}

#[test]
fn aliased_reordered_unsealed_and_writable_payloads_are_rejected() {
    let envelope_bytes = b"envelope-long";
    let hsaco_bytes = b"hsaco";
    let envelope = sealed_read_only(envelope_bytes);
    let hsaco = sealed_read_only(hsaco_bytes);
    let unsealed = unsealed_read_only(envelope_bytes);
    let writable = sealed_writable(envelope_bytes);
    let cases = [
        (
            vec![envelope.as_fd(), envelope.as_fd()],
            WorkerV3VerificationRejectionReasonV1::PayloadDescriptorAlias,
        ),
        (
            vec![hsaco.as_fd(), envelope.as_fd()],
            WorkerV3VerificationRejectionReasonV1::PayloadLengthMismatch(
                WorkerV3VerificationFdPayloadKindV1::LoadEnvelopeV2,
            ),
        ),
        (
            vec![unsealed.as_fd(), hsaco.as_fd()],
            WorkerV3VerificationRejectionReasonV1::InvalidPayloadDescriptor(
                WorkerV3VerificationFdPayloadKindV1::LoadEnvelopeV2,
            ),
        ),
        (
            vec![writable.as_fd(), hsaco.as_fd()],
            WorkerV3VerificationRejectionReasonV1::InvalidPayloadDescriptor(
                WorkerV3VerificationFdPayloadKindV1::LoadEnvelopeV2,
            ),
        ),
    ];
    for (index, (descriptors, expected)) in cases.into_iter().enumerate() {
        let request = request((20 + index) as u8, 2, 3, envelope_bytes, hsaco_bytes);
        let mut policy = FixedPolicy(Some(request.policy_identity()));
        let mut measurement = FixedMeasurement(Some(request.measurement_identity()));
        let mut replay = ReplayGuard::default();
        let (outcome, _) = run(
            &request,
            &descriptors,
            &mut policy,
            &mut measurement,
            &mut replay,
        );
        let WorkerV3VerificationSessionOutcomeV1::Rejected(rejected) = outcome else {
            panic!("hostile payload case {index} was framed");
        };
        assert_eq!(rejected.reason(), expected);
    }
}

#[test]
fn digest_substitution_is_rejected() {
    let envelope_bytes = b"envelope";
    let hsaco_bytes = b"hsaco";
    let request = WorkerV3VerificationRequestV1::new(
        WorkerV3VerificationFreshChallengeV1::new([30; 32]).unwrap(),
        WorkerV3VerificationRosterIdentityV1::new([0x22; 32]).unwrap(),
        WorkerV3VerificationPolicyIdentityV1::new([2; 32]).unwrap(),
        WorkerV3VerificationMeasurementIdentityV1::new([3; 32]).unwrap(),
        WorkerV3VerificationFdPayloadDescriptorV1::load_envelope_v2(
            envelope_bytes.len() as u64,
            [0x99; 32],
        )
        .unwrap(),
        WorkerV3VerificationFdPayloadDescriptorV1::finalized_hsaco(
            hsaco_bytes.len() as u64,
            sha256(hsaco_bytes),
        )
        .unwrap(),
        vec![
            WorkerV3VerificationEntryCoordinateV1::new(
                0,
                "kernel",
                "kernel_export",
                [0x31; 32],
                [0x32; 32],
                [0x33; 32],
            )
            .unwrap(),
        ],
    )
    .unwrap();
    let envelope = sealed_read_only(envelope_bytes);
    let hsaco = sealed_read_only(hsaco_bytes);
    let mut policy = FixedPolicy(Some(request.policy_identity()));
    let mut measurement = FixedMeasurement(Some(request.measurement_identity()));
    let mut replay = ReplayGuard::default();
    let (outcome, _) = run(
        &request,
        &[envelope.as_fd(), hsaco.as_fd()],
        &mut policy,
        &mut measurement,
        &mut replay,
    );
    let WorkerV3VerificationSessionOutcomeV1::Rejected(rejected) = outcome else {
        panic!("digest-substituted payload was framed");
    };
    assert_eq!(
        rejected.reason(),
        WorkerV3VerificationRejectionReasonV1::PayloadDigestMismatch(
            WorkerV3VerificationFdPayloadKindV1::LoadEnvelopeV2,
        )
    );
}

#[test]
fn missing_extra_and_truncated_transfers_are_terminal_without_a_bound_response() {
    let request = request(31, 2, 3, b"envelope", b"hsaco");
    let envelope = sealed_read_only(b"envelope");
    let hsaco = sealed_read_only(b"hsaco");
    for descriptors in [
        vec![envelope.as_fd()],
        vec![envelope.as_fd(), hsaco.as_fd(), envelope.as_fd()],
    ] {
        let (service, peer) = pair();
        send_packet(&peer, request.encode_canonical(), &descriptors);
        let error = serve_worker_v3_verification_session_v1(
            service,
            Duration::from_secs(1),
            &mut FixedPolicy(Some(request.policy_identity())),
            &mut FixedMeasurement(Some(request.measurement_identity())),
            &mut ReplayGuard::default(),
        )
        .unwrap_err();
        assert!(matches!(
            error,
            WorkerV3VerificationServiceErrorV1::MalformedTransfer
        ));
    }

    let (service, peer) = pair();
    let oversized = vec![
        0_u8;
        fe2o3_worker_v3_verification_protocol::MAX_WORKER_V3_VERIFICATION_REQUEST_BYTES_V1
            + 1
    ];
    send_packet(&peer, &oversized, &[envelope.as_fd(), hsaco.as_fd()]);
    assert!(matches!(
        serve_worker_v3_verification_session_v1(
            service,
            Duration::from_secs(1),
            &mut FixedPolicy(Some(request.policy_identity())),
            &mut FixedMeasurement(Some(request.measurement_identity())),
            &mut ReplayGuard::default(),
        ),
        Err(WorkerV3VerificationServiceErrorV1::MalformedTransfer)
    ));

    let (service, peer) = pair();
    send_packet_open(
        &peer,
        request.encode_canonical(),
        &[envelope.as_fd(), hsaco.as_fd()],
    );
    send_packet_open(
        &peer,
        request.encode_canonical(),
        &[envelope.as_fd(), hsaco.as_fd()],
    );
    shutdown(&peer, Shutdown::Write).unwrap();
    assert!(matches!(
        serve_worker_v3_verification_session_v1(
            service,
            Duration::from_secs(1),
            &mut FixedPolicy(Some(request.policy_identity())),
            &mut FixedMeasurement(Some(request.measurement_identity())),
            &mut ReplayGuard::default(),
        ),
        Err(WorkerV3VerificationServiceErrorV1::TrailingTransfer)
    ));
}

#[test]
fn empty_seqpacket_followed_by_shutdown_is_not_transport_eof() {
    let request = request(40, 2, 3, b"envelope", b"hsaco");
    let (service, peer) = pair();
    send_packet(&peer, &[], &[]);
    assert!(matches!(
        serve_worker_v3_verification_session_v1(
            service,
            Duration::from_secs(1),
            &mut FixedPolicy(Some(request.policy_identity())),
            &mut FixedMeasurement(Some(request.measurement_identity())),
            &mut ReplayGuard::default(),
        ),
        Err(WorkerV3VerificationServiceErrorV1::MalformedTransfer)
    ));
}

#[test]
fn noncanonical_request_and_timeout_are_terminal() {
    let request = request(32, 2, 3, b"envelope", b"hsaco");
    let envelope = sealed_read_only(b"envelope");
    let hsaco = sealed_read_only(b"hsaco");
    let mut corrupted = request.encode_canonical().to_vec();
    corrupted[0] ^= 0xff;
    let (service, peer) = pair();
    send_packet(&peer, &corrupted, &[envelope.as_fd(), hsaco.as_fd()]);
    assert!(matches!(
        serve_worker_v3_verification_session_v1(
            service,
            Duration::from_secs(1),
            &mut FixedPolicy(Some(request.policy_identity())),
            &mut FixedMeasurement(Some(request.measurement_identity())),
            &mut ReplayGuard::default(),
        ),
        Err(WorkerV3VerificationServiceErrorV1::CanonicalRequest(_))
    ));

    let (service, _peer) = pair();
    assert!(matches!(
        serve_worker_v3_verification_session_v1(
            service,
            Duration::from_millis(5),
            &mut FixedPolicy(Some(request.policy_identity())),
            &mut FixedMeasurement(Some(request.measurement_identity())),
            &mut ReplayGuard::default(),
        ),
        Err(WorkerV3VerificationServiceErrorV1::Timeout)
    ));

    let (service, peer) = pair();
    send_packet_open(
        &peer,
        request.encode_canonical(),
        &[envelope.as_fd(), hsaco.as_fd()],
    );
    assert!(matches!(
        serve_worker_v3_verification_session_v1(
            service,
            Duration::from_millis(5),
            &mut FixedPolicy(Some(request.policy_identity())),
            &mut FixedMeasurement(Some(request.measurement_identity())),
            &mut ReplayGuard::default(),
        ),
        Err(WorkerV3VerificationServiceErrorV1::Timeout)
    ));
}
