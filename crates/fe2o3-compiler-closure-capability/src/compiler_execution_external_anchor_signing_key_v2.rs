use fe2o3_compiler_execution_protocol::{
    CompilerExecutionExternalAnchorDeploymentIdentityV2 as DeploymentIdentity,
    CompilerExecutionExternalAnchorDeploymentV2 as Deployment,
};

crate::compiler_execution_external_anchor_signing_key_native::anchor_signing_key!(
    CompilerExecutionExternalAnchorSigningKeyCapabilityV2,
    "2",
    "3"
);

#[cfg(test)]
mod tests {
    use super::CompilerExecutionExternalAnchorSigningKeyCapabilityV2 as Cap;
    use fe2o3_compiler_execution_protocol::{
        CompilerExecutionIssuerPolicyV2 as Policy,
        CompilerExecutionSupervisorDeploymentV2 as Supervisor,
    };
    const VERSION: u16 = 2;
    include!("compiler_execution_external_anchor_signing_key_native_tests.rs");
}
