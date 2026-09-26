use super::tests::{LIMITS, SPACE, WORK, input, source};
use super::*;
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;

#[test]
fn production_orchestrator_keeps_the_original_resource_failure_and_caller_floor() {
    let (input, storage) = input(&source());
    let mut work = Work::new(15);
    work.charge_work(11).unwrap();
    let mut budget = Budget::new(&mut work, SPACE);
    budget.reserve_storage(29 + storage).unwrap();
    assert!(matches!(
        optimize_checked_canonical_kernel_ir_v18(&input, LIMITS, &mut budget),
        Err(KernelIrCheckedOptimizationErrorV18::Observation(
            KirNeutralOptimizationErrorV18::Resource(Resource::Work(_))
        ))
    ));
    assert_eq!(budget.storage(), 29 + storage);
    assert_eq!(budget.work(), 11);
    assert_eq!(work.failed_work(), Some(16));
}

#[test]
fn fixed_execution_and_adoption_do_not_reset_preexisting_denial_history() {
    let (input, storage) = input(&source());
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, SPACE);
    budget.reserve_storage(41 + storage).unwrap();
    assert!(budget.charge_work(WORK + 1).is_err());
    assert!(budget.reserve_storage(SPACE + 1).is_err());
    let denied_storage = budget.failed_storage();
    let checked = optimize_checked_canonical_kernel_ir_v18(&input, LIMITS, &mut budget).unwrap();
    assert_eq!(budget.storage(), 41 + storage);
    assert_eq!(budget.failed_storage(), denied_storage);
    assert_eq!(checked.report().passes().len(), 8);
    drop(budget);
    assert_eq!(work.failed_work(), Some(WORK + 1));
}
