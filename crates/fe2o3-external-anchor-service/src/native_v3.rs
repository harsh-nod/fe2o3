use fe2o3_compiler_closure_capability::CompilerExecutionExternalAnchorSigningKeyCapabilityV3 as Key;
use fe2o3_compiler_execution_protocol::CompilerExecutionExternalAnchorDeploymentV3 as Deployment;

crate::native_adapter::native_anchor!(DurableExternalAnchorV3, "3", "2");

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use DurableExternalAnchorV3 as Anchor;
    use fe2o3_compiler_execution_protocol::{
        CompilerExecutionIssuerPolicyV3 as Policy,
        CompilerExecutionSupervisorDeploymentV3 as Supervisor,
    };
    include!("native_tests.rs");
}
