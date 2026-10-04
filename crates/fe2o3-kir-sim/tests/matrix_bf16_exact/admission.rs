use super::*;
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget, CanonicalKernelIrWorkBudgetV1 as Work,
    VerifiedCanonicalKernelIrModuleV12 as Owner,
};
fn owner() -> (Owner, usize) {
    let mut work = Work::new(32_000_000);
    let mut budget = Budget::new(&mut work, 32_000_000);
    let (owner, receipt) =
        Owner::from_module_ref_with_verification_budget_v12(&module(64), &mut budget).unwrap();
    (owner, receipt.retained_storage())
}
#[test]
fn borrowed_v12_view_keeps_exact_owner_and_transfers_only_storage() {
    let (owner, source_bytes) = owner();
    let original = owner.canonical().identity();
    let mut work = Work::new(32_000_000);
    let mut budget = Budget::new(&mut work, 32_000_000);
    let floor = source_bytes + 73;
    budget.reserve_storage(floor).unwrap();
    budget.charge_work(29).unwrap();
    let (view, receipt) = AdmittedSimulationModuleV1::admit_v12_with_verification_budget(
        &owner,
        limits(),
        &mut budget,
    )
    .unwrap();
    assert_eq!(budget.storage(), floor);
    assert!(budget.work() > 29);
    assert_eq!(view.identity().digest(), original.digest());
    assert_eq!(view.identity().wire_version(), 12);
    assert_eq!(receipt.retained_storage(), view.admitted_resident_bytes());
    budget.reserve_storage(receipt.retained_storage()).unwrap();
    drop(view);
    budget.release_storage(receipt.retained_storage()).unwrap();
    assert_eq!(budget.storage(), floor);
    assert_eq!(owner.canonical().identity(), original);
}
#[test]
fn exact_and_one_short_v12_view_resources_preserve_original_histories() {
    let (owner, source_bytes) = owner();
    let floor = source_bytes + 73;
    let run = |work_limit, storage_limit| {
        let mut work = Work::new(work_limit);
        let mut budget = Budget::new(&mut work, storage_limit);
        budget.reserve_storage(floor).unwrap();
        budget.charge_work(29).unwrap();
        let accepted = AdmittedSimulationModuleV1::admit_v12_with_verification_budget(
            &owner,
            limits(),
            &mut budget,
        )
        .is_ok();
        assert_eq!(budget.storage(), floor);
        let w = budget.work();
        let peak = budget.peak_storage();
        let denied_storage = budget.failed_storage();
        (accepted, w, peak, work.failed_work(), denied_storage)
    };
    let (accepted, work, peak, _, _) = run(32_000_000, 32_000_000);
    assert!(accepted);
    assert!(run(work, peak).0);
    let short_work = run(work - 1, peak);
    assert!(!short_work.0);
    assert!(short_work.3.is_some());
    let short_storage = run(work, peak - 1);
    assert!(!short_storage.0);
    assert!(short_storage.4.is_some());
}
#[test]
fn v12_view_refuses_limits_without_resetting_floor_or_prior_denials() {
    let (owner, source_bytes) = owner();
    let mut work = Work::new(32_000_000);
    let mut budget = Budget::new(&mut work, 32_000_000);
    let floor = source_bytes + 73;
    budget.reserve_storage(floor).unwrap();
    assert!(budget.reserve_storage(usize::MAX).is_err());
    assert!(budget.charge_work(usize::MAX).is_err());
    let restricted = SimulationLimitsV1 {
        max_canonical_bytes: 1,
        ..limits()
    };
    assert!(
        AdmittedSimulationModuleV1::admit_v12_with_verification_budget(
            &owner,
            restricted,
            &mut budget
        )
        .is_err()
    );
    assert_eq!(budget.storage(), floor);
    assert_eq!(budget.failed_storage(), Some(usize::MAX));
    assert_eq!(work.failed_work(), Some(usize::MAX));
}
