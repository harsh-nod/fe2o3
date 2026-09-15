use fe2o3_amd_target::ProductionAmdTargetProfileV1;
use fe2o3_amdgcn_model::{
    LoweringDiagnosticCode, ProductionTargetLaunchEvidenceV13, ProductionV13AmdLoweringErrorV1,
    lower_verified_canonical_kir_v13_to_amd_llvm_ir_v1,
};
use fe2o3_kernel_ir::{VerifiedCanonicalKernelIrV13, WorkgroupSize};

#[path = "../../fe2o3-kernel-ir/tests/support/numerical_policy_v13.rs"]
mod fixture;

#[test]
fn unused_numerical_policy_adds_no_physical_operations_or_proof_authority() {
    let mut module = fixture::module();
    module.kernels[0].workgroup_size = Some(WorkgroupSize::new(1, 1, 1));
    let canonical = VerifiedCanonicalKernelIrV13::from_module(module.clone()).unwrap();
    module.functions[0].body.as_mut().unwrap().blocks[0]
        .operations
        .pop();
    let baseline = VerifiedCanonicalKernelIrV13::from_module(module).unwrap();
    let baseline_evidence =
        ProductionTargetLaunchEvidenceV13::for_static_launches(&baseline, 7).unwrap();
    let evidence = ProductionTargetLaunchEvidenceV13::for_static_launches(&canonical, 7).unwrap();
    for profile in [
        ProductionAmdTargetProfileV1::Gfx942,
        ProductionAmdTargetProfileV1::Gfx950,
    ] {
        let lowered =
            lower_verified_canonical_kir_v13_to_amd_llvm_ir_v1(&canonical, 7, &evidence, profile)
                .unwrap();
        let baseline_lowered = lower_verified_canonical_kir_v13_to_amd_llvm_ir_v1(
            &baseline,
            7,
            &baseline_evidence,
            profile,
        )
        .unwrap();
        assert_eq!(lowered.llvm_ir(), baseline_lowered.llvm_ir());
        assert_eq!(lowered.decisions(), baseline_lowered.decisions());
        assert_ne!(
            lowered.capability_closure_identity(),
            baseline_lowered.capability_closure_identity()
        );
        assert!(!lowered.grants_load_authority());
        assert!(!lowered.grants_launch_authority());
        assert!(!lowered.has_complete_operational_translation_derivation());
    }
}

#[test]
fn numerical_policy_use_is_not_erased_without_an_exact_consumer_adapter() {
    let mut module = fixture::used_policy_module();
    module.kernels[0].workgroup_size = Some(WorkgroupSize::new(1, 1, 1));
    let canonical = VerifiedCanonicalKernelIrV13::from_module(module).unwrap();
    let evidence = ProductionTargetLaunchEvidenceV13::for_static_launches(&canonical, 7).unwrap();
    let errors = lower_verified_canonical_kir_v13_to_amd_llvm_ir_v1(
        &canonical,
        7,
        &evidence,
        ProductionAmdTargetProfileV1::Gfx942,
    )
    .unwrap_err();
    let ProductionV13AmdLoweringErrorV1::Lowering(errors) = errors else {
        panic!("expected physical lowering to reject the capability boundary");
    };
    let [diagnostic] = errors.diagnostics() else {
        panic!("expected the capability boundary guard");
    };
    assert_eq!(diagnostic.code, LoweringDiagnosticCode::IncompleteOperation);
    assert_eq!(
        diagnostic.message,
        "V13 execution capabilities crossing function or block boundaries lack a physical ABI/phi lowering"
    );
}

#[test]
fn numerical_policy_lowering_rejects_substituted_mode_type_and_provenance() {
    for (name, bad) in fixture::rejected_issuance_mutations() {
        assert!(
            VerifiedCanonicalKernelIrV13::from_module(bad).is_err(),
            "{name}"
        );
    }
}

#[test]
fn unused_numerical_policy_preserves_actual_fp_arithmetic_and_strict_llvm_flags() {
    use fe2o3_kernel_ir::NumericalModeV1;

    let baseline_module = fixture::floating_point_module(false, NumericalModeV1::StrictIeee);
    let policy_module = fixture::floating_point_module(true, NumericalModeV1::StrictIeee);
    assert_eq!(
        policy_module.required_capabilities,
        baseline_module.required_capabilities
    );
    let baseline = VerifiedCanonicalKernelIrV13::from_module(baseline_module).unwrap();
    let policy = VerifiedCanonicalKernelIrV13::from_module(policy_module).unwrap();
    let baseline_evidence =
        ProductionTargetLaunchEvidenceV13::for_static_launches(&baseline, 7).unwrap();
    let policy_evidence =
        ProductionTargetLaunchEvidenceV13::for_static_launches(&policy, 7).unwrap();
    for profile in [
        ProductionAmdTargetProfileV1::Gfx942,
        ProductionAmdTargetProfileV1::Gfx950,
    ] {
        let lower = |canonical, evidence| {
            lower_verified_canonical_kir_v13_to_amd_llvm_ir_v1(canonical, 7, evidence, profile)
                .unwrap()
        };
        let baseline = lower(&baseline, &baseline_evidence);
        let policy = lower(&policy, &policy_evidence);
        assert_eq!(policy.llvm_ir(), baseline.llvm_ir());
        assert_eq!(policy.decisions(), baseline.decisions());
        let llvm = policy.llvm_ir();
        assert_eq!(llvm.matches(" = fmul float ").count(), 1);
        assert_eq!(llvm.matches(" = fadd float ").count(), 1);
        for flag in [
            "\"denormal-fp-math-f32\"=\"ieee,ieee\"",
            "\"unsafe-fp-math\"=\"false\"",
            "\"no-infs-fp-math\"=\"false\"",
            "\"no-nans-fp-math\"=\"false\"",
            "\"no-signed-zeros-fp-math\"=\"false\"",
            "\"approx-func-fp-math\"=\"false\"",
            "\"fp-contract\"=\"off\"",
        ] {
            assert!(llvm.contains(flag), "{flag}");
        }
        assert!(!llvm.contains("llvm.fma"));
        assert!(!llvm.contains("llvm.fmuladd"));
        assert!(!policy.has_complete_operational_translation_derivation());
        assert!(!baseline.has_complete_operational_translation_derivation());
        assert!(!policy.grants_launch_authority());
        assert!(!policy.grants_load_authority());
    }
}
