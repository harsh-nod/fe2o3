use ed25519_dalek::SigningKey;
use fe2o3_compiler_execution_protocol::{
    COMPILER_EXECUTION_CLIENT_PROFILE_BYTES_V2 as PROFILE_BYTES,
    COMPILER_EXECUTION_CLIENT_PROFILE_STORAGE_V2 as PROFILE_STORAGE,
    COMPILER_EXECUTION_CLIENT_PROFILE_WORK_V2 as PROFILE_WORK,
    COMPILER_EXECUTION_ISSUER_POLICY_BYTES_V2 as POLICY_BYTES,
    COMPILER_EXECUTION_ISSUER_POLICY_STORAGE_V2 as POLICY_STORAGE,
    COMPILER_EXECUTION_ISSUER_POLICY_WORK_V2 as POLICY_WORK,
    CompilerExecutionAttestationErrorV1 as LegacyError,
    CompilerExecutionAttestationErrorV2 as PolicyError,
    CompilerExecutionClientProfileErrorV1 as LegacyProfileError,
    CompilerExecutionClientProfileErrorV2 as ProfileError,
    CompilerExecutionClientProfileV1 as LegacyProfile, CompilerExecutionClientProfileV2 as Profile,
    CompilerExecutionExternalAnchorServiceIdentityV1 as Service,
    CompilerExecutionIssuerMeasurementV1 as Measurement,
    CompilerExecutionIssuerPolicyV1 as LegacyPolicy, CompilerExecutionIssuerPolicyV2 as Policy,
};
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
    CanonicalKernelIrWorkBudgetV1 as Work,
};
use sha2::{Digest, Sha256};

fn issuer_key() -> [u8; 32] {
    SigningKey::from_bytes(&[0x51; 32])
        .verifying_key()
        .to_bytes()
}
fn anchor_key() -> [u8; 32] {
    SigningKey::from_bytes(&[0x52; 32])
        .verifying_key()
        .to_bytes()
}
fn measurement(seed: u8, length: u64) -> Measurement {
    Measurement::new([seed; 32], length).unwrap()
}
fn service() -> Service {
    Service::new(6001, 7001).unwrap()
}
fn legacy() -> LegacyPolicy {
    LegacyPolicy::new(
        7,
        measurement(0x61, 12345),
        measurement(0x62, 67890),
        issuer_key(),
        anchor_key(),
    )
    .unwrap()
}
fn native(budget: &mut Budget<'_>) -> Policy {
    let (policy, storage) = Policy::new(
        7,
        measurement(0x61, 12345),
        measurement(0x62, 67890),
        issuer_key(),
        anchor_key(),
        budget,
    )
    .unwrap();
    assert_eq!(storage.additional_storage(), policy.retained_storage());
    budget
        .reserve_storage(storage.additional_storage())
        .unwrap();
    policy
}
fn policy_wire(version: u16) -> [u8; POLICY_BYTES] {
    let mut bytes = [0; POLICY_BYTES];
    bytes[..8].copy_from_slice(if version == 1 {
        b"F2O3CEP1"
    } else {
        b"F2O3CEP2"
    });
    bytes[8..10].copy_from_slice(&version.to_le_bytes());
    bytes[12..20].copy_from_slice(&216u64.to_le_bytes());
    bytes[24..32].copy_from_slice(&7u64.to_le_bytes());
    bytes[32..64].fill(0x61);
    bytes[64..72].copy_from_slice(&12345u64.to_le_bytes());
    bytes[72..104].fill(0x62);
    bytes[104..112].copy_from_slice(&67890u64.to_le_bytes());
    bytes[112..144].copy_from_slice(&issuer_key());
    bytes[144..176].copy_from_slice(&anchor_key());
    bytes[176..178].copy_from_slice(&version.to_le_bytes());
    rehash_policy(&mut bytes, version);
    bytes
}
fn rehash_policy(bytes: &mut [u8], version: u16) {
    let domain: &[u8] = if version == 1 {
        b"FE2O3/COMPILER-EXECUTION-ISSUER-POLICY/V1\0"
    } else {
        b"FE2O3/COMPILER-EXECUTION-ISSUER-POLICY/V2\0"
    };
    let mut hash = Sha256::new();
    hash.update(domain);
    hash.update(184u64.to_le_bytes());
    hash.update(&bytes[..184]);
    bytes[184..].copy_from_slice(&hash.finalize());
}
fn profile_wire(version: u16) -> [u8; PROFILE_BYTES] {
    let mut bytes = [0; PROFILE_BYTES];
    bytes[..8].copy_from_slice(if version == 1 {
        b"F2O3CEP1"
    } else {
        b"F2O3CEP2"
    });
    bytes[8..10].copy_from_slice(&version.to_le_bytes());
    bytes[12..16].copy_from_slice(&280u32.to_le_bytes());
    for (offset, value) in [(16, 1234u32), (20, 5678), (24, 6001), (28, 7001)] {
        bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
    }
    bytes[32..248].copy_from_slice(&policy_wire(version));
    rehash_profile(&mut bytes, version);
    bytes
}
fn rehash_profile(bytes: &mut [u8], version: u16) {
    let domain: &[u8] = if version == 1 {
        b"FE2O3/COMPILER-EXECUTION-CLIENT-PROFILE/V1\0"
    } else {
        b"FE2O3/COMPILER-EXECUTION-CLIENT-PROFILE/V2\0"
    };
    let mut hash = Sha256::new();
    hash.update((domain.len() as u64).to_le_bytes());
    hash.update(domain);
    hash.update(248u64.to_le_bytes());
    hash.update(&bytes[..248]);
    bytes[248..].copy_from_slice(&hash.finalize());
}

#[test]
fn independent_wire_transcripts_freeze_both_families() {
    assert_eq!(POLICY_BYTES, 216);
    assert_eq!(PROFILE_BYTES, 280);
    assert_eq!(legacy().canonical_bytes(), &policy_wire(1));
    let old = LegacyProfile::new(1234, 5678, service(), legacy()).unwrap();
    assert_eq!(old.canonical_bytes(), &profile_wire(1));
    assert_eq!(LegacyProfile::decode(&profile_wire(1)).unwrap(), old);
    let mut work = Work::new(2 * POLICY_WORK + 3 * PROFILE_WORK);
    let mut budget = Budget::new(&mut work, 100_000);
    let released;
    {
        let policy = native(&mut budget);
        assert_eq!(policy.canonical_bytes(), &policy_wire(2));
        assert_ne!(policy.identity().as_bytes(), legacy().identity().as_bytes());
        let inherited = policy.retained_storage();
        let (profile, delta) = Profile::new(1234, 5678, service(), policy, &mut budget).unwrap();
        assert_eq!(budget.storage(), inherited);
        assert_eq!(
            inherited + delta.additional_storage(),
            profile.retained_storage()
        );
        budget.reserve_storage(delta.additional_storage()).unwrap();
        assert_eq!(profile.canonical_bytes(), &profile_wire(2));
        assert_eq!(
            (profile.supervisor_uid(), profile.supervisor_gid()),
            (1234, 5678)
        );
        assert_eq!(profile.external_anchor_service(), service());
        assert_eq!(profile.policy().generation(), 7);
        assert_eq!(profile.policy().executable(), measurement(0x61, 12345));
        assert_eq!(profile.policy().runtime(), measurement(0x62, 67890));
        assert_eq!(profile.policy().verifying_key(), &issuer_key());
        assert_eq!(
            profile.policy().external_anchor_verifying_key(),
            &anchor_key()
        );
        let floor = budget.storage();
        assert!(
            profile
                .identity()
                .matches_canonical_bytes(profile.canonical_bytes(), &mut budget)
                .unwrap()
        );
        assert!(
            profile
                .policy()
                .identity()
                .matches_canonical_bytes(profile.policy().canonical_bytes(), &mut budget)
                .unwrap()
        );
        let (decoded, charge) = Profile::decode(profile.canonical_bytes(), &mut budget).unwrap();
        assert_eq!(decoded, profile);
        assert_eq!(charge.additional_storage(), decoded.retained_storage());
        assert_eq!(budget.storage(), floor);
        budget.reserve_storage(charge.additional_storage()).unwrap();
        released = decoded.retained_storage() + profile.retained_storage();
    }
    budget.release_storage(released).unwrap();
    assert_eq!(budget.storage(), 0);
    assert_eq!(budget.work(), 2 * POLICY_WORK + 3 * PROFILE_WORK);
}

#[test]
fn profile_constructor_exact_and_one_short_storage_preserves_inherited_floor() {
    for short in [false, true] {
        let inherited = std::mem::size_of::<Policy>()
            + std::mem::size_of::<
                fe2o3_compiler_execution_protocol::CompilerExecutionAttestationStorageV2,
            >();
        let mut work = Work::new(POLICY_WORK + PROFILE_WORK);
        let mut budget = Budget::new(&mut work, inherited + PROFILE_STORAGE - usize::from(short));
        let policy = native(&mut budget);
        assert_eq!(policy.retained_storage(), inherited);
        let result = Profile::new(1234, 5678, service(), policy, &mut budget);
        assert_eq!(budget.storage(), inherited);
        assert_eq!(budget.work(), POLICY_WORK + PROFILE_WORK);
        if short {
            assert!(matches!(
                result,
                Err(ProfileError::Resource(Resource::Storage(_)))
            ));
            assert_eq!(budget.failed_storage(), Some(inherited + PROFILE_STORAGE));
        } else {
            let (profile, delta) = result.unwrap();
            assert_eq!(
                inherited + delta.additional_storage(),
                profile.retained_storage()
            );
            assert_eq!(budget.peak_storage(), inherited + PROFILE_STORAGE);
        }
    }
}

#[test]
fn resealed_profiles_reject_bad_credentials_before_bad_nested_policy() {
    for version in [1, 2] {
        let mut bytes = profile_wire(version);
        bytes[16..20].fill(0);
        bytes[32] ^= 1;
        rehash_profile(&mut bytes, version);
        if version == 1 {
            assert!(matches!(
                LegacyProfile::decode(&bytes),
                Err(LegacyProfileError::InvalidSupervisorUid)
            ));
        } else {
            let mut work = Work::new(PROFILE_WORK);
            let mut budget = Budget::new(&mut work, PROFILE_BYTES + PROFILE_STORAGE);
            budget.reserve_storage(PROFILE_BYTES).unwrap();
            assert!(matches!(
                Profile::decode(&bytes, &mut budget),
                Err(ProfileError::Framing(
                    LegacyProfileError::InvalidSupervisorUid
                ))
            ));
        }
    }
}

#[test]
fn mixed_families_fail_in_both_directions_even_after_outer_rehash() {
    let mut work = Work::new(200_000);
    let mut budget = Budget::new(&mut work, 100_000);
    budget.reserve_storage(PROFILE_BYTES).unwrap();
    assert!(matches!(
        Policy::decode(&policy_wire(1), &mut budget),
        Err(PolicyError::Framing(LegacyError::InvalidMagic(_)))
    ));
    assert!(matches!(
        LegacyPolicy::decode(&policy_wire(2)),
        Err(LegacyError::InvalidMagic(_))
    ));
    assert!(matches!(
        Profile::decode(&profile_wire(1), &mut budget),
        Err(ProfileError::Framing(LegacyProfileError::Magic))
    ));
    assert!(matches!(
        LegacyProfile::decode(&profile_wire(2)),
        Err(LegacyProfileError::Magic)
    ));
    let mut mixed = profile_wire(2);
    mixed[32..248].copy_from_slice(&policy_wire(1));
    rehash_profile(&mut mixed, 2);
    assert!(matches!(
        Profile::decode(&mixed, &mut budget),
        Err(ProfileError::Policy(PolicyError::Framing(
            LegacyError::InvalidMagic(_)
        )))
    ));
    let mut mixed = profile_wire(1);
    mixed[32..248].copy_from_slice(&policy_wire(2));
    rehash_profile(&mut mixed, 1);
    assert!(matches!(
        LegacyProfile::decode(&mixed),
        Err(LegacyProfileError::Policy(LegacyError::InvalidMagic(_)))
    ));
    let mut mixed = policy_wire(2);
    mixed[176..178].copy_from_slice(&1u16.to_le_bytes());
    rehash_policy(&mut mixed, 2);
    assert!(matches!(
        Policy::decode(&mixed, &mut budget),
        Err(PolicyError::Framing(
            LegacyError::UnsupportedSubjectVersion(1)
        ))
    ));
    assert_eq!(budget.storage(), PROFILE_BYTES);
}

#[test]
fn every_single_byte_mutation_is_rejected() {
    let mut work = Work::new((POLICY_BYTES + PROFILE_BYTES) * PROFILE_WORK);
    let mut budget = Budget::new(&mut work, 100_000);
    budget.reserve_storage(PROFILE_BYTES).unwrap();
    for offset in 0..POLICY_BYTES {
        let mut bytes = policy_wire(2);
        bytes[offset] ^= 1;
        assert!(
            Policy::decode(&bytes, &mut budget).is_err(),
            "policy byte {offset}"
        );
        assert_eq!(budget.storage(), PROFILE_BYTES);
    }
    for offset in 0..PROFILE_BYTES {
        let mut bytes = profile_wire(2);
        bytes[offset] ^= 1;
        assert!(
            Profile::decode(&bytes, &mut budget).is_err(),
            "profile byte {offset}"
        );
        assert_eq!(budget.storage(), PROFILE_BYTES);
    }
}

#[test]
fn policy_exact_and_one_short_budgets() {
    for (work_limit, storage_limit) in [
        (POLICY_WORK, POLICY_BYTES + POLICY_STORAGE),
        (POLICY_WORK - 1, POLICY_BYTES + POLICY_STORAGE),
        (POLICY_WORK, POLICY_BYTES + POLICY_STORAGE - 1),
    ] {
        let mut work = Work::new(work_limit);
        let mut budget = Budget::new(&mut work, storage_limit);
        budget.reserve_storage(POLICY_BYTES).unwrap();
        let result = Policy::decode(&policy_wire(2), &mut budget);
        assert_eq!(budget.storage(), POLICY_BYTES);
        if work_limit < POLICY_WORK {
            assert!(matches!(
                result,
                Err(PolicyError::Resource(Resource::Work(_)))
            ));
            assert_eq!(budget.work(), 8);
        } else if storage_limit < POLICY_BYTES + POLICY_STORAGE {
            assert!(matches!(
                result,
                Err(PolicyError::Resource(Resource::Storage(_)))
            ));
            assert_eq!(budget.failed_storage(), Some(POLICY_BYTES + POLICY_STORAGE));
        } else {
            let (policy, charge) = result.unwrap();
            assert_eq!(charge.additional_storage(), policy.retained_storage());
            assert_eq!(budget.peak_storage(), POLICY_BYTES + POLICY_STORAGE);
        }
    }
}

#[test]
fn profile_decode_prepays_nested_crypto_and_returns_full_storage() {
    for (work_limit, storage_limit) in [
        (PROFILE_WORK, PROFILE_BYTES + PROFILE_STORAGE),
        (PROFILE_WORK - 1, PROFILE_BYTES + PROFILE_STORAGE),
        (PROFILE_WORK, PROFILE_BYTES + PROFILE_STORAGE - 1),
    ] {
        let mut work = Work::new(work_limit);
        let mut budget = Budget::new(&mut work, storage_limit);
        budget.reserve_storage(PROFILE_BYTES).unwrap();
        let result = Profile::decode(&profile_wire(2), &mut budget);
        assert_eq!(budget.storage(), PROFILE_BYTES);
        if work_limit < PROFILE_WORK {
            assert!(matches!(
                result,
                Err(ProfileError::Resource(Resource::Work(_)))
            ));
        } else if storage_limit < PROFILE_BYTES + PROFILE_STORAGE {
            assert!(matches!(
                result,
                Err(ProfileError::Resource(Resource::Storage(_)))
            ));
        } else {
            let (profile, charge) = result.unwrap();
            assert_eq!(charge.additional_storage(), profile.retained_storage());
            assert!(charge.additional_storage() > profile.policy().retained_storage());
            assert_eq!(budget.work(), PROFILE_WORK);
            assert_eq!(budget.peak_storage(), PROFILE_BYTES + PROFILE_STORAGE);
        }
    }
}

#[test]
fn nested_operations_cannot_reset_the_work_ledger() {
    let mut work = Work::new(POLICY_WORK + PROFILE_WORK - 1);
    let mut budget = Budget::new(&mut work, 100_000);
    let policy = native(&mut budget);
    let inherited = policy.retained_storage();
    assert!(matches!(
        Profile::new(1234, 5678, service(), policy, &mut budget),
        Err(ProfileError::Resource(Resource::Work(_)))
    ));
    assert_eq!(budget.work(), POLICY_WORK + 8);
    assert_eq!(budget.storage(), inherited);
    budget.release_storage(inherited).unwrap();
}

#[test]
fn unpaid_inputs_fail_before_decoding_or_consuming_construction() {
    let mut work = Work::new(100_000);
    let mut budget = Budget::new(&mut work, 100_000);
    assert!(matches!(
        Policy::decode(&policy_wire(2), &mut budget),
        Err(PolicyError::Resource(Resource::Accounting))
    ));
    assert!(matches!(
        Profile::decode(&profile_wire(2), &mut budget),
        Err(ProfileError::Resource(Resource::Accounting))
    ));
    let policy = native(&mut budget);
    budget.release_storage(policy.retained_storage()).unwrap();
    assert!(matches!(
        Profile::new(1234, 5678, service(), policy, &mut budget),
        Err(ProfileError::Resource(Resource::Accounting))
    ));
    assert_eq!(budget.storage(), 0);
}

#[test]
fn malformed_lengths_never_require_or_scan_an_unbounded_owner() {
    let mut work = Work::new(20 * PROFILE_WORK);
    let mut budget = Budget::new(&mut work, 100_000);
    for length in [0, 1, POLICY_BYTES - 1, POLICY_BYTES + 1, 100_000] {
        let bytes = vec![0; length];
        assert!(matches!(
            Policy::decode(&bytes, &mut budget),
            Err(PolicyError::Framing(LegacyError::InvalidLength { .. }))
        ));
    }
    for length in [0, 1, PROFILE_BYTES - 1, PROFILE_BYTES + 1, 100_000] {
        let bytes = vec![0; length];
        assert!(matches!(
            Profile::decode(&bytes, &mut budget),
            Err(ProfileError::Framing(LegacyProfileError::Length))
        ));
    }
    assert_eq!(budget.storage(), 0);
}

#[test]
fn stable_diagnostic_precedence_survives_shared_codecs() {
    for version in [1, 2] {
        let mut bytes = policy_wire(version);
        bytes[24..32].fill(0);
        bytes[178] = 1;
        bytes[184..].fill(0);
        let mut work = Work::new(10 * POLICY_WORK);
        let mut budget = Budget::new(&mut work, 100_000);
        budget.reserve_storage(POLICY_BYTES).unwrap();
        let check = |bytes: &[u8], budget: &mut Budget<'_>| {
            if version == 1 {
                LegacyPolicy::decode(bytes).map(|_| ())
            } else {
                Policy::decode(bytes, budget)
                    .map(|_| ())
                    .map_err(|e| match e {
                        PolicyError::Framing(e) => e,
                        _ => panic!("unexpected resource denial"),
                    })
            }
        };
        assert!(matches!(
            check(&bytes, &mut budget),
            Err(LegacyError::NonzeroReserved)
        ));
        bytes[178] = 0;
        assert!(matches!(
            check(&bytes, &mut budget),
            Err(LegacyError::ZeroValue("issuer policy"))
        ));
        rehash_policy(&mut bytes, version);
        assert!(matches!(
            check(&bytes, &mut budget),
            Err(LegacyError::ZeroValue("issuer policy generation"))
        ));
    }
}

#[test]
fn invalid_credentials_drop_consumed_policy_without_retiring_caller_reservation() {
    let mut work = Work::new(8 * (POLICY_WORK + PROFILE_WORK));
    let mut budget = Budget::new(&mut work, 100_000);
    for (uid, gid) in [(0, 5678), (u32::MAX, 5678), (1234, 0), (1234, u32::MAX)] {
        let policy = native(&mut budget);
        let inherited = policy.retained_storage();
        let error = Profile::new(uid, gid, service(), policy, &mut budget).unwrap_err();
        assert!(matches!(
            error,
            ProfileError::Framing(
                LegacyProfileError::InvalidSupervisorUid | LegacyProfileError::InvalidSupervisorGid
            )
        ));
        assert_eq!(budget.storage(), inherited);
        budget.release_storage(inherited).unwrap();
    }
}

#[test]
fn weak_and_equal_keys_reject_even_with_recomputed_identities() {
    let mut work = Work::new(100_000);
    let mut budget = Budget::new(&mut work, 100_000);
    budget.reserve_storage(POLICY_BYTES).unwrap();
    let mut weak = policy_wire(2);
    weak[112..144].fill(0);
    rehash_policy(&mut weak, 2);
    assert!(matches!(
        Policy::decode(&weak, &mut budget),
        Err(PolicyError::Framing(LegacyError::WeakVerifyingKey))
    ));
    let mut equal = policy_wire(2);
    equal[144..176].copy_from_slice(&issuer_key());
    rehash_policy(&mut equal, 2);
    assert!(matches!(
        Policy::decode(&equal, &mut budget),
        Err(PolicyError::Framing(LegacyError::NonDistinctVerifyingKeys))
    ));
}
