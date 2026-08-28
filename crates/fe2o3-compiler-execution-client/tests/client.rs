use std::mem;
use std::os::fd::{AsFd, AsRawFd, FromRawFd, OwnedFd};
use std::sync::Mutex;
use std::thread;
use std::time::Duration;

use ed25519_dalek::SigningKey;
use fe2o3_artifact_transaction::{
    INERT_COMPILER_EXECUTION_SUBJECT_BYTES_V1, INERT_COMPILER_EXECUTION_SUBJECT_MAGIC_V1,
    INERT_COMPILER_EXECUTION_SUBJECT_VERSION_V1, InertCompilerExecutionSubjectV1,
};
use fe2o3_compiler_execution_client::{
    COMPILER_EXECUTION_SERVICE_CHILD_FD_V1, CompilerExecutionClientErrorV1,
    CompilerExecutionClientV1, CompilerExecutionReceiptRecoveryV1,
};
use fe2o3_compiler_execution_protocol::{
    CompilerExecutionAttestationChallengeV1, CompilerExecutionAttestationReceiptV1,
    CompilerExecutionAttestationRequestV1, CompilerExecutionIssuerMeasurementV1,
    CompilerExecutionIssuerPolicyV1, CompilerExecutionReceiptCarriageV1,
    CompilerExecutionReceiptPublicationAckV1, CompilerExecutionReceiptPublicationV1,
    CompilerExecutionServicePublishDispositionV1, CompilerExecutionServiceRequestKindV1,
    CompilerExecutionServiceRequestV1, CompilerExecutionServiceResponseV1,
    MAX_COMPILER_EXECUTION_SERVICE_REQUEST_BYTES_V1,
    MAX_COMPILER_EXECUTION_SERVICE_RESPONSE_BYTES_V1,
};
use sha2::{Digest, Sha256};

const SUBJECT_IDENTITY_DOMAIN: &[u8] = b"FE2O3/INERT-COMPILER-EXECUTION-SUBJECT/V1\0";
const COMPILER_CLOSURE_IDENTITY_DOMAIN: &[u8] = b"fe2o3-compiler-closure-identity-v2\0";

static INHERITED_PEER_LOCK: Mutex<()> = Mutex::new(());

#[derive(Clone)]
struct Fixture {
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
        let policy = CompilerExecutionIssuerPolicyV1::new(
            7,
            CompilerExecutionIssuerMeasurementV1::new([0x61; 32], 123).unwrap(),
            CompilerExecutionIssuerMeasurementV1::new([0x62; 32], 456).unwrap(),
            key.verifying_key().to_bytes(),
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
            policy,
            subject,
            challenge,
            request,
            publication,
            acknowledgment,
            carriage,
        }
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
fn cumulative_issuer_delays_cannot_reanchor_the_session_deadline() {
    let fixture = Fixture::new();
    let (client, service) = socket_pair(libc::SOCK_SEQPACKET);
    let policy = fixture.policy.clone();
    let challenge = fixture.challenge.clone();
    let handle = thread::spawn(move || {
        let recover = receive_request(&service);
        thread::sleep(Duration::from_millis(15));
        let absent = CompilerExecutionServiceResponseV1::receipt_absent(
            recover.identity(),
            &policy,
            1,
            [0; 32],
        )
        .unwrap();
        send_raw(&service, absent.canonical_bytes());

        let inspect = receive_request(&service);
        thread::sleep(Duration::from_millis(15));
        let ready =
            CompilerExecutionServiceResponseV1::ready(inspect.identity(), &policy, 1, [0; 32])
                .unwrap();
        send_raw(&service, ready.canonical_bytes());

        let prepare = receive_request(&service);
        thread::sleep(Duration::from_millis(25));
        let prepared =
            CompilerExecutionServiceResponseV1::prepared(prepare.identity(), &policy, challenge)
                .unwrap();
        send_raw_allow_closed(&service, prepared.canonical_bytes());
    });
    let error = CompilerExecutionClientV1::admit(client, Duration::from_millis(40))
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

#[test]
fn inherited_child_admission_consumes_only_exact_fixed_peer() {
    let _guard = INHERITED_PEER_LOCK.lock().unwrap();
    let fixture = Fixture::new();
    let (client, service) = socket_pair(libc::SOCK_SEQPACKET);
    install_inherited_peer(client, false);
    let handle = spawn_service(service, fixture.clone(), DurableStage::Published);
    let carriage = CompilerExecutionClientV1::admit_inherited_child(Duration::from_secs(1))
        .unwrap()
        .acquire(&fixture.policy, fixture.subject.clone())
        .unwrap();
    assert_eq!(carriage, fixture.carriage);
    // SAFETY: F_GETFD reports that admission removed the inherited fixed alias.
    assert_eq!(
        unsafe { libc::fcntl(COMPILER_EXECUTION_SERVICE_CHILD_FD_V1, libc::F_GETFD) },
        -1
    );
    assert_eq!(handle.join().unwrap(), 1);

    assert!(matches!(
        CompilerExecutionClientV1::admit_inherited_child(Duration::from_secs(1)),
        Err(CompilerExecutionClientErrorV1::MissingInheritedPeer)
    ));

    let (client, _service) = socket_pair(libc::SOCK_SEQPACKET);
    install_inherited_peer(client, true);
    assert!(matches!(
        CompilerExecutionClientV1::admit_inherited_child(Duration::from_secs(1)),
        Err(CompilerExecutionClientErrorV1::InheritedPeerCloseOnExec)
    ));
    // SAFETY: rejection must already have consumed the hostile fixed alias.
    assert_eq!(
        unsafe { libc::fcntl(COMPILER_EXECUTION_SERVICE_CHILD_FD_V1, libc::F_GETFD) },
        -1
    );
    assert_eq!(
        std::io::Error::last_os_error().raw_os_error(),
        Some(libc::EBADF)
    );
}

fn install_inherited_peer(peer: OwnedFd, close_on_exec: bool) {
    let flags = if close_on_exec { libc::FD_CLOEXEC } else { 0 };
    if peer.as_raw_fd() == COMPILER_EXECUTION_SERVICE_CHILD_FD_V1 {
        // SAFETY: this test owns `peer`; forgetting it transfers ownership to fixed-FD admission.
        assert_eq!(
            unsafe { libc::fcntl(peer.as_raw_fd(), libc::F_SETFD, flags) },
            0
        );
        mem::forget(peer);
        return;
    }
    // SAFETY: dup3 creates the test-owned fixed alias with the requested descriptor flags.
    assert_eq!(
        unsafe {
            libc::dup3(
                peer.as_raw_fd(),
                COMPILER_EXECUTION_SERVICE_CHILD_FD_V1,
                if close_on_exec { libc::O_CLOEXEC } else { 0 },
            )
        },
        COMPILER_EXECUTION_SERVICE_CHILD_FD_V1
    );
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

fn send_raw_allow_closed(service: &OwnedFd, bytes: &[u8]) {
    // SAFETY: bytes remains readable and service owned throughout this single packet attempt.
    let sent = unsafe {
        libc::send(
            service.as_raw_fd(),
            bytes.as_ptr().cast(),
            bytes.len(),
            libc::MSG_NOSIGNAL,
        )
    };
    if sent < 0 {
        assert!(matches!(
            std::io::Error::last_os_error().kind(),
            std::io::ErrorKind::BrokenPipe | std::io::ErrorKind::ConnectionReset
        ));
    } else {
        assert_eq!(sent, bytes.len() as isize);
    }
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
