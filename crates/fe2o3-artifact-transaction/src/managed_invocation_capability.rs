use std::error::Error;
use std::fmt;

use crate::{BuildAttempt, BuildInvocation, BuildSession};

const REQUEST_MAGIC_V1: &[u8; 8] = b"F2BRKIV1";
const RELEASE_TAG_V1: u8 = 0;
const PREPARE_TAG_V1: u8 = 1;
const CONSUME_TAG_V1: u8 = 2;
const HEADER_BYTES_V1: usize = REQUEST_MAGIC_V1.len() + 1 + 7;
const REQUEST_MAGIC_V2: &[u8; 8] = b"F2BRKIV2";
const RELEASE_WITH_SOURCE_ISA_OBSERVER_TAG_V2: u8 = 3;
const HEADER_BYTES_V2: usize = REQUEST_MAGIC_V2.len() + 1 + 7;
/// Reserved child descriptor carrying the live brokered invocation authority.
pub const BROKERED_INVOCATION_AUTHORITY_CHILD_FD_V1: i32 = 195;
/// Reserved child descriptor carrying the brokered artifact directory.
pub const BROKERED_ARTIFACT_DIRECTORY_CHILD_FD_V1: i32 = 197;
/// Reserved child descriptor carrying the brokered codegen backend.
pub const BROKERED_CODEGEN_BACKEND_CHILD_FD_V1: i32 = 198;
/// Canonical artifact-directory path installed by the managed wrapper.
pub const BROKERED_ARTIFACT_DIRECTORY_PATH_V1: &str = "/proc/self/fd/197";
/// Canonical codegen-backend path installed by the managed wrapper.
pub const BROKERED_CODEGEN_BACKEND_PATH_V1: &str = "/proc/./self/fd/198";
/// Exact byte length of one brokered invocation-capability request.
pub const BROKERED_INVOCATION_REQUEST_BYTES_V1: usize = HEADER_BYTES_V1 + 8 + 16 + 32 + 32;
/// Exact byte length of the opt-in source/ISA observer release request.
pub const BROKERED_INVOCATION_REQUEST_BYTES_V2: usize = HEADER_BYTES_V2 + 32 + 32 + 8 + 16 + 32;
/// Exact response acknowledging that the authenticated wrapper prepared a claim.
pub const BROKERED_INVOCATION_PREPARED_V1: &[u8; 16] = b"F2IV-PREPARED-V1";
/// Exact response consuming the wrapper-prepared claim in its rustc child.
pub const BROKERED_INVOCATION_ADMITTED_V1: &[u8; 16] = b"F2IV-ADMITTED-V1";

/// Exact attempt and effective-argv identity authorized by the capability broker.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BrokeredInvocationCapabilityClaimV1 {
    attempt: BuildAttempt,
    effective_argv_sha256: [u8; 32],
}

impl BrokeredInvocationCapabilityClaimV1 {
    /// Constructs a claim only when the attempt names the same nonzero argv identity.
    pub fn new(
        attempt: BuildAttempt,
        effective_argv_sha256: [u8; 32],
    ) -> Result<Self, BrokeredInvocationCapabilityCodecErrorV1> {
        if attempt.session() == BuildSession::DIRECT
            || effective_argv_sha256 == [0; 32]
            || attempt.invocation().as_bytes() != &effective_argv_sha256
        {
            return Err(BrokeredInvocationCapabilityCodecErrorV1::InvalidClaim);
        }
        Ok(Self {
            attempt,
            effective_argv_sha256,
        })
    }

    /// Returns the exact managed build attempt.
    pub const fn attempt(self) -> BuildAttempt {
        self.attempt
    }

    /// Returns the SHA-256 identity of the complete effective rustc argv.
    pub const fn effective_argv_sha256(self) -> [u8; 32] {
        self.effective_argv_sha256
    }
}

/// One fixed-width request on the already authenticated broker connection.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BrokeredInvocationCapabilityRequestV1 {
    /// Closes an ordinary compile that does not request row-softmax authority.
    Release,
    /// Records the authenticated wrapper's exact prepared invocation.
    Prepare(BrokeredInvocationCapabilityClaimV1),
    /// Consumes that exact claim in the spawned rustc process.
    Consume(BrokeredInvocationCapabilityClaimV1),
}

impl BrokeredInvocationCapabilityRequestV1 {
    /// Encodes one canonical fixed-width request.
    pub fn encode(self) -> [u8; BROKERED_INVOCATION_REQUEST_BYTES_V1] {
        let mut encoded = [0_u8; BROKERED_INVOCATION_REQUEST_BYTES_V1];
        encoded[..REQUEST_MAGIC_V1.len()].copy_from_slice(REQUEST_MAGIC_V1);
        let (tag, claim) = match self {
            Self::Release => (RELEASE_TAG_V1, None),
            Self::Prepare(claim) => (PREPARE_TAG_V1, Some(claim)),
            Self::Consume(claim) => (CONSUME_TAG_V1, Some(claim)),
        };
        encoded[REQUEST_MAGIC_V1.len()] = tag;
        if let Some(claim) = claim {
            let mut offset = HEADER_BYTES_V1;
            encoded[offset..offset + 8].copy_from_slice(&claim.attempt.generation().to_le_bytes());
            offset += 8;
            encoded[offset..offset + 16].copy_from_slice(claim.attempt.session().as_bytes());
            offset += 16;
            encoded[offset..offset + 32].copy_from_slice(claim.attempt.invocation().as_bytes());
            offset += 32;
            encoded[offset..offset + 32].copy_from_slice(&claim.effective_argv_sha256);
        }
        encoded
    }

    /// Decodes one canonical fixed-width request.
    pub fn decode(encoded: &[u8]) -> Result<Self, BrokeredInvocationCapabilityCodecErrorV1> {
        if encoded.len() != BROKERED_INVOCATION_REQUEST_BYTES_V1
            || &encoded[..REQUEST_MAGIC_V1.len()] != REQUEST_MAGIC_V1
            || encoded[REQUEST_MAGIC_V1.len() + 1..HEADER_BYTES_V1]
                .iter()
                .any(|byte| *byte != 0)
        {
            return Err(BrokeredInvocationCapabilityCodecErrorV1::Malformed);
        }
        let tag = encoded[REQUEST_MAGIC_V1.len()];
        if tag == RELEASE_TAG_V1 {
            return encoded[HEADER_BYTES_V1..]
                .iter()
                .all(|byte| *byte == 0)
                .then_some(Self::Release)
                .ok_or(BrokeredInvocationCapabilityCodecErrorV1::Malformed);
        }

        let mut offset = HEADER_BYTES_V1;
        let generation = u64::from_le_bytes(
            encoded[offset..offset + 8]
                .try_into()
                .expect("fixed generation field"),
        );
        offset += 8;
        let session = BuildSession::from_bytes(
            encoded[offset..offset + 16]
                .try_into()
                .expect("fixed session field"),
        );
        offset += 16;
        let invocation = BuildInvocation::from_bytes(
            encoded[offset..offset + 32]
                .try_into()
                .expect("fixed invocation field"),
        );
        offset += 32;
        let effective_argv_sha256 = encoded[offset..offset + 32]
            .try_into()
            .expect("fixed argv identity field");
        let attempt = BuildAttempt::new(generation, session, invocation)
            .map_err(|_| BrokeredInvocationCapabilityCodecErrorV1::InvalidClaim)?;
        let claim = BrokeredInvocationCapabilityClaimV1::new(attempt, effective_argv_sha256)?;
        match tag {
            PREPARE_TAG_V1 => Ok(Self::Prepare(claim)),
            CONSUME_TAG_V1 => Ok(Self::Consume(claim)),
            _ => Err(BrokeredInvocationCapabilityCodecErrorV1::Malformed),
        }
    }
}

/// Canonical brokered invocation-capability codec failure.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BrokeredInvocationCapabilityCodecErrorV1 {
    /// The request has invalid framing, tags, reserved bytes, or length.
    Malformed,
    /// The claim is direct, zero, malformed, or does not bind its argv identity.
    InvalidClaim,
}

impl fmt::Display for BrokeredInvocationCapabilityCodecErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Malformed => formatter.write_str("malformed brokered invocation request"),
            Self::InvalidClaim => formatter.write_str("invalid brokered invocation claim"),
        }
    }
}

impl Error for BrokeredInvocationCapabilityCodecErrorV1 {}

/// One fixed-width opt-in request on the already authenticated broker connection.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BrokeredInvocationCapabilityRequestV2 {
    /// Releases invocation authority and turns the same stream into a one-shot observation sink.
    ReleaseWithSourceIsaObserver {
        /// Exact V2 production configuration identity authenticated by the broker route.
        config_identity: [u8; 32],
        /// Exact selected compilation-unit identity derived from that configuration.
        unit_identity: [u8; 32],
        /// Exact non-direct managed build attempt that will produce the observation.
        attempt: BuildAttempt,
    },
}

impl BrokeredInvocationCapabilityRequestV2 {
    /// Constructs the observer request only from nonzero exact identities.
    pub fn release_with_source_isa_observer(
        config_identity: [u8; 32],
        unit_identity: [u8; 32],
        attempt: BuildAttempt,
    ) -> Result<Self, BrokeredInvocationCapabilityCodecErrorV2> {
        if config_identity == [0; 32]
            || unit_identity == [0; 32]
            || attempt.session() == BuildSession::DIRECT
        {
            return Err(BrokeredInvocationCapabilityCodecErrorV2::InvalidClaim);
        }
        Ok(Self::ReleaseWithSourceIsaObserver {
            config_identity,
            unit_identity,
            attempt,
        })
    }

    /// Encodes one canonical fixed-width V2 request.
    pub fn encode(self) -> [u8; BROKERED_INVOCATION_REQUEST_BYTES_V2] {
        let mut encoded = [0_u8; BROKERED_INVOCATION_REQUEST_BYTES_V2];
        encoded[..REQUEST_MAGIC_V2.len()].copy_from_slice(REQUEST_MAGIC_V2);
        encoded[REQUEST_MAGIC_V2.len()] = RELEASE_WITH_SOURCE_ISA_OBSERVER_TAG_V2;
        let Self::ReleaseWithSourceIsaObserver {
            config_identity,
            unit_identity,
            attempt,
        } = self;
        let mut offset = HEADER_BYTES_V2;
        encoded[offset..offset + 32].copy_from_slice(&config_identity);
        offset += 32;
        encoded[offset..offset + 32].copy_from_slice(&unit_identity);
        offset += 32;
        encoded[offset..offset + 8].copy_from_slice(&attempt.generation().to_le_bytes());
        offset += 8;
        encoded[offset..offset + 16].copy_from_slice(attempt.session().as_bytes());
        offset += 16;
        encoded[offset..offset + 32].copy_from_slice(attempt.invocation().as_bytes());
        encoded
    }

    /// Decodes one canonical fixed-width V2 request.
    pub fn decode(encoded: &[u8]) -> Result<Self, BrokeredInvocationCapabilityCodecErrorV2> {
        if encoded.len() != BROKERED_INVOCATION_REQUEST_BYTES_V2
            || &encoded[..REQUEST_MAGIC_V2.len()] != REQUEST_MAGIC_V2
            || encoded[REQUEST_MAGIC_V2.len()] != RELEASE_WITH_SOURCE_ISA_OBSERVER_TAG_V2
            || encoded[REQUEST_MAGIC_V2.len() + 1..HEADER_BYTES_V2]
                .iter()
                .any(|byte| *byte != 0)
        {
            return Err(BrokeredInvocationCapabilityCodecErrorV2::Malformed);
        }
        let mut offset = HEADER_BYTES_V2;
        let config_identity = encoded[offset..offset + 32]
            .try_into()
            .expect("fixed config identity field");
        offset += 32;
        let unit_identity = encoded[offset..offset + 32]
            .try_into()
            .expect("fixed unit identity field");
        offset += 32;
        let generation = u64::from_le_bytes(
            encoded[offset..offset + 8]
                .try_into()
                .expect("fixed generation field"),
        );
        offset += 8;
        let session = BuildSession::from_bytes(
            encoded[offset..offset + 16]
                .try_into()
                .expect("fixed session field"),
        );
        offset += 16;
        let invocation = BuildInvocation::from_bytes(
            encoded[offset..offset + 32]
                .try_into()
                .expect("fixed invocation field"),
        );
        let attempt = BuildAttempt::new(generation, session, invocation)
            .map_err(|_| BrokeredInvocationCapabilityCodecErrorV2::InvalidClaim)?;
        Self::release_with_source_isa_observer(config_identity, unit_identity, attempt)
    }

    /// Returns the exact configuration identity carried by this request.
    pub const fn config_identity(self) -> [u8; 32] {
        let Self::ReleaseWithSourceIsaObserver {
            config_identity, ..
        } = self;
        config_identity
    }

    /// Returns the exact selected-unit identity carried by this request.
    pub const fn unit_identity(self) -> [u8; 32] {
        let Self::ReleaseWithSourceIsaObserver { unit_identity, .. } = self;
        unit_identity
    }

    /// Returns the exact non-direct managed build attempt carried by this request.
    pub const fn attempt(self) -> BuildAttempt {
        let Self::ReleaseWithSourceIsaObserver { attempt, .. } = self;
        attempt
    }
}

/// Canonical V2 brokered invocation request failure.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BrokeredInvocationCapabilityCodecErrorV2 {
    /// The request has invalid framing, tags, reserved bytes, or length.
    Malformed,
    /// The request has a zero identity or direct/malformed build attempt.
    InvalidClaim,
}

impl fmt::Display for BrokeredInvocationCapabilityCodecErrorV2 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Malformed => formatter.write_str("malformed V2 brokered invocation request"),
            Self::InvalidClaim => {
                formatter.write_str("invalid V2 brokered invocation observer claim")
            }
        }
    }
}

impl Error for BrokeredInvocationCapabilityCodecErrorV2 {}

#[cfg(test)]
mod tests {
    use super::*;

    fn claim() -> BrokeredInvocationCapabilityClaimV1 {
        let attempt = BuildAttempt::new(
            7,
            BuildSession::from_bytes([0x11; 16]),
            BuildInvocation::from_bytes([0x22; 32]),
        )
        .unwrap();
        BrokeredInvocationCapabilityClaimV1::new(attempt, [0x22; 32]).unwrap()
    }

    #[test]
    fn requests_round_trip_and_bind_attempt_to_argv() {
        for request in [
            BrokeredInvocationCapabilityRequestV1::Release,
            BrokeredInvocationCapabilityRequestV1::Prepare(claim()),
            BrokeredInvocationCapabilityRequestV1::Consume(claim()),
        ] {
            let encoded = request.encode();
            assert_eq!(
                BrokeredInvocationCapabilityRequestV1::decode(&encoded),
                Ok(request)
            );
        }
        assert!(BrokeredInvocationCapabilityClaimV1::new(claim().attempt(), [0x23; 32]).is_err());
    }

    #[test]
    fn every_single_byte_mutation_fails_or_changes_the_request() {
        let request = BrokeredInvocationCapabilityRequestV1::Prepare(claim());
        let encoded = request.encode();
        for index in 0..encoded.len() {
            let mut changed = encoded;
            changed[index] ^= 1;
            assert_ne!(
                BrokeredInvocationCapabilityRequestV1::decode(&changed),
                Ok(request),
                "byte {index} was not bound"
            );
        }
    }

    #[test]
    fn v1_wire_vectors_are_frozen_and_v2_is_a_distinct_fixed_width_request() {
        assert_eq!(BROKERED_INVOCATION_REQUEST_BYTES_V1, 104);
        let release = BrokeredInvocationCapabilityRequestV1::Release.encode();
        assert_eq!(&release[..8], b"F2BRKIV1");
        assert_eq!(release[8], RELEASE_TAG_V1);
        assert!(release[9..].iter().all(|byte| *byte == 0));

        let request = BrokeredInvocationCapabilityRequestV2::release_with_source_isa_observer(
            [0x31; 32],
            [0x42; 32],
            claim().attempt(),
        )
        .unwrap();
        let encoded = request.encode();
        assert_eq!(BROKERED_INVOCATION_REQUEST_BYTES_V2, 136);
        assert_eq!(&encoded[..8], b"F2BRKIV2");
        assert_eq!(
            BrokeredInvocationCapabilityRequestV2::decode(&encoded),
            Ok(request)
        );
        assert_eq!(request.config_identity(), [0x31; 32]);
        assert_eq!(request.unit_identity(), [0x42; 32]);
        assert_eq!(request.attempt(), claim().attempt());
    }

    #[test]
    fn v2_request_rejects_zero_claims_and_every_single_byte_mutation() {
        assert!(
            BrokeredInvocationCapabilityRequestV2::release_with_source_isa_observer(
                [0; 32],
                [1; 32],
                claim().attempt()
            )
            .is_err()
        );
        assert!(
            BrokeredInvocationCapabilityRequestV2::release_with_source_isa_observer(
                [1; 32],
                [0; 32],
                claim().attempt()
            )
            .is_err()
        );
        let direct = BuildAttempt::new(1, BuildSession::DIRECT, BuildInvocation::DIRECT).unwrap();
        assert!(
            BrokeredInvocationCapabilityRequestV2::release_with_source_isa_observer(
                [1; 32], [2; 32], direct
            )
            .is_err()
        );
        let request = BrokeredInvocationCapabilityRequestV2::release_with_source_isa_observer(
            [0x31; 32],
            [0x42; 32],
            claim().attempt(),
        )
        .unwrap();
        let encoded = request.encode();
        for index in 0..encoded.len() {
            let mut changed = encoded;
            changed[index] ^= 1;
            assert_ne!(
                BrokeredInvocationCapabilityRequestV2::decode(&changed),
                Ok(request),
                "byte {index} was not bound"
            );
        }
    }
}
