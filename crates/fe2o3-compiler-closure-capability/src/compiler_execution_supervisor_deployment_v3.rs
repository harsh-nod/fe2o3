use crate::{
    native_capability::{NativeCapability, Record, Result, Storage},
    sealed_image::CapabilityRole,
};
use fe2o3_compiler_execution_protocol::{
    COMPILER_EXECUTION_SUPERVISOR_DEPLOYMENT_BYTES_V3 as BYTES,
    COMPILER_EXECUTION_SUPERVISOR_DEPLOYMENT_STORAGE_V3 as DEPLOYMENT_STORAGE,
    COMPILER_EXECUTION_SUPERVISOR_DEPLOYMENT_WORK_V3 as DEPLOYMENT_WORK,
    CompilerExecutionAttestationStorageV3 as ProtocolStorage,
    CompilerExecutionIssuerPolicyV3 as Policy,
    CompilerExecutionSupervisorDeploymentV3 as Deployment,
};
use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as Budget;
use std::{fs::File, os::fd::RawFd};

crate::compiler_execution_supervisor_deployment_native::supervisor_deployment_capability!(
    CompilerExecutionSupervisorDeploymentCapabilityV3,
    "3",
    "2"
);
