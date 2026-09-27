use crate::native_trust_v2::CompilerExecutionSupervisorTrustV2 as Trust;
use fe2o3_external_anchor_coordinator::RootManagedExternalAnchorV2 as Anchor;

crate::native_adapter::preparation!(PreparedCompilerExecutionSupervisorV2, "2", "3");

mod consuming {
    #[cfg(test)]
    use fe2o3_compiler_execution_protocol::CompilerExecutionIssuerPolicyV2 as Policy;
    #[cfg(test)]
    type Prepared = PreparedCompilerExecutionSupervisorV2;
    use super::PreparedCompilerExecutionSupervisorV2;
    use fe2o3_compiler_closure_capability::{
        CompilerExecutionPolicyCapabilityV2 as PolicyCap,
        CompilerExecutionSigningKeyCapabilityV2 as Key,
        CompilerExecutionSupervisorDeploymentCapabilityV2 as DeploymentCap,
    };
    use fe2o3_compiler_execution_protocol::{
        COMPILER_EXECUTION_SUPERVISOR_READY_BYTES_V2 as READY_BYTES,
        COMPILER_EXECUTION_SUPERVISOR_READY_STORAGE_V2 as READY_SCRATCH,
        COMPILER_EXECUTION_SUPERVISOR_READY_WORK_V2 as READY_WORK,
        CompilerExecutionSupervisorDeploymentV2 as Deployment,
        CompilerExecutionSupervisorReadyV2 as Ready,
    };
    use fe2o3_external_anchor_coordinator::ExternalAnchorSupervisorTransferV2 as AnchorTransfer;
    const READY_OWNER_STORAGE: usize = std::mem::size_of::<(
        Ready,
        fe2o3_compiler_execution_protocol::CompilerExecutionAttestationStorageV2,
    )>();
    crate::native_launch_adapter::launch!(
        PreparedCompilerExecutionSupervisorV2,
        RootManagedCompilerExecutionServiceV2,
        "2",
        "3"
    );
    #[cfg(test)]
    mod tests {
        use super::*;
        include!("native_launch_ready_cases_tests.rs");
    }
}
pub use consuming::RootManagedCompilerExecutionServiceV2;
