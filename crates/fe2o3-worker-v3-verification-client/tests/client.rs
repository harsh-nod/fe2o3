use std::fs::File;
use std::io::{IoSlice, IoSliceMut, Write};
use std::mem::MaybeUninit;
use std::os::fd::{AsFd, OwnedFd};
use std::os::unix::net::UnixStream;
use std::thread;
use std::time::Duration;

use fe2o3_worker_v3_verification_client::{
    WorkerV3VerificationClientErrorV1, WorkerV3VerificationClientV1,
    WorkerV3VerificationPayloadSnapshotsV1,
};
use fe2o3_worker_v3_verification_protocol::{
    MAX_WORKER_V3_VERIFICATION_REQUEST_BYTES_V1, WorkerV3VerificationEntryCoordinateV1,
    WorkerV3VerificationFdPayloadDescriptorV1, WorkerV3VerificationFreshChallengeV1,
    WorkerV3VerificationMeasurementIdentityV1, WorkerV3VerificationPolicyIdentityV1,
    WorkerV3VerificationRequestV1, WorkerV3VerificationResponseDispositionV1,
    WorkerV3VerificationResponseV1, WorkerV3VerificationRosterIdentityV1,
    WorkerV3VerificationTranscriptIdentityV1,
};
use rustix::fs::{MemfdFlags, OFlags, SealFlags, SeekFrom};
use rustix::net::{
    AddressFamily, RecvAncillaryBuffer, RecvAncillaryMessage, RecvFlags, ReturnFlags,
    SendAncillaryBuffer, SendAncillaryMessage, SendFlags, SocketFlags, SocketType,
};
use sha2::{Digest, Sha256};

const REQUIRED_SEALS: SealFlags = SealFlags::WRITE
    .union(SealFlags::GROW)
    .union(SealFlags::SHRINK)
    .union(SealFlags::SEAL);
const LOAD_BYTES: &[u8] = b"canonical-v2-load-envelope";
const HSACO_BYTES: &[u8] = b"canonical-finalized-hsaco!";

fn sha256(bytes: &[u8]) -> [u8; 32] {
    Sha256::digest(bytes).into()
}

fn request(challenge: u8) -> WorkerV3VerificationRequestV1 {
    request_with_payloads(
        challenge,
        LOAD_BYTES.len() as u64,
        sha256(LOAD_BYTES),
        HSACO_BYTES.len() as u64,
        sha256(HSACO_BYTES),
    )
}

fn request_with_payloads(
    challenge: u8,
    load_len: u64,
    load_sha256: [u8; 32],
    hsaco_len: u64,
    hsaco_sha256: [u8; 32],
) -> WorkerV3VerificationRequestV1 {
    WorkerV3VerificationRequestV1::new(
        WorkerV3VerificationFreshChallengeV1::new([challenge; 32]).unwrap(),
        WorkerV3VerificationRosterIdentityV1::new([2; 32]).unwrap(),
        WorkerV3VerificationPolicyIdentityV1::new([3; 32]).unwrap(),
        WorkerV3VerificationMeasurementIdentityV1::new([4; 32]).unwrap(),
        WorkerV3VerificationFdPayloadDescriptorV1::load_envelope_v2(load_len, load_sha256).unwrap(),
        WorkerV3VerificationFdPayloadDescriptorV1::finalized_hsaco(hsaco_len, hsaco_sha256)
            .unwrap(),
        vec![
            WorkerV3VerificationEntryCoordinateV1::new(
                0,
                "kernel",
                "kernel_export",
                [5; 32],
                [6; 32],
                [7; 32],
            )
            .unwrap(),
        ],
    )
    .unwrap()
}

fn memfd(bytes: &[u8], seals: SealFlags) -> OwnedFd {
    let descriptor = rustix::fs::memfd_create(
        "fe2o3-worker-v3-client-test",
        MemfdFlags::CLOEXEC | MemfdFlags::ALLOW_SEALING,
    )
    .unwrap();
    let mut file = File::from(descriptor);
    file.write_all(bytes).unwrap();
    if !seals.is_empty() {
        rustix::fs::fcntl_add_seals(&file, seals).unwrap();
    }
    file.into()
}

fn snapshots(request: &WorkerV3VerificationRequestV1) -> WorkerV3VerificationPayloadSnapshotsV1 {
    WorkerV3VerificationPayloadSnapshotsV1::admit(
        request,
        vec![
            memfd(LOAD_BYTES, REQUIRED_SEALS),
            memfd(HSACO_BYTES, REQUIRED_SEALS),
        ],
    )
    .unwrap()
}

fn socketpair() -> (OwnedFd, OwnedFd) {
    rustix::net::socketpair(
        AddressFamily::UNIX,
        SocketType::SEQPACKET,
        SocketFlags::CLOEXEC,
        None,
    )
    .unwrap()
}

fn receive_request(peer: &OwnedFd) -> (WorkerV3VerificationRequestV1, Vec<OwnedFd>) {
    let mut bytes = [0_u8; MAX_WORKER_V3_VERIFICATION_REQUEST_BYTES_V1 + 1];
    let mut control_space = [MaybeUninit::uninit(); rustix::cmsg_space!(ScmRights(3))];
    let mut control = RecvAncillaryBuffer::new(&mut control_space);
    let received = {
        let mut vectors = [IoSliceMut::new(&mut bytes)];
        rustix::net::recvmsg(
            peer,
            &mut vectors,
            &mut control,
            RecvFlags::CMSG_CLOEXEC | RecvFlags::TRUNC,
        )
        .unwrap()
    };
    assert!(
        !received
            .flags
            .intersects(ReturnFlags::TRUNC | ReturnFlags::CTRUNC)
    );
    let mut descriptors = Vec::new();
    let mut messages = 0;
    for message in control.drain() {
        messages += 1;
        match message {
            RecvAncillaryMessage::ScmRights(rights) => descriptors.extend(rights),
            _ => panic!("unexpected request ancillary message"),
        }
    }
    assert_eq!(messages, 1);
    assert_eq!(descriptors.len(), 2);
    let request =
        WorkerV3VerificationRequestV1::decode_canonical(&bytes[..received.bytes]).unwrap();
    (request, descriptors)
}

fn read_exact_at(file: &OwnedFd, len: usize) -> Vec<u8> {
    let mut bytes = vec![0; len];
    let mut offset = 0;
    while offset < len {
        let count = rustix::io::pread(file, &mut bytes[offset..], offset as u64).unwrap();
        assert_ne!(count, 0);
        offset += count;
    }
    bytes
}

fn send_response(peer: &OwnedFd, bytes: &[u8]) {
    assert_eq!(
        rustix::net::send(peer, bytes, SendFlags::NOSIGNAL).unwrap(),
        bytes.len()
    );
}

#[test]
fn one_shot_exchange_preserves_order_and_grants_no_authority() {
    let request = request(1);
    let snapshots = snapshots(&request);
    assert!(!snapshots.grants_authority());
    let (client_peer, service_peer) = socketpair();
    let service = thread::spawn(move || {
        let (request, descriptors) = receive_request(&service_peer);
        assert_eq!(read_exact_at(&descriptors[0], LOAD_BYTES.len()), LOAD_BYTES);
        assert_eq!(
            read_exact_at(&descriptors[1], HSACO_BYTES.len()),
            HSACO_BYTES
        );
        for descriptor in &descriptors {
            assert_eq!(
                rustix::io::fcntl_getfd(descriptor).unwrap(),
                rustix::io::FdFlags::CLOEXEC
            );
            assert_eq!(
                rustix::fs::fcntl_getfl(descriptor).unwrap() & OFlags::ACCMODE,
                OFlags::RDONLY
            );
        }
        assert_eq!(
            rustix::fs::seek(&descriptors[0], SeekFrom::Current(0)).unwrap(),
            0
        );
        assert_eq!(
            rustix::fs::seek(&descriptors[1], SeekFrom::Current(0)).unwrap(),
            0
        );
        let mut eof = [0_u8; 1];
        assert_eq!(
            rustix::net::recv(&service_peer, &mut eof, RecvFlags::empty())
                .unwrap()
                .1,
            0
        );
        let response = WorkerV3VerificationResponseV1::new(
            &request,
            WorkerV3VerificationResponseDispositionV1::RequestFramed,
            WorkerV3VerificationTranscriptIdentityV1::new([8; 32]).unwrap(),
        );
        send_response(&service_peer, response.encode_canonical());
    });

    let client = WorkerV3VerificationClientV1::admit(client_peer, Duration::from_secs(2)).unwrap();
    assert!(!client.authenticates_peer());
    let receipt = client.exchange(request, snapshots).unwrap();
    assert_eq!(
        receipt.response().disposition(),
        WorkerV3VerificationResponseDispositionV1::RequestFramed
    );
    assert!(!receipt.authenticates_peer());
    assert!(!receipt.grants_theorem_authority());
    assert!(!receipt.grants_load_authority());
    assert!(!receipt.grants_launch_authority());
    service.join().unwrap();
}

#[test]
fn snapshot_admission_accepts_exact_future_write_seal_superset() {
    let request = request(1);
    let snapshots = WorkerV3VerificationPayloadSnapshotsV1::admit(
        &request,
        vec![
            memfd(LOAD_BYTES, REQUIRED_SEALS | SealFlags::FUTURE_WRITE),
            memfd(HSACO_BYTES, REQUIRED_SEALS | SealFlags::FUTURE_WRITE),
        ],
    )
    .unwrap();
    assert!(!snapshots.grants_authority());
}

#[test]
fn snapshot_admission_rejects_missing_and_extra_descriptors() {
    let request = request(1);
    let missing = WorkerV3VerificationPayloadSnapshotsV1::admit(
        &request,
        vec![memfd(LOAD_BYTES, REQUIRED_SEALS)],
    )
    .unwrap_err();
    assert!(matches!(
        missing,
        WorkerV3VerificationClientErrorV1::DescriptorCount {
            expected: 2,
            actual: 1
        }
    ));

    let extra = WorkerV3VerificationPayloadSnapshotsV1::admit(
        &request,
        vec![
            memfd(LOAD_BYTES, REQUIRED_SEALS),
            memfd(HSACO_BYTES, REQUIRED_SEALS),
            memfd(b"extra", REQUIRED_SEALS),
        ],
    )
    .unwrap_err();
    assert!(matches!(
        extra,
        WorkerV3VerificationClientErrorV1::DescriptorCount {
            expected: 2,
            actual: 3
        }
    ));
}

#[test]
fn snapshot_admission_rejects_reordered_descriptors() {
    assert_eq!(LOAD_BYTES.len(), HSACO_BYTES.len());
    let request = request(1);
    let error = WorkerV3VerificationPayloadSnapshotsV1::admit(
        &request,
        vec![
            memfd(HSACO_BYTES, REQUIRED_SEALS),
            memfd(LOAD_BYTES, REQUIRED_SEALS),
        ],
    )
    .unwrap_err();
    assert!(matches!(
        error,
        WorkerV3VerificationClientErrorV1::PayloadDigestMismatch { .. }
    ));
}

#[test]
fn snapshot_admission_rejects_mutable_and_partially_sealed_memfds() {
    let request = request(1);
    for load in [
        memfd(LOAD_BYTES, SealFlags::empty()),
        memfd(
            LOAD_BYTES,
            SealFlags::GROW | SealFlags::SHRINK | SealFlags::SEAL,
        ),
    ] {
        let error = WorkerV3VerificationPayloadSnapshotsV1::admit(
            &request,
            vec![load, memfd(HSACO_BYTES, REQUIRED_SEALS)],
        )
        .unwrap_err();
        assert!(matches!(
            error,
            WorkerV3VerificationClientErrorV1::PayloadNotImmutable { .. }
        ));
    }
}

#[test]
fn snapshot_admission_rejects_wrong_descriptor_type() {
    let request = request(1);
    let (wrong, _other) = UnixStream::pair().unwrap();
    let error = WorkerV3VerificationPayloadSnapshotsV1::admit(
        &request,
        vec![OwnedFd::from(wrong), memfd(HSACO_BYTES, REQUIRED_SEALS)],
    )
    .unwrap_err();
    assert!(matches!(
        error,
        WorkerV3VerificationClientErrorV1::PayloadNotRegular { .. }
    ));

    let regular = tempfile::tempfile().unwrap();
    let error = WorkerV3VerificationPayloadSnapshotsV1::admit(
        &request,
        vec![OwnedFd::from(regular), memfd(HSACO_BYTES, REQUIRED_SEALS)],
    )
    .unwrap_err();
    assert!(matches!(
        error,
        WorkerV3VerificationClientErrorV1::PayloadNotMemfd { .. }
            | WorkerV3VerificationClientErrorV1::PayloadLinked { .. }
    ));
}

#[test]
fn snapshot_admission_rejects_trailing_short_and_digest_mismatch() {
    let trailing_request = request_with_payloads(
        1,
        LOAD_BYTES.len() as u64,
        sha256(LOAD_BYTES),
        HSACO_BYTES.len() as u64,
        sha256(HSACO_BYTES),
    );
    let mut with_trailing = LOAD_BYTES.to_vec();
    with_trailing.push(0);
    let error = WorkerV3VerificationPayloadSnapshotsV1::admit(
        &trailing_request,
        vec![
            memfd(&with_trailing, REQUIRED_SEALS),
            memfd(HSACO_BYTES, REQUIRED_SEALS),
        ],
    )
    .unwrap_err();
    assert!(matches!(
        error,
        WorkerV3VerificationClientErrorV1::TrailingPayloadBytes { .. }
    ));

    let short_request = request_with_payloads(
        1,
        (LOAD_BYTES.len() + 1) as u64,
        sha256(LOAD_BYTES),
        HSACO_BYTES.len() as u64,
        sha256(HSACO_BYTES),
    );
    let error = WorkerV3VerificationPayloadSnapshotsV1::admit(
        &short_request,
        vec![
            memfd(LOAD_BYTES, REQUIRED_SEALS),
            memfd(HSACO_BYTES, REQUIRED_SEALS),
        ],
    )
    .unwrap_err();
    assert!(matches!(
        error,
        WorkerV3VerificationClientErrorV1::PayloadLengthMismatch { .. }
    ));

    let digest_request = request_with_payloads(
        1,
        LOAD_BYTES.len() as u64,
        [9; 32],
        HSACO_BYTES.len() as u64,
        sha256(HSACO_BYTES),
    );
    let error = WorkerV3VerificationPayloadSnapshotsV1::admit(
        &digest_request,
        vec![
            memfd(LOAD_BYTES, REQUIRED_SEALS),
            memfd(HSACO_BYTES, REQUIRED_SEALS),
        ],
    )
    .unwrap_err();
    assert!(matches!(
        error,
        WorkerV3VerificationClientErrorV1::PayloadDigestMismatch { .. }
    ));
}

#[test]
fn exchange_rejects_response_for_another_request() {
    let other = request(9);
    let request = request(1);
    let snapshots = snapshots(&request);
    let (client_peer, service_peer) = socketpair();
    let service = thread::spawn(move || {
        let _ = receive_request(&service_peer);
        let response = WorkerV3VerificationResponseV1::new(
            &other,
            WorkerV3VerificationResponseDispositionV1::RequestFramed,
            WorkerV3VerificationTranscriptIdentityV1::new([8; 32]).unwrap(),
        );
        send_response(&service_peer, response.encode_canonical());
    });
    let client = WorkerV3VerificationClientV1::admit(client_peer, Duration::from_secs(2)).unwrap();
    let error = client.exchange(request, snapshots).unwrap_err();
    assert!(matches!(
        error,
        WorkerV3VerificationClientErrorV1::ResponseRequestMismatch
    ));
    service.join().unwrap();
}

#[test]
fn snapshot_bundle_cannot_be_rebound_to_another_request() {
    let original = request(1);
    let snapshots = snapshots(&original);
    let replacement = request(9);
    let (client_peer, service_peer) = socketpair();
    let client = WorkerV3VerificationClientV1::admit(client_peer, Duration::from_secs(2)).unwrap();
    let error = client.exchange(replacement, snapshots).unwrap_err();
    assert!(matches!(
        error,
        WorkerV3VerificationClientErrorV1::SnapshotRequestMismatch
    ));
    drop(service_peer);
}

fn response_shape_error(
    mutator: impl FnOnce(Vec<u8>) -> Vec<u8> + Send + 'static,
) -> WorkerV3VerificationClientErrorV1 {
    let request = request(1);
    let snapshots = snapshots(&request);
    let (client_peer, service_peer) = socketpair();
    let service = thread::spawn(move || {
        let (request, _) = receive_request(&service_peer);
        let response = WorkerV3VerificationResponseV1::new(
            &request,
            WorkerV3VerificationResponseDispositionV1::RequestFramed,
            WorkerV3VerificationTranscriptIdentityV1::new([8; 32]).unwrap(),
        );
        send_response(
            &service_peer,
            &mutator(response.encode_canonical().to_vec()),
        );
    });
    let client = WorkerV3VerificationClientV1::admit(client_peer, Duration::from_secs(2)).unwrap();
    let error = client.exchange(request, snapshots).unwrap_err();
    service.join().unwrap();
    error
}

#[test]
fn exchange_rejects_truncated_and_oversize_responses() {
    let truncated = response_shape_error(|mut bytes| {
        bytes.pop();
        bytes
    });
    assert!(matches!(
        truncated,
        WorkerV3VerificationClientErrorV1::ResponseTruncated { .. }
    ));

    let oversize = response_shape_error(|mut bytes| {
        bytes.push(0);
        bytes
    });
    assert!(matches!(
        oversize,
        WorkerV3VerificationClientErrorV1::ResponseOversize { .. }
    ));
}

#[test]
fn exchange_rejects_response_ancillary_data() {
    let request = request(1);
    let snapshots = snapshots(&request);
    let (client_peer, service_peer) = socketpair();
    let service = thread::spawn(move || {
        let (request, _) = receive_request(&service_peer);
        let response = WorkerV3VerificationResponseV1::new(
            &request,
            WorkerV3VerificationResponseDispositionV1::RequestFramed,
            WorkerV3VerificationTranscriptIdentityV1::new([8; 32]).unwrap(),
        );
        let descriptor = memfd(b"forbidden", REQUIRED_SEALS);
        let descriptors = [descriptor.as_fd()];
        let mut control_space = [MaybeUninit::uninit(); rustix::cmsg_space!(ScmRights(1))];
        let mut control = SendAncillaryBuffer::new(&mut control_space);
        assert!(control.push(SendAncillaryMessage::ScmRights(&descriptors)));
        assert_eq!(
            rustix::net::sendmsg(
                &service_peer,
                &[IoSlice::new(response.encode_canonical())],
                &mut control,
                SendFlags::NOSIGNAL,
            )
            .unwrap(),
            response.encode_canonical().len()
        );
    });
    let client = WorkerV3VerificationClientV1::admit(client_peer, Duration::from_secs(2)).unwrap();
    let error = client.exchange(request, snapshots).unwrap_err();
    assert!(matches!(
        error,
        WorkerV3VerificationClientErrorV1::ResponseAncillaryData
    ));
    service.join().unwrap();
}
