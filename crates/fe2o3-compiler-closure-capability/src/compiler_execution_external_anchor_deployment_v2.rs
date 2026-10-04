use crate::{
    native_capability::{NativeCapability, Record, Result, Storage},
    sealed_image::CapabilityRole,
};
use fe2o3_compiler_execution_protocol::{
    COMPILER_EXECUTION_EXTERNAL_ANCHOR_DEPLOYMENT_BYTES_V2 as BYTES,
    COMPILER_EXECUTION_EXTERNAL_ANCHOR_DEPLOYMENT_STORAGE_V2 as DEPLOYMENT_STORAGE,
    COMPILER_EXECUTION_EXTERNAL_ANCHOR_DEPLOYMENT_WORK_V2 as DEPLOYMENT_WORK,
    CompilerExecutionAttestationStorageV2 as ProtocolStorage,
    CompilerExecutionExternalAnchorDeploymentV2 as Deployment,
    CompilerExecutionIssuerPolicyV2 as Policy,
    CompilerExecutionSupervisorDeploymentV2 as Supervisor,
};
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
};
use std::{fs::File, os::fd::RawFd};

crate::compiler_execution_external_anchor_deployment_native::external_anchor_deployment_capability!(
    CompilerExecutionExternalAnchorDeploymentCapabilityV2,
    "2",
    "3"
);
