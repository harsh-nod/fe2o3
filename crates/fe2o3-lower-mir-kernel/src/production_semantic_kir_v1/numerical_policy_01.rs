fn numerical_policy_transport_matches_v1(
    contract: SemanticExecutionCapabilityContractV1,
    capability: &ExecutionCapabilityTypeV1,
) -> bool {
    let SemanticExecutionCapabilityOperationV1::NumericalPolicyIssue { policy, .. } =
        contract.operation()
    else {
        return false;
    };
    let source = contract.provenance();
    capability.is_complete()
        && capability.provenance.kernel_binding == *source.kernel_binding().as_bytes()
        && capability.provenance.frontend_unit == *source.frontend_unit().as_bytes()
        && capability.provenance.kernel_marker == *source.kernel_marker().as_bytes()
        && capability.provenance.target_brand == *source.target_brand().as_bytes()
        && capability.provenance.launch_brand == *source.launch_brand().as_bytes()
        && capability.provenance.issuance == *source.issuance().as_bytes()
        && capability.role
            == ExecutionCapabilityRoleV1::NumericalPolicy {
                policy: ExecutionTypeIdentityV1::new(*policy.as_bytes()),
                mode: fe2o3_kernel_ir::NumericalModeV1::StrictIeee,
            }
}

fn numerical_policy_transport_type_v1(
    types: &[SemanticTypeDeclV1],
    semantic_type: SemanticTypeIdV1,
    contract: SemanticExecutionCapabilityContractV1,
    context: Option<&KernelContextTypeV1>,
) -> Result<Type, ProductionSemanticKirErrorV1> {
    let context = context.ok_or_else(|| {
        unsupported(
            0,
            None,
            None,
            "numerical-policy SSA transport lacks authenticated kernel context",
        )
    })?;
    let source = contract.provenance();
    if semantic_type != contract.signature().output()
        || context.kernel_marker() != source.kernel_marker().as_bytes()
        || context.target() != source.target_brand().as_bytes()
        || context.launch() != source.launch_brand().as_bytes()
    {
        return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
    }
    let operation = lower_execution_operation_v1(types, contract.operation(), None)?;
    let provenance = ExecutionCapabilityProvenanceV1 {
        root: context.root().clone(),
        kernel_binding: *source.kernel_binding().as_bytes(),
        frontend_unit: *source.frontend_unit().as_bytes(),
        kernel_marker: *source.kernel_marker().as_bytes(),
        target_brand: *source.target_brand().as_bytes(),
        launch_brand: *source.launch_brand().as_bytes(),
        issuance: *source.issuance().as_bytes(),
    };
    let results =
        execution_result_types_v1(types, &operation, &provenance, None, None, None, &[], 0)?;
    let [Type::ExecutionCapability(capability)] = results.as_slice() else {
        return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
    };
    if !numerical_policy_transport_matches_v1(contract, capability) {
        return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
    }
    Ok(results[0].clone())
}
