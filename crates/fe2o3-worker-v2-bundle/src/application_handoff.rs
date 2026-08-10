use core::fmt;

use sha2::{Digest, Sha256};

use crate::{WorkerV2LoadEnvelopeIdentityV1, WorkerV2LoadEnvelopeV1};

const COMMITMENT_DOMAIN: &[u8] = b"FE2O3/APPLICATION-WORKER-V2-HANDOFF/V1\0";
const ACK_MAGIC: &[u8] = b"FE2O3-WORKER-V2-APPLICATION-ACK\0";
const ACK_VERSION: u16 = 1;
const ACK_CHECKSUM_DOMAIN: &[u8] = b"FE2O3/WORKER-V2-APPLICATION-ACK-CHECKSUM/V1\0";
const ACK_BODY_BYTES: usize = ACK_MAGIC.len() + 2 + 32 * 4;
pub const WORKER_V2_APPLICATION_HANDOFF_ACK_BYTES_V1: usize = ACK_BODY_BYTES + 32;

/// Exact digest of the application executable admitted by the Cargo runner.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WorkerV2ApplicationIdentityV1([u8; 32]);

impl WorkerV2ApplicationIdentityV1 {
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub const fn as_bytes(self) -> [u8; 32] {
        self.0
    }
}

/// Fresh runner challenge binding one child acknowledgment to one spawn.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WorkerV2ApplicationHandoffChallengeV1([u8; 32]);

impl WorkerV2ApplicationHandoffChallengeV1 {
    pub fn from_bytes(bytes: [u8; 32]) -> Result<Self, ApplicationHandoffProtocolErrorV1> {
        if bytes == [0; 32] {
            return Err(ApplicationHandoffProtocolErrorV1::ZeroChallenge);
        }
        Ok(Self(bytes))
    }

    pub const fn as_bytes(self) -> [u8; 32] {
        self.0
    }

    pub fn to_hex(self) -> String {
        hex(&self.0)
    }

    pub fn from_hex(value: &str) -> Result<Self, ApplicationHandoffProtocolErrorV1> {
        Self::from_bytes(decode_hex(value)?)
    }
}

/// Canonical commitment joining the complete envelope publication claim to one child image.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WorkerV2ApplicationHandoffCommitmentV1([u8; 32]);

impl WorkerV2ApplicationHandoffCommitmentV1 {
    pub const fn as_bytes(self) -> [u8; 32] {
        self.0
    }

    pub fn to_hex(self) -> String {
        hex(&self.0)
    }

    pub fn from_hex(value: &str) -> Result<Self, ApplicationHandoffProtocolErrorV1> {
        Ok(Self(decode_hex(value)?))
    }
}

/// Typed expected identity shared by the runner and descriptor-consuming application.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WorkerV2ApplicationHandoffExpectationV1 {
    commitment: WorkerV2ApplicationHandoffCommitmentV1,
    envelope: WorkerV2LoadEnvelopeIdentityV1,
    application: WorkerV2ApplicationIdentityV1,
}

impl WorkerV2ApplicationHandoffExpectationV1 {
    pub fn new(
        envelope: &WorkerV2LoadEnvelopeV1,
        application: WorkerV2ApplicationIdentityV1,
    ) -> Self {
        let claim = envelope.published_claim();
        let plan = claim.plan();
        let attempt = plan.attempt();
        let receipt = claim.receipt();
        let envelope_identity = envelope.identity();
        let mut digest = Sha256::new();
        digest.update(COMMITMENT_DOMAIN);
        digest.update(plan.scope().package().as_bytes());
        digest.update(attempt.generation().to_le_bytes());
        digest.update(attempt.session().as_bytes());
        digest.update(attempt.invocation().as_bytes());
        for field in [
            receipt.attempt_identity(),
            receipt.producer_identity(),
            receipt.scope_identity(),
            receipt.plan_commitment(),
            receipt.upstream_evidence_identity(),
            receipt.finalized_output_identity(),
            receipt.publication_identity(),
        ] {
            digest.update(field);
        }
        digest.update(envelope_identity.as_bytes());
        digest.update(application.as_bytes());
        Self {
            commitment: WorkerV2ApplicationHandoffCommitmentV1(digest.finalize().into()),
            envelope: envelope_identity,
            application,
        }
    }

    pub const fn commitment(self) -> WorkerV2ApplicationHandoffCommitmentV1 {
        self.commitment
    }

    pub const fn envelope(self) -> WorkerV2LoadEnvelopeIdentityV1 {
        self.envelope
    }

    pub const fn application(self) -> WorkerV2ApplicationIdentityV1 {
        self.application
    }

    pub const fn acknowledgment(
        self,
        challenge: WorkerV2ApplicationHandoffChallengeV1,
    ) -> WorkerV2ApplicationHandoffAckV1 {
        WorkerV2ApplicationHandoffAckV1 {
            challenge,
            commitment: self.commitment,
            envelope: self.envelope,
            application: self.application,
        }
    }
}

/// Fixed-schema child acknowledgment of one validated descriptor handoff.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WorkerV2ApplicationHandoffAckV1 {
    challenge: WorkerV2ApplicationHandoffChallengeV1,
    commitment: WorkerV2ApplicationHandoffCommitmentV1,
    envelope: WorkerV2LoadEnvelopeIdentityV1,
    application: WorkerV2ApplicationIdentityV1,
}

impl WorkerV2ApplicationHandoffAckV1 {
    pub const fn challenge(self) -> WorkerV2ApplicationHandoffChallengeV1 {
        self.challenge
    }

    pub const fn commitment(self) -> WorkerV2ApplicationHandoffCommitmentV1 {
        self.commitment
    }

    pub fn encode_canonical(self) -> [u8; WORKER_V2_APPLICATION_HANDOFF_ACK_BYTES_V1] {
        let mut bytes = [0_u8; WORKER_V2_APPLICATION_HANDOFF_ACK_BYTES_V1];
        let mut offset = 0;
        push(&mut bytes, &mut offset, ACK_MAGIC);
        push(&mut bytes, &mut offset, &ACK_VERSION.to_le_bytes());
        push(&mut bytes, &mut offset, &self.challenge.as_bytes());
        push(&mut bytes, &mut offset, &self.commitment.as_bytes());
        push(&mut bytes, &mut offset, &self.envelope.as_bytes());
        push(&mut bytes, &mut offset, &self.application.as_bytes());
        debug_assert_eq!(offset, ACK_BODY_BYTES);
        let checksum = ack_checksum(&bytes[..ACK_BODY_BYTES]);
        push(&mut bytes, &mut offset, &checksum);
        debug_assert_eq!(offset, bytes.len());
        bytes
    }

    pub fn decode_canonical(bytes: &[u8]) -> Result<Self, ApplicationHandoffProtocolErrorV1> {
        if bytes.len() < WORKER_V2_APPLICATION_HANDOFF_ACK_BYTES_V1 {
            return Err(ApplicationHandoffProtocolErrorV1::TruncatedAck {
                actual: bytes.len(),
            });
        }
        if bytes.len() > WORKER_V2_APPLICATION_HANDOFF_ACK_BYTES_V1 {
            return Err(ApplicationHandoffProtocolErrorV1::TrailingAckBytes {
                actual: bytes.len(),
            });
        }
        let (body, checksum) = bytes.split_at(ACK_BODY_BYTES);
        if ack_checksum(body) != checksum {
            return Err(ApplicationHandoffProtocolErrorV1::AckChecksumMismatch);
        }
        if &body[..ACK_MAGIC.len()] != ACK_MAGIC {
            return Err(ApplicationHandoffProtocolErrorV1::BadAckMagic);
        }
        let mut offset = ACK_MAGIC.len();
        let version = u16::from_le_bytes(take(body, &mut offset));
        if version != ACK_VERSION {
            return Err(ApplicationHandoffProtocolErrorV1::UnsupportedAckVersion {
                actual: version,
            });
        }
        let ack = Self {
            challenge: WorkerV2ApplicationHandoffChallengeV1::from_bytes(take(body, &mut offset))?,
            commitment: WorkerV2ApplicationHandoffCommitmentV1(take(body, &mut offset)),
            envelope: WorkerV2LoadEnvelopeIdentityV1::from_bytes(take(body, &mut offset)),
            application: WorkerV2ApplicationIdentityV1::from_bytes(take(body, &mut offset)),
        };
        debug_assert_eq!(offset, body.len());
        if ack.encode_canonical().as_slice() != bytes {
            return Err(ApplicationHandoffProtocolErrorV1::NonCanonicalAck);
        }
        Ok(ack)
    }

    pub fn validate(
        self,
        expected: WorkerV2ApplicationHandoffExpectationV1,
        challenge: WorkerV2ApplicationHandoffChallengeV1,
    ) -> Result<(), ApplicationHandoffProtocolErrorV1> {
        if self.challenge != challenge {
            return Err(ApplicationHandoffProtocolErrorV1::ChallengeMismatch);
        }
        if self.commitment != expected.commitment {
            return Err(ApplicationHandoffProtocolErrorV1::CommitmentMismatch);
        }
        if self.envelope != expected.envelope {
            return Err(ApplicationHandoffProtocolErrorV1::EnvelopeMismatch);
        }
        if self.application != expected.application {
            return Err(ApplicationHandoffProtocolErrorV1::ApplicationMismatch);
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ApplicationHandoffProtocolErrorV1 {
    InvalidHex,
    ZeroChallenge,
    TruncatedAck { actual: usize },
    TrailingAckBytes { actual: usize },
    AckChecksumMismatch,
    BadAckMagic,
    UnsupportedAckVersion { actual: u16 },
    NonCanonicalAck,
    ChallengeMismatch,
    CommitmentMismatch,
    EnvelopeMismatch,
    ApplicationMismatch,
}

impl fmt::Display for ApplicationHandoffProtocolErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidHex => formatter.write_str("invalid 32-byte lowercase hexadecimal value"),
            Self::ZeroChallenge => formatter.write_str("application handoff challenge is zero"),
            Self::TruncatedAck { actual } => write!(
                formatter,
                "application handoff acknowledgment is truncated ({actual} bytes)"
            ),
            Self::TrailingAckBytes { actual } => write!(
                formatter,
                "application handoff acknowledgment has trailing bytes ({actual} bytes)"
            ),
            Self::AckChecksumMismatch => {
                formatter.write_str("application handoff acknowledgment checksum mismatch")
            }
            Self::BadAckMagic => {
                formatter.write_str("invalid application handoff acknowledgment magic")
            }
            Self::UnsupportedAckVersion { actual } => write!(
                formatter,
                "unsupported application handoff acknowledgment version {actual}"
            ),
            Self::NonCanonicalAck => {
                formatter.write_str("application handoff acknowledgment is not canonical")
            }
            Self::ChallengeMismatch => {
                formatter.write_str("application handoff acknowledgment challenge mismatch")
            }
            Self::CommitmentMismatch => {
                formatter.write_str("application handoff acknowledgment commitment mismatch")
            }
            Self::EnvelopeMismatch => {
                formatter.write_str("application handoff acknowledgment envelope mismatch")
            }
            Self::ApplicationMismatch => {
                formatter.write_str("application handoff acknowledgment child mismatch")
            }
        }
    }
}

impl std::error::Error for ApplicationHandoffProtocolErrorV1 {}

fn ack_checksum(body: &[u8]) -> [u8; 32] {
    let mut digest = Sha256::new();
    digest.update(ACK_CHECKSUM_DOMAIN);
    digest.update(body);
    digest.finalize().into()
}

fn push<const N: usize>(output: &mut [u8; N], offset: &mut usize, value: &[u8]) {
    let end = *offset + value.len();
    output[*offset..end].copy_from_slice(value);
    *offset = end;
}

fn take<const N: usize>(bytes: &[u8], offset: &mut usize) -> [u8; N] {
    let end = *offset + N;
    let value = bytes[*offset..end].try_into().expect("fixed ACK field");
    *offset = end;
    value
}

fn hex(bytes: &[u8; 32]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut encoded = String::with_capacity(64);
    for byte in bytes {
        encoded.push(HEX[(byte >> 4) as usize] as char);
        encoded.push(HEX[(byte & 0x0f) as usize] as char);
    }
    encoded
}

fn decode_hex(value: &str) -> Result<[u8; 32], ApplicationHandoffProtocolErrorV1> {
    if value.len() != 64 {
        return Err(ApplicationHandoffProtocolErrorV1::InvalidHex);
    }
    let mut decoded = [0_u8; 32];
    for (output, pair) in decoded.iter_mut().zip(value.as_bytes().chunks_exact(2)) {
        let high = nibble(pair[0]).ok_or(ApplicationHandoffProtocolErrorV1::InvalidHex)?;
        let low = nibble(pair[1]).ok_or(ApplicationHandoffProtocolErrorV1::InvalidHex)?;
        *output = (high << 4) | low;
    }
    Ok(decoded)
}

const fn nibble(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn expectation(seed: u8) -> WorkerV2ApplicationHandoffExpectationV1 {
        WorkerV2ApplicationHandoffExpectationV1 {
            commitment: WorkerV2ApplicationHandoffCommitmentV1([seed; 32]),
            envelope: WorkerV2LoadEnvelopeIdentityV1::from_bytes([seed.wrapping_add(1); 32]),
            application: WorkerV2ApplicationIdentityV1::from_bytes([seed.wrapping_add(2); 32]),
        }
    }

    #[test]
    fn acknowledgment_round_trips_canonically_and_validates_every_identity() {
        let expected = expectation(7);
        let challenge = WorkerV2ApplicationHandoffChallengeV1::from_bytes([9; 32]).unwrap();
        let ack = expected.acknowledgment(challenge);
        let bytes = ack.encode_canonical();
        let decoded = WorkerV2ApplicationHandoffAckV1::decode_canonical(&bytes).unwrap();
        assert_eq!(decoded, ack);
        assert_eq!(decoded.validate(expected, challenge), Ok(()));

        assert_eq!(
            decoded.validate(expectation(8), challenge),
            Err(ApplicationHandoffProtocolErrorV1::CommitmentMismatch)
        );
        let other_challenge = WorkerV2ApplicationHandoffChallengeV1::from_bytes([10; 32]).unwrap();
        assert_eq!(
            decoded.validate(expected, other_challenge),
            Err(ApplicationHandoffProtocolErrorV1::ChallengeMismatch)
        );
    }

    #[test]
    fn acknowledgment_rejects_truncation_trailing_bytes_and_substitution() {
        let expected = expectation(3);
        let challenge = WorkerV2ApplicationHandoffChallengeV1::from_bytes([4; 32]).unwrap();
        let bytes = expected.acknowledgment(challenge).encode_canonical();
        assert!(matches!(
            WorkerV2ApplicationHandoffAckV1::decode_canonical(&bytes[..bytes.len() - 1]),
            Err(ApplicationHandoffProtocolErrorV1::TruncatedAck { .. })
        ));
        let mut extra = bytes.to_vec();
        extra.push(0);
        assert!(matches!(
            WorkerV2ApplicationHandoffAckV1::decode_canonical(&extra),
            Err(ApplicationHandoffProtocolErrorV1::TrailingAckBytes { .. })
        ));
        let mut substituted = bytes;
        substituted[ACK_MAGIC.len() + 2 + 32] ^= 1;
        assert_eq!(
            WorkerV2ApplicationHandoffAckV1::decode_canonical(&substituted),
            Err(ApplicationHandoffProtocolErrorV1::AckChecksumMismatch)
        );
    }

    #[test]
    fn challenge_and_commitment_hex_are_exact_and_lowercase() {
        let challenge = WorkerV2ApplicationHandoffChallengeV1::from_bytes([0xab; 32]).unwrap();
        assert_eq!(
            WorkerV2ApplicationHandoffChallengeV1::from_hex(&challenge.to_hex()),
            Ok(challenge)
        );
        assert!(WorkerV2ApplicationHandoffChallengeV1::from_hex(&"AB".repeat(32)).is_err());
        assert!(WorkerV2ApplicationHandoffChallengeV1::from_bytes([0; 32]).is_err());
        let commitment = WorkerV2ApplicationHandoffCommitmentV1([0xcd; 32]);
        assert_eq!(
            WorkerV2ApplicationHandoffCommitmentV1::from_hex(&commitment.to_hex()),
            Ok(commitment)
        );
    }
}
