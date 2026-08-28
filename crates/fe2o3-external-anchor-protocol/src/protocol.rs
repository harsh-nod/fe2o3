use core::fmt;

use ed25519_dalek::{Signature, VerifyingKey};
use sha2::{Digest, Sha256};

pub const EXTERNAL_ANCHOR_PROTOCOL_VERSION_V1: u16 = 1;
pub const EXTERNAL_ANCHOR_AUTHORITY_V1: &str = "none";
pub const ANCHOR_CHALLENGE_WIRE_LEN_V1: usize = 184;
pub const ANCHOR_OBSERVATION_SIGNED_LEN_V1: usize = 224;
pub const ANCHOR_OBSERVATION_WIRE_LEN_V1: usize = 288;
/// Maximum caller-canonical byte length admitted by [`derive_transaction_digest_v1`].
pub const TRANSACTION_IDENTITY_MAX_LEN_V1: usize = 4096;

const CHALLENGE_MAGIC: [u8; 8] = *b"F2ARBA1\0";
const OBSERVATION_MAGIC: [u8; 8] = *b"F2ARBO1\0";
const SIGNING_DOMAIN: &[u8] = b"FE2O3/EXTERNAL-MONOTONIC-ANCHOR/OBSERVATION/V1\0";
const KEY_ID_DOMAIN: &[u8] = b"FE2O3/EXTERNAL-MONOTONIC-ANCHOR/KEY-ID/V1\0";
const HEAD_DOMAIN: &[u8] = b"FE2O3/EXTERNAL-MONOTONIC-ANCHOR/HASH-CHAIN-HEAD/V1\0";
const TRANSACTION_DIGEST_DOMAIN: &[u8] = b"FE2O3/EXTERNAL-MONOTONIC-ANCHOR/TRANSACTION-DIGEST/V1\0";

const VERSION_OFFSET: usize = 8;
const KIND_OFFSET: usize = 10;
const POSITION_OFFSET: usize = 11;
const RESERVED_OFFSET: usize = 12;
const NONCE_OFFSET: usize = 16;
const EXPECTED_SEQUENCE_OFFSET: usize = 48;
const PRIOR_HEAD_OFFSET: usize = 56;
const TRANSACTION_OFFSET: usize = 88;
const PROPOSED_HEAD_OFFSET: usize = 120;
const KEY_ID_OFFSET: usize = 152;
const OBSERVED_SEQUENCE_OFFSET: usize = 184;
const OBSERVED_HEAD_OFFSET: usize = 192;
const SIGNATURE_OFFSET: usize = ANCHOR_OBSERVATION_SIGNED_LEN_V1;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum ChallengeKindV1 {
    Advance = 1,
    Recover = 2,
}

impl ChallengeKindV1 {
    fn decode(value: u8) -> Result<Self, AnchorProtocolErrorV1> {
        match value {
            1 => Ok(Self::Advance),
            2 => Ok(Self::Recover),
            actual => Err(AnchorProtocolErrorV1::UnknownChallengeKind { actual }),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum AnchorPositionV1 {
    Prior = 1,
    Proposed = 2,
}

impl AnchorPositionV1 {
    fn decode(value: u8) -> Result<Self, AnchorProtocolErrorV1> {
        match value {
            1 => Ok(Self::Prior),
            2 => Ok(Self::Proposed),
            actual => Err(AnchorProtocolErrorV1::UnknownAnchorPosition { actual }),
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct CallerNonceV1([u8; 32]);

impl CallerNonceV1 {
    /// Wraps caller-supplied bytes. The caller remains responsible for cryptographic freshness.
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HashChainHeadV1([u8; 32]);

impl HashChainHeadV1 {
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub const fn to_bytes(self) -> [u8; 32] {
        self.0
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TransactionDigestV1([u8; 32]);

impl TransactionDigestV1 {
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub const fn to_bytes(self) -> [u8; 32] {
        self.0
    }
}

/// Derives a V1 transaction digest from bounded caller-canonical identity bytes.
///
/// The preimage is the exact concatenation of the NUL-terminated V1 domain, the
/// little-endian protocol version, the little-endian `u32` byte length, and
/// `canonical_identity`. The caller owns the canonical schema and must exclude
/// unstable or process-local values such as paths, raw file descriptors, and
/// pointers. This function establishes no provenance or publication authority.
pub fn derive_transaction_digest_v1(
    canonical_identity: &[u8],
) -> Result<TransactionDigestV1, AnchorProtocolErrorV1> {
    if canonical_identity.is_empty() || canonical_identity.len() > TRANSACTION_IDENTITY_MAX_LEN_V1 {
        return Err(AnchorProtocolErrorV1::InvalidTransactionIdentityLength {
            actual: canonical_identity.len(),
            maximum: TRANSACTION_IDENTITY_MAX_LEN_V1,
        });
    }
    let length = u32::try_from(canonical_identity.len())
        .expect("the V1 transaction identity bound fits in u32");
    Ok(TransactionDigestV1(sha256_parts(&[
        TRANSACTION_DIGEST_DOMAIN,
        &EXTERNAL_ANCHOR_PROTOCOL_VERSION_V1.to_le_bytes(),
        &length.to_le_bytes(),
        canonical_identity,
    ])))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AnchorKeyIdentityV1([u8; 32]);

impl AnchorKeyIdentityV1 {
    pub const fn to_bytes(self) -> [u8; 32] {
        self.0
    }
}

#[derive(Debug)]
pub struct PinnedAnchorKeyV1 {
    key: VerifyingKey,
    identity: AnchorKeyIdentityV1,
}

impl PinnedAnchorKeyV1 {
    /// Admits one exact caller-pinned Ed25519 public-key value.
    pub fn from_bytes(bytes: [u8; 32]) -> Result<Self, AnchorProtocolErrorV1> {
        let key = VerifyingKey::from_bytes(&bytes)
            .map_err(|_| AnchorProtocolErrorV1::InvalidVerifyingKey)?;
        if key.is_weak() {
            return Err(AnchorProtocolErrorV1::WeakVerifyingKey);
        }
        let identity = AnchorKeyIdentityV1(sha256_parts(&[KEY_ID_DOMAIN, &bytes]));
        Ok(Self { key, identity })
    }

    pub const fn identity(&self) -> AnchorKeyIdentityV1 {
        self.identity
    }

    pub fn to_bytes(&self) -> [u8; 32] {
        self.key.to_bytes()
    }

    pub(crate) fn verifying_key(&self) -> &VerifyingKey {
        &self.key
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AnchorChallengeV1 {
    bytes: [u8; ANCHOR_CHALLENGE_WIRE_LEN_V1],
}

impl AnchorChallengeV1 {
    pub fn decode(bytes: &[u8]) -> Result<Self, AnchorProtocolErrorV1> {
        if bytes.len() != ANCHOR_CHALLENGE_WIRE_LEN_V1 {
            return Err(AnchorProtocolErrorV1::InvalidLength {
                expected: ANCHOR_CHALLENGE_WIRE_LEN_V1,
                actual: bytes.len(),
            });
        }
        let mut canonical = [0_u8; ANCHOR_CHALLENGE_WIRE_LEN_V1];
        canonical.copy_from_slice(bytes);
        validate_common(&canonical, CHALLENGE_MAGIC)?;
        ChallengeKindV1::decode(canonical[KIND_OFFSET])?;
        if canonical[POSITION_OFFSET] != 0 {
            return Err(AnchorProtocolErrorV1::NonzeroReserved);
        }
        validate_nonce(&canonical)?;
        validate_chain_fields(&canonical)?;
        Ok(Self { bytes: canonical })
    }

    pub const fn as_bytes(&self) -> &[u8; ANCHOR_CHALLENGE_WIRE_LEN_V1] {
        &self.bytes
    }

    pub fn kind(&self) -> ChallengeKindV1 {
        ChallengeKindV1::decode(self.bytes[KIND_OFFSET]).expect("constructed challenge kind")
    }

    pub fn nonce(&self) -> [u8; 32] {
        array_at(&self.bytes, NONCE_OFFSET)
    }

    pub fn expected_sequence(&self) -> u64 {
        u64_at(&self.bytes, EXPECTED_SEQUENCE_OFFSET)
    }

    pub fn prior_head(&self) -> HashChainHeadV1 {
        HashChainHeadV1(array_at(&self.bytes, PRIOR_HEAD_OFFSET))
    }

    pub fn transaction(&self) -> TransactionDigestV1 {
        TransactionDigestV1(array_at(&self.bytes, TRANSACTION_OFFSET))
    }

    pub fn proposed_head(&self) -> HashChainHeadV1 {
        HashChainHeadV1(array_at(&self.bytes, PROPOSED_HEAD_OFFSET))
    }

    pub fn anchor_key_identity(&self) -> AnchorKeyIdentityV1 {
        AnchorKeyIdentityV1(array_at(&self.bytes, KEY_ID_OFFSET))
    }

    pub(crate) fn new(
        kind: ChallengeKindV1,
        nonce: CallerNonceV1,
        expected_sequence: u64,
        prior_head: HashChainHeadV1,
        transaction: TransactionDigestV1,
        proposed_head: HashChainHeadV1,
        anchor_key_identity: AnchorKeyIdentityV1,
    ) -> Result<Self, AnchorProtocolErrorV1> {
        if expected_sequence == 0 {
            return Err(AnchorProtocolErrorV1::SequenceRegression);
        }
        if nonce.as_bytes().iter().all(|byte| *byte == 0) {
            return Err(AnchorProtocolErrorV1::ZeroNonce);
        }
        let expected_head = derive_proposed_head_v1(
            expected_sequence,
            prior_head,
            transaction,
            anchor_key_identity,
        );
        if proposed_head != expected_head {
            return Err(AnchorProtocolErrorV1::InvalidProposedHead);
        }
        let mut bytes = [0_u8; ANCHOR_CHALLENGE_WIRE_LEN_V1];
        bytes[..8].copy_from_slice(&CHALLENGE_MAGIC);
        bytes[VERSION_OFFSET..VERSION_OFFSET + 2]
            .copy_from_slice(&EXTERNAL_ANCHOR_PROTOCOL_VERSION_V1.to_le_bytes());
        bytes[KIND_OFFSET] = kind as u8;
        bytes[NONCE_OFFSET..NONCE_OFFSET + 32].copy_from_slice(nonce.as_bytes());
        bytes[EXPECTED_SEQUENCE_OFFSET..EXPECTED_SEQUENCE_OFFSET + 8]
            .copy_from_slice(&expected_sequence.to_le_bytes());
        bytes[PRIOR_HEAD_OFFSET..PRIOR_HEAD_OFFSET + 32].copy_from_slice(&prior_head.to_bytes());
        bytes[TRANSACTION_OFFSET..TRANSACTION_OFFSET + 32].copy_from_slice(&transaction.to_bytes());
        bytes[PROPOSED_HEAD_OFFSET..PROPOSED_HEAD_OFFSET + 32]
            .copy_from_slice(&proposed_head.to_bytes());
        bytes[KEY_ID_OFFSET..KEY_ID_OFFSET + 32].copy_from_slice(&anchor_key_identity.to_bytes());
        Ok(Self { bytes })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UnsignedAnchorObservationV1 {
    bytes: [u8; ANCHOR_OBSERVATION_SIGNED_LEN_V1],
}

impl UnsignedAnchorObservationV1 {
    /// Constructs the only two canonical observations admitted for a challenge.
    pub fn from_challenge(challenge: &AnchorChallengeV1, position: AnchorPositionV1) -> Self {
        let mut bytes = [0_u8; ANCHOR_OBSERVATION_SIGNED_LEN_V1];
        bytes[..8].copy_from_slice(&OBSERVATION_MAGIC);
        bytes[VERSION_OFFSET..VERSION_OFFSET + 2]
            .copy_from_slice(&EXTERNAL_ANCHOR_PROTOCOL_VERSION_V1.to_le_bytes());
        bytes[KIND_OFFSET] = challenge.kind() as u8;
        bytes[POSITION_OFFSET] = position as u8;
        bytes[NONCE_OFFSET..ANCHOR_CHALLENGE_WIRE_LEN_V1]
            .copy_from_slice(&challenge.as_bytes()[NONCE_OFFSET..]);
        let (sequence, head) = match position {
            AnchorPositionV1::Prior => (challenge.expected_sequence() - 1, challenge.prior_head()),
            AnchorPositionV1::Proposed => {
                (challenge.expected_sequence(), challenge.proposed_head())
            }
        };
        bytes[OBSERVED_SEQUENCE_OFFSET..OBSERVED_SEQUENCE_OFFSET + 8]
            .copy_from_slice(&sequence.to_le_bytes());
        bytes[OBSERVED_HEAD_OFFSET..OBSERVED_HEAD_OFFSET + 32].copy_from_slice(&head.to_bytes());
        Self { bytes }
    }

    /// Returns the exact domain-separated bytes an external anchor must sign.
    pub fn signing_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(SIGNING_DOMAIN.len() + self.bytes.len());
        bytes.extend_from_slice(SIGNING_DOMAIN);
        bytes.extend_from_slice(&self.bytes);
        bytes
    }

    /// Attaches signature bytes without claiming they are valid.
    pub fn attach_signature(self, signature: [u8; 64]) -> [u8; ANCHOR_OBSERVATION_WIRE_LEN_V1] {
        let mut bytes = [0_u8; ANCHOR_OBSERVATION_WIRE_LEN_V1];
        bytes[..ANCHOR_OBSERVATION_SIGNED_LEN_V1].copy_from_slice(&self.bytes);
        bytes[SIGNATURE_OFFSET..].copy_from_slice(&signature);
        bytes
    }
}

pub fn derive_proposed_head_v1(
    expected_sequence: u64,
    prior_head: HashChainHeadV1,
    transaction: TransactionDigestV1,
    anchor_key_identity: AnchorKeyIdentityV1,
) -> HashChainHeadV1 {
    HashChainHeadV1(sha256_parts(&[
        HEAD_DOMAIN,
        &EXTERNAL_ANCHOR_PROTOCOL_VERSION_V1.to_le_bytes(),
        &expected_sequence.to_le_bytes(),
        &prior_head.to_bytes(),
        &transaction.to_bytes(),
        &anchor_key_identity.to_bytes(),
    ]))
}

pub(crate) fn verify_observation(
    challenge: &AnchorChallengeV1,
    key: &PinnedAnchorKeyV1,
    bytes: &[u8],
) -> Result<AnchorPositionV1, AnchorProtocolErrorV1> {
    if bytes.len() != ANCHOR_OBSERVATION_WIRE_LEN_V1 {
        return Err(AnchorProtocolErrorV1::InvalidLength {
            expected: ANCHOR_OBSERVATION_WIRE_LEN_V1,
            actual: bytes.len(),
        });
    }
    let signed = &bytes[..ANCHOR_OBSERVATION_SIGNED_LEN_V1];
    validate_common(signed, OBSERVATION_MAGIC)?;
    let kind = ChallengeKindV1::decode(signed[KIND_OFFSET])?;
    let position = AnchorPositionV1::decode(signed[POSITION_OFFSET])?;
    validate_nonce(signed)?;
    validate_chain_fields(signed)?;
    validate_observed_position(signed, position)?;

    if kind != challenge.kind()
        || signed[NONCE_OFFSET..ANCHOR_CHALLENGE_WIRE_LEN_V1]
            != challenge.as_bytes()[NONCE_OFFSET..]
    {
        return Err(AnchorProtocolErrorV1::ChallengeMismatch);
    }
    if challenge.anchor_key_identity() != key.identity() {
        return Err(AnchorProtocolErrorV1::AnchorKeyIdentityMismatch);
    }

    let mut signature_bytes = [0_u8; 64];
    signature_bytes.copy_from_slice(&bytes[SIGNATURE_OFFSET..]);
    let signature = Signature::from_bytes(&signature_bytes);
    let mut message = Vec::with_capacity(SIGNING_DOMAIN.len() + signed.len());
    message.extend_from_slice(SIGNING_DOMAIN);
    message.extend_from_slice(signed);
    key.verifying_key()
        .verify_strict(&message, &signature)
        .map_err(|_| AnchorProtocolErrorV1::SignatureRejected)?;
    Ok(position)
}

fn validate_common(bytes: &[u8], magic: [u8; 8]) -> Result<(), AnchorProtocolErrorV1> {
    if bytes[..8] != magic {
        return Err(AnchorProtocolErrorV1::InvalidMagic);
    }
    let version = u16::from_le_bytes([bytes[VERSION_OFFSET], bytes[VERSION_OFFSET + 1]]);
    if version != EXTERNAL_ANCHOR_PROTOCOL_VERSION_V1 {
        return Err(AnchorProtocolErrorV1::UnsupportedVersion { actual: version });
    }
    if bytes[RESERVED_OFFSET..NONCE_OFFSET]
        .iter()
        .any(|byte| *byte != 0)
    {
        return Err(AnchorProtocolErrorV1::NonzeroReserved);
    }
    Ok(())
}

fn validate_chain_fields(bytes: &[u8]) -> Result<(), AnchorProtocolErrorV1> {
    let sequence = u64_at(bytes, EXPECTED_SEQUENCE_OFFSET);
    if sequence == 0 {
        return Err(AnchorProtocolErrorV1::SequenceRegression);
    }
    let prior = HashChainHeadV1(array_at(bytes, PRIOR_HEAD_OFFSET));
    let transaction = TransactionDigestV1(array_at(bytes, TRANSACTION_OFFSET));
    let proposed = HashChainHeadV1(array_at(bytes, PROPOSED_HEAD_OFFSET));
    let key_id = AnchorKeyIdentityV1(array_at(bytes, KEY_ID_OFFSET));
    if proposed != derive_proposed_head_v1(sequence, prior, transaction, key_id) {
        return Err(AnchorProtocolErrorV1::InvalidProposedHead);
    }
    Ok(())
}

fn validate_nonce(bytes: &[u8]) -> Result<(), AnchorProtocolErrorV1> {
    if bytes[NONCE_OFFSET..NONCE_OFFSET + 32]
        .iter()
        .all(|byte| *byte == 0)
    {
        return Err(AnchorProtocolErrorV1::ZeroNonce);
    }
    Ok(())
}

fn validate_observed_position(
    bytes: &[u8],
    position: AnchorPositionV1,
) -> Result<(), AnchorProtocolErrorV1> {
    let expected = u64_at(bytes, EXPECTED_SEQUENCE_OFFSET);
    let observed = u64_at(bytes, OBSERVED_SEQUENCE_OFFSET);
    let observed_head = array_at::<32>(bytes, OBSERVED_HEAD_OFFSET);
    let valid = match position {
        AnchorPositionV1::Prior => {
            observed == expected - 1 && observed_head == array_at(bytes, PRIOR_HEAD_OFFSET)
        }
        AnchorPositionV1::Proposed => {
            observed == expected && observed_head == array_at(bytes, PROPOSED_HEAD_OFFSET)
        }
    };
    if !valid {
        return Err(AnchorProtocolErrorV1::InvalidObservedPosition);
    }
    Ok(())
}

fn sha256_parts(parts: &[&[u8]]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    for part in parts {
        hasher.update(part);
    }
    hasher.finalize().into()
}

fn u64_at(bytes: &[u8], offset: usize) -> u64 {
    u64::from_le_bytes(array_at(bytes, offset))
}

fn array_at<const N: usize>(bytes: &[u8], offset: usize) -> [u8; N] {
    bytes[offset..offset + N]
        .try_into()
        .expect("fixed protocol field is in bounds")
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AnchorProtocolErrorV1 {
    InvalidLength { expected: usize, actual: usize },
    InvalidMagic,
    UnsupportedVersion { actual: u16 },
    UnknownChallengeKind { actual: u8 },
    UnknownAnchorPosition { actual: u8 },
    NonzeroReserved,
    ZeroNonce,
    SequenceOverflow,
    SequenceRegression,
    InvalidTransactionIdentityLength { actual: usize, maximum: usize },
    InvalidProposedHead,
    InvalidObservedPosition,
    ChallengeMismatch,
    InvalidVerifyingKey,
    WeakVerifyingKey,
    AnchorKeyIdentityMismatch,
    SignatureRejected,
    ReceiptIdentityMismatch,
}

impl fmt::Display for AnchorProtocolErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidLength { expected, actual } => {
                write!(
                    formatter,
                    "invalid wire length: expected {expected}, got {actual}"
                )
            }
            Self::InvalidMagic => formatter.write_str("invalid protocol magic"),
            Self::UnsupportedVersion { actual } => {
                write!(formatter, "unsupported protocol version {actual}")
            }
            Self::UnknownChallengeKind { actual } => {
                write!(formatter, "unknown challenge kind {actual}")
            }
            Self::UnknownAnchorPosition { actual } => {
                write!(formatter, "unknown anchor position {actual}")
            }
            Self::NonzeroReserved => formatter.write_str("reserved bytes must be zero"),
            Self::ZeroNonce => formatter.write_str("caller nonce must not be all zero"),
            Self::SequenceOverflow => formatter.write_str("anchor sequence overflow"),
            Self::SequenceRegression => {
                formatter.write_str("anchor expected sequence must have a predecessor")
            }
            Self::InvalidTransactionIdentityLength { actual, maximum } => write!(
                formatter,
                "transaction identity length must be in 1..={maximum} bytes, got {actual}"
            ),
            Self::InvalidProposedHead => formatter.write_str("invalid proposed hash-chain head"),
            Self::InvalidObservedPosition => {
                formatter.write_str("observation is neither the exact prior nor proposed position")
            }
            Self::ChallengeMismatch => formatter.write_str("observation does not match challenge"),
            Self::InvalidVerifyingKey => formatter.write_str("invalid Ed25519 verifying key"),
            Self::WeakVerifyingKey => formatter.write_str("weak Ed25519 verifying key"),
            Self::AnchorKeyIdentityMismatch => {
                formatter.write_str("pinned key identity does not match prepared transition")
            }
            Self::SignatureRejected => formatter.write_str("Ed25519 signature rejected"),
            Self::ReceiptIdentityMismatch => {
                formatter.write_str("external anchor receipt identity mismatch")
            }
        }
    }
}

impl std::error::Error for AnchorProtocolErrorV1 {}
