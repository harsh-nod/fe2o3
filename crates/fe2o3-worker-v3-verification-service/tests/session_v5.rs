use std::fs::File;
use std::io::{IoSlice, Write};
use std::mem::MaybeUninit;
use std::os::fd::{AsFd, AsRawFd, OwnedFd};
use std::time::Duration;

use fe2o3_worker_v3_verification_protocol::{
    ExactIdentityCoordinateV5, MAX_WORKER_V3_VERIFICATION_CAPABILITY_RESPONSE_BYTES_V5,
    WorkerV3VerificationCapabilityCarriageV5, WorkerV3VerificationCapabilityRequestIdentityV5,
    WorkerV3VerificationCapabilityRequestV5, WorkerV3VerificationCapabilityResponseDispositionV5,
    WorkerV3VerificationCapabilityResponseV5, WorkerV3VerificationEntryCoordinateV1,
    WorkerV3VerificationFdPayloadDescriptorV1, WorkerV3VerificationFreshChallengeV1,
    WorkerV3VerificationMeasurementIdentityV1, WorkerV3VerificationPolicyIdentityV1,
    WorkerV3VerificationProductionAttemptV5, WorkerV3VerificationRequestV1,
    WorkerV3VerificationRosterIdentityV1,
};
use fe2o3_worker_v3_verification_service::{
    ProtectedWorkerV3VerifierSigningKeyV5, WorkerV3ProtectedCompletionResponseV5,
    WorkerV3VerificationCallerV1, WorkerV3VerificationCapabilityAttemptQuarantineV5,
    WorkerV3VerificationCapabilityCompletionJournalV5,
    WorkerV3VerificationCapabilityRejectionReasonV5, WorkerV3VerificationCapabilityServiceErrorV5,
    WorkerV3VerificationCapabilitySessionOutcomeV5, WorkerV3VerificationChallengeReplayGuardV1,
    WorkerV3VerificationMeasurementResolverV1, WorkerV3VerificationPolicyResolverV1,
    prepare_worker_v3_verification_receiver_v1, serve_worker_v3_verification_capability_session_v5,
};
use rustix::fs::{MemfdFlags, Mode, OFlags, SealFlags};
use rustix::net::{
    AddressFamily, RecvFlags, SendAncillaryBuffer, SendAncillaryMessage, SendFlags, Shutdown,
    SocketFlags, SocketType, recv, sendmsg, shutdown, socketpair,
};
use sha2::{Digest as _, Sha256};

const ENVELOPE: &[u8] = b"exact-v5-load-envelope";
const OBJECT: &[u8] = b"not-admitted-as-a-machine-proof";
const POLICY: [u8; 32] = [82; 32];
const MEASUREMENT: [u8; 32] = [83; 32];
const SEALS: SealFlags = SealFlags::WRITE
    .union(SealFlags::GROW)
    .union(SealFlags::SHRINK)
    .union(SealFlags::SEAL);

#[repr(align(16))]
struct AlignedAncillaryStorage<const N: usize>([MaybeUninit<u8>; N]);

struct Policy;
impl WorkerV3VerificationPolicyResolverV1 for Policy {
    fn resolve_expected_policy(
        &mut self,
        _caller: WorkerV3VerificationCallerV1,
        _request: &WorkerV3VerificationRequestV1,
    ) -> Option<WorkerV3VerificationPolicyIdentityV1> {
        WorkerV3VerificationPolicyIdentityV1::new(POLICY).ok()
    }
}

struct Measurement;
impl WorkerV3VerificationMeasurementResolverV1 for Measurement {
    fn resolve_expected_measurement(
        &mut self,
        _caller: WorkerV3VerificationCallerV1,
        _policy: WorkerV3VerificationPolicyIdentityV1,
        _request: &WorkerV3VerificationRequestV1,
    ) -> Option<WorkerV3VerificationMeasurementIdentityV1> {
        WorkerV3VerificationMeasurementIdentityV1::new(MEASUREMENT).ok()
    }
}

struct Fresh;
impl WorkerV3VerificationChallengeReplayGuardV1 for Fresh {
    fn admit_fresh_challenge(
        &mut self,
        _caller: WorkerV3VerificationCallerV1,
        _policy: WorkerV3VerificationPolicyIdentityV1,
        _challenge: WorkerV3VerificationFreshChallengeV1,
    ) -> bool {
        true
    }
}

struct Journal;

impl WorkerV3VerificationCapabilityCompletionJournalV5 for Journal {
    type Error = std::io::Error;

    fn record_issued(
        &mut self,
        _caller: WorkerV3VerificationCallerV1,
        _request: WorkerV3VerificationCapabilityRequestIdentityV5,
        _evidence_identity: [u8; 32],
        _response: &WorkerV3ProtectedCompletionResponseV5,
    ) -> Result<(), Self::Error> {
        Ok(())
    }
}

#[derive(Default)]
struct Quarantine {
    attempts: usize,
    fail: bool,
}

impl WorkerV3VerificationCapabilityAttemptQuarantineV5 for Quarantine {
    type Error = std::io::Error;

    fn quarantine_unidentified_submission(
        &mut self,
        _caller: WorkerV3VerificationCallerV1,
        _submission_sha256: Option<[u8; 32]>,
    ) -> Result<(), Self::Error> {
        self.result()
    }

    fn quarantine_attempt_coordinates(
        &mut self,
        _caller: WorkerV3VerificationCallerV1,
        _request: WorkerV3VerificationCapabilityRequestIdentityV5,
        _attempt: WorkerV3VerificationProductionAttemptV5,
    ) -> Result<(), Self::Error> {
        self.attempts += 1;
        self.result()
    }

    fn quarantine_evidence(
        &mut self,
        _caller: WorkerV3VerificationCallerV1,
        _request: WorkerV3VerificationCapabilityRequestIdentityV5,
        _evidence_identity: Option<[u8; 32]>,
        _reason: WorkerV3VerificationCapabilityRejectionReasonV5,
    ) -> Result<(), Self::Error> {
        self.attempts += 1;
        self.result()
    }
}

impl Quarantine {
    fn result(&self) -> Result<(), std::io::Error> {
        if self.fail {
            Err(std::io::Error::other("injected durable-store failure"))
        } else {
            Ok(())
        }
    }
}

#[test]
fn authenticated_v5_session_persists_then_returns_generic_rejection() {
    let (service, peer) = pair();
    let envelope = sealed(ENVELOPE);
    let object = sealed(OBJECT);
    let request = request();
    send(&peer, request.encode_canonical(), &envelope, &object);
    let mut quarantine = Quarantine::default();
    let signer = signer();
    let outcome = serve_worker_v3_verification_capability_session_v5(
        service,
        Duration::from_secs(2),
        &mut Policy,
        &mut Measurement,
        &mut Fresh,
        &signer,
        &mut Journal,
        &mut quarantine,
    )
    .unwrap();
    let WorkerV3VerificationCapabilitySessionOutcomeV5::Rejected(rejected) = outcome else {
        panic!("malformed evidence must reject");
    };
    assert_eq!(
        rejected.reason(),
        WorkerV3VerificationCapabilityRejectionReasonV5::EvidenceMalformed
    );
    assert_eq!(quarantine.attempts, 1);
    let response = receive(&peer);
    assert_eq!(
        response.disposition(),
        WorkerV3VerificationCapabilityResponseDispositionV5::Rejected
    );
    assert_eq!(response.request_identity(), request.identity());
}

#[test]
fn durable_quarantine_failure_suppresses_the_terminal_response() {
    let (service, peer) = pair();
    let envelope = sealed(ENVELOPE);
    let object = sealed(OBJECT);
    let request = request();
    send(&peer, request.encode_canonical(), &envelope, &object);
    let mut quarantine = Quarantine {
        attempts: 0,
        fail: true,
    };
    let signer = signer();
    let error = serve_worker_v3_verification_capability_session_v5(
        service,
        Duration::from_secs(2),
        &mut Policy,
        &mut Measurement,
        &mut Fresh,
        &signer,
        &mut Journal,
        &mut quarantine,
    )
    .unwrap_err();
    assert!(matches!(
        error,
        WorkerV3VerificationCapabilityServiceErrorV5::Persistence(_)
    ));
    let mut response = [0; MAX_WORKER_V3_VERIFICATION_CAPABILITY_RESPONSE_BYTES_V5];
    assert_eq!(recv(&peer, &mut response, RecvFlags::empty()).unwrap().0, 0);
}

fn request() -> WorkerV3VerificationCapabilityRequestV5 {
    let base = WorkerV3VerificationRequestV1::new(
        WorkerV3VerificationFreshChallengeV1::new([80; 32]).unwrap(),
        WorkerV3VerificationRosterIdentityV1::new([81; 32]).unwrap(),
        WorkerV3VerificationPolicyIdentityV1::new(POLICY).unwrap(),
        WorkerV3VerificationMeasurementIdentityV1::new(MEASUREMENT).unwrap(),
        WorkerV3VerificationFdPayloadDescriptorV1::protected_completion_evidence_v5(
            ENVELOPE.len() as u64,
            sha(ENVELOPE),
        )
        .unwrap(),
        WorkerV3VerificationFdPayloadDescriptorV1::finalized_hsaco(
            OBJECT.len() as u64,
            sha(OBJECT),
        )
        .unwrap(),
        vec![
            WorkerV3VerificationEntryCoordinateV1::new(
                0,
                "kernel",
                "kernel_export",
                [85; 32],
                [86; 32],
                [87; 32],
            )
            .unwrap(),
        ],
    )
    .unwrap();
    let coordinate = |seed| ExactIdentityCoordinateV5::new([seed; 32], seed as u64 + 1).unwrap();
    let carriage = WorkerV3VerificationCapabilityCarriageV5::new_exact(
        coordinate(1),
        coordinate(2),
        [3; 32],
        WorkerV3VerificationProductionAttemptV5::new(7, [4; 16], [5; 32]).unwrap(),
        0,
        13,
        coordinate(6),
        9,
        [7; 32],
        [8; 32],
        [9; 32],
        [10; 32],
        [11; 32],
        coordinate(12),
        [13; 32],
        coordinate(14),
        ExactIdentityCoordinateV5::new(sha(OBJECT), OBJECT.len() as u64).unwrap(),
        [15; 32],
        POLICY,
        coordinate(16),
    )
    .unwrap();
    WorkerV3VerificationCapabilityRequestV5::new(base, carriage).unwrap()
}

fn signer() -> ProtectedWorkerV3VerifierSigningKeyV5 {
    ProtectedWorkerV3VerifierSigningKeyV5::from_sealed_descriptor(sealed(&[99; 32]).into()).unwrap()
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

fn sealed(bytes: &[u8]) -> File {
    let descriptor = rustix::fs::memfd_create(
        "fe2o3-worker-v3-v5-session-test",
        MemfdFlags::CLOEXEC | MemfdFlags::ALLOW_SEALING,
    )
    .unwrap();
    let mut writer = File::from(descriptor);
    rustix::fs::fchmod(&writer, Mode::RUSR).unwrap();
    writer.write_all(bytes).unwrap();
    rustix::fs::fcntl_add_seals(&writer, SEALS).unwrap();
    let read_only = File::from(
        rustix::fs::open(
            format!("/proc/self/fd/{}", writer.as_raw_fd()),
            OFlags::RDONLY | OFlags::CLOEXEC,
            Mode::empty(),
        )
        .unwrap(),
    );
    drop(writer);
    read_only
}

fn send(peer: &OwnedFd, bytes: &[u8], envelope: &File, object: &File) {
    let mut space =
        AlignedAncillaryStorage([MaybeUninit::uninit(); rustix::cmsg_space!(ScmRights(2))]);
    let mut ancillary = SendAncillaryBuffer::new(&mut space.0);
    let descriptors = [envelope.as_fd(), object.as_fd()];
    assert!(ancillary.push(SendAncillaryMessage::ScmRights(&descriptors)));
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
    shutdown(peer, Shutdown::Write).unwrap();
}

fn receive(peer: &OwnedFd) -> WorkerV3VerificationCapabilityResponseV5 {
    let mut bytes = [0; MAX_WORKER_V3_VERIFICATION_CAPABILITY_RESPONSE_BYTES_V5];
    let count = recv(peer, &mut bytes, RecvFlags::empty()).unwrap().0;
    WorkerV3VerificationCapabilityResponseV5::decode_canonical(&bytes[..count]).unwrap()
}

fn sha(bytes: &[u8]) -> [u8; 32] {
    Sha256::digest(bytes).into()
}
