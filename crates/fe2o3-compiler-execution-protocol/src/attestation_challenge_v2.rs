//! Nominal native subject binding and issuer challenge, without issuance rights.
use crate::{
    CompilerExecutionAttestationErrorV2 as Error,
    CompilerExecutionIssuerPolicyIdentityV2 as PolicyIdentity,
    CompilerExecutionIssuerPolicyV2 as Policy, attestation_request_codec as codec,
    attestation_resources::{self as resources, CompilerExecutionAttestationStorageV2 as Storage},
};
use fe2o3_artifact_transaction::{
    InertCompilerExecutionSubjectStorageV2 as SubjectStorage,
    InertCompilerExecutionSubjectV2 as Subject,
};
use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as Budget;
use std::{fmt, mem::size_of};

pub const COMPILER_EXECUTION_ATTESTATION_CHALLENGE_BYTES_V2: usize = codec::CHALLENGE_BYTES;
pub const COMPILER_EXECUTION_ATTESTATION_CHALLENGE_WORK_V2: usize =
    resources::ENTRY_WORK + 32 * codec::CHALLENGE_BYTES;
pub(crate) const SUBJECT_RETAINED: usize = size_of::<Subject>() + size_of::<SubjectStorage>();
pub(crate) const RETAINED: usize =
    size_of::<CompilerExecutionAttestationChallengeV2>() + size_of::<Storage>();
/// Fixed additional logical peak including result, staging and hash/control
/// scratch. Caller inputs stay prepaid; this is not allocator/RSS/stack usage.
pub const COMPILER_EXECUTION_ATTESTATION_CHALLENGE_STORAGE_V2: usize = 4 * RETAINED
    + 4 * size_of::<codec::Fields>()
    + 4 * codec::CHALLENGE_BYTES
    + 2 * size_of::<sha2::Sha256>()
    + 4096;
const _: () = {
    assert!(size_of::<(CompilerExecutionAttestationChallengeV2, Storage)>() <= RETAINED);
    assert!(8 * size_of::<Error>() + 64 * size_of::<usize>() <= 4096);
};
type Result<T> = std::result::Result<T, Error>;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CompilerExecutionSubjectBindingV2(pub(crate) codec::Binding);
impl CompilerExecutionSubjectBindingV2 {
    pub(crate) fn from_subject(subject: &Subject) -> Self {
        Self(codec::Binding {
            sha256: *subject.identity().sha256(),
            byte_len: subject.identity().byte_len(),
        })
    }
    pub const fn sha256(self) -> [u8; 32] {
        self.0.sha256
    }
    pub const fn byte_len(self) -> u64 {
        self.0.byte_len
    }
    /// Compares the immutable, already-admitted subject identity. This does not
    /// reconstruct a handoff or authenticate a protected compiler occurrence.
    pub fn matches_subject(self, subject: &Subject, budget: &mut Budget<'_>) -> Result<bool> {
        metered(budget, SUBJECT_RETAINED, || {
            Ok(self == Self::from_subject(subject))
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CompilerExecutionAttestationChallengeIdentityV2([u8; 32]);
impl CompilerExecutionAttestationChallengeIdentityV2 {
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
    pub fn matches_canonical_bytes(self, bytes: &[u8], budget: &mut Budget<'_>) -> Result<bool> {
        metered(
            budget,
            resources::fixed_input_floor(bytes, codec::CHALLENGE_BYTES),
            || Ok(codec::V2.matches_challenge(self.0, bytes)),
        )
    }
}

/// Caller-supplied nonce/rollback position for one exact native policy/subject.
/// Canonical decoding establishes neither freshness nor trusted issuer origin.
/// Inputs stay prepaid; reserve the returned full retained charge before use.
///
/// ```compile_fail
/// use fe2o3_compiler_execution_protocol::CompilerExecutionAttestationChallengeV2;
/// fn duplicate(value: CompilerExecutionAttestationChallengeV2) { let _ = value.clone(); }
/// ```
/// ```compile_fail
/// use fe2o3_compiler_execution_protocol::{CompilerExecutionIssuerPolicyV1, CompilerExecutionAttestationChallengeV2};
/// use fe2o3_artifact_transaction::InertCompilerExecutionSubjectV2;
/// use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1;
/// fn mix(p: &CompilerExecutionIssuerPolicyV1, s: &InertCompilerExecutionSubjectV2,
///        b: &mut CanonicalKernelIrVerificationResourceBudgetV1<'_>) {
///     let _ = CompilerExecutionAttestationChallengeV2::new(p, s, [1; 32], 1, [0; 32], b);
/// }
/// ```
#[derive(Eq, PartialEq)]
pub struct CompilerExecutionAttestationChallengeV2 {
    pub(crate) record: codec::Challenge,
}
impl CompilerExecutionAttestationChallengeV2 {
    pub fn new(
        policy: &Policy,
        subject: &Subject,
        nonce: [u8; 32],
        sequence: u64,
        prior_rollback_anchor: [u8; 32],
        budget: &mut Budget<'_>,
    ) -> Result<(Self, Storage)> {
        metered(budget, policy.retained_storage() + SUBJECT_RETAINED, || {
            Ok((
                Self {
                    record: codec::V2.challenge(codec::Fields {
                        policy: *policy.identity().as_bytes(),
                        subject: CompilerExecutionSubjectBindingV2::from_subject(subject).0,
                        nonce,
                        sequence,
                        prior: prior_rollback_anchor,
                    })?,
                },
                Storage(RETAINED),
            ))
        })
    }
    pub fn decode(bytes: &[u8], budget: &mut Budget<'_>) -> Result<(Self, Storage)> {
        metered(
            budget,
            resources::fixed_input_floor(bytes, codec::CHALLENGE_BYTES),
            || {
                Ok((
                    Self {
                        record: codec::V2.decode_challenge(bytes)?,
                    },
                    Storage(RETAINED),
                ))
            },
        )
    }
    pub const fn policy_identity(&self) -> PolicyIdentity {
        PolicyIdentity::from_bytes_for_protocol(self.record.fields.policy)
    }
    pub const fn subject(&self) -> CompilerExecutionSubjectBindingV2 {
        CompilerExecutionSubjectBindingV2(self.record.fields.subject)
    }
    pub const fn nonce(&self) -> [u8; 32] {
        self.record.fields.nonce
    }
    pub const fn sequence(&self) -> u64 {
        self.record.fields.sequence
    }
    pub const fn prior_rollback_anchor(&self) -> [u8; 32] {
        self.record.fields.prior
    }
    pub const fn identity(&self) -> CompilerExecutionAttestationChallengeIdentityV2 {
        CompilerExecutionAttestationChallengeIdentityV2(self.record.identity)
    }
    pub const fn canonical_bytes(&self) -> &[u8; codec::CHALLENGE_BYTES] {
        &self.record.bytes
    }
    pub const fn retained_storage(&self) -> usize {
        RETAINED
    }
}
impl fmt::Debug for CompilerExecutionAttestationChallengeV2 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CompilerExecutionAttestationChallengeV2")
            .field("policy_identity", &self.policy_identity())
            .field("subject", &self.subject())
            .field("sequence", &self.sequence())
            .field("identity", &self.identity())
            .finish_non_exhaustive()
    }
}
fn metered<T>(budget: &mut Budget<'_>, floor: usize, f: impl FnOnce() -> Result<T>) -> Result<T> {
    resources::fixed(
        budget,
        floor,
        COMPILER_EXECUTION_ATTESTATION_CHALLENGE_WORK_V2,
        COMPILER_EXECUTION_ATTESTATION_CHALLENGE_STORAGE_V2,
        f,
    )
}
