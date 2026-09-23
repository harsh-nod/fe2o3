#![forbid(unsafe_code)]

use std::{error::Error, fmt};

use crate::attestation_receipt_codec as receipt_codec;
use crate::attestation_request_codec as request_codec;
use crate::issuer_policy_codec as policy_codec;
use ed25519_dalek::{SigningKey, VerifyingKey};
use fe2o3_artifact_transaction::{
    INERT_COMPILER_EXECUTION_SUBJECT_BYTES_V1, InertCompilerExecutionSubjectV1,
};
use sha2::{Digest, Sha256};

const SHA256_BYTES: usize = 32;
const SIGNATURE_BYTES: usize = 64;
const HEADER_BYTES: usize = 8 + 2 + 2 + 8 + 4;
const CONTENT_BINDING_BYTES: usize = SHA256_BYTES + 8;

const POLICY_IDENTITY_DOMAIN: &[u8] = b"FE2O3/COMPILER-EXECUTION-ISSUER-POLICY/V1\0";
const CHALLENGE_IDENTITY_DOMAIN: &[u8] = b"FE2O3/COMPILER-EXECUTION-CHALLENGE/V1\0";
const REQUEST_IDENTITY_DOMAIN: &[u8] = b"FE2O3/COMPILER-EXECUTION-REQUEST/V1\0";
const RECEIPT_IDENTITY_DOMAIN: &[u8] = b"FE2O3/COMPILER-EXECUTION-RECEIPT/V1\0";

/// Canonical content describing the loader-independent issuer runtime closure.
pub const SEALED_STATIC_ISSUER_RUNTIME_CLOSURE_V1: &[u8] =
    b"FE2O3/SEALED-STATIC-X86_64-ISSUER-RUNTIME-CLOSURE/V1\0";

/// Returns the sole runtime-closure measurement for a sealed-static issuer.
pub fn sealed_static_issuer_runtime_measurement_v1() -> CompilerExecutionIssuerMeasurementV1 {
    let digest = Sha256::digest(SEALED_STATIC_ISSUER_RUNTIME_CLOSURE_V1).into();
    CompilerExecutionIssuerMeasurementV1::new(
        digest,
        SEALED_STATIC_ISSUER_RUNTIME_CLOSURE_V1.len() as u64,
    )
    .expect("the fixed sealed-static runtime measurement is nonzero")
}

const POLICY_PREIMAGE_BYTES: usize = HEADER_BYTES
    + 8
    + CONTENT_BINDING_BYTES
    + CONTENT_BINDING_BYTES
    + SHA256_BYTES
    + SHA256_BYTES
    + 2
    + 6;
/// Exact canonical byte length of one compiler-execution issuer policy V1.
pub const COMPILER_EXECUTION_ISSUER_POLICY_BYTES_V1: usize = POLICY_PREIMAGE_BYTES + SHA256_BYTES;

const CHALLENGE_PREIMAGE_BYTES: usize =
    HEADER_BYTES + SHA256_BYTES + CONTENT_BINDING_BYTES + SHA256_BYTES + 8 + SHA256_BYTES;
/// Exact canonical byte length of one compiler-execution challenge V1.
pub const COMPILER_EXECUTION_ATTESTATION_CHALLENGE_BYTES_V1: usize =
    CHALLENGE_PREIMAGE_BYTES + SHA256_BYTES;

const REQUEST_PREIMAGE_BYTES: usize = HEADER_BYTES
    + COMPILER_EXECUTION_ATTESTATION_CHALLENGE_BYTES_V1
    + INERT_COMPILER_EXECUTION_SUBJECT_BYTES_V1;
/// Exact canonical byte length of one compiler-execution request V1.
pub const COMPILER_EXECUTION_ATTESTATION_REQUEST_BYTES_V1: usize =
    REQUEST_PREIMAGE_BYTES + SHA256_BYTES;

const RECEIPT_SIGNED_PREFIX_BYTES: usize = HEADER_BYTES
    + CONTENT_BINDING_BYTES
    + SHA256_BYTES
    + CONTENT_BINDING_BYTES
    + SHA256_BYTES
    + SHA256_BYTES
    + 8
    + SHA256_BYTES
    + SHA256_BYTES
    + SHA256_BYTES;
const RECEIPT_PREIMAGE_BYTES: usize = RECEIPT_SIGNED_PREFIX_BYTES + SIGNATURE_BYTES;
/// Exact canonical byte length of one signed compiler-execution receipt V1.
pub const COMPILER_EXECUTION_ATTESTATION_RECEIPT_BYTES_V1: usize =
    RECEIPT_PREIMAGE_BYTES + SHA256_BYTES;

macro_rules! identity_type {
    ($name:ident, $domain:ident, $size:ident, $preimage:ident) => {
        #[doc = concat!("Domain-separated identity of one canonical `", stringify!($name), "` record.")]
        #[derive(Clone, Copy, Eq, Hash, Ord, PartialEq, PartialOrd)]
        pub struct $name([u8; SHA256_BYTES]);

        impl $name {
            /// Returns the exact domain-separated SHA-256 identity bytes.
            pub const fn as_bytes(&self) -> &[u8; SHA256_BYTES] {
                &self.0
            }

            /// Independently rederives this identity from exact canonical bytes.
            pub fn matches_canonical_bytes(self, bytes: &[u8]) -> bool {
                bytes.len() == $size
                    && bytes[$preimage..] == self.0
                    && derive_identity($domain, &bytes[..$preimage]) == self.0
            }
        }

        impl fmt::Debug for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter
                    .debug_tuple(stringify!($name))
                    .field(&self.0)
                    .finish()
            }
        }
    };
}

identity_type!(
    CompilerExecutionIssuerPolicyIdentityV1,
    POLICY_IDENTITY_DOMAIN,
    COMPILER_EXECUTION_ISSUER_POLICY_BYTES_V1,
    POLICY_PREIMAGE_BYTES
);
identity_type!(
    CompilerExecutionAttestationChallengeIdentityV1,
    CHALLENGE_IDENTITY_DOMAIN,
    COMPILER_EXECUTION_ATTESTATION_CHALLENGE_BYTES_V1,
    CHALLENGE_PREIMAGE_BYTES
);
identity_type!(
    CompilerExecutionAttestationRequestIdentityV1,
    REQUEST_IDENTITY_DOMAIN,
    COMPILER_EXECUTION_ATTESTATION_REQUEST_BYTES_V1,
    REQUEST_PREIMAGE_BYTES
);
identity_type!(
    CompilerExecutionAttestationReceiptIdentityV1,
    RECEIPT_IDENTITY_DOMAIN,
    COMPILER_EXECUTION_ATTESTATION_RECEIPT_BYTES_V1,
    RECEIPT_PREIMAGE_BYTES
);

impl CompilerExecutionIssuerPolicyIdentityV1 {
    pub(crate) const fn from_bytes_for_protocol(bytes: [u8; SHA256_BYTES]) -> Self {
        Self(bytes)
    }
}

impl CompilerExecutionAttestationReceiptIdentityV1 {
    pub(crate) const fn from_bytes_for_protocol(bytes: [u8; SHA256_BYTES]) -> Self {
        Self(bytes)
    }
}

/// Exact content measurement used by an issuer policy.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CompilerExecutionIssuerMeasurementV1 {
    sha256: [u8; SHA256_BYTES],
    byte_len: u64,
}

impl CompilerExecutionIssuerMeasurementV1 {
    /// Constructs a nonzero bounded content measurement.
    pub fn new(
        sha256: [u8; SHA256_BYTES],
        byte_len: u64,
    ) -> Result<Self, CompilerExecutionAttestationErrorV1> {
        validate_binding(sha256, byte_len, "issuer measurement")?;
        Ok(Self { sha256, byte_len })
    }

    /// Returns the exact content digest.
    pub const fn sha256(self) -> [u8; SHA256_BYTES] {
        self.sha256
    }

    /// Returns the exact measured byte length.
    pub const fn byte_len(self) -> u64 {
        self.byte_len
    }
}

/// Caller-pinned issuer executable, runtime closure, key, and policy generation.
#[derive(Clone, Eq, PartialEq)]
pub struct CompilerExecutionIssuerPolicyV1 {
    record: policy_codec::Record,
}

impl CompilerExecutionIssuerPolicyV1 {
    /// Constructs one canonical policy from caller-pinned measurements.
    pub fn new(
        generation: u64,
        executable: CompilerExecutionIssuerMeasurementV1,
        runtime: CompilerExecutionIssuerMeasurementV1,
        verifying_key: [u8; SHA256_BYTES],
        external_anchor_verifying_key: [u8; SHA256_BYTES],
    ) -> Result<Self, CompilerExecutionAttestationErrorV1> {
        Ok(Self {
            record: policy_codec::V1.encode(policy_codec::Fields {
                generation,
                executable,
                runtime,
                verifying_key,
                external_anchor_verifying_key,
            })?,
        })
    }

    /// Strictly decodes one exact canonical policy.
    pub fn decode(bytes: &[u8]) -> Result<Self, CompilerExecutionAttestationErrorV1> {
        Ok(Self {
            record: policy_codec::V1.decode(bytes)?,
        })
    }

    /// Returns the monotonically provisioned caller policy generation.
    pub const fn generation(&self) -> u64 {
        self.record.fields.generation
    }

    /// Returns the caller-pinned protected issuer executable measurement.
    pub const fn executable(&self) -> CompilerExecutionIssuerMeasurementV1 {
        self.record.fields.executable
    }

    /// Returns the caller-pinned issuer runtime-closure measurement.
    pub const fn runtime(&self) -> CompilerExecutionIssuerMeasurementV1 {
        self.record.fields.runtime
    }

    /// Returns the caller-pinned Ed25519 verifying key.
    pub const fn verifying_key(&self) -> &[u8; SHA256_BYTES] {
        &self.record.fields.verifying_key
    }

    /// Returns the policy-pinned external monotonic-anchor verifying key.
    pub const fn external_anchor_verifying_key(&self) -> &[u8; SHA256_BYTES] {
        &self.record.fields.external_anchor_verifying_key
    }

    /// Returns the complete canonical policy identity.
    pub const fn identity(&self) -> CompilerExecutionIssuerPolicyIdentityV1 {
        CompilerExecutionIssuerPolicyIdentityV1(self.record.identity)
    }

    /// Returns the exact canonical policy bytes.
    pub const fn canonical_bytes(&self) -> &[u8; COMPILER_EXECUTION_ISSUER_POLICY_BYTES_V1] {
        &self.record.bytes
    }
}

impl fmt::Debug for CompilerExecutionIssuerPolicyV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CompilerExecutionIssuerPolicyV1")
            .field("generation", &self.generation())
            .field("executable", &self.executable())
            .field("runtime", &self.runtime())
            .field("verifying_key", self.verifying_key())
            .field(
                "external_anchor_verifying_key",
                self.external_anchor_verifying_key(),
            )
            .field("identity", &self.identity())
            .finish_non_exhaustive()
    }
}

/// Exact subject identity and length retained across the attestation protocol.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CompilerExecutionSubjectBindingV1 {
    sha256: [u8; SHA256_BYTES],
    byte_len: u64,
}

impl CompilerExecutionSubjectBindingV1 {
    fn from_subject(subject: &InertCompilerExecutionSubjectV1) -> Self {
        Self {
            sha256: *subject.identity().sha256(),
            byte_len: subject.identity().byte_len(),
        }
    }

    /// Returns the exact canonical subject identity digest.
    pub const fn sha256(self) -> [u8; SHA256_BYTES] {
        self.sha256
    }

    /// Returns the exact canonical subject byte length.
    pub const fn byte_len(self) -> u64 {
        self.byte_len
    }

    /// Checks this binding against independently rederived canonical subject bytes.
    pub fn matches_subject(self, subject: &InertCompilerExecutionSubjectV1) -> bool {
        self == Self::from_subject(subject)
            && subject
                .identity()
                .matches_canonical_bytes(subject.canonical_bytes())
    }
}

/// Issuer-generated nonce and rollback position bound to one policy and subject.
#[derive(Clone, Eq, PartialEq)]
pub struct CompilerExecutionAttestationChallengeV1 {
    policy_identity: CompilerExecutionIssuerPolicyIdentityV1,
    subject: CompilerExecutionSubjectBindingV1,
    nonce: [u8; SHA256_BYTES],
    sequence: u64,
    prior_rollback_anchor: [u8; SHA256_BYTES],
    identity: CompilerExecutionAttestationChallengeIdentityV1,
    canonical_bytes: [u8; COMPILER_EXECUTION_ATTESTATION_CHALLENGE_BYTES_V1],
}

impl CompilerExecutionAttestationChallengeV1 {
    /// Constructs a challenge from protected-issuer supplied freshness and rollback state.
    pub fn new(
        policy: &CompilerExecutionIssuerPolicyV1,
        subject: &InertCompilerExecutionSubjectV1,
        nonce: [u8; SHA256_BYTES],
        sequence: u64,
        prior_rollback_anchor: [u8; SHA256_BYTES],
    ) -> Result<Self, CompilerExecutionAttestationErrorV1> {
        require_identity(nonce, "challenge nonce")?;
        validate_rollback_position(sequence, prior_rollback_anchor)?;
        Self::from_fields(
            policy.identity(),
            CompilerExecutionSubjectBindingV1::from_subject(subject),
            nonce,
            sequence,
            prior_rollback_anchor,
        )
    }

    fn from_fields(
        policy_identity: CompilerExecutionIssuerPolicyIdentityV1,
        subject: CompilerExecutionSubjectBindingV1,
        nonce: [u8; SHA256_BYTES],
        sequence: u64,
        prior_rollback_anchor: [u8; SHA256_BYTES],
    ) -> Result<Self, CompilerExecutionAttestationErrorV1> {
        let record = request_codec::V1.challenge(request_codec::Fields {
            policy: policy_identity.0,
            subject: request_codec::Binding {
                sha256: subject.sha256,
                byte_len: subject.byte_len,
            },
            nonce,
            sequence,
            prior: prior_rollback_anchor,
        })?;
        Ok(Self {
            policy_identity,
            subject,
            nonce,
            sequence,
            prior_rollback_anchor,
            identity: CompilerExecutionAttestationChallengeIdentityV1(record.identity),
            canonical_bytes: record.bytes,
        })
    }

    /// Strictly decodes one exact canonical challenge.
    pub fn decode(bytes: &[u8]) -> Result<Self, CompilerExecutionAttestationErrorV1> {
        let record = request_codec::V1.decode_challenge(bytes)?;
        Ok(Self {
            policy_identity: CompilerExecutionIssuerPolicyIdentityV1(record.fields.policy),
            subject: CompilerExecutionSubjectBindingV1 {
                sha256: record.fields.subject.sha256,
                byte_len: record.fields.subject.byte_len,
            },
            nonce: record.fields.nonce,
            sequence: record.fields.sequence,
            prior_rollback_anchor: record.fields.prior,
            identity: CompilerExecutionAttestationChallengeIdentityV1(record.identity),
            canonical_bytes: record.bytes,
        })
    }

    /// Returns the caller-pinned issuer policy identity.
    pub const fn policy_identity(&self) -> CompilerExecutionIssuerPolicyIdentityV1 {
        self.policy_identity
    }

    /// Returns the exact compiler-execution subject binding.
    pub const fn subject(&self) -> CompilerExecutionSubjectBindingV1 {
        self.subject
    }

    /// Returns the issuer-generated challenge nonce.
    pub const fn nonce(&self) -> [u8; SHA256_BYTES] {
        self.nonce
    }

    /// Returns the issuer rollback-ledger sequence.
    pub const fn sequence(&self) -> u64 {
        self.sequence
    }

    /// Returns the caller-current rollback anchor required by this challenge.
    pub const fn prior_rollback_anchor(&self) -> [u8; SHA256_BYTES] {
        self.prior_rollback_anchor
    }

    /// Returns the complete canonical challenge identity.
    pub const fn identity(&self) -> CompilerExecutionAttestationChallengeIdentityV1 {
        self.identity
    }

    /// Returns the exact canonical challenge bytes.
    pub const fn canonical_bytes(
        &self,
    ) -> &[u8; COMPILER_EXECUTION_ATTESTATION_CHALLENGE_BYTES_V1] {
        &self.canonical_bytes
    }
}

impl fmt::Debug for CompilerExecutionAttestationChallengeV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CompilerExecutionAttestationChallengeV1")
            .field("policy_identity", &self.policy_identity)
            .field("subject", &self.subject)
            .field("sequence", &self.sequence)
            .field("identity", &self.identity)
            .finish_non_exhaustive()
    }
}

/// Exact challenge plus the complete canonical compiler-execution subject.
#[derive(Clone, Eq, PartialEq)]
pub struct CompilerExecutionAttestationRequestV1 {
    challenge: CompilerExecutionAttestationChallengeV1,
    subject: InertCompilerExecutionSubjectV1,
    identity: CompilerExecutionAttestationRequestIdentityV1,
    canonical_bytes: [u8; COMPILER_EXECUTION_ATTESTATION_REQUEST_BYTES_V1],
}

impl CompilerExecutionAttestationRequestV1 {
    /// Constructs one exact request, rejecting a challenge for any other subject.
    pub fn new(
        challenge: CompilerExecutionAttestationChallengeV1,
        subject: InertCompilerExecutionSubjectV1,
    ) -> Result<Self, CompilerExecutionAttestationErrorV1> {
        if !challenge.subject.matches_subject(&subject) {
            return Err(CompilerExecutionAttestationErrorV1::SubjectMismatch);
        }
        let (bytes, identity) =
            request_codec::V1.request(challenge.canonical_bytes(), subject.canonical_bytes());
        let identity = CompilerExecutionAttestationRequestIdentityV1(identity);
        Ok(Self {
            challenge,
            subject,
            identity,
            canonical_bytes: bytes,
        })
    }

    /// Strictly decodes one exact canonical request and its nested records.
    pub fn decode(bytes: &[u8]) -> Result<Self, CompilerExecutionAttestationErrorV1> {
        let parts = request_codec::V1.request_parts(bytes)?;
        let challenge = CompilerExecutionAttestationChallengeV1::decode(parts.challenge)?;
        let subject = InertCompilerExecutionSubjectV1::decode(parts.subject)
            .map_err(CompilerExecutionAttestationErrorV1::Subject)?;
        let declared_identity = parts.identity;
        require_identity(declared_identity, "request")?;
        let decoded = Self::new(challenge, subject)?;
        if decoded.identity.0 != declared_identity || decoded.canonical_bytes.as_slice() != bytes {
            return Err(CompilerExecutionAttestationErrorV1::IdentityMismatch(
                "request",
            ));
        }
        Ok(decoded)
    }

    /// Returns the complete issuer challenge.
    pub const fn challenge(&self) -> &CompilerExecutionAttestationChallengeV1 {
        &self.challenge
    }

    /// Returns the complete canonical compiler-execution subject.
    pub const fn subject(&self) -> &InertCompilerExecutionSubjectV1 {
        &self.subject
    }

    /// Returns the complete canonical request identity.
    pub const fn identity(&self) -> CompilerExecutionAttestationRequestIdentityV1 {
        self.identity
    }

    /// Returns the exact canonical request bytes.
    pub const fn canonical_bytes(&self) -> &[u8; COMPILER_EXECUTION_ATTESTATION_REQUEST_BYTES_V1] {
        &self.canonical_bytes
    }
}

impl fmt::Debug for CompilerExecutionAttestationRequestV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CompilerExecutionAttestationRequestV1")
            .field("challenge", &self.challenge)
            .field("subject", &self.subject)
            .field("identity", &self.identity)
            .finish_non_exhaustive()
    }
}

/// Signed response for one exact challenge, request, policy, and rollback transition.
#[derive(Clone, Eq, PartialEq)]
pub struct CompilerExecutionAttestationReceiptV1 {
    record: receipt_codec::Record,
}

impl CompilerExecutionAttestationReceiptV1 {
    /// Signs one canonical request. Key protection and process supervision are external duties.
    pub fn issue(
        policy: &CompilerExecutionIssuerPolicyV1,
        request: &CompilerExecutionAttestationRequestV1,
        signing_key: &SigningKey,
    ) -> Result<Self, CompilerExecutionAttestationErrorV1> {
        if signing_key.verifying_key().as_bytes() != policy.verifying_key() {
            return Err(CompilerExecutionAttestationErrorV1::SigningKeyMismatch);
        }
        Ok(Self {
            record: receipt_codec::V1.issue(expected_receipt(policy, request)?, signing_key)?,
        })
    }

    /// Strictly decodes and cryptographically verifies one exact canonical receipt.
    pub fn decode(bytes: &[u8]) -> Result<Self, CompilerExecutionAttestationErrorV1> {
        Ok(Self {
            record: receipt_codec::V1.decode(bytes)?,
        })
    }

    /// Verifies exact request, policy, and caller-current rollback state.
    pub fn verify(
        self,
        policy: &CompilerExecutionIssuerPolicyV1,
        request: &CompilerExecutionAttestationRequestV1,
        current_rollback_anchor: [u8; SHA256_BYTES],
    ) -> Result<VerifiedCompilerExecutionAttestationV1, CompilerExecutionAttestationErrorV1> {
        // Authenticate before expected-field construction or semantic comparison.
        receipt_codec::V1.verify_signature(&self.record)?;
        let expected = expected_receipt(policy, request)?;
        receipt_codec::compare(&self.record.fields, &expected, current_rollback_anchor)?;
        Ok(VerifiedCompilerExecutionAttestationV1 { receipt: self })
    }

    /// Returns the domain-separated identity of the exact request.
    pub const fn request_sha256(&self) -> &[u8; SHA256_BYTES] {
        &self.record.fields.request_sha256
    }

    /// Returns the caller-pinned issuer policy identity.
    pub const fn policy_identity(&self) -> CompilerExecutionIssuerPolicyIdentityV1 {
        CompilerExecutionIssuerPolicyIdentityV1(self.record.fields.policy_identity)
    }

    /// Returns the exact compiler-execution subject binding.
    pub const fn subject(&self) -> CompilerExecutionSubjectBindingV1 {
        CompilerExecutionSubjectBindingV1 {
            sha256: self.record.fields.subject.sha256,
            byte_len: self.record.fields.subject.byte_len,
        }
    }

    /// Returns the issuer challenge identity.
    pub const fn challenge_identity(&self) -> CompilerExecutionAttestationChallengeIdentityV1 {
        CompilerExecutionAttestationChallengeIdentityV1(self.record.fields.challenge_identity)
    }

    /// Returns the authenticated nonce needed to reconstruct the challenge after restart.
    pub const fn challenge_nonce(&self) -> [u8; SHA256_BYTES] {
        self.record.fields.nonce
    }

    /// Returns the issuer rollback-ledger sequence.
    pub const fn sequence(&self) -> u64 {
        self.record.fields.sequence
    }

    /// Returns the prior rollback anchor consumed by this receipt.
    pub const fn prior_rollback_anchor(&self) -> [u8; SHA256_BYTES] {
        self.record.fields.prior_rollback_anchor
    }

    /// Returns the next rollback anchor that a protected ledger must durably commit.
    pub const fn next_rollback_anchor(&self) -> [u8; SHA256_BYTES] {
        self.record.fields.next_rollback_anchor
    }

    /// Returns the complete canonical receipt identity.
    pub const fn identity(&self) -> CompilerExecutionAttestationReceiptIdentityV1 {
        CompilerExecutionAttestationReceiptIdentityV1(self.record.identity)
    }

    /// Returns the exact canonical signed receipt bytes.
    pub const fn canonical_bytes(&self) -> &[u8; COMPILER_EXECUTION_ATTESTATION_RECEIPT_BYTES_V1] {
        &self.record.bytes
    }

    /// Reports that a receipt alone grants no compiler authority.
    pub const fn grants_compiler_authority(&self) -> bool {
        false
    }

    /// Reports that a receipt alone grants no load authority.
    pub const fn grants_load_authority(&self) -> bool {
        false
    }

    /// Reports that a receipt alone grants no launch authority.
    pub const fn grants_launch_authority(&self) -> bool {
        false
    }
}

impl fmt::Debug for CompilerExecutionAttestationReceiptV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CompilerExecutionAttestationReceiptV1")
            .field("request_sha256", &self.request_sha256())
            .field("policy_identity", &self.policy_identity())
            .field("subject", &self.subject())
            .field("challenge_identity", &self.challenge_identity())
            .field("sequence", &self.sequence())
            .field("identity", &self.identity())
            .finish_non_exhaustive()
    }
}

/// Move-only proof of a valid pinned-key signature and exact rollback input.
///
/// This still does not prove protected process supervision and grants no compiler or runtime
/// authority. The protected issuer and Worker V3 authority joins must consume it with their own
/// occurrence evidence.
///
/// Replay exclusion is also external: a protected ledger must atomically compare and advance the
/// rollback anchor before a later authority-bearing type can be constructed.
///
/// ```compile_fail
/// fn duplicate(value: fe2o3_compiler_execution_protocol::VerifiedCompilerExecutionAttestationV1) {
///     let moved = value;
///     let _ = (moved, value);
/// }
/// ```
#[derive(Debug)]
pub struct VerifiedCompilerExecutionAttestationV1 {
    receipt: CompilerExecutionAttestationReceiptV1,
}

impl VerifiedCompilerExecutionAttestationV1 {
    /// Borrows the exact signed receipt.
    pub const fn receipt(&self) -> &CompilerExecutionAttestationReceiptV1 {
        &self.receipt
    }

    /// Consumes this verification result and returns its inert signed receipt.
    pub fn into_receipt(self) -> CompilerExecutionAttestationReceiptV1 {
        self.receipt
    }

    /// Reports that the receipt signature matches the caller-pinned policy key.
    pub const fn authenticates_pinned_signing_key(&self) -> bool {
        true
    }

    /// Reports that signature verification alone does not prove protected compiler execution.
    pub const fn authenticates_protected_compiler_execution(&self) -> bool {
        false
    }

    /// Reports that this protocol-level result grants no compiler authority.
    pub const fn grants_compiler_authority(&self) -> bool {
        false
    }

    /// Reports that this protocol-level result grants no load authority.
    pub const fn grants_load_authority(&self) -> bool {
        false
    }

    /// Reports that this protocol-level result grants no launch authority.
    pub const fn grants_launch_authority(&self) -> bool {
        false
    }
}

fn expected_receipt(
    policy: &CompilerExecutionIssuerPolicyV1,
    request: &CompilerExecutionAttestationRequestV1,
) -> Result<receipt_codec::Fields, CompilerExecutionAttestationErrorV1> {
    let challenge = &request.challenge;
    let subject = CompilerExecutionSubjectBindingV1::from_subject(&request.subject);
    receipt_codec::V1.expected(receipt_codec::ExpectedInput {
        policy_identity: policy.identity().0,
        verifying_key: *policy.verifying_key(),
        request_identity: *request.identity.as_bytes(),
        subject: request_codec::Binding {
            sha256: subject.sha256,
            byte_len: subject.byte_len,
        },
        challenge: &request_codec::Fields {
            policy: challenge.policy_identity.0,
            subject: request_codec::Binding {
                sha256: challenge.subject.sha256,
                byte_len: challenge.subject.byte_len,
            },
            nonce: challenge.nonce,
            sequence: challenge.sequence,
            prior: challenge.prior_rollback_anchor,
        },
        challenge_identity: challenge.identity.0,
    })
}

pub(crate) fn encode_header_version(output: &mut [u8], magic: [u8; 8], version: u16) -> usize {
    let total_len = output.len() as u64;
    let mut offset = 0;
    put(output, &mut offset, &magic);
    put(output, &mut offset, &version.to_le_bytes());
    put(output, &mut offset, &0_u16.to_le_bytes());
    put(output, &mut offset, &total_len.to_le_bytes());
    put(output, &mut offset, &0_u32.to_le_bytes());
    offset
}

pub(crate) fn decode_header_version(
    reader: &mut Reader<'_>,
    expected_magic: [u8; 8],
    expected_version: u16,
    expected_len: usize,
    field: &'static str,
) -> Result<(), CompilerExecutionAttestationErrorV1> {
    if reader.fixed::<8>()? != expected_magic {
        return Err(CompilerExecutionAttestationErrorV1::InvalidMagic(field));
    }
    let version = reader.u16()?;
    if version != expected_version {
        return Err(CompilerExecutionAttestationErrorV1::UnsupportedVersion { field, version });
    }
    let flags = reader.u16()?;
    if flags != 0 {
        return Err(CompilerExecutionAttestationErrorV1::UnsupportedFlags { field, flags });
    }
    let declared = reader.u64()?;
    if declared != expected_len as u64 {
        return Err(
            CompilerExecutionAttestationErrorV1::DeclaredLengthMismatch {
                field,
                declared,
                expected: expected_len,
            },
        );
    }
    if reader.u32()? != 0 {
        return Err(CompilerExecutionAttestationErrorV1::NonzeroReserved);
    }
    Ok(())
}

pub(crate) fn encode_measurement(
    output: &mut [u8],
    offset: &mut usize,
    value: CompilerExecutionIssuerMeasurementV1,
) {
    put(output, offset, &value.sha256);
    put(output, offset, &value.byte_len.to_le_bytes());
}

pub(crate) fn decode_measurement(
    reader: &mut Reader<'_>,
    field: &'static str,
) -> Result<CompilerExecutionIssuerMeasurementV1, CompilerExecutionAttestationErrorV1> {
    let sha256 = reader.fixed::<32>()?;
    let byte_len = reader.u64()?;
    validate_binding(sha256, byte_len, field)?;
    Ok(CompilerExecutionIssuerMeasurementV1 { sha256, byte_len })
}

pub(crate) fn validate_binding(
    sha256: [u8; SHA256_BYTES],
    byte_len: u64,
    field: &'static str,
) -> Result<(), CompilerExecutionAttestationErrorV1> {
    require_identity(sha256, field)?;
    if byte_len == 0 {
        return Err(CompilerExecutionAttestationErrorV1::ZeroValue(field));
    }
    Ok(())
}

pub(crate) fn validate_verifying_key(
    bytes: [u8; SHA256_BYTES],
) -> Result<VerifyingKey, CompilerExecutionAttestationErrorV1> {
    let key = VerifyingKey::from_bytes(&bytes)
        .map_err(|_| CompilerExecutionAttestationErrorV1::InvalidVerifyingKey)?;
    if key.is_weak() {
        return Err(CompilerExecutionAttestationErrorV1::WeakVerifyingKey);
    }
    Ok(key)
}

pub(crate) fn validate_rollback_position(
    sequence: u64,
    prior: [u8; SHA256_BYTES],
) -> Result<(), CompilerExecutionAttestationErrorV1> {
    if sequence == 0 {
        return Err(CompilerExecutionAttestationErrorV1::ZeroValue(
            "attestation sequence",
        ));
    }
    if (sequence == 1) != (prior == [0; SHA256_BYTES]) {
        return Err(CompilerExecutionAttestationErrorV1::InvalidRollbackPosition);
    }
    Ok(())
}

pub(crate) fn require_identity(
    value: [u8; SHA256_BYTES],
    field: &'static str,
) -> Result<(), CompilerExecutionAttestationErrorV1> {
    if value == [0; SHA256_BYTES] {
        Err(CompilerExecutionAttestationErrorV1::ZeroValue(field))
    } else {
        Ok(())
    }
}

pub(crate) fn require_length(
    bytes: &[u8],
    expected: usize,
    field: &'static str,
) -> Result<(), CompilerExecutionAttestationErrorV1> {
    if bytes.len() != expected {
        Err(CompilerExecutionAttestationErrorV1::InvalidLength {
            field,
            actual: bytes.len(),
            expected,
        })
    } else {
        Ok(())
    }
}

pub(crate) fn derive_identity(domain: &[u8], bytes: &[u8]) -> [u8; SHA256_BYTES] {
    let mut digest = Sha256::new();
    digest.update(domain);
    digest.update((bytes.len() as u64).to_le_bytes());
    digest.update(bytes);
    digest.finalize().into()
}

pub(crate) fn put(output: &mut [u8], offset: &mut usize, value: &[u8]) {
    let end = offset.checked_add(value.len()).expect("fixed codec offset");
    output[*offset..end].copy_from_slice(value);
    *offset = end;
}

pub(crate) struct Reader<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl<'a> Reader<'a> {
    pub(crate) const fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, offset: 0 }
    }

    fn take(&mut self, length: usize) -> Result<&'a [u8], CompilerExecutionAttestationErrorV1> {
        let end = self
            .offset
            .checked_add(length)
            .ok_or(CompilerExecutionAttestationErrorV1::Truncated)?;
        let value = self
            .bytes
            .get(self.offset..end)
            .ok_or(CompilerExecutionAttestationErrorV1::Truncated)?;
        self.offset = end;
        Ok(value)
    }

    pub(crate) fn fixed<const N: usize>(
        &mut self,
    ) -> Result<[u8; N], CompilerExecutionAttestationErrorV1> {
        self.take(N)?
            .try_into()
            .map_err(|_| CompilerExecutionAttestationErrorV1::Truncated)
    }

    pub(crate) fn u16(&mut self) -> Result<u16, CompilerExecutionAttestationErrorV1> {
        Ok(u16::from_le_bytes(self.fixed()?))
    }

    fn u32(&mut self) -> Result<u32, CompilerExecutionAttestationErrorV1> {
        Ok(u32::from_le_bytes(self.fixed()?))
    }

    pub(crate) fn u64(&mut self) -> Result<u64, CompilerExecutionAttestationErrorV1> {
        Ok(u64::from_le_bytes(self.fixed()?))
    }
}

/// Strict codec, signature, policy, or rollback validation failure.
#[derive(Debug)]
pub enum CompilerExecutionAttestationErrorV1 {
    InvalidLength {
        field: &'static str,
        actual: usize,
        expected: usize,
    },
    InvalidMagic(&'static str),
    UnsupportedVersion {
        field: &'static str,
        version: u16,
    },
    UnsupportedFlags {
        field: &'static str,
        flags: u16,
    },
    DeclaredLengthMismatch {
        field: &'static str,
        declared: u64,
        expected: usize,
    },
    NonzeroReserved,
    ZeroValue(&'static str),
    UnsupportedSubjectVersion(u16),
    SubjectLengthMismatch,
    InvalidVerifyingKey,
    WeakVerifyingKey,
    NonDistinctVerifyingKeys,
    InvalidRollbackPosition,
    Subject(fe2o3_artifact_transaction::CompilerExecutionSubjectErrorV1),
    SubjectMismatch,
    PolicyMismatch,
    ChallengeMismatch,
    RequestMismatch,
    RequestLengthMismatch,
    SequenceMismatch,
    RollbackAnchorMismatch,
    RollbackTransitionMismatch,
    SigningKeyMismatch,
    SignatureRejected,
    IdentityMismatch(&'static str),
    Truncated,
}

impl fmt::Display for CompilerExecutionAttestationErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidLength {
                field,
                actual,
                expected,
            } => write!(
                formatter,
                "{field} has {actual} bytes; expected exactly {expected}"
            ),
            Self::InvalidMagic(field) => write!(formatter, "{field} magic is invalid"),
            Self::UnsupportedVersion { field, version } => {
                write!(formatter, "{field} version {version} is unsupported")
            }
            Self::UnsupportedFlags { field, flags } => {
                write!(formatter, "{field} flags {flags:#06x} are unsupported")
            }
            Self::DeclaredLengthMismatch {
                field,
                declared,
                expected,
            } => write!(
                formatter,
                "{field} declares {declared} bytes; expected exactly {expected}"
            ),
            Self::NonzeroReserved => formatter.write_str("reserved bytes must be zero"),
            Self::ZeroValue(field) => write!(formatter, "{field} must be nonzero"),
            Self::UnsupportedSubjectVersion(version) => {
                write!(
                    formatter,
                    "compiler-execution subject version {version} is unsupported"
                )
            }
            Self::SubjectLengthMismatch => {
                formatter.write_str("compiler-execution subject length is not canonical")
            }
            Self::InvalidVerifyingKey => formatter.write_str("Ed25519 verifying key is invalid"),
            Self::WeakVerifyingKey => formatter.write_str("Ed25519 verifying key is weak"),
            Self::NonDistinctVerifyingKeys => {
                formatter.write_str("issuer and external-anchor verifying keys must be distinct")
            }
            Self::InvalidRollbackPosition => formatter
                .write_str("attestation sequence and prior rollback anchor are inconsistent"),
            Self::Subject(error) => write!(formatter, "compiler-execution subject failed: {error}"),
            Self::SubjectMismatch => formatter.write_str("compiler-execution subject mismatch"),
            Self::PolicyMismatch => formatter.write_str("issuer policy mismatch"),
            Self::ChallengeMismatch => formatter.write_str("attestation challenge mismatch"),
            Self::RequestMismatch => formatter.write_str("attestation request mismatch"),
            Self::RequestLengthMismatch => {
                formatter.write_str("attestation request length mismatch")
            }
            Self::SequenceMismatch => formatter.write_str("attestation sequence mismatch"),
            Self::RollbackAnchorMismatch => formatter.write_str("current rollback anchor mismatch"),
            Self::RollbackTransitionMismatch => formatter.write_str("rollback transition mismatch"),
            Self::SigningKeyMismatch => formatter.write_str("issuer signing key mismatch"),
            Self::SignatureRejected => formatter.write_str("issuer signature was rejected"),
            Self::IdentityMismatch(field) => write!(formatter, "{field} identity mismatch"),
            Self::Truncated => formatter.write_str("attestation wire is truncated"),
        }
    }
}

impl Error for CompilerExecutionAttestationErrorV1 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Subject(error) => Some(error),
            _ => None,
        }
    }
}
