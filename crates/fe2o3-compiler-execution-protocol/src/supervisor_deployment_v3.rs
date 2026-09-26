//! Native V3 trusted configuration; no provisioning authentication or launch authority.
use crate::{
    CompilerExecutionExternalAnchorServiceIdentityV1 as Service,
    CompilerExecutionIssuerMeasurementV1 as Measurement,
    CompilerExecutionIssuerPolicyIdentityV3 as PolicyIdentity,
    CompilerExecutionIssuerPolicyV3 as Policy,
    CompilerExecutionSupervisorDeploymentErrorV1 as Framing,
    attestation_resources::{self as resources, CompilerExecutionAttestationStorageV3 as Storage},
    supervisor_deployment_codec as codec,
};
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
};
use std::{error::Error, fmt, mem::size_of};

crate::supervisor_deployment_adapter::supervisor_deployment_adapter!(
    V3,
    "3",
    "2",
    COMPILER_EXECUTION_SUPERVISOR_DEPLOYMENT_BYTES_V3,
    COMPILER_EXECUTION_SUPERVISOR_DEPLOYMENT_WORK_V3,
    COMPILER_EXECUTION_SUPERVISOR_DEPLOYMENT_STORAGE_V3,
    MAX_COMPILER_EXECUTION_SUPERVISOR_EXECUTABLE_BYTES_V3,
    MAX_COMPILER_EXECUTION_SUPERVISOR_LAUNCHER_BYTES_V3,
    CompilerExecutionSupervisorDeploymentIdentityV3,
    CompilerExecutionSupervisorDeploymentV3,
    CompilerExecutionSupervisorDeploymentErrorV3
);
