use crate::{
    native_capability::{NativeCapability, Record, Result, Storage},
    sealed_image::CapabilityRole,
};
use fe2o3_compiler_execution_protocol::{
    COMPILER_EXECUTION_SUPERVISOR_DEPLOYMENT_BYTES_V2 as BYTES,
    COMPILER_EXECUTION_SUPERVISOR_DEPLOYMENT_STORAGE_V2 as DEPLOYMENT_STORAGE,
    COMPILER_EXECUTION_SUPERVISOR_DEPLOYMENT_WORK_V2 as DEPLOYMENT_WORK,
    CompilerExecutionAttestationStorageV2 as ProtocolStorage,
    CompilerExecutionIssuerPolicyV2 as Policy,
    CompilerExecutionSupervisorDeploymentV2 as Deployment,
};
use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as Budget;
use std::{fs::File, os::fd::RawFd};

crate::compiler_execution_supervisor_deployment_native::supervisor_deployment_capability!(
    CompilerExecutionSupervisorDeploymentCapabilityV2,
    "2",
    "3"
);
