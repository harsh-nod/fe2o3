use fe2o3_compiler_execution_protocol::{
    CompilerExecutionExternalAnchorDeploymentIdentityV3 as DeploymentIdentity,
    CompilerExecutionExternalAnchorDeploymentV3 as Deployment,
};

crate::compiler_execution_external_anchor_signing_key_native::anchor_signing_key!(
    CompilerExecutionExternalAnchorSigningKeyCapabilityV3,
    "3",
    "2"
);

#[cfg(test)]
mod tests {
    use super::CompilerExecutionExternalAnchorSigningKeyCapabilityV3 as Cap;
    use fe2o3_compiler_execution_protocol::{
        CompilerExecutionIssuerPolicyV3 as Policy,
        CompilerExecutionSupervisorDeploymentV3 as Supervisor,
    };
    const VERSION: u16 = 3;
    include!("compiler_execution_external_anchor_signing_key_native_tests.rs");
}
