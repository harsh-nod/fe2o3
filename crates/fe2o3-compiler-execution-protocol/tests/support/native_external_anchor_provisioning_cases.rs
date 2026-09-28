const CONTEXT_WORK: usize = POLICY_WORK + SUPERVISOR_WORK + DEPLOYMENT_WORK;

fn deployment(
    policy_axis: u8,
    supervisor_axis: u8,
    anchor_axis: u8,
    b: &mut Budget<'_>,
) -> Deployment {
    let (p, charge) = Policy::new(
        if policy_axis == 1 { 8 } else { 7 },
        measurement(
            if policy_axis == 2 { 0x63 } else { 0x61 },
            if policy_axis == 3 { 12346 } else { 12345 },
        ),
        measurement(
            if policy_axis == 4 { 0x64 } else { 0x62 },
            if policy_axis == 5 { 67891 } else { 67890 },
        ),
        key(if policy_axis == 6 { 0x53 } else { 0x51 }),
        key(if policy_axis == 7 { 0x54 } else { 0x52 }),
        b,
    )
    .unwrap();
    b.reserve_storage(charge.additional_storage()).unwrap();
    let (s, charge) = Supervisor::new(
        if supervisor_axis == 1 { 1235 } else { 1234 },
        if supervisor_axis == 2 { 5679 } else { 5678 },
        Service::new(
            if supervisor_axis == 3 { 6002 } else { 6001 },
            if supervisor_axis == 4 { 7002 } else { 7001 },
        )
        .unwrap(),
        measurement(
            if supervisor_axis == 5 { 0x75 } else { 0x71 },
            if supervisor_axis == 6 { 4097 } else { 4096 },
        ),
        measurement(
            if supervisor_axis == 7 { 0x76 } else { 0x72 },
            if supervisor_axis == 8 { 8193 } else { 8192 },
        ),
        &p,
        b,
    )
    .unwrap();
    b.reserve_storage(charge.additional_storage()).unwrap();
    let (d, charge) = Deployment::new(
        &s,
        &p,
        measurement(
            if anchor_axis == 1 { 0x77 } else { 0x73 },
            if anchor_axis == 2 { 16385 } else { 16384 },
        ),
        b,
    )
    .unwrap();
    b.reserve_storage(charge.additional_storage()).unwrap();
    // Provisioning needs only the actual completed deployment, even after its
    // construction inputs have dropped and their reservations have been retired.
    let retired = s.retained_storage() + p.retained_storage();
    drop((s, p));
    b.release_storage(retired).unwrap();
    d
}
fn provisioning(d: &Deployment, b: &mut Budget<'_>) -> Provisioning {
    let (floor, before) = (b.storage(), b.work());
    let (p, charge) = Provisioning::new(d, helper(), b).unwrap();
    assert_eq!((b.storage(), b.work()), (floor, before + WORK));
    assert_eq!(charge.additional_storage(), p.retained_storage());
    assert_eq!(
        p.retained_storage(),
        std::mem::size_of::<(Provisioning, Storage)>()
    );
    b.reserve_storage(charge.additional_storage()).unwrap();
    p
}

#[test]
fn independent_transcript_roundtrip_preserves_original_ledger_and_exact_retention() {
    assert_eq!((BYTES, WORK, MAX_HELPER), (128, 4104, 128 * 1024 * 1024));
    let old_d = legacy_deployment();
    let old = Legacy::new(&old_d, helper()).unwrap();
    assert_eq!(old.canonical_bytes(), &wire(1, old_d.identity().as_bytes()));
    assert_eq!(Legacy::decode(old.canonical_bytes()).unwrap(), old);
    let mut w = Work::new(CONTEXT_WORK + 5 * WORK);
    let mut b = Budget::new(&mut w, LIMIT);
    b.reserve_storage(EXTRA).unwrap();
    let ledger = b.work_ledger_identity_v1();
    let retired = {
        let d = deployment(0, 0, 0, &mut b);
        let p = provisioning(&d, &mut b);
        assert_eq!(p.canonical_bytes(), &wire(VERSION, d.identity().as_bytes()));
        assert_eq!(p.deployment_identity(), d.identity());
        assert_eq!(p.helper(), helper());
        assert_ne!(p.identity().as_bytes(), old.identity().as_bytes());
        assert!(matches!(
            Legacy::decode(p.canonical_bytes()),
            Err(Framing::Header)
        ));
        fn copy_identity<T: Copy>(id: T) -> (T, T) {
            (id, id)
        }
        let ids: (Identity, Identity) = copy_identity(p.identity());
        assert_eq!(ids.0, ids.1);
        let floor = b.storage();
        assert!(p.matches_deployment(&d, &mut b).unwrap());
        assert!(
            p.matches_deployment_and_helper(&d, helper(), &mut b)
                .unwrap()
        );
        assert!(
            ids.0
                .matches_canonical_bytes(p.canonical_bytes(), &d, &mut b)
                .unwrap()
        );
        let (decoded, charge) = Provisioning::decode(p.canonical_bytes(), &d, &mut b).unwrap();
        assert_eq!(b.storage(), floor);
        assert_eq!(charge.additional_storage(), decoded.retained_storage());
        b.reserve_storage(charge.additional_storage()).unwrap();
        assert_eq!(p, decoded);
        assert_eq!(b.work(), CONTEXT_WORK + 5 * WORK);
        d.retained_storage() + p.retained_storage() + decoded.retained_storage()
    };
    b.release_storage(retired).unwrap();
    assert_eq!(b.storage(), EXTRA);
    assert!(b.work_ledger_identity_v1() == ledger);
}

#[test]
fn every_single_byte_mutation_rejects_in_both_public_decoding_paths() {
    let mut w = Work::new(LIMIT);
    let mut b = Budget::new(&mut w, LIMIT);
    let d = deployment(0, 0, 0, &mut b);
    let p = provisioning(&d, &mut b);
    b.reserve_storage(BYTES).unwrap();
    let floor = b.storage();
    for offset in 0..BYTES {
        let mut bytes = *p.canonical_bytes();
        bytes[offset] ^= 1;
        assert!(
            Provisioning::decode(&bytes, &d, &mut b).is_err(),
            "byte {offset}"
        );
        assert!(
            !p.identity()
                .matches_canonical_bytes(&bytes, &d, &mut b)
                .unwrap(),
            "byte {offset}"
        );
        assert_eq!(b.storage(), floor);
    }
    assert_eq!(b.work(), CONTEXT_WORK + (1 + 2 * BYTES) * WORK);
}

#[test]
fn resealed_headers_and_invalid_fields_are_still_rejected_at_full_fixed_cost() {
    let mut w = Work::new(LIMIT);
    let mut b = Budget::new(&mut w, LIMIT);
    let d = deployment(0, 0, 0, &mut b);
    let good = wire(VERSION, d.identity().as_bytes());
    b.reserve_storage(BYTES).unwrap();
    let floor = b.storage();
    let mut check = |mut bytes: [u8; BYTES], expected: Framing| {
        reseal(&mut bytes, VERSION);
        let before = b.work();
        assert!(
            matches!(Provisioning::decode(&bytes, &d, &mut b), Err(Error::Framing(e)) if e == expected),
            "expected {expected:?}"
        );
        assert_eq!((b.storage(), b.work()), (floor, before + WORK));
    };
    for offset in (0..8).chain([10, 11, 16, 17, 18, 19, 20, 21, 22, 23]) {
        let mut bytes = good;
        bytes[offset] ^= 1;
        check(bytes, Framing::Header);
    }
    for version in [0, 1, OTHER_VERSION, u16::MAX] {
        let mut bytes = good;
        bytes[8..10].copy_from_slice(&version.to_le_bytes());
        check(bytes, Framing::Header);
    }
    for length in [0u32, 127, 129, u32::MAX] {
        let mut bytes = good;
        bytes[12..16].copy_from_slice(&length.to_le_bytes());
        check(bytes, Framing::Header);
    }
    for (offset, error) in [
        (24, Framing::DeploymentIdentity),
        (56, Framing::HelperMeasurement),
    ] {
        let mut bytes = good;
        bytes[offset..offset + 32].fill(0);
        check(bytes, error);
    }
    for length in [0u64, MAX_HELPER + 1, u64::MAX] {
        let mut bytes = good;
        bytes[88..96].copy_from_slice(&length.to_le_bytes());
        check(bytes, Framing::HelperMeasurement);
    }
}

#[test]
fn complete_deployment_binding_includes_every_policy_supervisor_and_anchor_axis() {
    let mut w = Work::new(LIMIT);
    let mut b = Budget::new(&mut w, LIMIT);
    let pinned = deployment(0, 0, 0, &mut b);
    let p = provisioning(&pinned, &mut b);
    b.reserve_storage(BYTES).unwrap();
    let axes = (1..=7)
        .map(|axis| (axis, 0, 0))
        .chain((1..=8).map(|axis| (0, axis, 0)))
        .chain([(0, 0, 1), (0, 0, 2)]);
    for (policy_axis, supervisor_axis, anchor_axis) in axes {
        let other = deployment(policy_axis, supervisor_axis, anchor_axis, &mut b);
        assert_ne!(other.identity(), pinned.identity());
        assert!(!p.matches_deployment(&other, &mut b).unwrap());
        assert!(
            !p.matches_deployment_and_helper(&other, helper(), &mut b)
                .unwrap()
        );
        assert!(
            !p.identity()
                .matches_canonical_bytes(p.canonical_bytes(), &other, &mut b)
                .unwrap()
        );
        assert!(matches!(
            Provisioning::decode(p.canonical_bytes(), &other, &mut b),
            Err(Error::ContextMismatch)
        ));
        let changed = provisioning(&other, &mut b);
        let mut bytes = *p.canonical_bytes();
        bytes[24..56].copy_from_slice(other.identity().as_bytes());
        reseal(&mut bytes, VERSION);
        assert_eq!(&bytes, changed.canonical_bytes());
        assert_ne!(p.identity(), changed.identity());
        assert!(matches!(
            Provisioning::decode(&bytes, &pinned, &mut b),
            Err(Error::ContextMismatch)
        ));
        let (decoded, charge) = Provisioning::decode(&bytes, &other, &mut b).unwrap();
        b.reserve_storage(charge.additional_storage()).unwrap();
        assert_eq!(decoded, changed);
        let retired =
            decoded.retained_storage() + changed.retained_storage() + other.retained_storage();
        drop((decoded, changed, other));
        b.release_storage(retired).unwrap();
    }
    let mut bytes = *p.canonical_bytes();
    bytes[24] ^= 1;
    reseal(&mut bytes, VERSION);
    assert!(matches!(
        Provisioning::decode(&bytes, &pinned, &mut b),
        Err(Error::ContextMismatch)
    ));
}

#[test]
fn helper_mutations_are_distinct_inert_configuration_and_limits_are_inclusive() {
    let mut w = Work::new(LIMIT);
    let mut b = Budget::new(&mut w, LIMIT);
    let d = deployment(0, 0, 0, &mut b);
    let p = provisioning(&d, &mut b);
    b.reserve_storage(BYTES).unwrap();
    for measured in [
        measurement(0x75, 32768),
        measurement(0x74, 32769),
        measurement(0x74, 1),
        measurement(0x74, MAX_HELPER),
    ] {
        assert!(
            !p.matches_deployment_and_helper(&d, measured, &mut b)
                .unwrap()
        );
        let (changed, charge) = Provisioning::new(&d, measured, &mut b).unwrap();
        b.reserve_storage(charge.additional_storage()).unwrap();
        let mut bytes = *p.canonical_bytes();
        bytes[56..88].copy_from_slice(&measured.sha256());
        bytes[88..96].copy_from_slice(&measured.byte_len().to_le_bytes());
        reseal(&mut bytes, VERSION);
        assert_eq!(&bytes, changed.canonical_bytes());
        let (decoded, charge) = Provisioning::decode(&bytes, &d, &mut b).unwrap();
        b.reserve_storage(charge.additional_storage()).unwrap();
        assert_eq!(decoded, changed);
        assert_ne!(decoded.identity(), p.identity());
        assert!(decoded.matches_deployment(&d, &mut b).unwrap());
        assert!(
            !decoded
                .matches_deployment_and_helper(&d, helper(), &mut b)
                .unwrap()
        );
        assert!(
            decoded
                .matches_deployment_and_helper(&d, measured, &mut b)
                .unwrap()
        );
        assert!(
            decoded
                .identity()
                .matches_canonical_bytes(&bytes, &d, &mut b)
                .unwrap()
        );
        assert!(
            !p.identity()
                .matches_canonical_bytes(&bytes, &d, &mut b)
                .unwrap()
        );
        let retired = decoded.retained_storage() + changed.retained_storage();
        drop((decoded, changed));
        b.release_storage(retired).unwrap();
    }
    let floor = b.storage();
    for length in [MAX_HELPER + 1, u64::MAX] {
        let before = b.work();
        assert!(matches!(
            Provisioning::new(&d, measurement(0x74, length), &mut b),
            Err(Error::Framing(Framing::HelperMeasurement))
        ));
        assert_eq!((b.storage(), b.work()), (floor, before + WORK));
    }
    assert!(Measurement::new([0; 32], 1).is_err());
    assert!(Measurement::new([1; 32], 0).is_err());
}

#[test]
fn legacy_foreign_and_resealed_foreign_deployment_bindings_never_upgrade() {
    let mut w = Work::new(LIMIT);
    let mut b = Budget::new(&mut w, LIMIT);
    let d = deployment(0, 0, 0, &mut b);
    let p = provisioning(&d, &mut b);
    let old_d = legacy_deployment();
    let old = Legacy::new(&old_d, helper()).unwrap();
    b.reserve_storage(std::mem::size_of_val(&old) + BYTES)
        .unwrap();
    let (other_p, charge) = OtherPolicy::new(
        7,
        measurement(0x61, 12345),
        measurement(0x62, 67890),
        key(0x51),
        key(0x52),
        &mut b,
    )
    .unwrap();
    b.reserve_storage(charge.additional_storage()).unwrap();
    let (other_s, charge) = OtherSupervisor::new(
        1234,
        5678,
        Service::new(6001, 7001).unwrap(),
        measurement(0x71, 4096),
        measurement(0x72, 8192),
        &other_p,
        &mut b,
    )
    .unwrap();
    b.reserve_storage(charge.additional_storage()).unwrap();
    let (other_d, charge) =
        OtherDeployment::new(&other_s, &other_p, measurement(0x73, 16384), &mut b).unwrap();
    b.reserve_storage(charge.additional_storage()).unwrap();
    let (other, charge) = OtherProvisioning::new(&other_d, helper(), &mut b).unwrap();
    b.reserve_storage(charge.additional_storage()).unwrap();
    assert_eq!(
        other.canonical_bytes(),
        &wire(OTHER_VERSION, other_d.identity().as_bytes())
    );
    assert_ne!(other.identity().as_bytes(), p.identity().as_bytes());
    for foreign in [old.canonical_bytes(), other.canonical_bytes()] {
        assert!(matches!(
            Provisioning::decode(foreign, &d, &mut b),
            Err(Error::Framing(Framing::Header))
        ));
        let mut bytes = *foreign;
        bytes[..10].copy_from_slice(&p.canonical_bytes()[..10]);
        reseal(&mut bytes, VERSION);
        assert!(matches!(
            Provisioning::decode(&bytes, &d, &mut b),
            Err(Error::ContextMismatch)
        ));
        assert!(
            !p.identity()
                .matches_canonical_bytes(&bytes, &d, &mut b)
                .unwrap()
        );
    }
    for foreign_version in [1, OTHER_VERSION] {
        let mut bytes = *p.canonical_bytes();
        reseal(&mut bytes, foreign_version);
        assert!(matches!(
            Provisioning::decode(&bytes, &d, &mut b),
            Err(Error::Framing(Framing::Identity))
        ));
    }
}

fn perform(
    operation: u8,
    owner: &Provisioning,
    bytes: &[u8],
    d: &Deployment,
    budget: &mut Budget<'_>,
) -> Result<usize, Error> {
    match operation {
        0 => Provisioning::new(d, helper(), budget).map(|(value, charge)| {
            assert_eq!(&value, owner);
            charge.additional_storage()
        }),
        1 => Provisioning::decode(bytes, d, budget).map(|(value, charge)| {
            assert_eq!(&value, owner);
            charge.additional_storage()
        }),
        2 => owner
            .identity()
            .matches_canonical_bytes(bytes, d, budget)
            .map(|matched| {
                assert!(matched);
                0
            }),
        3 => owner.matches_deployment(d, budget).map(|matched| {
            assert!(matched);
            0
        }),
        4 => owner
            .matches_deployment_and_helper(d, helper(), budget)
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
    let d = deployment(0, 0, 0, &mut fixture_budget);
    let p = provisioning(&d, &mut fixture_budget);
    let bytes = *p.canonical_bytes();
    for operation in 0..5 {
        let input = d.retained_storage()
            + match operation {
                0 => 0,
                1 | 2 => BYTES,
                3 | 4 => p.retained_storage(),
                _ => unreachable!(),
            };
        for mode in 0..5 {
            let floor = if mode == 3 { input - 1 } else { input };
            let work_limit = match mode {
                1 => WORK - 1,
                4 => 7,
                _ => WORK,
            };
            let storage_limit = floor + STORAGE - usize::from(mode == 2);
            let mut work = Work::new(work_limit);
            let mut budget = Budget::new(&mut work, storage_limit);
            budget.reserve_storage(floor).unwrap();
            let ledger = budget.work_ledger_identity_v1();
            let result = perform(operation, &p, &bytes, &d, &mut budget);
            assert_eq!(budget.storage(), floor);
            assert!(budget.work_ledger_identity_v1() == ledger);
            match mode {
                0 => {
                    assert_eq!(
                        result.unwrap(),
                        if operation < 2 {
                            p.retained_storage()
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
                1 | 4 => {
                    assert!(matches!(result, Err(Error::Resource(Resource::Work(_)))));
                    let (used, failed) = if mode == 1 { (8, WORK) } else { (0, 8) };
                    assert_eq!(
                        (budget.work(), budget.failed_work(), budget.peak_storage()),
                        (used, Some(failed), floor)
                    );
                    assert_eq!(budget.failed_storage(), None);
                }
                2 => {
                    assert!(matches!(result, Err(Error::Resource(Resource::Storage(_)))));
                    assert_eq!(
                        (
                            budget.work(),
                            budget.failed_storage(),
                            budget.peak_storage()
                        ),
                        (WORK, Some(floor + STORAGE), floor)
                    );
                    assert_eq!(budget.failed_work(), None);
                }
                3 => {
                    assert!(matches!(result, Err(Error::Resource(Resource::Accounting))));
                    assert_eq!((budget.work(), budget.peak_storage()), (8, floor));
                    assert_eq!(
                        (budget.failed_work(), budget.failed_storage()),
                        (None, None)
                    );
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
    let d = deployment(0, 0, 0, &mut fixture_budget);
    let p = provisioning(&d, &mut fixture_budget);
    let floor = d.retained_storage();
    for length in (0..BYTES).chain([BYTES + 1, 100_000]) {
        let bytes = if length < BYTES {
            p.canonical_bytes()[..length].to_vec()
        } else {
            vec![0; length]
        };
        let mut work = Work::new(2 * WORK);
        let mut budget = Budget::new(&mut work, floor + STORAGE);
        budget.reserve_storage(floor).unwrap();
        assert!(matches!(
            Provisioning::decode(&bytes, &d, &mut budget),
            Err(Error::Framing(Framing::Length))
        ));
        assert!(
            !p.identity()
                .matches_canonical_bytes(&bytes, &d, &mut budget)
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
    let d = deployment(0, 0, 0, &mut fixture_budget);
    let other = deployment(0, 0, 1, &mut fixture_budget);
    let p = provisioning(&d, &mut fixture_budget);
    let floor = d.retained_storage() + other.retained_storage() + p.retained_storage();
    let total = 5 * WORK;
    let ceiling = floor + STORAGE + EXTRA;
    let mut work = Work::new(total);
    let mut budget = Budget::new(&mut work, ceiling);
    let ledger = budget.work_ledger_identity_v1();
    assert!(budget.charge_work(total + 1).is_err());
    assert!(budget.reserve_storage(ceiling + 1).is_err());
    budget.reserve_storage(ceiling).unwrap();
    budget.release_storage(ceiling - floor).unwrap();
    assert!(p.matches_deployment(&d, &mut budget).unwrap());
    assert!(!p.matches_deployment(&other, &mut budget).unwrap());
    assert!(matches!(
        Provisioning::decode(p.canonical_bytes(), &other, &mut budget),
        Err(Error::ContextMismatch)
    ));
    assert!(matches!(
        Provisioning::decode(&[], &d, &mut budget),
        Err(Error::Framing(Framing::Length))
    ));
    assert!(matches!(
        Provisioning::new(&d, measurement(0x74, MAX_HELPER + 1), &mut budget),
        Err(Error::Framing(Framing::HelperMeasurement))
    ));
    assert_eq!(
        (budget.work(), budget.storage(), budget.peak_storage()),
        (total, floor, ceiling)
    );
    assert!(matches!(
        p.matches_deployment(&d, &mut budget),
        Err(Error::Resource(Resource::Work(_)))
    ));
    assert_eq!(
        (budget.failed_work(), budget.failed_storage()),
        (Some(total + 1), Some(ceiling + 1))
    );
    assert_eq!(
        (budget.work(), budget.storage(), budget.peak_storage()),
        (total, floor, ceiling)
    );
    assert!(budget.work_ledger_identity_v1() == ledger);
}

#[test]
fn checked_reservations_reject_overflow_and_restore_entry() {
    let mut fixture_work = Work::new(LIMIT);
    let mut fixture_budget = Budget::new(&mut fixture_work, LIMIT);
    let d = deployment(0, 0, 0, &mut fixture_budget);
    let p = provisioning(&d, &mut fixture_budget);
    for operation in 0..5 {
        let floor = usize::MAX - STORAGE + 1;
        let mut work = Work::new(WORK);
        let mut budget = Budget::new(&mut work, usize::MAX);
        budget.reserve_storage(floor).unwrap();
        assert!(matches!(
            perform(operation, &p, p.canonical_bytes(), &d, &mut budget),
            Err(Error::Resource(Resource::Storage(_)))
        ));
        assert_eq!(
            (budget.work(), budget.storage(), budget.peak_storage()),
            (WORK, floor, floor)
        );
        assert_eq!(
            (budget.failed_work(), budget.failed_storage()),
            (None, Some(usize::MAX))
        );
    }
}

#[test]
fn cumulative_work_overflow_preserves_the_original_ledger() {
    let mut fixture_work = Work::new(LIMIT);
    let mut fixture_budget = Budget::new(&mut fixture_work, LIMIT);
    let d = deployment(0, 0, 0, &mut fixture_budget);
    let floor = d.retained_storage();
    for prior in [usize::MAX - 7, usize::MAX - (WORK - 1)] {
        let mut work = Work::new(usize::MAX);
        let mut budget = Budget::new(&mut work, floor + STORAGE);
        budget.reserve_storage(floor).unwrap();
        budget.charge_work(prior).unwrap();
        let ledger = budget.work_ledger_identity_v1();
        assert!(matches!(
            Provisioning::new(&d, helper(), &mut budget),
            Err(Error::Resource(Resource::Work(_)))
        ));
        let used = if prior == usize::MAX - 7 {
            prior
        } else {
            prior + 8
        };
        assert_eq!(
            (budget.work(), budget.storage(), budget.peak_storage()),
            (used, floor, floor)
        );
        assert_eq!(
            (budget.failed_work(), budget.failed_storage()),
            (Some(usize::MAX), None)
        );
        assert!(budget.work_ledger_identity_v1() == ledger);
    }
}
