#![forbid(unsafe_code)]

use std::{error::Error, fmt};

use ed25519_dalek::{Signature, Signer, SigningKey, VerifyingKey};
use fe2o3_artifact_transaction::{
    INERT_COMPILER_EXECUTION_SUBJECT_BYTES_V1, INERT_COMPILER_EXECUTION_SUBJECT_VERSION_V1,
    InertCompilerExecutionSubjectV1,
};
use sha2::{Digest, Sha256};

const SHA256_BYTES: usize = 32;
const SIGNATURE_BYTES: usize = 64;
const HEADER_BYTES: usize = 8 + 2 + 2 + 8 + 4;
const CONTENT_BINDING_BYTES: usize = SHA256_BYTES + 8;

const POLICY_MAGIC: [u8; 8] = *b"F2O3CEP1";
const CHALLENGE_MAGIC: [u8; 8] = *b"F2O3CEC1";
const REQUEST_MAGIC: [u8; 8] = *b"F2O3CEQ1";
const RECEIPT_MAGIC: [u8; 8] = *b"F2O3CER1";
const VERSION_V1: u16 = 1;

const POLICY_IDENTITY_DOMAIN: &[u8] = b"FE2O3/COMPILER-EXECUTION-ISSUER-POLICY/V1\0";
const CHALLENGE_IDENTITY_DOMAIN: &[u8] = b"FE2O3/COMPILER-EXECUTION-CHALLENGE/V1\0";
const REQUEST_IDENTITY_DOMAIN: &[u8] = b"FE2O3/COMPILER-EXECUTION-REQUEST/V1\0";
const RECEIPT_IDENTITY_DOMAIN: &[u8] = b"FE2O3/COMPILER-EXECUTION-RECEIPT/V1\0";
const RECEIPT_SIGNATURE_DOMAIN: &[u8] = b"FE2O3/COMPILER-EXECUTION-RECEIPT-SIGNATURE/V1\0";
const ROLLBACK_ANCHOR_DOMAIN: &[u8] = b"FE2O3/COMPILER-EXECUTION-ROLLBACK-ANCHOR/V1\0";

const POLICY_PREIMAGE_BYTES: usize =
    HEADER_BYTES + 8 + CONTENT_BINDING_BYTES + CONTENT_BINDING_BYTES + SHA256_BYTES + 2 + 6;
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
    generation: u64,
    executable: CompilerExecutionIssuerMeasurementV1,
    runtime: CompilerExecutionIssuerMeasurementV1,
    verifying_key: [u8; SHA256_BYTES],
    identity: CompilerExecutionIssuerPolicyIdentityV1,
    canonical_bytes: [u8; COMPILER_EXECUTION_ISSUER_POLICY_BYTES_V1],
}

impl CompilerExecutionIssuerPolicyV1 {
    /// Constructs one canonical policy from caller-pinned measurements.
    pub fn new(
        generation: u64,
        executable: CompilerExecutionIssuerMeasurementV1,
        runtime: CompilerExecutionIssuerMeasurementV1,
        verifying_key: [u8; SHA256_BYTES],
    ) -> Result<Self, CompilerExecutionAttestationErrorV1> {
        if generation == 0 {
            return Err(CompilerExecutionAttestationErrorV1::ZeroValue(
                "issuer policy generation",
            ));
        }
        validate_verifying_key(verifying_key)?;

        let mut bytes = [0_u8; COMPILER_EXECUTION_ISSUER_POLICY_BYTES_V1];
        let mut offset = encode_header(&mut bytes, POLICY_MAGIC);
        put(&mut bytes, &mut offset, &generation.to_le_bytes());
        encode_measurement(&mut bytes, &mut offset, executable);
        encode_measurement(&mut bytes, &mut offset, runtime);
        put(&mut bytes, &mut offset, &verifying_key);
        put(
            &mut bytes,
            &mut offset,
            &INERT_COMPILER_EXECUTION_SUBJECT_VERSION_V1.to_le_bytes(),
        );
        offset += 6;
        debug_assert_eq!(offset, POLICY_PREIMAGE_BYTES);
        let identity = CompilerExecutionIssuerPolicyIdentityV1(derive_identity(
            POLICY_IDENTITY_DOMAIN,
            &bytes[..POLICY_PREIMAGE_BYTES],
        ));
        put(&mut bytes, &mut offset, identity.as_bytes());
        debug_assert_eq!(offset, bytes.len());
        Ok(Self {
            generation,
            executable,
            runtime,
            verifying_key,
            identity,
            canonical_bytes: bytes,
        })
    }

    /// Strictly decodes one exact canonical policy.
    pub fn decode(bytes: &[u8]) -> Result<Self, CompilerExecutionAttestationErrorV1> {
        require_length(
            bytes,
            COMPILER_EXECUTION_ISSUER_POLICY_BYTES_V1,
            "issuer policy",
        )?;
        let mut reader = Reader::new(bytes);
        decode_header(&mut reader, POLICY_MAGIC, bytes.len(), "issuer policy")?;
        let generation = reader.u64()?;
        let executable = decode_measurement(&mut reader, "issuer executable")?;
        let runtime = decode_measurement(&mut reader, "issuer runtime")?;
        let verifying_key = reader.fixed::<32>()?;
        let subject_version = reader.u16()?;
        if subject_version != INERT_COMPILER_EXECUTION_SUBJECT_VERSION_V1 {
            return Err(
                CompilerExecutionAttestationErrorV1::UnsupportedSubjectVersion(subject_version),
            );
        }
        if reader.fixed::<6>()? != [0; 6] {
            return Err(CompilerExecutionAttestationErrorV1::NonzeroReserved);
        }
        let declared_identity = reader.fixed::<32>()?;
        require_identity(declared_identity, "issuer policy")?;
        let decoded = Self::new(generation, executable, runtime, verifying_key)?;
        if decoded.identity.0 != declared_identity || decoded.canonical_bytes.as_slice() != bytes {
            return Err(CompilerExecutionAttestationErrorV1::IdentityMismatch(
                "issuer policy",
            ));
        }
        Ok(decoded)
    }

    /// Returns the monotonically provisioned caller policy generation.
    pub const fn generation(&self) -> u64 {
        self.generation
    }

    /// Returns the caller-pinned protected issuer executable measurement.
    pub const fn executable(&self) -> CompilerExecutionIssuerMeasurementV1 {
        self.executable
    }

    /// Returns the caller-pinned issuer runtime-closure measurement.
    pub const fn runtime(&self) -> CompilerExecutionIssuerMeasurementV1 {
        self.runtime
    }

    /// Returns the caller-pinned Ed25519 verifying key.
    pub const fn verifying_key(&self) -> &[u8; SHA256_BYTES] {
        &self.verifying_key
    }

    /// Returns the complete canonical policy identity.
    pub const fn identity(&self) -> CompilerExecutionIssuerPolicyIdentityV1 {
        self.identity
    }

    /// Returns the exact canonical policy bytes.
    pub const fn canonical_bytes(&self) -> &[u8; COMPILER_EXECUTION_ISSUER_POLICY_BYTES_V1] {
        &self.canonical_bytes
    }
}

impl fmt::Debug for CompilerExecutionIssuerPolicyV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CompilerExecutionIssuerPolicyV1")
            .field("generation", &self.generation)
            .field("executable", &self.executable)
            .field("runtime", &self.runtime)
            .field("verifying_key", &self.verifying_key)
            .field("identity", &self.identity)
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

    fn new(
        sha256: [u8; SHA256_BYTES],
        byte_len: u64,
    ) -> Result<Self, CompilerExecutionAttestationErrorV1> {
        validate_binding(sha256, byte_len, "compiler-execution subject")?;
        if byte_len != INERT_COMPILER_EXECUTION_SUBJECT_BYTES_V1 as u64 {
            return Err(CompilerExecutionAttestationErrorV1::SubjectLengthMismatch);
        }
        Ok(Self { sha256, byte_len })
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
        require_identity(policy_identity.0, "issuer policy")?;
        require_identity(nonce, "challenge nonce")?;
        validate_rollback_position(sequence, prior_rollback_anchor)?;
        let mut bytes = [0_u8; COMPILER_EXECUTION_ATTESTATION_CHALLENGE_BYTES_V1];
        let mut offset = encode_header(&mut bytes, CHALLENGE_MAGIC);
        put(&mut bytes, &mut offset, policy_identity.as_bytes());
        encode_subject_binding(&mut bytes, &mut offset, subject);
        put(&mut bytes, &mut offset, &nonce);
        put(&mut bytes, &mut offset, &sequence.to_le_bytes());
        put(&mut bytes, &mut offset, &prior_rollback_anchor);
        debug_assert_eq!(offset, CHALLENGE_PREIMAGE_BYTES);
        let identity = CompilerExecutionAttestationChallengeIdentityV1(derive_identity(
            CHALLENGE_IDENTITY_DOMAIN,
            &bytes[..CHALLENGE_PREIMAGE_BYTES],
        ));
        put(&mut bytes, &mut offset, identity.as_bytes());
        Ok(Self {
            policy_identity,
            subject,
            nonce,
            sequence,
            prior_rollback_anchor,
            identity,
            canonical_bytes: bytes,
        })
    }

    /// Strictly decodes one exact canonical challenge.
    pub fn decode(bytes: &[u8]) -> Result<Self, CompilerExecutionAttestationErrorV1> {
        require_length(
            bytes,
            COMPILER_EXECUTION_ATTESTATION_CHALLENGE_BYTES_V1,
            "challenge",
        )?;
        let mut reader = Reader::new(bytes);
        decode_header(&mut reader, CHALLENGE_MAGIC, bytes.len(), "challenge")?;
        let policy_identity = CompilerExecutionIssuerPolicyIdentityV1(reader.fixed::<32>()?);
        let subject = decode_subject_binding(&mut reader)?;
        let nonce = reader.fixed::<32>()?;
        let sequence = reader.u64()?;
        let prior_rollback_anchor = reader.fixed::<32>()?;
        let declared_identity = reader.fixed::<32>()?;
        require_identity(declared_identity, "challenge")?;
        let decoded = Self::from_fields(
            policy_identity,
            subject,
            nonce,
            sequence,
            prior_rollback_anchor,
        )?;
        if decoded.identity.0 != declared_identity || decoded.canonical_bytes.as_slice() != bytes {
            return Err(CompilerExecutionAttestationErrorV1::IdentityMismatch(
                "challenge",
            ));
        }
        Ok(decoded)
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
        let mut bytes = [0_u8; COMPILER_EXECUTION_ATTESTATION_REQUEST_BYTES_V1];
        let mut offset = encode_header(&mut bytes, REQUEST_MAGIC);
        put(&mut bytes, &mut offset, challenge.canonical_bytes());
        put(&mut bytes, &mut offset, subject.canonical_bytes());
        debug_assert_eq!(offset, REQUEST_PREIMAGE_BYTES);
        let identity = CompilerExecutionAttestationRequestIdentityV1(derive_identity(
            REQUEST_IDENTITY_DOMAIN,
            &bytes[..REQUEST_PREIMAGE_BYTES],
        ));
        put(&mut bytes, &mut offset, identity.as_bytes());
        Ok(Self {
            challenge,
            subject,
            identity,
            canonical_bytes: bytes,
        })
    }

    /// Strictly decodes one exact canonical request and its nested records.
    pub fn decode(bytes: &[u8]) -> Result<Self, CompilerExecutionAttestationErrorV1> {
        require_length(
            bytes,
            COMPILER_EXECUTION_ATTESTATION_REQUEST_BYTES_V1,
            "request",
        )?;
        let mut reader = Reader::new(bytes);
        decode_header(&mut reader, REQUEST_MAGIC, bytes.len(), "request")?;
        let challenge = CompilerExecutionAttestationChallengeV1::decode(
            reader.take(COMPILER_EXECUTION_ATTESTATION_CHALLENGE_BYTES_V1)?,
        )?;
        let subject = InertCompilerExecutionSubjectV1::decode(
            reader.take(INERT_COMPILER_EXECUTION_SUBJECT_BYTES_V1)?,
        )
        .map_err(CompilerExecutionAttestationErrorV1::Subject)?;
        let declared_identity = reader.fixed::<32>()?;
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
    request_sha256: [u8; SHA256_BYTES],
    request_byte_len: u64,
    policy_identity: CompilerExecutionIssuerPolicyIdentityV1,
    subject: CompilerExecutionSubjectBindingV1,
    challenge_identity: CompilerExecutionAttestationChallengeIdentityV1,
    nonce: [u8; SHA256_BYTES],
    sequence: u64,
    prior_rollback_anchor: [u8; SHA256_BYTES],
    next_rollback_anchor: [u8; SHA256_BYTES],
    verifying_key: [u8; SHA256_BYTES],
    signature: [u8; SIGNATURE_BYTES],
    identity: CompilerExecutionAttestationReceiptIdentityV1,
    canonical_bytes: [u8; COMPILER_EXECUTION_ATTESTATION_RECEIPT_BYTES_V1],
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
        if request.challenge.policy_identity != policy.identity {
            return Err(CompilerExecutionAttestationErrorV1::PolicyMismatch);
        }
        let fields = ReceiptFieldsV1::from_request(policy, request)?;
        let mut bytes = encode_receipt_prefix(&fields);
        let message = receipt_signature_message(&bytes[..RECEIPT_SIGNED_PREFIX_BYTES]);
        let signature = signing_key.sign(&message).to_bytes();
        bytes[RECEIPT_SIGNED_PREFIX_BYTES..RECEIPT_PREIMAGE_BYTES].copy_from_slice(&signature);
        finish_receipt(fields, signature, bytes)
    }

    /// Strictly decodes and cryptographically verifies one exact canonical receipt.
    pub fn decode(bytes: &[u8]) -> Result<Self, CompilerExecutionAttestationErrorV1> {
        require_length(
            bytes,
            COMPILER_EXECUTION_ATTESTATION_RECEIPT_BYTES_V1,
            "receipt",
        )?;
        let mut reader = Reader::new(bytes);
        decode_header(&mut reader, RECEIPT_MAGIC, bytes.len(), "receipt")?;
        let request_sha256 = reader.fixed::<32>()?;
        let request_byte_len = reader.u64()?;
        validate_binding(request_sha256, request_byte_len, "attestation request")?;
        if request_byte_len != COMPILER_EXECUTION_ATTESTATION_REQUEST_BYTES_V1 as u64 {
            return Err(CompilerExecutionAttestationErrorV1::RequestLengthMismatch);
        }
        let policy_identity = CompilerExecutionIssuerPolicyIdentityV1(reader.fixed::<32>()?);
        let subject = decode_subject_binding(&mut reader)?;
        let challenge_identity =
            CompilerExecutionAttestationChallengeIdentityV1(reader.fixed::<32>()?);
        let nonce = reader.fixed::<32>()?;
        let sequence = reader.u64()?;
        let prior_rollback_anchor = reader.fixed::<32>()?;
        let next_rollback_anchor = reader.fixed::<32>()?;
        let verifying_key = reader.fixed::<32>()?;
        let signature = reader.fixed::<64>()?;
        let declared_identity = reader.fixed::<32>()?;
        let fields = ReceiptFieldsV1 {
            request_sha256,
            request_byte_len,
            policy_identity,
            subject,
            challenge_identity,
            nonce,
            sequence,
            prior_rollback_anchor,
            next_rollback_anchor,
            verifying_key,
        };
        let mut canonical = encode_receipt_prefix(&fields);
        canonical[RECEIPT_SIGNED_PREFIX_BYTES..RECEIPT_PREIMAGE_BYTES].copy_from_slice(&signature);
        let decoded = finish_receipt(fields, signature, canonical)?;
        if decoded.identity.0 != declared_identity || decoded.canonical_bytes.as_slice() != bytes {
            return Err(CompilerExecutionAttestationErrorV1::IdentityMismatch(
                "receipt",
            ));
        }
        Ok(decoded)
    }

    /// Verifies exact request, policy, and caller-current rollback state.
    pub fn verify(
        self,
        policy: &CompilerExecutionIssuerPolicyV1,
        request: &CompilerExecutionAttestationRequestV1,
        current_rollback_anchor: [u8; SHA256_BYTES],
    ) -> Result<VerifiedCompilerExecutionAttestationV1, CompilerExecutionAttestationErrorV1> {
        // Authenticate every field before using any of them for semantic comparison.
        verify_receipt_signature(&self)?;
        let expected = ReceiptFieldsV1::from_request(policy, request)?;
        if self.policy_identity != expected.policy_identity
            || self.verifying_key != expected.verifying_key
        {
            return Err(CompilerExecutionAttestationErrorV1::PolicyMismatch);
        }
        if self.subject != expected.subject {
            return Err(CompilerExecutionAttestationErrorV1::SubjectMismatch);
        }
        if self.sequence != expected.sequence {
            return Err(CompilerExecutionAttestationErrorV1::SequenceMismatch);
        }
        if self.challenge_identity != expected.challenge_identity || self.nonce != expected.nonce {
            return Err(CompilerExecutionAttestationErrorV1::ChallengeMismatch);
        }
        if self.prior_rollback_anchor != current_rollback_anchor
            || self.prior_rollback_anchor != expected.prior_rollback_anchor
        {
            return Err(CompilerExecutionAttestationErrorV1::RollbackAnchorMismatch);
        }
        if self.next_rollback_anchor != expected.next_rollback_anchor {
            return Err(CompilerExecutionAttestationErrorV1::RollbackTransitionMismatch);
        }
        if self.request_sha256 != expected.request_sha256
            || self.request_byte_len != expected.request_byte_len
        {
            return Err(CompilerExecutionAttestationErrorV1::RequestMismatch);
        }
        Ok(VerifiedCompilerExecutionAttestationV1 { receipt: self })
    }

    /// Returns the domain-separated identity of the exact request.
    pub const fn request_sha256(&self) -> &[u8; SHA256_BYTES] {
        &self.request_sha256
    }

    /// Returns the caller-pinned issuer policy identity.
    pub const fn policy_identity(&self) -> CompilerExecutionIssuerPolicyIdentityV1 {
        self.policy_identity
    }

    /// Returns the exact compiler-execution subject binding.
    pub const fn subject(&self) -> CompilerExecutionSubjectBindingV1 {
        self.subject
    }

    /// Returns the issuer challenge identity.
    pub const fn challenge_identity(&self) -> CompilerExecutionAttestationChallengeIdentityV1 {
        self.challenge_identity
    }

    /// Returns the issuer nonce needed to reconstruct the exact challenge after an issued-state
    /// restart. The signed receipt authenticates this value; exposing it grants no authority.
    pub const fn challenge_nonce(&self) -> [u8; SHA256_BYTES] {
        self.nonce
    }

    /// Returns the issuer rollback-ledger sequence.
    pub const fn sequence(&self) -> u64 {
        self.sequence
    }

    /// Returns the prior rollback anchor consumed by this receipt.
    pub const fn prior_rollback_anchor(&self) -> [u8; SHA256_BYTES] {
        self.prior_rollback_anchor
    }

    /// Returns the next rollback anchor that a protected ledger must durably commit.
    pub const fn next_rollback_anchor(&self) -> [u8; SHA256_BYTES] {
        self.next_rollback_anchor
    }

    /// Returns the complete canonical receipt identity.
    pub const fn identity(&self) -> CompilerExecutionAttestationReceiptIdentityV1 {
        self.identity
    }

    /// Returns the exact canonical signed receipt bytes.
    pub const fn canonical_bytes(&self) -> &[u8; COMPILER_EXECUTION_ATTESTATION_RECEIPT_BYTES_V1] {
        &self.canonical_bytes
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
            .field("request_sha256", &self.request_sha256)
            .field("policy_identity", &self.policy_identity)
            .field("subject", &self.subject)
            .field("challenge_identity", &self.challenge_identity)
            .field("sequence", &self.sequence)
            .field("identity", &self.identity)
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
/// fn duplicate(value: fe2o3_runtime_protocol::VerifiedCompilerExecutionAttestationV1) {
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

#[derive(Clone, Copy)]
struct ReceiptFieldsV1 {
    request_sha256: [u8; SHA256_BYTES],
    request_byte_len: u64,
    policy_identity: CompilerExecutionIssuerPolicyIdentityV1,
    subject: CompilerExecutionSubjectBindingV1,
    challenge_identity: CompilerExecutionAttestationChallengeIdentityV1,
    nonce: [u8; SHA256_BYTES],
    sequence: u64,
    prior_rollback_anchor: [u8; SHA256_BYTES],
    next_rollback_anchor: [u8; SHA256_BYTES],
    verifying_key: [u8; SHA256_BYTES],
}

impl ReceiptFieldsV1 {
    fn from_request(
        policy: &CompilerExecutionIssuerPolicyV1,
        request: &CompilerExecutionAttestationRequestV1,
    ) -> Result<Self, CompilerExecutionAttestationErrorV1> {
        if request.challenge.policy_identity != policy.identity {
            return Err(CompilerExecutionAttestationErrorV1::PolicyMismatch);
        }
        let request_sha256 = *request.identity.as_bytes();
        let subject = CompilerExecutionSubjectBindingV1::from_subject(&request.subject);
        let challenge = &request.challenge;
        let next_rollback_anchor = derive_next_rollback_anchor(
            challenge.sequence,
            challenge.prior_rollback_anchor,
            request_sha256,
            subject,
            challenge.nonce,
            policy.identity.0,
        );
        require_identity(next_rollback_anchor, "next rollback anchor")?;
        Ok(Self {
            request_sha256,
            request_byte_len: COMPILER_EXECUTION_ATTESTATION_REQUEST_BYTES_V1 as u64,
            policy_identity: policy.identity,
            subject,
            challenge_identity: challenge.identity,
            nonce: challenge.nonce,
            sequence: challenge.sequence,
            prior_rollback_anchor: challenge.prior_rollback_anchor,
            next_rollback_anchor,
            verifying_key: policy.verifying_key,
        })
    }
}

fn encode_receipt_prefix(
    fields: &ReceiptFieldsV1,
) -> [u8; COMPILER_EXECUTION_ATTESTATION_RECEIPT_BYTES_V1] {
    let mut bytes = [0_u8; COMPILER_EXECUTION_ATTESTATION_RECEIPT_BYTES_V1];
    let mut offset = encode_header(&mut bytes, RECEIPT_MAGIC);
    put(&mut bytes, &mut offset, &fields.request_sha256);
    put(
        &mut bytes,
        &mut offset,
        &fields.request_byte_len.to_le_bytes(),
    );
    put(&mut bytes, &mut offset, fields.policy_identity.as_bytes());
    encode_subject_binding(&mut bytes, &mut offset, fields.subject);
    put(
        &mut bytes,
        &mut offset,
        fields.challenge_identity.as_bytes(),
    );
    put(&mut bytes, &mut offset, &fields.nonce);
    put(&mut bytes, &mut offset, &fields.sequence.to_le_bytes());
    put(&mut bytes, &mut offset, &fields.prior_rollback_anchor);
    put(&mut bytes, &mut offset, &fields.next_rollback_anchor);
    put(&mut bytes, &mut offset, &fields.verifying_key);
    debug_assert_eq!(offset, RECEIPT_SIGNED_PREFIX_BYTES);
    bytes
}

fn finish_receipt(
    fields: ReceiptFieldsV1,
    signature: [u8; SIGNATURE_BYTES],
    mut bytes: [u8; COMPILER_EXECUTION_ATTESTATION_RECEIPT_BYTES_V1],
) -> Result<CompilerExecutionAttestationReceiptV1, CompilerExecutionAttestationErrorV1> {
    validate_verifying_key(fields.verifying_key)?;
    validate_rollback_position(fields.sequence, fields.prior_rollback_anchor)?;
    let expected_next = derive_next_rollback_anchor(
        fields.sequence,
        fields.prior_rollback_anchor,
        fields.request_sha256,
        fields.subject,
        fields.nonce,
        fields.policy_identity.0,
    );
    if fields.next_rollback_anchor != expected_next {
        return Err(CompilerExecutionAttestationErrorV1::RollbackTransitionMismatch);
    }
    let identity = CompilerExecutionAttestationReceiptIdentityV1(derive_identity(
        RECEIPT_IDENTITY_DOMAIN,
        &bytes[..RECEIPT_PREIMAGE_BYTES],
    ));
    bytes[RECEIPT_PREIMAGE_BYTES..].copy_from_slice(identity.as_bytes());
    let receipt = CompilerExecutionAttestationReceiptV1 {
        request_sha256: fields.request_sha256,
        request_byte_len: fields.request_byte_len,
        policy_identity: fields.policy_identity,
        subject: fields.subject,
        challenge_identity: fields.challenge_identity,
        nonce: fields.nonce,
        sequence: fields.sequence,
        prior_rollback_anchor: fields.prior_rollback_anchor,
        next_rollback_anchor: fields.next_rollback_anchor,
        verifying_key: fields.verifying_key,
        signature,
        identity,
        canonical_bytes: bytes,
    };
    verify_receipt_signature(&receipt)?;
    Ok(receipt)
}

fn verify_receipt_signature(
    receipt: &CompilerExecutionAttestationReceiptV1,
) -> Result<(), CompilerExecutionAttestationErrorV1> {
    let key = validate_verifying_key(receipt.verifying_key)?;
    let message =
        receipt_signature_message(&receipt.canonical_bytes[..RECEIPT_SIGNED_PREFIX_BYTES]);
    key.verify_strict(&message, &Signature::from_bytes(&receipt.signature))
        .map_err(|_| CompilerExecutionAttestationErrorV1::SignatureRejected)
}

fn derive_next_rollback_anchor(
    sequence: u64,
    prior: [u8; SHA256_BYTES],
    request: [u8; SHA256_BYTES],
    subject: CompilerExecutionSubjectBindingV1,
    nonce: [u8; SHA256_BYTES],
    policy: [u8; SHA256_BYTES],
) -> [u8; SHA256_BYTES] {
    let mut digest = Sha256::new();
    digest.update(ROLLBACK_ANCHOR_DOMAIN);
    digest.update(sequence.to_le_bytes());
    digest.update(prior);
    digest.update(request);
    digest.update(subject.sha256);
    digest.update(subject.byte_len.to_le_bytes());
    digest.update(nonce);
    digest.update(policy);
    digest.finalize().into()
}

fn receipt_signature_message(prefix: &[u8]) -> [u8; SHA256_BYTES] {
    derive_identity(RECEIPT_SIGNATURE_DOMAIN, prefix)
}

fn encode_header(output: &mut [u8], magic: [u8; 8]) -> usize {
    let total_len = output.len() as u64;
    let mut offset = 0;
    put(output, &mut offset, &magic);
    put(output, &mut offset, &VERSION_V1.to_le_bytes());
    put(output, &mut offset, &0_u16.to_le_bytes());
    put(output, &mut offset, &total_len.to_le_bytes());
    put(output, &mut offset, &0_u32.to_le_bytes());
    offset
}

fn decode_header(
    reader: &mut Reader<'_>,
    expected_magic: [u8; 8],
    expected_len: usize,
    field: &'static str,
) -> Result<(), CompilerExecutionAttestationErrorV1> {
    if reader.fixed::<8>()? != expected_magic {
        return Err(CompilerExecutionAttestationErrorV1::InvalidMagic(field));
    }
    let version = reader.u16()?;
    if version != VERSION_V1 {
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

fn encode_measurement(
    output: &mut [u8],
    offset: &mut usize,
    value: CompilerExecutionIssuerMeasurementV1,
) {
    put(output, offset, &value.sha256);
    put(output, offset, &value.byte_len.to_le_bytes());
}

fn decode_measurement(
    reader: &mut Reader<'_>,
    field: &'static str,
) -> Result<CompilerExecutionIssuerMeasurementV1, CompilerExecutionAttestationErrorV1> {
    let sha256 = reader.fixed::<32>()?;
    let byte_len = reader.u64()?;
    validate_binding(sha256, byte_len, field)?;
    Ok(CompilerExecutionIssuerMeasurementV1 { sha256, byte_len })
}

fn encode_subject_binding(
    output: &mut [u8],
    offset: &mut usize,
    value: CompilerExecutionSubjectBindingV1,
) {
    put(output, offset, &value.sha256);
    put(output, offset, &value.byte_len.to_le_bytes());
}

fn decode_subject_binding(
    reader: &mut Reader<'_>,
) -> Result<CompilerExecutionSubjectBindingV1, CompilerExecutionAttestationErrorV1> {
    CompilerExecutionSubjectBindingV1::new(reader.fixed::<32>()?, reader.u64()?)
}

fn validate_binding(
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

fn validate_verifying_key(
    bytes: [u8; SHA256_BYTES],
) -> Result<VerifyingKey, CompilerExecutionAttestationErrorV1> {
    let key = VerifyingKey::from_bytes(&bytes)
        .map_err(|_| CompilerExecutionAttestationErrorV1::InvalidVerifyingKey)?;
    if key.is_weak() {
        return Err(CompilerExecutionAttestationErrorV1::WeakVerifyingKey);
    }
    Ok(key)
}

fn validate_rollback_position(
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

fn require_identity(
    value: [u8; SHA256_BYTES],
    field: &'static str,
) -> Result<(), CompilerExecutionAttestationErrorV1> {
    if value == [0; SHA256_BYTES] {
        Err(CompilerExecutionAttestationErrorV1::ZeroValue(field))
    } else {
        Ok(())
    }
}

fn require_length(
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

fn derive_identity(domain: &[u8], bytes: &[u8]) -> [u8; SHA256_BYTES] {
    let mut digest = Sha256::new();
    digest.update(domain);
    digest.update((bytes.len() as u64).to_le_bytes());
    digest.update(bytes);
    digest.finalize().into()
}

fn put(output: &mut [u8], offset: &mut usize, value: &[u8]) {
    let end = offset.checked_add(value.len()).expect("fixed codec offset");
    output[*offset..end].copy_from_slice(value);
    *offset = end;
}

struct Reader<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl<'a> Reader<'a> {
    const fn new(bytes: &'a [u8]) -> Self {
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

    fn fixed<const N: usize>(&mut self) -> Result<[u8; N], CompilerExecutionAttestationErrorV1> {
        self.take(N)?
            .try_into()
            .map_err(|_| CompilerExecutionAttestationErrorV1::Truncated)
    }

    fn u16(&mut self) -> Result<u16, CompilerExecutionAttestationErrorV1> {
        Ok(u16::from_le_bytes(self.fixed()?))
    }

    fn u32(&mut self) -> Result<u32, CompilerExecutionAttestationErrorV1> {
        Ok(u32::from_le_bytes(self.fixed()?))
    }

    fn u64(&mut self) -> Result<u64, CompilerExecutionAttestationErrorV1> {
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
