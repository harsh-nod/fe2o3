use fe2o3_compiler_closure_capability::{
    CompilerExecutionExternalAnchorDeploymentCapabilityV2 as DeploymentCap,
    CompilerExecutionExternalAnchorProvisioningCapabilityV2 as ProvisioningCap,
    CompilerExecutionExternalAnchorSigningKeyCapabilityV2 as Key,
    CompilerExecutionPolicyCapabilityV2 as PolicyCap,
    CompilerExecutionSupervisorDeploymentCapabilityV2 as SupervisorCap,
};
use fe2o3_compiler_execution_protocol::{
    COMPILER_EXECUTION_EXTERNAL_ANCHOR_DEPLOYMENT_STORAGE_V2 as DEPLOYMENT_STORAGE,
    COMPILER_EXECUTION_EXTERNAL_ANCHOR_DEPLOYMENT_WORK_V2 as DEPLOYMENT_WORK,
    COMPILER_EXECUTION_EXTERNAL_ANCHOR_PROVISIONING_STORAGE_V2 as PROVISIONING_STORAGE,
    COMPILER_EXECUTION_EXTERNAL_ANCHOR_PROVISIONING_WORK_V2 as PROVISIONING_WORK,
    CompilerExecutionExternalAnchorDeploymentIdentityV2 as TransferDeploymentIdentity,
    CompilerExecutionIssuerPolicyIdentityV2 as TransferPolicyIdentity,
    CompilerExecutionSupervisorDeploymentIdentityV2 as TransferSupervisorIdentity,
    MAX_COMPILER_EXECUTION_EXTERNAL_ANCHOR_EXECUTABLE_BYTES_V2 as MAX_DAEMON,
    MAX_COMPILER_EXECUTION_EXTERNAL_ANCHOR_PROVISIONING_HELPER_BYTES_V2 as MAX_HELPER,
};
#[cfg(test)]
use fe2o3_compiler_execution_protocol::{
    CompilerExecutionExternalAnchorDeploymentV2 as Deployment,
    CompilerExecutionExternalAnchorProvisioningV2 as Provisioning,
    CompilerExecutionIssuerPolicyV2 as Policy,
    CompilerExecutionSupervisorDeploymentV2 as Supervisor,
};
crate::native_adapter::preparation!(PreparedExternalAnchorOccurrenceV2, "2", "3");
crate::native_launch_adapter::launch!(
    PreparedExternalAnchorOccurrenceV2,
    RootManagedExternalAnchorV2,
    "2",
    "3"
);
crate::native_transfer_adapter::transfer!(
    RootManagedExternalAnchorV2,
    ExternalAnchorSupervisorTransferV2,
    "2",
    "3"
);
#[cfg(test)]
type TransferFixture = ExternalAnchorSupervisorTransferV2;
