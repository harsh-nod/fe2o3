use fe2o3_compiler_execution_protocol::{
    COMPILER_EXECUTION_EXTERNAL_ANCHOR_PROVISIONING_BYTES_V3 as BYTES,
    COMPILER_EXECUTION_EXTERNAL_ANCHOR_PROVISIONING_STORAGE_V3 as PROVISIONING_STORAGE,
    COMPILER_EXECUTION_EXTERNAL_ANCHOR_PROVISIONING_WORK_V3 as PROVISIONING_WORK,
    CompilerExecutionExternalAnchorDeploymentV3 as Deployment,
    CompilerExecutionExternalAnchorProvisioningV3 as Provisioning,
};
crate::compiler_execution_external_anchor_provisioning_native::provisioning_capability!(
    CompilerExecutionExternalAnchorProvisioningCapabilityV3,
    "3",
    "2"
);

#[cfg(test)]
mod tests {
    use super::CompilerExecutionExternalAnchorProvisioningCapabilityV3 as Cap;
    use fe2o3_compiler_execution_protocol::{
        CompilerExecutionExternalAnchorProvisioningErrorV3 as DecodeError,
        CompilerExecutionIssuerPolicyV3 as Policy,
        CompilerExecutionSupervisorDeploymentV3 as Supervisor,
    };
    const VERSION: u16 = 3;
    fn decode_error(e: &crate::CompilerExecutionCapabilityErrorV2) -> &DecodeError {
        match e {
            crate::CompilerExecutionCapabilityErrorV2::ExternalAnchorProvisioningV3(e) => e,
            other => panic!("lost native error: {other}"),
        }
    }
    include!("compiler_execution_external_anchor_provisioning_native_tests.rs");
}
