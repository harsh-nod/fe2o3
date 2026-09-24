use super::*;
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;

#[test]
fn legacy_stage_entry_preserves_atomic_work_charge_and_error_precedence() {
    for available in [67, 68] {
        for floor in [52, 53] {
            let mut work = Work::new(17 + available);
            let mut budget = Budget::new(&mut work, 100);
            budget.charge_work(17).unwrap();
            budget.reserve_storage(floor).unwrap();
            let ledger = budget.work_ledger_identity_v1();
            let result = check_stage_entry(53, &mut budget);
            if available == 67 {
                assert!(matches!(result, Err(E::Resource(Resource::Work(error)))
                    if error.actual() == 17 + 68 && error.limit() == 17 + 67));
                assert_eq!(budget.work(), 17);
            } else {
                assert_eq!(budget.work(), 17 + 68);
                if floor == 52 {
                    assert!(matches!(result, Err(E::Resource(Resource::Accounting))));
                } else {
                    result.unwrap();
                }
            }
            assert_eq!(budget.storage(), floor);
            assert!(budget.work_ledger_identity_v1() == ledger);
        }
    }
}
