//! Inert codec tests. Deterministic binding bytes exercise association, not
//! randomness, freshness, native readiness, peer authentication or admission.
use super::*;
use crate::{
    COMPILER_EXECUTION_ROOT_CONTROL_BYTES_V3 as N,
    CompilerExecutionClientProcessIdentityV1 as Client,
    CompilerExecutionExternalAnchorServiceIdentityV1 as Service,
    CompilerExecutionIssuerMeasurementV1 as Measurement,
};
use ed25519_dalek::SigningKey;
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceErrorV1 as Resource, CanonicalKernelIrWorkBudgetV1 as Work,
};
use sha2::{Digest, Sha256};
use std::fmt;

const WORK: usize = COMPILER_EXECUTION_ROOT_GATE_WORK_V3;
const SCRATCH: usize = COMPILER_EXECUTION_ROOT_GATE_STORAGE_V3;
const CODEC_WORK: usize = COMPILER_EXECUTION_ROOT_CONTROL_WORK_V3;
const CODEC_SCRATCH: usize = COMPILER_EXECUTION_ROOT_CONTROL_STORAGE_V3;
const LIMIT: usize = 4_000_000;
const WORK_LIMIT: usize = 1000 * WORK;
const EPOCH: [u8; 32] = [0x71; 32];
const GENERATION: [u8; 32] = [0x72; 32];
const KINDS: [Kind; 4] = [Kind::Reconcile, Kind::Observe, Kind::Validate, Kind::Retire];

fn policy(generation: u64, b: &mut Budget<'_>) -> Policy {
    let (policy, storage) = Policy::new(
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
    b.reserve_storage(storage.additional_storage()).unwrap();
    policy
}

fn manifest(policy: &Policy, pid: u32, b: &mut Budget<'_>) -> Manifest {
    let (manifest, storage) = Manifest::new(
        Client::new(pid, 5678, 9012).unwrap(),
        Service::new(6001, 7001).unwrap(),
        policy,
        b,
    )
    .unwrap();
    b.reserve_storage(storage.additional_storage()).unwrap();
    manifest
}

fn fixture(b: &mut Budget<'_>) -> (Policy, Manifest, Binding) {
    // Prepay the test's wire buffers and fixed/variant payloads independently.
    b.reserve_storage(37 + 3 * N).unwrap();
    let policy = policy(7, b);
    let manifest = manifest(&policy, 1234, b);
    let (binding, storage) = Binding::new(&policy, &manifest, EPOCH, GENERATION, b).unwrap();
    assert_eq!(storage.additional_storage(), binding.retained_storage());
    b.reserve_storage(storage.additional_storage()).unwrap();
    (policy, manifest, binding)
}

fn retain(result: Result<(Record, Storage)>, b: &mut Budget<'_>) -> Record {
    let (record, storage) = result.unwrap();
    assert_eq!(storage.additional_storage(), size_of::<(Record, Storage)>());
    assert_eq!(storage.additional_storage(), record.retained_storage());
    b.reserve_storage(storage.additional_storage()).unwrap();
    record
}

fn discard(record: Record, b: &mut Budget<'_>) {
    let charge = record.retained_storage();
    drop(record);
    b.release_storage(charge).unwrap();
}

fn checked<'work, T>(
    b: &mut Budget<'work>,
    work: usize,
    scratch: usize,
    operation: impl FnOnce(&mut Budget<'work>) -> Result<T>,
) -> Result<T> {
    let (prefix, floor, peak) = (b.work(), b.storage(), b.peak_storage());
    let history = (b.failed_work(), b.failed_storage());
    let ledger = b.work_ledger_identity_v1();
    let result = operation(b);
    assert_eq!((b.work(), b.storage()), (prefix + work, floor));
    assert_eq!(b.peak_storage(), peak.max(floor + scratch));
    assert_eq!((b.failed_work(), b.failed_storage()), history);
    assert!(b.work_ledger_identity_v1() == ledger);
    result
}

fn framing<T: fmt::Debug>(result: Result<T>, expected: &str) {
    assert!(
        matches!(&result, Err(Error::Framing(reason)) if *reason == expected),
        "{result:?}"
    );
}

// Independent full-wire oracle; no codec-private header or digest helpers.
fn reseal(bytes: &mut [u8; N]) {
    let mut hash = Sha256::new();
    hash.update(b"FE2O3/ROOT-ISSUER-CONTROL/V3\0");
    hash.update(&bytes[..4064]);
    bytes[4064..].copy_from_slice(&hash.finalize());
}

fn wire(
    policy: &Policy,
    manifest: &Manifest,
    kind: Kind,
    sequence: u64,
    payload: &[u8],
    join: Option<&[u8; 32]>,
) -> [u8; N] {
    let mut bytes = [0; N];
    bytes[..8].copy_from_slice(b"F2O3CRC3");
    bytes[8..10].copy_from_slice(&3u16.to_le_bytes());
    bytes[10..12].copy_from_slice(&(kind as u16).to_le_bytes());
    bytes[12..20].copy_from_slice(&4096u64.to_le_bytes());
    bytes[24..56].copy_from_slice(policy.identity().as_bytes());
    bytes[56..88].copy_from_slice(manifest.identity().as_bytes());
    bytes[88..120].copy_from_slice(&EPOCH);
    bytes[120..152].copy_from_slice(&GENERATION);
    bytes[152..160].copy_from_slice(&sequence.to_le_bytes());
    if let Some(join) = join {
        bytes[20] = 1;
        bytes[160..192].copy_from_slice(join);
    }
    bytes[192..196].copy_from_slice(&(payload.len() as u32).to_le_bytes());
    bytes[200..200 + payload.len()].copy_from_slice(payload);
    reseal(&mut bytes);
    bytes
}

#[test]
fn exact_gate_payloads_and_digest_join_roundtrip_with_full_unreserved_charges() {
    assert_eq!(WORK, 2 * CODEC_WORK);
    assert_eq!(SCRATCH, FRAME + CODEC_SCRATCH);
    let mut work = Work::new(WORK_LIMIT);
    let mut b = Budget::new(&mut work, LIMIT);
    let (policy, manifest, binding) = fixture(&mut b);
    let request = retain(
        checked(&mut b, WORK, SCRATCH, |b| {
            compiler_execution_root_gate_request_v3(&binding, b)
        }),
        &mut b,
    );
    let reply = retain(
        checked(&mut b, WORK, SCRATCH, |b| {
            compiler_execution_root_gate_reply_v3(&request, b)
        }),
        &mut b,
    );
    let expected_request = wire(
        &policy,
        &manifest,
        Kind::Reconcile,
        1,
        b"FE2O3/ROOT-GATE/V3\0",
        None,
    );
    let expected_reply = wire(
        &policy,
        &manifest,
        Kind::Reconcile,
        1,
        b"FE2O3/ISSUER-ADMITTED/V3\0",
        Some(request.identity()),
    );
    assert_eq!(request.canonical_bytes(), &expected_request);
    assert_eq!(reply.canonical_bytes(), &expected_reply);
    for (original, bytes) in [(&request, &expected_request), (&reply, &expected_reply)] {
        let decoded = retain(Record::decode(bytes, &mut b), &mut b);
        assert_eq!(&decoded, original);
        assert!(decoded.matches_binding(&binding, &mut b).unwrap());
        assert!(
            checked(&mut b, CODEC_WORK, CODEC_SCRATCH, |b| {
                decoded.matches_launch(&policy, &manifest, b)
            })
            .unwrap()
        );
        discard(decoded, &mut b);
    }
    checked(&mut b, WORK, SCRATCH, |b| {
        validate_compiler_execution_root_gate_request_v3(&request, &policy, &manifest, b)
    })
    .unwrap();
    checked(&mut b, WORK, SCRATCH, |b| {
        validate_compiler_execution_root_gate_reply_v3(&reply, &request, b)
    })
    .unwrap();
    // The same inert inputs produce the same request; no nonce is invented here.
    let again = retain(
        compiler_execution_root_gate_request_v3(&binding, &mut b),
        &mut b,
    );
    assert_eq!(again, request);
    discard(again, &mut b);
    discard(reply, &mut b);
    discard(request, &mut b);
}

fn payload_variants(payload: &[u8]) -> Vec<Vec<u8>> {
    let mut variants: Vec<_> = (0..payload.len()).map(|n| payload[..n].to_vec()).collect();
    let mut longer = payload.to_vec();
    longer.push(0);
    variants.push(longer);
    for offset in 0..payload.len() {
        let mut changed = payload.to_vec();
        changed[offset] ^= 1;
        variants.push(changed);
    }
    variants
}

#[test]
fn every_short_extra_or_changed_magic_is_rejected_even_with_a_matching_reply_digest() {
    let mut work = Work::new(WORK_LIMIT);
    let mut b = Budget::new(&mut work, LIMIT);
    let (policy, manifest, binding) = fixture(&mut b);
    let request = retain(
        compiler_execution_root_gate_request_v3(&binding, &mut b),
        &mut b,
    );
    for payload in payload_variants(REQUEST_PAYLOAD) {
        let bytes = wire(&policy, &manifest, Kind::Reconcile, 1, &payload, None);
        let wrong = retain(Record::decode(&bytes, &mut b), &mut b);
        framing(
            checked(&mut b, CODEC_WORK, FRAME, |b| {
                validate_compiler_execution_root_gate_request_v3(&wrong, &policy, &manifest, b)
            }),
            "root gate request",
        );
        framing(
            checked(&mut b, CODEC_WORK, FRAME, |b| {
                compiler_execution_root_gate_reply_v3(&wrong, b)
            }),
            "root gate request",
        );
        let reply = retain(Record::reply(&wrong, REPLY_PAYLOAD, &mut b), &mut b);
        assert!(reply.matches_reply(&wrong, &mut b).unwrap());
        framing(
            checked(&mut b, CODEC_WORK, FRAME, |b| {
                validate_compiler_execution_root_gate_reply_v3(&reply, &wrong, b)
            }),
            "root gate request",
        );
        discard(reply, &mut b);
        discard(wrong, &mut b);
    }
    for payload in payload_variants(REPLY_PAYLOAD) {
        let bytes = wire(
            &policy,
            &manifest,
            Kind::Reconcile,
            1,
            &payload,
            Some(request.identity()),
        );
        let wrong = retain(Record::decode(&bytes, &mut b), &mut b);
        assert!(wrong.matches_reply(&request, &mut b).unwrap());
        framing(
            checked(&mut b, CODEC_WORK, FRAME, |b| {
                validate_compiler_execution_root_gate_reply_v3(&wrong, &request, b)
            }),
            "root gate reply",
        );
        discard(wrong, &mut b);
    }
}

#[test]
fn full_wire_kind_sequence_and_direction_matrix_requires_the_exact_gate_exchange() {
    let mut work = Work::new(WORK_LIMIT);
    let mut b = Budget::new(&mut work, LIMIT);
    let (policy, manifest, binding) = fixture(&mut b);
    let request = retain(
        compiler_execution_root_gate_request_v3(&binding, &mut b),
        &mut b,
    );
    for kind in KINDS {
        for sequence in [0, 1, 2, u64::MAX] {
            for is_reply in [false, true] {
                for checking_reply in [false, true] {
                    let payload = if checking_reply {
                        REPLY_PAYLOAD
                    } else {
                        REQUEST_PAYLOAD
                    };
                    let bytes = wire(
                        &policy,
                        &manifest,
                        kind,
                        sequence,
                        payload,
                        is_reply.then(|| request.identity()),
                    );
                    let decoded = Record::decode(&bytes, &mut b);
                    if sequence == 0 {
                        framing(decoded, "root control sequence or reply join");
                        continue;
                    }
                    let record = retain(decoded, &mut b);
                    let valid =
                        kind == Kind::Reconcile && sequence == 1 && is_reply == checking_reply;
                    if checking_reply {
                        let result = checked(&mut b, WORK, SCRATCH, |b| {
                            validate_compiler_execution_root_gate_reply_v3(&record, &request, b)
                        });
                        if valid {
                            result.unwrap();
                        } else {
                            framing(result, "root gate reply");
                        }
                    } else {
                        let result = checked(
                            &mut b,
                            if valid { WORK } else { CODEC_WORK },
                            if valid { SCRATCH } else { FRAME },
                            |b| {
                                validate_compiler_execution_root_gate_request_v3(
                                    &record, &policy, &manifest, b,
                                )
                            },
                        );
                        if valid {
                            result.unwrap();
                        } else {
                            framing(result, "root gate request");
                            framing(
                                compiler_execution_root_gate_reply_v3(&record, &mut b),
                                "root gate request",
                            );
                            framing(
                                validate_compiler_execution_root_gate_reply_v3(
                                    &request, &record, &mut b,
                                ),
                                "root gate request",
                            );
                            if !is_reply {
                                let joined =
                                    retain(Record::reply(&record, REPLY_PAYLOAD, &mut b), &mut b);
                                framing(
                                    validate_compiler_execution_root_gate_reply_v3(
                                        &joined, &record, &mut b,
                                    ),
                                    "root gate request",
                                );
                                discard(joined, &mut b);
                            }
                        }
                    }
                    discard(record, &mut b);
                }
            }
        }
    }
}

#[test]
fn launch_matching_checks_the_actual_manifest_policy_even_after_rehashing() {
    let mut work = Work::new(WORK_LIMIT);
    let mut b = Budget::new(&mut work, LIMIT);
    let (policy, manifest, binding) = fixture(&mut b);
    let request = retain(
        compiler_execution_root_gate_request_v3(&binding, &mut b),
        &mut b,
    );
    let other_policy = self::policy(8, &mut b);
    let other_manifest = self::manifest(&other_policy, 1234, &mut b);
    let moved_manifest = self::manifest(&policy, 1235, &mut b);
    for (policy, manifest) in [
        (&other_policy, &manifest),
        (&policy, &other_manifest),
        (&other_policy, &other_manifest),
        (&policy, &moved_manifest),
    ] {
        assert!(!request.matches_launch(policy, manifest, &mut b).unwrap());
        framing(
            checked(&mut b, WORK, SCRATCH, |b| {
                validate_compiler_execution_root_gate_request_v3(&request, policy, manifest, b)
            }),
            "root gate launch association",
        );
    }
    for offset in [24, 56] {
        let mut bytes = *request.canonical_bytes();
        bytes[offset] ^= 1;
        reseal(&mut bytes);
        let changed = retain(Record::decode(&bytes, &mut b), &mut b);
        assert!(!changed.matches_launch(&policy, &manifest, &mut b).unwrap());
        framing(
            validate_compiler_execution_root_gate_request_v3(&changed, &policy, &manifest, &mut b),
            "root gate launch association",
        );
        discard(changed, &mut b);
    }
    // Both wire identities match the supplied objects, but those objects do not
    // match each other. Comparing only the record's two hashes would accept this.
    let mut bytes = *request.canonical_bytes();
    bytes[24..56].copy_from_slice(other_policy.identity().as_bytes());
    reseal(&mut bytes);
    let changed = retain(Record::decode(&bytes, &mut b), &mut b);
    assert!(
        !checked(&mut b, CODEC_WORK, CODEC_SCRATCH, |b| {
            changed.matches_launch(&other_policy, &manifest, b)
        })
        .unwrap()
    );
    framing(
        validate_compiler_execution_root_gate_request_v3(
            &changed,
            &other_policy,
            &manifest,
            &mut b,
        ),
        "root gate launch association",
    );
}

#[test]
fn reply_association_and_exact_original_digest_are_required_but_do_not_prove_freshness() {
    let mut work = Work::new(WORK_LIMIT);
    let mut b = Budget::new(&mut work, LIMIT);
    let (policy, manifest, binding) = fixture(&mut b);
    let request = retain(
        compiler_execution_root_gate_request_v3(&binding, &mut b),
        &mut b,
    );
    let reply = retain(
        compiler_execution_root_gate_reply_v3(&request, &mut b),
        &mut b,
    );
    for offset in [24, 56, 88, 120, 160] {
        let mut bytes = *reply.canonical_bytes();
        bytes[offset] ^= 1;
        reseal(&mut bytes);
        let changed = retain(Record::decode(&bytes, &mut b), &mut b);
        framing(
            checked(&mut b, WORK, SCRATCH, |b| {
                validate_compiler_execution_root_gate_reply_v3(&changed, &request, b)
            }),
            "root gate reply",
        );
        discard(changed, &mut b);
    }
    for offset in [88, 120] {
        let mut bytes = *request.canonical_bytes();
        bytes[offset] ^= 1;
        reseal(&mut bytes);
        let changed = retain(Record::decode(&bytes, &mut b), &mut b);
        assert_ne!(changed.identity(), request.identity());
        assert!(!changed.matches_binding(&binding, &mut b).unwrap());
        // Launch matching intentionally cannot certify an independently held epoch/generation.
        validate_compiler_execution_root_gate_request_v3(&changed, &policy, &manifest, &mut b)
            .unwrap();
        framing(
            validate_compiler_execution_root_gate_reply_v3(&reply, &changed, &mut b),
            "root gate reply",
        );
        let joined = retain(
            compiler_execution_root_gate_reply_v3(&changed, &mut b),
            &mut b,
        );
        validate_compiler_execution_root_gate_reply_v3(&joined, &changed, &mut b).unwrap();
        framing(
            validate_compiler_execution_root_gate_reply_v3(&joined, &request, &mut b),
            "root gate reply",
        );
        discard(joined, &mut b);
        discard(changed, &mut b);
    }
    // Correct association with a different original payload still has the wrong digest.
    let other = retain(
        Record::request(&binding, 1, Kind::Reconcile, b"different", &mut b),
        &mut b,
    );
    let mut bytes = *reply.canonical_bytes();
    bytes[160..192].copy_from_slice(other.identity());
    reseal(&mut bytes);
    let redirected = retain(Record::decode(&bytes, &mut b), &mut b);
    assert!(redirected.matches_reply(&other, &mut b).unwrap());
    framing(
        validate_compiler_execution_root_gate_reply_v3(&redirected, &request, &mut b),
        "root gate reply",
    );
    framing(
        validate_compiler_execution_root_gate_reply_v3(&redirected, &other, &mut b),
        "root gate request",
    );
}

#[derive(Clone, Copy, Debug)]
enum Quota {
    Exact,
    ExactInput,
    Entry,
    OuterWork,
    Input,
    OuterScratch,
    NestedEntry,
    NestedWork,
    NestedScratch,
}

#[test]
fn all_helpers_and_launch_matching_preserve_exact_and_short_nested_resource_history() {
    for operation in 0..5 {
        for quota in [
            Quota::Exact,
            Quota::ExactInput,
            Quota::Entry,
            Quota::OuterWork,
            Quota::Input,
            Quota::OuterScratch,
            Quota::NestedEntry,
            Quota::NestedWork,
            Quota::NestedScratch,
        ] {
            let nested = operation != 4;
            if !nested
                && matches!(
                    quota,
                    Quota::NestedEntry | Quota::NestedWork | Quota::NestedScratch
                )
            {
                continue;
            }
            let work_quote = if nested { WORK } else { CODEC_WORK };
            let scratch = if nested { SCRATCH } else { CODEC_SCRATCH };
            let frame = if nested { FRAME } else { CODEC_SCRATCH };
            let mut work = Work::new(WORK_LIMIT);
            let mut b = Budget::new(&mut work, LIMIT);
            assert!(b.charge_work(WORK_LIMIT + 37).is_err());
            assert!(b.reserve_storage(LIMIT + 53).is_err());
            let (policy, manifest, binding) = fixture(&mut b);
            let request = retain(
                compiler_execution_root_gate_request_v3(&binding, &mut b),
                &mut b,
            );
            let reply = retain(
                compiler_execution_root_gate_reply_v3(&request, &mut b),
                &mut b,
            );
            let required = match operation {
                0 => binding.retained_storage(),
                1 | 4 => {
                    request.retained_storage()
                        + policy.retained_storage()
                        + manifest.retained_storage()
                }
                2 => request.retained_storage(),
                _ => request.retained_storage() + reply.retained_storage(),
            };
            let floor = match quota {
                Quota::ExactInput => required,
                Quota::Input => required - 1,
                Quota::OuterScratch => LIMIT - frame + 1,
                Quota::NestedScratch => LIMIT - scratch + 1,
                _ => LIMIT - scratch,
            };
            // Deliberately replace the test's aggregate reservation to probe the
            // operation's exact borrowed-input floor, including one byte short.
            if floor < b.storage() {
                b.release_storage(b.storage() - floor).unwrap();
            } else {
                b.reserve_storage(floor - b.storage()).unwrap();
            }
            let remaining = match quota {
                Quota::Entry => resources::ENTRY_WORK - 1,
                Quota::OuterWork => CODEC_WORK - 1,
                Quota::NestedEntry => CODEC_WORK + resources::ENTRY_WORK - 1,
                Quota::NestedWork => WORK - 1,
                _ => work_quote,
            };
            b.charge_work(WORK_LIMIT - remaining - b.work()).unwrap();
            let (prefix, peak) = (b.work(), b.peak_storage());
            let ledger = b.work_ledger_identity_v1();
            let full_charge = |(record, storage): (Record, Storage)| {
                assert_eq!(storage.additional_storage(), record.retained_storage());
                assert_eq!(storage.additional_storage(), size_of::<(Record, Storage)>());
            };
            let result = match operation {
                0 => compiler_execution_root_gate_request_v3(&binding, &mut b).map(full_charge),
                1 => validate_compiler_execution_root_gate_request_v3(
                    &request, &policy, &manifest, &mut b,
                ),
                2 => compiler_execution_root_gate_reply_v3(&request, &mut b).map(full_charge),
                3 => validate_compiler_execution_root_gate_reply_v3(&reply, &request, &mut b),
                _ => request
                    .matches_launch(&policy, &manifest, &mut b)
                    .map(|matched| assert!(matched)),
            };
            let spent = match quota {
                Quota::Entry => 0,
                Quota::OuterWork | Quota::Input => resources::ENTRY_WORK,
                Quota::OuterScratch | Quota::NestedEntry => CODEC_WORK,
                Quota::NestedWork => CODEC_WORK + resources::ENTRY_WORK,
                _ => work_quote,
            };
            let temporary = match quota {
                Quota::Exact | Quota::ExactInput => scratch,
                Quota::NestedEntry | Quota::NestedWork | Quota::NestedScratch => frame,
                _ => 0,
            };
            assert_eq!(
                (b.work(), b.storage()),
                (prefix + spent, floor),
                "{operation}/{quota:?}"
            );
            assert_eq!(
                b.peak_storage(),
                peak.max(floor + temporary),
                "{operation}/{quota:?}"
            );
            assert_eq!(b.failed_work(), Some(WORK_LIMIT + 37));
            assert_eq!(b.failed_storage(), Some(LIMIT + 53));
            assert!(b.work_ledger_identity_v1() == ledger);
            match quota {
                Quota::Exact | Quota::ExactInput => result.unwrap(),
                Quota::Input => {
                    assert!(matches!(result, Err(Error::Resource(Resource::Accounting))))
                }
                Quota::OuterScratch | Quota::NestedScratch => {
                    assert!(matches!(result, Err(Error::Resource(Resource::Storage(e)))
                        if e.actual() == LIMIT + 1 && e.limit() == LIMIT));
                }
                _ => {
                    let attempted = prefix
                        + match quota {
                            Quota::Entry => resources::ENTRY_WORK,
                            Quota::OuterWork => CODEC_WORK,
                            Quota::NestedEntry => CODEC_WORK + resources::ENTRY_WORK,
                            _ => work_quote,
                        };
                    assert!(matches!(result, Err(Error::Resource(Resource::Work(e)))
                        if e.actual() == attempted && e.limit() == WORK_LIMIT));
                }
            }
        }
    }
}
