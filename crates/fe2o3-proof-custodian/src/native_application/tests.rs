use super::*;
use fe2o3_kernel_ir::CanonicalKernelIrOwnedVerificationResourceBudgetV1 as Owned;
use fe2o3_runtime_protocol as protocol;

#[allow(dead_code)]
#[path = "../../../fe2o3-runtime-protocol/tests/support/native_application_registration_fixture.rs"]
mod fixture;

fn capsule() -> (Owned, Capsule) {
    let mut account = Owned::new(Work::new(1_000_000_000), 64 * 1024 * 1024);
    let value = account.with_budget(|b| {
        let binding = fixture::binding(1201, 1200, 1001, 1001, b);
        let inherited = binding.retained_storage();
        let (transcript, charge) =
            Transcript::from_untrusted_parts([12; 32], [13; 32], *binding.identity().as_bytes(), b)
                .unwrap();
        b.reserve_storage(charge.additional_storage()).unwrap();
        let value = Capsule {
            parent: 2000,
            nonce: [11; 32],
            binding,
            transcript,
            peer: [14; 32],
            app_start: 1500,
            cargo_start: 1000,
        };
        value.validate().unwrap();
        b.reserve_storage(value.retained_storage() - inherited)
            .unwrap();
        value
    });
    (account, value)
}
fn wire() -> [u8; CAPSULE_BYTES] {
    let (mut account, value) = capsule();
    account.with_budget(|b| value.encode(b).unwrap().0)
}
fn decode(bytes: &[u8]) -> io::Result<Capsule> {
    let mut work = Work::new(1_000_000_000);
    let mut b = Budget::new(&mut work, 64 * 1024 * 1024);
    b.reserve_storage(bytes.len()).unwrap();
    Capsule::decode(bytes, &mut b).map(|(value, _)| value)
}

#[test]
fn inert_native_capsule_round_trip_binds_every_byte() {
    let bytes = wire();
    let decoded = decode(&bytes).unwrap();
    assert_eq!(decoded.parent, 2000);
    assert_eq!(decoded.nonce, [11; 32]);
    for offset in 0..CAPSULE_BYTES {
        let mut changed = bytes;
        changed[offset] ^= 1;
        assert!(decode(&changed).is_err(), "byte {offset}");
    }
    let mut extended = bytes.to_vec();
    extended.push(0);
    assert!(decode(&extended).is_err());
}

#[test]
fn native_capsule_resealed_invalid_binding_and_occurrences_reject() {
    let bytes = wire();
    for range in [
        16..20,
        24..56,
        56..88,
        88..120,
        120..152,
        BINDING_END..BINDING_END + 32,
        BINDING_END + 32..BINDING_END + 40,
        BINDING_END + 40..IDENTITY,
    ] {
        let mut changed = bytes;
        changed[range.clone()].fill(0);
        let identity = hash(&changed[..IDENTITY]);
        changed[IDENTITY..].copy_from_slice(&identity);
        assert!(decode(&changed).is_err(), "range {range:?}");
    }
    for parent in [1200_u32, 1201_u32, u32::MAX] {
        let mut changed = bytes;
        changed[16..20].copy_from_slice(&parent.to_le_bytes());
        let identity = hash(&changed[..IDENTITY]);
        changed[IDENTITY..].copy_from_slice(&identity);
        assert!(decode(&changed).is_err());
    }
}

#[test]
fn native_capsule_decode_exact_and_one_short_account_limits() {
    let bytes = wire();
    let floor = bytes.len() + 37;
    let mut work = Work::new(1_000_000_000);
    let mut b = Budget::new(&mut work, 64 * 1024 * 1024);
    b.reserve_storage(floor).unwrap();
    let (value, retained) = Capsule::decode(&bytes, &mut b).unwrap();
    assert_eq!(retained, value.retained_storage());
    assert_eq!(b.storage(), floor);
    let work_used = b.work();
    let peak = b.peak_storage();
    for (work_limit, storage_limit, succeeds) in [
        (work_used, peak, true),
        (work_used - 1, peak, false),
        (work_used, peak - 1, false),
    ] {
        let mut work = Work::new(work_limit);
        let mut b = Budget::new(&mut work, storage_limit);
        b.reserve_storage(floor).unwrap();
        assert_eq!(Capsule::decode(&bytes, &mut b).is_ok(), succeeds);
        assert!(b.storage() >= floor);
    }
}

#[test]
fn native_capsule_short_and_legacy_frames_never_decode() {
    let mut work = Work::new(1_000_000);
    let mut b = Budget::new(&mut work, 1_000_000);
    let bytes = [0; CAPSULE_BYTES];
    b.reserve_storage(bytes.len()).unwrap();
    for len in [0, 8, CAPSULE_BYTES - 1, CAPSULE_BYTES] {
        assert!(Capsule::decode(&bytes[..len], &mut b).is_err());
    }
    let mut legacy = bytes;
    legacy[..8].copy_from_slice(b"F3APCP1\0");
    assert!(Capsule::decode(&legacy, &mut b).is_err());
}

#[test]
fn native_capsule_resource_refusal_precedes_nested_decoding() {
    let mut work = Work::new(IO_WORK - 1);
    let mut b = Budget::new(&mut work, 1_000_000);
    b.reserve_storage(CAPSULE_BYTES).unwrap();
    let before = b.storage();
    assert!(Capsule::decode(&[0; CAPSULE_BYTES], &mut b).is_err());
    assert_eq!(b.storage(), before);
}
