use super::*;
use ed25519_dalek::SigningKey;
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;

fn key(seed: u8) -> [u8; 32] {
    SigningKey::from_bytes(&[seed; 32])
        .verifying_key()
        .to_bytes()
}

fn measurement(digest: u8, byte_len: u64) -> Measurement {
    Measurement::new([digest; 32], byte_len).unwrap()
}

fn inputs() -> Inputs {
    Inputs {
        generation: 7,
        compiler_service_uid: 1001,
        compiler_service_gid: 1002,
        external_anchor_service: Service::new(2001, 2002).unwrap(),
        supervisor: measurement(0x11, 0x1100),
        launcher: measurement(0x22, 0x2200),
        issuer: measurement(0x33, 0x3300),
        anchor_helper: measurement(0x44, 0x4400),
        anchor_daemon: measurement(0x55, 0x5500),
        issuer_verifying_key: key(0x66),
        anchor_verifying_key: key(0x77),
    }
}

fn retain(inputs: &Inputs, budget: &mut Budget<'_>) -> Bundle {
    let floor = budget.storage();
    let (bundle, charge) = Bundle::new(inputs, budget).unwrap();
    assert_eq!(budget.storage(), floor);
    assert_eq!(charge.additional_storage(), bundle.retained_storage());
    assert_eq!(
        bundle.retained_storage(),
        bundle.client_profile().retained_storage()
            + bundle.supervisor().retained_storage()
            + bundle.anchor_deployment().retained_storage()
            + bundle.anchor_provisioning().retained_storage()
            + Bundle::GROWTH_STORAGE
    );
    budget.reserve_storage(charge.additional_storage()).unwrap();
    bundle
}

fn identities(bundle: &Bundle) -> [[u8; 32]; 5] {
    [
        *bundle.policy().identity().as_bytes(),
        *bundle.client_profile().identity().as_bytes(),
        *bundle.supervisor().identity().as_bytes(),
        *bundle.anchor_deployment().identity().as_bytes(),
        *bundle.anchor_provisioning().identity().as_bytes(),
    ]
}

fn resource(error: &Error) -> Resource {
    let mut current: &dyn StdError = error;
    loop {
        if let Some(resource) = current.downcast_ref::<Resource>() {
            return *resource;
        }
        current = current
            .source()
            .expect("typed resource error in source chain");
    }
}

#[test]
fn complete_graph_is_deterministic_and_contains_only_native_records() {
    let inputs = inputs();
    let mut work = Work::new(4 * Bundle::WORK);
    let mut budget = Budget::new(&mut work, inputs.retained_storage() + 4 * Bundle::SCRATCH);
    budget.reserve_storage(inputs.retained_storage()).unwrap();
    let ledger = budget.work_ledger_identity_v1();
    let bundle = retain(&inputs, &mut budget);
    let repeated = retain(&inputs, &mut budget);
    assert_eq!(bundle, repeated);
    assert_eq!(budget.work(), 2 * Bundle::WORK);
    assert!(std::ptr::eq(
        bundle.policy(),
        bundle.client_profile().policy()
    ));
    assert_eq!(bundle.policy().generation(), inputs.generation);
    assert_eq!(bundle.policy().executable(), inputs.issuer);
    assert_eq!(
        bundle.policy().runtime(),
        sealed_static_issuer_runtime_measurement_v1()
    );
    assert_eq!(
        bundle.policy().verifying_key(),
        &inputs.issuer_verifying_key
    );
    assert_eq!(
        bundle.policy().external_anchor_verifying_key(),
        &inputs.anchor_verifying_key
    );
    assert_eq!(bundle.supervisor().executable(), inputs.supervisor);
    assert_eq!(bundle.supervisor().launcher(), inputs.launcher);
    assert_eq!(
        bundle.supervisor().service_uid(),
        inputs.compiler_service_uid
    );
    assert_eq!(
        bundle.supervisor().service_gid(),
        inputs.compiler_service_gid
    );
    assert_eq!(
        bundle.supervisor().external_anchor_service(),
        inputs.external_anchor_service
    );
    assert_eq!(
        bundle.client_profile().supervisor_uid(),
        inputs.compiler_service_uid
    );
    assert_eq!(
        bundle.client_profile().supervisor_gid(),
        inputs.compiler_service_gid
    );
    assert_eq!(
        bundle.client_profile().external_anchor_service(),
        inputs.external_anchor_service
    );
    assert_eq!(
        bundle.anchor_deployment().executable(),
        inputs.anchor_daemon
    );
    assert_eq!(
        bundle.anchor_deployment().service(),
        inputs.external_anchor_service
    );
    assert_eq!(
        bundle.anchor_deployment().verifying_key(),
        &inputs.anchor_verifying_key
    );
    assert_eq!(bundle.anchor_provisioning().helper(), inputs.anchor_helper);
    assert!(
        bundle
            .supervisor()
            .matches_policy(bundle.policy(), &mut budget)
            .unwrap()
    );
    assert!(
        bundle
            .anchor_deployment()
            .matches_supervisor_and_policy(bundle.supervisor(), bundle.policy(), &mut budget,)
            .unwrap()
    );
    assert!(
        bundle
            .anchor_provisioning()
            .matches_deployment_and_helper(
                bundle.anchor_deployment(),
                inputs.anchor_helper,
                &mut budget,
            )
            .unwrap()
    );

    let floor = budget.storage();
    let (policy, charge) = Policy::decode(bundle.policy().canonical_bytes(), &mut budget).unwrap();
    budget.reserve_storage(charge.additional_storage()).unwrap();
    let (supervisor, charge) =
        Supervisor::decode(bundle.supervisor().canonical_bytes(), &policy, &mut budget).unwrap();
    budget.reserve_storage(charge.additional_storage()).unwrap();
    let (anchor, charge) = Anchor::decode(
        bundle.anchor_deployment().canonical_bytes(),
        &supervisor,
        &policy,
        &mut budget,
    )
    .unwrap();
    budget.reserve_storage(charge.additional_storage()).unwrap();
    let (provisioning, charge) = Provisioning::decode(
        bundle.anchor_provisioning().canonical_bytes(),
        &anchor,
        &mut budget,
    )
    .unwrap();
    budget.reserve_storage(charge.additional_storage()).unwrap();
    let (profile, charge) =
        Profile::decode(bundle.client_profile().canonical_bytes(), &mut budget).unwrap();
    budget.reserve_storage(charge.additional_storage()).unwrap();
    assert_eq!(&policy, bundle.policy());
    assert_eq!(&supervisor, bundle.supervisor());
    assert_eq!(&anchor, bundle.anchor_deployment());
    assert_eq!(&provisioning, bundle.anchor_provisioning());
    assert_eq!(&profile, bundle.client_profile());
    assert_eq!(&policy.canonical_bytes()[..8], b"F2O3CEP3");
    assert_eq!(&supervisor.canonical_bytes()[..8], b"F2O3CED3");
    let decoded_storage = policy.retained_storage()
        + supervisor.retained_storage()
        + anchor.retained_storage()
        + provisioning.retained_storage()
        + profile.retained_storage();
    drop((policy, supervisor, anchor, provisioning, profile));
    budget.release_storage(decoded_storage).unwrap();
    assert_eq!(budget.storage(), floor);
    let retained = bundle.retained_storage() + repeated.retained_storage();
    drop((bundle, repeated));
    budget.release_storage(retained).unwrap();
    assert_eq!(budget.storage(), inputs.retained_storage());
    assert!(budget.work_ledger_identity_v1() == ledger);
}

#[test]
fn substitutions_change_exactly_the_dependent_native_identities() {
    let original = inputs();
    let mut work = Work::new(12 * Bundle::WORK);
    let mut budget = Budget::new(
        &mut work,
        2 * original.retained_storage() + 3 * Bundle::SCRATCH,
    );
    budget
        .reserve_storage(2 * original.retained_storage())
        .unwrap();
    let bundle = retain(&original, &mut budget);
    let original_ids = identities(&bundle);
    for case in 0..11 {
        let mut changed = original;
        // policy, profile, supervisor, anchor deployment, anchor provisioning
        let expected = match case {
            0 => {
                changed.generation += 1;
                [true; 5]
            }
            1 => {
                changed.issuer = measurement(0x81, 0x8100);
                [true; 5]
            }
            2 => {
                changed.issuer_verifying_key = key(0x82);
                [true; 5]
            }
            3 => {
                changed.anchor_verifying_key = key(0x83);
                [true; 5]
            }
            4 => {
                changed.supervisor = measurement(0x84, 0x8400);
                [false, false, true, true, true]
            }
            5 => {
                changed.launcher = measurement(0x85, 0x8500);
                [false, false, true, true, true]
            }
            6 => {
                changed.anchor_daemon = measurement(0x86, 0x8600);
                [false, false, false, true, true]
            }
            7 => {
                changed.anchor_helper = measurement(0x87, 0x8700);
                [false, false, false, false, true]
            }
            8 => {
                changed.compiler_service_uid += 1;
                [false, true, true, true, true]
            }
            9 => {
                changed.compiler_service_gid += 1;
                [false, true, true, true, true]
            }
            _ => {
                changed.external_anchor_service = Service::new(3001, 3002).unwrap();
                [false, true, true, true, true]
            }
        };
        let replacement = retain(&changed, &mut budget);
        for (index, (before, after)) in original_ids
            .iter()
            .zip(identities(&replacement))
            .enumerate()
        {
            assert_eq!(
                before != &after,
                expected[index],
                "case {case}, record {index}"
            );
        }
        let retained = replacement.retained_storage();
        drop(replacement);
        budget.release_storage(retained).unwrap();
    }
    assert_eq!(budget.work(), 12 * Bundle::WORK);
    let retained = bundle.retained_storage();
    drop(bundle);
    budget.release_storage(retained).unwrap();
    assert_eq!(budget.storage(), 2 * original.retained_storage());
}

fn set_measurement(inputs: &mut Inputs, role: usize, measurement: Measurement) {
    match role {
        0 => inputs.supervisor = measurement,
        1 => inputs.launcher = measurement,
        2 => inputs.issuer = measurement,
        3 => inputs.anchor_helper = measurement,
        4 => inputs.anchor_daemon = measurement,
        _ => unreachable!(),
    }
}

#[test]
fn every_pair_of_executable_roles_must_have_distinct_exact_measurements() {
    let mut inputs = inputs();
    let measurements = [
        inputs.supervisor,
        inputs.launcher,
        inputs.issuer,
        inputs.anchor_helper,
        inputs.anchor_daemon,
    ];
    let mut work = Work::new(10 * LOCAL_WORK);
    let mut budget = Budget::new(&mut work, inputs.retained_storage() + Bundle::SCRATCH);
    budget.reserve_storage(inputs.retained_storage()).unwrap();
    for first in 0..5 {
        for second in first + 1..5 {
            set_measurement(&mut inputs, second, measurements[first]);
            let error = Bundle::new(&inputs, &mut budget).unwrap_err();
            assert!(
                matches!(error, Error::AliasedExecutableMeasurements { first: a, second: b }
                if a == EXECUTABLE_ROLES[first] && b == EXECUTABLE_ROLES[second])
            );
            assert_eq!(budget.storage(), inputs.retained_storage());
            set_measurement(&mut inputs, second, measurements[second]);
        }
    }
    assert_eq!(budget.work(), 10 * LOCAL_WORK);
}

#[test]
fn measurement_aliasing_compares_both_digest_and_length() {
    let mut inputs = inputs();
    inputs.launcher =
        Measurement::new(inputs.supervisor.sha256(), inputs.supervisor.byte_len() + 1).unwrap();
    inputs.anchor_daemon = Measurement::new([0x99; 32], inputs.anchor_helper.byte_len()).unwrap();
    let mut work = Work::new(Bundle::WORK);
    let mut budget = Budget::new(&mut work, inputs.retained_storage() + Bundle::SCRATCH);
    budget.reserve_storage(inputs.retained_storage()).unwrap();
    let bundle = retain(&inputs, &mut budget);
    let retained = bundle.retained_storage();
    drop(bundle);
    budget.release_storage(retained).unwrap();
    assert_eq!(budget.storage(), inputs.retained_storage());
}

#[test]
fn issuer_ceiling_matches_the_native_root_runner() {
    for length in [MAX_ISSUER, MAX_ISSUER + 1, u64::MAX] {
        let mut inputs = inputs();
        inputs.issuer = measurement(0x91, length);
        let mut work = Work::new(Bundle::WORK);
        let mut budget = Budget::new(&mut work, inputs.retained_storage() + Bundle::SCRATCH);
        budget.reserve_storage(inputs.retained_storage()).unwrap();
        let result = Bundle::new(&inputs, &mut budget);
        if length == MAX_ISSUER {
            assert!(result.is_ok());
            assert_eq!(budget.work(), Bundle::WORK);
        } else {
            assert!(matches!(result, Err(Error::IssuerImageTooLarge)));
            assert_eq!(budget.work(), LOCAL_WORK);
        }
        assert_eq!(budget.storage(), inputs.retained_storage());
    }
}

#[test]
fn native_validation_errors_preserve_typed_causes_and_restore_storage() {
    for case in 0..13 {
        let mut inputs = inputs();
        match case {
            0 => inputs.generation = 0,
            1 => inputs.anchor_verifying_key = inputs.issuer_verifying_key,
            2 => inputs.issuer_verifying_key = [0; 32],
            3 => inputs.anchor_verifying_key = [0; 32],
            4 => inputs.compiler_service_uid = 0,
            5 => inputs.compiler_service_uid = u32::MAX,
            6 => inputs.compiler_service_gid = 0,
            7 => inputs.compiler_service_gid = u32::MAX,
            8 => inputs.compiler_service_uid = inputs.external_anchor_service.uid(),
            // All four bounded executable roles currently share the 128 MiB cap.
            9..=12 => set_measurement(
                &mut inputs,
                [0, 1, 4, 3][case - 9],
                measurement(0x90, 128 * 1024 * 1024 + 1),
            ),
            _ => unreachable!(),
        }
        let mut work = Work::new(Bundle::WORK);
        let mut budget = Budget::new(&mut work, inputs.retained_storage() + Bundle::SCRATCH);
        budget.reserve_storage(inputs.retained_storage()).unwrap();
        let error = Bundle::new(&inputs, &mut budget).unwrap_err();
        let accepted_work = match case {
            0..=3 => {
                assert!(matches!(&error, Error::Policy(PolicyError::Framing(_))));
                assert!(error.source().unwrap().is::<PolicyError>());
                LOCAL_WORK + POLICY_WORK
            }
            4..=10 => {
                assert!(matches!(
                    &error,
                    Error::SupervisorDeployment(SupervisorError::Framing(_))
                ));
                assert!(error.source().unwrap().is::<SupervisorError>());
                LOCAL_WORK + POLICY_WORK + SUPERVISOR_WORK
            }
            11 => {
                assert!(matches!(
                    &error,
                    Error::ExternalAnchorDeployment(AnchorError::Framing(_))
                ));
                assert!(error.source().unwrap().is::<AnchorError>());
                LOCAL_WORK + POLICY_WORK + SUPERVISOR_WORK + ANCHOR_WORK
            }
            _ => {
                assert!(matches!(
                    &error,
                    Error::ExternalAnchorProvisioning(ProvisioningError::Framing(_))
                ));
                assert!(error.source().unwrap().is::<ProvisioningError>());
                LOCAL_WORK + POLICY_WORK + SUPERVISOR_WORK + ANCHOR_WORK + PROVISIONING_WORK
            }
        };
        assert_eq!(budget.work(), accepted_work, "case {case}");
        assert_eq!(budget.storage(), inputs.retained_storage());
        assert_eq!(budget.failed_work(), None);
        assert_eq!(budget.failed_storage(), None);
    }
}

#[test]
fn input_floor_is_checked_before_local_scratch_can_cover_it() {
    let inputs = inputs();
    let mut work = Work::new(Bundle::WORK);
    let mut budget = Budget::new(&mut work, inputs.retained_storage() + Bundle::SCRATCH);
    budget
        .reserve_storage(inputs.retained_storage() - 1)
        .unwrap();
    assert!(matches!(
        Bundle::new(&inputs, &mut budget),
        Err(Error::Resource(Resource::Accounting))
    ));
    assert_eq!(budget.storage(), inputs.retained_storage() - 1);
    assert_eq!(budget.peak_storage(), inputs.retained_storage() - 1);
    assert_eq!(budget.work(), ENTRY_WORK);
}

#[test]
fn work_exhaustion_at_each_stage_preserves_the_original_ledger() {
    let inputs = inputs();
    let limits = [
        0,
        ENTRY_WORK - 1,
        LOCAL_WORK - 1,
        LOCAL_WORK + POLICY_WORK - 1,
        LOCAL_WORK + POLICY_WORK + SUPERVISOR_WORK - 1,
        LOCAL_WORK + POLICY_WORK + SUPERVISOR_WORK + ANCHOR_WORK - 1,
        LOCAL_WORK + POLICY_WORK + SUPERVISOR_WORK + ANCHOR_WORK + PROVISIONING_WORK - 1,
        Bundle::WORK - 1,
    ];
    for limit in limits {
        let mut work = Work::new(limit);
        let mut budget = Budget::new(&mut work, inputs.retained_storage() + Bundle::SCRATCH);
        budget.reserve_storage(inputs.retained_storage()).unwrap();
        let ledger = budget.work_ledger_identity_v1();
        let error = Bundle::new(&inputs, &mut budget).unwrap_err();
        assert!(
            matches!(resource(&error), Resource::Work(_)),
            "limit {limit}"
        );
        assert!(budget.work() <= limit);
        assert!(budget.failed_work().unwrap() > limit);
        assert_eq!(budget.storage(), inputs.retained_storage());
        assert!(budget.work_ledger_identity_v1() == ledger);
    }
}

#[test]
fn exact_work_and_peak_succeed_and_one_less_peak_refuses_without_retaining_records() {
    let inputs = inputs();
    let floor = 19 + inputs.retained_storage();
    let mut work = Work::new(Bundle::WORK);
    let mut budget = Budget::new(&mut work, floor + Bundle::SCRATCH);
    budget.reserve_storage(floor).unwrap();
    let (bundle, charge) = Bundle::new(&inputs, &mut budget).unwrap();
    let peak = budget.peak_storage();
    assert_eq!(budget.storage(), floor);
    assert_eq!(budget.work(), Bundle::WORK);
    assert!(peak <= floor + Bundle::SCRATCH);
    assert!(charge.additional_storage() >= size_of::<(Bundle, Storage)>());
    let policy_retained = bundle.policy().retained_storage();
    let supervisor_retained = bundle.supervisor().retained_storage();
    let anchor_retained = bundle.anchor_deployment().retained_storage();
    let provisioning_retained = bundle.anchor_provisioning().retained_storage();
    // Dropping the unreserved return requires no release.
    drop(bundle);

    let boundaries = [
        floor + Bundle::OUTER_STORAGE,
        floor + Bundle::OUTER_STORAGE + POLICY_STORAGE,
        floor + Bundle::OUTER_STORAGE + policy_retained + SUPERVISOR_STORAGE,
        floor + Bundle::OUTER_STORAGE + policy_retained + supervisor_retained + ANCHOR_STORAGE,
        floor
            + Bundle::OUTER_STORAGE
            + policy_retained
            + supervisor_retained
            + anchor_retained
            + PROVISIONING_STORAGE,
        floor
            + Bundle::OUTER_STORAGE
            + policy_retained
            + supervisor_retained
            + anchor_retained
            + provisioning_retained
            + PROFILE_STORAGE,
        peak,
    ];
    for boundary in boundaries {
        // A non-peak stage can already be covered by an earlier, larger frame.
        if boundary > peak {
            continue;
        }
        let mut work = Work::new(Bundle::WORK);
        let mut budget = Budget::new(&mut work, boundary - 1);
        budget.reserve_storage(floor).unwrap();
        let error = Bundle::new(&inputs, &mut budget).unwrap_err();
        assert!(matches!(resource(&error), Resource::Storage(_)));
        assert_eq!(budget.storage(), floor);
        assert!(budget.peak_storage() < boundary);
        assert!(budget.failed_storage().unwrap() >= boundary);
    }
    let mut work = Work::new(Bundle::WORK);
    let mut budget = Budget::new(&mut work, peak);
    budget.reserve_storage(floor).unwrap();
    let bundle = retain(&inputs, &mut budget);
    assert_eq!(budget.peak_storage(), peak);
    assert_eq!(budget.work(), Bundle::WORK);
    let retained = bundle.retained_storage();
    drop(bundle);
    budget.release_storage(retained).unwrap();
    assert_eq!(budget.storage(), floor);
}

#[test]
fn successful_construction_keeps_prior_work_storage_and_first_denials() {
    let inputs = inputs();
    let floor = 19 + inputs.retained_storage();
    let work_limit = 23 + Bundle::WORK;
    let storage_limit = floor + Bundle::SCRATCH;
    let mut work = Work::new(work_limit);
    let mut budget = Budget::new(&mut work, storage_limit);
    budget.charge_work(23).unwrap();
    budget.reserve_storage(floor).unwrap();
    assert!(budget.charge_work(Bundle::WORK + 1).is_err());
    assert!(budget.reserve_storage(Bundle::SCRATCH + 1).is_err());
    let ledger = budget.work_ledger_identity_v1();
    let bundle = retain(&inputs, &mut budget);
    assert_eq!(budget.work(), work_limit);
    assert_eq!(budget.failed_work(), Some(work_limit + 1));
    assert_eq!(budget.failed_storage(), Some(storage_limit + 1));
    assert!(budget.work_ledger_identity_v1() == ledger);
    let retained = bundle.retained_storage();
    drop(bundle);
    budget.release_storage(retained).unwrap();
    assert_eq!(budget.storage(), floor);
}
