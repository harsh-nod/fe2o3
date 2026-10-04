fn policy(variation: u8, budget: &mut Budget<'_>) -> Policy {
    let (owner, charge) = Policy::new(
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
    assert_eq!(charge.additional_storage(), owner.retained_storage());
    budget.reserve_storage(charge.additional_storage()).unwrap();
    owner
}

fn supervisor(policy: &Policy, variation: u8, budget: &mut Budget<'_>) -> Supervisor {
    let (owner, charge) = Supervisor::new(
        if variation == 1 { 1235 } else { 1234 },
        if variation == 2 { 5679 } else { 5678 },
        Service::new(
            if variation == 3 { 6002 } else { 6001 },
            if variation == 4 { 7002 } else { 7001 },
        )
        .unwrap(),
        measurement(
            if variation == 5 { 0x74 } else { 0x71 },
            if variation == 6 { 4097 } else { 4096 },
        ),
        measurement(
            if variation == 7 { 0x75 } else { 0x72 },
            if variation == 8 { 8193 } else { 8192 },
        ),
        policy,
        budget,
    )
    .unwrap();
    assert_eq!(charge.additional_storage(), owner.retained_storage());
    budget.reserve_storage(charge.additional_storage()).unwrap();
    owner
}

fn deployment(supervisor: &Supervisor, policy: &Policy, budget: &mut Budget<'_>) -> Deployment {
    let floor = budget.storage();
    let before = budget.work();
    let (owner, charge) = Deployment::new(supervisor, policy, executable(), budget).unwrap();
    assert_eq!((budget.storage(), budget.work()), (floor, before + WORK));
    assert_eq!(charge.additional_storage(), owner.retained_storage());
    assert_eq!(
        owner.retained_storage(),
        std::mem::size_of::<(Deployment, Storage)>()
    );
    budget.reserve_storage(charge.additional_storage()).unwrap();
    owner
}

#[test]
fn independent_transcripts_round_trip_and_retire_exact_full_owner_charges() {
    assert_eq!(
        (BYTES, WORK, MAX_EXECUTABLE),
        (168, 11280, 128 * 1024 * 1024)
    );
    let (legacy_supervisor, legacy_policy) = legacy_context();
    let legacy = Legacy::new(&legacy_supervisor, &legacy_policy, executable()).unwrap();
    assert_eq!(
        legacy.canonical_bytes(),
        &wire(1, legacy_supervisor.identity().as_bytes())
    );
    assert_eq!(Legacy::decode(legacy.canonical_bytes()).unwrap(), legacy);
    let total = POLICY_WORK + SUPERVISOR_WORK + 5 * WORK;
    let mut work = Work::new(total);
    let mut budget = Budget::new(&mut work, LIMIT);
    budget.reserve_storage(EXTRA).unwrap();
    let retained = {
        let policy = policy(0, &mut budget);
        let supervisor = supervisor(&policy, 0, &mut budget);
        let owner = deployment(&supervisor, &policy, &mut budget);
        assert_eq!(
            owner.canonical_bytes(),
            &wire(VERSION, supervisor.identity().as_bytes())
        );
        assert_eq!(owner.service(), service());
        assert_eq!(
            owner.verifying_key(),
            policy.external_anchor_verifying_key()
        );
        assert_eq!(
            owner.supervisor_deployment_identity(),
            supervisor.identity()
        );
        assert_eq!(owner.executable(), executable());
        assert_ne!(owner.identity().as_bytes(), legacy.identity().as_bytes());
        assert!(matches!(
            Legacy::decode(owner.canonical_bytes()),
            Err(Framing::Magic)
        ));
        fn copy_identity<T: Copy + Eq>(id: T) -> (T, T) {
            (id, id)
        }
        let ids: (Identity, Identity) = copy_identity(owner.identity());
        assert_eq!(ids.0, ids.1);
        let floor = budget.storage();
        assert!(
            owner
                .matches_supervisor_and_policy(&supervisor, &policy, &mut budget)
                .unwrap()
        );
        assert!(
            owner
                .matches_supervisor_policy_and_executable(
                    &supervisor,
                    &policy,
                    executable(),
                    &mut budget
                )
                .unwrap()
        );
        assert!(
            ids.0
                .matches_canonical_bytes(owner.canonical_bytes(), &supervisor, &policy, &mut budget)
                .unwrap()
        );
        let (decoded, charge) =
            Deployment::decode(owner.canonical_bytes(), &supervisor, &policy, &mut budget).unwrap();
        assert_eq!(budget.storage(), floor);
        assert_eq!(charge.additional_storage(), decoded.retained_storage());
        budget.reserve_storage(charge.additional_storage()).unwrap();
        assert_eq!(owner, decoded);
        assert_eq!(budget.work(), total);
        policy.retained_storage()
            + supervisor.retained_storage()
            + owner.retained_storage()
            + decoded.retained_storage()
    };
    budget.release_storage(retained).unwrap();
    assert_eq!(budget.storage(), EXTRA);
}

#[test]
fn every_byte_mutation_rejects_without_resetting_work_or_storage() {
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    let policy = policy(0, &mut budget);
    let supervisor = supervisor(&policy, 0, &mut budget);
    let owner = deployment(&supervisor, &policy, &mut budget);
    budget.reserve_storage(BYTES).unwrap();
    let floor = budget.storage();
    for offset in 0..BYTES {
        let mut bytes = *owner.canonical_bytes();
        bytes[offset] ^= 1;
        assert!(
            Deployment::decode(&bytes, &supervisor, &policy, &mut budget).is_err(),
            "byte {offset}"
        );
        assert!(
            !owner
                .identity()
                .matches_canonical_bytes(&bytes, &supervisor, &policy, &mut budget)
                .unwrap(),
            "byte {offset}"
        );
        assert_eq!(budget.storage(), floor);
    }
    assert_eq!(
        budget.work(),
        POLICY_WORK + SUPERVISOR_WORK + (1 + 2 * BYTES) * WORK
    );
}

#[test]
fn resealed_headers_credentials_and_measurements_require_valid_fields() {
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    let policy = policy(0, &mut budget);
    let supervisor = supervisor(&policy, 0, &mut budget);
    let good = wire(VERSION, supervisor.identity().as_bytes());
    budget.reserve_storage(BYTES).unwrap();
    let floor = budget.storage();
    let mut check = |mut bytes: [u8; BYTES], expected: Framing| {
        reseal(&mut bytes, VERSION);
        let before = budget.work();
        assert!(
            matches!(Deployment::decode(&bytes, &supervisor, &policy, &mut budget), Err(Error::Framing(e)) if e == expected),
            "expected {expected:?}"
        );
        assert_eq!((budget.storage(), budget.work()), (floor, before + WORK));
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
    for length in [0u32, 167, 169, u32::MAX] {
        let mut bytes = good;
        bytes[12..16].copy_from_slice(&length.to_le_bytes());
        check(bytes, Framing::Length);
    }
    for invalid in [0u32, u32::MAX] {
        for (offset, error) in [
            (24, ServiceError::InvalidUid),
            (28, ServiceError::InvalidGid),
        ] {
            let mut bytes = good;
            bytes[offset..offset + 4].copy_from_slice(&invalid.to_le_bytes());
            check(bytes, Framing::ServiceIdentity(error));
        }
    }
    for (offset, error) in [
        (32, Framing::VerifyingKey),
        (64, Framing::SupervisorIdentity),
        (96, Framing::ExecutableMeasurement),
    ] {
        let mut bytes = good;
        bytes[offset..offset + 32].fill(0);
        check(bytes, error);
    }
    for length in [0u64, MAX_EXECUTABLE + 1, u64::MAX] {
        let mut bytes = good;
        bytes[128..136].copy_from_slice(&length.to_le_bytes());
        check(bytes, Framing::ExecutableMeasurement);
    }
}

#[test]
fn resealed_context_substitutions_and_hostile_keys_fail_closed() {
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    let policy = policy(0, &mut budget);
    let supervisor = supervisor(&policy, 0, &mut budget);
    let owner = deployment(&supervisor, &policy, &mut budget);
    budget.reserve_storage(BYTES).unwrap();
    for offset in [24, 28, 32, 64] {
        let mut bytes = *owner.canonical_bytes();
        bytes[offset] ^= 1;
        reseal(&mut bytes, VERSION);
        assert!(matches!(
            Deployment::decode(&bytes, &supervisor, &policy, &mut budget),
            Err(Error::ContextMismatch)
        ));
        assert!(
            !owner
                .identity()
                .matches_canonical_bytes(&bytes, &supervisor, &policy, &mut budget)
                .unwrap()
        );
    }
    let mut weak = [0; 32];
    weak[0] = 1;
    for hostile_key in [
        [0; 32],
        weak,
        [0xff; 32],
        key(0x55),
        *policy.verifying_key(),
    ] {
        let mut bytes = *owner.canonical_bytes();
        bytes[32..64].copy_from_slice(&hostile_key);
        reseal(&mut bytes, VERSION);
        assert!(Deployment::decode(&bytes, &supervisor, &policy, &mut budget).is_err());
        assert!(
            !owner
                .identity()
                .matches_canonical_bytes(&bytes, &supervisor, &policy, &mut budget)
                .unwrap()
        );
    }
}

#[test]
fn all_policy_axes_require_actual_supervisor_policy_agreement() {
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    let pinned = policy(0, &mut budget);
    let original_supervisor = supervisor(&pinned, 0, &mut budget);
    let owner = deployment(&original_supervisor, &pinned, &mut budget);
    budget.reserve_storage(BYTES).unwrap();
    for variation in 1..=7 {
        let other = policy(variation, &mut budget);
        let other_supervisor = supervisor(&other, 0, &mut budget);
        let before = budget.work();
        assert!(matches!(
            Deployment::new(&original_supervisor, &other, executable(), &mut budget),
            Err(Error::SupervisorPolicyMismatch)
        ));
        assert!(matches!(
            Deployment::decode(
                owner.canonical_bytes(),
                &original_supervisor,
                &other,
                &mut budget
            ),
            Err(Error::SupervisorPolicyMismatch)
        ));
        assert!(
            !owner
                .matches_supervisor_and_policy(&original_supervisor, &other, &mut budget)
                .unwrap()
        );
        assert!(
            !owner
                .matches_supervisor_policy_and_executable(
                    &original_supervisor,
                    &other,
                    executable(),
                    &mut budget
                )
                .unwrap()
        );
        assert!(
            !owner
                .identity()
                .matches_canonical_bytes(
                    owner.canonical_bytes(),
                    &original_supervisor,
                    &other,
                    &mut budget
                )
                .unwrap()
        );
        assert_eq!(budget.work(), before + 5 * WORK);
        assert!(matches!(
            Deployment::decode(
                owner.canonical_bytes(),
                &other_supervisor,
                &other,
                &mut budget
            ),
            Err(Error::ContextMismatch)
        ));
        assert!(
            !owner
                .matches_supervisor_and_policy(&other_supervisor, &other, &mut budget)
                .unwrap()
        );
        let changed = deployment(&other_supervisor, &other, &mut budget);
        assert_ne!(owner.identity(), changed.identity());
        let mut bytes = *owner.canonical_bytes();
        bytes[64..96].copy_from_slice(other_supervisor.identity().as_bytes());
        bytes[32..64].copy_from_slice(other.external_anchor_verifying_key());
        reseal(&mut bytes, VERSION);
        assert_eq!(&bytes, changed.canonical_bytes());
        assert!(matches!(
            Deployment::decode(&bytes, &original_supervisor, &pinned, &mut budget),
            Err(Error::ContextMismatch)
        ));
        let retained = changed.retained_storage()
            + other_supervisor.retained_storage()
            + other.retained_storage();
        drop(changed);
        drop(other_supervisor);
        drop(other);
        budget.release_storage(retained).unwrap();
    }
}

#[test]
fn complete_supervisor_identity_binds_every_supervisor_axis() {
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    let policy = policy(0, &mut budget);
    let original = supervisor(&policy, 0, &mut budget);
    let owner = deployment(&original, &policy, &mut budget);
    budget.reserve_storage(BYTES).unwrap();
    for variation in 1..=8 {
        let other = supervisor(&policy, variation, &mut budget);
        assert!(other.matches_policy(&policy, &mut budget).unwrap());
        assert!(
            !owner
                .matches_supervisor_and_policy(&other, &policy, &mut budget)
                .unwrap()
        );
        assert!(matches!(
            Deployment::decode(owner.canonical_bytes(), &other, &policy, &mut budget),
            Err(Error::ContextMismatch)
        ));
        let changed = deployment(&other, &policy, &mut budget);
        assert_ne!(owner.identity(), changed.identity());
        let mut bytes = *changed.canonical_bytes();
        // Restoring only the claimed supervisor ID cannot repair changed anchor credentials.
        if variation == 3 || variation == 4 {
            bytes[64..96].copy_from_slice(original.identity().as_bytes());
            reseal(&mut bytes, VERSION);
        }
        assert!(matches!(
            Deployment::decode(&bytes, &original, &policy, &mut budget),
            Err(Error::ContextMismatch)
        ));
        let retained = changed.retained_storage() + other.retained_storage();
        drop(changed);
        drop(other);
        budget.release_storage(retained).unwrap();
    }
}

#[test]
fn foreign_families_cannot_be_relabelled_or_resealed_into_native_context() {
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    let policy = policy(0, &mut budget);
    let supervisor = supervisor(&policy, 0, &mut budget);
    let owner = deployment(&supervisor, &policy, &mut budget);
    let (other_policy, charge) = OtherPolicy::new(
        7,
        measurement(0x61, 12345),
        measurement(0x62, 67890),
        key(0x51),
        key(0x52),
        &mut budget,
    )
    .unwrap();
    budget.reserve_storage(charge.additional_storage()).unwrap();
    let (other_supervisor, charge) = OtherSupervisor::new(
        1234,
        5678,
        service(),
        measurement(0x71, 4096),
        measurement(0x72, 8192),
        &other_policy,
        &mut budget,
    )
    .unwrap();
    budget.reserve_storage(charge.additional_storage()).unwrap();
    let (other, charge) =
        OtherDeployment::new(&other_supervisor, &other_policy, executable(), &mut budget).unwrap();
    budget.reserve_storage(charge.additional_storage()).unwrap();
    let (legacy_supervisor, legacy_policy) = legacy_context();
    let legacy = Legacy::new(&legacy_supervisor, &legacy_policy, executable()).unwrap();
    budget
        .reserve_storage(std::mem::size_of_val(&legacy) + BYTES)
        .unwrap();
    assert_eq!(
        other.canonical_bytes(),
        &wire(OTHER_VERSION, other_supervisor.identity().as_bytes())
    );
    assert_ne!(other.identity().as_bytes(), owner.identity().as_bytes());
    for foreign in [legacy.canonical_bytes(), other.canonical_bytes()] {
        assert!(matches!(
            Deployment::decode(foreign, &supervisor, &policy, &mut budget),
            Err(Error::Framing(Framing::Magic))
        ));
        let mut bytes = *foreign;
        bytes[..10].copy_from_slice(&owner.canonical_bytes()[..10]);
        reseal(&mut bytes, VERSION);
        assert!(matches!(
            Deployment::decode(&bytes, &supervisor, &policy, &mut budget),
            Err(Error::ContextMismatch)
        ));
        assert!(
            !owner
                .identity()
                .matches_canonical_bytes(&bytes, &supervisor, &policy, &mut budget)
                .unwrap()
        );
    }
    for foreign_version in [1, OTHER_VERSION] {
        let mut bytes = *owner.canonical_bytes();
        reseal(&mut bytes, foreign_version);
        assert!(matches!(
            Deployment::decode(&bytes, &supervisor, &policy, &mut budget),
            Err(Error::Framing(Framing::Identity))
        ));
    }
}

#[test]
fn executable_limits_are_inclusive_and_matching_checks_both_digest_and_length() {
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    let policy = policy(0, &mut budget);
    let supervisor = supervisor(&policy, 0, &mut budget);
    let owner = deployment(&supervisor, &policy, &mut budget);
    budget.reserve_storage(BYTES).unwrap();
    for different in [
        measurement(0x74, 16384),
        measurement(0x73, 16385),
        measurement(0x73, MAX_EXECUTABLE + 1),
    ] {
        assert!(
            !owner
                .matches_supervisor_policy_and_executable(
                    &supervisor,
                    &policy,
                    different,
                    &mut budget
                )
                .unwrap()
        );
    }
    for length in [1, MAX_EXECUTABLE] {
        let measured = measurement(0x73, length);
        let (changed, charge) =
            Deployment::new(&supervisor, &policy, measured, &mut budget).unwrap();
        budget.reserve_storage(charge.additional_storage()).unwrap();
        let mut bytes = *owner.canonical_bytes();
        bytes[128..136].copy_from_slice(&length.to_le_bytes());
        reseal(&mut bytes, VERSION);
        assert_eq!(&bytes, changed.canonical_bytes());
        let (decoded, charge) =
            Deployment::decode(&bytes, &supervisor, &policy, &mut budget).unwrap();
        budget.reserve_storage(charge.additional_storage()).unwrap();
        assert_eq!(changed, decoded);
        assert_ne!(owner.identity(), changed.identity());
        assert!(
            changed
                .matches_supervisor_policy_and_executable(
                    &supervisor,
                    &policy,
                    measured,
                    &mut budget
                )
                .unwrap()
        );
        assert!(
            !changed
                .matches_supervisor_policy_and_executable(
                    &supervisor,
                    &policy,
                    executable(),
                    &mut budget
                )
                .unwrap()
        );
        let retained = decoded.retained_storage() + changed.retained_storage();
        drop(decoded);
        drop(changed);
        budget.release_storage(retained).unwrap();
    }
    // Public resealing may describe a different executable, but cannot authenticate it.
    let mut bytes = *owner.canonical_bytes();
    bytes[96] ^= 1;
    reseal(&mut bytes, VERSION);
    let (changed, charge) = Deployment::decode(&bytes, &supervisor, &policy, &mut budget).unwrap();
    budget.reserve_storage(charge.additional_storage()).unwrap();
    assert!(
        !changed
            .matches_supervisor_policy_and_executable(
                &supervisor,
                &policy,
                executable(),
                &mut budget
            )
            .unwrap()
    );
    let floor = budget.storage();
    for length in [MAX_EXECUTABLE + 1, u64::MAX] {
        let before = budget.work();
        assert!(matches!(
            Deployment::new(&supervisor, &policy, measurement(0x73, length), &mut budget),
            Err(Error::Framing(Framing::ExecutableMeasurement))
        ));
        assert_eq!((budget.storage(), budget.work()), (floor, before + WORK));
    }
    assert!(Measurement::new([0; 32], 1).is_err());
    assert!(Measurement::new([1; 32], 0).is_err());
}

#[test]
fn anchor_credentials_accept_nonroot_endpoints_and_remain_exact() {
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    let policy = policy(0, &mut budget);
    for (uid, gid) in [(1, 1), (u32::MAX - 1, u32::MAX - 1), (6001, 5678)] {
        let service = Service::new(uid, gid).unwrap();
        let (supervisor, charge) = Supervisor::new(
            1234,
            5678,
            service,
            measurement(0x71, 4096),
            measurement(0x72, 8192),
            &policy,
            &mut budget,
        )
        .unwrap();
        budget.reserve_storage(charge.additional_storage()).unwrap();
        let owner = deployment(&supervisor, &policy, &mut budget);
        assert_eq!(owner.service(), service);
        let (decoded, charge) =
            Deployment::decode(owner.canonical_bytes(), &supervisor, &policy, &mut budget).unwrap();
        budget.reserve_storage(charge.additional_storage()).unwrap();
        assert_eq!(owner, decoded);
        let retained =
            owner.retained_storage() + decoded.retained_storage() + supervisor.retained_storage();
        drop(decoded);
        drop(owner);
        drop(supervisor);
        budget.release_storage(retained).unwrap();
    }
}

fn perform(
    operation: u8,
    owner: &Deployment,
    bytes: &[u8],
    supervisor: &Supervisor,
    policy: &Policy,
    budget: &mut Budget<'_>,
) -> Result<usize, Error> {
    match operation {
        0 => Deployment::new(supervisor, policy, executable(), budget).map(|(value, charge)| {
            assert_eq!(&value, owner);
            charge.additional_storage()
        }),
        1 => Deployment::decode(bytes, supervisor, policy, budget).map(|(value, charge)| {
            assert_eq!(&value, owner);
            charge.additional_storage()
        }),
        2 => owner
            .identity()
            .matches_canonical_bytes(bytes, supervisor, policy, budget)
            .map(|matched| {
                assert!(matched);
                0
            }),
        3 => owner
            .matches_supervisor_and_policy(supervisor, policy, budget)
            .map(|matched| {
                assert!(matched);
                0
            }),
        4 => owner
            .matches_supervisor_policy_and_executable(supervisor, policy, executable(), budget)
            .map(|matched| {
                assert!(matched);
                0
            }),
        _ => unreachable!(),
    }
}

#[test]
fn every_operation_admits_exact_resources_and_rejects_one_short_at_each_stage() {
    let mut fixture_work = Work::new(LIMIT);
    let mut fixture_budget = Budget::new(&mut fixture_work, LIMIT);
    let policy = policy(0, &mut fixture_budget);
    let supervisor = supervisor(&policy, 0, &mut fixture_budget);
    let owner = deployment(&supervisor, &policy, &mut fixture_budget);
    let bytes = *owner.canonical_bytes();
    let local_work = WORK - SUPERVISOR_WORK;
    let local_storage = STORAGE - SUPERVISOR_STORAGE;
    for operation in 0..5 {
        let input = policy.retained_storage()
            + supervisor.retained_storage()
            + match operation {
                0 => 0,
                1 | 2 => BYTES,
                3 | 4 => owner.retained_storage(),
                _ => unreachable!(),
            };
        for mode in 0..8 {
            let floor = if mode == 3 { input - 1 } else { input + EXTRA };
            let work_limit = match mode {
                1 => WORK - 1,
                4 => 7,
                5 => local_work - 1,
                7 => local_work + 7,
                _ => WORK,
            };
            let storage_limit = floor
                + match mode {
                    2 => STORAGE - 1,
                    6 => local_storage - 1,
                    _ => STORAGE,
                };
            let mut work = Work::new(work_limit);
            let mut budget = Budget::new(&mut work, storage_limit);
            budget.reserve_storage(floor).unwrap();
            let result = perform(operation, &owner, &bytes, &supervisor, &policy, &mut budget);
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
                    assert_eq!(
                        (budget.failed_work(), budget.failed_storage()),
                        (None, None)
                    );
                }
                1 | 4 | 5 | 7 => {
                    assert!(matches!(result, Err(Error::Resource(Resource::Work(_)))));
                    let (used, failed, peak) = match mode {
                        1 => (local_work + 8, WORK, floor + local_storage),
                        4 => (0, 8, floor),
                        5 => (8, local_work, floor),
                        7 => (local_work, local_work + 8, floor + local_storage),
                        _ => unreachable!(),
                    };
                    assert_eq!(
                        (budget.work(), budget.failed_work(), budget.peak_storage()),
                        (used, Some(failed), peak)
                    );
                }
                2 | 6 => {
                    assert!(matches!(result, Err(Error::Resource(Resource::Storage(_)))));
                    let (used, failed, peak) = if mode == 2 {
                        (WORK, floor + STORAGE, floor + local_storage)
                    } else {
                        (local_work, floor + local_storage, floor)
                    };
                    assert_eq!(
                        (
                            budget.work(),
                            budget.failed_storage(),
                            budget.peak_storage()
                        ),
                        (used, Some(failed), peak)
                    );
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
fn every_truncation_and_oversized_input_needs_no_wire_floor_or_scan() {
    let mut fixture_work = Work::new(LIMIT);
    let mut fixture_budget = Budget::new(&mut fixture_work, LIMIT);
    let policy = policy(0, &mut fixture_budget);
    let supervisor = supervisor(&policy, 0, &mut fixture_budget);
    let owner = deployment(&supervisor, &policy, &mut fixture_budget);
    let floor = policy.retained_storage() + supervisor.retained_storage();
    for length in (0..BYTES).chain([BYTES + 1, 100_000]) {
        let bytes = if length < BYTES {
            owner.canonical_bytes()[..length].to_vec()
        } else {
            vec![0; length]
        };
        let mut work = Work::new(2 * WORK);
        let mut budget = Budget::new(&mut work, floor + STORAGE);
        budget.reserve_storage(floor).unwrap();
        assert!(matches!(
            Deployment::decode(&bytes, &supervisor, &policy, &mut budget),
            Err(Error::Framing(Framing::Length))
        ));
        assert!(
            !owner
                .identity()
                .matches_canonical_bytes(&bytes, &supervisor, &policy, &mut budget)
                .unwrap()
        );
        assert_eq!(
            (budget.work(), budget.storage(), budget.peak_storage()),
            (2 * WORK, floor, floor + STORAGE)
        );
    }
}

#[test]
fn first_denials_and_prior_peak_survive_success_mismatch_and_malformed_input() {
    let mut fixture_work = Work::new(LIMIT);
    let mut fixture_budget = Budget::new(&mut fixture_work, LIMIT);
    let policy = policy(0, &mut fixture_budget);
    let other = self::policy(1, &mut fixture_budget);
    let supervisor = supervisor(&policy, 0, &mut fixture_budget);
    let owner = deployment(&supervisor, &policy, &mut fixture_budget);
    let floor = owner.retained_storage()
        + supervisor.retained_storage()
        + policy.retained_storage()
        + other.retained_storage();
    let total = 4 * WORK;
    let ceiling = floor + STORAGE + EXTRA;
    let mut work = Work::new(total);
    let mut budget = Budget::new(&mut work, ceiling);
    assert!(budget.charge_work(total + 1).is_err());
    assert!(budget.reserve_storage(ceiling + 1).is_err());
    budget.reserve_storage(ceiling).unwrap();
    budget.release_storage(ceiling - floor).unwrap();
    assert!(
        owner
            .matches_supervisor_and_policy(&supervisor, &policy, &mut budget)
            .unwrap()
    );
    assert!(matches!(
        Deployment::new(&supervisor, &other, executable(), &mut budget),
        Err(Error::SupervisorPolicyMismatch)
    ));
    assert!(matches!(
        Deployment::decode(&[], &supervisor, &policy, &mut budget),
        Err(Error::Framing(Framing::Length))
    ));
    assert!(
        !owner
            .matches_supervisor_and_policy(&supervisor, &other, &mut budget)
            .unwrap()
    );
    assert_eq!(
        (budget.work(), budget.storage(), budget.peak_storage()),
        (total, floor, ceiling)
    );
    assert!(matches!(
        owner.matches_supervisor_and_policy(&supervisor, &policy, &mut budget),
        Err(Error::Resource(Resource::Work(_)))
    ));
    assert_eq!(budget.failed_work(), Some(total + 1));
    assert_eq!(budget.failed_storage(), Some(ceiling + 1));
    assert_eq!(budget.storage(), floor);
}

#[test]
fn checked_outer_and_nested_reservations_reject_overflow_and_restore_entry() {
    let mut fixture_work = Work::new(LIMIT);
    let mut fixture_budget = Budget::new(&mut fixture_work, LIMIT);
    let policy = policy(0, &mut fixture_budget);
    let supervisor = supervisor(&policy, 0, &mut fixture_budget);
    let owner = deployment(&supervisor, &policy, &mut fixture_budget);
    let local_storage = STORAGE - SUPERVISOR_STORAGE;
    for operation in 0..5 {
        for nested in [false, true] {
            let floor = usize::MAX - if nested { STORAGE } else { local_storage } + 1;
            let mut work = Work::new(WORK);
            let mut budget = Budget::new(&mut work, usize::MAX);
            budget.reserve_storage(floor).unwrap();
            assert!(matches!(
                perform(
                    operation,
                    &owner,
                    owner.canonical_bytes(),
                    &supervisor,
                    &policy,
                    &mut budget
                ),
                Err(Error::Resource(Resource::Storage(_)))
            ));
            assert_eq!(budget.storage(), floor);
            assert_eq!(
                budget.work(),
                if nested { WORK } else { WORK - SUPERVISOR_WORK }
            );
            assert_eq!(
                budget.peak_storage(),
                floor + if nested { local_storage } else { 0 }
            );
            assert_eq!(budget.failed_storage(), Some(usize::MAX));
        }
    }
}

#[test]
fn cumulative_work_overflow_preserves_the_original_ledger() {
    let mut fixture_work = Work::new(LIMIT);
    let mut fixture_budget = Budget::new(&mut fixture_work, LIMIT);
    let policy = policy(0, &mut fixture_budget);
    let supervisor = supervisor(&policy, 0, &mut fixture_budget);
    let floor = policy.retained_storage() + supervisor.retained_storage();
    for prior in [usize::MAX - 7, usize::MAX - (WORK - SUPERVISOR_WORK)] {
        let mut work = Work::new(usize::MAX);
        let mut budget = Budget::new(&mut work, floor + STORAGE);
        budget.reserve_storage(floor).unwrap();
        budget.charge_work(prior).unwrap();
        assert!(matches!(
            Deployment::new(&supervisor, &policy, executable(), &mut budget),
            Err(Error::Resource(Resource::Work(_)))
        ));
        assert_eq!(budget.storage(), floor);
        assert_eq!(budget.failed_work(), Some(usize::MAX));
        assert_eq!(
            budget.work(),
            if prior == usize::MAX - 7 {
                prior
            } else {
                usize::MAX
            }
        );
    }
}
