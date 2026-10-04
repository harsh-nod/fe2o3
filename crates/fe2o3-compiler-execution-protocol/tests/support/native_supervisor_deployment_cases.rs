fn policy(variation: u8, budget: &mut Budget<'_>) -> Policy {
    let (policy, charge) = Policy::new(
        if variation == 1 { 8 } else { 7 },
        measurement(
            if variation == 2 { 0x63 } else { 0x61 },
            if variation == 3 { 12346 } else { 12345 },
        ),
        measurement(
            if variation == 4 { 0x64 } else { 0x62 },
            if variation == 5 { 67891 } else { 67890 },
        ),
        key(if variation == 6 { 0x53 } else { 0x51 }),
        key(if variation == 7 { 0x54 } else { 0x52 }),
        budget,
    )
    .unwrap();
    assert_eq!(charge.additional_storage(), policy.retained_storage());
    budget.reserve_storage(charge.additional_storage()).unwrap();
    policy
}

fn deployment(policy: &Policy, budget: &mut Budget<'_>) -> Deployment {
    let floor = budget.storage();
    let (owner, charge) = Deployment::new(
        1234,
        5678,
        service(),
        measurement(0x71, 4096),
        measurement(0x72, 8192),
        policy,
        budget,
    )
    .unwrap();
    assert_eq!(budget.storage(), floor);
    assert_eq!(charge.additional_storage(), owner.retained_storage());
    assert_eq!(
        owner.retained_storage(),
        std::mem::size_of::<(Deployment, Storage)>()
    );
    budget.reserve_storage(charge.additional_storage()).unwrap();
    owner
}

#[test]
fn independent_transcripts_round_trip_on_the_original_ledger() {
    assert_eq!((BYTES, WORK), (184, 5896));
    let legacy_policy = legacy_policy();
    let legacy = legacy_deployment(&legacy_policy);
    assert_eq!(
        legacy.canonical_bytes(),
        &wire(1, legacy_policy.identity().as_bytes())
    );
    assert_eq!(
        Legacy::decode(legacy.canonical_bytes()).unwrap(),
        legacy.clone()
    );

    let mut work = Work::new(POLICY_WORK + 4 * WORK);
    let mut budget = Budget::new(&mut work, LIMIT);
    budget.reserve_storage(EXTRA).unwrap();
    let retained = {
        let policy = policy(0, &mut budget);
        let owner = deployment(&policy, &mut budget);
        assert_eq!(
            owner.canonical_bytes(),
            &wire(VERSION, policy.identity().as_bytes())
        );
        assert_eq!(owner.service_uid(), 1234);
        assert_eq!(owner.service_gid(), 5678);
        assert_eq!(owner.external_anchor_service(), service());
        assert_eq!(owner.executable(), measurement(0x71, 4096));
        assert_eq!(owner.launcher(), measurement(0x72, 8192));
        assert_eq!(owner.policy_identity(), policy.identity());
        assert_ne!(owner.identity().as_bytes(), legacy.identity().as_bytes());
        assert!(matches!(
            Legacy::decode(owner.canonical_bytes()),
            Err(Framing::Magic)
        ));
        let floor = budget.storage();
        assert!(owner.matches_policy(&policy, &mut budget).unwrap());
        assert!(
            owner
                .identity()
                .matches_canonical_bytes(owner.canonical_bytes(), &mut budget)
                .unwrap()
        );
        let (decoded, charge) =
            Deployment::decode(owner.canonical_bytes(), &policy, &mut budget).unwrap();
        assert_eq!(budget.storage(), floor);
        assert_eq!(charge.additional_storage(), decoded.retained_storage());
        budget.reserve_storage(charge.additional_storage()).unwrap();
        assert_eq!(owner, decoded);
        assert_eq!(budget.work(), POLICY_WORK + 4 * WORK);
        policy.retained_storage() + owner.retained_storage() + decoded.retained_storage()
    };
    budget.release_storage(retained).unwrap();
    assert_eq!(budget.storage(), EXTRA);
}

#[test]
fn every_single_byte_mutation_is_rejected_by_decode_and_identity_matching() {
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    let policy = policy(0, &mut budget);
    let owner = deployment(&policy, &mut budget);
    budget.reserve_storage(BYTES).unwrap();
    let floor = budget.storage();
    for offset in 0..BYTES {
        let mut bytes = *owner.canonical_bytes();
        bytes[offset] ^= 1;
        assert!(
            Deployment::decode(&bytes, &policy, &mut budget).is_err(),
            "byte {offset}"
        );
        assert!(
            !owner
                .identity()
                .matches_canonical_bytes(&bytes, &mut budget)
                .unwrap(),
            "byte {offset}"
        );
        assert_eq!(budget.storage(), floor);
    }
    assert_eq!(budget.work(), POLICY_WORK + (1 + 2 * BYTES) * WORK);
}

#[test]
fn resealed_headers_credentials_and_measurements_still_require_valid_fields() {
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    let policy = policy(0, &mut budget);
    let good = wire(VERSION, policy.identity().as_bytes());
    budget.reserve_storage(BYTES).unwrap();
    let floor = budget.storage();
    let mut check = |mut bytes: [u8; BYTES], expected: Framing| {
        reseal(&mut bytes, VERSION);
        let error = Deployment::decode(&bytes, &policy, &mut budget).unwrap_err();
        assert!(
            matches!(error, Error::Framing(e) if e == expected),
            "expected {expected:?}"
        );
        assert_eq!(budget.storage(), floor);
    };
    for offset in [10, 11, 16, 17, 18, 19, 20, 21, 22, 23] {
        let mut bytes = good;
        bytes[offset] = 1;
        check(bytes, Framing::Reserved);
    }
    for version in [0, 1, OTHER_VERSION, u16::MAX] {
        let mut bytes = good;
        bytes[8..10].copy_from_slice(&version.to_le_bytes());
        check(bytes, Framing::Version);
    }
    for length in [0u32, 183, 185, u32::MAX] {
        let mut bytes = good;
        bytes[12..16].copy_from_slice(&length.to_le_bytes());
        check(bytes, Framing::Length);
    }
    for invalid in [0u32, u32::MAX] {
        for (offset, expected) in [
            (24, Framing::ServiceUid),
            (28, Framing::ServiceGid),
            (
                32,
                Framing::ExternalAnchorServiceIdentity(ServiceError::InvalidUid),
            ),
            (
                36,
                Framing::ExternalAnchorServiceIdentity(ServiceError::InvalidGid),
            ),
        ] {
            let mut bytes = good;
            bytes[offset..offset + 4].copy_from_slice(&invalid.to_le_bytes());
            check(bytes, expected);
        }
    }
    let mut bytes = good;
    bytes[24..28].copy_from_slice(&service().uid().to_le_bytes());
    check(bytes, Framing::SharedServiceUid);
    for (offset, expected) in [
        (40, Framing::ExecutableMeasurement),
        (80, Framing::LauncherMeasurement),
    ] {
        let mut bytes = good;
        bytes[offset..offset + 32].fill(0);
        check(bytes, expected);
    }
    for (offset, maximum, expected) in [
        (72, MAX_EXECUTABLE, Framing::ExecutableMeasurement),
        (112, MAX_LAUNCHER, Framing::LauncherMeasurement),
    ] {
        for invalid in [0, maximum + 1, u64::MAX] {
            let mut bytes = good;
            bytes[offset..offset + 8].copy_from_slice(&invalid.to_le_bytes());
            check(bytes, expected.clone());
        }
    }
    let mut bytes = good;
    bytes.copy_within(40..80, 80);
    check(bytes, Framing::AliasedExecutableMeasurements);
    let mut bytes = good;
    bytes[120..152].fill(0);
    check(bytes, Framing::PolicyIdentity);
}

#[test]
fn native_policy_binding_includes_every_actual_policy_field() {
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    let pinned = policy(0, &mut budget);
    let owner = deployment(&pinned, &mut budget);
    budget.reserve_storage(BYTES).unwrap();
    for variation in 1..=7 {
        let other = policy(variation, &mut budget);
        assert!(
            !owner.matches_policy(&other, &mut budget).unwrap(),
            "policy variation {variation}"
        );
        assert!(matches!(
            Deployment::decode(owner.canonical_bytes(), &other, &mut budget),
            Err(Error::PolicyMismatch)
        ));
        let changed = deployment(&other, &mut budget);
        assert_ne!(owner.identity(), changed.identity());
        assert_eq!(changed.policy_identity(), other.identity());
        let mut resealed = *owner.canonical_bytes();
        resealed[120..152].copy_from_slice(other.identity().as_bytes());
        reseal(&mut resealed, VERSION);
        assert_eq!(&resealed, changed.canonical_bytes());
        assert!(matches!(
            Deployment::decode(&resealed, &pinned, &mut budget),
            Err(Error::PolicyMismatch)
        ));
        let released = other.retained_storage() + changed.retained_storage();
        drop(changed);
        drop(other);
        budget.release_storage(released).unwrap();
    }
}

#[test]
fn foreign_families_cannot_be_relabelled_or_resealed_into_a_native_policy_binding() {
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    let policy = policy(0, &mut budget);
    let (other, charge) = OtherPolicy::new(
        7,
        measurement(0x61, 12345),
        measurement(0x62, 67890),
        key(0x51),
        key(0x52),
        &mut budget,
    )
    .unwrap();
    budget.reserve_storage(charge.additional_storage()).unwrap();
    let legacy_policy = legacy_policy();
    let owner = deployment(&policy, &mut budget);
    budget.reserve_storage(BYTES).unwrap();
    for (foreign_version, foreign_identity) in [
        (1, *legacy_policy.identity().as_bytes()),
        (OTHER_VERSION, *other.identity().as_bytes()),
    ] {
        assert_ne!(&foreign_identity, policy.identity().as_bytes());
        let mut bytes = wire(foreign_version, &foreign_identity);
        assert!(matches!(
            Deployment::decode(&bytes, &policy, &mut budget),
            Err(Error::Framing(Framing::Magic))
        ));
        assert!(
            !owner
                .identity()
                .matches_canonical_bytes(&bytes, &mut budget)
                .unwrap()
        );
        bytes[..10].copy_from_slice(&owner.canonical_bytes()[..10]);
        reseal(&mut bytes, VERSION);
        assert!(matches!(
            Deployment::decode(&bytes, &policy, &mut budget),
            Err(Error::PolicyMismatch)
        ));
        assert!(
            !owner
                .identity()
                .matches_canonical_bytes(&bytes, &mut budget)
                .unwrap()
        );
    }
    // The hash domain is independent of the magic/version check.
    let mut wrong_domain = *owner.canonical_bytes();
    reseal(&mut wrong_domain, OTHER_VERSION);
    assert!(matches!(
        Deployment::decode(&wrong_domain, &policy, &mut budget),
        Err(Error::Framing(Framing::Identity))
    ));
}

#[test]
fn valid_resealing_changes_identity_but_never_authenticates_provisioning() {
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    let policy = policy(0, &mut budget);
    let owner = deployment(&policy, &mut budget);
    budget.reserve_storage(BYTES).unwrap();
    // Every bound credential, digest and length can be publicly reconstructed as
    // different inert configuration. Only a trusted caller can pin which is intended.
    for offset in [24, 28, 32, 36, 40, 72, 80, 112] {
        let mut bytes = *owner.canonical_bytes();
        bytes[offset] ^= 1;
        reseal(&mut bytes, VERSION);
        let (changed, charge) = Deployment::decode(&bytes, &policy, &mut budget).unwrap();
        budget.reserve_storage(charge.additional_storage()).unwrap();
        assert_ne!(owner.identity(), changed.identity());
        assert!(changed.matches_policy(&policy, &mut budget).unwrap());
        let retained = changed.retained_storage();
        drop(changed);
        budget.release_storage(retained).unwrap();
    }
}

#[test]
fn constructor_rejections_and_exact_measurement_limits_are_prepaid() {
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    let policy = policy(0, &mut budget);
    let floor = budget.storage();
    for (uid, gid, executable, launcher, expected) in [
        (
            0,
            5678,
            measurement(0x71, 4096),
            measurement(0x72, 8192),
            Framing::ServiceUid,
        ),
        (
            u32::MAX,
            5678,
            measurement(0x71, 4096),
            measurement(0x72, 8192),
            Framing::ServiceUid,
        ),
        (
            1234,
            0,
            measurement(0x71, 4096),
            measurement(0x72, 8192),
            Framing::ServiceGid,
        ),
        (
            1234,
            u32::MAX,
            measurement(0x71, 4096),
            measurement(0x72, 8192),
            Framing::ServiceGid,
        ),
        (
            6001,
            5678,
            measurement(0x71, 4096),
            measurement(0x72, 8192),
            Framing::SharedServiceUid,
        ),
        (
            1234,
            5678,
            measurement(0x71, MAX_EXECUTABLE + 1),
            measurement(0x72, 8192),
            Framing::ExecutableMeasurement,
        ),
        (
            1234,
            5678,
            measurement(0x71, 4096),
            measurement(0x72, MAX_LAUNCHER + 1),
            Framing::LauncherMeasurement,
        ),
        (
            1234,
            5678,
            measurement(0x71, 4096),
            measurement(0x71, 4096),
            Framing::AliasedExecutableMeasurements,
        ),
    ] {
        let before = budget.work();
        assert!(
            matches!(Deployment::new(uid, gid, service(), executable, launcher, &policy, &mut budget), Err(Error::Framing(e)) if e == expected)
        );
        assert_eq!((budget.work(), budget.storage()), (before + WORK, floor));
    }
    let (owner, charge) = Deployment::new(
        u32::MAX - 1,
        1,
        Service::new(1, u32::MAX - 1).unwrap(),
        measurement(0x71, MAX_EXECUTABLE),
        measurement(0x72, MAX_LAUNCHER),
        &policy,
        &mut budget,
    )
    .unwrap();
    budget.reserve_storage(charge.additional_storage()).unwrap();
    let (decoded, charge) =
        Deployment::decode(owner.canonical_bytes(), &policy, &mut budget).unwrap();
    budget.reserve_storage(charge.additional_storage()).unwrap();
    assert_eq!(decoded, owner);
}

#[test]
fn all_working_operations_enforce_exact_work_scratch_and_input_floors() {
    // Fixtures are admitted separately; each isolated operation below explicitly
    // prepays all its own borrowed inputs on the ledger under test.
    let mut fixture_work = Work::new(LIMIT);
    let mut fixture_budget = Budget::new(&mut fixture_work, LIMIT);
    let policy = policy(0, &mut fixture_budget);
    let owner = deployment(&policy, &mut fixture_budget);
    let bytes = *owner.canonical_bytes();
    for operation in 0..4 {
        let input = match operation {
            0 => policy.retained_storage(),
            1 => policy.retained_storage() + BYTES,
            2 => BYTES,
            3 => policy.retained_storage() + owner.retained_storage(),
            _ => unreachable!(),
        };
        for mode in 0..5 {
            let floor = if mode == 3 { input - 1 } else { input + EXTRA };
            let work_limit = match mode {
                1 => WORK - 1,
                4 => 7,
                _ => WORK,
            };
            let mut work = Work::new(work_limit);
            let mut budget = Budget::new(&mut work, floor + STORAGE - usize::from(mode == 2));
            budget.reserve_storage(floor).unwrap();
            let result: Result<usize, Error> = match operation {
                0 => Deployment::new(
                    1234,
                    5678,
                    service(),
                    measurement(0x71, 4096),
                    measurement(0x72, 8192),
                    &policy,
                    &mut budget,
                )
                .map(|(value, charge)| {
                    assert_eq!(value, owner);
                    charge.additional_storage()
                }),
                1 => Deployment::decode(&bytes, &policy, &mut budget).map(|(value, charge)| {
                    assert_eq!(value, owner);
                    charge.additional_storage()
                }),
                2 => owner
                    .identity()
                    .matches_canonical_bytes(&bytes, &mut budget)
                    .map(|matched| {
                        assert!(matched);
                        0
                    }),
                3 => owner.matches_policy(&policy, &mut budget).map(|matched| {
                    assert!(matched);
                    0
                }),
                _ => unreachable!(),
            };
            assert_eq!(budget.storage(), floor);
            match mode {
                0 => {
                    assert_eq!(
                        result.unwrap(),
                        if operation < 2 {
                            owner.retained_storage()
                        } else {
                            0
                        }
                    );
                    assert_eq!(
                        (budget.work(), budget.peak_storage()),
                        (WORK, floor + STORAGE)
                    );
                }
                1 | 4 => {
                    assert!(matches!(result, Err(Error::Resource(Resource::Work(_)))));
                    assert_eq!(budget.work(), if mode == 1 { 8 } else { 0 });
                    assert_eq!(budget.failed_work(), Some(if mode == 1 { WORK } else { 8 }));
                    assert_eq!(budget.peak_storage(), floor);
                }
                2 => {
                    assert!(matches!(result, Err(Error::Resource(Resource::Storage(_)))));
                    assert_eq!(
                        (budget.work(), budget.failed_storage()),
                        (WORK, Some(floor + STORAGE))
                    );
                    assert_eq!(budget.peak_storage(), floor);
                }
                3 => {
                    assert!(matches!(result, Err(Error::Resource(Resource::Accounting))));
                    assert_eq!((budget.work(), budget.peak_storage()), (8, floor));
                }
                _ => unreachable!(),
            }
        }
    }
}

#[test]
fn wrong_lengths_do_not_scan_or_require_the_wire_but_still_charge_resources() {
    let mut fixture_work = Work::new(LIMIT);
    let mut fixture_budget = Budget::new(&mut fixture_work, LIMIT);
    let policy = policy(0, &mut fixture_budget);
    let owner = deployment(&policy, &mut fixture_budget);
    for length in [0, 1, BYTES - 1, BYTES + 1, 100_000] {
        let bytes = vec![0; length];
        let floor = policy.retained_storage();
        let mut work = Work::new(WORK);
        let mut budget = Budget::new(&mut work, floor + STORAGE);
        budget.reserve_storage(floor).unwrap();
        assert!(matches!(
            Deployment::decode(&bytes, &policy, &mut budget),
            Err(Error::Framing(Framing::Length))
        ));
        assert_eq!((budget.work(), budget.storage()), (WORK, floor));
        let mut work = Work::new(WORK);
        let mut budget = Budget::new(&mut work, STORAGE);
        assert!(
            !owner
                .identity()
                .matches_canonical_bytes(&bytes, &mut budget)
                .unwrap()
        );
        assert_eq!((budget.work(), budget.storage()), (WORK, 0));
    }
}

#[test]
fn cumulative_work_and_denial_history_survive_success_and_semantic_errors() {
    let total = POLICY_WORK + 3 * WORK;
    let mut work = Work::new(total);
    let mut budget = Budget::new(&mut work, LIMIT);
    assert!(budget.charge_work(total + 1).is_err());
    assert!(budget.reserve_storage(LIMIT + 1).is_err());
    let policy = policy(0, &mut budget);
    let owner = deployment(&policy, &mut budget);
    let floor = budget.storage();
    assert!(owner.matches_policy(&policy, &mut budget).unwrap());
    assert!(matches!(
        Deployment::decode(&[], &policy, &mut budget),
        Err(Error::Framing(Framing::Length))
    ));
    assert_eq!((budget.work(), budget.storage()), (total, floor));
    assert!(matches!(
        owner.matches_policy(&policy, &mut budget),
        Err(Error::Resource(Resource::Work(_)))
    ));
    assert_eq!(budget.failed_work(), Some(total + 1));
    assert_eq!(budget.failed_storage(), Some(LIMIT + 1));
    assert_eq!(budget.storage(), floor);
}

#[test]
fn checked_scratch_reservation_rejects_overflow_without_clearing_the_input_floor() {
    let mut fixture_work = Work::new(LIMIT);
    let mut fixture_budget = Budget::new(&mut fixture_work, LIMIT);
    let policy = policy(0, &mut fixture_budget);
    let bytes = wire(VERSION, policy.identity().as_bytes());
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, usize::MAX);
    let floor = usize::MAX - STORAGE + 1;
    budget.reserve_storage(floor).unwrap();
    assert!(matches!(
        Deployment::decode(&bytes, &policy, &mut budget),
        Err(Error::Resource(Resource::Storage(_)))
    ));
    assert_eq!((budget.work(), budget.storage()), (WORK, floor));
    assert_eq!(budget.failed_storage(), Some(usize::MAX));
    assert_eq!(budget.peak_storage(), floor);
}
