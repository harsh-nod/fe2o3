use crate::{
    native_capability::{NativeCapability, Record, Result, Storage},
    sealed_image::CapabilityRole,
};
use fe2o3_compiler_execution_protocol::{
    COMPILER_EXECUTION_EXTERNAL_ANCHOR_DEPLOYMENT_BYTES_V3 as BYTES,
    COMPILER_EXECUTION_EXTERNAL_ANCHOR_DEPLOYMENT_STORAGE_V3 as DEPLOYMENT_STORAGE,
    COMPILER_EXECUTION_EXTERNAL_ANCHOR_DEPLOYMENT_WORK_V3 as DEPLOYMENT_WORK,
    CompilerExecutionAttestationStorageV3 as ProtocolStorage,
    CompilerExecutionExternalAnchorDeploymentV3 as Deployment,
    CompilerExecutionIssuerPolicyV3 as Policy,
    CompilerExecutionSupervisorDeploymentV3 as Supervisor,
};
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
};
use std::{fs::File, os::fd::RawFd};

crate::compiler_execution_external_anchor_deployment_native::external_anchor_deployment_capability!(
    CompilerExecutionExternalAnchorDeploymentCapabilityV3,
    "3",
    "2"
);
