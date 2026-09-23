//! Native signed receipts. Signature validity is not protected execution authority.
use crate::{
    CompilerExecutionAttestationChallengeIdentityV2 as ChallengeIdentity,
    CompilerExecutionAttestationErrorV1 as Framing, CompilerExecutionAttestationErrorV2 as Error,
    CompilerExecutionAttestationRequestV2 as Request,
    CompilerExecutionIssuerPolicyIdentityV2 as PolicyIdentity,
    CompilerExecutionIssuerPolicyV2 as Policy, CompilerExecutionSubjectBindingV2 as Binding,
    attestation_receipt_codec as codec,
    attestation_resources::{self as resources, CompilerExecutionAttestationStorageV2 as Storage},
};
use ed25519_dalek::{Signature, SigningKey, VerifyingKey};
use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as Budget;
use std::{fmt, mem::size_of};

pub const COMPILER_EXECUTION_ATTESTATION_RECEIPT_BYTES_V2: usize = codec::BYTES;
pub const COMPILER_EXECUTION_ATTESTATION_RECEIPT_IDENTITY_WORK_V2: usize =
    resources::ENTRY_WORK + 32 * codec::BYTES;
pub const COMPILER_EXECUTION_ATTESTATION_RECEIPT_DECODE_WORK_V2: usize =
    COMPILER_EXECUTION_ATTESTATION_RECEIPT_IDENTITY_WORK_V2
        + 2 * resources::KEY_VALIDATION_WORK
        + resources::STRICT_VERIFY_WORK;
pub const COMPILER_EXECUTION_ATTESTATION_RECEIPT_ISSUE_WORK_V2: usize =
    COMPILER_EXECUTION_ATTESTATION_RECEIPT_DECODE_WORK_V2 + resources::SIGN_WORK;
pub const COMPILER_EXECUTION_ATTESTATION_RECEIPT_VERIFY_WORK_V2: usize =
    COMPILER_EXECUTION_ATTESTATION_RECEIPT_IDENTITY_WORK_V2
        + resources::KEY_VALIDATION_WORK
        + resources::STRICT_VERIFY_WORK;
const RETAINED: usize = size_of::<CompilerExecutionAttestationReceiptV2>() + size_of::<Storage>();
const VERIFIED_RETAINED: usize =
    size_of::<VerifiedCompilerExecutionAttestationV2>() + size_of::<Storage>();
/// Fixed additional logical peak: result, canonical staging, hash/public crypto
/// state and control scratch. Crypto work weights are admission units, not CPU
/// instructions, latency or a bound on the library's generated stack/RSS usage.
pub const COMPILER_EXECUTION_ATTESTATION_RECEIPT_STORAGE_V2: usize = 4 * VERIFIED_RETAINED
    + 4 * size_of::<codec::Fields>()
    + 4 * codec::BYTES
    + 2 * size_of::<sha2::Sha256>()
    + 4 * size_of::<VerifyingKey>()
    + 2 * size_of::<Signature>()
    + 4096;
const _: () = {
    assert!(VERIFIED_RETAINED == RETAINED);
    assert!(size_of::<(CompilerExecutionAttestationReceiptV2, Storage)>() <= RETAINED);
    assert!(size_of::<(VerifiedCompilerExecutionAttestationV2, Storage)>() <= VERIFIED_RETAINED);
    assert!(8 * size_of::<Error>() + 64 * size_of::<usize>() + size_of::<SigningKey>() <= 4096);
    assert!(
        size_of::<std::thread::Result<Result<(CompilerExecutionAttestationReceiptV2, Storage)>>>()
            <= RETAINED + 4096
    );
    assert!(
        size_of::<std::thread::Result<Result<(VerifiedCompilerExecutionAttestationV2, Storage)>>>()
            <= VERIFIED_RETAINED + 4096
    );
    assert!(size_of::<std::thread::Result<Result<bool>>>() <= 4096);
};
type Result<T> = std::result::Result<T, Error>;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CompilerExecutionAttestationReceiptIdentityV2([u8; 32]);
impl CompilerExecutionAttestationReceiptIdentityV2 {
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
    /// Content identity only; matching bytes does not authenticate a signature.
    pub fn matches_canonical_bytes(self, bytes: &[u8], budget: &mut Budget<'_>) -> Result<bool> {
        metered(
            budget,
            resources::fixed_input_floor(bytes, codec::BYTES),
            COMPILER_EXECUTION_ATTESTATION_RECEIPT_IDENTITY_WORK_V2,
            || Ok(codec::V2.matches(self.0, bytes)),
        )
    }
}

/// Move-only signed native request/subject binding. Decode authenticates the
/// embedded key, not a pinned policy, protected process, or fresh ledger state.
/// Issue/decode borrow prepaid inputs and return the full result charge.
///
/// ```compile_fail
/// use fe2o3_compiler_execution_protocol::CompilerExecutionAttestationReceiptV2;
/// fn duplicate(value: CompilerExecutionAttestationReceiptV2) { let _ = value.clone(); }
/// ```
/// ```compile_fail
/// use fe2o3_compiler_execution_protocol::{CompilerExecutionAttestationReceiptV2, CompilerExecutionIssuerPolicyV2, CompilerExecutionAttestationRequestV1};
/// use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1;
/// fn mix(r: CompilerExecutionAttestationReceiptV2, p: &CompilerExecutionIssuerPolicyV2,
///        q: &CompilerExecutionAttestationRequestV1, b: &mut CanonicalKernelIrVerificationResourceBudgetV1<'_>) {
///     let _ = r.verify(p, q, [0; 32], b);
/// }
/// ```
#[derive(Eq, PartialEq)]
pub struct CompilerExecutionAttestationReceiptV2 {
    record: codec::Record,
}
impl CompilerExecutionAttestationReceiptV2 {
    /// The borrowed key's full owner must stay prepaid along with policy/request.
    /// Key custody and protected execution supervision remain external duties.
    pub fn issue(
        policy: &Policy,
        request: &Request,
        key: &SigningKey,
        budget: &mut Budget<'_>,
    ) -> Result<(Self, Storage)> {
        metered(
            budget,
            policy.retained_storage() + request.retained_storage() + size_of::<SigningKey>(),
            COMPILER_EXECUTION_ATTESTATION_RECEIPT_ISSUE_WORK_V2,
            || {
                if key.verifying_key().as_bytes() != policy.verifying_key() {
                    return Err(Framing::SigningKeyMismatch.into());
                }
                Ok((
                    Self {
                        record: codec::V2.issue(expected(policy, request)?, key)?,
                    },
                    Storage(RETAINED),
                ))
            },
        )
    }
    pub fn decode(bytes: &[u8], budget: &mut Budget<'_>) -> Result<(Self, Storage)> {
        metered(
            budget,
            resources::fixed_input_floor(bytes, codec::BYTES),
            COMPILER_EXECUTION_ATTESTATION_RECEIPT_DECODE_WORK_V2,
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
    /// Consumes only the receipt. Its reservation transfers to the verified
    /// owner; reserve the returned delta. On refusal the receipt drops but its
    /// reservation remains for caller retirement. No rollback state is advanced.
    pub fn verify(
        self,
        policy: &Policy,
        request: &Request,
        current_rollback_anchor: [u8; 32],
        budget: &mut Budget<'_>,
    ) -> Result<(VerifiedCompilerExecutionAttestationV2, Storage)> {
        metered(
            budget,
            RETAINED + policy.retained_storage() + request.retained_storage(),
            COMPILER_EXECUTION_ATTESTATION_RECEIPT_VERIFY_WORK_V2,
            || {
                codec::V2.verify_signature(&self.record)?;
                let expected = expected(policy, request)?;
                codec::compare(&self.record.fields, &expected, current_rollback_anchor)?;
                Ok((
                    VerifiedCompilerExecutionAttestationV2 { receipt: self },
                    Storage(VERIFIED_RETAINED - RETAINED),
                ))
            },
        )
    }
    pub const fn request_sha256(&self) -> &[u8; 32] {
        &self.record.fields.request_sha256
    }
    pub const fn policy_identity(&self) -> PolicyIdentity {
        PolicyIdentity::from_bytes_for_protocol(self.record.fields.policy_identity)
    }
    pub const fn subject(&self) -> Binding {
        Binding(self.record.fields.subject)
    }
    pub const fn challenge_identity(&self) -> ChallengeIdentity {
        ChallengeIdentity::from_bytes_for_protocol(self.record.fields.challenge_identity)
    }
    pub const fn challenge_nonce(&self) -> [u8; 32] {
        self.record.fields.nonce
    }
    pub const fn sequence(&self) -> u64 {
        self.record.fields.sequence
    }
    pub const fn prior_rollback_anchor(&self) -> [u8; 32] {
        self.record.fields.prior_rollback_anchor
    }
    pub const fn next_rollback_anchor(&self) -> [u8; 32] {
        self.record.fields.next_rollback_anchor
    }
    pub const fn identity(&self) -> CompilerExecutionAttestationReceiptIdentityV2 {
        CompilerExecutionAttestationReceiptIdentityV2(self.record.identity)
    }
    pub const fn canonical_bytes(&self) -> &[u8; codec::BYTES] {
        &self.record.bytes
    }
    pub const fn retained_storage(&self) -> usize {
        RETAINED
    }
    pub const fn grants_compiler_authority(&self) -> bool {
        false
    }
    pub const fn grants_load_authority(&self) -> bool {
        false
    }
    pub const fn grants_launch_authority(&self) -> bool {
        false
    }
}
impl fmt::Debug for CompilerExecutionAttestationReceiptV2 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CompilerExecutionAttestationReceiptV2")
            .field("request_sha256", &self.request_sha256())
            .field("policy_identity", &self.policy_identity())
            .field("subject", &self.subject())
            .field("challenge_identity", &self.challenge_identity())
            .field("sequence", &self.sequence())
            .field("identity", &self.identity())
            .finish_non_exhaustive()
    }
}

/// A pinned-key signature and exact caller-supplied rollback input, not evidence
/// of protected execution or a durable ledger advance. Move-only; no authority.
///
/// ```compile_fail
/// use fe2o3_compiler_execution_protocol::VerifiedCompilerExecutionAttestationV2;
/// fn duplicate(value: VerifiedCompilerExecutionAttestationV2) { let moved = value; let _ = (moved, value); }
/// ```
/// ```compile_fail
/// use fe2o3_compiler_execution_protocol::VerifiedCompilerExecutionAttestationV2;
/// fn duplicate(value: VerifiedCompilerExecutionAttestationV2) { let _ = value.clone(); }
/// ```
#[derive(Debug)]
pub struct VerifiedCompilerExecutionAttestationV2 {
    receipt: CompilerExecutionAttestationReceiptV2,
}
impl VerifiedCompilerExecutionAttestationV2 {
    pub const fn receipt(&self) -> &CompilerExecutionAttestationReceiptV2 {
        &self.receipt
    }
    /// Storage-neutral downgrade, enforced by the equal-layout assertion above.
    pub fn into_receipt(self) -> CompilerExecutionAttestationReceiptV2 {
        self.receipt
    }
    pub const fn retained_storage(&self) -> usize {
        VERIFIED_RETAINED
    }
    pub const fn authenticates_pinned_signing_key(&self) -> bool {
        true
    }
    pub const fn authenticates_protected_compiler_execution(&self) -> bool {
        false
    }
    pub const fn grants_compiler_authority(&self) -> bool {
        false
    }
    pub const fn grants_load_authority(&self) -> bool {
        false
    }
    pub const fn grants_launch_authority(&self) -> bool {
        false
    }
}
fn expected(policy: &Policy, request: &Request) -> Result<codec::Fields> {
    Ok(codec::V2.expected(codec::ExpectedInput {
        policy_identity: *policy.identity().as_bytes(),
        verifying_key: *policy.verifying_key(),
        request_identity: *request.identity().as_bytes(),
        subject: Binding::from_subject(request.subject()).0,
        challenge: &request.challenge().record.fields,
        challenge_identity: *request.challenge().identity().as_bytes(),
    })?)
}
fn metered<T>(
    budget: &mut Budget<'_>,
    floor: usize,
    work: usize,
    f: impl FnOnce() -> Result<T>,
) -> Result<T> {
    resources::fixed(
        budget,
        floor,
        work,
        COMPILER_EXECUTION_ATTESTATION_RECEIPT_STORAGE_V2,
        f,
    )
}
