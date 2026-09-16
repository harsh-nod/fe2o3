use super::*;
use crate::{CanonicalKernelIrWorkBudgetV1 as Work, Operation, OperationKind, ValueId};

#[test]
fn reserved_call_effect_classifier_matches_every_descriptor_and_arity() {
    let names = AmdGpuDiagnosticOperation::intrinsic_descriptor_roster_v1()
        .map(|(name, _)| name)
        .chain(FloatOperation::intrinsic_descriptor_roster_v1().map(|(name, _)| name))
        .chain([
            "ordinary",
            "__fe2o3_ir_float_v1_missing",
            "__fe2o3_ir_amdgpu_diagnostics_gfx942_v1_missing",
        ]);
    for name in names {
        for count in 0..6 {
            let operation = Operation::new(
                vec![],
                OperationKind::Call {
                    callee: name.into(),
                    arguments: vec![ValueId(0); count],
                },
            );
            let mut work = Work::new(1_000_000);
            let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, 0);
            let expected = operation.has_complete_effect_summary();
            assert_eq!(
                operation
                    .has_complete_effect_summary_with_budget_v1(&mut budget)
                    .unwrap(),
                expected,
                "{name}/{count}"
            );
            let cost = budget.work();
            for limit in [cost, cost - 1, 0] {
                let mut work = Work::new(limit);
                let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, 0);
                let actual = operation.has_complete_effect_summary_with_budget_v1(&mut budget);
                if limit == cost {
                    assert_eq!(actual.unwrap(), expected);
                } else {
                    assert!(matches!(
                        actual,
                        Err(CanonicalKernelIrVerificationResourceErrorV1::Work(_))
                    ));
                }
                assert_eq!(budget.storage(), 0);
            }
        }
    }
}
