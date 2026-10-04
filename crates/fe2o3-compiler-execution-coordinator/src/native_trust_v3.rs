//! Nominal V3 trust; no generic authority or legacy owner adaptation.
use fe2o3_compiler_closure_capability::{
    CompilerExecutionPolicyCapabilityV3 as PolicyCap,
    CompilerExecutionSigningKeyCapabilityV3 as Key,
    CompilerExecutionSupervisorDeploymentCapabilityV3 as DeploymentCap,
};
use fe2o3_compiler_execution_protocol::{
    COMPILER_EXECUTION_SUPERVISOR_DEPLOYMENT_STORAGE_V3 as DEPLOYMENT_STORAGE,
    COMPILER_EXECUTION_SUPERVISOR_DEPLOYMENT_WORK_V3 as DEPLOYMENT_WORK,
};
#[cfg(test)]
use fe2o3_compiler_execution_protocol::{
    CompilerExecutionIssuerPolicyV3 as Policy,
    CompilerExecutionSupervisorDeploymentV3 as Deployment,
};
crate::native_trust_adapter::trust!(CompilerExecutionSupervisorTrustV3, "3", "2");
