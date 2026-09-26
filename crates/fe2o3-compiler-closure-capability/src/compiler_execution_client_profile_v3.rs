use crate::{
    native_capability::{
        CompilerExecutionCapabilityErrorV2 as Error, ENTRY_WORK, NativeCapability, Record, Result,
        Storage,
    },
    sealed_image::CapabilityRole,
    trusted_profile_tree as tree,
};
use fe2o3_compiler_execution_protocol::{
    COMPILER_EXECUTION_CLIENT_PROFILE_BYTES_V3 as BYTES,
    CompilerExecutionClientProfileV3 as Profile,
};
use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as Budget;
use rustix::fs::Mode;
use std::{fs::File, mem::size_of};

/// Move-only sealed SubjectV3 trust configuration, not compiler or launch authority.
/// The caller must retain the original resource budget and reserve returned deltas
/// before keeping owners. Production admission uses only the fixed root-owned V3
/// path, with no environment overrides or fallback to V2/V1.
///
/// ```compile_fail
/// use fe2o3_compiler_closure_capability::CompilerExecutionClientProfileCapabilityV3;
/// fn duplicate(value: CompilerExecutionClientProfileCapabilityV3) { let _ = value.clone(); }
/// ```
/// ```compile_fail
/// use fe2o3_compiler_closure_capability::CompilerExecutionClientProfileCapabilityV3;
/// fn descriptor<T: std::os::fd::AsFd>() {}
/// descriptor::<CompilerExecutionClientProfileCapabilityV3>();
/// ```
/// ```compile_fail
/// use fe2o3_compiler_closure_capability::CompilerExecutionClientProfileCapabilityV3;
/// use fe2o3_compiler_execution_protocol::CompilerExecutionClientProfileV2;
/// use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1;
/// fn mix(profile: CompilerExecutionClientProfileV2, budget: &mut CanonicalKernelIrVerificationResourceBudgetV1<'_>) {
///     let _ = CompilerExecutionClientProfileCapabilityV3::create(profile, budget);
/// }
/// ```
/// ```compile_fail
/// use fe2o3_compiler_closure_capability::{
///     CompilerExecutionClientProfileCapabilityV2 as V2,
///     CompilerExecutionClientProfileCapabilityV3 as V3,
/// };
/// fn downgrade(cap: V3) -> V2 { cap.into() }
/// ```
pub struct CompilerExecutionClientProfileCapabilityV3(Capability);
crate::compiler_execution_client_profile_native::profile_capability!(
    CompilerExecutionClientProfileCapabilityV3,
    "client-profile-v3",
    "fe2o3-compiler-execution-client-profile-v3"
);

#[cfg(test)]
#[path = "native_profile_v3_tests.rs"]
mod tests;
