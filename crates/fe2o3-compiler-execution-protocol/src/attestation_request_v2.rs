//! Complete native request. Subject decoding shares the caller's bounded ledger.
use crate::{
    CompilerExecutionAttestationErrorV1 as Framing, CompilerExecutionAttestationErrorV2 as Error,
    attestation_challenge_v2::{
        self as challenge, CompilerExecutionAttestationChallengeV2 as Challenge,
        CompilerExecutionSubjectBindingV2 as Binding,
    },
    attestation_request_codec as codec,
    attestation_resources::{self as resources, CompilerExecutionAttestationStorageV2 as Storage},
};
use fe2o3_artifact_transaction::{
    INERT_COMPILER_EXECUTION_SUBJECT_STORAGE_V2 as SUBJECT_STORAGE,
    INERT_COMPILER_EXECUTION_SUBJECT_WORK_V2 as SUBJECT_WORK,
    InertCompilerExecutionSubjectV2 as Subject,
};
use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as Budget;
use std::{fmt, mem::size_of};

pub const COMPILER_EXECUTION_ATTESTATION_REQUEST_BYTES_V2: usize = codec::REQUEST_BYTES;
pub const COMPILER_EXECUTION_ATTESTATION_REQUEST_WORK_V2: usize =
    resources::ENTRY_WORK + 32 * codec::REQUEST_BYTES;
/// Decode work includes the actual nested SubjectV2 decoder, not a new meter.
pub const COMPILER_EXECUTION_ATTESTATION_REQUEST_DECODE_WORK_V2: usize =
    COMPILER_EXECUTION_ATTESTATION_REQUEST_WORK_V2 + SUBJECT_WORK;
pub(crate) const RETAINED: usize =
    size_of::<CompilerExecutionAttestationRequestV2>() + size_of::<Storage>();
const INHERITED: usize = challenge::RETAINED + challenge::SUBJECT_RETAINED;
/// Additional fixed logical construction peak, excluding prepaid input owners.
pub const COMPILER_EXECUTION_ATTESTATION_REQUEST_STORAGE_V2: usize =
    4 * RETAINED + 4 * codec::REQUEST_BYTES + 2 * size_of::<sha2::Sha256>() + 4096;
/// Exact valid-decode peak quota: fixed outer frame and nested subject scratch.
pub const COMPILER_EXECUTION_ATTESTATION_REQUEST_DECODE_STORAGE_V2: usize =
    COMPILER_EXECUTION_ATTESTATION_REQUEST_STORAGE_V2 + SUBJECT_STORAGE;
const _: () = {
    assert!(RETAINED >= INHERITED);
    assert!(SUBJECT_STORAGE >= challenge::SUBJECT_RETAINED);
    assert!(size_of::<(CompilerExecutionAttestationRequestV2, Storage)>() <= RETAINED);
    assert!(8 * size_of::<Error>() + 64 * size_of::<usize>() <= 4096);
};
type Result<T> = std::result::Result<T, Error>;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CompilerExecutionAttestationRequestIdentityV2([u8; 32]);
impl CompilerExecutionAttestationRequestIdentityV2 {
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
    pub fn matches_canonical_bytes(self, bytes: &[u8], budget: &mut Budget<'_>) -> Result<bool> {
        resources::fixed(
            budget,
            resources::fixed_input_floor(bytes, codec::REQUEST_BYTES),
            COMPILER_EXECUTION_ATTESTATION_REQUEST_WORK_V2,
            COMPILER_EXECUTION_ATTESTATION_REQUEST_STORAGE_V2,
            || Ok(codec::V2.matches_request(self.0, bytes)),
        )
    }
}

/// Move-only complete native subject plus its exact challenge. This is inert
/// framing/content agreement, not protected execution or signing authority.
///
/// ```compile_fail
/// use fe2o3_compiler_execution_protocol::CompilerExecutionAttestationRequestV2;
/// fn duplicate(value: CompilerExecutionAttestationRequestV2) { let _ = value.clone(); }
/// ```
/// ```compile_fail
/// use fe2o3_compiler_execution_protocol::{CompilerExecutionAttestationChallengeV2, CompilerExecutionAttestationRequestV2};
/// use fe2o3_artifact_transaction::InertCompilerExecutionSubjectV1;
/// use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1;
/// fn mix(c: CompilerExecutionAttestationChallengeV2, s: InertCompilerExecutionSubjectV1,
///        b: &mut CanonicalKernelIrVerificationResourceBudgetV1<'_>) {
///     let _ = CompilerExecutionAttestationRequestV2::new(c, s, b);
/// }
/// ```
/// ```compile_fail
/// use fe2o3_compiler_execution_protocol::{CompilerExecutionAttestationChallengeV1, CompilerExecutionAttestationRequestV2};
/// use fe2o3_artifact_transaction::InertCompilerExecutionSubjectV2;
/// use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1;
/// fn mix(c: CompilerExecutionAttestationChallengeV1, s: InertCompilerExecutionSubjectV2,
///        b: &mut CanonicalKernelIrVerificationResourceBudgetV1<'_>) {
///     let _ = CompilerExecutionAttestationRequestV2::new(c, s, b);
/// }
/// ```
#[derive(Eq, PartialEq)]
pub struct CompilerExecutionAttestationRequestV2 {
    challenge: Challenge,
    subject: Subject,
    identity: CompilerExecutionAttestationRequestIdentityV2,
    canonical_bytes: [u8; codec::REQUEST_BYTES],
}
impl CompilerExecutionAttestationRequestV2 {
    /// Transfer both prepaid input reservations; reserve only the returned delta.
    /// On error both inputs drop but their reservations remain for caller cleanup.
    pub fn new(
        challenge: Challenge,
        subject: Subject,
        budget: &mut Budget<'_>,
    ) -> Result<(Self, Storage)> {
        resources::fixed(
            budget,
            INHERITED,
            COMPILER_EXECUTION_ATTESTATION_REQUEST_WORK_V2,
            COMPILER_EXECUTION_ATTESTATION_REQUEST_STORAGE_V2,
            || {
                Ok((
                    Self::from_owned(challenge, subject)?,
                    Storage(RETAINED - INHERITED),
                ))
            },
        )
    }
    /// Borrow a prepaid wire owner; returns the FULL result charge, not a delta
    /// inherited from its internally decoded children. SubjectV2 enforces its
    /// existing V4 storage ceiling on this same ledger. Entry storage is restored
    /// on success, refusal and unwind, without resetting work or denial history.
    pub fn decode(bytes: &[u8], budget: &mut Budget<'_>) -> Result<(Self, Storage)> {
        resources::nested_fixed(
            budget,
            resources::fixed_input_floor(bytes, codec::REQUEST_BYTES),
            COMPILER_EXECUTION_ATTESTATION_REQUEST_WORK_V2,
            COMPILER_EXECUTION_ATTESTATION_REQUEST_STORAGE_V2,
            |budget| {
                let parts = codec::V2.request_parts(bytes)?;
                let challenge = Challenge {
                    record: codec::V2.decode_challenge(parts.challenge)?,
                };
                let (subject, storage) = Subject::decode(parts.subject, budget)?;
                budget.reserve_storage(storage.retained_storage())?;
                crate::attestation::require_identity(parts.identity, "request")?;
                let decoded = Self::from_owned(challenge, subject)?;
                if *decoded.identity.as_bytes() != parts.identity
                    || decoded.canonical_bytes.as_slice() != bytes
                {
                    return Err(Framing::IdentityMismatch("request").into());
                }
                Ok((decoded, Storage(RETAINED)))
            },
        )
    }
    fn from_owned(challenge: Challenge, subject: Subject) -> Result<Self> {
        if challenge.subject() != Binding::from_subject(&subject) {
            return Err(Framing::SubjectMismatch.into());
        }
        let (canonical_bytes, identity) =
            codec::V2.request(challenge.canonical_bytes(), subject.canonical_bytes());
        Ok(Self {
            challenge,
            subject,
            identity: CompilerExecutionAttestationRequestIdentityV2(identity),
            canonical_bytes,
        })
    }
    pub const fn challenge(&self) -> &Challenge {
        &self.challenge
    }
    pub const fn subject(&self) -> &Subject {
        &self.subject
    }
    pub const fn identity(&self) -> CompilerExecutionAttestationRequestIdentityV2 {
        self.identity
    }
    pub const fn canonical_bytes(&self) -> &[u8; codec::REQUEST_BYTES] {
        &self.canonical_bytes
    }
    pub const fn retained_storage(&self) -> usize {
        RETAINED
    }
}
impl fmt::Debug for CompilerExecutionAttestationRequestV2 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CompilerExecutionAttestationRequestV2")
            .field("challenge", &self.challenge)
            .field("subject", &self.subject)
            .field("identity", &self.identity)
            .finish_non_exhaustive()
    }
}
