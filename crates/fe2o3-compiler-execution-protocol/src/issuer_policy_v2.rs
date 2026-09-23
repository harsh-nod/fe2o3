//! SubjectV2 policy admission. Decoding is not provisioning or execution proof.
use crate::{
    CompilerExecutionAttestationErrorV1, CompilerExecutionIssuerMeasurementV1 as Measurement,
    attestation_resources::{self as resources, CompilerExecutionAttestationStorageV2 as Storage},
    issuer_policy_codec as codec,
};
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
};
use std::{error::Error, fmt, mem::size_of};

pub const COMPILER_EXECUTION_ISSUER_POLICY_BYTES_V2: usize = codec::BYTES;
pub(crate) const RETAINED: usize =
    size_of::<CompilerExecutionIssuerPolicyV2>() + size_of::<Storage>();
/// Fixed logical admission work, including two Ed25519 public-key validations,
/// canonical encoding, hash, comparison and entry. Not an instruction bound.
pub const COMPILER_EXECUTION_ISSUER_POLICY_WORK_V2: usize =
    resources::ENTRY_WORK + 2 * resources::KEY_VALIDATION_WORK + 32 * codec::BYTES;
/// Additional logical peak: result and staging records, fields, SHA state and
/// fixed codec/crypto scratch. Caller-owned input remains separately prepaid.
/// This is not a generated stack, heap-allocation or RSS bound.
pub const COMPILER_EXECUTION_ISSUER_POLICY_STORAGE_V2: usize = 4 * RETAINED
    + 4 * size_of::<codec::Fields>()
    + 4 * codec::BYTES
    + 2 * size_of::<sha2::Sha256>()
    + 4096;
const _: () = {
    assert!(size_of::<(CompilerExecutionIssuerPolicyV2, Storage)>() <= RETAINED);
    assert!(
        8 * size_of::<CompilerExecutionAttestationErrorV2>()
            + 64 * size_of::<usize>()
            + 2 * size_of::<ed25519_dalek::VerifyingKey>()
            <= 4096
    );
};

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CompilerExecutionIssuerPolicyIdentityV2([u8; 32]);
impl CompilerExecutionIssuerPolicyIdentityV2 {
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    /// Checks an exact fixed wire identity, not policy pinning or authority.
    /// Keep the complete borrowed input owner prepaid on this same ledger.
    pub fn matches_canonical_bytes(self, bytes: &[u8], budget: &mut Budget<'_>) -> Result<bool> {
        metered(
            budget,
            resources::fixed_input_floor(bytes, codec::BYTES),
            || Ok(codec::V2.matches(self.0, bytes)),
        )
    }
}

/// Move-only, SubjectV2-specific public trust configuration. The policy's
/// measurements and two distinct keys must still be pinned by a trusted caller.
/// Construction/decoding grants no compiler, signing, load or launch authority.
///
/// Inputs stay prepaid; reserve the returned additional storage before keeping
/// the owner. All calls restore entry storage on success, error and unwind.
///
/// ```compile_fail
/// use fe2o3_compiler_execution_protocol::CompilerExecutionIssuerPolicyV2;
/// fn duplicate(policy: CompilerExecutionIssuerPolicyV2) { let _ = policy.clone(); }
/// ```
/// ```compile_fail
/// use fe2o3_compiler_execution_protocol::{CompilerExecutionIssuerPolicyV1, CompilerExecutionIssuerPolicyV2};
/// fn legacy(_: &CompilerExecutionIssuerPolicyV1) {}
/// fn mix(policy: &CompilerExecutionIssuerPolicyV2) { legacy(policy); }
/// ```
#[derive(Eq, PartialEq)]
pub struct CompilerExecutionIssuerPolicyV2 {
    pub(crate) record: codec::Record,
}
impl CompilerExecutionIssuerPolicyV2 {
    pub fn new(
        generation: u64,
        executable: Measurement,
        runtime: Measurement,
        verifying_key: [u8; 32],
        external_anchor_verifying_key: [u8; 32],
        budget: &mut Budget<'_>,
    ) -> Result<(Self, Storage)> {
        metered(budget, 0, || {
            Ok((
                Self {
                    record: codec::V2.encode(codec::Fields {
                        generation,
                        executable,
                        runtime,
                        verifying_key,
                        external_anchor_verifying_key,
                    })?,
                },
                Storage(RETAINED),
            ))
        })
    }

    /// Strict V2 decode. A V1 policy, including one with the same generation,
    /// keys and measurements, cannot be upgraded or accepted by this method.
    pub fn decode(bytes: &[u8], budget: &mut Budget<'_>) -> Result<(Self, Storage)> {
        metered(
            budget,
            resources::fixed_input_floor(bytes, codec::BYTES),
            || {
                Ok((
                    Self {
                        record: codec::V2.decode(bytes)?,
                    },
                    Storage(RETAINED),
                ))
            },
        )
    }

    pub const fn generation(&self) -> u64 {
        self.record.fields.generation
    }
    pub const fn executable(&self) -> Measurement {
        self.record.fields.executable
    }
    pub const fn runtime(&self) -> Measurement {
        self.record.fields.runtime
    }
    pub const fn verifying_key(&self) -> &[u8; 32] {
        &self.record.fields.verifying_key
    }
    pub const fn external_anchor_verifying_key(&self) -> &[u8; 32] {
        &self.record.fields.external_anchor_verifying_key
    }
    pub const fn identity(&self) -> CompilerExecutionIssuerPolicyIdentityV2 {
        CompilerExecutionIssuerPolicyIdentityV2(self.record.identity)
    }
    pub const fn canonical_bytes(&self) -> &[u8; codec::BYTES] {
        &self.record.bytes
    }
    /// Total fixed reservation to retain this owner, including its descriptor.
    pub const fn retained_storage(&self) -> usize {
        RETAINED
    }
}
impl fmt::Debug for CompilerExecutionIssuerPolicyV2 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CompilerExecutionIssuerPolicyV2")
            .field("generation", &self.generation())
            .field("identity", &self.identity())
            .finish_non_exhaustive()
    }
}

/// Shared framing diagnostics do not invoke a V1 decoder or grant V1 admission.
#[derive(Debug)]
pub enum CompilerExecutionAttestationErrorV2 {
    Framing(CompilerExecutionAttestationErrorV1),
    Resource(Resource),
}
type Result<T> = std::result::Result<T, CompilerExecutionAttestationErrorV2>;
impl From<CompilerExecutionAttestationErrorV1> for CompilerExecutionAttestationErrorV2 {
    fn from(value: CompilerExecutionAttestationErrorV1) -> Self {
        Self::Framing(value)
    }
}
impl From<Resource> for CompilerExecutionAttestationErrorV2 {
    fn from(value: Resource) -> Self {
        Self::Resource(value)
    }
}
impl fmt::Display for CompilerExecutionAttestationErrorV2 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Framing(e) => e.fmt(f),
            Self::Resource(e) => e.fmt(f),
        }
    }
}
impl Error for CompilerExecutionAttestationErrorV2 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(match self {
            Self::Framing(e) => e,
            Self::Resource(e) => e,
        })
    }
}

fn metered<T>(
    budget: &mut Budget<'_>,
    floor: usize,
    operation: impl FnOnce() -> Result<T>,
) -> Result<T> {
    resources::fixed(
        budget,
        floor,
        COMPILER_EXECUTION_ISSUER_POLICY_WORK_V2,
        COMPILER_EXECUTION_ISSUER_POLICY_STORAGE_V2,
        operation,
    )
}
