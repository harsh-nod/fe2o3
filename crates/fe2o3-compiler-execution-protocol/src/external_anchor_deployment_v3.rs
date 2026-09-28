//! Native V3 inert configuration, not provisioning origin or process authority.
use crate::{
    COMPILER_EXECUTION_SUPERVISOR_DEPLOYMENT_STORAGE_V3 as SUPERVISOR_STORAGE,
    COMPILER_EXECUTION_SUPERVISOR_DEPLOYMENT_WORK_V3 as SUPERVISOR_WORK,
    CompilerExecutionExternalAnchorDeploymentErrorV1 as Framing,
    CompilerExecutionExternalAnchorServiceIdentityV1 as Service,
    CompilerExecutionIssuerMeasurementV1 as Measurement, CompilerExecutionIssuerPolicyV3 as Policy,
    CompilerExecutionSupervisorDeploymentErrorV3 as SupervisorError,
    CompilerExecutionSupervisorDeploymentIdentityV3 as SupervisorIdentity,
    CompilerExecutionSupervisorDeploymentV3 as Supervisor,
    attestation_resources::{self as resources, CompilerExecutionAttestationStorageV3 as Storage},
    external_anchor_deployment_codec as codec,
};
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
};
use std::{error::Error, fmt, mem::size_of};

crate::external_anchor_deployment_adapter::external_anchor_deployment_adapter!(
    V3,
    "3",
    "2",
    COMPILER_EXECUTION_EXTERNAL_ANCHOR_DEPLOYMENT_BYTES_V3,
    COMPILER_EXECUTION_EXTERNAL_ANCHOR_DEPLOYMENT_WORK_V3,
    COMPILER_EXECUTION_EXTERNAL_ANCHOR_DEPLOYMENT_STORAGE_V3,
    MAX_COMPILER_EXECUTION_EXTERNAL_ANCHOR_EXECUTABLE_BYTES_V3,
    CompilerExecutionExternalAnchorDeploymentIdentityV3,
    CompilerExecutionExternalAnchorDeploymentV3,
    CompilerExecutionExternalAnchorDeploymentErrorV3
);
