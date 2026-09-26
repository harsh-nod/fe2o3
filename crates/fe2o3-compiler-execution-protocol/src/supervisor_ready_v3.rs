use crate::{
    CompilerExecutionSupervisorDeploymentIdentityV3 as DeploymentIdentity,
    CompilerExecutionSupervisorDeploymentV3 as Deployment,
    CompilerExecutionSupervisorReadyErrorV1 as Framing,
    attestation_resources::{self as resources, CompilerExecutionAttestationStorageV3 as Storage},
    supervisor_ready_native_codec as codec,
};
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
};
use std::{error::Error, fmt, mem::size_of};
crate::supervisor_ready_native_adapter::supervisor_ready!(
    V3,
    "3",
    "2",
    COMPILER_EXECUTION_SUPERVISOR_READY_BYTES_V3,
    COMPILER_EXECUTION_SUPERVISOR_READY_WORK_V3,
    COMPILER_EXECUTION_SUPERVISOR_READY_STORAGE_V3,
    CompilerExecutionSupervisorReadyV3,
    CompilerExecutionSupervisorReadyIdentityV3,
    CompilerExecutionSupervisorReadyErrorV3
);
