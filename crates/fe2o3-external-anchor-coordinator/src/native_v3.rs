use fe2o3_compiler_closure_capability::{
    CompilerExecutionExternalAnchorDeploymentCapabilityV3 as DeploymentCap,
    CompilerExecutionExternalAnchorProvisioningCapabilityV3 as ProvisioningCap,
    CompilerExecutionExternalAnchorSigningKeyCapabilityV3 as Key,
    CompilerExecutionPolicyCapabilityV3 as PolicyCap,
    CompilerExecutionSupervisorDeploymentCapabilityV3 as SupervisorCap,
};
use fe2o3_compiler_execution_protocol::{
    COMPILER_EXECUTION_EXTERNAL_ANCHOR_DEPLOYMENT_STORAGE_V3 as DEPLOYMENT_STORAGE,
    COMPILER_EXECUTION_EXTERNAL_ANCHOR_DEPLOYMENT_WORK_V3 as DEPLOYMENT_WORK,
    COMPILER_EXECUTION_EXTERNAL_ANCHOR_PROVISIONING_STORAGE_V3 as PROVISIONING_STORAGE,
    COMPILER_EXECUTION_EXTERNAL_ANCHOR_PROVISIONING_WORK_V3 as PROVISIONING_WORK,
    CompilerExecutionExternalAnchorDeploymentIdentityV3 as TransferDeploymentIdentity,
    CompilerExecutionIssuerPolicyIdentityV3 as TransferPolicyIdentity,
    CompilerExecutionSupervisorDeploymentIdentityV3 as TransferSupervisorIdentity,
    MAX_COMPILER_EXECUTION_EXTERNAL_ANCHOR_EXECUTABLE_BYTES_V3 as MAX_DAEMON,
    MAX_COMPILER_EXECUTION_EXTERNAL_ANCHOR_PROVISIONING_HELPER_BYTES_V3 as MAX_HELPER,
};
#[cfg(test)]
use fe2o3_compiler_execution_protocol::{
    CompilerExecutionExternalAnchorDeploymentV3 as Deployment,
    CompilerExecutionExternalAnchorProvisioningV3 as Provisioning,
    CompilerExecutionIssuerPolicyV3 as Policy,
    CompilerExecutionSupervisorDeploymentV3 as Supervisor,
};
crate::native_adapter::preparation!(PreparedExternalAnchorOccurrenceV3, "3", "2");
crate::native_launch_adapter::launch!(
    PreparedExternalAnchorOccurrenceV3,
    RootManagedExternalAnchorV3,
    "3",
    "2"
);
crate::native_transfer_adapter::transfer!(
    RootManagedExternalAnchorV3,
    ExternalAnchorSupervisorTransferV3,
    "3",
    "2"
);
#[cfg(test)]
type TransferFixture = ExternalAnchorSupervisorTransferV3;
