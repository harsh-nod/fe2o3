//! Native V2 inert configuration, not provisioning origin or process authority.
use crate::{
    COMPILER_EXECUTION_SUPERVISOR_DEPLOYMENT_STORAGE_V2 as SUPERVISOR_STORAGE,
    COMPILER_EXECUTION_SUPERVISOR_DEPLOYMENT_WORK_V2 as SUPERVISOR_WORK,
    CompilerExecutionExternalAnchorDeploymentErrorV1 as Framing,
    CompilerExecutionExternalAnchorServiceIdentityV1 as Service,
    CompilerExecutionIssuerMeasurementV1 as Measurement, CompilerExecutionIssuerPolicyV2 as Policy,
    CompilerExecutionSupervisorDeploymentErrorV2 as SupervisorError,
    CompilerExecutionSupervisorDeploymentIdentityV2 as SupervisorIdentity,
    CompilerExecutionSupervisorDeploymentV2 as Supervisor,
    attestation_resources::{self as resources, CompilerExecutionAttestationStorageV2 as Storage},
    external_anchor_deployment_codec as codec,
};
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
};
use std::{error::Error, fmt, mem::size_of};

crate::external_anchor_deployment_adapter::external_anchor_deployment_adapter!(
    V2,
    "2",
    "3",
    COMPILER_EXECUTION_EXTERNAL_ANCHOR_DEPLOYMENT_BYTES_V2,
    COMPILER_EXECUTION_EXTERNAL_ANCHOR_DEPLOYMENT_WORK_V2,
    COMPILER_EXECUTION_EXTERNAL_ANCHOR_DEPLOYMENT_STORAGE_V2,
    MAX_COMPILER_EXECUTION_EXTERNAL_ANCHOR_EXECUTABLE_BYTES_V2,
    CompilerExecutionExternalAnchorDeploymentIdentityV2,
    CompilerExecutionExternalAnchorDeploymentV2,
    CompilerExecutionExternalAnchorDeploymentErrorV2
);
