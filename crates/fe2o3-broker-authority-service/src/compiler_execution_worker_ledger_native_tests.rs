//! Ordinary fixtures exercise the service's actual durable consumer, not public
//! protected-process admission, independent administration, or GPU authority.
use super::super::worker::{ANCHOR, WORKER};
use super::*;
use ed25519_dalek::{Signer, SigningKey};
use fe2o3_artifact_transaction::MAX_COMPILER_MODULE_HANDOFF_STORAGE_V4 as STORAGE_LIMIT;
use fe2o3_external_anchor_protocol::{
    AnchorChallengeV1, AnchorPositionV1, AnchorTransitionReceiptV1, PinnedAnchorKeyV1,
    UnsignedAnchorObservationV1,
};

#[allow(dead_code)]
#[path = "../../fe2o3-compiler-execution-protocol/tests/support/native_attestation_fixture.rs"]
mod fixture;

fn subject(seed: u8, b: &mut Budget<'_>) -> Subject {
    let floor = b.storage();
    let (v, storage) = b
        .with_prepaid_scope(floor, 8, 8, fixture::SUBJECT_BYTES, |b| {
            let mut bytes = fixture::subject_wire(2);
            bytes[32..48].fill(seed);
            fixture::seal(&mut bytes, "INERT-COMPILER-EXECUTION-SUBJECT", 2);
            Subject::decode(&bytes, b)
        })
        .unwrap();
    assert_eq!(b.storage(), floor);
    b.reserve_storage(storage.retained_storage()).unwrap();
    v
}
fn issued(
    ledger: &mut Ledger,
    p: &Policy,
    key: &Key,
    seed: u8,
    b: &mut Budget<'_>,
) -> (Request, Publication) {
    let s = subject(seed, b);
    let next = ledger.record.fixture_prepared(s, p, key, b).unwrap();
    b.reserve_storage(Record::STORAGE).unwrap();
    ledger.commit(next, b).unwrap();
    let next = ledger.record.fixture_issued(p, key, b).unwrap();
    b.reserve_storage(Record::STORAGE).unwrap();
    ledger.commit(next, b).unwrap();
    let Body::Issued { request, .. } = &ledger.record.body else {
        panic!("issued fixture");
    };
    let q = retain(Request::decode(request.canonical_bytes(), b).unwrap(), b).unwrap();
    let u = ledger.record.publication(b).unwrap();
    (q, u)
}
fn observation(c: &AnchorChallengeV1, position: AnchorPositionV1) -> AnchorTransitionReceiptV1 {
    let key = SigningKey::from_bytes(&[9; 32]);
    let unsigned = UnsignedAnchorObservationV1::from_challenge(c, position);
    let signature = key.sign(&unsigned.signing_bytes()).to_bytes();
    let signed = unsigned.attach_signature(signature);
    AnchorTransitionReceiptV1::new(
        c.clone(),
        &signed,
        &PinnedAnchorKeyV1::from_bytes(key.verifying_key().to_bytes()).unwrap(),
    )
    .unwrap()
}
fn proposed(c: &AnchorChallengeV1, _: &mut Budget<'_>) -> Result<AnchorTransitionReceiptV1> {
    Ok(observation(c, AnchorPositionV1::Proposed))
}
fn snapshot(root: &std::path::Path) -> Vec<(std::ffi::OsString, Vec<u8>, u64, i64, i64)> {
    let mut entries: Vec<_> = std::fs::read_dir(root)
        .unwrap()
        .map(|e| {
            let e = e.unwrap();
            let m = e.metadata().unwrap();
            (
                e.file_name(),
                std::fs::read(e.path()).unwrap(),
                m.ino(),
                m.ctime(),
                m.ctime_nsec(),
            )
        })
        .collect();
    entries.sort_by(|a, b| a.0.cmp(&b.0));
    entries
}

#[test]
fn native_worker_publication_replay_second_sequence_and_currentness() {
    let mut work = Work::new(usize::MAX);
    let mut b = Budget::new(&mut work, STORAGE_LIMIT);
    b.reserve_storage(65536).unwrap();
    let (p, key) = custody(&mut b);
    let (_dir, root) = directory();
    let mut ledger = Ledger::recover(root.as_fd(), &p, &key, &mut b).unwrap();
    b.reserve_storage(Ledger::STORAGE).unwrap();
    for seed in [1, 2] {
        let (q, u) = issued(&mut ledger, &p, &key, seed, &mut b);
        let floor = b.storage();
        let (ack, advanced) = ledger
            .publish(&p, &key, &q, &u, &mut proposed, &mut b)
            .unwrap();
        assert!(advanced);
        assert_eq!(b.storage(), floor);
        b.reserve_storage(ack.retained_storage()).unwrap();
        let (same, advanced) = ledger
            .publish(
                &p,
                &key,
                &q,
                &u,
                &mut |_, _| panic!("replay must not exchange"),
                &mut b,
            )
            .unwrap();
        assert!(!advanced);
        assert_eq!(same.canonical_bytes(), ack.canonical_bytes());
        let carriage = ledger
            .recover_carriage(q.subject(), &mut b)
            .unwrap()
            .unwrap();
        b.reserve_storage(carriage.retained_storage()).unwrap();
        let current = ledger
            .verify_current(&carriage, [81; 32], &mut proposed, &mut b)
            .unwrap();
        b.reserve_storage(std::mem::size_of::<(Current, ProtocolStorage)>())
            .unwrap();
        let attestation = retain(
            key.attest_current(&p, &carriage, current, [81; 32], &mut b)
                .unwrap(),
            &mut b,
        )
        .unwrap();
        assert!(
            attestation
                .verify_native(&p, &carriage, [81; 32], &mut b)
                .is_ok()
        );
        assert_eq!(ledger.record.sequence, u64::from(seed) + 1);
    }
    drop(ledger);
    let recovered = Ledger::recover(root.as_fd(), &p, &key, &mut b).unwrap();
    assert_eq!(recovered.record.sequence, 3);
    recovered.validate(&mut b).unwrap();
}

struct NthFault {
    boundary: Boundary,
    timing: Timing,
    remaining: usize,
    fired: bool,
}
impl RetainedDurableDirectoryHooksV1 for NthFault {
    fn record(&mut self, boundary: Boundary, timing: Timing) -> std::io::Result<()> {
        if (boundary, timing) == (self.boundary, self.timing) {
            self.remaining -= 1;
            if self.remaining == 0 {
                self.fired = true;
                return Err(std::io::Error::from_raw_os_error(libc::EIO));
            }
        }
        Ok(())
    }
}

#[test]
fn native_worker_every_publication_commit_boundary_recovers_exactly_once() {
    for commit in 1..=5 {
        for boundary in [
            Boundary::CreateTemp,
            Boundary::WriteTemp,
            Boundary::SyncTemp,
            Boundary::RenameTempToRedo,
            Boundary::SyncRedoName,
            Boundary::RenameRedoToCanonical,
            Boundary::SyncCanonicalName,
        ] {
            for timing in [Timing::Before, Timing::After] {
                let mut work = Work::new(usize::MAX);
                let mut b = Budget::new(&mut work, STORAGE_LIMIT);
                b.reserve_storage(65536).unwrap();
                let (p, key) = custody(&mut b);
                let (_dir, root) = directory();
                let mut ledger = Ledger::recover(root.as_fd(), &p, &key, &mut b).unwrap();
                b.reserve_storage(Ledger::STORAGE).unwrap();
                let (q, u) = issued(&mut ledger, &p, &key, 1, &mut b);
                let mut challenge = None;
                let mut exchange = |c: &AnchorChallengeV1, _: &mut Budget<'_>| {
                    if let Some(prior) = &challenge {
                        assert_eq!(c, prior);
                    } else {
                        challenge = Some(c.clone());
                    }
                    Ok(observation(c, AnchorPositionV1::Proposed))
                };
                let mut fault = NthFault {
                    boundary,
                    timing,
                    remaining: commit,
                    fired: false,
                };
                let floor = b.storage();
                assert!(
                    ledger
                        .publish_with_hooks(&p, &key, &q, &u, &mut exchange, &mut fault, &mut b)
                        .is_err()
                );
                assert!(fault.fired, "{commit}/{boundary:?}/{timing:?}");
                assert_eq!(b.storage(), floor);
                drop(ledger);
                let mut ledger = Ledger::recover(root.as_fd(), &p, &key, &mut b).unwrap();
                let (ack, _) = ledger
                    .publish(&p, &key, &q, &u, &mut exchange, &mut b)
                    .unwrap();
                assert_eq!(ledger.record.sequence, 2);
                b.reserve_storage(ack.retained_storage()).unwrap();
                let (replay, advanced) = ledger
                    .publish(
                        &p,
                        &key,
                        &q,
                        &u,
                        &mut |_, _| panic!("committed replay"),
                        &mut b,
                    )
                    .unwrap();
                assert!(!advanced);
                assert_eq!(ack.canonical_bytes(), replay.canonical_bytes());
            }
        }
    }
}

#[test]
fn native_worker_anchor_send_receive_loss_reuses_exact_persisted_challenge() {
    for remote_committed in [false, true] {
        let mut work = Work::new(usize::MAX);
        let mut b = Budget::new(&mut work, STORAGE_LIMIT);
        b.reserve_storage(65536).unwrap();
        let (p, key) = custody(&mut b);
        let (dir, root) = directory();
        let mut ledger = Ledger::recover(root.as_fd(), &p, &key, &mut b).unwrap();
        b.reserve_storage(Ledger::STORAGE).unwrap();
        let (q, u) = issued(&mut ledger, &p, &key, 1, &mut b);
        let mut saved = None;
        assert!(
            ledger
                .publish(
                    &p,
                    &key,
                    &q,
                    &u,
                    &mut |c, _| {
                        assert!(dir.path().join(ANCHOR.canonical).exists());
                        saved = Some((
                            c.clone(),
                            remote_committed.then(|| observation(c, AnchorPositionV1::Proposed)),
                        ));
                        Err(Error::rejected(
                            "fixture disconnect before/after remote commit",
                        ))
                    },
                    &mut b
                )
                .is_err()
        );
        assert!(!dir.path().join(WORKER.canonical).exists());
        drop(ledger);
        let mut ledger = Ledger::recover(root.as_fd(), &p, &key, &mut b).unwrap();
        let (c, receipt) = saved.unwrap();
        ledger
            .publish(
                &p,
                &key,
                &q,
                &u,
                &mut |retry, _| {
                    assert_eq!(&c, retry);
                    Ok(receipt
                        .clone()
                        .unwrap_or_else(|| observation(retry, AnchorPositionV1::Proposed)))
                },
                &mut b,
            )
            .unwrap();
        assert_eq!(ledger.record.sequence, 2);
    }
}

#[test]
fn native_worker_incomplete_publication_cannot_recover_or_attest_ack_carriage() {
    use fe2o3_compiler_execution_protocol::CompilerExecutionReceiptCarriageV2 as Carriage;
    // Commit 3 is Worker; commit 4 is Published anchor. In both cases the
    // signed issuer still says Issued, not advanced with a persisted Worker ACK.
    for commit in [3, 4] {
        let mut work = Work::new(usize::MAX);
        let mut b = Budget::new(&mut work, STORAGE_LIMIT);
        b.reserve_storage(65536).unwrap();
        let (p, key) = custody(&mut b);
        let (dir, root) = directory();
        let mut ledger = Ledger::recover(root.as_fd(), &p, &key, &mut b).unwrap();
        b.reserve_storage(Ledger::STORAGE).unwrap();
        let (q, u) = issued(&mut ledger, &p, &key, 1, &mut b);
        let mut fault = NthFault {
            boundary: Boundary::SyncCanonicalName,
            timing: Timing::After,
            remaining: commit,
            fired: false,
        };
        assert!(
            ledger
                .publish_with_hooks(&p, &key, &q, &u, &mut proposed, &mut fault, &mut b)
                .is_err()
        );
        assert!(fault.fired);
        drop(ledger);
        let mut ledger = Ledger::recover(root.as_fd(), &p, &key, &mut b).unwrap();
        assert!(matches!(ledger.record.body, Body::Issued { .. }));
        assert!(
            ledger
                .recover_carriage(q.subject(), &mut b)
                .unwrap()
                .is_none()
        );

        // The client can synthesize inert ACK bytes from a Worker identity;
        // those bytes must not make the interrupted transaction current.
        let wire = std::fs::read(dir.path().join(WORKER.canonical)).unwrap();
        let identity: [u8; 32] = wire[wire.len() - 32..].try_into().unwrap();
        let ack = retain(Ack::new(&u, identity, &mut b).unwrap(), &mut b).unwrap();
        let cp = retain(Policy::decode(p.canonical_bytes(), &mut b).unwrap(), &mut b).unwrap();
        let cq = retain(
            Request::decode(q.canonical_bytes(), &mut b).unwrap(),
            &mut b,
        )
        .unwrap();
        let cu = retain(
            Publication::decode(u.canonical_bytes(), &mut b).unwrap(),
            &mut b,
        )
        .unwrap();
        let inert = retain(Carriage::new(cp, cq, cu, ack, &mut b).unwrap(), &mut b).unwrap();
        let before = snapshot(dir.path());
        assert!(
            ledger
                .verify_current(
                    &inert,
                    [91; 32],
                    &mut |_, _| panic!("incomplete publication must refuse before exchange"),
                    &mut b
                )
                .is_err()
        );
        assert_eq!(snapshot(dir.path()), before);
        ledger
            .publish(
                &p,
                &key,
                &q,
                &u,
                &mut |_, _| panic!("committed anchor needs no exchange"),
                &mut b,
            )
            .unwrap();
        let carriage = ledger
            .recover_carriage(q.subject(), &mut b)
            .unwrap()
            .unwrap();
        b.reserve_storage(carriage.retained_storage()).unwrap();
        assert_eq!(carriage.canonical_bytes(), inert.canonical_bytes());
        assert!(
            ledger
                .verify_current(&carriage, [92; 32], &mut proposed, &mut b)
                .is_ok()
        );
    }
}

#[test]
fn native_worker_prior_position_abort_is_durable_and_terminal() {
    let mut work = Work::new(usize::MAX);
    let mut b = Budget::new(&mut work, STORAGE_LIMIT);
    b.reserve_storage(65536).unwrap();
    let (p, key) = custody(&mut b);
    let (dir, root) = directory();
    let mut ledger = Ledger::recover(root.as_fd(), &p, &key, &mut b).unwrap();
    b.reserve_storage(Ledger::STORAGE).unwrap();
    let (q, u) = issued(&mut ledger, &p, &key, 1, &mut b);
    assert!(
        ledger
            .publish(
                &p,
                &key,
                &q,
                &u,
                &mut |c, _| Ok(observation(c, AnchorPositionV1::Prior)),
                &mut b
            )
            .is_err()
    );
    assert!(!dir.path().join(WORKER.canonical).exists());
    drop(ledger);
    let mut ledger = Ledger::recover(root.as_fd(), &p, &key, &mut b).unwrap();
    assert!(
        ledger
            .publish(
                &p,
                &key,
                &q,
                &u,
                &mut |_, _| panic!("abort cannot restart transaction"),
                &mut b
            )
            .is_err()
    );
    assert_eq!(ledger.record.sequence, 1);
}

#[test]
fn native_worker_fresh_currentness_rejects_stale_receipt_and_post_exchange_replacement() {
    let mut work = Work::new(usize::MAX);
    let mut b = Budget::new(&mut work, STORAGE_LIMIT);
    b.reserve_storage(65536).unwrap();
    let (p, key) = custody(&mut b);
    let (dir, root) = directory();
    let mut ledger = Ledger::recover(root.as_fd(), &p, &key, &mut b).unwrap();
    b.reserve_storage(Ledger::STORAGE).unwrap();
    let (q, u) = issued(&mut ledger, &p, &key, 1, &mut b);
    ledger
        .publish(&p, &key, &q, &u, &mut proposed, &mut b)
        .unwrap();
    let carriage = ledger
        .recover_carriage(q.subject(), &mut b)
        .unwrap()
        .unwrap();
    b.reserve_storage(carriage.retained_storage()).unwrap();
    let mut saved = None;
    ledger
        .verify_current(
            &carriage,
            [61; 32],
            &mut |c, _| {
                let receipt = observation(c, AnchorPositionV1::Proposed);
                saved = Some(receipt.clone());
                Ok(receipt)
            },
            &mut b,
        )
        .unwrap();
    let before = snapshot(dir.path());
    let floor = b.storage();
    assert!(
        ledger
            .verify_current(
                &carriage,
                [62; 32],
                &mut |_, _| Ok(saved.clone().unwrap()),
                &mut b
            )
            .is_err()
    );
    assert_eq!(b.storage(), floor);
    assert_eq!(snapshot(dir.path()), before);
    let path = dir.path().join(WORKER.canonical);
    let bytes = std::fs::read(&path).unwrap();
    assert!(
        ledger
            .verify_current(
                &carriage,
                [63; 32],
                &mut |c, _| {
                    let mut changed = bytes.clone();
                    changed[24] ^= 1;
                    std::fs::write(&path, changed).unwrap();
                    Ok(observation(c, AnchorPositionV1::Proposed))
                },
                &mut b
            )
            .is_err()
    );
    std::fs::write(path, bytes).unwrap();
    let (next_q, next_u) = issued(&mut ledger, &p, &key, 2, &mut b);
    ledger
        .publish(&p, &key, &next_q, &next_u, &mut proposed, &mut b)
        .unwrap();
    assert!(
        ledger
            .verify_current(
                &carriage,
                [64; 32],
                &mut |_, _| panic!("stale carriage must refuse before exchange"),
                &mut b
            )
            .is_err()
    );
    assert!(
        ledger
            .publish(
                &p,
                &key,
                &q,
                &u,
                &mut |_, _| panic!("stale publication must refuse before exchange"),
                &mut b
            )
            .is_err()
    );
}

#[test]
fn native_worker_cross_journal_substitution_refuses_before_any_recovery_mutation() {
    let mut work = Work::new(usize::MAX);
    let mut b = Budget::new(&mut work, STORAGE_LIMIT);
    b.reserve_storage(65536).unwrap();
    let (p, key) = custody(&mut b);
    let (dir, root) = directory();
    let mut ledger = Ledger::recover(root.as_fd(), &p, &key, &mut b).unwrap();
    b.reserve_storage(Ledger::STORAGE).unwrap();
    let (q, u) = issued(&mut ledger, &p, &key, 1, &mut b);
    ledger
        .publish(&p, &key, &q, &u, &mut proposed, &mut b)
        .unwrap();
    let old = snapshot(dir.path());
    let (q, u) = issued(&mut ledger, &p, &key, 2, &mut b);
    ledger
        .publish(&p, &key, &q, &u, &mut proposed, &mut b)
        .unwrap();
    drop(ledger);
    for name in [
        WORKER.canonical,
        ANCHOR.canonical,
        "compiler-execution-issuer-v3.state",
    ] {
        let path = dir.path().join(name);
        let current = std::fs::read(&path).unwrap();
        let stale = &old
            .iter()
            .find(|e| e.0 == std::ffi::OsStr::new(name))
            .unwrap()
            .1;
        std::fs::write(&path, stale).unwrap();
        let before = snapshot(dir.path());
        let mut writes = RecoveryWrites::default();
        let floor = b.storage();
        assert!(
            Ledger::recover_with_hooks(root.as_fd(), &p, &key, &mut writes, &mut b).is_err(),
            "{name}"
        );
        assert_eq!(writes.0, 0, "{name}");
        assert_eq!(snapshot(dir.path()), before, "{name}");
        assert_eq!(b.storage(), floor);
        std::fs::write(path, current).unwrap();
    }
}

#[test]
fn native_worker_all_three_journal_recovery_crashes_preserve_publication() {
    use crate::compiler_execution_journal_recovery::test_support::RecoveryFault;
    for journal in 1..=3 {
        for mut fault in RecoveryFault::cases(journal) {
            let mut work = Work::new(usize::MAX);
            let mut b = Budget::new(&mut work, STORAGE_LIMIT);
            b.reserve_storage(65536).unwrap();
            let (p, key) = custody(&mut b);
            let (_dir, root) = directory();
            let mut ledger = Ledger::recover(root.as_fd(), &p, &key, &mut b).unwrap();
            b.reserve_storage(Ledger::STORAGE).unwrap();
            let (q, u) = issued(&mut ledger, &p, &key, 1, &mut b);
            let (ack, _) = ledger
                .publish(&p, &key, &q, &u, &mut proposed, &mut b)
                .unwrap();
            drop(ledger);
            assert!(
                Ledger::recover_with_hooks(root.as_fd(), &p, &key, &mut fault, &mut b).is_err()
            );
            assert!(fault.fired, "{journal}/{fault:?}");
            let mut ledger = Ledger::recover(root.as_fd(), &p, &key, &mut b).unwrap();
            let (replay, advanced) = ledger
                .publish(
                    &p,
                    &key,
                    &q,
                    &u,
                    &mut |_, _| panic!("already published"),
                    &mut b,
                )
                .unwrap();
            assert!(!advanced);
            assert_eq!(ack.canonical_bytes(), replay.canonical_bytes());
        }
    }
}

#[test]
fn native_worker_exhausted_original_meter_cannot_start_anchor_exchange() {
    let mut work = Work::new(usize::MAX);
    let mut b = Budget::new(&mut work, STORAGE_LIMIT);
    b.reserve_storage(65536).unwrap();
    let (p, key) = custody(&mut b);
    let (dir, root) = directory();
    let mut ledger = Ledger::recover(root.as_fd(), &p, &key, &mut b).unwrap();
    b.reserve_storage(Ledger::STORAGE).unwrap();
    let (q, u) = issued(&mut ledger, &p, &key, 1, &mut b);
    let before = snapshot(dir.path());
    b.charge_work(usize::MAX - b.work()).unwrap();
    assert!(b.charge_work(1).is_err());
    let floor = b.storage();
    assert!(
        ledger
            .publish(
                &p,
                &key,
                &q,
                &u,
                &mut |_, _| panic!("denied meter cannot exchange"),
                &mut b
            )
            .is_err()
    );
    assert_eq!(b.storage(), floor);
    assert_eq!(snapshot(dir.path()), before);
}
