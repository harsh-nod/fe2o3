use fe2o3_compiler_execution_protocol::{
    COMPILER_EXECUTION_EXTERNAL_ANCHOR_PROVISIONING_BYTES_V2 as BYTES,
    COMPILER_EXECUTION_EXTERNAL_ANCHOR_PROVISIONING_STORAGE_V2 as PROVISIONING_STORAGE,
    COMPILER_EXECUTION_EXTERNAL_ANCHOR_PROVISIONING_WORK_V2 as PROVISIONING_WORK,
    CompilerExecutionExternalAnchorDeploymentV2 as Deployment,
    CompilerExecutionExternalAnchorProvisioningV2 as Provisioning,
};
crate::compiler_execution_external_anchor_provisioning_native::provisioning_capability!(
    CompilerExecutionExternalAnchorProvisioningCapabilityV2,
    "2",
    "3"
);

#[cfg(test)]
mod tests {
    use super::CompilerExecutionExternalAnchorProvisioningCapabilityV2 as Cap;
    use fe2o3_compiler_execution_protocol::{
        CompilerExecutionExternalAnchorProvisioningErrorV2 as DecodeError,
        CompilerExecutionIssuerPolicyV2 as Policy,
        CompilerExecutionSupervisorDeploymentV2 as Supervisor,
    };
    const VERSION: u16 = 2;
    fn decode_error(e: &crate::CompilerExecutionCapabilityErrorV2) -> &DecodeError {
        match e {
            crate::CompilerExecutionCapabilityErrorV2::ExternalAnchorProvisioning(e) => e,
            other => panic!("lost native error: {other}"),
        }
    }
    include!("compiler_execution_external_anchor_provisioning_native_tests.rs");
}
