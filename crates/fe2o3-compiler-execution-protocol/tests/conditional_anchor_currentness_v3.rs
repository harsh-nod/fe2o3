//! Component transcripts only: local keys do not represent protected issuer/anchor custody.
use ed25519_dalek::{Signer, SigningKey};
use fe2o3_compiler_execution_protocol::*;
use fe2o3_external_anchor_protocol::{
    AnchorChallengeV1, AnchorPositionV1, AnchorTransitionReceiptV1, AnchoredStateV1, CallerNonceV1,
    HashChainHeadV1, PinnedAnchorKeyV1, UnsignedAnchorObservationV1, derive_transaction_digest_v1,
};
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
    CanonicalKernelIrWorkBudgetV1 as Work,
};
use sha2::{Digest, Sha256};
use std::mem::size_of;

#[allow(dead_code)]
#[path = "support/native_attestation_fixture.rs"]
mod fixture;
#[allow(dead_code)]
#[path = "support/native_publication_fixture.rs"]
mod publication_fixture;
#[allow(dead_code)]
#[path = "support/native_receipt_fixture.rs"]
mod receipt_fixture;
use fixture::{header, policy_wire, request_wire, seal};
use publication_fixture::{carriage_wire, publication_wire};

type Policy = CompilerExecutionIssuerPolicyV3;
type Request = CompilerExecutionAttestationRequestV3;
type Publication = CompilerExecutionReceiptPublicationV3;
type Carriage = CompilerExecutionReceiptCarriageV3;
type Transaction = CompilerExecutionExternalAnchorTransactionV3;
type Verification = CompilerExecutionCurrentRecordVerificationV3;
type Attestation = CompilerExecutionCurrentRecordAttestationV3;
type Error = CompilerExecutionNativeJournalErrorV3;
type CurrentError = CompilerExecutionCurrentRecordVerificationErrorV3;
type Storage = CompilerExecutionAttestationStorageV3;
const WORK: usize = 100_000_000;
const STORAGE: usize = 4_000_000;
const CHALLENGE: [u8; 32] = [0xa1; 32];
const VS: usize = size_of::<(Verification, Storage)>();
const AS: usize = size_of::<(Attestation, Storage)>();

// This test scaffold isolates component calls; it does not manufacture production accounts.
fn run<T>(floor: usize, f: impl FnOnce(&mut Budget<'_>) -> Result<T, Error>) -> Result<T, Error> {
    probe(floor, WORK, STORAGE, f).0
}
fn probe<T>(
    floor: usize,
    work: usize,
    storage: usize,
    f: impl FnOnce(&mut Budget<'_>) -> Result<T, Error>,
) -> (Result<T, Error>, usize, usize) {
    let mut w = Work::new(work);
    let mut b = Budget::new(&mut w, storage);
    b.reserve_storage(floor).unwrap();
    let ledger = b.work_ledger_identity_v1();
    let result = f(&mut b);
    assert_eq!(b.storage(), floor);
    assert!(b.work_ledger_identity_v1() == ledger);
    (result, b.work(), b.peak_storage())
}
fn transaction_wire(version: u16) -> [u8; 1874] {
    let mut out = [0; 1874];
    header(
        &mut out,
        if version == 2 {
            b"F2O3CAT2"
        } else {
            b"F2O3CAT3"
        },
        version,
    );
    out[24..240].copy_from_slice(&policy_wire(version));
    out[240..1186].copy_from_slice(&request_wire(version));
    out[1186..1770].copy_from_slice(&publication_wire(version));
    out[1770..1778].copy_from_slice(&1u64.to_le_bytes());
    out[1810..1842].copy_from_slice(&receipt_fixture::receipt_wire(version)[240..272]);
    seal_transaction(&mut out, version);
    out
}
fn seal_transaction(bytes: &mut [u8; 1874], version: u16) {
    // The anchor transaction identity predates the length-prefixed leaf codecs.
    let mut hash = Sha256::new();
    hash.update(format!(
        "FE2O3/COMPILER-EXECUTION-EXTERNAL-ANCHOR-TRANSACTION/V{version}\0"
    ));
    hash.update(&bytes[..1842]);
    bytes[1842..].copy_from_slice(&hash.finalize());
}
fn carriage() -> Carriage {
    run(2090, |b| Ok(Carriage::decode(&carriage_wire(3), b)?))
        .unwrap()
        .0
}
fn observe(
    challenge: AnchorChallengeV1,
    position: AnchorPositionV1,
    seed: u8,
) -> AnchorTransitionReceiptV1 {
    let key = SigningKey::from_bytes(&[seed; 32]);
    let unsigned = UnsignedAnchorObservationV1::from_challenge(&challenge, position);
    let signature = key.sign(&unsigned.signing_bytes()).to_bytes();
    AnchorTransitionReceiptV1::new(
        challenge,
        &unsigned.attach_signature(signature),
        &PinnedAnchorKeyV1::from_bytes(key.verifying_key().to_bytes()).unwrap(),
    )
    .unwrap()
}
fn anchors(c: &Carriage) -> (AnchorTransitionReceiptV1, AnchorTransitionReceiptV1) {
    let bytes = transaction_wire(3);
    let key = PinnedAnchorKeyV1::from_bytes(*c.policy().external_anchor_verifying_key()).unwrap();
    let pending = AnchoredStateV1::from_local_state(0, HashChainHeadV1::from_bytes([0; 32]))
        .prepare(derive_transaction_digest_v1(&bytes).unwrap(), &key)
        .unwrap()
        .begin_advance(CallerNonceV1::from_bytes([0x81; 32]), &key)
        .unwrap();
    let commit = observe(
        pending.challenge().clone(),
        AnchorPositionV1::Proposed,
        0x52,
    );
    let (challenge, _) = run(
        c.retained_storage() + size_of::<AnchorTransitionReceiptV1>(),
        |b| Verification::external_anchor_currentness_challenge_native_v3(c, &commit, CHALLENGE, b),
    )
    .unwrap();
    let current = observe(challenge, AnchorPositionV1::Proposed, 0x52);
    (commit, current)
}
fn verification(c: &Carriage) -> Verification {
    let (commit, current) = anchors(c);
    run(
        c.retained_storage() + 2 * size_of::<AnchorTransitionReceiptV1>(),
        |b| Verification::new_native_v3(c, commit, current, CHALLENGE, [0x91; 32], [0x92; 32], b),
    )
    .unwrap()
    .0
}
fn issue(c: &Carriage) -> Attestation {
    let v = verification(c);
    let key = SigningKey::from_bytes(&[0x51; 32]);
    run(
        c.policy().retained_storage() + c.retained_storage() + VS + size_of::<SigningKey>(),
        |b| Attestation::issue_native_v3(c.policy(), c, v, CHALLENGE, &key, b),
    )
    .unwrap()
    .0
}

#[test]
fn conditional_anchor_v3_independent_wire_and_v2_parity() {
    assert_eq!(
        COMPILER_EXECUTION_EXTERNAL_ANCHOR_TRANSACTION_BYTES_V3,
        1874
    );
    assert_eq!(
        Transaction::WORK,
        CompilerExecutionExternalAnchorTransactionV2::WORK
    );
    assert_eq!(
        Transaction::FRAME_STORAGE,
        CompilerExecutionExternalAnchorTransactionV2::FRAME_STORAGE
    );
    assert_eq!(
        Transaction::RETAINED_STORAGE,
        CompilerExecutionExternalAnchorTransactionV2::RETAINED_STORAGE
    );
    let bytes = transaction_wire(3);
    let (t, s) = run(bytes.len(), |b| Transaction::decode(&bytes, b)).unwrap();
    assert_eq!(t.canonical_bytes(), &bytes);
    assert_eq!(s.additional_storage(), t.retained_storage());
    assert_eq!(t.identity().as_bytes(), &bytes[1842..]);
    assert_eq!(t.sequence(), 1);
    assert_eq!(t.prior_rollback_anchor(), [0; 32]);
    assert_eq!(
        t.current_rollback_anchor(),
        receipt_fixture::receipt_wire(3)[240..272]
    );
    assert_eq!(
        run(t.retained_storage(), |b| t.external_anchor_digest(b)).unwrap(),
        derive_transaction_digest_v1(&bytes).unwrap()
    );
    let old = transaction_wire(2);
    let mut w = Work::new(WORK);
    let mut b = Budget::new(&mut w, STORAGE);
    b.reserve_storage(old.len()).unwrap();
    let (v2, _) = CompilerExecutionExternalAnchorTransactionV2::decode(&old, &mut b).unwrap();
    assert_eq!(v2.canonical_bytes(), &old);
    assert_ne!(v2.identity().as_bytes(), t.identity().as_bytes());
    assert!(Transaction::decode(&old, &mut b).is_err());
    assert!(CompilerExecutionExternalAnchorTransactionV2::decode(&bytes, &mut b).is_err());
}

#[test]
fn conditional_anchor_v3_consumes_actual_children_on_one_ledger() {
    let (p, q, u) = (policy_wire(3), request_wire(3), publication_wire(3));
    let input = p.len() + q.len() + u.len();
    let mut w = Work::new(WORK);
    let mut b = Budget::new(&mut w, STORAGE);
    b.reserve_storage(input).unwrap();
    let ledger = b.work_ledger_identity_v1();
    let (p, s) = Policy::decode(&p, &mut b).unwrap();
    b.reserve_storage(s.additional_storage()).unwrap();
    let (q, s) = Request::decode(&q, &mut b).unwrap();
    b.reserve_storage(s.additional_storage()).unwrap();
    let (u, s) = Publication::decode(&u, &mut b).unwrap();
    b.reserve_storage(s.additional_storage()).unwrap();
    let children = p.retained_storage() + q.retained_storage() + u.retained_storage();
    let floor = b.storage();
    let (t, s) = Transaction::new(p, q, u, &mut b).unwrap();
    assert_eq!(b.storage(), floor);
    assert_eq!(children + s.additional_storage(), t.retained_storage());
    b.reserve_storage(s.additional_storage()).unwrap();
    assert_eq!(t.canonical_bytes(), &transaction_wire(3));
    assert!(b.work_ledger_identity_v1() == ledger);
    let retained = t.retained_storage();
    drop(t);
    b.release_storage(retained).unwrap();
    assert_eq!(b.storage(), input);
}

#[test]
fn conditional_anchor_v3_rejects_framing_nested_families_and_resealed_joins() {
    let original = transaction_wire(3);
    for offset in [
        0, 8, 10, 12, 20, 24, 240, 264, 464, 1186, 1282, 1586, 1770, 1778, 1810, 1842, 1873,
    ] {
        let mut bytes = original;
        bytes[offset] ^= 1;
        assert!(
            run(bytes.len(), |b| Transaction::decode(&bytes, b)).is_err(),
            "offset {offset}"
        );
        seal_transaction(&mut bytes, 3);
        if offset < 1842 {
            assert!(
                run(bytes.len(), |b| Transaction::decode(&bytes, b)).is_err(),
                "resealed {offset}"
            );
        }
    }
    for range in [24..240, 240..1186, 1186..1770] {
        let mut bytes = original;
        bytes[range.clone()].copy_from_slice(&transaction_wire(2)[range]);
        seal_transaction(&mut bytes, 3);
        assert!(run(bytes.len(), |b| Transaction::decode(&bytes, b)).is_err());
    }
    for length in [0, 23, 1873, 1875] {
        let mut bytes = original.to_vec();
        bytes.resize(length, 0);
        assert!(run(length, |b| Transaction::decode(&bytes, b)).is_err());
    }
}

#[test]
fn conditional_anchor_v3_denials_preserve_original_floor_and_work() {
    let bytes = transaction_wire(3);
    for floor in [0, bytes.len() - 1] {
        let (r, work, peak) = probe(floor, WORK, STORAGE, |b| Transaction::decode(&bytes, b));
        assert!(matches!(r, Err(Error::Resource(Resource::Accounting))));
        assert_eq!(work, 8);
        assert_eq!(peak, floor);
    }
    for work in [0, 7, Transaction::WORK - 1] {
        let (r, charged, peak) = probe(bytes.len(), work, STORAGE, |b| {
            Transaction::decode(&bytes, b)
        });
        assert!(matches!(r, Err(Error::Resource(_))));
        assert!(charged <= 8);
        assert_eq!(peak, bytes.len());
    }
    let floor = bytes.len();
    let (r, _, peak) = probe(floor, WORK, floor + Transaction::FRAME_STORAGE - 1, |b| {
        Transaction::decode(&bytes, b)
    });
    assert!(matches!(r, Err(Error::Resource(_))));
    assert_eq!(peak, floor);
    assert!(run(floor, |b| Transaction::decode(&bytes, b)).is_ok());
}

#[test]
fn conditional_currentness_v3_challenge_bound_complete_join_stays_inert() {
    let c = carriage();
    let (commit, current) = anchors(&c);
    let key = SigningKey::from_bytes(&[0x51; 32]);
    let borrowed = c.retained_storage() + c.policy().retained_storage() + size_of::<SigningKey>();
    let anchors = 2 * size_of::<AnchorTransitionReceiptV1>();
    let mut w = Work::new(WORK);
    let mut b = Budget::new(&mut w, STORAGE);
    b.reserve_storage(borrowed + anchors).unwrap();
    let ledger = b.work_ledger_identity_v1();
    let (v, s) = Verification::new_native_v3(
        &c, commit, current, CHALLENGE, [0x91; 32], [0x92; 32], &mut b,
    )
    .unwrap();
    assert_eq!(s.additional_storage(), VS);
    b.reserve_storage(s.additional_storage()).unwrap();
    b.release_storage(anchors).unwrap();
    assert_eq!(
        v.subject_identity(),
        *c.request().subject().identity().sha256()
    );
    assert_eq!(v.carriage_identity(), *c.identity().as_bytes());
    assert!(!v.grants_authority());
    let (a, s) = Attestation::issue_native_v3(c.policy(), &c, v, CHALLENGE, &key, &mut b).unwrap();
    assert_eq!(VS + s.additional_storage(), AS);
    b.reserve_storage(s.additional_storage()).unwrap();
    assert!(!a.grants_authority());
    let wire = a.canonical_bytes().to_vec();
    assert_eq!(Attestation::decode(&wire).unwrap(), a);
    let (verified, s) = a
        .verify_native_v3(c.policy(), &c, CHALLENGE, &mut b)
        .unwrap();
    assert_eq!(s.additional_storage(), 0);
    assert!(verified.authenticates_pinned_signing_key());
    assert!(verified.authenticates_expected_challenge());
    assert!(verified.authenticates_external_anchor_commit());
    assert!(verified.authenticates_external_rollback_currentness());
    assert!(!verified.authenticates_protected_current_record());
    assert!(!verified.grants_authority());
    assert!(b.work_ledger_identity_v1() == ledger);
    drop(verified);
    b.release_storage(AS).unwrap();
    assert_eq!(b.storage(), borrowed);
}

#[test]
fn conditional_currentness_v3_rejects_policy_challenge_and_cross_family_substitution() {
    let c = carriage();
    let a = issue(&c);
    let floor = c.retained_storage() + c.policy().retained_storage() + AS;
    assert!(matches!(
        run(floor, |b| a.clone().verify_native_v3(
            c.policy(),
            &c,
            [1; 32],
            b
        )),
        Err(Error::Current(CurrentError::ChallengeMismatch))
    ));
    let mut changed = policy_wire(3);
    changed[24] ^= 1;
    seal(&mut changed, "COMPILER-EXECUTION-ISSUER-POLICY", 3);
    let (other, _) = run(changed.len(), |b| Ok(Policy::decode(&changed, b)?)).unwrap();
    assert!(matches!(
        run(floor, |b| a
            .clone()
            .verify_native_v3(&other, &c, CHALLENGE, b)),
        Err(Error::Current(CurrentError::PolicyMismatch))
    ));
    let mut w = Work::new(WORK);
    let mut b = Budget::new(&mut w, STORAGE);
    let v2bytes = carriage_wire(2);
    b.reserve_storage(v2bytes.len()).unwrap();
    let (v2, s) = CompilerExecutionReceiptCarriageV2::decode(&v2bytes, &mut b).unwrap();
    b.reserve_storage(s.additional_storage() + v2.policy().retained_storage() + AS)
        .unwrap();
    assert!(
        a.clone()
            .verify_native(v2.policy(), &v2, CHALLENGE, &mut b)
            .is_err()
    );
    assert!(run(floor, |b| a.verify_native_v3(c.policy(), &c, CHALLENGE, b)).is_ok());
}

#[test]
fn conditional_currentness_v3_rejects_wrong_anchor_key_position_and_stale_observation() {
    let c = carriage();
    let (commit, current) = anchors(&c);
    let floor = c.retained_storage() + 2 * size_of::<AnchorTransitionReceiptV1>();
    let other_key = PinnedAnchorKeyV1::from_bytes(
        SigningKey::from_bytes(&[0x53; 32])
            .verifying_key()
            .to_bytes(),
    )
    .unwrap();
    let other_pending = AnchoredStateV1::from_local_state(0, HashChainHeadV1::from_bytes([0; 32]))
        .prepare(commit.challenge().transaction(), &other_key)
        .unwrap()
        .begin_advance(CallerNonceV1::from_bytes([0x81; 32]), &other_key)
        .unwrap();
    let bad_key = observe(
        other_pending.challenge().clone(),
        AnchorPositionV1::Proposed,
        0x53,
    );
    let bad_position = observe(commit.challenge().clone(), AnchorPositionV1::Prior, 0x52);
    let stale = observe(current.challenge().clone(), AnchorPositionV1::Prior, 0x52);
    for (commit, current, challenge) in [
        (bad_key, current.clone(), CHALLENGE),
        (bad_position, current.clone(), CHALLENGE),
        (commit.clone(), stale, CHALLENGE),
        (commit.clone(), current.clone(), [1; 32]),
    ] {
        assert!(
            run(floor, |b| Verification::new_native_v3(
                &c, commit, current, challenge, [0x91; 32], [0x92; 32], b
            ))
            .is_err()
        );
    }
    let key = SigningKey::from_bytes(&[0x54; 32]);
    assert!(matches!(
        run(
            c.retained_storage() + c.policy().retained_storage() + VS + size_of::<SigningKey>(),
            |b| Attestation::issue_native_v3(c.policy(), &c, verification(&c), CHALLENGE, &key, b)
        ),
        Err(Error::Current(CurrentError::SigningKeyMismatch))
    ));
}

#[test]
fn conditional_currentness_v3_underpaid_inputs_fail_before_scratch_or_crypto() {
    let c = carriage();
    let (commit, current) = anchors(&c);
    let v = verification(&c);
    let a = issue(&c);
    let key = SigningKey::from_bytes(&[0x51; 32]);
    let floors = [
        c.retained_storage() + 2 * size_of::<AnchorTransitionReceiptV1>(),
        c.retained_storage() + size_of::<AnchorTransitionReceiptV1>(),
        c.retained_storage() + c.policy().retained_storage() + VS + size_of::<SigningKey>(),
        c.retained_storage() + c.policy().retained_storage() + AS,
    ];
    for (operation, required) in floors.into_iter().enumerate() {
        for floor in [0, required - 1] {
            let (r, work, peak) = probe(floor, WORK, STORAGE, |b| match operation {
                0 => Verification::new_native_v3(
                    &c,
                    commit.clone(),
                    current.clone(),
                    CHALLENGE,
                    [0x91; 32],
                    [0x92; 32],
                    b,
                )
                .map(|_| ()),
                1 => Verification::external_anchor_currentness_challenge_native_v3(
                    &c, &commit, CHALLENGE, b,
                )
                .map(|_| ()),
                2 => Attestation::issue_native_v3(c.policy(), &c, v.clone(), CHALLENGE, &key, b)
                    .map(|_| ()),
                _ => a
                    .clone()
                    .verify_native_v3(c.policy(), &c, CHALLENGE, b)
                    .map(|_| ()),
            });
            assert!(matches!(r, Err(Error::Resource(Resource::Accounting))));
            assert_eq!(work, 8);
            assert_eq!(peak, floor);
        }
    }
}

#[test]
fn conditional_currentness_v3_resealed_record_coordinates_cannot_replace_expected_carriage() {
    let c = carriage();
    let v = verification(&c);
    let key = SigningKey::from_bytes(&[0x51; 32]);
    let floor = c.retained_storage() + c.policy().retained_storage() + VS + size_of::<SigningKey>();
    // Canonical and signed anchors remain valid; the caller's exact carriage is different.
    for offset in [24, 56, 88, 120, 152, 224] {
        let mut wire = *v.canonical_bytes();
        wire[offset] ^= 1;
        seal(
            &mut wire,
            "COMPILER-EXECUTION-CURRENT-RECORD-VERIFICATION",
            3,
        );
        let changed = Verification::decode(&wire).unwrap();
        assert!(
            matches!(
                run(floor, |b| Attestation::issue_native_v3(
                    c.policy(),
                    &c,
                    changed,
                    CHALLENGE,
                    &key,
                    b
                )),
                Err(Error::Current(CurrentError::VerificationMismatch))
            ),
            "coordinate {offset}"
        );
    }
}

#[test]
fn conditional_currentness_v3_exact_and_one_short_nested_work_and_storage() {
    let c = carriage();
    let (commit, current) = anchors(&c);
    let v = verification(&c);
    let a = issue(&c);
    let key = SigningKey::from_bytes(&[0x51; 32]);
    let floors = [
        c.retained_storage() + 2 * size_of::<AnchorTransitionReceiptV1>(),
        c.retained_storage() + size_of::<AnchorTransitionReceiptV1>(),
        c.retained_storage() + c.policy().retained_storage() + VS + size_of::<SigningKey>(),
        c.retained_storage() + c.policy().retained_storage() + AS,
    ];
    for (operation, floor) in floors.into_iter().enumerate() {
        let invoke = |b: &mut Budget<'_>| -> Result<(), Error> {
            match operation {
                0 => Verification::new_native_v3(
                    &c,
                    commit.clone(),
                    current.clone(),
                    CHALLENGE,
                    [0x91; 32],
                    [0x92; 32],
                    b,
                )
                .map(|_| ()),
                1 => Verification::external_anchor_currentness_challenge_native_v3(
                    &c, &commit, CHALLENGE, b,
                )
                .map(|_| ()),
                2 => Attestation::issue_native_v3(c.policy(), &c, v.clone(), CHALLENGE, &key, b)
                    .map(|_| ()),
                _ => a
                    .clone()
                    .verify_native_v3(c.policy(), &c, CHALLENGE, b)
                    .map(|_| ()),
            }
        };
        let (result, work, peak) = probe(floor, WORK, STORAGE, invoke);
        result.unwrap();
        assert!(probe(floor, work, peak, invoke).0.is_ok());
        for (work_limit, storage_limit) in [(work - 1, peak), (work, peak - 1)] {
            let (result, charged, observed_peak) = probe(floor, work_limit, storage_limit, invoke);
            assert!(matches!(
                result,
                Err(Error::Resource(_))
                    | Err(Error::Attestation(
                        CompilerExecutionAttestationErrorV3::Resource(_)
                    ))
            ));
            assert!(charged <= work_limit);
            assert!(observed_peak <= storage_limit);
        }
    }
}
