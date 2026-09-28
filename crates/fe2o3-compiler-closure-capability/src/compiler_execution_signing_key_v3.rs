//! Fresh native key custody, separate from public-record transport and V1 owners.
use crate::{
    native_capability::{
        CompilerExecutionCapabilityErrorV2 as Error, ENTRY_WORK, Result, Storage, envelope_overhead,
    },
    native_secret::{SeedGuard, read_secret, with_secret},
    sealed_image::{CapabilityRole, SealedCapabilityImage},
};
use ed25519_dalek::{Signer, SigningKey};
use fe2o3_compiler_execution_protocol::{
    COMPILER_EXECUTION_SUPERVISOR_DEPLOYMENT_STORAGE_V3 as DEPLOYMENT_STORAGE,
    COMPILER_EXECUTION_SUPERVISOR_DEPLOYMENT_WORK_V3 as DEPLOYMENT_WORK,
    CompilerExecutionAttestationReceiptV3 as Receipt,
    CompilerExecutionAttestationRequestV3 as Request,
    CompilerExecutionAttestationStorageV3 as ProtocolStorage,
    CompilerExecutionCurrentRecordAttestationV3 as CurrentAttestation,
    CompilerExecutionCurrentRecordVerificationV3 as CurrentVerification,
    CompilerExecutionIssuerPolicyIdentityV3 as PolicyIdentity,
    CompilerExecutionIssuerPolicyV3 as Policy, CompilerExecutionReceiptCarriageV3 as Carriage,
    CompilerExecutionSupervisorDeploymentV3 as Deployment,
};
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
};
use std::{fmt, fs::File, mem::size_of, os::fd::RawFd};
use subtle::ConstantTimeEq;
use zeroize::ZeroizeOnDrop;

const KEY_BYTES: usize = 32;
const CRYPTO_SCRATCH: usize = 4096;
const ROLE: CapabilityRole = CapabilityRole {
    name: "native compiler-execution signing-key capability",
    memfd_name: "fe2o3-compiler-execution-signing-key-v3",
};

/// Move-only secret custody pinned to the complete SubjectV3 policy identity.
/// No raw key, seed or descriptor getter is exposed. A transferred read-only
/// File contains the readable seed and must remain in trusted custody.
/// Signing authenticates bytes; the issuer separately establishes real compiler
/// occurrence, currentness, durability and authority.
///
/// Keep inputs prepaid on the same ledger and reserve returned output growth.
/// The original seed is wiped on success, refusal and unwind, including entry
/// resource denial. Closing the sealed image does not prove kernel-page erasure.
///
/// ```
/// use fe2o3_compiler_closure_capability::{
///     CompilerExecutionSigningKeyCapabilityV3 as Cap, CompilerExecutionCapabilityErrorV2 as Error,
/// };
/// use fe2o3_compiler_execution_protocol::{
///     CompilerExecutionIssuerPolicyV3 as Policy, CompilerExecutionAttestationRequestV3 as Request,
///     CompilerExecutionAttestationReceiptV3 as Receipt, CompilerExecutionAttestationStorageV3 as Storage,
/// };
/// use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as Budget;
/// // The key, policy and request are already prepaid on this ledger.
/// fn issue(key: &Cap, policy: &Policy, request: &Request, budget: &mut Budget<'_>)
///     -> Result<(Receipt, Storage), Error>
/// {
///     key.issue_receipt(policy, request, budget)
/// }
/// ```
/// ```compile_fail
/// use fe2o3_compiler_closure_capability::CompilerExecutionSigningKeyCapabilityV3 as Cap;
/// fn duplicate<T: Clone>() {}
/// duplicate::<Cap>();
/// ```
/// ```compile_fail
/// use fe2o3_compiler_closure_capability::CompilerExecutionSigningKeyCapabilityV3 as Cap;
/// fn descriptor<T: std::os::fd::AsFd>() {}
/// descriptor::<Cap>();
/// ```
/// ```compile_fail
/// use fe2o3_compiler_closure_capability::CompilerExecutionSigningKeyCapabilityV3 as Cap;
/// use fe2o3_compiler_execution_protocol::CompilerExecutionIssuerPolicyV2 as Policy;
/// use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as Budget;
/// fn mix(seed: &mut [u8; 32], policy: &Policy, budget: &mut Budget<'_>) {
///     let _ = Cap::create_and_zeroize(seed, policy, budget);
/// }
/// ```
/// ```compile_fail
/// use fe2o3_compiler_closure_capability::CompilerExecutionSigningKeyCapabilityV3 as Cap;
/// use fe2o3_compiler_execution_protocol::{
///     CompilerExecutionIssuerPolicyV3 as Policy, CompilerExecutionAttestationRequestV2 as Request,
/// };
/// use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as Budget;
/// fn mix(key: &Cap, policy: &Policy, request: &Request, budget: &mut Budget<'_>) {
///     let _ = key.issue_receipt(policy, request, budget);
/// }
/// ```
/// ```compile_fail
/// use fe2o3_compiler_closure_capability::CompilerExecutionSigningKeyCapabilityV3 as Cap;
/// use fe2o3_compiler_execution_protocol::{
///     CompilerExecutionIssuerPolicyV3 as Policy, CompilerExecutionReceiptCarriageV2 as Carriage,
///     CompilerExecutionCurrentRecordVerificationV3 as Verification,
/// };
/// use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as Budget;
/// fn mix(key: &Cap, policy: &Policy, carriage: &Carriage, v: Verification, b: &mut Budget<'_>) {
///     let _ = key.attest_current(policy, carriage, v, [1; 32], b);
/// }
/// ```
/// ```compile_fail
/// use fe2o3_compiler_closure_capability::{
///     CompilerExecutionSigningKeyCapabilityV2 as V2, CompilerExecutionSigningKeyCapabilityV3 as V3,
/// };
/// fn downgrade(key: V3) -> V2 { key.into() }
/// ```
pub struct CompilerExecutionSigningKeyCapabilityV3 {
    key: SigningKey,
    image: SealedCapabilityImage,
    policy: PolicyIdentity,
}
crate::compiler_execution_signing_key_native::signing_key_capability!(
    CompilerExecutionSigningKeyCapabilityV3,
    issue_native_v3,
    Error::from
);

#[cfg(test)]
#[path = "compiler_execution_signing_key_v3_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "compiler_execution_signing_key_v3_operations_tests.rs"]
mod operations_tests;

#[cfg(test)]
mod reissue_tests {
    use super::CompilerExecutionSigningKeyCapabilityV3 as Cap;
    include!("compiler_execution_signing_key_native_reissue_tests.rs");
}
