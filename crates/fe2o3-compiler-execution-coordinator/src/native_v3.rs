use crate::native_trust_v3::CompilerExecutionSupervisorTrustV3 as Trust;
use fe2o3_external_anchor_coordinator::RootManagedExternalAnchorV3 as Anchor;

crate::native_adapter::preparation!(PreparedCompilerExecutionSupervisorV3, "3", "2");

mod consuming {
    #[cfg(test)]
    use fe2o3_compiler_execution_protocol::CompilerExecutionIssuerPolicyV3 as Policy;
    #[cfg(test)]
    type Prepared = PreparedCompilerExecutionSupervisorV3;
    use super::PreparedCompilerExecutionSupervisorV3;
    use fe2o3_compiler_closure_capability::{
        CompilerExecutionPolicyCapabilityV3 as PolicyCap,
        CompilerExecutionSigningKeyCapabilityV3 as Key,
        CompilerExecutionSupervisorDeploymentCapabilityV3 as DeploymentCap,
    };
    use fe2o3_compiler_execution_protocol::{
        COMPILER_EXECUTION_SUPERVISOR_READY_BYTES_V3 as READY_BYTES,
        COMPILER_EXECUTION_SUPERVISOR_READY_STORAGE_V3 as READY_SCRATCH,
        COMPILER_EXECUTION_SUPERVISOR_READY_WORK_V3 as READY_WORK,
        CompilerExecutionSupervisorDeploymentV3 as Deployment,
        CompilerExecutionSupervisorReadyV3 as Ready,
    };
    use fe2o3_external_anchor_coordinator::ExternalAnchorSupervisorTransferV3 as AnchorTransfer;
    const READY_OWNER_STORAGE: usize = std::mem::size_of::<(
        Ready,
        fe2o3_compiler_execution_protocol::CompilerExecutionAttestationStorageV3,
    )>();
    crate::native_launch_adapter::launch!(
        PreparedCompilerExecutionSupervisorV3,
        RootManagedCompilerExecutionServiceV3,
        "3",
        "2"
    );
    #[cfg(test)]
    mod tests {
        use super::*;
        include!("native_launch_ready_cases_tests.rs");
    }
}
pub use consuming::RootManagedCompilerExecutionServiceV3;
