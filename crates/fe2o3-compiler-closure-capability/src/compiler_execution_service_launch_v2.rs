use crate::{
    native_capability::{NativeCapability, Record, Result, Storage},
    sealed_image::CapabilityRole,
};
use fe2o3_compiler_execution_protocol::{
    COMPILER_EXECUTION_SERVICE_LAUNCH_MANIFEST_BYTES_V2 as BYTES,
    CompilerExecutionServiceLaunchManifestV2 as Manifest,
};
use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as Budget;
use std::{fs::File, os::fd::RawFd};

crate::compiler_execution_service_launch_native::launch_capability!(
    CompilerExecutionServiceLaunchCapabilityV2,
    "2"
);

#[cfg(test)]
use crate::native_capability::CompilerExecutionCapabilityErrorV2::Launch as LaunchError;
#[cfg(test)]
use CompilerExecutionServiceLaunchCapabilityV2 as Cap;
#[cfg(test)]
use fe2o3_compiler_execution_protocol::{
    COMPILER_EXECUTION_SERVICE_LAUNCH_MANIFEST_STORAGE_V2 as SCRATCH,
    COMPILER_EXECUTION_SERVICE_LAUNCH_MANIFEST_WORK_V2 as WORK,
    CompilerExecutionIssuerPolicyV2 as Policy, CompilerExecutionIssuerPolicyV3 as OtherPolicy,
    CompilerExecutionServiceLaunchManifestErrorV2 as ManifestError,
    CompilerExecutionServiceLaunchManifestV3 as OtherManifest,
};
#[cfg(test)]
#[path = "native_service_launch_tests.rs"]
mod tests;
