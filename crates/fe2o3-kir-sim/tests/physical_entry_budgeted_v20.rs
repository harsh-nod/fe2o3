//! Diagnostic view admission does not create source authority.
use fe2o3_kernel_ir as physical_entry_fixture_ir;
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget, CanonicalKernelIrWorkBudgetV1 as Work,
    VerifiedCanonicalKernelIrModuleV20 as Owner,
};
use fe2o3_kir_sim::*;
#[path = "../../fe2o3-kernel-ir/tests/fixtures/physical_entry_v20.rs"]
mod fixture;
fn owner(select: bool) -> Owner {
    let mut work = Work::new(16_000_000);
    Owner::from_module_ref_with_verification_budget_v20(
        &fixture::module(select),
        &mut Budget::new(&mut work, 16_000_000),
    )
    .unwrap()
    .0
}
#[test]
fn admitted_view_identity_and_receipt_are_from_exact_owner() {
    for select in [false, true] {
        let owner = owner(select);
        let mut work = Work::new(16_000_000);
        let mut b = Budget::new(&mut work, 16_000_000);
        b.reserve_storage(73).unwrap();
        let (view, receipt) = AdmittedSimulationModuleV1::admit_v20_with_verification_budget(
            &owner,
            SimulationLimitsV1::default(),
            &mut b,
        )
        .unwrap();
        assert_eq!(view.identity().digest(), owner.identity().digest());
        assert_eq!(receipt.retained_storage(), view.admitted_resident_bytes());
        assert_eq!(b.storage(), 73);
    }
}
#[test]
fn exact_and_one_short_admission_resources_preserve_prior_work_and_storage() {
    let owner = owner(true);
    let run = |wl, sl| {
        let mut work = Work::new(wl);
        let mut b = Budget::new(&mut work, sl);
        b.reserve_storage(73).unwrap();
        b.charge_work(29).unwrap();
        let ok = AdmittedSimulationModuleV1::admit_v20_with_verification_budget(
            &owner,
            SimulationLimitsV1::default(),
            &mut b,
        )
        .is_ok();
        assert_eq!(b.storage(), 73);
        let accepted = b.work();
        let peak = b.peak_storage();
        let failed_storage = b.failed_storage();
        (ok, accepted, peak, work.failed_work(), failed_storage)
    };
    let (ok, w, p, _, _) = run(16_000_000, 16_000_000);
    assert!(ok);
    assert!(run(w, p).0);
    let short = run(w - 1, p);
    assert!(!short.0);
    assert!(short.3.is_some());
    let short = run(w, p - 1);
    assert!(!short.0);
    assert!(short.4.is_some());
}
#[test]
fn invalid_limits_refuse_without_losing_nonzero_floor() {
    let owner = owner(false);
    let mut work = Work::new(16_000_000);
    let mut b = Budget::new(&mut work, 16_000_000);
    b.reserve_storage(73).unwrap();
    let limits = SimulationLimitsV1 {
        max_canonical_bytes: 1,
        ..SimulationLimitsV1::default()
    };
    assert!(
        AdmittedSimulationModuleV1::admit_v20_with_verification_budget(&owner, limits, &mut b)
            .is_err()
    );
    assert_eq!(b.storage(), 73);
}
