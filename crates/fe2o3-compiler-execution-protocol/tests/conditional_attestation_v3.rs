//! Component wire/signature tests. No protected process, receipt or authority fixture.
use ed25519_dalek::SigningKey;
use fe2o3_artifact_transaction::InertCompilerExecutionSubjectV3 as Subject;
use fe2o3_compiler_execution_protocol::*;
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
    CanonicalKernelIrWorkBudgetV1 as Work,
};
use sha2::{Digest, Sha256};
use std::mem::size_of;

#[path = "support/native_attestation_fixture.rs"]
mod fixture;
#[path = "support/native_receipt_fixture.rs"]
mod receipt_fixture;
use fixture::*;
use receipt_fixture::*;

type Policy = CompilerExecutionIssuerPolicyV3;
type Challenge = CompilerExecutionAttestationChallengeV3;
type Request = CompilerExecutionAttestationRequestV3;
type Receipt = CompilerExecutionAttestationReceiptV3;
type Error = CompilerExecutionAttestationErrorV3;
type Framing = CompilerExecutionAttestationErrorV1;
const LIMIT: usize = 1_000_000;
const WORK: usize = 1_000_000_000;

fn run<T>(floor: usize, f: impl FnOnce(&mut Budget<'_>) -> Result<T, Error>) -> Result<T, Error> {
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, LIMIT);
    budget.reserve_storage(floor).unwrap();
    let ledger = budget.work_ledger_identity_v1();
    let result = f(&mut budget);
    assert_eq!(budget.storage(), floor);
    assert!(budget.work_ledger_identity_v1() == ledger);
    result
}
fn policy() -> Policy {
    run(216, |b| Policy::decode(&policy_wire(3), b)).unwrap().0
}
fn request(bytes: &[u8]) -> Result<Request, Error> {
    run(bytes.len(), |b| Request::decode(bytes, b)).map(|v| v.0)
}
fn receipt(bytes: &[u8]) -> Result<Receipt, Error> {
    run(bytes.len(), |b| Receipt::decode(bytes, b)).map(|v| v.0)
}
fn verify(bytes: &[u8], p: &Policy, q: &Request, anchor: [u8; 32]) -> Result<(), Error> {
    let r = receipt(bytes)?;
    run(
        p.retained_storage() + q.retained_storage() + r.retained_storage(),
        |b| r.verify(p, q, anchor, b).map(|_| ()),
    )
}
fn framing(error: Error) -> Framing {
    match error {
        Error::Framing(error) => error,
        other => panic!("expected framing failure, got {other:?}"),
    }
}
fn assert_framing(error: Error, expected: Framing) {
    assert_eq!(format!("{:?}", framing(error)), format!("{expected:?}"));
}

#[test]
fn conditional_protocol_v3_independent_transcripts_and_v2_quotes() {
    let p = policy();
    let q = request(&request_wire(3)).unwrap();
    let r = receipt(&receipt_wire(3)).unwrap();
    assert_eq!(p.canonical_bytes(), &policy_wire(3));
    assert_eq!(q.canonical_bytes(), &request_wire(3));
    assert_eq!(q.challenge().canonical_bytes(), &challenge_wire(3));
    assert_eq!(q.subject().canonical_bytes(), &subject_wire(3));
    assert_eq!(r.canonical_bytes(), &receipt_wire(3));
    assert_eq!(&p.canonical_bytes()[176..178], &3u16.to_le_bytes());
    verify(r.canonical_bytes(), &p, &q, [0; 32]).unwrap();

    for (v2, v3, exact) in [
        (
            COMPILER_EXECUTION_ISSUER_POLICY_WORK_V2,
            COMPILER_EXECUTION_ISSUER_POLICY_WORK_V3,
            15_112,
        ),
        (
            COMPILER_EXECUTION_ATTESTATION_CHALLENGE_WORK_V2,
            COMPILER_EXECUTION_ATTESTATION_CHALLENGE_WORK_V3,
            6_408,
        ),
        (
            COMPILER_EXECUTION_ATTESTATION_REQUEST_WORK_V2,
            COMPILER_EXECUTION_ATTESTATION_REQUEST_WORK_V3,
            30_280,
        ),
        (
            COMPILER_EXECUTION_ATTESTATION_REQUEST_DECODE_WORK_V2,
            COMPILER_EXECUTION_ATTESTATION_REQUEST_DECODE_WORK_V3,
            56_464,
        ),
        (
            COMPILER_EXECUTION_ATTESTATION_RECEIPT_IDENTITY_WORK_V2,
            COMPILER_EXECUTION_ATTESTATION_RECEIPT_IDENTITY_WORK_V3,
            12_808,
        ),
        (
            COMPILER_EXECUTION_ATTESTATION_RECEIPT_DECODE_WORK_V2,
            COMPILER_EXECUTION_ATTESTATION_RECEIPT_DECODE_WORK_V3,
            152_072,
        ),
        (
            COMPILER_EXECUTION_ATTESTATION_RECEIPT_ISSUE_WORK_V2,
            COMPILER_EXECUTION_ATTESTATION_RECEIPT_ISSUE_WORK_V3,
            217_608,
        ),
        (
            COMPILER_EXECUTION_ATTESTATION_RECEIPT_VERIFY_WORK_V2,
            COMPILER_EXECUTION_ATTESTATION_RECEIPT_VERIFY_WORK_V3,
            147_976,
        ),
    ] {
        assert_eq!(v2, exact);
        assert_eq!(v3, exact);
    }
    for (a, b) in [
        (
            COMPILER_EXECUTION_ISSUER_POLICY_STORAGE_V2,
            COMPILER_EXECUTION_ISSUER_POLICY_STORAGE_V3,
        ),
        (
            COMPILER_EXECUTION_ATTESTATION_CHALLENGE_STORAGE_V2,
            COMPILER_EXECUTION_ATTESTATION_CHALLENGE_STORAGE_V3,
        ),
        (
            COMPILER_EXECUTION_ATTESTATION_REQUEST_STORAGE_V2,
            COMPILER_EXECUTION_ATTESTATION_REQUEST_STORAGE_V3,
        ),
        (
            COMPILER_EXECUTION_ATTESTATION_REQUEST_DECODE_STORAGE_V2,
            COMPILER_EXECUTION_ATTESTATION_REQUEST_DECODE_STORAGE_V3,
        ),
        (
            COMPILER_EXECUTION_ATTESTATION_RECEIPT_STORAGE_V2,
            COMPILER_EXECUTION_ATTESTATION_RECEIPT_STORAGE_V3,
        ),
        (
            size_of::<CompilerExecutionIssuerPolicyV2>(),
            size_of::<Policy>(),
        ),
        (
            size_of::<CompilerExecutionAttestationChallengeV2>(),
            size_of::<Challenge>(),
        ),
        (
            size_of::<CompilerExecutionAttestationRequestV2>(),
            size_of::<Request>(),
        ),
        (
            size_of::<CompilerExecutionAttestationReceiptV2>(),
            size_of::<Receipt>(),
        ),
        (
            size_of::<CompilerExecutionAttestationErrorV2>(),
            size_of::<Error>(),
        ),
    ] {
        assert_eq!(a, b);
    }
    // The existing independent V1/V2 golden fixtures remain byte-identical.
    for (version, expected) in [
        (
            1,
            "e1afce82f8f05d4ce4362ef7313320b3167b94a4884e008d851f29a8532cd3c7",
        ),
        (
            2,
            "7783073c13c574699b897e5498a9f8f1c32e87e2e5c738e2ca77a9572c8d3e2d",
        ),
    ] {
        assert_eq!(hex(&Sha256::digest(receipt_wire(version))), expected);
    }
}

#[test]
fn conditional_protocol_v3_construction_transfers_owners_without_authority() {
    let key = SigningKey::from_bytes(&[0x51; 32]);
    let anchor_key = SigningKey::from_bytes(&[0x52; 32]);
    let mut work = Work::new(WORK);
    let mut b = Budget::new(&mut work, LIMIT);
    b.reserve_storage(SUBJECT_BYTES + size_of::<SigningKey>())
        .unwrap();
    let (p, s) = Policy::new(
        7,
        CompilerExecutionIssuerMeasurementV1::new([0x61; 32], 12345).unwrap(),
        CompilerExecutionIssuerMeasurementV1::new([0x62; 32], 67890).unwrap(),
        key.verifying_key().to_bytes(),
        anchor_key.verifying_key().to_bytes(),
        &mut b,
    )
    .unwrap();
    b.reserve_storage(s.additional_storage()).unwrap();
    assert_eq!(p.canonical_bytes(), &policy_wire(3));
    let (subject, s) = Subject::decode(&subject_wire(3), &mut b).unwrap();
    b.reserve_storage(s.retained_storage()).unwrap();
    let subject_storage = s.retained_storage();
    let (challenge, s) = Challenge::new(&p, &subject, [0x71; 32], 1, [0; 32], &mut b).unwrap();
    b.reserve_storage(s.additional_storage()).unwrap();
    assert!(
        challenge
            .subject()
            .matches_subject(&subject, &mut b)
            .unwrap()
    );
    let inherited = subject_storage + s.additional_storage();
    let floor = b.storage();
    let (q, s) = Request::new(challenge, subject, &mut b).unwrap();
    assert_eq!(b.storage(), floor);
    assert_eq!(q.retained_storage(), inherited + s.additional_storage());
    b.reserve_storage(s.additional_storage()).unwrap();
    assert_eq!(q.canonical_bytes(), &request_wire(3));
    let (r, s) = Receipt::issue(&p, &q, &key, &mut b).unwrap();
    assert_eq!(r.retained_storage(), s.additional_storage());
    b.reserve_storage(s.additional_storage()).unwrap();
    assert_eq!(r.canonical_bytes(), &receipt_wire(3));
    assert!(!r.grants_compiler_authority());
    assert!(!r.grants_load_authority());
    assert!(!r.grants_launch_authority());
    let floor = b.storage();
    let (verified, s) = r.verify(&p, &q, [0; 32], &mut b).unwrap();
    assert_eq!(s.additional_storage(), 0);
    assert_eq!(b.storage(), floor);
    assert!(verified.authenticates_pinned_signing_key());
    assert!(!verified.authenticates_protected_compiler_execution());
    assert!(!verified.grants_compiler_authority());
    assert!(!verified.grants_load_authority());
    assert!(!verified.grants_launch_authority());
    assert_eq!(verified.receipt().request_sha256(), q.identity().as_bytes());
    assert_eq!(verified.into_receipt().canonical_bytes(), &receipt_wire(3));
}

#[test]
fn conditional_protocol_v3_outer_and_nested_families_cannot_mix() {
    for old in [1, 2] {
        assert!(run(216, |b| Policy::decode(&policy_wire(old), b)).is_err());
        assert!(run(200, |b| Challenge::decode(&challenge_wire(old), b)).is_err());
        assert!(request(&request_wire(old)).is_err());
        assert!(receipt(&receipt_wire(old)).is_err());
        for source in [false, true] {
            let mut bytes = request_wire(3);
            if source {
                bytes[224..914].copy_from_slice(&subject_wire(old));
            } else {
                bytes[24..224].copy_from_slice(&challenge_wire(old));
            }
            seal(&mut bytes, "COMPILER-EXECUTION-REQUEST", 3);
            assert!(request(&bytes).is_err());
            let mut bytes = request_wire(old);
            if source {
                bytes[224..914].copy_from_slice(&subject_wire(3));
            } else {
                bytes[24..224].copy_from_slice(&challenge_wire(3));
            }
            seal(&mut bytes, "COMPILER-EXECUTION-REQUEST", old);
            if old == 1 {
                assert!(CompilerExecutionAttestationRequestV1::decode(&bytes).is_err());
            } else {
                assert!(
                    run(946, |b| Ok(CompilerExecutionAttestationRequestV2::decode(
                        &bytes, b
                    )
                    .is_err()))
                    .unwrap()
                );
            }
        }
    }
    assert!(CompilerExecutionIssuerPolicyV1::decode(&policy_wire(3)).is_err());
    assert!(CompilerExecutionAttestationChallengeV1::decode(&challenge_wire(3)).is_err());
    assert!(CompilerExecutionAttestationRequestV1::decode(&request_wire(3)).is_err());
    assert!(CompilerExecutionAttestationReceiptV1::decode(&receipt_wire(3)).is_err());
    run(946, |b| {
        assert!(CompilerExecutionIssuerPolicyV2::decode(&policy_wire(3), b).is_err());
        assert!(CompilerExecutionAttestationChallengeV2::decode(&challenge_wire(3), b).is_err());
        assert!(CompilerExecutionAttestationRequestV2::decode(&request_wire(3), b).is_err());
        assert!(CompilerExecutionAttestationReceiptV2::decode(&receipt_wire(3), b).is_err());
        Ok(())
    })
    .unwrap();
}

#[test]
fn conditional_protocol_v3_every_byte_and_wrong_length_refuse() {
    for original in [
        policy_wire(3).to_vec(),
        challenge_wire(3).to_vec(),
        request_wire(3).to_vec(),
        receipt_wire(3).to_vec(),
    ] {
        let check = |bytes: &[u8]| {
            run(bytes.len(), |b| match original.len() {
                216 => Policy::decode(bytes, b).map(|_| ()),
                200 => Challenge::decode(bytes, b).map(|_| ()),
                946 => Request::decode(bytes, b).map(|_| ()),
                _ => Receipt::decode(bytes, b).map(|_| ()),
            })
        };
        for i in 0..original.len() {
            let mut changed = original.clone();
            changed[i] ^= 0x80;
            assert!(
                check(&changed).is_err(),
                "frame {} byte {i}",
                original.len()
            );
        }
        for len in [0, original.len() - 1, original.len() + 1] {
            assert!(check(&vec![0; len]).is_err());
        }
    }
}

#[path = "support/conditional_attestation_v3_mutations.rs"]
mod mutations;
#[path = "support/conditional_attestation_v3_resources.rs"]
mod resources;
