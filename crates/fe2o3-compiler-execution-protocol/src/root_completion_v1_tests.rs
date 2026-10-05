use super::*;
use crate::{
    CompilerExecutionIssuerMeasurementV1 as Measurement, CompilerExecutionIssuerPolicyV3 as Policy,
};
use ed25519_dalek::SigningKey;
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;

fn retain<T, E: fmt::Debug>(result: std::result::Result<(T, Storage), E>, b: &mut Budget<'_>) -> T {
    let (value, charge) = result.unwrap();
    b.reserve_storage(charge.additional_storage()).unwrap();
    value
}

fn inputs(mask: u8, nonce: u8, b: &mut Budget<'_>) -> (Intake, Intake, Intake) {
    let policy = retain(
        Policy::new(
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
        ),
        b,
    );
    let hello = retain(
        Intake::hello(&policy, [0x71; 32], [nonce; 32], mask, 4096, (71, 72), b),
        b,
    );
    let challenge = retain(Intake::challenge(&hello, [0x73; 32], b), b);
    let last = retain(
        Intake::input(&challenge, challenge.roles().last().unwrap(), b),
        b,
    );
    (hello, challenge, last)
}

#[test]
fn terminal_roundtrip_and_original_intake_binding_cover_every_stdio_mask() {
    for mask in 0..8 {
        let mut work = Work::new(usize::MAX);
        let mut b = Budget::new(&mut work, 1_000_000);
        let (hello, challenge, last) = inputs(mask, 0x72, &mut b);
        let (_, _, other) = inputs(mask, 0x74, &mut b);
        for termination in [
            Termination::Exited(0),
            Termination::Exited(255),
            Termination::Signaled(1),
            Termination::Signaled(64),
        ] {
            let complete = retain(Record::new(&last, termination, &mut b), &mut b);
            assert_eq!(complete.termination(), termination);
            assert!(complete.matches_intake(&last, &mut b).unwrap());
            assert!(!complete.matches_intake(&other, &mut b).unwrap());
            assert!(!complete.matches_intake(&challenge, &mut b).unwrap());
            b.reserve_storage(N).unwrap();
            let decoded = retain(Record::decode(complete.canonical_bytes(), &mut b), &mut b);
            assert_eq!(decoded, complete);
            assert!(Intake::decode(complete.canonical_bytes(), &mut b).is_err());
        }
        assert!(Record::new(&hello, Termination::Exited(0), &mut b).is_err());
        assert!(Record::new(&challenge, Termination::Exited(0), &mut b).is_err());
        let ack = retain(Intake::enforcement_unavailable(&last, &mut b), &mut b);
        assert!(Record::new(&ack, Termination::Exited(0), &mut b).is_err());
        assert!(Record::decode(ack.canonical_bytes(), &mut b).is_err());
        for signal in [0, 65, 255] {
            assert!(Record::new(&last, Termination::Signaled(signal), &mut b).is_err());
        }
    }
}

#[test]
fn malformed_rehashed_completion_and_every_changed_association_are_rejected() {
    let mut work = Work::new(usize::MAX);
    let mut b = Budget::new(&mut work, 1_000_000);
    let (_, _, last) = inputs(0, 0x72, &mut b);
    let complete = retain(Record::new(&last, Termination::Exited(0), &mut b), &mut b);
    b.reserve_storage(N).unwrap();
    for (index, value) in [
        (0, b'X'),
        (8, 2),
        (12, 1),
        (16, 0),
        (16, 3),
        (17, 1),
        (18, 8),
        (19, 1),
        (21, 1),
    ] {
        let mut bytes = *complete.canonical_bytes();
        bytes[index] = value;
        let identity = digest(&bytes);
        bytes[DIGEST..].copy_from_slice(&identity);
        assert!(Record::decode(&bytes, &mut b).is_err(), "field {index}");
    }
    for start in [24, 56, 88, 120, 152] {
        let mut bytes = *complete.canonical_bytes();
        bytes[start..start + 32].fill(0);
        let identity = digest(&bytes);
        bytes[DIGEST..].copy_from_slice(&identity);
        assert!(Record::decode(&bytes, &mut b).is_err());
    }
    for index in [18, 24, 56, 88, 120, 152, 184, 192, 200] {
        let mut bytes = *complete.canonical_bytes();
        bytes[index] ^= 1;
        let identity = digest(&bytes);
        bytes[DIGEST..].copy_from_slice(&identity);
        let changed = retain(Record::decode(&bytes, &mut b), &mut b);
        assert!(
            !changed.matches_intake(&last, &mut b).unwrap(),
            "field {index}"
        );
    }
    let mut corrupt = *complete.canonical_bytes();
    corrupt[N - 1] ^= 1;
    assert!(Record::decode(&corrupt, &mut b).is_err());
    assert!(Record::decode(&corrupt[..N - 1], &mut b).is_err());
    for length in [0, MAX_DESCRIPTOR_BYTES_V3 as u64 + 1, u64::MAX] {
        let mut bytes = *complete.canonical_bytes();
        bytes[184..192].copy_from_slice(&length.to_le_bytes());
        let identity = digest(&bytes);
        bytes[DIGEST..].copy_from_slice(&identity);
        assert!(Record::decode(&bytes, &mut b).is_err());
    }
}

#[test]
fn completion_decode_exact_one_short_and_missing_floor_preserve_account() {
    let mut initial_work = Work::new(usize::MAX);
    let mut initial = Budget::new(&mut initial_work, 1_000_000);
    let (_, _, last) = inputs(0, 0x72, &mut initial);
    let complete = retain(
        Record::new(&last, Termination::Exited(0), &mut initial),
        &mut initial,
    );
    for (allowance, scratch, floor, expected) in [
        (WORK, SCRATCH, N, None),
        (WORK - 1, SCRATCH, N, Some("work")),
        (WORK, SCRATCH - 1, N, Some("storage")),
        (WORK, SCRATCH, N - 1, Some("account")),
    ] {
        let mut work = Work::new(allowance);
        let mut b = Budget::new(&mut work, floor + scratch);
        b.reserve_storage(floor).unwrap();
        let result = Record::decode(complete.canonical_bytes(), &mut b);
        assert_eq!(b.storage(), floor);
        match (expected, result) {
            (None, Ok((decoded, charge))) => {
                assert_eq!(decoded, complete);
                assert_eq!(charge.additional_storage(), RETAINED);
                assert_eq!(b.work(), WORK);
                assert_eq!(b.peak_storage(), N + SCRATCH);
            }
            (Some("work"), Err(Error::Resource(Resource::Work(_)))) => {}
            (Some("storage"), Err(Error::Resource(Resource::Storage(_)))) => {}
            (Some("account"), Err(Error::Resource(Resource::Accounting))) => {}
            (_, result) => panic!("unexpected completion funding result: {result:?}"),
        }
    }
}

#[test]
fn completion_construction_and_matching_require_original_input_floor_and_funding() {
    let mut source_work = Work::new(usize::MAX);
    let mut source = Budget::new(&mut source_work, 1_000_000);
    let (_, _, last) = inputs(7, 0x72, &mut source);
    let complete = retain(
        Record::new(&last, Termination::Exited(7), &mut source),
        &mut source,
    );
    for matching in [false, true] {
        let floor = last.retained_storage() + if matching { RETAINED } else { 0 };
        for mode in 0..4 {
            let mut work = Work::new(WORK - usize::from(mode == 1));
            let mut b = Budget::new(&mut work, floor + SCRATCH - usize::from(mode == 2));
            b.reserve_storage(floor - usize::from(mode == 3)).unwrap();
            let result = if matching {
                complete.matches_intake(&last, &mut b)
            } else {
                Record::new(&last, Termination::Exited(7), &mut b).map(|(r, _)| r == complete)
            };
            assert_eq!(b.storage(), floor - usize::from(mode == 3));
            match (mode, result) {
                (0, Ok(true)) => assert_eq!(b.work(), WORK),
                (1, Err(Error::Resource(Resource::Work(_)))) => {}
                (2, Err(Error::Resource(Resource::Storage(_)))) => {}
                (3, Err(Error::Resource(Resource::Accounting))) => {}
                (_, result) => panic!("unexpected completion operation result: {result:?}"),
            }
        }
    }
}
