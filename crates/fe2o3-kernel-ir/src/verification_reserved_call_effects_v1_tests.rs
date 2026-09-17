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
            "__fe2o3_ir_amdgpu_diagnostics_gfx950_v1_missing",
            "__fe2o3_ir_amdgpu_diagnostics_gfx951_v1_realtime64",
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

#[test]
fn realtime_v32_reserved_declaration_requires_u64_and_both_exact_capabilities() {
    let valid = AmdGpuDiagnosticOperation::Realtime64.declaration();
    let descriptor = AmdGpuDiagnosticIntrinsicDescriptorV1::Realtime64;
    let check = |function: &Function, limit| {
        let mut work = Work::new(limit);
        let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, 0);
        let result = diagnostic_declaration_matches_v1(function, descriptor, &mut budget);
        assert_eq!(budget.storage(), 0);
        (result, budget.work())
    };
    let (result, cost) = check(&valid, 1_000_000);
    assert_eq!(result, Ok(true));
    assert_eq!(check(&valid, cost).0, Ok(true));
    assert!(matches!(
        check(&valid, cost - 1).0,
        Err(CanonicalKernelIrVerificationResourceErrorV1::Work(_))
    ));
    for mutation in 0..6 {
        let mut wrong = valid.clone();
        match mutation {
            0 => wrong.signature.results[0] = Type::Scalar(ScalarType::U32),
            1 => wrong
                .signature
                .parameters
                .push(Type::Scalar(ScalarType::U32)),
            2 => {
                wrong
                    .required_capabilities
                    .remove(&crate::gfx950_xnack_minus_target_capability());
            }
            3 => {
                wrong.required_capabilities.clear();
                wrong
                    .required_capabilities
                    .insert(crate::gfx950_xnack_minus_target_capability());
                wrong
                    .required_capabilities
                    .insert(TargetCapability::Extension {
                        namespace: AMDGPU_GFX942_DIAGNOSTICS_CAPABILITY_NAMESPACE.into(),
                        name: AMDGPU_GFX942_DIAGNOSTICS_CAPABILITY_NAME.into(),
                    });
            }
            4 => wrong.role = FunctionRole::InternalHelper,
            5 => wrong.signature.results.clear(),
            _ => unreachable!(),
        }
        assert_eq!(check(&wrong, 1_000_000).0, Ok(false), "mutation {mutation}");
    }
}
