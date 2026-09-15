use fe2o3_kernel_analysis::{
    ExecutionCapabilitySemanticReasonV1, KernelCheckStatusV1, ProductionCapabilityAnalysisKindV1,
    analyze_execution_capability_final_graph_v1,
};
use fe2o3_kernel_ir::VerifiedCanonicalKernelIrV13;

#[path = "../../fe2o3-kernel-ir/tests/support/numerical_policy_v13.rs"]
mod fixture;

#[cfg(feature = "pliron-analysis")]
#[path = "support/numerical_policy_refinement.rs"]
mod numerical_refinement;

#[test]
fn numerical_policy_issuance_never_certifies_numerical_refinement() {
    let module = fixture::module();
    let canonical = VerifiedCanonicalKernelIrV13::from_module(module.clone()).unwrap();
    let report = analyze_execution_capability_final_graph_v1(&canonical, &module, 7).unwrap();
    assert_eq!(report.status(), KernelCheckStatusV1::Clean);
    assert_eq!(
        report.stage_status(ProductionCapabilityAnalysisKindV1::SemanticRefinement),
        KernelCheckStatusV1::Clean
    );
    assert!(report.findings().is_empty());
    assert_eq!(report.canonical_identity(), canonical.identity());
    assert!(!report.grants_proof_machine_artifact_or_launch_authority());
    assert_eq!(
        fixture::contract().obligations.bits(),
        fe2o3_kernel_ir::ExecutionSafetyObligationsV1::TARGET_SUPPORT
            | fe2o3_kernel_ir::ExecutionSafetyObligationsV1::NUMERICAL_POLICY
    );
}

#[test]
fn numerical_policy_use_retains_its_unresolved_consumer_obligation() {
    let module = fixture::used_policy_module();
    let canonical = VerifiedCanonicalKernelIrV13::from_module(module.clone()).unwrap();
    let report = analyze_execution_capability_final_graph_v1(&canonical, &module, 7).unwrap();
    assert_eq!(report.status(), KernelCheckStatusV1::Incomplete);
    let finding = report
        .findings()
        .iter()
        .find(|finding| {
            matches!(
                finding.reason(),
                ExecutionCapabilitySemanticReasonV1::NumericalPolicyRefinementUnavailable
            )
        })
        .unwrap();
    assert_eq!(
        finding.location().source(),
        Some(fixture::contract().source)
    );
    assert_eq!(finding.location().operation(), 1);
    assert!(!report.grants_proof_machine_artifact_or_launch_authority());
}

#[test]
fn numerical_policy_analysis_rejects_substituted_mode_type_and_provenance() {
    let source = VerifiedCanonicalKernelIrV13::from_module(fixture::module()).unwrap();
    for (name, bad) in fixture::rejected_issuance_mutations() {
        assert!(
            VerifiedCanonicalKernelIrV13::from_module(bad.clone()).is_err(),
            "{name}"
        );
        assert!(
            analyze_execution_capability_final_graph_v1(&source, &bad, 7).is_err(),
            "{name}"
        );
    }
}
