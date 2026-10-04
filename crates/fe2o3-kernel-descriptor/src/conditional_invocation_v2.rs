//! Inert CPU-bound conditional invocation V2 schema.
//!
//! This projection commits the seventh full-frame CPU input digest. Successful
//! validation checks content only, never source origin, a proof signature, native
//! custody, a host premise, or publication/launch authority.

use crate::conditional_invocation_v1::*;

/// Distinct V2 frame tag; V1 never accepts this tag.
pub const CONDITIONAL_INVOCATION_MAGIC_V2: [u8; 8] = *b"FE2O3C2\0";
/// CPU-bound conditional invocation wire version.
pub const CONDITIONAL_INVOCATION_VERSION_V2: u16 = 2;
/// Domain for the length-prefixed complete canonical V2 content identity.
pub const CONDITIONAL_INVOCATION_DOMAIN_V2: &[u8] = b"FE2O3/CONDITIONAL-INVOCATION/V2\0";
/// Exact seven-field verifier statement domain, including terminal NUL.
pub const CONDITIONAL_MEMORY_THEOREM_DOMAIN_V2: &[u8] =
    b"FE2O3/CONDITIONAL-MEMORY-TRANSITION/V2/LE/SHARED-IEEE/CPU-SOURCE\0";
/// Same complete frame byte limit as V1, including the extra V2 digest.
pub const MAX_CONDITIONAL_INVOCATION_BYTES_V2: usize = MAX_CONDITIONAL_INVOCATION_BYTES_V1;
/// Same argument count bound as V1.
pub const MAX_CONDITIONAL_ARGUMENTS_V2: usize = MAX_CONDITIONAL_ARGUMENTS_V1;
/// Same occurrence count bound as V1.
pub const MAX_CONDITIONAL_READS_V2: usize = MAX_CONDITIONAL_READS_V1;
/// Same typed-root count bound as V1.
pub const MAX_CONDITIONAL_ROOTS_V2: usize = MAX_CONDITIONAL_ROOTS_V1;
/// Same complete premise count bound as V1.
pub const MAX_CONDITIONAL_PREMISES_V2: usize = MAX_CONDITIONAL_PREMISES_V1;

/// Caller-authored V2 theorem claims. No field authenticates its own provenance.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ConditionalTheoremV2 {
    /// Hash of the V2 domain and exactly the seven specified inputs.
    pub statement_identity: [u8; 32],
    /// Generated formula source identity, the second statement input.
    pub generated_source_identity: [u8; 32],
    /// Claimed formula execution identity; not checked as a receipt here.
    pub execution_identity: [u8; 32],
    /// Claimed formula receipt identity; not a signature validation.
    pub receipt_identity: [u8; 32],
    /// Staging receipt identity, the third statement input.
    pub staging_receipt_identity: [u8; 32],
    /// Staging effect obligation identity, the fourth statement input.
    pub staging_obligation_identity: [u8; 32],
    /// Staging signer identity, the fifth statement input.
    pub staging_signer_identity: [u8; 32],
    /// Staging execution identity, the sixth statement input.
    pub staging_execution_identity: [u8; 32],
    /// B1 full canonical CPU-frame commitment, the seventh statement input.
    /// The consuming owner must derive and compare it from the actual CPU input.
    pub cpu_input_commitment: [u8; 32],
}

/// Caller data for V2 only, with the existing unchanged row grammar.
pub struct ConditionalInvocationContractInputV2<'a> {
    /// Existing little-endian shared-IEEE numerical domain.
    pub numerical_domain: ConditionalNumericalDomainV1,
    /// Existing full projected subject tuple.
    pub subjects: ConditionalSubjectsV1,
    /// Distinct seven-field CPU-bound theorem.
    pub theorem: ConditionalTheoremV2,
    /// Complete ordered typed roots, including any repeated roots.
    pub typed_roots: &'a [[u64; 4]],
    /// Strict canonical-parameter order; other ordinal spaces are independent.
    pub arguments: &'a [ConditionalArgumentBindingV1],
    /// Sole output occurrence and its address domain.
    pub output: ConditionalOutputV1,
    /// Complete producer occurrence order; never deduplicated by parameter.
    pub reads: &'a [ConditionalReadOccurrenceV1],
    /// Exact complete ordered premise roster, without synthesized obligations.
    pub premises: &'a [ConditionalRuntimePremiseV1],
}

/// Distinct V2 content digest, not a receipt or source-origin certificate.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct ConditionalInvocationIdentityV2([u8; 32]);
impl ConditionalInvocationIdentityV2 {
    /// Wrap caller-supplied content bytes without authenticating them.
    pub const fn from_untrusted_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }
    /// Borrow the exact content digest.
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}
