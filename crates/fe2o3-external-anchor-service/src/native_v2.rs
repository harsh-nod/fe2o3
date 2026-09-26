use fe2o3_compiler_closure_capability::CompilerExecutionExternalAnchorSigningKeyCapabilityV2 as Key;
use fe2o3_compiler_execution_protocol::CompilerExecutionExternalAnchorDeploymentV2 as Deployment;

crate::native_adapter::native_anchor!(DurableExternalAnchorV2, "2", "3");

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use DurableExternalAnchorV2 as Anchor;
    use fe2o3_compiler_execution_protocol::{
        CompilerExecutionIssuerPolicyV2 as Policy,
        CompilerExecutionSupervisorDeploymentV2 as Supervisor,
    };
    include!("native_tests.rs");
}
