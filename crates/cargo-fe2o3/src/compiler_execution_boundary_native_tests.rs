//! Portable custody/receipt checks only; fixtures do not authenticate a service boot.
use super::*;
use fe2o3_compiler_execution_protocol::{
    CompilerExecutionAttestationReceiptV3 as Receipt,
    CompilerExecutionAttestationRequestV3 as Request,
    CompilerExecutionClientProcessIdentityV1 as Process,
    CompilerExecutionClientProfileV3 as ProfileRecord,
    CompilerExecutionExternalAnchorServiceIdentityV1 as Anchor,
    CompilerExecutionIssuerPolicyV3 as PolicyRecord,
    CompilerExecutionReceiptPublicationAckV3 as Ack,
    CompilerExecutionReceiptPublicationV3 as Publication,
};
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;

#[path = "../../fe2o3-compiler-execution-protocol/tests/support/native_attestation_fixture.rs"]
mod fixture;
#[path = "../../fe2o3-compiler-execution-protocol/tests/support/native_receipt_fixture.rs"]
mod receipt_fixture;

const WORK: usize = 30_000_000;
const LIMIT: usize = 2_000_000;
const CHILD: u32 = 101;

fn policy(generation: u64, b: &mut Budget<'_>) -> PolicyRecord {
    let mut bytes = fixture::policy_wire(3);
    bytes[24..32].copy_from_slice(&generation.to_le_bytes());
    fixture::seal(&mut bytes, "COMPILER-EXECUTION-ISSUER-POLICY", 3);
    b.reserve_storage(bytes.len()).unwrap();
    let (policy, storage) = PolicyRecord::decode(&bytes, b).unwrap();
    b.reserve_storage(storage.additional_storage()).unwrap();
    b.release_storage(bytes.len()).unwrap();
    policy
}

fn profile(generation: u64, b: &mut Budget<'_>) -> Profile {
    let policy = policy(generation, b);
    let (profile, storage) =
        ProfileRecord::new(2000, 2001, Anchor::new(6000, 6001).unwrap(), policy, b).unwrap();
    b.reserve_storage(storage.additional_storage()).unwrap();
    let (profile, storage) = Profile::create(profile, b).unwrap();
    b.reserve_storage(storage.additional_storage()).unwrap();
    profile
}

fn sealed_policy(generation: u64, b: &mut Budget<'_>) -> Policy {
    let policy = policy(generation, b);
    let (policy, storage) = Policy::create(policy, b).unwrap();
    b.reserve_storage(storage.additional_storage()).unwrap();
    policy
}

fn records(
    policy: &Policy,
    pid: u32,
    uid: u32,
    anchor: u32,
    b: &mut Budget<'_>,
) -> (Manifest, Ready) {
    let (manifest, storage) = Manifest::new(
        Process::new(pid, uid, 1000).unwrap(),
        Anchor::new(anchor, 6001).unwrap(),
        policy.policy(),
        b,
    )
    .unwrap();
    b.reserve_storage(storage.additional_storage()).unwrap();
    let (ready, storage) = Ready::new(200, &manifest, policy.policy(), b).unwrap();
    b.reserve_storage(storage.additional_storage()).unwrap();
    (manifest, ready)
}

fn subject(b: &mut Budget<'_>) -> Subject {
    let bytes = fixture::subject_wire(3);
    b.reserve_storage(bytes.len()).unwrap();
    let (subject, storage) = Subject::decode(&bytes, b).unwrap();
    b.reserve_storage(storage.retained_storage()).unwrap();
    b.release_storage(bytes.len()).unwrap();
    subject
}

fn carriage(b: &mut Budget<'_>) -> Carriage {
    let p = policy(7, b);
    let q = fixture::request_wire(3);
    let r = receipt_fixture::receipt_wire(3);
    b.reserve_storage(q.len() + r.len()).unwrap();
    let (q, storage) = Request::decode(&q, b).unwrap();
    b.reserve_storage(storage.additional_storage()).unwrap();
    let (r, storage) = Receipt::decode(&r, b).unwrap();
    b.reserve_storage(storage.additional_storage()).unwrap();
    let (u, storage) = Publication::new([0x81; 32], [0x82; 32], r, b).unwrap();
    b.reserve_storage(storage.additional_storage()).unwrap();
    let (a, storage) = Ack::new(&u, [0x83; 32], b).unwrap();
    b.reserve_storage(storage.additional_storage()).unwrap();
    let (carriage, storage) = Carriage::new(p, q, u, a, b).unwrap();
    b.reserve_storage(storage.additional_storage()).unwrap();
    b.release_storage(fixture::REQUEST_BYTES + 400).unwrap();
    carriage
}

#[test]
fn exact_readiness_retains_seals_and_every_context_axis() {
    let mut work = Work::new(WORK);
    let mut b = Budget::new(&mut work, LIMIT);
    let profile = profile(7, &mut b);
    let policy = sealed_policy(7, &mut b);
    let (manifest, ready) = records(&policy, CHILD, 1000, 6000, &mut b);
    let floor = b.storage();
    validate_readiness(&profile, &policy, CHILD, &manifest, &ready, &mut b).unwrap();
    assert_eq!(b.storage(), floor);
    for child in [0, CHILD + 1] {
        assert!(matches!(
            validate_readiness(&profile, &policy, child, &manifest, &ready, &mut b),
            Err(Failure::Mismatch(_))
        ));
    }
    for (pid, uid, anchor) in [
        (CHILD + 1, 1000, 6000),
        (CHILD, 2000, 6000),
        (CHILD, 1000, 6002),
    ] {
        let (other, other_ready) = records(&policy, pid, uid, anchor, &mut b);
        assert!(matches!(
            validate_readiness(&profile, &policy, CHILD, &other, &other_ready, &mut b),
            Err(Failure::Mismatch(_))
        ));
        assert!(
            validate_readiness(&profile, &policy, CHILD, &manifest, &other_ready, &mut b).is_err()
        );
    }
    let other_policy = sealed_policy(8, &mut b);
    assert!(matches!(
        validate_readiness(&profile, &other_policy, CHILD, &manifest, &ready, &mut b),
        Err(Failure::Mismatch(_))
    ));
    let (other, other_ready) = records(&other_policy, CHILD, 1000, 6000, &mut b);
    assert!(matches!(
        validate_readiness(&profile, &policy, CHILD, &other, &other_ready, &mut b),
        Err(Failure::Mismatch(_))
    ));
}

#[test]
fn exact_readiness_and_one_short_limits_preserve_original_ledger() {
    let mut setup_work = Work::new(WORK);
    let mut setup = Budget::new(&mut setup_work, LIMIT);
    let profile = profile(7, &mut setup);
    let policy = sealed_policy(7, &mut setup);
    let (manifest, ready) = records(&policy, CHILD, 1000, 6000, &mut setup);
    let floor = profile.retained_storage()
        + policy.retained_storage()
        + manifest.retained_storage()
        + ready.retained_storage();
    let mut expected = (WORK, LIMIT);
    for case in 0..5 {
        let (quota, limit, input) = match case {
            2 => (expected.0 - 1, expected.1, floor),
            3 => (expected.0, expected.1 - 1, floor),
            4 => (expected.0, expected.1, floor - 1),
            _ => (expected.0, expected.1, floor),
        };
        let mut work = Work::new(quota);
        let mut b = Budget::new(&mut work, limit);
        b.reserve_storage(input).unwrap();
        b.charge_work(19).unwrap();
        let ledger = b.work_ledger_identity_v1();
        let result = validate_readiness(&profile, &policy, CHILD, &manifest, &ready, &mut b);
        assert_eq!(result.is_ok(), case < 2, "case {case}: {result:?}");
        assert_eq!(b.storage(), input);
        assert!(b.work_ledger_identity_v1() == ledger);
        if case == 0 {
            expected = (b.work(), b.peak_storage());
        }
        if case == 3 {
            assert!(b.failed_storage().is_some());
        }
        if case == 4 {
            assert_eq!(b.work(), 19);
        }
    }
}

#[test]
fn receipt_matches_full_policy_subject_and_signature_without_granting_authority() {
    let mut work = Work::new(WORK);
    let mut b = Budget::new(&mut work, LIMIT);
    let profile = profile(7, &mut b);
    let subject = subject(&mut b);
    let carriage = carriage(&mut b);
    let floor = b.storage();
    let (decoded, storage) =
        decode_receipt(&profile, &subject, carriage.canonical_bytes(), &mut b).unwrap();
    assert_eq!(decoded, carriage);
    assert_eq!(storage.additional_storage(), decoded.retained_storage());
    assert_eq!(b.storage(), floor);
    assert!(!decoded.grants_compiler_authority());
    assert!(!decoded.grants_load_authority());
    assert!(!decoded.grants_launch_authority());
    assert!(decoded.requires_protected_policy_verification());
    drop(decoded);

    let other_profile = self::profile(8, &mut b);
    assert!(matches!(
        decode_receipt(&other_profile, &subject, carriage.canonical_bytes(), &mut b),
        Err(Failure::Mismatch(_))
    ));
    let mut other_subject = fixture::subject_wire(3);
    other_subject[24..32].copy_from_slice(&10u64.to_le_bytes());
    fixture::seal(&mut other_subject, "INERT-COMPILER-EXECUTION-SUBJECT", 3);
    b.reserve_storage(other_subject.len()).unwrap();
    let (other_subject, storage) = Subject::decode(&other_subject, &mut b).unwrap();
    b.reserve_storage(storage.retained_storage()).unwrap();
    assert!(matches!(
        decode_receipt(&profile, &other_subject, carriage.canonical_bytes(), &mut b),
        Err(Failure::Mismatch(_))
    ));

    let mut corrupt = *carriage.canonical_bytes();
    b.reserve_storage(corrupt.len()).unwrap();
    corrupt[30] ^= 1;
    fixture::seal(&mut corrupt, "COMPILER-EXECUTION-RECEIPT-CARRIAGE", 3);
    assert!(matches!(
        decode_receipt(&profile, &subject, &corrupt, &mut b),
        Err(Failure::Receipt(_))
    ));
    for version in [1u16, 2] {
        let mut downgraded = *carriage.canonical_bytes();
        downgraded[7] = b'0' + version as u8;
        downgraded[8..10].copy_from_slice(&version.to_le_bytes());
        fixture::seal(
            &mut downgraded,
            "COMPILER-EXECUTION-RECEIPT-CARRIAGE",
            version,
        );
        assert!(matches!(
            decode_receipt(&profile, &subject, &downgraded, &mut b),
            Err(Failure::Receipt(_))
        ));
    }
    for bytes in [
        &[][..],
        &carriage.canonical_bytes()[..carriage.canonical_bytes().len() - 1],
    ] {
        assert!(matches!(
            decode_receipt(&profile, &subject, bytes, &mut b),
            Err(Failure::Receipt(_))
        ));
    }
}

#[test]
fn receipt_decode_limits_refund_only_scratch_and_return_full_owner_charge() {
    let mut setup_work = Work::new(WORK);
    let mut setup = Budget::new(&mut setup_work, LIMIT);
    let profile = profile(7, &mut setup);
    let subject = subject(&mut setup);
    let carriage = carriage(&mut setup);
    let bytes = carriage.canonical_bytes();
    let floor = profile.retained_storage() + SUBJECT_STORAGE + bytes.len();
    let mut expected = (WORK, LIMIT);
    for case in 0..5 {
        let (quota, limit, input) = match case {
            2 => (expected.0 - 1, expected.1, floor),
            3 => (expected.0, expected.1 - 1, floor),
            4 => (expected.0, expected.1, floor - 1),
            _ => (expected.0, expected.1, floor),
        };
        let mut work = Work::new(quota);
        let mut b = Budget::new(&mut work, limit);
        b.reserve_storage(input).unwrap();
        b.charge_work(19).unwrap();
        let ledger = b.work_ledger_identity_v1();
        let result = decode_receipt(&profile, &subject, bytes, &mut b);
        assert_eq!(result.is_ok(), case < 2, "case {case}: {result:?}");
        assert_eq!(b.storage(), input);
        assert!(b.work_ledger_identity_v1() == ledger);
        if case == 0 {
            expected = (b.work(), b.peak_storage());
        }
        if case == 3 {
            assert!(b.failed_storage().is_some());
        }
        if case == 4 {
            assert_eq!(b.work(), 19);
        }
        if let Ok((owner, charge)) = result {
            assert_eq!(charge.additional_storage(), owner.retained_storage());
            b.reserve_storage(charge.additional_storage()).unwrap();
            drop(owner);
            b.release_storage(charge.additional_storage()).unwrap();
            assert_eq!(b.storage(), input);
        }
    }
}
