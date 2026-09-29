use super::*;
use crate::{CanonicalKernelIrWorkBudgetV1 as Work, Operation, OperationKind, ValueId};

#[test]
fn registered_trap_query_is_closed_allocation_free_and_exactly_metered() {
    let names = AmdGpuDiagnosticOperation::intrinsic_descriptor_roster_v1()
        .map(|(name, descriptor)| {
            (
                name,
                matches!(descriptor, AmdGpuDiagnosticIntrinsicDescriptorV1::Trap),
            )
        })
        .chain([
            ("ordinary", false),
            ("__fe2o3_ir_amdgpu_diagnostics_gfx942_v1_missing", false),
        ]);
    for (name, trap) in names {
        for arguments in 0..3 {
            for results in 0..2 {
                let operation = Operation::new(
                    vec![crate::ValueDef::new(ValueId(5), Type::Scalar(ScalarType::U32)); results],
                    OperationKind::Call {
                        callee: name.into(),
                        arguments: vec![ValueId(0); arguments],
                    },
                );
                let mut work = Work::new(1_000_000);
                let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, 0);
                let expected = trap && arguments == 0 && results == 0;
                assert_eq!(
                    operation
                        .has_registered_trap_contract_with_budget_v26(&mut budget)
                        .unwrap(),
                    expected
                );
                let cost = budget.work();
                let expected_cost =
                    4 + AmdGpuDiagnosticOperation::INTRINSIC_DESCRIPTOR_COUNT_V1 * (name.len() + 2);
                assert_eq!(cost, expected_cost);
                for limit in [cost, cost - 1, 0] {
                    let mut work = Work::new(limit);
                    let mut budget =
                        CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, 0);
                    let observed =
                        operation.has_registered_trap_contract_with_budget_v26(&mut budget);
                    if limit == cost {
                        assert_eq!(observed.unwrap(), expected);
                    } else {
                        assert!(matches!(
                            observed,
                            Err(CanonicalKernelIrVerificationResourceErrorV1::Work(_))
                        ));
                    }
                    assert_eq!(budget.storage(), 0);
                }
            }
        }
    }
}

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
