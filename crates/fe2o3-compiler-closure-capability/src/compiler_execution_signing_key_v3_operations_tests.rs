//! Public key operations on component fixtures, without protected deployment credit.
use super::*;
use fe2o3_compiler_execution_protocol::{
    CompilerExecutionCurrentRecordVerificationErrorV3 as CurrentError,
    CompilerExecutionExternalAnchorTransactionV3 as Transaction,
    CompilerExecutionNativeJournalErrorV3 as JournalError,
    CompilerExecutionReceiptPublicationV3 as Publication,
};
use fe2o3_external_anchor_protocol::{
    AnchorChallengeV1, AnchorPositionV1, AnchorTransitionReceiptV1, AnchoredStateV1, CallerNonceV1,
    HashChainHeadV1, PinnedAnchorKeyV1, UnsignedAnchorObservationV1,
};
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;

#[allow(dead_code)]
#[path = "../../fe2o3-compiler-execution-protocol/tests/support/native_attestation_fixture.rs"]
mod fixture;
#[allow(dead_code)]
#[path = "../../fe2o3-compiler-execution-protocol/tests/support/native_publication_fixture.rs"]
mod publication_fixture;
#[allow(dead_code)]
#[path = "../../fe2o3-compiler-execution-protocol/tests/support/native_receipt_fixture.rs"]
mod receipt_fixture;

type Cap = CompilerExecutionSigningKeyCapabilityV3;
const CHALLENGE: [u8; 32] = [0xa1; 32];
const VS: usize = size_of::<(CurrentVerification, ProtocolStorage)>();
const AS: usize = size_of::<(CurrentAttestation, ProtocolStorage)>();
const WORK: usize = 100_000_000;
const STORAGE: usize = 4_000_000;

fn policy(generation: u64, b: &mut Budget<'_>) -> Policy {
    b.reserve_storage(216).unwrap();
    let (policy, charge) = {
        let mut bytes = fixture::policy_wire(3);
        bytes[24..32].copy_from_slice(&generation.to_le_bytes());
        fixture::seal(&mut bytes, "COMPILER-EXECUTION-ISSUER-POLICY", 3);
        Policy::decode(&bytes, b).unwrap()
    };
    b.reserve_storage(charge.additional_storage()).unwrap();
    b.release_storage(216).unwrap();
    policy
}

fn request(b: &mut Budget<'_>) -> Request {
    b.reserve_storage(fixture::REQUEST_BYTES).unwrap();
    let (request, charge) = {
        let bytes = fixture::request_wire(3);
        Request::decode(&bytes, b).unwrap()
    };
    b.reserve_storage(charge.additional_storage()).unwrap();
    b.release_storage(fixture::REQUEST_BYTES).unwrap();
    request
}

fn carriage(b: &mut Budget<'_>) -> Carriage {
    b.reserve_storage(2090).unwrap();
    let (carriage, charge) = {
        let bytes = publication_fixture::carriage_wire(3);
        Carriage::decode(&bytes, b).unwrap()
    };
    b.reserve_storage(charge.additional_storage()).unwrap();
    b.release_storage(2090).unwrap();
    carriage
}

fn key(policy: &Policy, b: &mut Budget<'_>) -> Cap {
    b.reserve_storage(KEY_BYTES).unwrap();
    let (cap, charge) = {
        let mut seed = [0x51; KEY_BYTES];
        let result = Cap::create_and_zeroize(&mut seed, policy, b);
        assert_eq!(seed, [0; KEY_BYTES]);
        result.unwrap()
    };
    b.reserve_storage(charge.additional_storage()).unwrap();
    b.release_storage(KEY_BYTES).unwrap();
    cap
}

fn observe(
    challenge: AnchorChallengeV1,
    key: &SigningKey,
    pin: &PinnedAnchorKeyV1,
) -> AnchorTransitionReceiptV1 {
    let unsigned =
        UnsignedAnchorObservationV1::from_challenge(&challenge, AnchorPositionV1::Proposed);
    let signature = key.sign(&unsigned.signing_bytes()).to_bytes();
    AnchorTransitionReceiptV1::new(challenge, &unsigned.attach_signature(signature), pin).unwrap()
}

fn verification(carriage: &Carriage, b: &mut Budget<'_>) -> CurrentVerification {
    let floor = b.storage();
    let verification = {
        let (policy, charge) = Policy::decode(carriage.policy().canonical_bytes(), b).unwrap();
        b.reserve_storage(charge.additional_storage()).unwrap();
        let (request, charge) = Request::decode(carriage.request().canonical_bytes(), b).unwrap();
        b.reserve_storage(charge.additional_storage()).unwrap();
        let (publication, charge) =
            Publication::decode(carriage.publication().canonical_bytes(), b).unwrap();
        b.reserve_storage(charge.additional_storage()).unwrap();
        let (transaction, charge) = Transaction::new(policy, request, publication, b).unwrap();
        b.reserve_storage(charge.additional_storage()).unwrap();
        // Test-only V1 fixture scratch covers anchor keys, challenges and signature buffers.
        // Every metered V3 operation below still charges the caller's original ledger.
        b.reserve_storage(16 * 1024).unwrap();
        let digest = transaction.external_anchor_digest(b).unwrap();
        let key = SigningKey::from_bytes(&[0x52; 32]);
        let pin = PinnedAnchorKeyV1::from_bytes(*carriage.policy().external_anchor_verifying_key())
            .unwrap();
        let pending = AnchoredStateV1::from_local_state(0, HashChainHeadV1::from_bytes([0; 32]))
            .prepare(digest, &pin)
            .unwrap()
            .begin_advance(CallerNonceV1::from_bytes([0x81; 32]), &pin)
            .unwrap();
        let commit = observe(pending.challenge().clone(), &key, &pin);
        b.reserve_storage(size_of::<AnchorTransitionReceiptV1>())
            .unwrap();
        let (challenge, charge) =
            CurrentVerification::external_anchor_currentness_challenge_native_v3(
                carriage, &commit, CHALLENGE, b,
            )
            .unwrap();
        b.reserve_storage(charge.additional_storage()).unwrap();
        let current = observe(challenge, &key, &pin);
        b.reserve_storage(size_of::<AnchorTransitionReceiptV1>())
            .unwrap();
        let (verification, charge) = CurrentVerification::new_native_v3(
            carriage, commit, current, CHALLENGE, [0x91; 32], [0x92; 32], b,
        )
        .unwrap();
        assert_eq!(charge.additional_storage(), VS);
        b.reserve_storage(charge.additional_storage()).unwrap();
        verification
    };
    // Only the verification survives: retire actual dropped setup owners and scratch.
    b.release_storage(b.storage() - floor - VS).unwrap();
    verification
}

#[test]
fn public_issue_receipt_matches_independent_v3_wire_and_verifies_on_original_ledger() {
    let mut work = Work::new(WORK);
    let mut b = Budget::new(&mut work, STORAGE);
    let ledger = b.work_ledger_identity_v1();
    let policy = policy(7, &mut b);
    let request = request(&mut b);
    let cap = key(&policy, &mut b);
    let floor = cap.retained_storage() + policy.retained_storage() + request.retained_storage();
    assert_eq!(b.storage(), floor);
    let before = b.work();
    let (receipt, charge) = cap.issue_receipt(&policy, &request, &mut b).unwrap();
    assert_eq!(b.storage(), floor);
    assert_eq!(b.work() - before, 2 * Cap::IO_WORK
        + fe2o3_compiler_execution_protocol::COMPILER_EXECUTION_ATTESTATION_RECEIPT_ISSUE_WORK_V3);
    assert_eq!(charge.additional_storage(), receipt.retained_storage());
    b.reserve_storage(charge.additional_storage()).unwrap();
    b.reserve_storage(400).unwrap();
    {
        let expected = receipt_fixture::receipt_wire(3);
        assert_eq!(receipt.canonical_bytes(), &expected);
    }
    b.release_storage(400).unwrap();
    let (verified, charge) = receipt.verify(&policy, &request, [0; 32], &mut b).unwrap();
    b.reserve_storage(charge.additional_storage()).unwrap();
    assert!(verified.authenticates_pinned_signing_key());
    assert!(!verified.authenticates_protected_compiler_execution());
    assert!(!verified.grants_compiler_authority());
    assert!(b.work_ledger_identity_v1() == ledger);
    let retained = verified.retained_storage();
    drop((verified, cap, policy, request));
    b.release_storage(floor + retained).unwrap();
    assert_eq!(b.storage(), 0);
}

#[test]
fn public_attest_current_joins_signed_anchors_and_verifies_on_original_ledger() {
    let mut work = Work::new(WORK);
    let mut b = Budget::new(&mut work, STORAGE);
    let ledger = b.work_ledger_identity_v1();
    let policy = policy(7, &mut b);
    let carriage = carriage(&mut b);
    let cap = key(&policy, &mut b);
    let retained = cap.retained_storage() + policy.retained_storage() + carriage.retained_storage();
    let v = verification(&carriage, &mut b);
    assert_eq!(b.storage(), retained + VS);
    let before = b.work();
    let (attestation, charge) = cap
        .attest_current(&policy, &carriage, v, CHALLENGE, &mut b)
        .unwrap();
    assert_eq!(b.storage(), retained + VS);
    assert!(b.work() > before + 2 * Cap::IO_WORK);
    assert_eq!(VS + charge.additional_storage(), AS);
    b.reserve_storage(charge.additional_storage()).unwrap();
    let (verified, charge) = attestation
        .verify_native_v3(&policy, &carriage, CHALLENGE, &mut b)
        .unwrap();
    assert_eq!(charge.additional_storage(), 0);
    assert!(verified.authenticates_pinned_signing_key());
    assert!(verified.authenticates_expected_challenge());
    assert!(verified.authenticates_external_anchor_commit());
    assert!(verified.authenticates_external_rollback_currentness());
    assert!(!verified.authenticates_protected_current_record());
    assert!(!verified.grants_authority());
    assert!(b.work_ledger_identity_v1() == ledger);
    drop((verified, cap, policy, carriage));
    b.release_storage(retained + AS).unwrap();
    assert_eq!(b.storage(), 0);
}

#[test]
fn public_issue_receipt_rejects_underpaid_inputs_and_wrong_generation() {
    let mut work = Work::new(WORK);
    let mut b = Budget::new(&mut work, STORAGE);
    let ledger = b.work_ledger_identity_v1();
    let policy = policy(7, &mut b);
    let request = request(&mut b);
    let cap = key(&policy, &mut b);
    let floor = b.storage();
    assert_eq!(
        floor,
        cap.retained_storage() + policy.retained_storage() + request.retained_storage()
    );
    for missing in [1, floor] {
        b.release_storage(missing).unwrap(); // Deliberate caller underpayment.
        let before = (b.work(), b.peak_storage());
        let error = cap.issue_receipt(&policy, &request, &mut b).unwrap_err();
        assert!(matches!(error, Error::Resource(Resource::Accounting)));
        assert_eq!(b.storage(), floor - missing);
        assert_eq!(
            (b.work(), b.peak_storage()),
            (before.0 + ENTRY_WORK, before.1)
        );
        b.reserve_storage(missing).unwrap();
    }
    let other = self::policy(8, &mut b);
    let before = (b.storage(), b.work());
    let error = cap.issue_receipt(&other, &request, &mut b).unwrap_err();
    assert!(matches!(
        error,
        Error::Rejected("signing key is pinned to another native policy")
    ));
    assert_eq!(
        (b.storage(), b.work()),
        (before.0, before.1 + 2 * Cap::IO_WORK)
    );
    assert!(b.work_ledger_identity_v1() == ledger);
    let retained = floor + other.retained_storage();
    drop((cap, policy, other, request));
    b.release_storage(retained).unwrap();
    assert_eq!(b.storage(), 0);
}

#[test]
fn public_attest_current_rejects_underpayment_generation_and_stale_challenge() {
    let mut work = Work::new(WORK);
    let mut b = Budget::new(&mut work, STORAGE);
    let ledger = b.work_ledger_identity_v1();
    let policy = policy(7, &mut b);
    let carriage = carriage(&mut b);
    let cap = key(&policy, &mut b);
    let retained = b.storage();
    assert_eq!(
        retained,
        cap.retained_storage() + policy.retained_storage() + carriage.retained_storage()
    );
    for empty in [false, true] {
        let v = verification(&carriage, &mut b);
        let floor = retained + VS;
        assert_eq!(b.storage(), floor);
        let missing = if empty { floor } else { 1 };
        b.release_storage(missing).unwrap(); // Deliberate caller underpayment.
        let before = (b.work(), b.peak_storage());
        let error = cap
            .attest_current(&policy, &carriage, v, CHALLENGE, &mut b)
            .unwrap_err();
        assert!(matches!(error, Error::Resource(Resource::Accounting)));
        assert_eq!(b.storage(), floor - missing);
        assert_eq!(
            (b.work(), b.peak_storage()),
            (before.0 + ENTRY_WORK, before.1)
        );
        b.reserve_storage(missing).unwrap();
        b.release_storage(VS).unwrap(); // The failed consuming operation dropped v.
    }
    let other = self::policy(8, &mut b);
    let v = verification(&carriage, &mut b);
    let before = (b.storage(), b.work());
    let error = cap
        .attest_current(&other, &carriage, v, CHALLENGE, &mut b)
        .unwrap_err();
    assert!(matches!(
        error,
        Error::Rejected("signing key is pinned to another native policy")
    ));
    assert_eq!(
        (b.storage(), b.work()),
        (before.0, before.1 + 2 * Cap::IO_WORK)
    );
    b.release_storage(VS).unwrap();
    let v = verification(&carriage, &mut b);
    let before = (b.storage(), b.work());
    let error = cap
        .attest_current(&policy, &carriage, v, [1; 32], &mut b)
        .unwrap_err();
    assert!(matches!(
        error,
        Error::JournalV3(JournalError::Current(
            CurrentError::ExternalAnchorCurrentnessReceiptMismatch
        ))
    ));
    assert_eq!(b.storage(), before.0);
    assert!(b.work() > before.1 + 2 * Cap::IO_WORK);
    b.release_storage(VS).unwrap();
    assert!(b.work_ledger_identity_v1() == ledger);
    let retained = retained + other.retained_storage();
    drop((cap, policy, other, carriage));
    b.release_storage(retained).unwrap();
    assert_eq!(b.storage(), 0);
}
