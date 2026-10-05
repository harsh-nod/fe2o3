use super::tests::{LIMITS, SPACE, WORK, input};
use super::*;
use fe2o3_kernel_ir::{CanonicalKernelIrWorkBudgetV1 as Work, Module};

#[test]
fn original_work_floor_and_atomic_cleanup_admission_survive_first_denial() {
    let input = input(&Module::new("empty"));
    for allowance in [4, 5, 9] {
        let mut work = Work::new(11 + allowance);
        work.charge_work(11).unwrap();
        let mut budget = Budget::new(&mut work, SPACE);
        let floor = 37 + input.storage;
        budget.reserve_storage(floor).unwrap();
        let result = optimize_neutral_kernel_ir_v18(&input.owner, LIMITS, &mut budget);
        assert!(result.is_err());
        assert_eq!(budget.storage(), floor);
        // Four bounded destructor retries are paid atomically with entry.
        assert_eq!(budget.work(), 11 + if allowance < 5 { 0 } else { 5 });
        assert_eq!(
            work.failed_work(),
            Some(11 + if allowance < 5 { 5 } else { 10 })
        );
    }
}

#[test]
fn first_storage_denial_does_not_construct_or_publish_a_graph() {
    let input = input(&Module::new("empty"));
    let floor = 23 + input.storage;
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, floor);
    budget.reserve_storage(floor).unwrap();
    assert!(matches!(
        optimize_neutral_kernel_ir_v18(&input.owner, LIMITS, &mut budget),
        Err(KirNeutralOptimizationErrorV18::Resource(Resource::Storage(
            _
        )))
    ));
    assert_eq!(budget.work(), 5);
    assert_eq!(budget.storage(), floor);
}

#[test]
fn inherited_policy3_profile_has_independent_literal_work_and_storage_oracles() {
    // Canonical byte length B=37, volume 32806, structural registered N=1.
    // Policy-3 map capture: work 144; persistent capture payload 5888.
    let (profile, capture) =
        crate::optimization_v12::policy3_execution_resources_v1(37, 1).unwrap();
    assert_eq!(profile.work(), 31_567_120);
    assert_eq!(profile.persistent_storage(), 272_432);
    assert_eq!(profile.temporary_storage(), 2_628_576);
    assert_eq!(
        profile.retained_storage(),
        size_of::<PlironOptimizationReportV1>()
            + 8 * size_of::<crate::PlironOptimizationPassReportV1>()
    );
    assert_eq!(capture.work().unwrap(), 144);
    assert_eq!(capture.storage().unwrap(), 5888);
    assert!(crate::optimization_v12::policy3_execution_resources_v1(usize::MAX, 1).is_err());
}
