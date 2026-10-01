use super::*;

#[test]
fn ordinary_publication_requires_the_fixed_typed_source_route_and_real_execution() {
    let pipeline = include_str!("production_pipeline.rs");
    let method = pipeline
        .split_once("pub(crate) fn publish_worker_handoff(")
        .unwrap()
        .1
        .split_once("/// Retains the original extraction milestone")
        .unwrap()
        .0;
    assert!(
        method.contains("self.publish_mixed_worker_handoff_v53(target_budget, compiler_execution)")
    );
    for old in ["lower_production_target(", "unwrap_or", "if ", "match "] {
        assert!(
            !method.contains(old),
            "ordinary publication can select {old}"
        );
    }
    let source = include_str!("production_pipeline_source_mixed_publish_v53.rs");
    let method = source
        .split_once("pub(crate) fn publish_mixed_worker_handoff_v53(")
        .unwrap()
        .1;
    let execute = method.find("with_executed_composition_v29(").unwrap();
    let capsule = method.find("with_strict_handoff_v53(").unwrap();
    let publish = method.find("publish_compiler_module_handoff_v3(").unwrap();
    let acquire = method.find("compiler_execution.acquire(").unwrap();
    assert!(execute < capsule && capsule < publish && publish < acquire);
    assert!(method.contains("FunctionalRefinementVerusRuntimeLeaseV1::open("));
    assert!(method.contains("ProductionPipelineError::MixedRuntime"));
    assert!(method.contains("check_strict_handoff_v53(handoff, budget)"));
    assert!(!method.contains("unwrap_or") && !method.contains("lower_production_target("));
}

#[test]
fn typed_capsule_association_is_not_an_integer_or_nominal_v5_substitution() {
    let source = include_str!("production_pipeline_source_mixed_publish_v53.rs");
    let check = source
        .split_once("fn check_strict_handoff_v53(")
        .unwrap()
        .1
        .split_once("impl ")
        .unwrap()
        .0;
    for required in [
        "replay_lineage_capsule_v50(",
        "self.executed",
        "signed_receipt(budget)",
        "descriptor_wire(",
        "verus_execution_evidence()",
        "formal_memory()",
        "abi()",
        "target_receipts_v53(",
        "SemanticToLlvmAssociationTranscriptV3::new(",
        "self.with_module_v53(",
    ] {
        assert!(check.contains(required), "missing {required}");
    }
    for forbidden in [
        "ExecutedMixedComposedRefinementV29",
        "ConditionalTheoremV2",
        "decode_device_descriptor_table_v5",
    ] {
        assert!(!source.contains(forbidden));
    }
    let nominal = include_str!("kernel_ir_codegen_mixed_descriptor_v53.rs");
    assert!(nominal.contains("VerifiedCanonicalKernelIrModuleV18"));
    assert!(nominal.contains(".fe2o3.kd.v53"));
}

#[test]
fn mixed_publication_preserves_nested_resource_failure_cause() {
    let original = Resource::Accounting;
    let error = wire_error(MixedDescriptorErrorV53::Contract(
        fe2o3_kernel_descriptor::mixed_conditional_v26::MixedContractErrorV26::Resource(original),
    ));
    assert!(matches!(error, Error::Resource(Resource::Accounting)));
    let error = wire_error(MixedDescriptorErrorV53::Nominal(
        fe2o3_kernel_descriptor::DescriptorWireErrorV3::Work(Resource::Arithmetic),
    ));
    assert!(matches!(error, Error::Resource(Resource::Arithmetic)));
    let error = module_error(mixed_v53::MixedModuleErrorV53::Descriptor(
        MixedDescriptorErrorV53::Contract(
            fe2o3_kernel_descriptor::mixed_conditional_v26::MixedContractErrorV26::Resource(
                Resource::Accounting,
            ),
        ),
    ));
    assert!(matches!(error, Error::Resource(Resource::Accounting)));
}

#[test]
fn mixed_target_publication_uses_shared_unchanged_v18_selection_not_legacy_transformation() {
    let source = include_str!("production_pipeline_source_mixed_publish_v53.rs");
    let helper = source
        .split_once("fn target_receipts_v53(")
        .unwrap()
        .1
        .split_once("fn with_module_v53")
        .unwrap()
        .0;
    assert!(helper.contains("with_mixed_target_selection_v53("));
    assert!(helper.contains("MixedTargetSelectionSubjectV53"));
    assert!(helper.contains("profile.rustc_features() != features"));
    assert!(helper.contains("profile.rustc_target() != layout.llvm_target()"));
    assert!(!helper.contains("MultiRootTargetBindingTranscriptV2"));
    assert!(!helper.contains("MultiRootTargetBindingTranscriptV3"));
    assert!(!helper.contains("target_bound_kir:"));
}
