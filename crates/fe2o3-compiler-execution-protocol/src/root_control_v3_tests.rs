//! Inert wire tests only: no authentication, admission, or retirement credit.
use super::*;
use crate::{
    CompilerExecutionClientProcessIdentityV1 as Client,
    CompilerExecutionExternalAnchorServiceIdentityV1 as Service,
    CompilerExecutionIssuerMeasurementV1 as Measurement,
};
use ed25519_dalek::SigningKey;
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;

const WORK: usize = COMPILER_EXECUTION_ROOT_CONTROL_WORK_V3;
const SCRATCH: usize = COMPILER_EXECUTION_ROOT_CONTROL_STORAGE_V3;
const MAX_PAYLOAD: usize = COMPILER_EXECUTION_ROOT_CONTROL_PAYLOAD_BYTES_V3;
const LIMIT: usize = 2_000_000;
const TOTAL_WORK: usize = 30_000_000;
const EPOCH: [u8; 32] = [0x71; 32];
const GENERATION: [u8; 32] = [0x72; 32];
const KINDS: [Kind; 4] = [Kind::Reconcile, Kind::Observe, Kind::Validate, Kind::Retire];

fn retain<T>(result: Result<(T, Storage)>, b: &mut Budget<'_>) -> T {
    let (owner, charge) = result.unwrap();
    b.reserve_storage(charge.additional_storage()).unwrap();
    owner
}

// The same real V3 constructors and measurement/key seeds as launch fixtures.
fn policy(generation: u64, b: &mut Budget<'_>) -> Policy {
    let (p, charge) = Policy::new(
        generation,
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

fn manifest(p: &Policy, pid: u32, b: &mut Budget<'_>) -> Manifest {
    let (m, charge) = Manifest::new(
        Client::new(pid, 5678, 9012).unwrap(),
        Service::new(6001, 7001).unwrap(),
        p,
        b,
    )
    .unwrap();
    b.reserve_storage(charge.additional_storage()).unwrap();
    m
}

fn fixture(b: &mut Budget<'_>) -> (Policy, Manifest, Binding) {
    let p = policy(7, b);
    let m = manifest(&p, 1234, b);
    let floor = b.storage();
    let (binding, charge) = Binding::new(&p, &m, EPOCH, GENERATION, b).unwrap();
    assert_eq!(b.storage(), floor);
    assert_eq!(charge.additional_storage(), binding.retained_storage());
    b.reserve_storage(charge.additional_storage()).unwrap();
    (p, m, binding)
}

// Independent wire oracle: do not use the codec's header/digest/finish helpers.
fn reseal(bytes: &mut [u8]) {
    let mut hash = Sha256::new();
    hash.update(b"FE2O3/ROOT-ISSUER-CONTROL/V3\0");
    hash.update(&bytes[..4064]);
    bytes[4064..].copy_from_slice(&hash.finalize());
}

fn wire(
    p: &Policy,
    m: &Manifest,
    kind: Kind,
    sequence: u64,
    payload: &[u8],
    request_hash: Option<&[u8; 32]>,
) -> [u8; 4096] {
    let mut bytes = [0; 4096];
    bytes[..8].copy_from_slice(b"F2O3CRC3");
    bytes[8..10].copy_from_slice(&3u16.to_le_bytes());
    bytes[10..12].copy_from_slice(&(kind as u16).to_le_bytes());
    bytes[12..20].copy_from_slice(&4096u64.to_le_bytes());
    bytes[24..56].copy_from_slice(p.identity().as_bytes());
    bytes[56..88].copy_from_slice(m.identity().as_bytes());
    bytes[88..120].copy_from_slice(&EPOCH);
    bytes[120..152].copy_from_slice(&GENERATION);
    bytes[152..160].copy_from_slice(&sequence.to_le_bytes());
    if let Some(hash) = request_hash {
        bytes[20] = 1;
        bytes[160..192].copy_from_slice(hash);
    }
    bytes[192..196].copy_from_slice(&(payload.len() as u32).to_le_bytes());
    bytes[200..200 + payload.len()].copy_from_slice(payload);
    reseal(&mut bytes);
    bytes
}

fn framing<T: fmt::Debug>(result: Result<T>, expected: &str) {
    assert!(
        matches!(&result, Err(Error::Framing(reason)) if *reason == expected),
        "expected {expected}, got {result:?}"
    );
}

fn decode_error(bytes: &[u8], expected: &str) {
    let mut work = Work::new(WORK);
    let floor = if bytes.len() == 4096 { 4096 } else { 0 };
    let mut b = Budget::new(&mut work, floor + SCRATCH);
    b.reserve_storage(floor).unwrap();
    framing(Record::decode(bytes, &mut b), expected);
    assert_eq!(
        (b.storage(), b.work(), b.peak_storage()),
        (floor, WORK, floor + SCRATCH)
    );
}

#[test]
fn every_kind_and_direction_roundtrips_empty_carriage_sized_and_max_opaque_payloads() {
    assert_eq!(N, 4096);
    assert_eq!(MAX_PAYLOAD, 3864);
    assert_eq!(crate::COMPILER_EXECUTION_RECEIPT_CARRIAGE_BYTES_V3, 2090);
    let mut work = Work::new(TOTAL_WORK);
    let mut b = Budget::new(&mut work, LIMIT);
    let (p, m, binding) = fixture(&mut b);
    for (tag, kind) in (1u16..).zip(KINDS) {
        assert_eq!(kind as u16, tag);
        for length in [0, 2090, MAX_PAYLOAD] {
            // Deliberately invalid as nested carriage; only byte transport is tested.
            let payload = vec![0xa5; length];
            b.reserve_storage(length).unwrap();
            let floor = b.storage();
            let sequence = if length == 0 { 1 } else { u64::MAX };
            let (request, charge) =
                Record::request(&binding, sequence, kind, &payload, &mut b).unwrap();
            assert_eq!(b.storage(), floor);
            assert_eq!(charge.additional_storage(), request.retained_storage());
            b.reserve_storage(charge.additional_storage()).unwrap();
            assert_eq!(
                request.canonical_bytes(),
                &wire(&p, &m, kind, sequence, &payload, None)
            );
            let before_reply = b.storage();
            let (reply, charge) = Record::reply(&request, &payload, &mut b).unwrap();
            assert_eq!(b.storage(), before_reply);
            assert_eq!(charge.additional_storage(), reply.retained_storage());
            b.reserve_storage(charge.additional_storage()).unwrap();
            assert_eq!(
                reply.canonical_bytes(),
                &wire(&p, &m, kind, sequence, &payload, Some(request.identity()))
            );
            for (record, is_reply) in [(&request, false), (&reply, true)] {
                let entry = b.storage();
                let (decoded, charge) = Record::decode(record.canonical_bytes(), &mut b).unwrap();
                assert_eq!(b.storage(), entry);
                assert_eq!(charge.additional_storage(), decoded.retained_storage());
                b.reserve_storage(charge.additional_storage()).unwrap();
                assert_eq!(record, &decoded);
                assert_ne!(
                    record.canonical_bytes().as_ptr(),
                    decoded.canonical_bytes().as_ptr()
                );
                assert_eq!(
                    (decoded.kind(), decoded.sequence(), decoded.is_reply()),
                    (kind, sequence, is_reply)
                );
                assert_eq!(decoded.payload(), payload);
                assert!(decoded.matches_binding(&binding, &mut b).unwrap());
                assert_eq!(decoded.matches_reply(&request, &mut b).unwrap(), is_reply);
                let retained = decoded.retained_storage();
                drop(decoded);
                b.release_storage(retained).unwrap();
            }
            let retained = request.retained_storage() + reply.retained_storage();
            drop((request, reply, payload));
            b.release_storage(retained + length).unwrap();
            assert_eq!(b.storage(), floor - length);
        }
    }
}

#[test]
fn malformed_headers_roles_lengths_associations_and_joins_fail_even_when_resealed() {
    let mut work = Work::new(TOTAL_WORK);
    let mut b = Budget::new(&mut work, LIMIT);
    let (p, m, _) = fixture(&mut b);
    for reply in [false, true] {
        let good = wire(&p, &m, Kind::Observe, 9, &[], reply.then_some(&[0x81; 32]));
        for offset in (0..10).chain(12..20) {
            let mut bytes = good;
            bytes[offset] ^= 0xff;
            reseal(&mut bytes);
            decode_error(&bytes, "root control header");
        }
        for version in [0u16, 1, 2, 4, u16::MAX] {
            let mut bytes = good;
            bytes[8..10].copy_from_slice(&version.to_le_bytes());
            reseal(&mut bytes);
            decode_error(&bytes, "root control header");
        }
        for kind in [0u16, 5, 0x0101, u16::MAX] {
            let mut bytes = good;
            bytes[10..12].copy_from_slice(&kind.to_le_bytes());
            reseal(&mut bytes);
            decode_error(&bytes, "root control kind");
        }
        for length in [3865u32, 4096, u32::MAX] {
            let mut bytes = good;
            bytes[192..196].copy_from_slice(&length.to_le_bytes());
            reseal(&mut bytes);
            decode_error(&bytes, "root control payload length");
        }
        for role in 2u8..=u8::MAX {
            let mut bytes = good;
            bytes[20] = role;
            reseal(&mut bytes);
            decode_error(&bytes, "root control reserved bytes");
        }
        for offset in (21..24).chain(196..200).chain([200, 2090, 4063]) {
            let mut bytes = good;
            bytes[offset] = 1;
            reseal(&mut bytes);
            decode_error(&bytes, "root control reserved bytes");
        }
        for offset in [24, 56, 88, 120] {
            let mut bytes = good;
            bytes[offset..offset + 32].fill(0);
            reseal(&mut bytes);
            decode_error(&bytes, "zero root control association");
        }
        for zero_sequence in [false, true] {
            let mut bytes = good;
            if zero_sequence {
                bytes[152..160].fill(0);
            } else {
                bytes[160..192].fill(if reply { 0 } else { 1 });
            }
            reseal(&mut bytes);
            decode_error(&bytes, "root control sequence or reply join");
        }
        let mut bytes = good;
        bytes[20] ^= 1;
        reseal(&mut bytes);
        decode_error(&bytes, "root control sequence or reply join");
    }
}

#[test]
fn declared_payload_boundary_requires_zero_padding_and_digest_covers_every_region() {
    let mut work = Work::new(TOTAL_WORK);
    let mut b = Budget::new(&mut work, LIMIT);
    let (p, m, _) = fixture(&mut b);
    for reply in [false, true] {
        let good = wire(
            &p,
            &m,
            Kind::Validate,
            1,
            &[0xa5; 17],
            reply.then_some(&[0x81; 32]),
        );
        for offset in [10, 24, 55, 56, 87, 88, 119, 120, 151, 152, 159, 200, 216] {
            let mut bytes = good;
            bytes[offset] ^= if offset == 152 { 2 } else { 1 };
            decode_error(&bytes, "root control identity");
        }
        for offset in 4064..4096 {
            let mut bytes = good;
            bytes[offset] ^= 1;
            decode_error(&bytes, "root control identity");
        }
        for domain in [b"".as_slice(), b"FE2O3/ROOT-ISSUER-CONTROL/V2\0"] {
            let mut bytes = good;
            let mut hash = Sha256::new();
            hash.update(domain);
            hash.update(&bytes[..4064]);
            bytes[4064..].copy_from_slice(&hash.finalize());
            decode_error(&bytes, "root control identity");
        }
        let mut bytes = good;
        bytes[20] ^= 1;
        bytes[160..192].fill(if reply { 0 } else { 0x81 });
        decode_error(&bytes, "root control identity");
        if reply {
            for offset in [160, 191] {
                let mut bytes = good;
                bytes[offset] ^= 1;
                decode_error(&bytes, "root control identity");
            }
        }
        for offset in [217, 4063] {
            let mut bytes = good;
            bytes[offset] = 1;
            reseal(&mut bytes);
            decode_error(&bytes, "root control reserved bytes");
        }
        let mut bytes = good;
        bytes[192..196].copy_from_slice(&16u32.to_le_bytes());
        reseal(&mut bytes);
        decode_error(&bytes, "root control reserved bytes");
        // A zero byte newly declared as payload is structurally valid but must be hashed.
        let mut bytes = good;
        bytes[192..196].copy_from_slice(&18u32.to_le_bytes());
        decode_error(&bytes, "root control identity");
    }
}

#[test]
fn bindings_reject_mismatched_policies_and_zero_epoch_or_generation() {
    let mut work = Work::new(TOTAL_WORK);
    let mut b = Budget::new(&mut work, LIMIT);
    let (p, m, _) = fixture(&mut b);
    let other = policy(8, &mut b);
    let floor = b.storage();
    framing(
        Binding::new(&other, &m, EPOCH, GENERATION, &mut b),
        "root control manifest policy",
    );
    for (epoch, generation) in [([0; 32], GENERATION), (EPOCH, [0; 32])] {
        framing(
            Binding::new(&p, &m, epoch, generation, &mut b),
            "zero root control association",
        );
    }
    assert_eq!(b.storage(), floor);
}

#[test]
fn replies_join_the_exact_request_and_cannot_be_used_as_requests() {
    let mut work = Work::new(TOTAL_WORK);
    let mut b = Budget::new(&mut work, LIMIT);
    let (_, _, binding) = fixture(&mut b);
    b.reserve_storage(2).unwrap();
    for kind in KINDS {
        let first = retain(Record::request(&binding, 7, kind, b"a", &mut b), &mut b);
        let other = retain(Record::request(&binding, 7, kind, b"b", &mut b), &mut b);
        let reply = retain(Record::reply(&first, &[], &mut b), &mut b);
        assert_ne!(first.identity(), other.identity());
        assert!(reply.matches_reply(&first, &mut b).unwrap());
        assert!(!reply.matches_reply(&other, &mut b).unwrap());
        assert!(!first.matches_reply(&reply, &mut b).unwrap());
        assert!(!first.matches_reply(&first, &mut b).unwrap());
        assert!(!reply.matches_reply(&reply, &mut b).unwrap());
        let floor = b.storage();
        framing(
            Record::reply(&reply, &[], &mut b),
            "reply requires a root control request",
        );
        framing(
            Record::request(&binding, 0, kind, &[], &mut b),
            "root control sequence or payload length",
        );
        assert_eq!(b.storage(), floor);
        let retained =
            first.retained_storage() + other.retained_storage() + reply.retained_storage();
        drop((first, other, reply));
        b.release_storage(retained).unwrap();
    }
}

#[test]
fn hostile_rehashed_substitutions_decode_but_require_independent_binding_and_pending_request() {
    let mut work = Work::new(TOTAL_WORK);
    let mut b = Budget::new(&mut work, LIMIT);
    let (p, m, binding) = fixture(&mut b);
    b.reserve_storage(1 + 4096).unwrap();
    let request = retain(
        Record::request(&binding, 7, Kind::Retire, b"a", &mut b),
        &mut b,
    );
    let reply = retain(Record::reply(&request, b"a", &mut b), &mut b);
    let other_p = policy(8, &mut b);
    let other_m = manifest(&other_p, 1234, &mut b);
    let moved_m = manifest(&p, 1235, &mut b);
    for (p, m, epoch, generation) in [
        (&other_p, &other_m, EPOCH, GENERATION),
        (&p, &moved_m, EPOCH, GENERATION),
        (&p, &m, [0x73; 32], GENERATION),
        (&p, &m, EPOCH, [0x74; 32]),
    ] {
        let other = retain(Binding::new(p, m, epoch, generation, &mut b), &mut b);
        assert!(!request.matches_binding(&other, &mut b).unwrap());
        assert!(!reply.matches_binding(&other, &mut b).unwrap());
        let retained = other.retained_storage();
        drop(other);
        b.release_storage(retained).unwrap();
    }
    // Nonzero claims are opaque on decode; only independently retained context binds them.
    for original in [&request, &reply] {
        for offset in [24, 56, 88, 120, 152, 10, 200] {
            let mut bytes = *original.canonical_bytes();
            bytes[offset] ^= if offset == 10 { 5 } else { 1 };
            reseal(&mut bytes);
            let changed = retain(Record::decode(&bytes, &mut b), &mut b);
            assert_ne!(changed.identity(), original.identity());
            assert_eq!(
                changed.matches_binding(&binding, &mut b).unwrap(),
                offset >= 152 || offset == 10
            );
            assert_eq!(
                changed.matches_reply(&request, &mut b).unwrap(),
                original.is_reply() && offset == 200
            );
            assert!(!reply.matches_reply(&changed, &mut b).unwrap());
            let retained = changed.retained_storage();
            drop(changed);
            b.release_storage(retained).unwrap();
        }
    }
    let other = retain(
        Record::request(&binding, 7, Kind::Retire, b"b", &mut b),
        &mut b,
    );
    let mut bytes = *reply.canonical_bytes();
    bytes[160..192].copy_from_slice(other.identity());
    reseal(&mut bytes);
    let redirected = retain(Record::decode(&bytes, &mut b), &mut b);
    assert!(!redirected.matches_reply(&request, &mut b).unwrap());
    assert!(redirected.matches_reply(&other, &mut b).unwrap());
    // Even a rewritten Retire reply is just framing: these hashes authenticate no sender.
}

#[test]
fn every_operation_requires_exact_work_scratch_and_prepaid_inputs() {
    for operation in 0..6 {
        for shortage in 0..5 {
            let mut work = Work::new(TOTAL_WORK);
            let mut b = Budget::new(&mut work, LIMIT);
            assert!(b.charge_work(TOTAL_WORK + 1).is_err());
            assert!(b.reserve_storage(LIMIT + 1).is_err());
            let (p, m, binding) = fixture(&mut b);
            let payload = [0xa5; 7];
            b.reserve_storage(payload.len() + 4096).unwrap();
            let request = retain(
                Record::request(&binding, 1, Kind::Observe, &payload, &mut b),
                &mut b,
            );
            let reply = retain(Record::reply(&request, &payload, &mut b), &mut b);
            let bytes = *request.canonical_bytes();
            let input_floor = match operation {
                0 => p.retained_storage() + m.retained_storage(),
                1 => binding.retained_storage() + payload.len(),
                2 => request.retained_storage() + payload.len(),
                3 => bytes.len(),
                4 => request.retained_storage() + binding.retained_storage(),
                _ => reply.retained_storage() + request.retained_storage(),
            };
            if shortage == 3 {
                // Deliberate accounting violation probes the borrowed-owner preflight.
                b.release_storage(b.storage() - (input_floor - 1)).unwrap();
            } else {
                let padding = LIMIT - SCRATCH + usize::from(shortage == 2) - b.storage();
                b.reserve_storage(padding).unwrap();
            }
            let remaining = match shortage {
                1 => WORK - 1,
                4 => resources::ENTRY_WORK - 1,
                _ => WORK,
            };
            b.charge_work(TOTAL_WORK - remaining - b.work()).unwrap();
            let floor = b.storage();
            let prefix = b.work();
            let ledger = b.work_ledger_identity_v1();
            let result = match operation {
                0 => Binding::new(&p, &m, EPOCH, GENERATION, &mut b)
                    .map(|(v, s)| assert_eq!(s.additional_storage(), v.retained_storage())),
                1 => Record::request(&binding, 1, Kind::Observe, &payload, &mut b)
                    .map(|(v, s)| assert_eq!(s.additional_storage(), v.retained_storage())),
                2 => Record::reply(&request, &payload, &mut b)
                    .map(|(v, s)| assert_eq!(s.additional_storage(), v.retained_storage())),
                3 => Record::decode(&bytes, &mut b)
                    .map(|(v, s)| assert_eq!(s.additional_storage(), v.retained_storage())),
                4 => request
                    .matches_binding(&binding, &mut b)
                    .map(|v| assert!(v)),
                _ => reply.matches_reply(&request, &mut b).map(|v| assert!(v)),
            };
            assert_eq!(b.storage(), floor);
            assert!(b.work_ledger_identity_v1() == ledger);
            assert_eq!(b.failed_work(), Some(TOTAL_WORK + 1));
            assert_eq!(b.failed_storage(), Some(LIMIT + 1));
            match shortage {
                0 => {
                    result.unwrap();
                    assert_eq!(b.work(), prefix + WORK);
                    assert_eq!(b.peak_storage(), LIMIT);
                }
                1 | 4 => {
                    assert!(matches!(result, Err(Error::Resource(Resource::Work(_)))));
                    assert_eq!(
                        b.work(),
                        prefix
                            + if shortage == 1 {
                                resources::ENTRY_WORK
                            } else {
                                0
                            }
                    );
                }
                2 => {
                    assert!(matches!(result, Err(Error::Resource(Resource::Storage(_)))));
                    assert_eq!(b.work(), prefix + WORK);
                }
                _ => {
                    assert!(matches!(result, Err(Error::Resource(Resource::Accounting))));
                    assert_eq!(b.work(), prefix + resources::ENTRY_WORK);
                }
            }
        }
    }
}

#[test]
fn retained_records_and_failed_decodes_share_cumulative_storage_and_work() {
    let mut fixture_work = Work::new(TOTAL_WORK);
    let mut fixture_budget = Budget::new(&mut fixture_work, LIMIT);
    let (p, m, _) = fixture(&mut fixture_budget);
    let bytes = wire(&p, &m, Kind::Observe, 1, &[], None);
    let floor = bytes.len() + 37;
    let limit = floor + SCRATCH + RETAINED - 1;
    let mut work = Work::new(3 * WORK);
    let mut b = Budget::new(&mut work, limit);
    b.reserve_storage(floor).unwrap();
    let ledger = b.work_ledger_identity_v1();
    let record = retain(Record::decode(&bytes, &mut b), &mut b);
    assert_eq!(b.storage(), floor + record.retained_storage());
    assert!(matches!(
        Record::decode(&bytes, &mut b),
        Err(Error::Resource(Resource::Storage(_)))
    ));
    assert_eq!(b.work(), 2 * WORK);
    assert_eq!(b.storage(), floor + record.retained_storage());
    assert_eq!(b.failed_storage(), Some(limit + 1));
    let retained = record.retained_storage();
    drop(record);
    b.release_storage(retained).unwrap();
    let mut bad = bytes;
    bad[4064] ^= 1;
    framing(Record::decode(&bad, &mut b), "root control identity");
    assert_eq!(b.work(), 3 * WORK);
    for _ in 0..2 {
        assert!(matches!(
            Record::decode(&bytes, &mut b),
            Err(Error::Resource(Resource::Work(_)))
        ));
        assert_eq!(b.work(), 3 * WORK);
        assert_eq!(b.storage(), floor);
        assert_eq!(b.failed_work(), Some(3 * WORK + resources::ENTRY_WORK));
        assert_eq!(b.failed_storage(), Some(limit + 1));
        assert!(b.work_ledger_identity_v1() == ledger);
    }
    assert_eq!(b.peak_storage(), floor + SCRATCH);
}

#[test]
fn nonexact_and_oversized_inputs_fail_before_header_or_payload_contents() {
    for length in [0, 1, 199, 4095, 4097, 1_000_000] {
        for fill in [0, 0xff] {
            // No input reservation: nonexact lengths must be rejected before inspection.
            decode_error(&vec![fill; length], "root control length");
        }
    }
    let mut work = Work::new(TOTAL_WORK);
    let mut b = Budget::new(&mut work, LIMIT);
    let (_, _, binding) = fixture(&mut b);
    let request = retain(
        Record::request(&binding, 1, Kind::Reconcile, &[], &mut b),
        &mut b,
    );
    for length in [MAX_PAYLOAD + 1, 1_000_000] {
        for fill in [0, 0xff] {
            let payload = vec![fill; length];
            let floor = b.storage();
            let prefix = b.work();
            framing(
                Record::request(&binding, 1, Kind::Reconcile, &payload, &mut b),
                "root control sequence or payload length",
            );
            framing(
                Record::reply(&request, &payload, &mut b),
                "root control sequence or payload length",
            );
            assert_eq!(b.storage(), floor);
            assert_eq!(b.work(), prefix + 2 * WORK);
        }
    }
}
