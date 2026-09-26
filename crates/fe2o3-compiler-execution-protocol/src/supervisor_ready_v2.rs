use crate::{
    CompilerExecutionSupervisorDeploymentIdentityV2 as DeploymentIdentity,
    CompilerExecutionSupervisorDeploymentV2 as Deployment,
    CompilerExecutionSupervisorReadyErrorV1 as Framing,
    attestation_resources::{self as resources, CompilerExecutionAttestationStorageV2 as Storage},
    supervisor_ready_native_codec as codec,
};
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
};
use std::{error::Error, fmt, mem::size_of};
crate::supervisor_ready_native_adapter::supervisor_ready!(
    V2,
    "2",
    "3",
    COMPILER_EXECUTION_SUPERVISOR_READY_BYTES_V2,
    COMPILER_EXECUTION_SUPERVISOR_READY_WORK_V2,
    COMPILER_EXECUTION_SUPERVISOR_READY_STORAGE_V2,
    CompilerExecutionSupervisorReadyV2,
    CompilerExecutionSupervisorReadyIdentityV2,
    CompilerExecutionSupervisorReadyErrorV2
);
