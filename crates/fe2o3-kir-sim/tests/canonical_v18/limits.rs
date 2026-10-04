use super::*;

#[test]
fn exact_and_one_short_original_resources_preserve_floor_and_history() {
    let raw = module();
    let run = |work_limit, storage_limit| {
        let mut work = Work::new(work_limit);
        let mut budget = Budget::new(&mut work, storage_limit);
        budget.reserve_storage(FLOOR).unwrap();
        budget.charge_work(29).unwrap();
        let ledger = budget.work_ledger_identity_v1();
        let (canonical, owner_paid) = owner(&raw, &mut budget);
        // Make this view's peak dominate earlier owner admission while keeping
        // both the owner and a distinct caller-owned floor on the same account.
        let padding = budget.peak_storage().checked_sub(budget.storage()).unwrap() + FLOOR;
        budget.reserve_storage(padding).unwrap();
        let floor = budget.storage();
        let result = AdmittedSimulationModuleV1::admit_v18_with_verification_budget(
            &canonical,
            SimulationLimitsV1::default(),
            &mut budget,
        );
        assert_eq!(budget.storage(), floor);
        let ok = match result {
            Ok((view, receipt)) => {
                budget.reserve_storage(receipt.retained_storage()).unwrap();
                drop(view);
                budget.release_storage(receipt.retained_storage()).unwrap();
                true
            }
            Err(error) => {
                assert!(matches!(
                    error,
                    SimulationViewAdmissionErrorV18::Resource(_)
                        | SimulationViewAdmissionErrorV18::CanonicalView(_)
                ));
                false
            }
        };
        assert_eq!(budget.storage(), floor);
        assert!(budget.work_ledger_identity_v1() == ledger);
        drop(canonical);
        budget.release_storage(owner_paid).unwrap();
        budget.release_storage(padding).unwrap();
        assert_eq!(budget.storage(), FLOOR);
        (
            ok,
            budget.work(),
            budget.peak_storage(),
            budget.failed_work(),
            budget.failed_storage(),
        )
    };
    let (ok, work, peak, work_denial, storage_denial) = run(BOUND, BOUND);
    assert!(ok);
    assert!(work_denial.is_none());
    assert!(storage_denial.is_none());
    assert!(run(work, peak).0);
    let one_short = run(work - 1, peak);
    assert!(!one_short.0);
    assert!(one_short.3.is_some());
    let one_short = run(work, peak - 1);
    assert!(!one_short.0);
    assert!(one_short.4.is_some());
}

#[test]
fn prior_denials_survive_success_and_failure_without_replacing_budget() {
    let mut work = Work::new(BOUND);
    let mut budget = Budget::new(&mut work, BOUND);
    budget.reserve_storage(FLOOR).unwrap();
    let (canonical, owner_paid) = owner(&module(), &mut budget);
    assert!(budget.charge_work(BOUND).is_err());
    assert!(budget.reserve_storage(BOUND).is_err());
    let prior_work = budget.failed_work();
    let prior_storage = budget.failed_storage();
    let prior_peak = budget.peak_storage();
    let ledger = budget.work_ledger_identity_v1();
    let floor = budget.storage();
    let (view, view_paid) = view(&canonical, &mut budget);
    assert_eq!(budget.failed_work(), prior_work);
    assert_eq!(budget.failed_storage(), prior_storage);
    assert!(budget.peak_storage() >= prior_peak);
    drop(view);
    budget.release_storage(view_paid).unwrap();
    let limits = SimulationLimitsV1 {
        max_canonical_bytes: 1,
        ..SimulationLimitsV1::default()
    };
    assert!(
        AdmittedSimulationModuleV1::admit_v18_with_verification_budget(
            &canonical,
            limits,
            &mut budget,
        )
        .is_err()
    );
    assert_eq!(budget.storage(), floor);
    assert_eq!(budget.failed_work(), prior_work);
    assert_eq!(budget.failed_storage(), prior_storage);
    assert!(budget.work_ledger_identity_v1() == ledger);
    drop(canonical);
    budget.release_storage(owner_paid).unwrap();
    assert_eq!(budget.storage(), FLOOR);
}

#[test]
fn canonical_and_resident_limits_are_exact_and_restore_owner_reservation() {
    let mut work = Work::new(BOUND);
    let mut budget = Budget::new(&mut work, BOUND);
    budget.reserve_storage(FLOOR).unwrap();
    let (canonical, owner_paid) = owner(&module(), &mut budget);
    let floor = budget.storage();
    let bytes = canonical.canonical_bytes().len();
    let before_work = budget.work();
    let before_peak = budget.peak_storage();
    let too_short = SimulationLimitsV1 {
        max_canonical_bytes: bytes - 1,
        ..SimulationLimitsV1::default()
    };
    assert!(matches!(
        AdmittedSimulationModuleV1::admit_v18_with_verification_budget(
            &canonical,
            too_short,
            &mut budget,
        ),
        Err(SimulationViewAdmissionErrorV18::Admission(
            SimulationAdmissionErrorV1::CanonicalBytesLimit { .. }
        ))
    ));
    assert_eq!(budget.work(), before_work + 1);
    assert_eq!(budget.storage(), floor);
    assert_eq!(budget.peak_storage(), before_peak);
    let (view, paid) = view(&canonical, &mut budget);
    let resident = view.admitted_resident_bytes() + bytes;
    drop(view);
    budget.release_storage(paid).unwrap();
    for (wire, memory, accepted) in [
        (bytes, resident, true),
        (bytes - 1, resident, false),
        (bytes, resident - 1, false),
        (bytes + 1, resident + 1, true),
    ] {
        let limits = SimulationLimitsV1 {
            max_canonical_bytes: wire,
            max_resident_bytes: memory,
            ..SimulationLimitsV1::default()
        };
        let before_peak = budget.peak_storage();
        let result = AdmittedSimulationModuleV1::admit_v18_with_verification_budget(
            &canonical,
            limits,
            &mut budget,
        );
        assert_eq!(budget.storage(), floor);
        assert_eq!(result.is_ok(), accepted);
        match result {
            Ok((view, receipt)) => {
                assert_eq!(receipt.retained_storage(), paid);
                budget.reserve_storage(receipt.retained_storage()).unwrap();
                drop(view);
                budget.release_storage(receipt.retained_storage()).unwrap();
            }
            Err(SimulationViewAdmissionErrorV18::Admission(
                SimulationAdmissionErrorV1::CanonicalBytesLimit { actual, limit },
            )) => {
                assert_eq!((actual, limit), (bytes, bytes - 1));
                assert_eq!(budget.peak_storage(), before_peak);
            }
            Err(SimulationViewAdmissionErrorV18::Admission(
                SimulationAdmissionErrorV1::ResidentBytesLimit { actual, limit, .. },
            )) => assert_eq!((actual, limit), (resident, resident - 1)),
            other => panic!("unexpected admission result: {other:?}"),
        }
        assert_eq!(budget.storage(), floor);
    }
    let invalid = SimulationLimitsV1 {
        max_canonical_bytes: 0,
        ..SimulationLimitsV1::default()
    };
    assert!(matches!(
        AdmittedSimulationModuleV1::admit_v18_with_verification_budget(
            &canonical,
            invalid,
            &mut budget,
        ),
        Err(SimulationViewAdmissionErrorV18::Admission(
            SimulationAdmissionErrorV1::InvalidLimits(_)
        ))
    ));
    assert_eq!(budget.storage(), floor);
    drop(canonical);
    budget.release_storage(owner_paid).unwrap();
    assert_eq!(budget.storage(), FLOOR);
}
