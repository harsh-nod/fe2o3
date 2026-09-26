//! Fresh native key custody, separate from public-record transport and V1 owners.
use crate::{
    native_capability::{
        CompilerExecutionCapabilityErrorV2 as Error, ENTRY_WORK, Result, Storage, envelope_overhead,
    },
    sealed_image::{CapabilityRole, SealedCapabilityImage},
};
use ed25519_dalek::{Signer, SigningKey};
use fe2o3_compiler_execution_protocol::{
    CompilerExecutionAttestationReceiptV2 as Receipt,
    CompilerExecutionAttestationRequestV2 as Request,
    CompilerExecutionAttestationStorageV2 as ProtocolStorage,
    CompilerExecutionCurrentRecordAttestationV3 as CurrentAttestation,
    CompilerExecutionCurrentRecordVerificationV3 as CurrentVerification,
    CompilerExecutionIssuerPolicyIdentityV2 as PolicyIdentity,
    CompilerExecutionIssuerPolicyV2 as Policy,
    CompilerExecutionNativeJournalErrorV2 as JournalError,
    CompilerExecutionReceiptCarriageV2 as Carriage,
};
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
};
use std::{fmt, fs::File, mem::size_of, os::fd::RawFd};
use subtle::ConstantTimeEq;
use zeroize::{Zeroize, ZeroizeOnDrop};

const KEY_BYTES: usize = 32;
const CRYPTO_SCRATCH: usize = 4096;
const ROLE: CapabilityRole = CapabilityRole {
    name: "native compiler-execution signing-key capability",
    memfd_name: "fe2o3-compiler-execution-signing-key-v2",
};

/// Move-only secret custody pinned to a complete native policy identity.
///
/// Fresh admission derives the public key once. Revalidation compares the seed
/// without deriving another key. Signing operations retain and revalidate this
/// capability; no direct seed/key getter, V1 owner conversion, process launch,
/// or execution authority is exposed. The consuming protected issuer must
/// independently establish occurrence, currentness and durable ordering.
/// A transferred File contains the readable seed: its recipient must be trusted.
///
/// Inputs stay prepaid on the same ledger. Calls restore entry storage; reserve
/// each returned delta before retaining the result. Retire a consumed input's
/// charge only after its drop or explicit ownership transfer, including errors.
///
/// ```compile_fail
/// use fe2o3_compiler_closure_capability::CompilerExecutionSigningKeyCapabilityV2;
/// fn clone<T: Clone>() {}
/// clone::<CompilerExecutionSigningKeyCapabilityV2>();
/// ```
/// ```compile_fail
/// use fe2o3_compiler_closure_capability::CompilerExecutionSigningKeyCapabilityV2;
/// fn descriptor<T: std::os::fd::AsFd>() {}
/// descriptor::<CompilerExecutionSigningKeyCapabilityV2>();
/// ```
/// ```compile_fail
/// use fe2o3_compiler_closure_capability::{
///     CompilerExecutionSigningKeyCapabilityV1, CompilerExecutionSigningKeyCapabilityV2,
/// };
/// fn upgrade(key: CompilerExecutionSigningKeyCapabilityV1) -> CompilerExecutionSigningKeyCapabilityV2 {
///     key.into()
/// }
/// ```
pub struct CompilerExecutionSigningKeyCapabilityV2 {
    key: SigningKey,
    image: SealedCapabilityImage,
    policy: PolicyIdentity,
}

crate::compiler_execution_signing_key_native::signing_key_capability!(
    CompilerExecutionSigningKeyCapabilityV2,
    issue_native,
    currentness_error
);

// Preserve the V2 currentness error contract.
fn currentness_error(error: JournalError) -> Error {
    match error {
        JournalError::Resource(resource) => Error::Resource(resource),
        _ => Error::Rejected("native currentness authentication failed"),
    }
}

#[cfg(test)]
#[path = "compiler_execution_signing_key_v2_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "compiler_execution_signing_key_v2_binding_tests.rs"]
mod binding_tests;
