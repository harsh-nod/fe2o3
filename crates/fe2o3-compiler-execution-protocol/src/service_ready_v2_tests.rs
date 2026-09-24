use super::{
    COMPILER_EXECUTION_SERVICE_READY_BYTES_V2 as BYTES,
    COMPILER_EXECUTION_SERVICE_READY_STORAGE_V2 as STORAGE,
    COMPILER_EXECUTION_SERVICE_READY_WORK_V2 as WORK,
    CompilerExecutionServiceReadyErrorV2 as ReadyError, CompilerExecutionServiceReadyV2 as Ready,
    *,
};
use crate::{
    COMPILER_EXECUTION_ISSUER_POLICY_WORK_V2 as POLICY_WORK,
    COMPILER_EXECUTION_SERVICE_LAUNCH_MANIFEST_WORK_V2 as MANIFEST_WORK,
    CompilerExecutionClientProcessIdentityV1 as Client,
    CompilerExecutionExternalAnchorServiceIdentityV1 as Service,
    CompilerExecutionIssuerMeasurementV1 as Measurement,
    CompilerExecutionIssuerPolicyV1 as LegacyPolicy,
    CompilerExecutionServiceLaunchManifestV1 as LegacyManifest,
    CompilerExecutionServiceReadyV1 as LegacyReady,
};
use ed25519_dalek::SigningKey;
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;
use sha2::{Digest, Sha256};
use std::panic::{AssertUnwindSafe, catch_unwind};

const PID: u32 = 5678;
const SETUP_WORK: usize = POLICY_WORK + MANIFEST_WORK;

fn policy(generation: u64, budget: &mut Budget<'_>) -> Policy {
    let (policy, charge) = Policy::new(
        generation,
        Measurement::new([0x61; 32], 12345).unwrap(),
        Measurement::new([0x62; 32], 67890).unwrap(),
        SigningKey::from_bytes(&[0x51; 32])
            .verifying_key()
            .to_bytes(),
        SigningKey::from_bytes(&[0x52; 32])
            .verifying_key()
            .to_bytes(),
        budget,
    )
    .unwrap();
    assert_eq!(charge.additional_storage(), policy.retained_storage());
    budget.reserve_storage(charge.additional_storage()).unwrap();
    policy
}

fn manifest(pid: u32, policy: &Policy, budget: &mut Budget<'_>) -> Manifest {
    let (manifest, charge) = Manifest::new(
        Client::new(pid, 1000, 1001).unwrap(),
        Service::new(6000, 7000).unwrap(),
        policy,
        budget,
    )
    .unwrap();
    budget.reserve_storage(charge.additional_storage()).unwrap();
    manifest
}

fn inputs(budget: &mut Budget<'_>) -> (Policy, Manifest) {
    let policy = policy(7, budget);
    let manifest = manifest(1234, &policy, budget);
    (policy, manifest)
}

// Independent wire oracle: no shared-codec or V1 owner construction.
fn wire(pid: u32, launch: &[u8; 32], policy: &[u8; 32]) -> [u8; BYTES] {
    let mut bytes = [0; BYTES];
    bytes[..8].copy_from_slice(b"F2O3CER1");
    bytes[8..10].copy_from_slice(&1u16.to_le_bytes());
    bytes[12..16].copy_from_slice(&120u32.to_le_bytes());
    bytes[16..20].copy_from_slice(&pid.to_le_bytes());
    bytes[24..56].copy_from_slice(launch);
    bytes[56..88].copy_from_slice(policy);
    rehash(&mut bytes);
    bytes
}

fn rehash(bytes: &mut [u8; BYTES]) {
    let mut hash = Sha256::new();
    hash.update(b"FE2O3/COMPILER-EXECUTION-SERVICE-READY/V1\0");
    hash.update(88u64.to_le_bytes());
    hash.update(&bytes[..88]);
    bytes[88..].copy_from_slice(&hash.finalize());
}

fn reject(bytes: &[u8], expected: Framing, budget: &mut Budget<'_>) {
    let floor = budget.storage();
    let work = budget.work();
    assert_eq!(LegacyReady::decode(bytes).unwrap_err(), expected);
    assert!(matches!(Ready::decode(bytes, budget),
        Err(ReadyError::Framing(error)) if error == expected));
    assert_eq!(budget.storage(), floor);
    assert_eq!(budget.work(), work + WORK);
}

#[test]
fn independent_wire_round_trip_pid_boundaries_and_explicit_retention() {
    let mut work = Work::new(1_000_000);
    let mut budget = Budget::new(&mut work, 1_000_000);
    let (policy, manifest) = inputs(&mut budget);
    let input_floor = budget.storage();
    assert_eq!(
        input_floor,
        policy.retained_storage() + manifest.retained_storage()
    );

    for pid in [1, PID, u32::MAX] {
        let retained = {
            let before = budget.work();
            let (ready, charge) = Ready::new(pid, &manifest, &policy, &mut budget).unwrap();
            assert_eq!(budget.storage(), input_floor);
            assert_eq!(budget.work(), before + WORK);
            assert_eq!(charge.additional_storage(), ready.retained_storage());
            assert_eq!(ready.retained_storage(), size_of::<(Ready, Storage)>());
            budget.reserve_storage(charge.additional_storage()).unwrap();
            assert_eq!(ready.issuer_pid(), pid);
            assert_eq!(ready.launch_manifest_identity(), manifest.identity());
            assert_eq!(ready.policy_identity(), policy.identity());
            assert_eq!(
                ready.canonical_bytes(),
                &wire(
                    pid,
                    manifest.identity().as_bytes(),
                    policy.identity().as_bytes()
                )
            );
            assert_eq!(ready.identity().as_bytes(), &ready.canonical_bytes()[88..]);
            assert!(
                ready
                    .matches_launch(pid, &manifest, &policy, &mut budget)
                    .unwrap()
            );
            assert!(
                !ready
                    .matches_launch(0, &manifest, &policy, &mut budget)
                    .unwrap()
            );

            let floor = budget.storage();
            let (decoded, charge) = Ready::decode(ready.canonical_bytes(), &mut budget).unwrap();
            assert_eq!(budget.storage(), floor);
            assert_eq!(decoded, ready);
            assert_eq!(charge.additional_storage(), decoded.retained_storage());
            budget.reserve_storage(charge.additional_storage()).unwrap();
            assert!(
                decoded
                    .matches_launch(pid, &manifest, &policy, &mut budget)
                    .unwrap()
            );
            decoded.retained_storage() + ready.retained_storage()
        };
        budget.release_storage(retained).unwrap();
        assert_eq!(budget.storage(), input_floor);
    }
    assert_eq!(budget.work(), SETUP_WORK + 3 * 5 * WORK);
}

#[test]
fn exact_native_manifest_and_policy_relationships_are_required() {
    let mut work = Work::new(1_000_000);
    let mut budget = Budget::new(&mut work, 1_000_000);
    let first = policy(7, &mut budget);
    let second = policy(8, &mut budget);
    let launch = manifest(1234, &first, &mut budget);
    let other_client = manifest(1235, &first, &mut budget);
    let other_policy = manifest(1234, &second, &mut budget);
    let (ready, charge) = Ready::new(PID, &launch, &first, &mut budget).unwrap();
    budget.reserve_storage(charge.additional_storage()).unwrap();
    for (pid, manifest, policy) in [
        (PID + 1, &launch, &first),
        (PID, &other_client, &first),
        (PID, &launch, &second),
        (PID, &other_policy, &second),
    ] {
        let before = budget.work();
        let floor = budget.storage();
        assert!(
            !ready
                .matches_launch(pid, manifest, policy, &mut budget)
                .unwrap()
        );
        assert_eq!(budget.work(), before + WORK);
        assert_eq!(budget.storage(), floor);
    }
    for (pid, expected) in [(0, Framing::IssuerPid), (PID, Framing::PolicyMismatch)] {
        let before = budget.work();
        let floor = budget.storage();
        assert!(
            matches!(Ready::new(pid, &other_policy, &first, &mut budget),
            Err(ReadyError::Framing(error)) if error == expected)
        );
        assert_eq!(budget.work(), before + WORK);
        assert_eq!(budget.storage(), floor);
    }

    // Both stored identities agree with the arguments, but the manifest names
    // a different policy. Canonical framing alone must not establish a match.
    budget.reserve_storage(BYTES).unwrap();
    let bytes = wire(
        PID,
        other_policy.identity().as_bytes(),
        first.identity().as_bytes(),
    );
    let (inconsistent, charge) = Ready::decode(&bytes, &mut budget).unwrap();
    budget.reserve_storage(charge.additional_storage()).unwrap();
    assert_eq!(
        inconsistent.launch_manifest_identity(),
        other_policy.identity()
    );
    assert_eq!(inconsistent.policy_identity(), first.identity());
    assert!(
        !inconsistent
            .matches_launch(PID, &other_policy, &first, &mut budget)
            .unwrap()
    );
}

#[test]
fn legacy_wire_is_preserved_but_cannot_match_the_native_policy() {
    let mut work = Work::new(1_000_000);
    let mut budget = Budget::new(&mut work, 1_000_000);
    let (policy, manifest) = inputs(&mut budget);
    let old_policy = LegacyPolicy::new(
        policy.generation(),
        policy.executable(),
        policy.runtime(),
        *policy.verifying_key(),
        *policy.external_anchor_verifying_key(),
    )
    .unwrap();
    let old_manifest = LegacyManifest::new(
        manifest.client(),
        manifest.external_anchor_service(),
        &old_policy,
    );
    let old_ready = LegacyReady::new(PID, &old_manifest, &old_policy).unwrap();
    assert_eq!(
        old_ready.canonical_bytes(),
        &wire(
            PID,
            old_manifest.identity().as_bytes(),
            old_policy.identity().as_bytes()
        )
    );
    assert_eq!(
        LegacyReady::decode(old_ready.canonical_bytes()).unwrap(),
        old_ready
    );
    assert!(old_ready.matches_launch(PID, &old_manifest, &old_policy));
    budget
        .reserve_storage(size_of::<LegacyManifest>() + size_of::<LegacyReady>())
        .unwrap();
    let (opaque_manifest, charge) =
        Manifest::decode(old_manifest.canonical_bytes(), &mut budget).unwrap();
    budget.reserve_storage(charge.additional_storage()).unwrap();
    let (opaque_ready, charge) = Ready::decode(old_ready.canonical_bytes(), &mut budget).unwrap();
    budget.reserve_storage(charge.additional_storage()).unwrap();
    assert_eq!(opaque_ready.identity(), old_ready.identity());
    assert_eq!(opaque_ready.canonical_bytes(), old_ready.canonical_bytes());
    assert!(
        !opaque_ready
            .matches_launch(PID, &manifest, &policy, &mut budget)
            .unwrap()
    );
    assert!(
        !opaque_ready
            .matches_launch(PID, &opaque_manifest, &policy, &mut budget)
            .unwrap()
    );
    assert!(matches!(
        Ready::new(PID, &opaque_manifest, &policy, &mut budget),
        Err(ReadyError::Framing(Framing::PolicyMismatch))
    ));
    let (native, charge) = Ready::new(PID, &manifest, &policy, &mut budget).unwrap();
    budget.reserve_storage(charge.additional_storage()).unwrap();
    assert_ne!(native.identity(), old_ready.identity());
    let legacy_view = LegacyReady::decode(native.canonical_bytes()).unwrap();
    assert!(!legacy_view.matches_launch(PID, &old_manifest, &old_policy));
}

#[test]
fn every_byte_mutation_and_every_truncation_reject_with_legacy_precedence() {
    let mut work = Work::new(300 * WORK);
    let mut budget = Budget::new(&mut work, 3 * BYTES + 1 + STORAGE);
    budget.reserve_storage(3 * BYTES + 1).unwrap();
    let good = wire(PID, &[0x61; 32], &[0x62; 32]);
    for index in 0..BYTES {
        let mut bytes = good;
        bytes[index] ^= 1;
        let expected = LegacyReady::decode(&bytes).unwrap_err();
        reject(&bytes, expected, &mut budget);
    }
    for length in 0..BYTES {
        reject(&good[..length], Framing::Length, &mut budget);
    }
    let mut extended = [0; BYTES + 1];
    extended[..BYTES].copy_from_slice(&good);
    reject(&extended, Framing::Length, &mut budget);
}

#[test]
fn resealed_invalid_fields_and_multiple_errors_preserve_diagnostic_precedence() {
    let mut work = Work::new(100 * WORK);
    let mut budget = Budget::new(&mut work, 2 * BYTES + STORAGE);
    budget.reserve_storage(2 * BYTES).unwrap();
    let good = wire(PID, &[0x61; 32], &[0x62; 32]);
    for index in [10, 11, 20, 21, 22, 23] {
        let mut bytes = good;
        bytes[index] = 1;
        rehash(&mut bytes);
        reject(&bytes, Framing::Reserved, &mut budget);
    }
    for (range, expected) in [
        (0..8, Framing::Magic),
        (8..10, Framing::Version),
        (12..16, Framing::Length),
        (16..20, Framing::IssuerPid),
        (24..56, Framing::LaunchManifestIdentity),
        (56..88, Framing::PolicyIdentity),
    ] {
        let mut bytes = good;
        bytes[range].fill(0);
        rehash(&mut bytes);
        reject(&bytes, expected, &mut budget);
    }
    let mut bytes = good;
    bytes[8..10].copy_from_slice(&2u16.to_le_bytes());
    rehash(&mut bytes);
    reject(&bytes, Framing::Version, &mut budget);
    bytes = good;
    bytes[12..16].copy_from_slice(&u32::MAX.to_le_bytes());
    rehash(&mut bytes);
    reject(&bytes, Framing::Length, &mut budget);

    for (start, expected) in [
        (0, Framing::Magic),
        (8, Framing::Version),
        (12, Framing::Length),
        (16, Framing::IssuerPid),
        (24, Framing::LaunchManifestIdentity),
        (56, Framing::PolicyIdentity),
        (88, Framing::Identity),
    ] {
        bytes = good;
        bytes[start..].fill(0);
        reject(&bytes, expected, &mut budget);
    }
    bytes = good;
    bytes[10] = 1;
    bytes[12..].fill(0);
    reject(&bytes, Framing::Reserved, &mut budget);
}

#[test]
fn exact_and_short_decode_budgets_preserve_prefix_peak_and_denials() {
    let bytes = wire(PID, &[0x61; 32], &[0x62; 32]);
    for mode in 0..5 {
        let floor = BYTES - usize::from(mode == 0);
        let available_work = if mode == 1 {
            resources::ENTRY_WORK - 1
        } else {
            WORK - usize::from(mode == 2)
        };
        let limit = floor + STORAGE - usize::from(mode == 3);
        let mut work = Work::new(13 + available_work);
        work.charge_work(13).unwrap();
        assert!(work.charge_work(13 + WORK + 1).is_err());
        {
            let mut budget = Budget::new(&mut work, limit);
            budget.reserve_storage(floor).unwrap();
            assert!(budget.reserve_storage(limit + 1).is_err());
            let result = Ready::decode(&bytes, &mut budget);
            assert_eq!(budget.storage(), floor);
            assert_eq!(budget.failed_storage(), Some(floor + limit + 1));
            match mode {
                0 => {
                    assert!(matches!(
                        result,
                        Err(ReadyError::Resource(Resource::Accounting))
                    ));
                    assert_eq!(budget.work(), 13 + resources::ENTRY_WORK);
                }
                1 | 2 => {
                    assert!(matches!(
                        result,
                        Err(ReadyError::Resource(Resource::Work(_)))
                    ));
                    assert_eq!(
                        budget.work(),
                        13 + if mode == 1 { 0 } else { resources::ENTRY_WORK }
                    );
                }
                3 => {
                    assert!(matches!(
                        result,
                        Err(ReadyError::Resource(Resource::Storage(_)))
                    ));
                    assert_eq!(budget.work(), 13 + WORK);
                }
                _ => {
                    let (ready, charge) = result.unwrap();
                    assert_eq!(charge.additional_storage(), ready.retained_storage());
                    assert_eq!(budget.work(), 13 + WORK);
                }
            }
            assert_eq!(budget.peak_storage(), if mode == 4 { limit } else { floor });
        }
        assert_eq!(work.failed_work(), Some(26 + WORK + 1));
    }
}

#[test]
fn constructor_and_match_preflight_all_resources_on_the_original_ledger() {
    for matching in [false, true] {
        for mode in 0..5 {
            let setup = SETUP_WORK + if matching { WORK } else { 0 };
            let available_work = if mode == 1 {
                resources::ENTRY_WORK - 1
            } else {
                WORK - usize::from(mode == 2)
            };
            let mut work = Work::new(setup + available_work);
            let mut budget = Budget::new(&mut work, 100_000);
            let (policy, manifest) = inputs(&mut budget);
            let ready = if matching {
                let (ready, charge) = Ready::new(PID, &manifest, &policy, &mut budget).unwrap();
                budget.reserve_storage(charge.additional_storage()).unwrap();
                Some(ready)
            } else {
                None
            };
            assert_eq!(budget.work(), setup);
            if mode == 0 {
                budget.release_storage(1).unwrap();
            } else {
                // Leave exactly the operation's scratch (or one byte less),
                // after admitting fixtures on this same original ledger.
                let remaining = STORAGE - usize::from(mode == 3);
                let padding = budget.storage_limit() - budget.storage() - remaining;
                budget.reserve_storage(padding).unwrap();
            }
            let floor = budget.storage();
            let peak = budget.peak_storage();
            let result = match ready.as_ref() {
                Some(ready) => ready.matches_launch(PID, &manifest, &policy, &mut budget),
                None => Ready::new(PID, &manifest, &policy, &mut budget).map(|(ready, charge)| {
                    assert_eq!(charge.additional_storage(), ready.retained_storage());
                    true
                }),
            };
            assert_eq!(budget.storage(), floor);
            match mode {
                0 => {
                    assert!(matches!(
                        result,
                        Err(ReadyError::Resource(Resource::Accounting))
                    ));
                    assert_eq!(budget.work(), setup + resources::ENTRY_WORK);
                }
                1 | 2 => {
                    assert!(matches!(
                        result,
                        Err(ReadyError::Resource(Resource::Work(_)))
                    ));
                    assert_eq!(
                        budget.work(),
                        setup + if mode == 1 { 0 } else { resources::ENTRY_WORK }
                    );
                }
                3 => {
                    assert!(matches!(
                        result,
                        Err(ReadyError::Resource(Resource::Storage(_)))
                    ));
                    assert_eq!(budget.work(), setup + WORK);
                    assert_eq!(budget.failed_storage(), Some(budget.storage_limit() + 1));
                }
                _ => {
                    assert!(result.unwrap());
                    assert_eq!(budget.work(), setup + WORK);
                }
            }
            assert_eq!(
                budget.peak_storage(),
                if mode == 4 {
                    budget.storage_limit()
                } else {
                    peak
                }
            );
        }
    }
}

#[test]
fn exhausted_work_cannot_be_reset_by_repeated_decode_and_storage_is_explicit() {
    let bytes = wire(PID, &[0x61; 32], &[0x62; 32]);
    let mut work = Work::new(2 * WORK);
    {
        let mut budget = Budget::new(&mut work, BYTES + RETAINED + STORAGE);
        budget.reserve_storage(BYTES).unwrap();
        let retained = {
            let (first, charge) = Ready::decode(&bytes, &mut budget).unwrap();
            assert_eq!(budget.storage(), BYTES);
            budget.reserve_storage(charge.additional_storage()).unwrap();
            let (second, charge) = Ready::decode(&bytes, &mut budget).unwrap();
            assert_eq!(budget.storage(), BYTES + first.retained_storage());
            budget.reserve_storage(charge.additional_storage()).unwrap();
            assert_eq!(budget.storage(), BYTES + 2 * RETAINED);
            for _ in 0..3 {
                assert!(matches!(
                    Ready::decode(&bytes, &mut budget),
                    Err(ReadyError::Resource(Resource::Work(_)))
                ));
                assert_eq!(budget.work(), 2 * WORK);
                assert_eq!(budget.storage(), BYTES + 2 * RETAINED);
            }
            first.retained_storage() + second.retained_storage()
        };
        budget.release_storage(retained).unwrap();
        assert_eq!(budget.storage(), BYTES);
        assert_eq!(budget.peak_storage(), BYTES + RETAINED + STORAGE);
        assert_eq!(budget.failed_storage(), None);
    }
    assert_eq!(work.failed_work(), Some(2 * WORK + resources::ENTRY_WORK));
}

#[test]
fn wrong_length_requires_prepaid_work_and_scratch_before_framing() {
    for mode in 0..3 {
        let mut work = Work::new(WORK - usize::from(mode == 0));
        let mut budget = Budget::new(&mut work, STORAGE - usize::from(mode == 1));
        let result = Ready::decode(&[], &mut budget);
        match mode {
            0 => assert!(matches!(
                result,
                Err(ReadyError::Resource(Resource::Work(_)))
            )),
            1 => assert!(matches!(
                result,
                Err(ReadyError::Resource(Resource::Storage(_)))
            )),
            _ => assert!(matches!(result, Err(ReadyError::Framing(Framing::Length)))),
        }
        assert_eq!(budget.storage(), 0);
        assert_eq!(
            budget.work(),
            if mode == 0 {
                resources::ENTRY_WORK
            } else {
                WORK
            }
        );
    }
}

#[test]
fn success_error_and_unwind_restore_storage_without_refunding_work_or_denials() {
    for mode in 0..3 {
        let mut work = Work::new(13 + WORK);
        work.charge_work(13).unwrap();
        assert!(work.charge_work(WORK + 1).is_err());
        {
            let mut budget = Budget::new(&mut work, 31 + STORAGE);
            budget.reserve_storage(31).unwrap();
            assert!(budget.reserve_storage(STORAGE + 1).is_err());
            let result = catch_unwind(AssertUnwindSafe(|| {
                metered::<()>(&mut budget, 31, || match mode {
                    0 => Ok(()),
                    1 => Err(Framing::Identity.into()),
                    _ => panic!("readiness fixed scope"),
                })
            }));
            match mode {
                0 => assert!(result.unwrap().is_ok()),
                1 => assert!(matches!(
                    result.unwrap(),
                    Err(ReadyError::Framing(Framing::Identity))
                )),
                _ => assert!(result.is_err()),
            }
            assert_eq!(budget.storage(), 31);
            assert_eq!(budget.work(), 13 + WORK);
            assert_eq!(budget.peak_storage(), 31 + STORAGE);
            assert_eq!(budget.failed_storage(), Some(32 + STORAGE));
        }
        assert_eq!(work.failed_work(), Some(14 + WORK));
    }
}
