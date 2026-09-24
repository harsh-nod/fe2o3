//! Public native client transcripts. Fixture keys are diagnostic only: these
//! tests do not establish protected service custody, journal durability or GPU authority.
use ed25519_dalek::{Signer, SigningKey};
use fe2o3_artifact_transaction::InertCompilerExecutionSubjectV2 as Subject;
use fe2o3_compiler_execution_client::{
    CompilerExecutionClientErrorV2 as Error, CompilerExecutionClientV2 as Client,
};
use fe2o3_compiler_execution_protocol::{
    CompilerExecutionAttestationReceiptV2 as Receipt,
    CompilerExecutionAttestationRequestV2 as Request,
    CompilerExecutionAttestationStorageV2 as Storage,
    CompilerExecutionCurrentRecordAttestationV3 as Current,
    CompilerExecutionCurrentRecordVerificationV3 as Verification,
    CompilerExecutionExternalAnchorTransactionV2 as Transaction,
    CompilerExecutionIssuerPolicyV2 as Policy, CompilerExecutionReceiptCarriageV2 as Carriage,
    CompilerExecutionReceiptPublicationAckV2 as Ack,
    CompilerExecutionReceiptPublicationV2 as Publication,
    CompilerExecutionServicePublishDispositionV1 as Disposition,
    CompilerExecutionServiceRequestKindV2 as QueryKind,
    CompilerExecutionServiceRequestPayloadV2 as QueryPayload,
    CompilerExecutionServiceRequestV2 as Query, CompilerExecutionServiceResponsePayloadV2 as Reply,
    CompilerExecutionServiceResponseV2 as Response,
    MAX_COMPILER_EXECUTION_SERVICE_REQUEST_BYTES_V2 as MAX_REQUEST,
};
use fe2o3_external_anchor_protocol::{
    AnchorChallengeV1, AnchorPositionV1, AnchorTransitionReceiptV1, AnchoredStateV1, CallerNonceV1,
    HashChainHeadV1, PinnedAnchorKeyV1, UnsignedAnchorObservationV1,
};
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
    CanonicalKernelIrWorkBudgetV1 as Work,
};
use rustix::net::{self, AddressFamily, RecvFlags, SendFlags, SocketFlags, SocketType};
use std::{mem::size_of, os::fd::OwnedFd, thread, time::Duration};

#[allow(dead_code)]
#[path = "../../fe2o3-compiler-execution-protocol/tests/support/native_attestation_fixture.rs"]
mod fixture;

const WORK: usize = 100_000_000;
const STORAGE: usize = 4 * 1024 * 1024;
const TIMEOUT: Duration = Duration::from_secs(10);
// Prepay fixed fixture wire arrays, keys and socket/anchor staging. Production
// output reservations are added separately, including every packet decode.
const FIXTURE_STORAGE: usize = 64 * 1024;

fn keep<T>((value, storage): (T, Storage), b: &mut Budget<'_>) -> T {
    b.reserve_storage(storage.additional_storage()).unwrap();
    value
}
fn policy(b: &mut Budget<'_>) -> Policy {
    keep(Policy::decode(&fixture::policy_wire(2), b).unwrap(), b)
}
fn subject(b: &mut Budget<'_>) -> Subject {
    let (subject, storage) = Subject::decode(&fixture::subject_wire(2), b).unwrap();
    b.reserve_storage(storage.retained_storage()).unwrap();
    subject
}
fn carriage(b: &mut Budget<'_>) -> Carriage {
    let policy = policy(b);
    let request = keep(Request::decode(&fixture::request_wire(2), b).unwrap(), b);
    let receipt = keep(
        Receipt::issue(&policy, &request, &SigningKey::from_bytes(&[0x51; 32]), b).unwrap(),
        b,
    );
    let publication = keep(
        Publication::new([0x81; 32], [0x82; 32], receipt, b).unwrap(),
        b,
    );
    let ack = keep(Ack::new(&publication, [0x83; 32], b).unwrap(), b);
    keep(
        Carriage::new(policy, request, publication, ack, b).unwrap(),
        b,
    )
}
fn pair() -> (OwnedFd, OwnedFd) {
    let pair = net::socketpair(
        AddressFamily::UNIX,
        SocketType::SEQPACKET,
        SocketFlags::CLOEXEC,
        None,
    )
    .unwrap();
    for fd in [&pair.0, &pair.1] {
        net::sockopt::set_socket_timeout(fd, net::sockopt::Timeout::Recv, Some(TIMEOUT)).unwrap();
        net::sockopt::set_socket_timeout(fd, net::sockopt::Timeout::Send, Some(TIMEOUT)).unwrap();
    }
    pair
}
fn receive(peer: &OwnedFd, b: &mut Budget<'_>) -> Query {
    let mut bytes = [0; MAX_REQUEST];
    let (_, n) = net::recv(peer, &mut bytes[..], RecvFlags::TRUNC).unwrap();
    assert!(n > 0 && n <= bytes.len());
    keep(Query::decode(&bytes[..n], b).unwrap(), b)
}
fn send(peer: &OwnedFd, bytes: &[u8]) {
    assert_eq!(
        net::send(peer, bytes, SendFlags::NOSIGNAL).unwrap(),
        bytes.len()
    );
}
fn respond(peer: &OwnedFd, query: &Query, policy: &Policy, reply: Reply<'_>, b: &mut Budget<'_>) {
    let response = keep(
        Response::new(query.identity(), policy, reply, b).unwrap(),
        b,
    );
    send(peer, response.canonical_bytes());
}
fn assert_closed(peer: &OwnedFd) {
    let mut bytes = [0; MAX_REQUEST];
    assert_eq!(
        net::recv(peer, &mut bytes[..], RecvFlags::empty())
            .unwrap()
            .1,
        0
    );
}

#[derive(Clone, Copy)]
enum Stage {
    Ready,
    Prepared,
    Issued,
    Published,
}
fn spawn_lifecycle(peer: OwnedFd, stage: Stage) -> thread::JoinHandle<usize> {
    thread::spawn(move || {
        let mut work = Work::new(WORK);
        let mut b = Budget::new(&mut work, STORAGE);
        b.reserve_storage(FIXTURE_STORAGE).unwrap();
        let c = carriage(&mut b);
        let mut packets = 0;
        for _ in 0..5 {
            let query = receive(&peer, &mut b);
            packets += 1;
            let reply = match query.kind() {
                QueryKind::Recover => {
                    let s = keep(query.decode_subject(&mut b).unwrap(), &mut b);
                    assert_eq!(s.canonical_bytes(), c.request().subject().canonical_bytes());
                    if matches!(stage, Stage::Published) {
                        Reply::Recovered(&c)
                    } else {
                        Reply::ReceiptAbsent {
                            sequence: 1,
                            prior_rollback_anchor: [0; 32],
                        }
                    }
                }
                QueryKind::Inspect => match stage {
                    Stage::Ready => Reply::Ready {
                        sequence: 1,
                        prior_rollback_anchor: [0; 32],
                    },
                    Stage::Prepared => Reply::Prepared(c.request().challenge()),
                    Stage::Issued => Reply::Issued(c.publication()),
                    Stage::Published => panic!("recovery must terminate"),
                },
                QueryKind::Prepare => Reply::Prepared(c.request().challenge()),
                QueryKind::Issue => {
                    let r = keep(query.decode_request(&mut b).unwrap(), &mut b);
                    assert_eq!(r.canonical_bytes(), c.request().canonical_bytes());
                    Reply::Issued(c.publication())
                }
                QueryKind::Publish => {
                    let r = keep(query.decode_request(&mut b).unwrap(), &mut b);
                    let p = keep(query.decode_publication(&mut b).unwrap(), &mut b);
                    assert_eq!(r.canonical_bytes(), c.request().canonical_bytes());
                    assert_eq!(p.canonical_bytes(), c.publication().canonical_bytes());
                    Reply::Published {
                        acknowledgment: c.acknowledgment(),
                        disposition: Disposition::Advanced,
                    }
                }
                _ => panic!("unexpected transcript command"),
            };
            let terminal = matches!(reply, Reply::Recovered(_) | Reply::Published { .. });
            respond(&peer, &query, c.policy(), reply, &mut b);
            if terminal {
                assert_closed(&peer);
                return packets;
            }
        }
        panic!("native lifecycle exceeded five exchanges");
    })
}

#[test]
fn native_acquisition_and_all_recovery_suffixes_use_the_public_client() {
    for (stage, count) in [
        (Stage::Ready, 5),
        (Stage::Prepared, 4),
        (Stage::Issued, 3),
        (Stage::Published, 1),
    ] {
        let mut work = Work::new(WORK);
        let mut b = Budget::new(&mut work, STORAGE);
        b.reserve_storage(FIXTURE_STORAGE).unwrap();
        let p = policy(&mut b);
        let s = subject(&mut b);
        let expected = carriage(&mut b);
        let baseline = b.storage();
        let prefix = b.work();
        b.reserve_storage(Client::PEER_STORAGE).unwrap();
        let (peer, service) = pair();
        let server = spawn_lifecycle(service, stage);
        let result = Client::admit(peer, TIMEOUT, &mut b).unwrap().acquire(&p, s);
        let seen = server.join().unwrap();
        let (actual, charge) = result.unwrap();
        assert_eq!(seen, count);
        assert_eq!(actual.canonical_bytes(), expected.canonical_bytes());
        assert_eq!(charge.additional_storage(), actual.retained_storage());
        assert_eq!(b.storage(), baseline);
        assert!(b.work() > prefix);
        b.reserve_storage(charge.additional_storage()).unwrap();
        assert!(!actual.grants_compiler_authority());
        assert!(!actual.grants_launch_authority());
    }
}

#[test]
fn native_rejects_request_substitution_and_v1_without_retry() {
    for mixed_version in [false, true] {
        let mut work = Work::new(WORK);
        let mut b = Budget::new(&mut work, STORAGE);
        b.reserve_storage(FIXTURE_STORAGE).unwrap();
        let p = policy(&mut b);
        let s = subject(&mut b);
        let baseline = b.storage();
        b.reserve_storage(Client::PEER_STORAGE).unwrap();
        let (peer, service) = pair();
        let server = thread::spawn(move || {
            let mut work = Work::new(WORK);
            let mut b = Budget::new(&mut work, STORAGE);
            b.reserve_storage(FIXTURE_STORAGE).unwrap();
            let p = policy(&mut b);
            let original = receive(&service, &mut b);
            let other = keep(
                Query::new(&p, QueryPayload::Inspect, &mut b).unwrap(),
                &mut b,
            );
            assert_ne!(original.identity(), other.identity());
            let response = keep(
                Response::new(
                    other.identity(),
                    &p,
                    Reply::ReceiptAbsent {
                        sequence: 1,
                        prior_rollback_anchor: [0; 32],
                    },
                    &mut b,
                )
                .unwrap(),
                &mut b,
            );
            if mixed_version {
                use fe2o3_compiler_execution_protocol::{
                    CompilerExecutionIssuerPolicyV1 as LegacyPolicy,
                    CompilerExecutionServiceRequestV1 as LegacyRequest,
                    CompilerExecutionServiceResponseV1 as LegacyResponse,
                };
                let legacy = LegacyPolicy::decode(&fixture::policy_wire(1)).unwrap();
                let query = LegacyRequest::inspect(&legacy);
                let response =
                    LegacyResponse::receipt_absent(query.identity(), &legacy, 1, [0; 32]).unwrap();
                send(&service, response.canonical_bytes());
            } else {
                send(&service, response.canonical_bytes());
            }
            assert_closed(&service);
        });
        let result = Client::admit(peer, TIMEOUT, &mut b).unwrap().acquire(&p, s);
        server.join().unwrap();
        if mixed_version {
            assert!(matches!(result, Err(Error::Protocol(_))));
        } else {
            assert!(matches!(result, Err(Error::Mismatch(_))));
        }
        assert_eq!(b.storage(), baseline);
    }
}

fn signed_anchor(challenge: &AnchorChallengeV1, key: &SigningKey) -> AnchorTransitionReceiptV1 {
    let unsigned =
        UnsignedAnchorObservationV1::from_challenge(challenge, AnchorPositionV1::Proposed);
    let signed = unsigned.attach_signature(key.sign(&unsigned.signing_bytes()).to_bytes());
    AnchorTransitionReceiptV1::new(
        challenge.clone(),
        &signed,
        &PinnedAnchorKeyV1::from_bytes(key.verifying_key().to_bytes()).unwrap(),
    )
    .unwrap()
}
fn current(c: &Carriage, nonce: [u8; 32], b: &mut Budget<'_>) -> Current {
    let p = keep(Policy::decode(c.policy().canonical_bytes(), b).unwrap(), b);
    let r = keep(
        Request::decode(c.request().canonical_bytes(), b).unwrap(),
        b,
    );
    let publication = keep(
        Publication::decode(c.publication().canonical_bytes(), b).unwrap(),
        b,
    );
    let transaction = keep(Transaction::new(p, r, publication, b).unwrap(), b);
    let signer = SigningKey::from_bytes(&[0x52; 32]);
    let key = PinnedAnchorKeyV1::from_bytes(signer.verifying_key().to_bytes()).unwrap();
    let pending = AnchoredStateV1::from_local_state(0, HashChainHeadV1::from_bytes([0; 32]))
        .prepare(transaction.external_anchor_digest(b).unwrap(), &key)
        .unwrap()
        .begin_advance(CallerNonceV1::from_bytes([0x91; 32]), &key)
        .unwrap();
    let commit = signed_anchor(pending.challenge(), &signer);
    let challenge = keep(
        Verification::external_anchor_currentness_challenge_native(c, &commit, nonce, b).unwrap(),
        b,
    );
    let observation = signed_anchor(&challenge, &signer);
    let verification = keep(
        Verification::new_native(c, commit, observation, nonce, [0x94; 32], [0x95; 32], b).unwrap(),
        b,
    );
    keep(
        Current::issue_native(
            c.policy(),
            c,
            verification,
            nonce,
            &SigningKey::from_bytes(&[0x51; 32]),
            b,
        )
        .unwrap(),
        b,
    )
}

#[test]
fn native_currentness_authenticates_fresh_challenges_and_rejects_replay() {
    let mut previous = None;
    for replay in [false, false, true] {
        let mut work = Work::new(WORK);
        let mut b = Budget::new(&mut work, STORAGE);
        b.reserve_storage(FIXTURE_STORAGE).unwrap();
        let p = policy(&mut b);
        let c = carriage(&mut b);
        let baseline = b.storage();
        b.reserve_storage(Client::PEER_STORAGE).unwrap();
        let (peer, service) = pair();
        let server = thread::spawn(move || {
            let mut work = Work::new(WORK);
            let mut b = Budget::new(&mut work, STORAGE);
            b.reserve_storage(FIXTURE_STORAGE).unwrap();
            let query = receive(&service, &mut b);
            assert_eq!(query.kind(), QueryKind::VerifyCurrent);
            let c = keep(query.decode_carriage(&mut b).unwrap(), &mut b);
            let nonce = query.verification_challenge().unwrap();
            assert_ne!(nonce, [0; 32]);
            let attestation = current(&c, if replay { previous.unwrap() } else { nonce }, &mut b);
            respond(
                &service,
                &query,
                c.policy(),
                Reply::VerifiedCurrent(&attestation),
                &mut b,
            );
            assert_closed(&service);
            nonce
        });
        let result = Client::admit(peer, TIMEOUT, &mut b)
            .unwrap()
            .verify_current_only(&p, &c);
        let nonce = server.join().unwrap();
        assert_ne!(Some(nonce), previous);
        if replay {
            assert!(matches!(result, Err(Error::Journal(_))));
        } else {
            let (verified, charge) = result.unwrap();
            assert_eq!(verified.attestation().challenge(), nonce);
            assert!(!verified.grants_authority());
            assert_eq!(
                charge.additional_storage(),
                size_of::<(
                    fe2o3_compiler_execution_protocol::VerifiedCompilerExecutionCurrentRecordV3,
                    Storage
                )>()
            );
        }
        assert_eq!(b.storage(), baseline);
        previous = Some(nonce);
    }
}

#[test]
fn native_admission_resource_refusal_precedes_io_and_preserves_cumulative_work() {
    for (limit, storage) in [(7, STORAGE), (WORK, Client::PEER_STORAGE)] {
        let mut work = Work::new(limit);
        let mut b = Budget::new(&mut work, storage);
        b.reserve_storage(Client::PEER_STORAGE).unwrap();
        let (peer, service) = pair();
        let result = Client::admit(peer, TIMEOUT, &mut b);
        assert!(matches!(result, Err(Error::Resource(_))));
        drop(result);
        assert_closed(&service);
        assert_eq!(b.storage(), Client::PEER_STORAGE);
        assert_eq!(b.work(), if limit == 7 { 0 } else { 64 * 1024 });
    }
}

#[test]
fn native_acquisition_quota_exhaustion_closes_peer_without_a_packet() {
    let mut work = Work::new(WORK);
    let mut b = Budget::new(&mut work, STORAGE);
    b.reserve_storage(FIXTURE_STORAGE).unwrap();
    let p = policy(&mut b);
    let s = subject(&mut b);
    let baseline = b.storage();
    // Leave exactly the admission quota, then fail the first session charge.
    b.charge_work(WORK - b.work() - 64 * 1024).unwrap();
    b.reserve_storage(Client::PEER_STORAGE).unwrap();
    let (peer, service) = pair();
    let result = Client::admit(peer, TIMEOUT, &mut b).unwrap().acquire(&p, s);
    assert!(matches!(result, Err(Error::Resource(Resource::Work(_)))));
    assert_eq!(b.work(), WORK);
    assert_eq!(b.storage(), baseline);
    assert_closed(&service);
}
