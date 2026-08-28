//! Canonical evidence that a protected service reacquired one exact current Worker record.

use std::{error::Error, fmt};

use ed25519_dalek::{Signature, Signer, SigningKey, VerifyingKey};
use fe2o3_artifact_transaction::InertCompilerExecutionSubjectV1;
use sha2::{Digest, Sha256};

use crate::{CompilerExecutionIssuerPolicyV1, CompilerExecutionReceiptCarriageV1};

const MAGIC: [u8; 8] = *b"F2O3CEV1";
const ATTESTATION_MAGIC: [u8; 8] = *b"F2O3CEA1";
const VERSION: u16 = 1;
const HEADER_BYTES: usize = 24;
const SHA256_BYTES: usize = 32;
const SIGNATURE_BYTES: usize = 64;
const IDENTITY_DOMAIN: &[u8] = b"FE2O3/COMPILER-EXECUTION-CURRENT-RECORD-VERIFICATION/V1\0";
const ATTESTATION_IDENTITY_DOMAIN: &[u8] =
    b"FE2O3/COMPILER-EXECUTION-CURRENT-RECORD-ATTESTATION/V1\0";
const ATTESTATION_SIGNATURE_DOMAIN: &[u8] =
    b"FE2O3/COMPILER-EXECUTION-CURRENT-RECORD-ATTESTATION-SIGNATURE/V1\0";
const PREIMAGE_BYTES: usize = HEADER_BYTES + 9 * SHA256_BYTES + 8;

/// Exact byte length of one current-record verification result.
pub const COMPILER_EXECUTION_CURRENT_RECORD_VERIFICATION_BYTES_V1: usize =
    PREIMAGE_BYTES + SHA256_BYTES;

const ATTESTATION_SIGNED_PREFIX_BYTES: usize = HEADER_BYTES
    + SHA256_BYTES
    + COMPILER_EXECUTION_CURRENT_RECORD_VERIFICATION_BYTES_V1
    + SHA256_BYTES;
const ATTESTATION_PREIMAGE_BYTES: usize = ATTESTATION_SIGNED_PREFIX_BYTES + SIGNATURE_BYTES;

/// Exact byte length of one challenge-bound, issuer-signed current-record attestation.
pub const COMPILER_EXECUTION_CURRENT_RECORD_ATTESTATION_BYTES_V1: usize =
    ATTESTATION_PREIMAGE_BYTES + SHA256_BYTES;

/// Domain-separated identity of one exact current-record verification result.
#[derive(Clone, Copy, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CompilerExecutionCurrentRecordVerificationIdentityV1([u8; SHA256_BYTES]);

impl CompilerExecutionCurrentRecordVerificationIdentityV1 {
    pub const fn as_bytes(&self) -> &[u8; SHA256_BYTES] {
        &self.0
    }
}

impl fmt::Debug for CompilerExecutionCurrentRecordVerificationIdentityV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_tuple("CompilerExecutionCurrentRecordVerificationIdentityV1")
            .field(&self.0)
            .finish()
    }
}

/// Domain-separated identity of one exact signed current-record attestation.
#[derive(Clone, Copy, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CompilerExecutionCurrentRecordAttestationIdentityV1([u8; SHA256_BYTES]);

impl CompilerExecutionCurrentRecordAttestationIdentityV1 {
    pub const fn as_bytes(&self) -> &[u8; SHA256_BYTES] {
        &self.0
    }
}

impl fmt::Debug for CompilerExecutionCurrentRecordAttestationIdentityV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_tuple("CompilerExecutionCurrentRecordAttestationIdentityV1")
            .field(&self.0)
            .finish()
    }
}

/// Authority-free record of one protected policy comparison and Worker-ledger reacquisition.
///
/// Decoding proves canonical structure only. A caller must authenticate the service session and
/// compare every field with its exact request before treating the final two identities as
/// protected evidence.
#[derive(Clone, Eq, PartialEq)]
pub struct CompilerExecutionCurrentRecordVerificationV1 {
    fields: FieldsV1,
    identity: CompilerExecutionCurrentRecordVerificationIdentityV1,
    canonical_bytes: [u8; COMPILER_EXECUTION_CURRENT_RECORD_VERIFICATION_BYTES_V1],
}

impl CompilerExecutionCurrentRecordVerificationV1 {
    /// Constructs a descriptive record from one exact internally consistent carriage.
    ///
    /// The final two identities are meaningful only when a protected service derived them after
    /// independent policy comparison and exact durable record reacquisition. Construction alone
    /// grants no authority.
    pub fn new(
        subject: &InertCompilerExecutionSubjectV1,
        carriage: &CompilerExecutionReceiptCarriageV1,
        protected_policy_verification_identity: [u8; SHA256_BYTES],
        protected_worker_ledger_verification_identity: [u8; SHA256_BYTES],
    ) -> Result<Self, CompilerExecutionCurrentRecordVerificationErrorV1> {
        if carriage.request().subject() != subject {
            return Err(CompilerExecutionCurrentRecordVerificationErrorV1::SubjectMismatch);
        }
        Self::encode(FieldsV1 {
            policy_identity: *carriage.policy().identity().as_bytes(),
            subject_identity: *subject.identity().sha256(),
            carriage_identity: *carriage.identity().as_bytes(),
            issuer_journal_identity: carriage.acknowledgment().issuer_journal_identity(),
            worker_ledger_record_identity: carriage
                .acknowledgment()
                .worker_ledger_record_identity(),
            sequence: carriage.acknowledgment().sequence(),
            prior_rollback_anchor: carriage.publication().receipt().prior_rollback_anchor(),
            current_rollback_anchor: carriage.acknowledgment().current_rollback_anchor(),
            protected_policy_verification_identity,
            protected_worker_ledger_verification_identity,
        })
    }

    /// Strictly decodes one complete canonical result.
    pub fn decode(bytes: &[u8]) -> Result<Self, CompilerExecutionCurrentRecordVerificationErrorV1> {
        if bytes.len() != COMPILER_EXECUTION_CURRENT_RECORD_VERIFICATION_BYTES_V1 {
            return Err(
                CompilerExecutionCurrentRecordVerificationErrorV1::InvalidLength {
                    actual: bytes.len(),
                },
            );
        }
        let mut reader = Reader::new(bytes);
        if reader.fixed::<8>()? != MAGIC
            || reader.u16()? != VERSION
            || reader.u16()? != 0
            || reader.u64()? != COMPILER_EXECUTION_CURRENT_RECORD_VERIFICATION_BYTES_V1 as u64
            || reader.fixed::<4>()? != [0; 4]
        {
            return Err(CompilerExecutionCurrentRecordVerificationErrorV1::InvalidHeader);
        }
        let fields = FieldsV1 {
            policy_identity: reader.fixed()?,
            subject_identity: reader.fixed()?,
            carriage_identity: reader.fixed()?,
            issuer_journal_identity: reader.fixed()?,
            worker_ledger_record_identity: reader.fixed()?,
            sequence: reader.u64()?,
            prior_rollback_anchor: reader.fixed()?,
            current_rollback_anchor: reader.fixed()?,
            protected_policy_verification_identity: reader.fixed()?,
            protected_worker_ledger_verification_identity: reader.fixed()?,
        };
        let declared_identity = reader.fixed::<SHA256_BYTES>()?;
        if !reader.is_empty() {
            return Err(CompilerExecutionCurrentRecordVerificationErrorV1::TrailingBytes);
        }
        let decoded = Self::encode(fields)?;
        if declared_identity != decoded.identity.0 || decoded.canonical_bytes.as_slice() != bytes {
            return Err(CompilerExecutionCurrentRecordVerificationErrorV1::IdentityMismatch);
        }
        Ok(decoded)
    }

    fn encode(fields: FieldsV1) -> Result<Self, CompilerExecutionCurrentRecordVerificationErrorV1> {
        fields.validate()?;
        let mut canonical_bytes = [0_u8; COMPILER_EXECUTION_CURRENT_RECORD_VERIFICATION_BYTES_V1];
        let mut offset = 0;
        put(&mut canonical_bytes, &mut offset, &MAGIC);
        put(&mut canonical_bytes, &mut offset, &VERSION.to_le_bytes());
        put(&mut canonical_bytes, &mut offset, &0_u16.to_le_bytes());
        put(
            &mut canonical_bytes,
            &mut offset,
            &(COMPILER_EXECUTION_CURRENT_RECORD_VERIFICATION_BYTES_V1 as u64).to_le_bytes(),
        );
        put(&mut canonical_bytes, &mut offset, &0_u32.to_le_bytes());
        put(&mut canonical_bytes, &mut offset, &fields.policy_identity);
        put(&mut canonical_bytes, &mut offset, &fields.subject_identity);
        put(&mut canonical_bytes, &mut offset, &fields.carriage_identity);
        put(
            &mut canonical_bytes,
            &mut offset,
            &fields.issuer_journal_identity,
        );
        put(
            &mut canonical_bytes,
            &mut offset,
            &fields.worker_ledger_record_identity,
        );
        put(
            &mut canonical_bytes,
            &mut offset,
            &fields.sequence.to_le_bytes(),
        );
        put(
            &mut canonical_bytes,
            &mut offset,
            &fields.prior_rollback_anchor,
        );
        put(
            &mut canonical_bytes,
            &mut offset,
            &fields.current_rollback_anchor,
        );
        put(
            &mut canonical_bytes,
            &mut offset,
            &fields.protected_policy_verification_identity,
        );
        put(
            &mut canonical_bytes,
            &mut offset,
            &fields.protected_worker_ledger_verification_identity,
        );
        debug_assert_eq!(offset, PREIMAGE_BYTES);
        let identity = CompilerExecutionCurrentRecordVerificationIdentityV1(derive_identity(
            &canonical_bytes[..offset],
        ));
        put(&mut canonical_bytes, &mut offset, &identity.0);
        debug_assert_eq!(offset, canonical_bytes.len());
        Ok(Self {
            fields,
            identity,
            canonical_bytes,
        })
    }

    pub const fn policy_identity(&self) -> [u8; SHA256_BYTES] {
        self.fields.policy_identity
    }

    pub const fn subject_identity(&self) -> [u8; SHA256_BYTES] {
        self.fields.subject_identity
    }

    pub const fn carriage_identity(&self) -> [u8; SHA256_BYTES] {
        self.fields.carriage_identity
    }

    pub const fn issuer_journal_identity(&self) -> [u8; SHA256_BYTES] {
        self.fields.issuer_journal_identity
    }

    pub const fn worker_ledger_record_identity(&self) -> [u8; SHA256_BYTES] {
        self.fields.worker_ledger_record_identity
    }

    pub const fn sequence(&self) -> u64 {
        self.fields.sequence
    }

    pub const fn prior_rollback_anchor(&self) -> [u8; SHA256_BYTES] {
        self.fields.prior_rollback_anchor
    }

    pub const fn current_rollback_anchor(&self) -> [u8; SHA256_BYTES] {
        self.fields.current_rollback_anchor
    }

    pub const fn protected_policy_verification_identity(&self) -> [u8; SHA256_BYTES] {
        self.fields.protected_policy_verification_identity
    }

    pub const fn protected_worker_ledger_verification_identity(&self) -> [u8; SHA256_BYTES] {
        self.fields.protected_worker_ledger_verification_identity
    }

    pub const fn identity(&self) -> CompilerExecutionCurrentRecordVerificationIdentityV1 {
        self.identity
    }

    pub const fn canonical_bytes(
        &self,
    ) -> &[u8; COMPILER_EXECUTION_CURRENT_RECORD_VERIFICATION_BYTES_V1] {
        &self.canonical_bytes
    }

    pub const fn grants_authority(&self) -> bool {
        false
    }
}

impl fmt::Debug for CompilerExecutionCurrentRecordVerificationV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CompilerExecutionCurrentRecordVerificationV1")
            .field("policy_identity", &self.fields.policy_identity)
            .field("subject_identity", &self.fields.subject_identity)
            .field("carriage_identity", &self.fields.carriage_identity)
            .field(
                "worker_ledger_record_identity",
                &self.fields.worker_ledger_record_identity,
            )
            .field("sequence", &self.fields.sequence)
            .field("identity", &self.identity)
            .finish_non_exhaustive()
    }
}

/// Challenge-bound signature over one exact current-record verification.
///
/// Decoding authenticates the embedded signature under the embedded key. [`Self::verify`] also
/// requires that key to equal the caller-pinned policy key, the challenge to equal the caller's
/// fresh challenge, and the complete nested verification to equal the expected record. This
/// protocol result does not by itself prove protected key custody or external anti-rollback.
#[derive(Clone, Eq, PartialEq)]
pub struct CompilerExecutionCurrentRecordAttestationV1 {
    challenge: [u8; SHA256_BYTES],
    verification: CompilerExecutionCurrentRecordVerificationV1,
    verifying_key: [u8; SHA256_BYTES],
    signature: [u8; SIGNATURE_BYTES],
    identity: CompilerExecutionCurrentRecordAttestationIdentityV1,
    canonical_bytes: [u8; COMPILER_EXECUTION_CURRENT_RECORD_ATTESTATION_BYTES_V1],
}

impl CompilerExecutionCurrentRecordAttestationV1 {
    /// Signs one exact verification and caller challenge.
    ///
    /// Key protection, service admission, challenge generation, and external rollback are
    /// deliberately outside this pure protocol constructor.
    pub fn issue(
        policy: &CompilerExecutionIssuerPolicyV1,
        verification: CompilerExecutionCurrentRecordVerificationV1,
        challenge: [u8; SHA256_BYTES],
        signing_key: &SigningKey,
    ) -> Result<Self, CompilerExecutionCurrentRecordVerificationErrorV1> {
        if challenge == [0; SHA256_BYTES] {
            return Err(CompilerExecutionCurrentRecordVerificationErrorV1::ZeroChallenge);
        }
        if verification.policy_identity() != *policy.identity().as_bytes() {
            return Err(CompilerExecutionCurrentRecordVerificationErrorV1::PolicyMismatch);
        }
        if signing_key.verifying_key().as_bytes() != policy.verifying_key() {
            return Err(CompilerExecutionCurrentRecordVerificationErrorV1::SigningKeyMismatch);
        }
        let verifying_key = signing_key.verifying_key().to_bytes();
        let mut bytes = encode_attestation_prefix(challenge, &verification, verifying_key);
        let message = attestation_signature_message(&bytes[..ATTESTATION_SIGNED_PREFIX_BYTES]);
        let signature = signing_key.sign(&message).to_bytes();
        bytes[ATTESTATION_SIGNED_PREFIX_BYTES..ATTESTATION_PREIMAGE_BYTES]
            .copy_from_slice(&signature);
        finish_attestation(challenge, verification, verifying_key, signature, bytes)
    }

    /// Strictly decodes one exact canonical attestation and verifies its embedded signature.
    pub fn decode(bytes: &[u8]) -> Result<Self, CompilerExecutionCurrentRecordVerificationErrorV1> {
        if bytes.len() != COMPILER_EXECUTION_CURRENT_RECORD_ATTESTATION_BYTES_V1 {
            return Err(
                CompilerExecutionCurrentRecordVerificationErrorV1::InvalidAttestationLength {
                    actual: bytes.len(),
                },
            );
        }
        let mut reader = Reader::new(bytes);
        if reader.fixed::<8>()? != ATTESTATION_MAGIC
            || reader.u16()? != VERSION
            || reader.u16()? != 0
            || reader.u64()? != COMPILER_EXECUTION_CURRENT_RECORD_ATTESTATION_BYTES_V1 as u64
            || reader.fixed::<4>()? != [0; 4]
        {
            return Err(
                CompilerExecutionCurrentRecordVerificationErrorV1::InvalidAttestationHeader,
            );
        }
        let challenge = reader.fixed::<SHA256_BYTES>()?;
        if challenge == [0; SHA256_BYTES] {
            return Err(CompilerExecutionCurrentRecordVerificationErrorV1::ZeroChallenge);
        }
        let verification = CompilerExecutionCurrentRecordVerificationV1::decode(
            reader.take(COMPILER_EXECUTION_CURRENT_RECORD_VERIFICATION_BYTES_V1)?,
        )?;
        let verifying_key = reader.fixed::<SHA256_BYTES>()?;
        let signature = reader.fixed::<SIGNATURE_BYTES>()?;
        let declared_identity = reader.fixed::<SHA256_BYTES>()?;
        if !reader.is_empty() {
            return Err(CompilerExecutionCurrentRecordVerificationErrorV1::TrailingBytes);
        }
        let mut canonical = encode_attestation_prefix(challenge, &verification, verifying_key);
        canonical[ATTESTATION_SIGNED_PREFIX_BYTES..ATTESTATION_PREIMAGE_BYTES]
            .copy_from_slice(&signature);
        let decoded =
            finish_attestation(challenge, verification, verifying_key, signature, canonical)?;
        if decoded.identity.0 != declared_identity || decoded.canonical_bytes.as_slice() != bytes {
            return Err(CompilerExecutionCurrentRecordVerificationErrorV1::IdentityMismatch);
        }
        Ok(decoded)
    }

    /// Authenticates the pinned policy key, exact challenge, and exact nested verification.
    pub fn verify(
        self,
        policy: &CompilerExecutionIssuerPolicyV1,
        expected_verification: &CompilerExecutionCurrentRecordVerificationV1,
        expected_challenge: [u8; SHA256_BYTES],
    ) -> Result<
        VerifiedCompilerExecutionCurrentRecordV1,
        CompilerExecutionCurrentRecordVerificationErrorV1,
    > {
        verify_attestation_signature(&self)?;
        if self.verifying_key != *policy.verifying_key()
            || self.verification.policy_identity() != *policy.identity().as_bytes()
        {
            return Err(CompilerExecutionCurrentRecordVerificationErrorV1::PolicyMismatch);
        }
        if self.challenge != expected_challenge {
            return Err(CompilerExecutionCurrentRecordVerificationErrorV1::ChallengeMismatch);
        }
        if &self.verification != expected_verification {
            return Err(CompilerExecutionCurrentRecordVerificationErrorV1::VerificationMismatch);
        }
        Ok(VerifiedCompilerExecutionCurrentRecordV1 { attestation: self })
    }

    pub const fn challenge(&self) -> [u8; SHA256_BYTES] {
        self.challenge
    }

    pub const fn verification(&self) -> &CompilerExecutionCurrentRecordVerificationV1 {
        &self.verification
    }

    pub const fn verifying_key(&self) -> [u8; SHA256_BYTES] {
        self.verifying_key
    }

    pub const fn identity(&self) -> CompilerExecutionCurrentRecordAttestationIdentityV1 {
        self.identity
    }

    pub const fn canonical_bytes(
        &self,
    ) -> &[u8; COMPILER_EXECUTION_CURRENT_RECORD_ATTESTATION_BYTES_V1] {
        &self.canonical_bytes
    }

    pub const fn grants_authority(&self) -> bool {
        false
    }
}

impl fmt::Debug for CompilerExecutionCurrentRecordAttestationV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CompilerExecutionCurrentRecordAttestationV1")
            .field("challenge", &self.challenge)
            .field("verification", &self.verification.identity())
            .field("identity", &self.identity)
            .finish_non_exhaustive()
    }
}

/// Move-only proof of a pinned-key signature over an expected fresh challenge and exact record.
///
/// This authenticates the cryptographic endpoint response only. Protected key custody, durable
/// ledger admission, and external monotonic currentness remain separate production joins.
#[derive(Debug)]
pub struct VerifiedCompilerExecutionCurrentRecordV1 {
    attestation: CompilerExecutionCurrentRecordAttestationV1,
}

impl VerifiedCompilerExecutionCurrentRecordV1 {
    pub const fn attestation(&self) -> &CompilerExecutionCurrentRecordAttestationV1 {
        &self.attestation
    }

    pub const fn verification(&self) -> &CompilerExecutionCurrentRecordVerificationV1 {
        self.attestation.verification()
    }

    pub fn into_attestation(self) -> CompilerExecutionCurrentRecordAttestationV1 {
        self.attestation
    }

    pub const fn authenticates_pinned_signing_key(&self) -> bool {
        true
    }

    pub const fn authenticates_expected_challenge(&self) -> bool {
        true
    }

    pub const fn authenticates_protected_current_record(&self) -> bool {
        false
    }

    pub const fn authenticates_external_rollback_currentness(&self) -> bool {
        false
    }

    pub const fn grants_authority(&self) -> bool {
        false
    }
}

fn encode_attestation_prefix(
    challenge: [u8; SHA256_BYTES],
    verification: &CompilerExecutionCurrentRecordVerificationV1,
    verifying_key: [u8; SHA256_BYTES],
) -> [u8; COMPILER_EXECUTION_CURRENT_RECORD_ATTESTATION_BYTES_V1] {
    let mut bytes = [0_u8; COMPILER_EXECUTION_CURRENT_RECORD_ATTESTATION_BYTES_V1];
    let mut offset = 0;
    put(&mut bytes, &mut offset, &ATTESTATION_MAGIC);
    put(&mut bytes, &mut offset, &VERSION.to_le_bytes());
    put(&mut bytes, &mut offset, &0_u16.to_le_bytes());
    put(
        &mut bytes,
        &mut offset,
        &(COMPILER_EXECUTION_CURRENT_RECORD_ATTESTATION_BYTES_V1 as u64).to_le_bytes(),
    );
    put(&mut bytes, &mut offset, &0_u32.to_le_bytes());
    put(&mut bytes, &mut offset, &challenge);
    put(&mut bytes, &mut offset, verification.canonical_bytes());
    put(&mut bytes, &mut offset, &verifying_key);
    debug_assert_eq!(offset, ATTESTATION_SIGNED_PREFIX_BYTES);
    bytes
}

fn finish_attestation(
    challenge: [u8; SHA256_BYTES],
    verification: CompilerExecutionCurrentRecordVerificationV1,
    verifying_key: [u8; SHA256_BYTES],
    signature: [u8; SIGNATURE_BYTES],
    mut canonical_bytes: [u8; COMPILER_EXECUTION_CURRENT_RECORD_ATTESTATION_BYTES_V1],
) -> Result<
    CompilerExecutionCurrentRecordAttestationV1,
    CompilerExecutionCurrentRecordVerificationErrorV1,
> {
    let identity =
        CompilerExecutionCurrentRecordAttestationIdentityV1(derive_identity_with_domain(
            ATTESTATION_IDENTITY_DOMAIN,
            &canonical_bytes[..ATTESTATION_PREIMAGE_BYTES],
        ));
    canonical_bytes[ATTESTATION_PREIMAGE_BYTES..].copy_from_slice(identity.as_bytes());
    let attestation = CompilerExecutionCurrentRecordAttestationV1 {
        challenge,
        verification,
        verifying_key,
        signature,
        identity,
        canonical_bytes,
    };
    verify_attestation_signature(&attestation)?;
    Ok(attestation)
}

fn verify_attestation_signature(
    attestation: &CompilerExecutionCurrentRecordAttestationV1,
) -> Result<(), CompilerExecutionCurrentRecordVerificationErrorV1> {
    let key = VerifyingKey::from_bytes(&attestation.verifying_key)
        .map_err(|_| CompilerExecutionCurrentRecordVerificationErrorV1::InvalidVerifyingKey)?;
    if key.is_weak() {
        return Err(CompilerExecutionCurrentRecordVerificationErrorV1::WeakVerifyingKey);
    }
    let message = attestation_signature_message(
        &attestation.canonical_bytes[..ATTESTATION_SIGNED_PREFIX_BYTES],
    );
    key.verify_strict(&message, &Signature::from_bytes(&attestation.signature))
        .map_err(|_| CompilerExecutionCurrentRecordVerificationErrorV1::SignatureRejected)
}

fn attestation_signature_message(prefix: &[u8]) -> [u8; SHA256_BYTES] {
    derive_identity_with_domain(ATTESTATION_SIGNATURE_DOMAIN, prefix)
}

#[derive(Clone, Copy, Eq, PartialEq)]
struct FieldsV1 {
    policy_identity: [u8; SHA256_BYTES],
    subject_identity: [u8; SHA256_BYTES],
    carriage_identity: [u8; SHA256_BYTES],
    issuer_journal_identity: [u8; SHA256_BYTES],
    worker_ledger_record_identity: [u8; SHA256_BYTES],
    sequence: u64,
    prior_rollback_anchor: [u8; SHA256_BYTES],
    current_rollback_anchor: [u8; SHA256_BYTES],
    protected_policy_verification_identity: [u8; SHA256_BYTES],
    protected_worker_ledger_verification_identity: [u8; SHA256_BYTES],
}

impl FieldsV1 {
    fn validate(self) -> Result<(), CompilerExecutionCurrentRecordVerificationErrorV1> {
        if [
            self.policy_identity,
            self.subject_identity,
            self.carriage_identity,
            self.issuer_journal_identity,
            self.worker_ledger_record_identity,
            self.current_rollback_anchor,
            self.protected_policy_verification_identity,
            self.protected_worker_ledger_verification_identity,
        ]
        .contains(&[0; SHA256_BYTES])
        {
            return Err(CompilerExecutionCurrentRecordVerificationErrorV1::ZeroIdentity);
        }
        if self.sequence == 0
            || (self.sequence == 1) != (self.prior_rollback_anchor == [0; SHA256_BYTES])
        {
            return Err(CompilerExecutionCurrentRecordVerificationErrorV1::InvalidPosition);
        }
        Ok(())
    }
}

fn derive_identity(bytes: &[u8]) -> [u8; SHA256_BYTES] {
    derive_identity_with_domain(IDENTITY_DOMAIN, bytes)
}

fn derive_identity_with_domain(domain: &[u8], bytes: &[u8]) -> [u8; SHA256_BYTES] {
    let mut digest = Sha256::new();
    digest.update(domain);
    digest.update((bytes.len() as u64).to_le_bytes());
    digest.update(bytes);
    digest.finalize().into()
}

fn put(output: &mut [u8], offset: &mut usize, value: &[u8]) {
    let end = *offset + value.len();
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

    fn take(
        &mut self,
        length: usize,
    ) -> Result<&'a [u8], CompilerExecutionCurrentRecordVerificationErrorV1> {
        let end = self
            .offset
            .checked_add(length)
            .ok_or(CompilerExecutionCurrentRecordVerificationErrorV1::Truncated)?;
        let value = self
            .bytes
            .get(self.offset..end)
            .ok_or(CompilerExecutionCurrentRecordVerificationErrorV1::Truncated)?;
        self.offset = end;
        Ok(value)
    }

    fn fixed<const N: usize>(
        &mut self,
    ) -> Result<[u8; N], CompilerExecutionCurrentRecordVerificationErrorV1> {
        self.take(N)?
            .try_into()
            .map_err(|_| CompilerExecutionCurrentRecordVerificationErrorV1::Truncated)
    }

    fn u16(&mut self) -> Result<u16, CompilerExecutionCurrentRecordVerificationErrorV1> {
        Ok(u16::from_le_bytes(self.fixed()?))
    }

    fn u64(&mut self) -> Result<u64, CompilerExecutionCurrentRecordVerificationErrorV1> {
        Ok(u64::from_le_bytes(self.fixed()?))
    }

    const fn is_empty(&self) -> bool {
        self.offset == self.bytes.len()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CompilerExecutionCurrentRecordVerificationErrorV1 {
    InvalidLength { actual: usize },
    InvalidAttestationLength { actual: usize },
    InvalidHeader,
    InvalidAttestationHeader,
    Truncated,
    TrailingBytes,
    SubjectMismatch,
    ZeroIdentity,
    ZeroChallenge,
    InvalidPosition,
    IdentityMismatch,
    InvalidVerifyingKey,
    WeakVerifyingKey,
    SignatureRejected,
    SigningKeyMismatch,
    PolicyMismatch,
    ChallengeMismatch,
    VerificationMismatch,
}

impl fmt::Display for CompilerExecutionCurrentRecordVerificationErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidLength { actual } => {
                write!(
                    formatter,
                    "current-record verification has invalid length {actual}"
                )
            }
            Self::InvalidAttestationLength { actual } => write!(
                formatter,
                "current-record attestation has invalid length {actual}"
            ),
            Self::InvalidHeader => {
                formatter.write_str("current-record verification header is invalid")
            }
            Self::InvalidAttestationHeader => {
                formatter.write_str("current-record attestation header is invalid")
            }
            Self::Truncated => formatter.write_str("current-record verification is truncated"),
            Self::TrailingBytes => {
                formatter.write_str("current-record verification has trailing bytes")
            }
            Self::SubjectMismatch => {
                formatter.write_str("current-record verification subject differs from carriage")
            }
            Self::ZeroIdentity => {
                formatter.write_str("current-record verification contains a zero identity")
            }
            Self::ZeroChallenge => {
                formatter.write_str("current-record attestation challenge is zero")
            }
            Self::InvalidPosition => {
                formatter.write_str("current-record verification rollback position is invalid")
            }
            Self::IdentityMismatch => {
                formatter.write_str("current-record verification identity mismatch")
            }
            Self::InvalidVerifyingKey => {
                formatter.write_str("current-record attestation verifying key is invalid")
            }
            Self::WeakVerifyingKey => {
                formatter.write_str("current-record attestation verifying key is weak")
            }
            Self::SignatureRejected => {
                formatter.write_str("current-record attestation signature was rejected")
            }
            Self::SigningKeyMismatch => {
                formatter.write_str("current-record attestation signing key differs from policy")
            }
            Self::PolicyMismatch => {
                formatter.write_str("current-record attestation differs from pinned policy")
            }
            Self::ChallengeMismatch => {
                formatter.write_str("current-record attestation challenge mismatch")
            }
            Self::VerificationMismatch => {
                formatter.write_str("current-record attestation verification mismatch")
            }
        }
    }
}

impl Error for CompilerExecutionCurrentRecordVerificationErrorV1 {}
