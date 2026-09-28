//! Nominal V2 trust; no generic authority or legacy owner adaptation.
use fe2o3_compiler_closure_capability::{
    CompilerExecutionPolicyCapabilityV2 as PolicyCap,
    CompilerExecutionSigningKeyCapabilityV2 as Key,
    CompilerExecutionSupervisorDeploymentCapabilityV2 as DeploymentCap,
};
use fe2o3_compiler_execution_protocol::{
    COMPILER_EXECUTION_SUPERVISOR_DEPLOYMENT_STORAGE_V2 as DEPLOYMENT_STORAGE,
    COMPILER_EXECUTION_SUPERVISOR_DEPLOYMENT_WORK_V2 as DEPLOYMENT_WORK,
};
#[cfg(test)]
use fe2o3_compiler_execution_protocol::{
    CompilerExecutionIssuerPolicyV2 as Policy,
    CompilerExecutionSupervisorDeploymentV2 as Deployment,
};
crate::native_trust_adapter::trust!(CompilerExecutionSupervisorTrustV2, "2", "3");
