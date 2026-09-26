use ed25519_dalek::SigningKey;
use fe2o3_compiler_execution_protocol::{
    COMPILER_EXECUTION_CLIENT_PROFILE_BYTES_V3 as BYTES,
    COMPILER_EXECUTION_CLIENT_PROFILE_STORAGE_V3 as STORAGE,
    COMPILER_EXECUTION_CLIENT_PROFILE_WORK_V3 as WORK,
    COMPILER_EXECUTION_ISSUER_POLICY_WORK_V3 as POLICY_WORK,
    CompilerExecutionAttestationErrorV1 as PolicyFraming,
    CompilerExecutionAttestationErrorV3 as PolicyError,
    CompilerExecutionAttestationStorageV3 as Charge,
    CompilerExecutionClientProfileErrorV1 as Framing,
    CompilerExecutionClientProfileErrorV2 as ErrorV2,
    CompilerExecutionClientProfileErrorV3 as Error,
    CompilerExecutionClientProfileV1 as ProfileV1,
    CompilerExecutionClientProfileV2 as ProfileV2,
    CompilerExecutionClientProfileV3 as Profile,
    CompilerExecutionExternalAnchorServiceIdentityV1 as Service,
    CompilerExecutionIssuerMeasurementV1 as Measurement,
    CompilerExecutionIssuerPolicyV1 as PolicyV1,
    CompilerExecutionIssuerPolicyV2 as PolicyV2,
    CompilerExecutionIssuerPolicyV3 as Policy,
};
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
    CanonicalKernelIrWorkBudgetV1 as Work,
};
use sha2::{Digest, Sha256};
use std::{error::Error as _, mem::size_of};

fn key(seed: u8) -> [u8; 32] {
    SigningKey::from_bytes(&[seed; 32]).verifying_key().to_bytes()
}
fn service() -> Service {
    Service::new(6001, 7001).unwrap()
}
fn policy(budget: &mut Budget<'_>) -> Policy {
    let (policy, charge) = Policy::new(
        7,
        Measurement::new([0x61; 32], 12345).unwrap(),
        Measurement::new([0x62; 32], 67890).unwrap(),
        key(0x51),
        key(0x52),
        budget,
    )
    .unwrap();
    budget.reserve_storage(charge.additional_storage()).unwrap();
    policy
}
fn rehash_profile(bytes: &mut [u8], domain: &[u8]) {
    let mut hash = Sha256::new();
    hash.update((domain.len() as u64).to_le_bytes());
    hash.update(domain);
    hash.update(248_u64.to_le_bytes());
    hash.update(&bytes[..248]);
    bytes[248..].copy_from_slice(&hash.finalize());
}
const DOMAIN: &[u8] = b"FE2O3/COMPILER-EXECUTION-CLIENT-PROFILE/V3\0";

// Independent transcript, including the actual SubjectV3 policy framing/hash.
fn wire() -> [u8; 280] {
    let mut policy = [0; 216];
    policy[..8].copy_from_slice(b"F2O3CEP3");
    policy[8..10].copy_from_slice(&3_u16.to_le_bytes());
    policy[12..20].copy_from_slice(&216_u64.to_le_bytes());
    policy[24..32].copy_from_slice(&7_u64.to_le_bytes());
    policy[32..64].fill(0x61);
    policy[64..72].copy_from_slice(&12345_u64.to_le_bytes());
    policy[72..104].fill(0x62);
    policy[104..112].copy_from_slice(&67890_u64.to_le_bytes());
    policy[112..144].copy_from_slice(&key(0x51));
    policy[144..176].copy_from_slice(&key(0x52));
    policy[176..178].copy_from_slice(&3_u16.to_le_bytes());
    let mut hash = Sha256::new();
    hash.update(b"FE2O3/COMPILER-EXECUTION-ISSUER-POLICY/V3\0");
    hash.update(184_u64.to_le_bytes());
    hash.update(&policy[..184]);
    policy[184..].copy_from_slice(&hash.finalize());

    let mut bytes = [0; 280];
    bytes[..8].copy_from_slice(b"F2O3CEP3");
    bytes[8..10].copy_from_slice(&3_u16.to_le_bytes());
    bytes[12..16].copy_from_slice(&280_u32.to_le_bytes());
    for (offset, value) in [(16, 1234_u32), (20, 5678), (24, 6001), (28, 7001)] {
        bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
    }
    bytes[32..248].copy_from_slice(&policy);
    rehash_profile(&mut bytes, DOMAIN);
    bytes
}

#[test]
fn actual_v3_roundtrip_preserves_original_ledger_and_full_delta_charges() {
    let mut work = Work::new(POLICY_WORK + 3 * WORK);
    let mut budget = Budget::new(&mut work, 100_000);
    budget.reserve_storage(19).unwrap();
    let ledger = budget.work_ledger_identity_v1();
    let policy = policy(&mut budget);
    let inherited = policy.retained_storage();
    let policy_identity = policy.identity();
    let (profile, delta) = Profile::new(1234, 5678, service(), policy, &mut budget).unwrap();
    assert_eq!(budget.storage(), 19 + inherited);
    assert_eq!(inherited + delta.additional_storage(), profile.retained_storage());
    budget.reserve_storage(delta.additional_storage()).unwrap();
    assert_eq!(BYTES, 280);
    assert_eq!(profile.canonical_bytes(), &wire());
    let actual_policy: &Policy = profile.policy();
    assert_eq!(actual_policy.identity(), policy_identity);
    assert_eq!(actual_policy.generation(), 7);
    assert_eq!(actual_policy.verifying_key(), &key(0x51));
    assert_eq!(actual_policy.external_anchor_verifying_key(), &key(0x52));
    assert_eq!((profile.supervisor_uid(), profile.supervisor_gid()), (1234, 5678));
    assert_eq!(profile.external_anchor_service(), service());
    assert!(profile.identity().matches_canonical_bytes(profile.canonical_bytes(), &mut budget).unwrap());
    let floor = budget.storage();
    let (decoded, full): (Profile, Charge) = Profile::decode(profile.canonical_bytes(), &mut budget).unwrap();
    assert_eq!(decoded, profile);
    assert_eq!(budget.storage(), floor);
    assert_eq!(full.additional_storage(), decoded.retained_storage());
    budget.reserve_storage(full.additional_storage()).unwrap();
    let retained = profile.retained_storage();
    drop((decoded, profile));
    budget.release_storage(2 * retained).unwrap();
    assert_eq!(budget.storage(), 19);
    assert_eq!(budget.work(), POLICY_WORK + 3 * WORK);
    assert!(budget.work_ledger_identity_v1() == ledger);
}

#[test]
fn families_reject_each_other_and_rehashed_mixed_nested_policies() {
    let mut work = Work::new(1_000_000);
    let mut budget = Budget::new(&mut work, 100_000);
    budget.reserve_storage(BYTES).unwrap();
    let old_policy = PolicyV1::new(
        7,
        Measurement::new([0x61; 32], 12345).unwrap(),
        Measurement::new([0x62; 32], 67890).unwrap(),
        key(0x51), key(0x52),
    ).unwrap();
    let (v2_policy, charge) = PolicyV2::new(
        7, old_policy.executable(), old_policy.runtime(), key(0x51), key(0x52), &mut budget,
    ).unwrap();
    budget.reserve_storage(charge.additional_storage()).unwrap();
    let (v2, delta) = ProfileV2::new(1234, 5678, service(), v2_policy, &mut budget).unwrap();
    budget.reserve_storage(delta.additional_storage()).unwrap();
    let v1 = ProfileV1::new(1234, 5678, service(), old_policy).unwrap();
    let (v3, charge) = Profile::decode(&wire(), &mut budget).unwrap();
    budget.reserve_storage(charge.additional_storage()).unwrap();
    for older in [v1.canonical_bytes(), v2.canonical_bytes()] {
        assert!(matches!(Profile::decode(older, &mut budget), Err(Error::Framing(Framing::Magic))));
        assert!(!v3.identity().matches_canonical_bytes(older, &mut budget).unwrap());
        let mut mixed = wire();
        mixed[32..248].copy_from_slice(&older[32..248]);
        rehash_profile(&mut mixed, DOMAIN);
        let error = Profile::decode(&mixed, &mut budget).unwrap_err();
        assert!(matches!(&error, Error::Policy(PolicyError::Framing(PolicyFraming::InvalidMagic(_)))));
        assert!(error.source().unwrap().is::<PolicyError>());
    }
    assert!(matches!(ProfileV1::decode(&wire()), Err(Framing::Magic)));
    assert!(matches!(ProfileV2::decode(&wire(), &mut budget), Err(ErrorV2::Framing(Framing::Magic))));
    for (older, domain, family) in [
        (v1.canonical_bytes(), &b"FE2O3/COMPILER-EXECUTION-CLIENT-PROFILE/V1\0"[..], 1),
        (v2.canonical_bytes(), &b"FE2O3/COMPILER-EXECUTION-CLIENT-PROFILE/V2\0"[..], 2),
    ] {
        let mut mixed = *older;
        mixed[32..248].copy_from_slice(&wire()[32..248]);
        rehash_profile(&mut mixed, domain);
        if family == 1 {
            assert!(matches!(ProfileV1::decode(&mixed), Err(Framing::Policy(_))));
        } else {
            assert!(matches!(ProfileV2::decode(&mixed, &mut budget), Err(ErrorV2::Policy(_))));
        }
    }
    let mut wrong_domain = wire();
    rehash_profile(&mut wrong_domain, b"FE2O3/COMPILER-EXECUTION-CLIENT-PROFILE/V2\0");
    assert!(matches!(Profile::decode(&wrong_domain, &mut budget), Err(Error::Framing(Framing::Identity))));
}

#[test]
fn framing_credentials_nested_policy_and_hash_keep_diagnostic_order() {
    let mut bytes = wire();
    bytes[16..20].fill(0);
    bytes[32] ^= 1;
    bytes[248..].fill(0);
    let mut work = Work::new(5 * WORK);
    let mut budget = Budget::new(&mut work, BYTES + STORAGE);
    budget.reserve_storage(BYTES).unwrap();
    assert!(matches!(
        Profile::decode(&bytes, &mut budget),
        Err(Error::Framing(Framing::InvalidSupervisorUid))
    ));
    bytes[16..20].copy_from_slice(&1234_u32.to_le_bytes());
    assert!(matches!(Profile::decode(&bytes, &mut budget), Err(Error::Policy(_))));
    bytes[32] ^= 1;
    assert!(matches!(Profile::decode(&bytes, &mut budget), Err(Error::Framing(Framing::Identity))));
    bytes[8..10].copy_from_slice(&2_u16.to_le_bytes());
    assert!(matches!(Profile::decode(&bytes, &mut budget), Err(Error::Framing(Framing::Version(2)))));
    bytes[0] ^= 1;
    assert!(matches!(Profile::decode(&bytes, &mut budget), Err(Error::Framing(Framing::Magic))));
    assert_eq!(budget.storage(), BYTES);
}

#[test]
fn decoding_and_identity_checks_require_exact_prepaid_quotas() {
    let mut fixture_work = Work::new(WORK);
    let mut fixture_budget = Budget::new(&mut fixture_work, BYTES + STORAGE);
    fixture_budget.reserve_storage(BYTES).unwrap();
    let profile = Profile::decode(&wire(), &mut fixture_budget).unwrap().0;
    for identity_only in [false, true] {
        for case in 0..6 {
            let floor = if case == 1 { BYTES - 1 } else { BYTES };
            let work_limit = match case { 0 => 7, 2 => WORK - 1, _ => WORK };
            let limit = floor + STORAGE - usize::from(case == 3);
            let mut work = Work::new(work_limit);
            let mut budget = Budget::new(&mut work, limit);
            budget.reserve_storage(floor).unwrap();
            let ledger = budget.work_ledger_identity_v1();
            let bytes = if case == 5 { [0; BYTES] } else { wire() };
            let result = if identity_only {
                profile.identity().matches_canonical_bytes(&bytes, &mut budget).map(|value| {
                    assert_eq!(value, case != 5);
                })
            } else {
                Profile::decode(&bytes, &mut budget).map(|(decoded, charge)| {
                    assert_eq!(charge.additional_storage(), decoded.retained_storage());
                })
            };
            assert!(budget.work_ledger_identity_v1() == ledger);
            assert_eq!(budget.storage(), floor);
            assert_eq!(budget.work(), match case { 0 => 0, 1 | 2 => 8, _ => WORK });
            assert_eq!(budget.peak_storage(), floor + if case >= 4 { STORAGE } else { 0 });
            match case {
                0 | 2 => assert!(matches!(result, Err(Error::Resource(Resource::Work(_))))),
                1 => assert!(matches!(result, Err(Error::Resource(Resource::Accounting)))),
                3 => {
                    assert!(matches!(result, Err(Error::Resource(Resource::Storage(_)))));
                    assert_eq!(budget.failed_storage(), Some(floor + STORAGE));
                }
                5 if !identity_only => assert!(matches!(result, Err(Error::Framing(Framing::Magic)))),
                _ => result.unwrap(),
            }
        }
    }
}

#[test]
fn construction_retains_consumed_floor_and_cannot_restart_the_ledger() {
    let inherited = size_of::<Policy>() + size_of::<Charge>();
    for case in 0..5 {
        let mut work = Work::new(POLICY_WORK + WORK - usize::from(case == 0));
        let mut budget = Budget::new(&mut work, inherited + STORAGE - usize::from(case == 1));
        let policy = policy(&mut budget);
        assert_eq!(policy.retained_storage(), inherited);
        if case == 2 {
            budget.release_storage(1).unwrap();
        }
        let floor = budget.storage();
        let uid = if case == 3 { 0 } else { 1234 };
        let result = Profile::new(uid, 5678, service(), policy, &mut budget);
        assert_eq!(budget.storage(), floor);
        assert_eq!(budget.work(), POLICY_WORK + if case == 0 || case == 2 { 8 } else { WORK });
        match case {
            0 => assert!(matches!(result, Err(Error::Resource(Resource::Work(_))))),
            1 => assert!(matches!(result, Err(Error::Resource(Resource::Storage(_))))),
            2 => assert!(matches!(result, Err(Error::Resource(Resource::Accounting)))),
            3 => assert!(matches!(result, Err(Error::Framing(Framing::InvalidSupervisorUid)))),
            _ => {
                let (profile, delta) = result.unwrap();
                assert_eq!(inherited + delta.additional_storage(), profile.retained_storage());
            }
        }
        budget.release_storage(floor).unwrap();
        assert_eq!(budget.storage(), 0);
    }
}

#[test]
fn malformed_lengths_and_single_byte_mutations_never_admit() {
    let mut work = Work::new((BYTES + 5) * WORK);
    let mut budget = Budget::new(&mut work, BYTES + STORAGE);
    for length in [0, 1, BYTES - 1, BYTES + 1, 100_000] {
        assert!(matches!(
            Profile::decode(&vec![0; length], &mut budget),
            Err(Error::Framing(Framing::Length))
        ));
        assert_eq!(budget.storage(), 0);
    }
    budget.reserve_storage(BYTES).unwrap();
    for offset in 0..BYTES {
        let mut bytes = wire();
        bytes[offset] ^= 1;
        assert!(Profile::decode(&bytes, &mut budget).is_err(), "byte {offset}");
        assert_eq!(budget.storage(), BYTES);
    }
}

#[test]
fn v2_layout_quotas_and_v3_aggregate_costs_remain_fixed() {
    use fe2o3_compiler_execution_protocol::*;
    assert_eq!(COMPILER_EXECUTION_CLIENT_PROFILE_BYTES_V2, 280);
    assert_eq!(COMPILER_EXECUTION_CLIENT_PROFILE_WORK_V2, 24072);
    let retained_v2 = size_of::<ProfileV2>() + size_of::<CompilerExecutionAttestationStorageV2>();
    assert_eq!(COMPILER_EXECUTION_CLIENT_PROFILE_STORAGE_V2,
        COMPILER_EXECUTION_ISSUER_POLICY_STORAGE_V2 + 4 * retained_v2 + 4 * 280
            + 2 * size_of::<Sha256>() + 4096);
    assert_eq!(size_of::<Profile>(), size_of::<ProfileV2>());
    assert_eq!(WORK, POLICY_WORK + 32 * BYTES);
    assert_eq!(WORK, COMPILER_EXECUTION_CLIENT_PROFILE_WORK_V2);
    assert_eq!(STORAGE, COMPILER_EXECUTION_CLIENT_PROFILE_STORAGE_V2);
}
