//! Nominal V3 sealed launch image; the shared wire alone does not pin a policy.
//!
//! ```compile_fail
//! use fe2o3_compiler_closure_capability::CompilerExecutionServiceLaunchCapabilityV3 as C;
//! use fe2o3_compiler_execution_protocol::CompilerExecutionServiceLaunchManifestV2 as M;
//! use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as B;
//! fn mix(manifest: M, budget: &mut B<'_>) { let _ = C::create(manifest, budget); }
//! ```
//! ```compile_fail
//! use fe2o3_compiler_closure_capability::{CompilerExecutionServiceLaunchCapabilityV3 as C3,
//!     CompilerExecutionServiceLaunchCapabilityV2 as C2};
//! fn downgrade(cap: C3) -> C2 { cap.into() }
//! ```
use crate::{
    native_capability::{NativeCapability, Record, Result, Storage},
    sealed_image::CapabilityRole,
};
use fe2o3_compiler_execution_protocol::{
    COMPILER_EXECUTION_SERVICE_LAUNCH_MANIFEST_BYTES_V3 as BYTES,
    CompilerExecutionServiceLaunchManifestV3 as Manifest,
};
use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as Budget;
use std::{fs::File, os::fd::RawFd};

crate::compiler_execution_service_launch_native::launch_capability!(
    CompilerExecutionServiceLaunchCapabilityV3,
    "3"
);

#[cfg(test)]
use crate::native_capability::CompilerExecutionCapabilityErrorV2::LaunchV3 as LaunchError;
#[cfg(test)]
use CompilerExecutionServiceLaunchCapabilityV3 as Cap;
#[cfg(test)]
use fe2o3_compiler_execution_protocol::{
    COMPILER_EXECUTION_SERVICE_LAUNCH_MANIFEST_STORAGE_V3 as SCRATCH,
    COMPILER_EXECUTION_SERVICE_LAUNCH_MANIFEST_WORK_V3 as WORK,
    CompilerExecutionIssuerPolicyV2 as OtherPolicy, CompilerExecutionIssuerPolicyV3 as Policy,
    CompilerExecutionServiceLaunchManifestErrorV3 as ManifestError,
    CompilerExecutionServiceLaunchManifestV2 as OtherManifest,
};
#[cfg(test)]
#[path = "native_service_launch_tests.rs"]
mod tests;
