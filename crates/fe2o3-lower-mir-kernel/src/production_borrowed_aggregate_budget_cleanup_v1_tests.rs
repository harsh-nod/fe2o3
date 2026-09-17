use super::*;
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;

#[test]
fn late_borrowed_payload_accounting_failure_restores_lowering_floor() {
    let source = production_borrowed_aggregate_replay_v1_tests::owner_source();
    let run = |limit| {
        let mut work = Work::new(limit);
        let mut budget = ArgumentBudgetV1::new(&mut work, usize::MAX);
        let mut origin_work = Work::new(usize::MAX);
        let mut origin_budget = ArgumentBudgetV1::new(&mut origin_work, usize::MAX);
        let mut origins = AssertOriginEmissionV1::new(&mut origin_budget);
        let mut admission = HelperLoweringAdmissionV1::PendingSource {
            requires_source: false,
            requires_borrowed: false,
        };
        budget.reserve_storage(23).unwrap();
        let result = lower_module_with_call_budget_for_helper_admission_v1(
            &source,
            ProductionSemanticKirLimitsV1::default(),
            None,
            Some(&mut origins),
            &mut budget,
            &mut admission,
        );
        let used = budget.work();
        if result.is_err() {
            assert_eq!(budget.storage(), 23);
        }
        (result, used)
    };
    let (output, exact) = run(usize::MAX);
    let (_, rows) = output.expect("actual borrowed source must reach payload accounting");
    assert!(!rows.borrowed_aggregate_calls.is_empty());
    assert!(run(exact).0.is_ok());
    assert!(matches!(
        run(exact - 1).0,
        Err(
            ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(ArgumentResourceV1::Work(
                _
            ))
        )
    ));
}
