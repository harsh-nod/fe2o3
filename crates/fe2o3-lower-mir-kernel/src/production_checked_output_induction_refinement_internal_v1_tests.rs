//! Reuses the original admitted source fixtures without adding a source constructor.
use super::*;
use fe2o3_kernel_analysis::CanonicalKirLoopLimitsV1 as Limits;

#[test]
fn source_induction_refinement_genuine_direct_rejects_origins_limits_reports_and_receipts() {
    for profile in [Profile::Gfx942, Profile::Gfx950] {
        let (prefix, inherited) = direct::prefix(profile, false);
        let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
        let mut budget = AssertOriginBudgetV1::new(&mut work, STORAGE);
        budget.reserve_storage(inherited).unwrap();
        let (prefix, storage) = prefix.continue_licm_v1(&mut budget).unwrap();
        budget.reserve_storage(storage.retained_storage()).unwrap();
        let (mut owner, storage) = prefix
            .continue_induction_refinement_v1(Limits::default(), &mut budget)
            .unwrap();
        budget.reserve_storage(storage.retained_storage()).unwrap();
        assert_eq!(
            owner.output().canonical().canonical_bytes(),
            owner.prefix().output().canonical().canonical_bytes()
        );
        owner.exercise_induction_refinement_hostile_v1(&mut budget);
    }
}

#[test]
fn source_induction_refinement_genuine_unit_local_rejects_origins_limits_reports_and_receipts() {
    for profile in [Profile::Gfx942, Profile::Gfx950] {
        let (prefix, inherited) = erased::prefix(profile, false);
        let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
        let mut budget = AssertOriginBudgetV1::new(&mut work, STORAGE);
        budget.reserve_storage(inherited).unwrap();
        let (prefix, storage) = prefix.continue_licm_v1(&mut budget).unwrap();
        budget.reserve_storage(storage.retained_storage()).unwrap();
        let (mut owner, storage) = prefix
            .continue_induction_refinement_v1(Limits::default(), &mut budget)
            .unwrap();
        budget.reserve_storage(storage.retained_storage()).unwrap();
        assert_eq!(owner.kernels().len(), 2);
        assert_eq!(
            owner.output().canonical().canonical_bytes(),
            owner.prefix().output().canonical().canonical_bytes()
        );
        owner.exercise_induction_refinement_hostile_v1(&mut budget);
    }
}

#[test]
fn source_induction_refinement_real_direct_candidate_error_and_panic_keep_sibling_credit() {
    let (prefix, inherited) = direct::prefix(Profile::Gfx942, false);
    let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
    let mut budget = AssertOriginBudgetV1::new(&mut work, STORAGE);
    budget.reserve_storage(inherited).unwrap();
    let (prefix, storage) = prefix.continue_licm_v1(&mut budget).unwrap();
    budget.reserve_storage(storage.retained_storage()).unwrap();
    let floor = budget.storage();
    prefix.exercise_induction_refinement_partial_failure_v1(&mut budget);
    assert_eq!(budget.storage(), floor);
    prefix.verify_equivalence(&mut budget).unwrap();
}

#[test]
fn source_induction_refinement_real_unit_local_candidate_error_and_panic_keep_sibling_credit() {
    let (prefix, inherited) = erased::prefix(Profile::Gfx942, false);
    let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
    let mut budget = AssertOriginBudgetV1::new(&mut work, STORAGE);
    budget.reserve_storage(inherited).unwrap();
    let (prefix, storage) = prefix.continue_licm_v1(&mut budget).unwrap();
    budget.reserve_storage(storage.retained_storage()).unwrap();
    let floor = budget.storage();
    prefix.exercise_induction_refinement_partial_failure_v1(&mut budget);
    assert_eq!(budget.storage(), floor);
    prefix.verify_equivalence(&mut budget).unwrap();
}
