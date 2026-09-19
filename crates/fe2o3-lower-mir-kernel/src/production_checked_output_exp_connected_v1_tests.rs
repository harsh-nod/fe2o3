use super::*;
use fe2o3_kernel_ir::{F32MathFunction, F32MathImplementation, FloatOperation};

include!("production_checked_output_exp_fixture_v1_tests.rs");
#[path = "production_checked_output_exp_native_v1_tests.rs"]
mod exp_native;

fn prepared(helper: bool, profile: Profile, mutation: Option<fn(&mut Module)>) -> FpPrepared4 {
    fp_prepare_receipt4(exp_receipt(helper), profile, mutation)
}

fn exp_census(module: &Module, helper: bool) {
    let mut counts = (0, 0, 0);
    let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
    let mut budget = AssertOriginBudgetV1::new(&mut work, 0);
    for function in &module.functions {
        if function.role == FunctionRole::ExternalImport {
            assert_eq!(
                FloatOperation::f32_math_descriptor_with_budget_v1(&function.id, &mut budget)
                    .unwrap(),
                Some((F32MathFunction::Exp, F32MathImplementation::OcmlAbiV1))
            );
            assert_eq!(function.signature.parameters, [Type::F32]);
            assert_eq!(function.signature.results, [Type::F32]);
            assert!(function.required_capabilities.is_empty());
            counts.0 += 1;
        }
        for operation in function
            .body
            .iter()
            .flat_map(|body| &body.blocks)
            .flat_map(|block| &block.operations)
        {
            if let OperationKind::Call { callee, arguments } = &operation.kind
                && FloatOperation::f32_math_descriptor_with_budget_v1(callee, &mut budget)
                    .unwrap()
                    .is_some()
            {
                assert_eq!(
                    FloatOperation::f32_math_descriptor_with_budget_v1(callee, &mut budget)
                        .unwrap(),
                    Some((F32MathFunction::Exp, F32MathImplementation::OcmlAbiV1))
                );
                assert_eq!(arguments.len(), 1);
                assert_eq!(function.role == FunctionRole::InternalHelper, helper);
                counts.1 += 1;
            }
            if matches!(operation.kind, OperationKind::Store { .. }) {
                counts.2 += 1;
            }
        }
    }
    assert_eq!(counts, (1, 1, 1));
}

#[test]
fn typed_exp_source_replays_actual_n_b_c_o_and_strict_native_on_both_profiles() {
    for helper in [false, true] {
        for profile in [Profile::Gfx942, Profile::Gfx950] {
            let input = prepared(helper, profile, None);
            exp_census(input.receipt.materialized.executable().module(), helper);
            exp_census(input.bound.module(), helper);
            exp_census(
                input.checked.intermediate_policy3().owner().module(),
                helper,
            );
            let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
            let mut budget = AssertOriginBudgetV1::new(&mut work, STORAGE);
            budget.reserve_storage(input.floor).unwrap();
            let owner = crate::ProductionCheckedOutputOwnerPolicy4V1::try_admit_v1(
                input.receipt,
                input.bound,
                input.checked,
                &mut budget,
            )
            .unwrap();
            exp_census(owner.output().module(), helper);
            owner.verify_equivalence(&mut budget).unwrap();
            assert_eq!(owner.kernels()[0].accesses().len(), 1);
            assert!(!owner.grants_artifact_or_launch_authority());
            exp_native::check_actual(&owner, profile, &mut budget);
            assert_eq!(budget.storage(), input.floor);
            let canonical = owner.output().canonical();
            let decoded = VerifiedCanonicalKernelIrV12::from_canonical_bytes(
                canonical.canonical_bytes().to_vec(),
            )
            .unwrap();
            assert_eq!(decoded.identity(), canonical.identity());
            assert_eq!(decoded.canonical_bytes(), canonical.canonical_bytes());
            let simulation = AdmittedSimulationModuleV1::admit_v12(decoded, fp_limits()).unwrap();
            assert!(!simulation.grants_execution_authority());
            let error = simulation
                .preflight(
                    &fp_request(1.0f32.to_bits(), 2.0f32.to_bits()),
                    SimulationTargetV1::amdgpu_64(),
                    fp_limits(),
                )
                .unwrap_err();
            let fe2o3_kir_sim::SimulationPreflightErrorV1::Unsupported(report) = error else {
                panic!("OCML Exp has no admitted numerical simulator: {error:?}");
            };
            assert!(report.findings().iter().any(|finding| finding.feature
                == fe2o3_kir_sim::UnsupportedFeatureV1::FloatFunction(F32MathFunction::Exp)));
        }
    }
}

fn mutate_operand(module: &mut Module) {
    for body in module
        .functions
        .iter_mut()
        .filter_map(|function| function.body.as_mut())
    {
        let other = *body.parameters.last().unwrap();
        for operation in body
            .blocks
            .iter_mut()
            .flat_map(|block| &mut block.operations)
        {
            if let OperationKind::Call { arguments, .. } = &mut operation.kind
                && arguments.len() == 1
            {
                assert_ne!(arguments[0], other);
                arguments[0] = other;
                return;
            }
        }
    }
    panic!("Exp call absent");
}

#[test]
fn valid_but_changed_exp_operand_or_unrelated_history_cannot_replay_source() {
    for helper in [false, true] {
        for profile in [Profile::Gfx942, Profile::Gfx950] {
            let input = prepared(helper, profile, Some(mutate_operand));
            let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
            let mut budget = AssertOriginBudgetV1::new(&mut work, STORAGE);
            budget.reserve_storage(input.floor).unwrap();
            let result = crate::ProductionCheckedOutputOwnerPolicy4V1::try_admit_v1(
                input.receipt,
                input.bound,
                input.checked,
                &mut budget,
            );
            assert!(
                matches!(
                    result,
                    Err(
                        crate::ProductionCheckedOutputAdmissionErrorPolicy4V1::Admission(
                            AdmissionError::Coordinates(_)
                        )
                    )
                ),
                "{result:?}"
            );
            assert_eq!(budget.storage(), input.floor);
            let input = prepared(helper, profile, None);
            let other = prepared(helper, profile, Some(mutate_operand));
            let floor = input.floor + other.floor;
            let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
            let mut budget = AssertOriginBudgetV1::new(&mut work, STORAGE);
            budget.reserve_storage(floor).unwrap();
            assert!(
                crate::ProductionCheckedOutputOwnerPolicy4V1::try_admit_v1(
                    input.receipt,
                    input.bound,
                    other.checked,
                    &mut budget
                )
                .is_err()
            );
            assert_eq!(budget.storage(), floor);
        }
    }
}

#[test]
fn exact_exp_owner_work_storage_determinism_and_one_short_unwind() {
    for helper in [false, true] {
        let input = prepared(helper, Profile::Gfx942, None);
        let expected = input.checked.owner().canonical().canonical_bytes().to_vec();
        let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
        let mut budget = AssertOriginBudgetV1::new(&mut work, STORAGE);
        budget.reserve_storage(input.floor).unwrap();
        drop(
            crate::ProductionCheckedOutputOwnerPolicy4V1::try_admit_v1(
                input.receipt,
                input.bound,
                input.checked,
                &mut budget,
            )
            .unwrap(),
        );
        let exact_work = budget.work();
        let exact_storage = budget.peak_storage();
        assert_eq!(budget.storage(), input.floor);
        for (work_limit, storage_limit, success) in [
            (exact_work, exact_storage, true),
            (exact_work - 1, exact_storage, false),
            (exact_work, exact_storage - 1, false),
        ] {
            let input = prepared(helper, Profile::Gfx942, None);
            let mut work = CanonicalKernelIrWorkBudgetV1::new(work_limit);
            let mut budget = AssertOriginBudgetV1::new(&mut work, storage_limit);
            budget.reserve_storage(input.floor).unwrap();
            let result = crate::ProductionCheckedOutputOwnerPolicy4V1::try_admit_v1(
                input.receipt,
                input.bound,
                input.checked,
                &mut budget,
            );
            assert_eq!(result.is_ok(), success, "{result:?}");
            assert_eq!(budget.storage(), input.floor);
            if let Ok(owner) = result {
                assert_eq!(owner.output().canonical().canonical_bytes(), expected);
                assert_eq!(budget.work(), exact_work);
            } else if work_limit < exact_work {
                assert!(work.failed_work().is_some());
            } else {
                assert!(budget.failed_storage().is_some());
            }
        }
    }
}
