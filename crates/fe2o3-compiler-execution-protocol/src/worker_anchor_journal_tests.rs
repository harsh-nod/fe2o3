use super::*;
use ed25519_dalek::{Signer, SigningKey};
use fe2o3_external_anchor_protocol::{
    AnchorPositionV1, AnchoredStateV1, CallerNonceV1, UnsignedAnchorObservationV1,
    derive_transaction_digest_v1,
};
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;
use sha2::{Digest, Sha256};

#[allow(dead_code)]
#[path = "../tests/support/native_attestation_fixture.rs"]
mod fixture;
#[allow(dead_code)]
#[path = "../tests/support/native_publication_fixture.rs"]
mod publication_fixture;
#[allow(dead_code)]
#[path = "../tests/support/native_receipt_fixture.rs"]
mod receipt_fixture;

const WORK_LIMIT: usize = 100_000_000;
const STORAGE_LIMIT: usize = 4_000_000;

struct Probe<T> {
    result: Result<T>,
    work: usize,
    peak: usize,
    failed_work: Option<usize>,
    failed_storage: Option<usize>,
}

fn probe<T>(
    floor: usize,
    work: usize,
    storage: usize,
    f: impl FnOnce(&mut Budget<'_>) -> Result<T>,
) -> Probe<T> {
    let mut w = Work::new(work);
    let mut b = Budget::new(&mut w, storage);
    b.reserve_storage(floor).unwrap();
    let ledger = b.work_ledger_identity_v1();
    let result = f(&mut b);
    assert_eq!(b.storage(), floor);
    assert!(b.work_ledger_identity_v1() == ledger);
    Probe {
        result,
        work: b.work(),
        peak: b.peak_storage(),
        failed_work: b.failed_work(),
        failed_storage: b.failed_storage(),
    }
}

// Isolated component probes; the complete ownership chain below uses one ledger.
fn run<T: std::fmt::Debug>(floor: usize, f: impl FnOnce(&mut Budget<'_>) -> Result<T>) -> T {
    probe(floor, WORK_LIMIT, STORAGE_LIMIT, f).result.unwrap()
}

fn seal_record(bytes: &mut [u8], kind: &str, version: u16) {
    let end = bytes.len() - 32;
    let mut hash = Sha256::new();
    hash.update(format!("FE2O3/COMPILER-EXECUTION-{kind}/V{version}\0"));
    hash.update(&bytes[..end]);
    bytes[end..].copy_from_slice(&hash.finalize());
}

// Build each family's actual leaf schemas and signatures, never convert a V2 owner.
fn transaction_wire(version: u16, sequence: u64, prior: [u8; 32], nonce: u8) -> [u8; 1874] {
    let mut request = fixture::request_wire(version);
    request[120..152].fill(nonce);
    request[152..160].copy_from_slice(&sequence.to_le_bytes());
    request[160..192].copy_from_slice(&prior);
    fixture::seal(
        &mut request[24..224],
        "COMPILER-EXECUTION-CHALLENGE",
        version,
    );
    fixture::seal(&mut request, "COMPILER-EXECUTION-REQUEST", version);
    let mut receipt = receipt_fixture::receipt_wire(version);
    receipt[24..56].copy_from_slice(&request[914..]);
    receipt[136..168].copy_from_slice(&request[192..224]);
    receipt[168..200].fill(nonce);
    receipt[200..208].copy_from_slice(&sequence.to_le_bytes());
    receipt[208..240].copy_from_slice(&prior);
    receipt_fixture::rebuild(&mut receipt, version);
    let mut publication = publication_fixture::publication_wire(version);
    publication[120..152].copy_from_slice(&receipt[368..]);
    publication[152..552].copy_from_slice(&receipt);
    fixture::seal(
        &mut publication,
        "COMPILER-EXECUTION-RECEIPT-PUBLICATION",
        version,
    );
    let mut bytes = [0; 1874];
    fixture::header(
        &mut bytes,
        match version {
            2 => b"F2O3CAT2",
            3 => b"F2O3CAT3",
            _ => panic!("unsupported transaction fixture family"),
        },
        version,
    );
    bytes[24..240].copy_from_slice(&fixture::policy_wire(version));
    bytes[240..1186].copy_from_slice(&request);
    bytes[1186..1770].copy_from_slice(&publication);
    bytes[1770..1778].copy_from_slice(&sequence.to_le_bytes());
    bytes[1778..1810].copy_from_slice(&prior);
    bytes[1810..1842].copy_from_slice(&receipt[240..272]);
    seal_record(&mut bytes, "EXTERNAL-ANCHOR-TRANSACTION", version);
    bytes
}

struct Fixture {
    version: u16,
    transaction: [u8; 1874],
    challenge: AnchorChallengeV1,
}

impl Fixture {
    fn new(version: u16, sequence: u64, prior: [u8; 32], head: [u8; 32], nonce: u8) -> Self {
        let transaction = transaction_wire(version, sequence, prior, nonce);
        let key = anchor_key();
        let pending =
            AnchoredStateV1::from_local_state(sequence - 1, HashChainHeadV1::from_bytes(head))
                .prepare(derive_transaction_digest_v1(&transaction).unwrap(), &key)
                .unwrap()
                .begin_advance(CallerNonceV1::from_bytes([nonce; 32]), &key)
                .unwrap();
        Self {
            version,
            transaction,
            challenge: pending.challenge().clone(),
        }
    }

    fn genesis() -> Self {
        Self::new(SCHEMA.version, 1, [0; 32], [0; 32], 0x71)
    }

    fn prepared(&self) -> Journal {
        let (t, _) = run(T, |b| Ok(Transaction::decode(&self.transaction, b)?));
        run(t.retained_storage() + size_of::<AnchorChallengeV1>(), |b| {
            Journal::prepared(&t, &self.challenge, b)
        })
        .0
    }

    fn receipt(&self, position: AnchorPositionV1) -> AnchorTransitionReceiptV1 {
        let unsigned = UnsignedAnchorObservationV1::from_challenge(&self.challenge, position);
        let signature = SigningKey::from_bytes(&[0x52; 32])
            .sign(&unsigned.signing_bytes())
            .to_bytes();
        AnchorTransitionReceiptV1::new(
            self.challenge.clone(),
            &unsigned.attach_signature(signature),
            &anchor_key(),
        )
        .unwrap()
    }

    fn wire(
        &self,
        stage: Stage,
        receipt: Option<&AnchorTransitionReceiptV1>,
        worker: [u8; 32],
    ) -> [u8; N] {
        let mut bytes = [0; N];
        fixture::header(
            &mut bytes,
            match self.version {
                2 => b"F2O3CAJ2",
                3 => b"F2O3CAJ3",
                _ => panic!("unsupported journal fixture family"),
            },
            self.version,
        );
        bytes[24] = stage as u8;
        bytes[32..32 + T].copy_from_slice(&self.transaction);
        bytes[32 + T..32 + T + C].copy_from_slice(self.challenge.as_bytes());
        if let Some(receipt) = receipt {
            bytes[32 + T + C..32 + T + C + R].copy_from_slice(receipt.canonical_bytes());
        }
        bytes[N - 64..N - 32].copy_from_slice(&worker);
        seal_record(&mut bytes, "WORKER-ANCHOR-JOURNAL", self.version);
        bytes
    }
}

fn anchor_key() -> PinnedAnchorKeyV1 {
    PinnedAnchorKeyV1::from_bytes(
        SigningKey::from_bytes(&[0x52; 32])
            .verifying_key()
            .to_bytes(),
    )
    .unwrap()
}

fn record(prepared: &Journal, receipt: &AnchorTransitionReceiptV1) -> Journal {
    run(
        prepared.retained_storage() + size_of::<AnchorTransitionReceiptV1>(),
        |b| prepared.record_anchor_receipt(receipt, b),
    )
    .0
}

fn publish(committed: &Journal) -> Journal {
    run(committed.retained_storage(), |b| {
        committed.mark_published([0x91; 32], b)
    })
    .0
}

fn successor(next: &Journal, prior: &Journal) -> bool {
    run(next.retained_storage() + prior.retained_storage(), |b| {
        next.is_legal_successor_of(prior, b)
    })
}

#[test]
fn all_stages_match_independent_native_schema_and_round_trip() {
    assert_eq!(T, 1874);
    assert_eq!(N, 2682);
    assert_eq!(N, crate::COMPILER_EXECUTION_WORKER_ANCHOR_JOURNAL_BYTES_V2);
    assert_eq!(
        Journal::RETAINED_STORAGE,
        crate::CompilerExecutionWorkerAnchorJournalV2::RETAINED_STORAGE
    );
    let f = Fixture::genesis();
    let prepared = f.prepared();
    let proposed = f.receipt(AnchorPositionV1::Proposed);
    let prior = f.receipt(AnchorPositionV1::Prior);
    let committed = record(&prepared, &proposed);
    let published = publish(&committed);
    let aborted = record(&prepared, &prior);
    for (record, stage, receipt, worker) in [
        (&prepared, Stage::PreparedAnchor, None, [0; 32]),
        (&committed, Stage::AnchorCommitted, Some(&proposed), [0; 32]),
        (&published, Stage::Published, Some(&proposed), [0x91; 32]),
        (&aborted, Stage::Aborted, Some(&prior), [0; 32]),
    ] {
        let wire = f.wire(stage, receipt, worker);
        assert_eq!(record.canonical_bytes(), &wire);
        let (decoded, charge) = run(N, |b| Journal::decode(&wire, b));
        assert_eq!(decoded.canonical_bytes(), &wire);
        assert_eq!(decoded.transaction().canonical_bytes(), &f.transaction);
        assert_eq!(decoded.challenge(), &f.challenge);
        assert_eq!(decoded.stage(), stage);
        assert_eq!(decoded.receipt(), receipt);
        assert_eq!(decoded.worker_record_identity(), worker);
        assert_eq!(charge.additional_storage(), decoded.retained_storage());
    }
    assert!(prepared.is_genesis_prepared());
    assert!(!committed.is_genesis_prepared());
    assert!(!published.is_genesis_prepared());
    assert!(!aborted.is_genesis_prepared());
}

#[test]
fn successors_preserve_stages_retry_and_both_chain_positions() {
    let f = Fixture::genesis();
    let prepared = f.prepared();
    let committed = record(&prepared, &f.receipt(AnchorPositionV1::Proposed));
    let published = publish(&committed);
    let aborted = record(&prepared, &f.receipt(AnchorPositionV1::Prior));
    let stages = [&prepared, &committed, &published, &aborted];
    for (p, prior) in stages.iter().enumerate() {
        for (n, next) in stages.iter().enumerate() {
            assert_eq!(
                successor(next, prior),
                matches!((p, n), (0, 1) | (0, 3) | (1, 2))
            );
        }
    }
    let current = published.transaction().current_rollback_anchor();
    let head = f.challenge.proposed_head().to_bytes();
    let next = Fixture::new(SCHEMA.version, 2, current, head, 0x72).prepared();
    assert!(successor(&next, &published));
    assert!(!successor(&next, &aborted));
    assert!(!next.is_genesis_prepared());
    for (sequence, rollback, external) in [
        (3, current, head),
        (2, [0x41; 32], head),
        (2, current, [0x42; 32]),
    ] {
        let wrong = Fixture::new(SCHEMA.version, sequence, rollback, external, 0x72).prepared();
        assert!(!successor(&wrong, &published));
    }
    let retry = Fixture::new(SCHEMA.version, 1, [0; 32], [0; 32], 0x73).prepared();
    assert!(successor(&retry, &aborted));
    assert!(!successor(&retry, &published));
    assert!(!successor(&prepared, &aborted));
    // A different challenge over the same transaction cannot advance this journal.
    let mut changed = Fixture::genesis();
    changed.challenge = AnchoredStateV1::from_local_state(0, HashChainHeadV1::from_bytes([0; 32]))
        .prepare(
            derive_transaction_digest_v1(&changed.transaction).unwrap(),
            &anchor_key(),
        )
        .unwrap()
        .begin_advance(CallerNonceV1::from_bytes([0x74; 32]), &anchor_key())
        .unwrap()
        .challenge()
        .clone();
    let wrong = record(
        &changed.prepared(),
        &changed.receipt(AnchorPositionV1::Proposed),
    );
    assert!(!successor(&wrong, &prepared));
}

#[test]
fn illegal_transitions_precede_budget_and_receipt_validation() {
    let f = Fixture::genesis();
    let prepared = f.prepared();
    let receipt = f.receipt(AnchorPositionV1::Proposed);
    let committed = record(&prepared, &receipt);
    let published = publish(&committed);
    let aborted = record(&prepared, &f.receipt(AnchorPositionV1::Prior));
    for record in [&committed, &published, &aborted] {
        let p = probe(0, 0, 0, |b| record.record_anchor_receipt(&receipt, b));
        assert!(matches!(
            p.result,
            Err(Error::Frame(FrameError::IllegalTransition))
        ));
        assert_eq!((p.work, p.peak), (0, 0));
    }
    for (record, worker) in [
        (&prepared, [1; 32]),
        (&aborted, [1; 32]),
        (&published, [1; 32]),
        (&committed, [0; 32]),
    ] {
        let p = probe(0, 0, 0, |b| record.mark_published(worker, b));
        assert!(matches!(
            p.result,
            Err(Error::Frame(FrameError::IllegalTransition))
        ));
        assert_eq!((p.work, p.peak), (0, 0));
    }
}

fn boundaries(floor: usize, f: impl Fn(&mut Budget<'_>) -> Result<(Journal, Storage)>) {
    let base = probe(floor, WORK_LIMIT, STORAGE_LIMIT, &f);
    let (journal, charge) = base.result.unwrap();
    assert_eq!(charge.additional_storage(), journal.retained_storage());
    let exact = probe(floor, base.work, base.peak, &f);
    exact.result.unwrap();
    assert_eq!((exact.work, exact.peak), (base.work, base.peak));
    let work = probe(floor, base.work - 1, base.peak, &f);
    assert!(work.result.is_err());
    assert_eq!(work.failed_work, Some(base.work));
    assert!(work.work < base.work);
    let storage = probe(floor, base.work, base.peak - 1, &f);
    assert!(storage.result.is_err());
    assert_eq!(storage.failed_storage, Some(base.peak));
    let missing = probe(floor - 1, WORK_LIMIT, STORAGE_LIMIT, &f);
    assert!(matches!(
        missing.result,
        Err(Error::Resource(Resource::Accounting))
    ));
    assert_eq!((missing.work, missing.peak), (8, floor - 1));
    for limit in [0, 7, Journal::WORK - 1] {
        let denied = probe(floor, limit, STORAGE_LIMIT, &f);
        assert!(matches!(
            denied.result,
            Err(Error::Resource(Resource::Work(_)))
        ));
        assert_eq!(denied.work, if limit < 8 { 0 } else { 8 });
        assert_eq!(denied.peak, floor);
    }
    let frame = probe(floor, WORK_LIMIT, floor + Journal::FRAME - 1, &f);
    assert!(matches!(
        frame.result,
        Err(Error::Resource(Resource::Storage(_)))
    ));
    assert_eq!((frame.work, frame.peak), (Journal::WORK, floor));
    assert_eq!(frame.failed_storage, Some(floor + Journal::FRAME));
}

#[test]
fn original_budget_denials_preserve_exact_entry_order_and_nested_charges() {
    let f = Fixture::genesis();
    let (t, _) = run(T, |b| Ok(Transaction::decode(&f.transaction, b)?));
    let floor = t.retained_storage() + size_of::<AnchorChallengeV1>();
    boundaries(floor, |b| Journal::prepared(&t, &f.challenge, b));
    let prepared = f.prepared();
    let receipt = f.receipt(AnchorPositionV1::Proposed);
    boundaries(floor + size_of::<AnchorTransitionReceiptV1>(), |b| {
        prepared.record_anchor_receipt(&receipt, b)
    });
    let committed = record(&prepared, &receipt);
    boundaries(floor + size_of::<AnchorTransitionReceiptV1>(), |b| {
        committed.mark_published([1; 32], b)
    });
    for record in [&prepared, &committed] {
        boundaries(N, |b| Journal::decode(record.canonical_bytes(), b));
    }
    let floor = 2 * Journal::RETAINED_STORAGE;
    let exact = probe(floor, 16 * N, floor + 256, |b| {
        committed.is_legal_successor_of(&prepared, b)
    });
    assert!(exact.result.unwrap());
    assert_eq!((exact.work, exact.peak), (16 * N, floor + 256));
    let work = probe(floor, 16 * N - 1, floor + 256, |b| {
        committed.is_legal_successor_of(&prepared, b)
    });
    assert!(matches!(
        work.result,
        Err(Error::Resource(Resource::Work(_)))
    ));
    assert_eq!(work.work, 8);
    let storage = probe(floor, 16 * N, floor + 255, |b| {
        committed.is_legal_successor_of(&prepared, b)
    });
    assert!(matches!(
        storage.result,
        Err(Error::Resource(Resource::Storage(_)))
    ));
    let missing = probe(floor - 1, WORK_LIMIT, STORAGE_LIMIT, |b| {
        committed.is_legal_successor_of(&prepared, b)
    });
    assert!(matches!(
        missing.result,
        Err(Error::Resource(Resource::Accounting))
    ));
    assert_eq!(missing.work, 8);
}

#[test]
fn borrowed_inputs_and_each_full_output_stay_on_original_ledger() {
    let f = Fixture::genesis();
    let receipt = f.receipt(AnchorPositionV1::Proposed);
    let mut w = Work::new(WORK_LIMIT);
    let mut b = Budget::new(&mut w, STORAGE_LIMIT);
    let floor = T + size_of::<AnchorChallengeV1>() + size_of::<AnchorTransitionReceiptV1>() + 19;
    b.reserve_storage(floor).unwrap();
    let ledger = b.work_ledger_identity_v1();
    let (t, charge) = Transaction::decode(&f.transaction, &mut b).unwrap();
    b.reserve_storage(charge.additional_storage()).unwrap();
    let transaction_storage = t.retained_storage();
    let before = b.storage();
    let (prepared, charge) = Journal::prepared(&t, &f.challenge, &mut b).unwrap();
    assert_eq!(b.storage(), before);
    assert_eq!(charge.additional_storage(), Journal::RETAINED_STORAGE);
    b.reserve_storage(charge.additional_storage()).unwrap();
    let before = b.storage();
    let (committed, charge) = prepared.record_anchor_receipt(&receipt, &mut b).unwrap();
    assert_eq!(b.storage(), before);
    assert_eq!(charge.additional_storage(), Journal::RETAINED_STORAGE);
    b.reserve_storage(charge.additional_storage()).unwrap();
    let before = b.storage();
    let (published, charge) = committed.mark_published([0x91; 32], &mut b).unwrap();
    assert_eq!(b.storage(), before);
    assert_eq!(charge.additional_storage(), Journal::RETAINED_STORAGE);
    b.reserve_storage(charge.additional_storage()).unwrap();
    assert!(committed.is_legal_successor_of(&prepared, &mut b).unwrap());
    assert!(published.is_legal_successor_of(&committed, &mut b).unwrap());
    assert_eq!(
        b.storage(),
        floor + transaction_storage + 3 * Journal::RETAINED_STORAGE
    );
    assert!(b.work_ledger_identity_v1() == ledger);
    drop((prepared, committed, published, t));
    b.release_storage(transaction_storage + 3 * Journal::RETAINED_STORAGE)
        .unwrap();
    assert_eq!(b.storage(), floor);
}

#[test]
fn mixed_families_fail_at_outer_frame_or_actual_transaction_decode() {
    let f = Fixture::genesis();
    let other = Fixture::new(
        if SCHEMA.version == 2 { 3 } else { 2 },
        1,
        [0; 32],
        [0; 32],
        0x71,
    );
    let wire = other.wire(Stage::PreparedAnchor, None, [0; 32]);
    let outer = probe(N, WORK_LIMIT, STORAGE_LIMIT, |b| Journal::decode(&wire, b));
    assert!(matches!(
        outer.result,
        Err(Error::Frame(FrameError::InvalidEncoding(_)))
    ));
    assert_eq!(outer.work, Journal::WORK);
    let mut mixed = f.wire(Stage::PreparedAnchor, None, [0; 32]);
    mixed[32..32 + T].copy_from_slice(&other.transaction);
    seal_record(&mut mixed, "WORKER-ANCHOR-JOURNAL", SCHEMA.version);
    let inner = probe(N, WORK_LIMIT, STORAGE_LIMIT, |b| Journal::decode(&mixed, b));
    assert!(matches!(
        inner.result,
        Err(Error::Transaction(TransactionError::Framing(_)))
    ));
    assert_eq!(inner.work, Journal::WORK + Transaction::WORK);
    let mut nested = f.wire(Stage::PreparedAnchor, None, [0; 32]);
    nested[32 + 24..32 + 240].copy_from_slice(&fixture::policy_wire(other.version));
    seal_record(
        &mut nested[32..32 + T],
        "EXTERNAL-ANCHOR-TRANSACTION",
        SCHEMA.version,
    );
    seal_record(&mut nested, "WORKER-ANCHOR-JOURNAL", SCHEMA.version);
    assert!(matches!(
        probe(N, WORK_LIMIT, STORAGE_LIMIT, |b| Journal::decode(
            &nested, b
        ))
        .result,
        Err(Error::Transaction(TransactionError::Attestation(_)))
    ));
}

#[test]
fn challenges_receipts_and_stage_payloads_remain_exactly_bound() {
    let f = Fixture::genesis();
    let prepared = f.prepared();
    let (t, _) = run(T, |b| Ok(Transaction::decode(&f.transaction, b)?));
    let floor = t.retained_storage() + size_of::<AnchorChallengeV1>();
    for wrong in [
        Fixture::new(SCHEMA.version, 1, [0; 32], [0; 32], 0x72).challenge,
        Fixture::new(SCHEMA.version, 1, [0; 32], [0x41; 32], 0x71).challenge,
        Fixture::new(SCHEMA.version, 2, [0x42; 32], [0x43; 32], 0x71).challenge,
    ] {
        assert!(matches!(
            probe(floor, WORK_LIMIT, STORAGE_LIMIT, |b| Journal::prepared(
                &t, &wrong, b
            ))
            .result,
            Err(Error::Frame(FrameError::ChallengeMismatch))
        ));
    }
    let recovery = AnchoredStateV1::from_local_state(0, HashChainHeadV1::from_bytes([0; 32]))
        .prepare(
            derive_transaction_digest_v1(&f.transaction).unwrap(),
            &anchor_key(),
        )
        .unwrap()
        .begin_recovery(CallerNonceV1::from_bytes([0x71; 32]), &anchor_key())
        .unwrap();
    assert!(matches!(
        probe(floor, WORK_LIMIT, STORAGE_LIMIT, |b| Journal::prepared(
            &t,
            recovery.challenge(),
            b
        ))
        .result,
        Err(Error::Frame(FrameError::ChallengeMismatch))
    ));
    let other = Fixture::new(SCHEMA.version, 1, [0; 32], [0; 32], 0x72);
    let wrong_receipt = other.receipt(AnchorPositionV1::Proposed);
    assert!(matches!(
        probe(
            prepared.retained_storage() + size_of::<AnchorTransitionReceiptV1>(),
            WORK_LIMIT,
            STORAGE_LIMIT,
            |b| prepared.record_anchor_receipt(&wrong_receipt, b)
        )
        .result,
        Err(Error::Frame(FrameError::ReceiptMismatch))
    ));
    let proposed = f.receipt(AnchorPositionV1::Proposed);
    let prior = f.receipt(AnchorPositionV1::Prior);
    for (stage, receipt, worker) in [
        (Stage::PreparedAnchor, Some(&proposed), [0; 32]),
        (Stage::PreparedAnchor, None, [1; 32]),
        (Stage::AnchorCommitted, Some(&prior), [0; 32]),
        (Stage::AnchorCommitted, Some(&proposed), [1; 32]),
        (Stage::Published, Some(&proposed), [0; 32]),
        (Stage::Aborted, Some(&proposed), [0; 32]),
        (Stage::Aborted, Some(&prior), [1; 32]),
    ] {
        let wire = f.wire(stage, receipt, worker);
        assert!(matches!(
            probe(N, WORK_LIMIT, STORAGE_LIMIT, |b| Journal::decode(&wire, b)).result,
            Err(Error::Frame(FrameError::StagePayloadMismatch))
        ));
    }
}

#[test]
fn framing_and_resealed_signature_corruption_fail_closed() {
    let f = Fixture::genesis();
    let receipt = f.receipt(AnchorPositionV1::Proposed);
    for (stage, receipt, worker) in [
        (Stage::PreparedAnchor, None, [0; 32]),
        (Stage::AnchorCommitted, Some(&receipt), [0; 32]),
        (Stage::Published, Some(&receipt), [1; 32]),
    ] {
        let wire = f.wire(stage, receipt, worker);
        for offset in [
            0,
            8,
            10,
            12,
            20,
            24,
            25,
            32,
            32 + T,
            32 + T + C,
            N - 64,
            N - 1,
        ] {
            let mut bad = wire;
            bad[offset] ^= 0x80;
            assert!(
                probe(N, WORK_LIMIT, STORAGE_LIMIT, |b| Journal::decode(&bad, b))
                    .result
                    .is_err()
            );
        }
        for length in [0, 31, N - 1, N + 1] {
            let mut bad = wire.to_vec();
            bad.resize(length, 0);
            assert!(matches!(
                probe(length.min(N), WORK_LIMIT, STORAGE_LIMIT, |b| {
                    Journal::decode(&bad, b)
                })
                .result,
                Err(Error::Frame(FrameError::InvalidLength { .. }))
            ));
        }
    }
    let mut bad = f.wire(Stage::AnchorCommitted, Some(&receipt), [0; 32]);
    // Preserve both enclosing hashes so rejection must reach signature verification.
    bad[32 + T + C + R - 33] ^= 1;
    let receipt_start = 32 + T + C;
    let identity_start = receipt_start + R - 32;
    let mut hash = Sha256::new();
    hash.update(b"FE2O3/EXTERNAL-MONOTONIC-ANCHOR/TRANSITION-RECEIPT/V1\0");
    hash.update(&bad[receipt_start..identity_start]);
    bad[identity_start..identity_start + 32].copy_from_slice(&hash.finalize());
    seal_record(&mut bad, "WORKER-ANCHOR-JOURNAL", SCHEMA.version);
    assert!(matches!(
        probe(N, WORK_LIMIT, STORAGE_LIMIT, |b| Journal::decode(&bad, b)).result,
        Err(Error::Frame(FrameError::Anchor(
            AnchorProtocolErrorV1::SignatureRejected
        )))
    ));
}
