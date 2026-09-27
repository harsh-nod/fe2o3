use crate::{
    CompilerExecutionSupervisorTrustV2 as Trust, PreparedCompilerExecutionSupervisorV2 as Prepared,
    RootManagedCompilerExecutionServiceV2 as Managed,
};
use fe2o3_compiler_closure_capability::{
    CompilerExecutionExternalAnchorDeploymentCapabilityV2 as AnchorCap,
    CompilerExecutionExternalAnchorProvisioningCapabilityV2 as ProvisioningCap,
    CompilerExecutionExternalAnchorSigningKeyCapabilityV2 as AnchorKey,
    CompilerExecutionPolicyCapabilityV2 as PolicyCap,
    CompilerExecutionSigningKeyCapabilityV2 as Key,
    CompilerExecutionSupervisorDeploymentCapabilityV2 as SupervisorCap,
};
use fe2o3_compiler_execution_protocol::{
    COMPILER_EXECUTION_EXTERNAL_ANCHOR_DEPLOYMENT_BYTES_V2 as ANCHOR_BYTES,
    COMPILER_EXECUTION_EXTERNAL_ANCHOR_PROVISIONING_BYTES_V2 as PROVISIONING_BYTES,
    COMPILER_EXECUTION_ISSUER_POLICY_BYTES_V2 as POLICY_BYTES,
    COMPILER_EXECUTION_SUPERVISOR_DEPLOYMENT_BYTES_V2 as SUPERVISOR_BYTES,
    CompilerExecutionExternalAnchorDeploymentV2 as AnchorDeployment,
    CompilerExecutionExternalAnchorProvisioningV2 as Provisioning,
    CompilerExecutionIssuerPolicyV2 as Policy,
    CompilerExecutionSupervisorDeploymentV2 as Supervisor,
    MAX_COMPILER_EXECUTION_EXTERNAL_ANCHOR_EXECUTABLE_BYTES_V2 as MAX_ANCHOR,
    MAX_COMPILER_EXECUTION_EXTERNAL_ANCHOR_PROVISIONING_HELPER_BYTES_V2 as MAX_HELPER,
    MAX_COMPILER_EXECUTION_SUPERVISOR_EXECUTABLE_BYTES_V2 as MAX_SUPERVISOR,
    MAX_COMPILER_EXECUTION_SUPERVISOR_LAUNCHER_BYTES_V2 as MAX_LAUNCHER,
};
use fe2o3_external_anchor_coordinator::PreparedExternalAnchorOccurrenceV2 as Anchor;
crate::native_inherited_adapter::inherited!(InheritedCompilerExecutionDeploymentV2, "2", "3");
