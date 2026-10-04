//! Inert subject comparison only; no compiler, issuer or GPU authority is minted.
use super::*;
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;
use sha2::{Digest, Sha256};

const BYTES: usize = fe2o3_artifact_transaction::INERT_COMPILER_EXECUTION_SUBJECT_BYTES_V3;

fn wire() -> [u8; BYTES] {
    let mut bytes = [0; BYTES];
    bytes[..8].copy_from_slice(b"F2O3CES3");
    bytes[8..10].copy_from_slice(&3u16.to_le_bytes());
    bytes[12..20].copy_from_slice(&(BYTES as u64).to_le_bytes());
    bytes[24..32].copy_from_slice(&9u64.to_le_bytes());
    bytes[32..80].fill(1);
    bytes[88..152].fill(2);
    let pins = [[3; 32], [4; 32], [5; 32], [6; 32], [7; 32], [8; 32]];
    let closure = fe2o3_build_authority::CompilerClosureV2::new(
        pins[0], pins[1], pins[2], pins[3], pins[4], pins[5],
    )
    .unwrap();
    for (slot, pin) in bytes[152..344].chunks_exact_mut(32).zip(pins) {
        slot.copy_from_slice(&pin);
    }
    bytes[344..346].copy_from_slice(&1u16.to_le_bytes());
    bytes[346..378].copy_from_slice(&closure.identity_sha256());
    for slot in bytes[378..658].chunks_exact_mut(40) {
        slot[..32].fill(9);
        slot[32..].copy_from_slice(&1000u64.to_le_bytes());
    }
    seal(&mut bytes);
    bytes
}

fn seal(bytes: &mut [u8; BYTES]) {
    let mut hash = Sha256::new();
    hash.update(b"FE2O3/INERT-COMPILER-EXECUTION-SUBJECT/V3\0");
    hash.update(658u64.to_le_bytes());
    hash.update(&bytes[..658]);
    bytes[658..].copy_from_slice(&hash.finalize());
}

fn decode(bytes: &[u8; BYTES], b: &mut Budget<'_>) -> Subject {
    let (subject, storage) = Subject::decode(bytes, b).unwrap();
    b.reserve_storage(storage.retained_storage()).unwrap();
    subject
}

#[test]
fn native_publication_rejects_resealed_occurrence_and_content_substitution() {
    let mut work = Work::new(usize::MAX);
    let mut b = Budget::new(&mut work, 4 * 1024 * 1024);
    b.reserve_storage(2 * BYTES).unwrap();
    let original = wire();
    let expected = decode(&original, &mut b);
    let account = b.work_ledger_identity_v1();
    // Occurrence, invocation, and all seven capsule/module bindings, including
    // their lengths. Every mutated wire is independently valid and resealed.
    for offset in [
        24, 32, 48, 88, 120, 378, 410, 418, 450, 458, 490, 498, 530, 538, 570, 578, 610, 618, 650,
    ] {
        let mut changed = original;
        changed[offset] ^= 1;
        seal(&mut changed);
        let actual = decode(&changed, &mut b);
        let floor = b.storage();
        let before = b.work();
        assert!(matches!(
            require_exact_subject(&actual, &expected, &mut b),
            Err(Error::Mismatch(
                "native receipt differs from published V5 subject"
            ))
        ));
        assert_eq!(b.storage(), floor);
        assert_eq!(b.work(), before + BYTES);
        assert!(b.work_ledger_identity_v1() == account);
    }
    require_exact_subject(&expected, &expected, &mut b).unwrap();
}

#[test]
fn native_publication_subject_comparison_refuses_work_before_comparing() {
    const DECODE: usize = fe2o3_artifact_transaction::INERT_COMPILER_EXECUTION_SUBJECT_WORK_V3;
    let mut work = Work::new(DECODE + BYTES - 1);
    let mut b = Budget::new(&mut work, 4 * 1024 * 1024);
    b.reserve_storage(BYTES).unwrap();
    let subject = decode(&wire(), &mut b);
    let floor = b.storage();
    assert!(matches!(
        require_exact_subject(&subject, &subject, &mut b),
        Err(Error::Resource(Resource::Work(_)))
    ));
    assert_eq!(b.storage(), floor);
    assert_eq!(b.work(), DECODE);
    assert_eq!(b.failed_work(), Some(DECODE + BYTES));
}
