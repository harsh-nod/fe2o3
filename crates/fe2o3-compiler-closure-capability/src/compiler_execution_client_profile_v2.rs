use crate::{
    native_capability::{
        CompilerExecutionCapabilityErrorV2 as Error, ENTRY_WORK, NativeCapability, Record, Result,
        Storage,
    },
    sealed_image::CapabilityRole,
    trusted_profile_tree as tree,
};
use fe2o3_compiler_execution_protocol::{
    COMPILER_EXECUTION_CLIENT_PROFILE_BYTES_V2 as BYTES,
    CompilerExecutionClientProfileV2 as Profile,
};
use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as Budget;
use rustix::fs::Mode;
use std::{fs::File, mem::size_of};

/// Sealed native trust configuration, not compiler or GPU launch authority.
/// Uses the same ownership/resource contract as PolicyCapabilityV2. Production
/// admission has one fixed root-owned path and never falls back to V1.
///
/// ```compile_fail
/// use fe2o3_compiler_closure_capability::CompilerExecutionClientProfileCapabilityV2;
/// fn duplicate(value: CompilerExecutionClientProfileCapabilityV2) { let _ = value.clone(); }
/// ```
/// ```compile_fail
/// use fe2o3_compiler_closure_capability::CompilerExecutionClientProfileCapabilityV2;
/// fn descriptor<T: std::os::fd::AsFd>() {}
/// descriptor::<CompilerExecutionClientProfileCapabilityV2>();
/// ```
/// ```compile_fail
/// use fe2o3_compiler_closure_capability::CompilerExecutionClientProfileCapabilityV2;
/// use fe2o3_compiler_execution_protocol::CompilerExecutionClientProfileV1;
/// use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1;
/// fn mix(profile: CompilerExecutionClientProfileV1, budget: &mut CanonicalKernelIrVerificationResourceBudgetV1<'_>) {
///     let _ = CompilerExecutionClientProfileCapabilityV2::create(profile, budget);
/// }
/// ```
pub struct CompilerExecutionClientProfileCapabilityV2(Capability);
crate::compiler_execution_client_profile_native::profile_capability!(
    CompilerExecutionClientProfileCapabilityV2,
    "client-profile-v2",
    "fe2o3-compiler-execution-client-profile-v2"
);

#[cfg(test)]
mod tests {
    use crate::native_capability::tests::profile;
    use fe2o3_compiler_execution_protocol::{
        COMPILER_EXECUTION_CLIENT_PROFILE_PATH_V2 as PROFILE_PATH,
        COMPILER_EXECUTION_CLIENT_PROFILE_STORAGE_V2 as PROFILE_STORAGE,
        COMPILER_EXECUTION_CLIENT_PROFILE_WORK_V2 as PROFILE_WORK,
        CompilerExecutionClientProfileErrorV2 as ProfileError,
    };
    type Cap = super::CompilerExecutionClientProfileCapabilityV2;
    const PROFILE_DOMAIN: &[u8] = b"FE2O3/COMPILER-EXECUTION-CLIENT-PROFILE/V2\0";
    fn profile_failure<T>(result: super::Result<T>) -> ProfileError {
        match crate::native_capability::tests::failure(result) {
            super::Error::Profile(error) => error,
            error => panic!("expected V2 profile error: {error:?}"),
        }
    }
    include!("native_profile_tests.rs");
}
