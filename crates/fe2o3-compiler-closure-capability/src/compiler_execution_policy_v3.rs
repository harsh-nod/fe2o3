use crate::{
    native_capability::{NativeCapability, Record, Result, Storage},
    sealed_image::CapabilityRole,
};
use fe2o3_compiler_execution_protocol::{
    COMPILER_EXECUTION_ISSUER_POLICY_BYTES_V3 as BYTES, CompilerExecutionIssuerPolicyV3 as Policy,
};
use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as Budget;
use std::{fs::File, os::fd::RawFd};

/// Move-only sealed SubjectV3 policy, requiring independent policy pinning.
/// This transports public trust data; it grants no signing or execution authority.
/// Inputs stay prepaid on the same ledger; reserve returned deltas before retaining
/// results, and retire the full charge only after drop or ownership transfer.
///
/// ```
/// use fe2o3_compiler_closure_capability::{
///     CompilerExecutionPolicyCapabilityV3 as Cap, CompilerExecutionCapabilityErrorV2 as Error,
/// };
/// use fe2o3_compiler_execution_protocol::CompilerExecutionIssuerPolicyV3 as Policy;
/// use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as Budget;
/// fn retain(policy: Policy, budget: &mut Budget<'_>) -> Result<Cap, Error> {
///     budget.reserve_storage(policy.retained_storage())?;
///     let (cap, delta) = Cap::create(policy, budget)?;
///     budget.reserve_storage(delta.additional_storage())?;
///     cap.revalidate(budget)?;
///     Ok(cap)
/// }
/// ```
/// ```compile_fail
/// use fe2o3_compiler_closure_capability::CompilerExecutionPolicyCapabilityV3 as Cap;
/// fn duplicate<T: Clone>() {}
/// duplicate::<Cap>();
/// ```
/// ```compile_fail
/// use fe2o3_compiler_closure_capability::CompilerExecutionPolicyCapabilityV3 as Cap;
/// fn descriptor<T: std::os::fd::AsFd>() {}
/// descriptor::<Cap>();
/// ```
/// ```compile_fail
/// use fe2o3_compiler_closure_capability::CompilerExecutionPolicyCapabilityV3 as Cap;
/// use fe2o3_compiler_execution_protocol::CompilerExecutionIssuerPolicyV2;
/// use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as Budget;
/// fn mix(policy: CompilerExecutionIssuerPolicyV2, budget: &mut Budget<'_>) {
///     let _ = Cap::create(policy, budget);
/// }
/// ```
/// ```compile_fail
/// use fe2o3_compiler_closure_capability::{
///     CompilerExecutionPolicyCapabilityV2 as V2, CompilerExecutionPolicyCapabilityV3 as V3,
/// };
/// fn downgrade(cap: V3) -> V2 { cap.into() }
/// ```
pub struct CompilerExecutionPolicyCapabilityV3(Capability);
crate::compiler_execution_policy_native::policy_capability!(
    CompilerExecutionPolicyCapabilityV3,
    "fe2o3-compiler-execution-policy-v3"
);

#[cfg(test)]
#[path = "compiler_execution_policy_v3_tests.rs"]
mod tests;
