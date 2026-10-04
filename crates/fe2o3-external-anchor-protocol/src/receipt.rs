use core::fmt;

use sha2::{Digest, Sha256};

use crate::protocol::verify_observation;
use crate::{
    ANCHOR_CHALLENGE_WIRE_LEN_V1, ANCHOR_OBSERVATION_WIRE_LEN_V1, AnchorChallengeV1,
    AnchorKeyIdentityV1, AnchorPositionV1, AnchorProtocolErrorV1, PinnedAnchorKeyV1,
};

const HEADER_BYTES: usize = 24;
const SHA256_BYTES: usize = 32;
const RECEIPT_MAGIC: [u8; 8] = *b"F2ARRC1\0";
const RECEIPT_VERSION_V1: u16 = 1;
const RECEIPT_IDENTITY_DOMAIN: &[u8] = b"FE2O3/EXTERNAL-MONOTONIC-ANCHOR/TRANSITION-RECEIPT/V1\0";
const RECEIPT_PREIMAGE_BYTES: usize =
    HEADER_BYTES + ANCHOR_CHALLENGE_WIRE_LEN_V1 + ANCHOR_OBSERVATION_WIRE_LEN_V1;

/// Exact canonical byte length of one signed external-anchor transition receipt.
pub const ANCHOR_TRANSITION_RECEIPT_BYTES_V1: usize = RECEIPT_PREIMAGE_BYTES + SHA256_BYTES;

/// Domain-separated identity of one exact challenge and signed observation.
#[derive(Clone, Copy, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct AnchorTransitionReceiptIdentityV1([u8; SHA256_BYTES]);

impl AnchorTransitionReceiptIdentityV1 {
    pub const fn as_bytes(&self) -> &[u8; SHA256_BYTES] {
        &self.0
    }

    pub fn matches_canonical_bytes(self, bytes: &[u8]) -> bool {
        bytes.len() == ANCHOR_TRANSITION_RECEIPT_BYTES_V1
            && bytes[RECEIPT_PREIMAGE_BYTES..] == self.0
            && receipt_identity(&bytes[..RECEIPT_PREIMAGE_BYTES]) == self.0
    }
}

impl fmt::Debug for AnchorTransitionReceiptIdentityV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_tuple("AnchorTransitionReceiptIdentityV1")
            .field(&self.0)
            .finish()
    }
}

/// Canonical authority-free evidence for one exact signed anchor observation.
#[derive(Clone, Eq, PartialEq)]
pub struct AnchorTransitionReceiptV1 {
    challenge: AnchorChallengeV1,
    observation: [u8; ANCHOR_OBSERVATION_WIRE_LEN_V1],
    position: AnchorPositionV1,
    identity: AnchorTransitionReceiptIdentityV1,
    canonical_bytes: [u8; ANCHOR_TRANSITION_RECEIPT_BYTES_V1],
}

impl AnchorTransitionReceiptV1 {
    /// Verifies and binds one exact signed observation under the caller-pinned key.
    pub fn new(
        challenge: AnchorChallengeV1,
        observation: &[u8],
        key: &PinnedAnchorKeyV1,
    ) -> Result<Self, AnchorProtocolErrorV1> {
        let position = verify_observation(&challenge, key, observation)?;
        let observation: [u8; ANCHOR_OBSERVATION_WIRE_LEN_V1] =
            observation
                .try_into()
                .map_err(|_| AnchorProtocolErrorV1::InvalidLength {
                    expected: ANCHOR_OBSERVATION_WIRE_LEN_V1,
                    actual: observation.len(),
                })?;
        let mut canonical_bytes = [0_u8; ANCHOR_TRANSITION_RECEIPT_BYTES_V1];
        let mut offset = encode_header(&mut canonical_bytes);
        put(&mut canonical_bytes, &mut offset, challenge.as_bytes());
        put(&mut canonical_bytes, &mut offset, &observation);
        debug_assert_eq!(offset, RECEIPT_PREIMAGE_BYTES);
        let identity =
            AnchorTransitionReceiptIdentityV1(receipt_identity(&canonical_bytes[..offset]));
        put(&mut canonical_bytes, &mut offset, identity.as_bytes());
        debug_assert_eq!(offset, canonical_bytes.len());
        Ok(Self {
            challenge,
            observation,
            position,
            identity,
            canonical_bytes,
        })
    }

    /// Strictly decodes and re-verifies one complete receipt under the caller-pinned key.
    pub fn decode(bytes: &[u8], key: &PinnedAnchorKeyV1) -> Result<Self, AnchorProtocolErrorV1> {
        if bytes.len() != ANCHOR_TRANSITION_RECEIPT_BYTES_V1 {
            return Err(AnchorProtocolErrorV1::InvalidLength {
                expected: ANCHOR_TRANSITION_RECEIPT_BYTES_V1,
                actual: bytes.len(),
            });
        }
        let mut reader = Reader::new(bytes);
        decode_header(&mut reader)?;
        let challenge = AnchorChallengeV1::decode(reader.take(ANCHOR_CHALLENGE_WIRE_LEN_V1)?)?;
        let observation = reader.take(ANCHOR_OBSERVATION_WIRE_LEN_V1)?;
        let declared_identity = AnchorTransitionReceiptIdentityV1(reader.fixed::<SHA256_BYTES>()?);
        if !reader.is_empty() {
            return Err(AnchorProtocolErrorV1::ReceiptIdentityMismatch);
        }
        let decoded = Self::new(challenge, observation, key)?;
        if decoded.identity != declared_identity
            || decoded.canonical_bytes.as_slice() != bytes
            || !declared_identity.matches_canonical_bytes(bytes)
        {
            return Err(AnchorProtocolErrorV1::ReceiptIdentityMismatch);
        }
        Ok(decoded)
    }

    pub const fn challenge(&self) -> &AnchorChallengeV1 {
        &self.challenge
    }

    pub const fn observation_bytes(&self) -> &[u8; ANCHOR_OBSERVATION_WIRE_LEN_V1] {
        &self.observation
    }

    pub const fn position(&self) -> AnchorPositionV1 {
        self.position
    }

    pub fn anchor_key_identity(&self) -> AnchorKeyIdentityV1 {
        self.challenge.anchor_key_identity()
    }

    pub const fn identity(&self) -> AnchorTransitionReceiptIdentityV1 {
        self.identity
    }

    pub const fn canonical_bytes(&self) -> &[u8; ANCHOR_TRANSITION_RECEIPT_BYTES_V1] {
        &self.canonical_bytes
    }

    pub const fn observes_proposed_position(&self) -> bool {
        matches!(self.position, AnchorPositionV1::Proposed)
    }

    pub const fn authenticates_pinned_key_and_exact_challenge(&self) -> bool {
        true
    }

    pub const fn grants_authority(&self) -> bool {
        false
    }
}

impl fmt::Debug for AnchorTransitionReceiptV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("AnchorTransitionReceiptV1")
            .field("kind", &self.challenge.kind())
            .field("position", &self.position)
            .field("sequence", &self.challenge.expected_sequence())
            .field("identity", &self.identity)
            .finish_non_exhaustive()
    }
}

fn encode_header(output: &mut [u8]) -> usize {
    let mut offset = 0;
    put(output, &mut offset, &RECEIPT_MAGIC);
    put(output, &mut offset, &RECEIPT_VERSION_V1.to_le_bytes());
    put(output, &mut offset, &0_u16.to_le_bytes());
    put(
        output,
        &mut offset,
        &(ANCHOR_TRANSITION_RECEIPT_BYTES_V1 as u64).to_le_bytes(),
    );
    put(output, &mut offset, &0_u32.to_le_bytes());
    offset
}

fn decode_header(reader: &mut Reader<'_>) -> Result<(), AnchorProtocolErrorV1> {
    if reader.fixed::<8>()? != RECEIPT_MAGIC {
        return Err(AnchorProtocolErrorV1::InvalidMagic);
    }
    let version = reader.u16()?;
    if version != RECEIPT_VERSION_V1 {
        return Err(AnchorProtocolErrorV1::UnsupportedVersion { actual: version });
    }
    if reader.u16()? != 0 {
        return Err(AnchorProtocolErrorV1::NonzeroReserved);
    }
    if reader.u64()? != ANCHOR_TRANSITION_RECEIPT_BYTES_V1 as u64 {
        return Err(AnchorProtocolErrorV1::ReceiptIdentityMismatch);
    }
    if reader.u32()? != 0 {
        return Err(AnchorProtocolErrorV1::NonzeroReserved);
    }
    Ok(())
}

fn receipt_identity(preimage: &[u8]) -> [u8; SHA256_BYTES] {
    let mut digest = Sha256::new();
    digest.update(RECEIPT_IDENTITY_DOMAIN);
    digest.update(preimage);
    digest.finalize().into()
}

fn put(output: &mut [u8], offset: &mut usize, bytes: &[u8]) {
    let end = *offset + bytes.len();
    output[*offset..end].copy_from_slice(bytes);
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

    fn take(&mut self, length: usize) -> Result<&'a [u8], AnchorProtocolErrorV1> {
        let end = self
            .offset
            .checked_add(length)
            .ok_or(AnchorProtocolErrorV1::ReceiptIdentityMismatch)?;
        let value = self
            .bytes
            .get(self.offset..end)
            .ok_or(AnchorProtocolErrorV1::ReceiptIdentityMismatch)?;
        self.offset = end;
        Ok(value)
    }

    fn fixed<const N: usize>(&mut self) -> Result<[u8; N], AnchorProtocolErrorV1> {
        self.take(N)?
            .try_into()
            .map_err(|_| AnchorProtocolErrorV1::ReceiptIdentityMismatch)
    }

    fn u16(&mut self) -> Result<u16, AnchorProtocolErrorV1> {
        Ok(u16::from_le_bytes(self.fixed()?))
    }

    fn u32(&mut self) -> Result<u32, AnchorProtocolErrorV1> {
        Ok(u32::from_le_bytes(self.fixed()?))
    }

    fn u64(&mut self) -> Result<u64, AnchorProtocolErrorV1> {
        Ok(u64::from_le_bytes(self.fixed()?))
    }

    fn is_empty(&self) -> bool {
        self.offset == self.bytes.len()
    }
}
