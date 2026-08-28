//! Canonical evidence that a protected service reacquired one exact current Worker record.

use std::{error::Error, fmt};

use ed25519_dalek::{Signature, Signer, SigningKey, VerifyingKey};
use fe2o3_external_anchor_protocol::{
    ANCHOR_TRANSITION_RECEIPT_BYTES_V1, AnchorPositionV1, AnchorProtocolErrorV1,
    AnchorTransitionReceiptV1, ChallengeKindV1, HashChainHeadV1, PinnedAnchorKeyV1,
};
use sha2::{Digest, Sha256};

use crate::{
    CompilerExecutionExternalAnchorTransactionErrorV1,
    CompilerExecutionExternalAnchorTransactionV1, CompilerExecutionIssuerPolicyV1,
    CompilerExecutionReceiptCarriageV1,
};

const MAGIC: [u8; 8] = *b"F2O3CEV2";
const ATTESTATION_MAGIC: [u8; 8] = *b"F2O3CEA2";
const VERSION: u16 = 2;
const HEADER_BYTES: usize = 24;
const SHA256_BYTES: usize = 32;
const SIGNATURE_BYTES: usize = 64;
const IDENTITY_DOMAIN: &[u8] = b"FE2O3/COMPILER-EXECUTION-CURRENT-RECORD-VERIFICATION/V2\0";
const ATTESTATION_IDENTITY_DOMAIN: &[u8] =
    b"FE2O3/COMPILER-EXECUTION-CURRENT-RECORD-ATTESTATION/V2\0";
const ATTESTATION_SIGNATURE_DOMAIN: &[u8] =
    b"FE2O3/COMPILER-EXECUTION-CURRENT-RECORD-ATTESTATION-SIGNATURE/V2\0";
const PREIMAGE_BYTES: usize =
    HEADER_BYTES + 10 * SHA256_BYTES + 8 + ANCHOR_TRANSITION_RECEIPT_BYTES_V1;

/// Exact byte length of one current-record verification result.
pub const COMPILER_EXECUTION_CURRENT_RECORD_VERIFICATION_BYTES_V2: usize =
    PREIMAGE_BYTES + SHA256_BYTES;

const ATTESTATION_SIGNED_PREFIX_BYTES: usize = HEADER_BYTES
    + SHA256_BYTES
    + COMPILER_EXECUTION_CURRENT_RECORD_VERIFICATION_BYTES_V2
    + SHA256_BYTES;
const ATTESTATION_PREIMAGE_BYTES: usize = ATTESTATION_SIGNED_PREFIX_BYTES + SIGNATURE_BYTES;

/// Exact byte length of one challenge-bound, issuer-signed current-record attestation.
pub const COMPILER_EXECUTION_CURRENT_RECORD_ATTESTATION_BYTES_V2: usize =
    ATTESTATION_PREIMAGE_BYTES + SHA256_BYTES;

/// Domain-separated identity of one exact current-record verification result.
#[derive(Clone, Copy, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CompilerExecutionCurrentRecordVerificationIdentityV2([u8; SHA256_BYTES]);

impl CompilerExecutionCurrentRecordVerificationIdentityV2 {
    pub const fn as_bytes(&self) -> &[u8; SHA256_BYTES] {
        &self.0
    }
}

impl fmt::Debug for CompilerExecutionCurrentRecordVerificationIdentityV2 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_tuple("CompilerExecutionCurrentRecordVerificationIdentityV2")
            .field(&self.0)
            .finish()
    }
}

/// Domain-separated identity of one exact signed current-record attestation.
#[derive(Clone, Copy, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CompilerExecutionCurrentRecordAttestationIdentityV2([u8; SHA256_BYTES]);

impl CompilerExecutionCurrentRecordAttestationIdentityV2 {
    pub const fn as_bytes(&self) -> &[u8; SHA256_BYTES] {
        &self.0
    }
}

impl fmt::Debug for CompilerExecutionCurrentRecordAttestationIdentityV2 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_tuple("CompilerExecutionCurrentRecordAttestationIdentityV2")
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
pub struct CompilerExecutionCurrentRecordVerificationV2 {
    fields: FieldsV2,
    identity: CompilerExecutionCurrentRecordVerificationIdentityV2,
    canonical_bytes: [u8; COMPILER_EXECUTION_CURRENT_RECORD_VERIFICATION_BYTES_V2],
}

impl CompilerExecutionCurrentRecordVerificationV2 {
    /// Constructs a descriptive record from one exact internally consistent carriage.
    ///
    /// The final two identities are meaningful only when a protected service derived them after
    /// independent policy comparison and exact durable record reacquisition. Construction alone
    /// grants no authority.
    pub fn new(
        carriage: &CompilerExecutionReceiptCarriageV1,
        external_anchor_receipt: AnchorTransitionReceiptV1,
        protected_policy_verification_identity: [u8; SHA256_BYTES],
        protected_worker_ledger_verification_identity: [u8; SHA256_BYTES],
    ) -> Result<Self, CompilerExecutionCurrentRecordVerificationErrorV2> {
        validate_external_anchor_receipt(carriage, &external_anchor_receipt)?;
        Self::encode(FieldsV2 {
            policy_identity: *carriage.policy().identity().as_bytes(),
            subject_identity: *carriage.request().subject().identity().sha256(),
            carriage_identity: *carriage.identity().as_bytes(),
            issuer_journal_identity: carriage.acknowledgment().issuer_journal_identity(),
            worker_ledger_record_identity: carriage
                .acknowledgment()
                .worker_ledger_record_identity(),
            sequence: carriage.acknowledgment().sequence(),
            prior_rollback_anchor: carriage.publication().receipt().prior_rollback_anchor(),
            current_rollback_anchor: carriage.acknowledgment().current_rollback_anchor(),
            external_anchor_verifying_key: *carriage.policy().external_anchor_verifying_key(),
            external_anchor_receipt,
            protected_policy_verification_identity,
            protected_worker_ledger_verification_identity,
        })
    }

    /// Strictly decodes one complete canonical result.
    pub fn decode(bytes: &[u8]) -> Result<Self, CompilerExecutionCurrentRecordVerificationErrorV2> {
        if bytes.len() != COMPILER_EXECUTION_CURRENT_RECORD_VERIFICATION_BYTES_V2 {
            return Err(
                CompilerExecutionCurrentRecordVerificationErrorV2::InvalidLength {
                    actual: bytes.len(),
                },
            );
        }
        let mut reader = Reader::new(bytes);
        if reader.fixed::<8>()? != MAGIC
            || reader.u16()? != VERSION
            || reader.u16()? != 0
            || reader.u64()? != COMPILER_EXECUTION_CURRENT_RECORD_VERIFICATION_BYTES_V2 as u64
            || reader.fixed::<4>()? != [0; 4]
        {
            return Err(CompilerExecutionCurrentRecordVerificationErrorV2::InvalidHeader);
        }
        let policy_identity = reader.fixed()?;
        let subject_identity = reader.fixed()?;
        let carriage_identity = reader.fixed()?;
        let issuer_journal_identity = reader.fixed()?;
        let worker_ledger_record_identity = reader.fixed()?;
        let sequence = reader.u64()?;
        let prior_rollback_anchor = reader.fixed()?;
        let current_rollback_anchor = reader.fixed()?;
        let external_anchor_verifying_key = reader.fixed()?;
        let key = PinnedAnchorKeyV1::from_bytes(external_anchor_verifying_key)?;
        let external_anchor_receipt = AnchorTransitionReceiptV1::decode(
            reader.take(ANCHOR_TRANSITION_RECEIPT_BYTES_V1)?,
            &key,
        )?;
        let fields = FieldsV2 {
            policy_identity,
            subject_identity,
            carriage_identity,
            issuer_journal_identity,
            worker_ledger_record_identity,
            sequence,
            prior_rollback_anchor,
            current_rollback_anchor,
            external_anchor_verifying_key,
            external_anchor_receipt,
            protected_policy_verification_identity: reader.fixed()?,
            protected_worker_ledger_verification_identity: reader.fixed()?,
        };
        let declared_identity = reader.fixed::<SHA256_BYTES>()?;
        if !reader.is_empty() {
            return Err(CompilerExecutionCurrentRecordVerificationErrorV2::TrailingBytes);
        }
        let decoded = Self::encode(fields)?;
        if declared_identity != decoded.identity.0 || decoded.canonical_bytes.as_slice() != bytes {
            return Err(CompilerExecutionCurrentRecordVerificationErrorV2::IdentityMismatch);
        }
        Ok(decoded)
    }

    fn encode(fields: FieldsV2) -> Result<Self, CompilerExecutionCurrentRecordVerificationErrorV2> {
        fields.validate()?;
        let mut canonical_bytes = [0_u8; COMPILER_EXECUTION_CURRENT_RECORD_VERIFICATION_BYTES_V2];
        let mut offset = 0;
        put(&mut canonical_bytes, &mut offset, &MAGIC);
        put(&mut canonical_bytes, &mut offset, &VERSION.to_le_bytes());
        put(&mut canonical_bytes, &mut offset, &0_u16.to_le_bytes());
        put(
            &mut canonical_bytes,
            &mut offset,
            &(COMPILER_EXECUTION_CURRENT_RECORD_VERIFICATION_BYTES_V2 as u64).to_le_bytes(),
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
            &fields.external_anchor_verifying_key,
        );
        put(
            &mut canonical_bytes,
            &mut offset,
            fields.external_anchor_receipt.canonical_bytes(),
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
        let identity = CompilerExecutionCurrentRecordVerificationIdentityV2(derive_identity(
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

    fn verify_expected_carriage(
        &self,
        policy: &CompilerExecutionIssuerPolicyV1,
        carriage: &CompilerExecutionReceiptCarriageV1,
    ) -> Result<(), CompilerExecutionCurrentRecordVerificationErrorV2> {
        if carriage.policy() != policy
            || self.policy_identity() != *policy.identity().as_bytes()
            || self.subject_identity() != *carriage.request().subject().identity().sha256()
            || self.carriage_identity() != *carriage.identity().as_bytes()
            || self.issuer_journal_identity() != carriage.acknowledgment().issuer_journal_identity()
            || self.worker_ledger_record_identity()
                != carriage.acknowledgment().worker_ledger_record_identity()
            || self.sequence() != carriage.acknowledgment().sequence()
            || self.prior_rollback_anchor()
                != carriage.publication().receipt().prior_rollback_anchor()
            || self.current_rollback_anchor() != carriage.acknowledgment().current_rollback_anchor()
            || self.external_anchor_verifying_key() != *policy.external_anchor_verifying_key()
        {
            return Err(CompilerExecutionCurrentRecordVerificationErrorV2::VerificationMismatch);
        }
        validate_external_anchor_receipt(carriage, self.external_anchor_receipt())
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

    pub const fn external_anchor_verifying_key(&self) -> [u8; SHA256_BYTES] {
        self.fields.external_anchor_verifying_key
    }

    pub const fn external_anchor_receipt(&self) -> &AnchorTransitionReceiptV1 {
        &self.fields.external_anchor_receipt
    }

    pub const fn external_rollback_verification_identity(&self) -> [u8; SHA256_BYTES] {
        *self.fields.external_anchor_receipt.identity().as_bytes()
    }

    pub const fn protected_policy_verification_identity(&self) -> [u8; SHA256_BYTES] {
        self.fields.protected_policy_verification_identity
    }

    pub const fn protected_worker_ledger_verification_identity(&self) -> [u8; SHA256_BYTES] {
        self.fields.protected_worker_ledger_verification_identity
    }

    pub const fn identity(&self) -> CompilerExecutionCurrentRecordVerificationIdentityV2 {
        self.identity
    }

    pub const fn canonical_bytes(
        &self,
    ) -> &[u8; COMPILER_EXECUTION_CURRENT_RECORD_VERIFICATION_BYTES_V2] {
        &self.canonical_bytes
    }

    pub const fn grants_authority(&self) -> bool {
        false
    }
}

impl fmt::Debug for CompilerExecutionCurrentRecordVerificationV2 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CompilerExecutionCurrentRecordVerificationV2")
            .field("policy_identity", &self.fields.policy_identity)
            .field("subject_identity", &self.fields.subject_identity)
            .field("carriage_identity", &self.fields.carriage_identity)
            .field(
                "worker_ledger_record_identity",
                &self.fields.worker_ledger_record_identity,
            )
            .field("sequence", &self.fields.sequence)
            .field(
                "external_anchor_receipt",
                &self.fields.external_anchor_receipt.identity(),
            )
            .field("identity", &self.identity)
            .finish_non_exhaustive()
    }
}

/// Challenge-bound signature over one exact current-record verification.
///
/// Decoding authenticates the embedded signature under the embedded key. [`Self::verify`] also
/// requires that key to equal the caller-pinned policy key, the challenge to equal the caller's
/// fresh challenge, and the complete nested verification to match the caller's expected carriage
/// and its signed external-anchor transition.
#[derive(Clone, Eq, PartialEq)]
pub struct CompilerExecutionCurrentRecordAttestationV2 {
    challenge: [u8; SHA256_BYTES],
    verification: CompilerExecutionCurrentRecordVerificationV2,
    verifying_key: [u8; SHA256_BYTES],
    signature: [u8; SIGNATURE_BYTES],
    identity: CompilerExecutionCurrentRecordAttestationIdentityV2,
    canonical_bytes: [u8; COMPILER_EXECUTION_CURRENT_RECORD_ATTESTATION_BYTES_V2],
}

impl CompilerExecutionCurrentRecordAttestationV2 {
    /// Signs one exact verification and caller challenge.
    ///
    /// Key protection, service admission, challenge generation, and a fresh external-currentness
    /// query remain outside this pure protocol constructor.
    pub fn issue(
        policy: &CompilerExecutionIssuerPolicyV1,
        expected_carriage: &CompilerExecutionReceiptCarriageV1,
        verification: CompilerExecutionCurrentRecordVerificationV2,
        challenge: [u8; SHA256_BYTES],
        signing_key: &SigningKey,
    ) -> Result<Self, CompilerExecutionCurrentRecordVerificationErrorV2> {
        if challenge == [0; SHA256_BYTES] {
            return Err(CompilerExecutionCurrentRecordVerificationErrorV2::ZeroChallenge);
        }
        if signing_key.verifying_key().as_bytes() != policy.verifying_key() {
            return Err(CompilerExecutionCurrentRecordVerificationErrorV2::SigningKeyMismatch);
        }
        verification.verify_expected_carriage(policy, expected_carriage)?;
        let verifying_key = signing_key.verifying_key().to_bytes();
        let mut bytes = encode_attestation_prefix(challenge, &verification, verifying_key);
        let message = attestation_signature_message(&bytes[..ATTESTATION_SIGNED_PREFIX_BYTES]);
        let signature = signing_key.sign(&message).to_bytes();
        bytes[ATTESTATION_SIGNED_PREFIX_BYTES..ATTESTATION_PREIMAGE_BYTES]
            .copy_from_slice(&signature);
        finish_attestation(challenge, verification, verifying_key, signature, bytes)
    }

    /// Strictly decodes one exact canonical attestation and verifies its embedded signature.
    pub fn decode(bytes: &[u8]) -> Result<Self, CompilerExecutionCurrentRecordVerificationErrorV2> {
        if bytes.len() != COMPILER_EXECUTION_CURRENT_RECORD_ATTESTATION_BYTES_V2 {
            return Err(
                CompilerExecutionCurrentRecordVerificationErrorV2::InvalidAttestationLength {
                    actual: bytes.len(),
                },
            );
        }
        let mut reader = Reader::new(bytes);
        if reader.fixed::<8>()? != ATTESTATION_MAGIC
            || reader.u16()? != VERSION
            || reader.u16()? != 0
            || reader.u64()? != COMPILER_EXECUTION_CURRENT_RECORD_ATTESTATION_BYTES_V2 as u64
            || reader.fixed::<4>()? != [0; 4]
        {
            return Err(
                CompilerExecutionCurrentRecordVerificationErrorV2::InvalidAttestationHeader,
            );
        }
        let challenge = reader.fixed::<SHA256_BYTES>()?;
        if challenge == [0; SHA256_BYTES] {
            return Err(CompilerExecutionCurrentRecordVerificationErrorV2::ZeroChallenge);
        }
        let verification = CompilerExecutionCurrentRecordVerificationV2::decode(
            reader.take(COMPILER_EXECUTION_CURRENT_RECORD_VERIFICATION_BYTES_V2)?,
        )?;
        let verifying_key = reader.fixed::<SHA256_BYTES>()?;
        let signature = reader.fixed::<SIGNATURE_BYTES>()?;
        let declared_identity = reader.fixed::<SHA256_BYTES>()?;
        if !reader.is_empty() {
            return Err(CompilerExecutionCurrentRecordVerificationErrorV2::TrailingBytes);
        }
        let mut canonical = encode_attestation_prefix(challenge, &verification, verifying_key);
        canonical[ATTESTATION_SIGNED_PREFIX_BYTES..ATTESTATION_PREIMAGE_BYTES]
            .copy_from_slice(&signature);
        let decoded =
            finish_attestation(challenge, verification, verifying_key, signature, canonical)?;
        if decoded.identity.0 != declared_identity || decoded.canonical_bytes.as_slice() != bytes {
            return Err(CompilerExecutionCurrentRecordVerificationErrorV2::IdentityMismatch);
        }
        Ok(decoded)
    }

    /// Authenticates both pinned keys, the exact challenge, carriage, and anchor transition.
    pub fn verify(
        self,
        policy: &CompilerExecutionIssuerPolicyV1,
        expected_carriage: &CompilerExecutionReceiptCarriageV1,
        expected_challenge: [u8; SHA256_BYTES],
    ) -> Result<
        VerifiedCompilerExecutionCurrentRecordV2,
        CompilerExecutionCurrentRecordVerificationErrorV2,
    > {
        verify_attestation_signature(&self)?;
        if self.verifying_key != *policy.verifying_key()
            || self.verification.policy_identity() != *policy.identity().as_bytes()
        {
            return Err(CompilerExecutionCurrentRecordVerificationErrorV2::PolicyMismatch);
        }
        if self.challenge != expected_challenge {
            return Err(CompilerExecutionCurrentRecordVerificationErrorV2::ChallengeMismatch);
        }
        self.verification
            .verify_expected_carriage(policy, expected_carriage)?;
        Ok(VerifiedCompilerExecutionCurrentRecordV2 { attestation: self })
    }

    pub const fn challenge(&self) -> [u8; SHA256_BYTES] {
        self.challenge
    }

    pub const fn verification(&self) -> &CompilerExecutionCurrentRecordVerificationV2 {
        &self.verification
    }

    pub const fn verifying_key(&self) -> [u8; SHA256_BYTES] {
        self.verifying_key
    }

    pub const fn identity(&self) -> CompilerExecutionCurrentRecordAttestationIdentityV2 {
        self.identity
    }

    pub const fn canonical_bytes(
        &self,
    ) -> &[u8; COMPILER_EXECUTION_CURRENT_RECORD_ATTESTATION_BYTES_V2] {
        &self.canonical_bytes
    }

    pub const fn grants_authority(&self) -> bool {
        false
    }
}

impl fmt::Debug for CompilerExecutionCurrentRecordAttestationV2 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CompilerExecutionCurrentRecordAttestationV2")
            .field("challenge", &self.challenge)
            .field("verification", &self.verification.identity())
            .field("identity", &self.identity)
            .finish_non_exhaustive()
    }
}

/// Move-only proof of a pinned-key signature over an expected fresh challenge and exact record.
///
/// This authenticates the cryptographic endpoint response and the exact signed external-anchor
/// commit carried by the current Worker record. Protected key custody and a fresh query proving
/// that the committed external head is still current remain separate production joins.
#[derive(Debug)]
pub struct VerifiedCompilerExecutionCurrentRecordV2 {
    attestation: CompilerExecutionCurrentRecordAttestationV2,
}

impl VerifiedCompilerExecutionCurrentRecordV2 {
    pub const fn attestation(&self) -> &CompilerExecutionCurrentRecordAttestationV2 {
        &self.attestation
    }

    pub const fn verification(&self) -> &CompilerExecutionCurrentRecordVerificationV2 {
        self.attestation.verification()
    }

    pub fn into_attestation(self) -> CompilerExecutionCurrentRecordAttestationV2 {
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

    pub const fn authenticates_external_anchor_commit(&self) -> bool {
        true
    }

    pub const fn external_rollback_verification_identity(&self) -> [u8; SHA256_BYTES] {
        self.verification()
            .external_rollback_verification_identity()
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
    verification: &CompilerExecutionCurrentRecordVerificationV2,
    verifying_key: [u8; SHA256_BYTES],
) -> [u8; COMPILER_EXECUTION_CURRENT_RECORD_ATTESTATION_BYTES_V2] {
    let mut bytes = [0_u8; COMPILER_EXECUTION_CURRENT_RECORD_ATTESTATION_BYTES_V2];
    let mut offset = 0;
    put(&mut bytes, &mut offset, &ATTESTATION_MAGIC);
    put(&mut bytes, &mut offset, &VERSION.to_le_bytes());
    put(&mut bytes, &mut offset, &0_u16.to_le_bytes());
    put(
        &mut bytes,
        &mut offset,
        &(COMPILER_EXECUTION_CURRENT_RECORD_ATTESTATION_BYTES_V2 as u64).to_le_bytes(),
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
    verification: CompilerExecutionCurrentRecordVerificationV2,
    verifying_key: [u8; SHA256_BYTES],
    signature: [u8; SIGNATURE_BYTES],
    mut canonical_bytes: [u8; COMPILER_EXECUTION_CURRENT_RECORD_ATTESTATION_BYTES_V2],
) -> Result<
    CompilerExecutionCurrentRecordAttestationV2,
    CompilerExecutionCurrentRecordVerificationErrorV2,
> {
    let identity =
        CompilerExecutionCurrentRecordAttestationIdentityV2(derive_identity_with_domain(
            ATTESTATION_IDENTITY_DOMAIN,
            &canonical_bytes[..ATTESTATION_PREIMAGE_BYTES],
        ));
    canonical_bytes[ATTESTATION_PREIMAGE_BYTES..].copy_from_slice(identity.as_bytes());
    let attestation = CompilerExecutionCurrentRecordAttestationV2 {
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
    attestation: &CompilerExecutionCurrentRecordAttestationV2,
) -> Result<(), CompilerExecutionCurrentRecordVerificationErrorV2> {
    let key = VerifyingKey::from_bytes(&attestation.verifying_key)
        .map_err(|_| CompilerExecutionCurrentRecordVerificationErrorV2::InvalidVerifyingKey)?;
    if key.is_weak() {
        return Err(CompilerExecutionCurrentRecordVerificationErrorV2::WeakVerifyingKey);
    }
    let message = attestation_signature_message(
        &attestation.canonical_bytes[..ATTESTATION_SIGNED_PREFIX_BYTES],
    );
    key.verify_strict(&message, &Signature::from_bytes(&attestation.signature))
        .map_err(|_| CompilerExecutionCurrentRecordVerificationErrorV2::SignatureRejected)
}

fn attestation_signature_message(prefix: &[u8]) -> [u8; SHA256_BYTES] {
    derive_identity_with_domain(ATTESTATION_SIGNATURE_DOMAIN, prefix)
}

#[derive(Clone, Eq, PartialEq)]
struct FieldsV2 {
    policy_identity: [u8; SHA256_BYTES],
    subject_identity: [u8; SHA256_BYTES],
    carriage_identity: [u8; SHA256_BYTES],
    issuer_journal_identity: [u8; SHA256_BYTES],
    worker_ledger_record_identity: [u8; SHA256_BYTES],
    sequence: u64,
    prior_rollback_anchor: [u8; SHA256_BYTES],
    current_rollback_anchor: [u8; SHA256_BYTES],
    external_anchor_verifying_key: [u8; SHA256_BYTES],
    external_anchor_receipt: AnchorTransitionReceiptV1,
    protected_policy_verification_identity: [u8; SHA256_BYTES],
    protected_worker_ledger_verification_identity: [u8; SHA256_BYTES],
}

impl FieldsV2 {
    fn validate(&self) -> Result<(), CompilerExecutionCurrentRecordVerificationErrorV2> {
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
            return Err(CompilerExecutionCurrentRecordVerificationErrorV2::ZeroIdentity);
        }
        if self.sequence == 0
            || (self.sequence == 1) != (self.prior_rollback_anchor == [0; SHA256_BYTES])
        {
            return Err(CompilerExecutionCurrentRecordVerificationErrorV2::InvalidPosition);
        }
        reverify_external_anchor_receipt(
            self.external_anchor_verifying_key,
            self.sequence,
            &self.external_anchor_receipt,
        )?;
        Ok(())
    }
}

fn validate_external_anchor_receipt(
    carriage: &CompilerExecutionReceiptCarriageV1,
    receipt: &AnchorTransitionReceiptV1,
) -> Result<(), CompilerExecutionCurrentRecordVerificationErrorV2> {
    let sequence = carriage.acknowledgment().sequence();
    reverify_external_anchor_receipt(
        *carriage.policy().external_anchor_verifying_key(),
        sequence,
        receipt,
    )?;
    let transaction = CompilerExecutionExternalAnchorTransactionV1::new(
        carriage.policy().clone(),
        carriage.request().clone(),
        carriage.publication().clone(),
    )?;
    if receipt.challenge().transaction() != transaction.external_anchor_digest() {
        return Err(
            CompilerExecutionCurrentRecordVerificationErrorV2::ExternalAnchorReceiptMismatch,
        );
    }
    Ok(())
}

fn reverify_external_anchor_receipt(
    external_anchor_verifying_key: [u8; SHA256_BYTES],
    sequence: u64,
    receipt: &AnchorTransitionReceiptV1,
) -> Result<(), CompilerExecutionCurrentRecordVerificationErrorV2> {
    let key = PinnedAnchorKeyV1::from_bytes(external_anchor_verifying_key)?;
    let reverified = AnchorTransitionReceiptV1::decode(receipt.canonical_bytes(), &key)?;
    let challenge = receipt.challenge();
    if reverified != *receipt
        || receipt.position() != AnchorPositionV1::Proposed
        || challenge.kind() != ChallengeKindV1::Advance
        || challenge.expected_sequence() != sequence
        || challenge.anchor_key_identity() != key.identity()
        || ((sequence == 1)
            != (challenge.prior_head() == HashChainHeadV1::from_bytes([0; SHA256_BYTES])))
    {
        return Err(
            CompilerExecutionCurrentRecordVerificationErrorV2::ExternalAnchorReceiptMismatch,
        );
    }
    Ok(())
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
    ) -> Result<&'a [u8], CompilerExecutionCurrentRecordVerificationErrorV2> {
        let end = self
            .offset
            .checked_add(length)
            .ok_or(CompilerExecutionCurrentRecordVerificationErrorV2::Truncated)?;
        let value = self
            .bytes
            .get(self.offset..end)
            .ok_or(CompilerExecutionCurrentRecordVerificationErrorV2::Truncated)?;
        self.offset = end;
        Ok(value)
    }

    fn fixed<const N: usize>(
        &mut self,
    ) -> Result<[u8; N], CompilerExecutionCurrentRecordVerificationErrorV2> {
        self.take(N)?
            .try_into()
            .map_err(|_| CompilerExecutionCurrentRecordVerificationErrorV2::Truncated)
    }

    fn u16(&mut self) -> Result<u16, CompilerExecutionCurrentRecordVerificationErrorV2> {
        Ok(u16::from_le_bytes(self.fixed()?))
    }

    fn u64(&mut self) -> Result<u64, CompilerExecutionCurrentRecordVerificationErrorV2> {
        Ok(u64::from_le_bytes(self.fixed()?))
    }

    const fn is_empty(&self) -> bool {
        self.offset == self.bytes.len()
    }
}

#[derive(Debug)]
#[non_exhaustive]
pub enum CompilerExecutionCurrentRecordVerificationErrorV2 {
    InvalidLength { actual: usize },
    InvalidAttestationLength { actual: usize },
    InvalidHeader,
    InvalidAttestationHeader,
    Truncated,
    TrailingBytes,
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
    ExternalAnchorReceiptMismatch,
    ExternalAnchor(AnchorProtocolErrorV1),
    ExternalAnchorTransaction(CompilerExecutionExternalAnchorTransactionErrorV1),
}

impl fmt::Display for CompilerExecutionCurrentRecordVerificationErrorV2 {
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
            Self::ExternalAnchorReceiptMismatch => formatter.write_str(
                "current-record external-anchor receipt differs from the exact carriage",
            ),
            Self::ExternalAnchor(error) => {
                write!(formatter, "current-record external-anchor receipt: {error}")
            }
            Self::ExternalAnchorTransaction(error) => {
                write!(
                    formatter,
                    "current-record external-anchor transaction: {error}"
                )
            }
        }
    }
}

impl Error for CompilerExecutionCurrentRecordVerificationErrorV2 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::ExternalAnchor(error) => Some(error),
            Self::ExternalAnchorTransaction(error) => Some(error),
            _ => None,
        }
    }
}

impl From<AnchorProtocolErrorV1> for CompilerExecutionCurrentRecordVerificationErrorV2 {
    fn from(error: AnchorProtocolErrorV1) -> Self {
        Self::ExternalAnchor(error)
    }
}

impl From<CompilerExecutionExternalAnchorTransactionErrorV1>
    for CompilerExecutionCurrentRecordVerificationErrorV2
{
    fn from(error: CompilerExecutionExternalAnchorTransactionErrorV1) -> Self {
        Self::ExternalAnchorTransaction(error)
    }
}
