//! Canonical inert transcript/accounting only, not authenticated intake coverage.
use super::*;
use crate::CompilerExecutionIssuerMeasurementV1 as Measurement;
use ed25519_dalek::SigningKey;
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;

const WORK: usize = COMPILER_EXECUTION_ROOT_INTAKE_WORK_V4;
const SCRATCH: usize = COMPILER_EXECUTION_ROOT_INTAKE_STORAGE_V4;
fn retain<T>(result: Result<(T, Storage)>, b: &mut Budget<'_>) -> T {
    let (owner, charge) = result.unwrap();
    b.reserve_storage(charge.additional_storage()).unwrap();
    owner
}
fn policy(b: &mut Budget<'_>) -> Policy {
    let (p, charge) = Policy::new(
        7,
        Measurement::new([0x61; 32], 12345).unwrap(),
        Measurement::new([0x62; 32], 67890).unwrap(),
        SigningKey::from_bytes(&[0x51; 32])
            .verifying_key()
            .to_bytes(),
        SigningKey::from_bytes(&[0x52; 32])
            .verifying_key()
            .to_bytes(),
        b,
    )
    .unwrap();
    b.reserve_storage(charge.additional_storage()).unwrap();
    p
}
fn hello(p: &Policy, mask: u8, b: &mut Budget<'_>) -> Record {
    retain(
        Record::hello(p, [0x71; 32], [0x72; 32], mask, 4096, (71, 72), b),
        b,
    )
}
fn reseal(bytes: &mut [u8; N]) {
    let mut hash = Sha256::new();
    hash.update(b"FE2O3/COMPILER-ROOT-INTAKE/V4\0");
    hash.update(&bytes[..208]);
    bytes[208..].copy_from_slice(&hash.finalize());
}

#[test]
fn canonical_roles_and_exact_predecessors_cover_every_stdio_mask() {
    for mask in 0..8 {
        let mut work = Work::new(usize::MAX);
        let mut b = Budget::new(&mut work, 1_000_000);
        let p = policy(&mut b);
        let hello = hello(&p, mask, &mut b);
        assert_eq!(
            &hello.canonical_bytes()[..16],
            b"F2O3CRI4\x04\0\0\0\xf0\0\0\0"
        );
        assert_eq!(hello.kind(), Kind::Hello);
        assert_eq!(hello.role(), None);
        assert_eq!(hello.invocation_bytes(), 4096);
        let challenge = retain(Record::challenge(&hello, [0x73; 32], &mut b), &mut b);
        assert!(challenge.matches_predecessor(&hello, &mut b).unwrap());
        assert!(!hello.matches_predecessor(&challenge, &mut b).unwrap());
        let mut count = 0;
        for role in challenge.roles() {
            let input = retain(Record::input(&challenge, role, &mut b), &mut b);
            assert!(input.matches_predecessor(&challenge, &mut b).unwrap());
            b.reserve_storage(N).unwrap();
            let decoded = retain(Record::decode(input.canonical_bytes(), &mut b), &mut b);
            assert_eq!(decoded, input);
            let result = Record::enforcement_unavailable(&input, &mut b);
            if Some(role) == challenge.roles().last() {
                let ack = retain(result, &mut b);
                assert_eq!(ack.kind(), Kind::Ack);
                assert_eq!(ack.canonical_bytes()[19], 1);
                assert!(ack.matches_predecessor(&input, &mut b).unwrap());
                assert!(!ack.matches_predecessor(&challenge, &mut b).unwrap());
                assert!(Record::enforcement_unavailable(&ack, &mut b).is_err());
            } else {
                assert!(result.is_err());
            }
            count += 1;
        }
        assert_eq!(count, 3 + mask.count_ones());
        for (bit, role) in [(1, Role::Stdin), (2, Role::Stdout), (4, Role::Stderr)] {
            assert_eq!(
                Record::input(&challenge, role, &mut b).is_ok(),
                mask & bit != 0
            );
        }
    }
}

#[test]
fn decoded_rehashed_noncanonical_shapes_never_gain_a_status_or_role() {
    let mut work = Work::new(usize::MAX);
    let mut b = Budget::new(&mut work, 1_000_000);
    let p = policy(&mut b);
    let hello = hello(&p, 0, &mut b);
    b.reserve_storage(N).unwrap();
    for (index, value) in [
        (0, b'X'),
        (8, 1),
        (12, 1),
        (16, 0),
        (16, 5),
        (17, 1),
        (18, 8),
        (19, 1),
        (20, 1),
        (56, 1),
        (152, 1),
    ] {
        let mut bytes = *hello.canonical_bytes();
        bytes[index] = value;
        reseal(&mut bytes);
        assert!(Record::decode(&bytes, &mut b).is_err(), "changed {index}");
    }
    for start in [24, 88, 120] {
        let mut bytes = *hello.canonical_bytes();
        bytes[start..start + 32].fill(0);
        reseal(&mut bytes);
        assert!(Record::decode(&bytes, &mut b).is_err());
    }
    for length in [0, MAX_DESCRIPTOR_BYTES_V3 as u64 + 1, u64::MAX] {
        let mut bytes = *hello.canonical_bytes();
        bytes[184..192].copy_from_slice(&length.to_le_bytes());
        reseal(&mut bytes);
        assert!(Record::decode(&bytes, &mut b).is_err());
        assert!(Record::hello(&p, [1; 32], [2; 32], 0, length, (71, 72), &mut b).is_err());
    }
    for length in [1, MAX_DESCRIPTOR_BYTES_V3 as u64] {
        assert!(Record::hello(&p, [1; 32], [2; 32], 0, length, (71, 72), &mut b).is_ok());
    }
    assert!(Record::decode(&hello.canonical_bytes()[..N - 1], &mut b).is_err());
    let mut bytes = *hello.canonical_bytes();
    bytes[N - 1] ^= 1;
    assert!(Record::decode(&bytes, &mut b).is_err());
}

#[test]
fn association_and_last_input_digest_are_not_replaceable() {
    let mut work = Work::new(usize::MAX);
    let mut b = Budget::new(&mut work, 1_000_000);
    let p = policy(&mut b);
    let hello = hello(&p, 0, &mut b);
    let challenge = retain(Record::challenge(&hello, [3; 32], &mut b), &mut b);
    let other = retain(Record::challenge(&hello, [4; 32], &mut b), &mut b);
    let input = retain(
        Record::input(&challenge, Role::OutputDirectory, &mut b),
        &mut b,
    );
    assert!(!input.matches_predecessor(&other, &mut b).unwrap());
    let ack = retain(Record::enforcement_unavailable(&input, &mut b), &mut b);
    b.reserve_storage(N).unwrap();
    for index in [18, 24, 56, 88, 120, 152, 184, 192, 200] {
        let mut bytes = *ack.canonical_bytes();
        bytes[index] ^= 1;
        reseal(&mut bytes);
        let changed = retain(Record::decode(&bytes, &mut b), &mut b);
        assert!(
            !changed.matches_predecessor(&input, &mut b).unwrap(),
            "changed {index}"
        );
    }
    let mut bytes = *ack.canonical_bytes();
    bytes[19] = 0; // No accepted/Ready ACK exists.
    reseal(&mut bytes);
    assert!(Record::decode(&bytes, &mut b).is_err());
}

#[test]
fn exact_one_short_and_input_floor_refusal_keep_original_account_history() {
    const LIMIT: usize = 10_000_000;
    for mode in 0..4 {
        let mut work = Work::new(LIMIT);
        let mut b = Budget::new(&mut work, 1_000_000);
        let p = policy(&mut b);
        let hello = hello(&p, 0, &mut b);
        let bytes = *hello.canonical_bytes();
        // These are inert bytes; retire their unrelated constructor inputs first.
        drop((hello, p));
        b.release_storage(b.storage()).unwrap();
        b.reserve_storage(if mode == 3 { N - 1 } else { N })
            .unwrap();
        if mode != 3 {
            b.reserve_storage(b.storage_limit() - b.storage() - SCRATCH + usize::from(mode == 2))
                .unwrap();
        }
        b.charge_work(LIMIT - b.work() - WORK + usize::from(mode == 1))
            .unwrap();
        let floor = b.storage();
        let ledger = b.work_ledger_identity_v1();
        let result = Record::decode(&bytes, &mut b);
        assert_eq!(b.storage(), floor);
        assert!(b.work_ledger_identity_v1() == ledger);
        match mode {
            0 => {
                assert!(result.is_ok());
                assert_eq!(b.work(), LIMIT);
                assert_eq!(b.peak_storage(), 1_000_000);
            }
            1 => {
                assert!(matches!(result, Err(Error::Resource(Resource::Work(_)))));
                assert_eq!(b.failed_work(), Some(LIMIT + 1));
            }
            2 => {
                assert!(matches!(result, Err(Error::Resource(Resource::Storage(_)))));
                assert_eq!(b.failed_storage(), Some(1_000_001));
            }
            _ => assert!(matches!(result, Err(Error::Resource(Resource::Accounting)))),
        }
    }
}

#[test]
fn v3_and_v4_never_upgrade_or_downgrade_and_output_role_is_mandatory() {
    use crate::CompilerExecutionRootIntakeRecordV3 as Old;
    let mut work = Work::new(usize::MAX);
    let mut b = Budget::new(&mut work, 1_000_000);
    let p = policy(&mut b);
    let hello = hello(&p, 0, &mut b);
    assert_eq!(hello.output_identity(), (71, 72));
    assert_eq!(
        hello.roles().collect::<Vec<_>>(),
        [
            Role::Invocation,
            Role::WorkingDirectory,
            Role::OutputDirectory
        ]
    );
    b.reserve_storage(N).unwrap();
    assert!(Old::decode(hello.canonical_bytes(), &mut b).is_err());
    let (old, charge) = Old::hello(&p, [1; 32], [2; 32], 0, 4096, &mut b).unwrap();
    b.reserve_storage(charge.additional_storage()).unwrap();
    assert!(Record::decode(old.canonical_bytes(), &mut b).is_err());
    let (old, charge) = Old::challenge(&old, [3; 32], &mut b).unwrap();
    b.reserve_storage(charge.additional_storage()).unwrap();
    let mut bytes = *old.canonical_bytes();
    bytes[16] = 3;
    bytes[17] = 6;
    let mut hash = Sha256::new();
    hash.update(b"FE2O3/COMPILER-ROOT-INTAKE/V3\0");
    hash.update(&bytes[..192]);
    bytes[192..].copy_from_slice(&hash.finalize());
    assert!(Old::decode(&bytes, &mut b).is_err());
    // Neither coordinate is proof; do not impose invented nonzero-ID policy.
    assert!(Record::hello(&p, [1; 32], [2; 32], 0, 1, (0, 0), &mut b).is_ok());
}
