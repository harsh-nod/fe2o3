//! Native V2 trusted configuration; no provisioning authentication or launch authority.
use crate::{
    CompilerExecutionExternalAnchorServiceIdentityV1 as Service,
    CompilerExecutionIssuerMeasurementV1 as Measurement,
    CompilerExecutionIssuerPolicyIdentityV2 as PolicyIdentity,
    CompilerExecutionIssuerPolicyV2 as Policy,
    CompilerExecutionSupervisorDeploymentErrorV1 as Framing,
    attestation_resources::{self as resources, CompilerExecutionAttestationStorageV2 as Storage},
    supervisor_deployment_codec as codec,
};
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
};
use std::{error::Error, fmt, mem::size_of};

crate::supervisor_deployment_adapter::supervisor_deployment_adapter!(
    V2,
    "2",
    "3",
    COMPILER_EXECUTION_SUPERVISOR_DEPLOYMENT_BYTES_V2,
    COMPILER_EXECUTION_SUPERVISOR_DEPLOYMENT_WORK_V2,
    COMPILER_EXECUTION_SUPERVISOR_DEPLOYMENT_STORAGE_V2,
    MAX_COMPILER_EXECUTION_SUPERVISOR_EXECUTABLE_BYTES_V2,
    MAX_COMPILER_EXECUTION_SUPERVISOR_LAUNCHER_BYTES_V2,
    CompilerExecutionSupervisorDeploymentIdentityV2,
    CompilerExecutionSupervisorDeploymentV2,
    CompilerExecutionSupervisorDeploymentErrorV2
);
