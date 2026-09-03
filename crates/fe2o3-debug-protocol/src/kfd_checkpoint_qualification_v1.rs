//! Canonical, inert qualification receipts for one direct-KFD opaque checkpoint.
//!
//! A receipt contains only redacted correlation commitments and public-header
//! relative ranges. Decoding one never authenticates its producer and grants no
//! live debugger, execution, resume, or target-memory authority.

use std::collections::BTreeSet;
use std::fmt;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::OpaqueIdentityV1;

pub const KFD_OPAQUE_CHECKPOINT_QUALIFICATION_SCHEMA_V1: &str =
    "fe2o3-direct-kfd-opaque-checkpoint-qualification-v1";
pub const KFD_OPAQUE_CHECKPOINT_QUALIFICATION_SCHEMA_VERSION_V1: u16 = 1;
pub const MAX_KFD_OPAQUE_CHECKPOINT_QUALIFICATION_BYTES_V1: usize = 64 * 1024;
pub const KFD_OPAQUE_CHECKPOINT_GFX942_TARGET_VERSION_V1: u32 = 90_402;
pub const KFD_OPAQUE_CHECKPOINT_GFX942_XCC_COUNT_V1: u32 = 8;
pub const KFD_OPAQUE_CHECKPOINT_GFX942_CONTEXT_BYTES_PER_XCC_V1: u32 = 0x162_1000;
pub const MAX_KFD_OPAQUE_CHECKPOINT_CAPTURE_BYTES_V1: u64 =
    KFD_OPAQUE_CHECKPOINT_GFX942_CONTEXT_BYTES_PER_XCC_V1 as u64
        * KFD_OPAQUE_CHECKPOINT_GFX942_XCC_COUNT_V1 as u64;
pub const KFD_OPAQUE_CHECKPOINT_RANGE_SLOTS_V1: usize = 16;

const KFD_CONTEXT_HEADER_BYTES_V1: u32 = 40;
const RECEIPT_IDENTITY_DOMAIN_V1: &[u8] = b"fe2o3-direct-kfd-opaque-checkpoint-qualification-v1\0";
const PRODUCER_MANIFEST_SHA256_V1: [u8; 32] = [
    0x18, 0xfd, 0xfd, 0x09, 0xa0, 0x75, 0xea, 0x73, 0xd0, 0xe7, 0xf7, 0x31, 0x95, 0x4d, 0x0a, 0x06,
    0x81, 0x17, 0x2c, 0xff, 0x16, 0x30, 0x82, 0xc3, 0x69, 0xa3, 0xa3, 0xf5, 0x09, 0x49, 0x22, 0x58,
];

const REQUIRED_UNAVAILABLE_V1: [KfdOpaqueCheckpointUnavailableV1; 6] = [
    KfdOpaqueCheckpointUnavailableV1::DecodedWave,
    KfdOpaqueCheckpointUnavailableV1::DecodedLane,
    KfdOpaqueCheckpointUnavailableV1::Register,
    KfdOpaqueCheckpointUnavailableV1::ProgramCounter,
    KfdOpaqueCheckpointUnavailableV1::Source,
    KfdOpaqueCheckpointUnavailableV1::TargetMemory,
];

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum KfdOpaqueCheckpointQualificationSchemaV1 {
    #[serde(rename = "fe2o3-direct-kfd-opaque-checkpoint-qualification-v1")]
    V1,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum KfdOpaqueCheckpointQualificationOriginV1 {
    CallerBoundDirectKfdObservation,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum KfdOpaqueCheckpointRangeKindV1 {
    ControlStack,
    WaveState,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "status", rename_all = "snake_case", deny_unknown_fields)]
pub enum KfdOpaqueCheckpointRangeContentV1 {
    Empty,
    Complete { content_identity: OpaqueIdentityV1 },
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct KfdOpaqueCheckpointRangeSlotV1 {
    pub xcc_ordinal: u8,
    pub kind: KfdOpaqueCheckpointRangeKindV1,
    pub offset: u32,
    pub bytes: u32,
    pub content: KfdOpaqueCheckpointRangeContentV1,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum KfdOpaqueCheckpointUnavailableV1 {
    DecodedWave,
    DecodedLane,
    Register,
    ProgramCounter,
    Source,
    TargetMemory,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct KfdOpaqueCheckpointQualificationTruthV1 {
    pub origin: KfdOpaqueCheckpointQualificationOriginV1,
    pub session_local_prior_suspension_retained: bool,
    pub queue_device_snapshots_reobserved_equal: bool,
    pub adjacent_segment_reads_equal: bool,
    pub final_headers_reread_equal: bool,
    pub private_bytes_exposed: bool,
    pub raw_addresses_exposed: bool,
    pub native_ids_exposed: bool,
    pub handles_or_descriptors_exposed: bool,
    pub live_selectors_exposed: bool,
    pub coherent_stopped_interval: bool,
    pub runtime_reobserved: bool,
    pub suspension_reobserved: bool,
    pub physical_execution_authenticated: bool,
    pub grants_observation_authority: bool,
    pub grants_execution_authority: bool,
    pub grants_resume_authority: bool,
    pub grants_memory_authority: bool,
}

impl KfdOpaqueCheckpointQualificationTruthV1 {
    pub const fn caller_bound_direct_kfd() -> Self {
        Self {
            origin: KfdOpaqueCheckpointQualificationOriginV1::CallerBoundDirectKfdObservation,
            session_local_prior_suspension_retained: true,
            queue_device_snapshots_reobserved_equal: true,
            adjacent_segment_reads_equal: true,
            final_headers_reread_equal: true,
            private_bytes_exposed: false,
            raw_addresses_exposed: false,
            native_ids_exposed: false,
            handles_or_descriptors_exposed: false,
            live_selectors_exposed: false,
            coherent_stopped_interval: false,
            runtime_reobserved: false,
            suspension_reobserved: false,
            physical_execution_authenticated: false,
            grants_observation_authority: false,
            grants_execution_authority: false,
            grants_resume_authority: false,
            grants_memory_authority: false,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct KfdOpaqueCheckpointQualificationObservationV1 {
    pub producer_manifest_sha256: OpaqueIdentityV1,
    pub gfx_target_version: u32,
    pub xcc_count: u32,
    pub context_bytes_per_xcc: u32,
    pub stopped_snapshot_identity: OpaqueIdentityV1,
    pub queue_observation_identity: OpaqueIdentityV1,
    pub device_observation_identity: OpaqueIdentityV1,
    pub context_save_identity: OpaqueIdentityV1,
    pub checkpoint_identity: OpaqueIdentityV1,
    pub checkpoint_content_identity: OpaqueIdentityV1,
    pub capture_limit_bytes: u64,
    pub captured_bytes: u64,
    pub ranges: Vec<KfdOpaqueCheckpointRangeSlotV1>,
    pub unavailable: Vec<KfdOpaqueCheckpointUnavailableV1>,
    pub truth: KfdOpaqueCheckpointQualificationTruthV1,
}

impl KfdOpaqueCheckpointQualificationObservationV1 {
    pub fn validate(&self) -> Result<(), KfdOpaqueCheckpointQualificationErrorV1> {
        if self.producer_manifest_sha256.as_bytes() != PRODUCER_MANIFEST_SHA256_V1 {
            return Err(KfdOpaqueCheckpointQualificationErrorV1::ProducerManifestMismatch);
        }
        if self.gfx_target_version != KFD_OPAQUE_CHECKPOINT_GFX942_TARGET_VERSION_V1
            || self.xcc_count != KFD_OPAQUE_CHECKPOINT_GFX942_XCC_COUNT_V1
            || self.context_bytes_per_xcc != KFD_OPAQUE_CHECKPOINT_GFX942_CONTEXT_BYTES_PER_XCC_V1
        {
            return Err(KfdOpaqueCheckpointQualificationErrorV1::UnsupportedTarget);
        }
        if self.capture_limit_bytes == 0
            || self.capture_limit_bytes > MAX_KFD_OPAQUE_CHECKPOINT_CAPTURE_BYTES_V1
            || self.captured_bytes == 0
            || self.captured_bytes > self.capture_limit_bytes
        {
            return Err(KfdOpaqueCheckpointQualificationErrorV1::InvalidCaptureBounds);
        }
        if self.ranges.len() != KFD_OPAQUE_CHECKPOINT_RANGE_SLOTS_V1 {
            return Err(KfdOpaqueCheckpointQualificationErrorV1::InvalidRangeRoster);
        }
        if self.unavailable.as_slice() != REQUIRED_UNAVAILABLE_V1 {
            return Err(KfdOpaqueCheckpointQualificationErrorV1::InvalidUnavailableRoster);
        }
        if self.truth != KfdOpaqueCheckpointQualificationTruthV1::caller_bound_direct_kfd() {
            return Err(KfdOpaqueCheckpointQualificationErrorV1::InvalidTruthBoundary);
        }

        let mut identities = BTreeSet::from([
            self.stopped_snapshot_identity,
            self.queue_observation_identity,
            self.device_observation_identity,
            self.context_save_identity,
            self.checkpoint_identity,
            self.checkpoint_content_identity,
        ]);
        if identities.len() != 6 {
            return Err(KfdOpaqueCheckpointQualificationErrorV1::DuplicateIdentity);
        }

        let mut captured_bytes = 0_u64;
        let mut nonempty_ranges = 0_usize;
        for (index, slot) in self.ranges.iter().copied().enumerate() {
            let expected_xcc = u8::try_from(index / 2)
                .map_err(|_| KfdOpaqueCheckpointQualificationErrorV1::InvalidRangeRoster)?;
            let expected_kind = if index % 2 == 0 {
                KfdOpaqueCheckpointRangeKindV1::ControlStack
            } else {
                KfdOpaqueCheckpointRangeKindV1::WaveState
            };
            if slot.xcc_ordinal != expected_xcc || slot.kind != expected_kind {
                return Err(KfdOpaqueCheckpointQualificationErrorV1::InvalidRangeRoster);
            }
            validate_range(slot.offset, slot.bytes, self.context_bytes_per_xcc)?;
            match (slot.bytes, slot.content) {
                (0, KfdOpaqueCheckpointRangeContentV1::Empty) => {}
                (0, KfdOpaqueCheckpointRangeContentV1::Complete { .. })
                | (_, KfdOpaqueCheckpointRangeContentV1::Empty) => {
                    return Err(KfdOpaqueCheckpointQualificationErrorV1::InvalidRangeContent);
                }
                (bytes, KfdOpaqueCheckpointRangeContentV1::Complete { content_identity }) => {
                    if !identities.insert(content_identity) {
                        return Err(KfdOpaqueCheckpointQualificationErrorV1::DuplicateIdentity);
                    }
                    captured_bytes = captured_bytes
                        .checked_add(u64::from(bytes))
                        .ok_or(KfdOpaqueCheckpointQualificationErrorV1::InvalidCaptureBounds)?;
                    nonempty_ranges += 1;
                }
            }
        }
        for pair in self.ranges.chunks_exact(2) {
            if ranges_overlap(pair[0], pair[1]) {
                return Err(KfdOpaqueCheckpointQualificationErrorV1::OverlappingRanges);
            }
        }
        if nonempty_ranges == 0 || captured_bytes != self.captured_bytes {
            return Err(KfdOpaqueCheckpointQualificationErrorV1::InvalidCaptureBounds);
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct KfdOpaqueCheckpointQualificationReceiptV1 {
    pub schema: KfdOpaqueCheckpointQualificationSchemaV1,
    pub schema_version: u16,
    pub receipt_identity: OpaqueIdentityV1,
    pub observation: KfdOpaqueCheckpointQualificationObservationV1,
}

impl KfdOpaqueCheckpointQualificationReceiptV1 {
    pub fn new(
        observation: KfdOpaqueCheckpointQualificationObservationV1,
    ) -> Result<Self, KfdOpaqueCheckpointQualificationErrorV1> {
        observation.validate()?;
        let receipt_identity = expected_receipt_identity(&observation)?;
        Ok(Self {
            schema: KfdOpaqueCheckpointQualificationSchemaV1::V1,
            schema_version: KFD_OPAQUE_CHECKPOINT_QUALIFICATION_SCHEMA_VERSION_V1,
            receipt_identity,
            observation,
        })
    }

    pub fn validate(&self) -> Result<(), KfdOpaqueCheckpointQualificationErrorV1> {
        if self.schema != KfdOpaqueCheckpointQualificationSchemaV1::V1
            || self.schema_version != KFD_OPAQUE_CHECKPOINT_QUALIFICATION_SCHEMA_VERSION_V1
        {
            return Err(KfdOpaqueCheckpointQualificationErrorV1::UnsupportedVersion);
        }
        self.observation.validate()?;
        if self.receipt_identity != expected_receipt_identity(&self.observation)? {
            return Err(KfdOpaqueCheckpointQualificationErrorV1::ReceiptIdentityMismatch);
        }
        Ok(())
    }

    pub const fn grants_observation_authority(&self) -> bool {
        false
    }

    pub const fn grants_execution_authority(&self) -> bool {
        false
    }

    pub const fn grants_resume_authority(&self) -> bool {
        false
    }

    pub const fn grants_memory_authority(&self) -> bool {
        false
    }
}

#[derive(Serialize)]
#[serde(deny_unknown_fields)]
struct ReceiptIdentityPreimageV1<'a> {
    schema: KfdOpaqueCheckpointQualificationSchemaV1,
    schema_version: u16,
    observation: &'a KfdOpaqueCheckpointQualificationObservationV1,
}

fn expected_receipt_identity(
    observation: &KfdOpaqueCheckpointQualificationObservationV1,
) -> Result<OpaqueIdentityV1, KfdOpaqueCheckpointQualificationErrorV1> {
    let preimage = serde_json::to_vec(&ReceiptIdentityPreimageV1 {
        schema: KfdOpaqueCheckpointQualificationSchemaV1::V1,
        schema_version: KFD_OPAQUE_CHECKPOINT_QUALIFICATION_SCHEMA_VERSION_V1,
        observation,
    })
    .map_err(|_| KfdOpaqueCheckpointQualificationErrorV1::JsonEncode)?;
    let mut hash = Sha256::new();
    hash.update(RECEIPT_IDENTITY_DOMAIN_V1);
    hash.update(
        u64::try_from(preimage.len())
            .map_err(|_| KfdOpaqueCheckpointQualificationErrorV1::SizeOutOfRange)?
            .to_le_bytes(),
    );
    hash.update(preimage);
    OpaqueIdentityV1::new(hash.finalize().into())
        .map_err(|_| KfdOpaqueCheckpointQualificationErrorV1::ReceiptIdentityMismatch)
}

fn validate_range(
    offset: u32,
    bytes: u32,
    limit: u32,
) -> Result<(), KfdOpaqueCheckpointQualificationErrorV1> {
    if bytes == 0 {
        if offset == 0 || (offset >= KFD_CONTEXT_HEADER_BYTES_V1 && offset <= limit) {
            return Ok(());
        }
        return Err(KfdOpaqueCheckpointQualificationErrorV1::InvalidRange);
    }
    let end = offset
        .checked_add(bytes)
        .ok_or(KfdOpaqueCheckpointQualificationErrorV1::InvalidRange)?;
    if offset < KFD_CONTEXT_HEADER_BYTES_V1 || end > limit {
        return Err(KfdOpaqueCheckpointQualificationErrorV1::InvalidRange);
    }
    Ok(())
}

fn ranges_overlap(
    first: KfdOpaqueCheckpointRangeSlotV1,
    second: KfdOpaqueCheckpointRangeSlotV1,
) -> bool {
    if first.bytes == 0 || second.bytes == 0 {
        return false;
    }
    let Some(first_end) = first.offset.checked_add(first.bytes) else {
        return true;
    };
    let Some(second_end) = second.offset.checked_add(second.bytes) else {
        return true;
    };
    first.offset < second_end && second.offset < first_end
}

pub fn encode_kfd_opaque_checkpoint_qualification_v1(
    receipt: &KfdOpaqueCheckpointQualificationReceiptV1,
) -> Result<Vec<u8>, KfdOpaqueCheckpointQualificationErrorV1> {
    receipt.validate()?;
    let bytes = serde_json::to_vec(receipt)
        .map_err(|_| KfdOpaqueCheckpointQualificationErrorV1::JsonEncode)?;
    validate_encoded_size(bytes.len())?;
    Ok(bytes)
}

pub fn decode_kfd_opaque_checkpoint_qualification_v1(
    bytes: &[u8],
) -> Result<KfdOpaqueCheckpointQualificationReceiptV1, KfdOpaqueCheckpointQualificationErrorV1> {
    validate_encoded_size(bytes.len())?;
    let receipt: KfdOpaqueCheckpointQualificationReceiptV1 = serde_json::from_slice(bytes)
        .map_err(|_| KfdOpaqueCheckpointQualificationErrorV1::JsonDecode)?;
    receipt.validate()?;
    if serde_json::to_vec(&receipt)
        .map_err(|_| KfdOpaqueCheckpointQualificationErrorV1::JsonEncode)?
        != bytes
    {
        return Err(KfdOpaqueCheckpointQualificationErrorV1::NonCanonicalEncoding);
    }
    Ok(receipt)
}

fn validate_encoded_size(size: usize) -> Result<(), KfdOpaqueCheckpointQualificationErrorV1> {
    if size == 0 || size > MAX_KFD_OPAQUE_CHECKPOINT_QUALIFICATION_BYTES_V1 {
        return Err(KfdOpaqueCheckpointQualificationErrorV1::SizeOutOfRange);
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum KfdOpaqueCheckpointQualificationErrorV1 {
    SizeOutOfRange,
    JsonEncode,
    JsonDecode,
    NonCanonicalEncoding,
    UnsupportedVersion,
    ProducerManifestMismatch,
    UnsupportedTarget,
    InvalidCaptureBounds,
    InvalidRangeRoster,
    InvalidRange,
    OverlappingRanges,
    InvalidRangeContent,
    DuplicateIdentity,
    InvalidUnavailableRoster,
    InvalidTruthBoundary,
    ReceiptIdentityMismatch,
}

impl fmt::Display for KfdOpaqueCheckpointQualificationErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = match self {
            Self::SizeOutOfRange => "checkpoint qualification receipt size is out of range",
            Self::JsonEncode => "checkpoint qualification receipt JSON encoding failed",
            Self::JsonDecode => "checkpoint qualification receipt JSON decoding failed",
            Self::NonCanonicalEncoding => "checkpoint qualification receipt is not canonical JSON",
            Self::UnsupportedVersion => "checkpoint qualification receipt version is unsupported",
            Self::ProducerManifestMismatch => "checkpoint producer manifest does not match",
            Self::UnsupportedTarget => "checkpoint qualification target is unsupported",
            Self::InvalidCaptureBounds => "checkpoint qualification capture bounds are invalid",
            Self::InvalidRangeRoster => "checkpoint qualification range roster is invalid",
            Self::InvalidRange => "checkpoint qualification range is invalid",
            Self::OverlappingRanges => "checkpoint qualification ranges overlap",
            Self::InvalidRangeContent => "checkpoint qualification range content is invalid",
            Self::DuplicateIdentity => "checkpoint qualification identity domains overlap",
            Self::InvalidUnavailableRoster => "checkpoint unavailable roster is invalid",
            Self::InvalidTruthBoundary => "checkpoint qualification truth boundary is invalid",
            Self::ReceiptIdentityMismatch => "checkpoint qualification receipt identity mismatches",
        };
        formatter.write_str(message)
    }
}

impl std::error::Error for KfdOpaqueCheckpointQualificationErrorV1 {}

#[cfg(test)]
mod tests {
    use super::*;

    fn identity(byte: u8) -> OpaqueIdentityV1 {
        OpaqueIdentityV1::new([byte; 32]).unwrap()
    }

    fn observation() -> KfdOpaqueCheckpointQualificationObservationV1 {
        let mut ranges = Vec::with_capacity(KFD_OPAQUE_CHECKPOINT_RANGE_SLOTS_V1);
        for xcc in 0..8_u8 {
            for kind in [
                KfdOpaqueCheckpointRangeKindV1::ControlStack,
                KfdOpaqueCheckpointRangeKindV1::WaveState,
            ] {
                ranges.push(KfdOpaqueCheckpointRangeSlotV1 {
                    xcc_ordinal: xcc,
                    kind,
                    offset: 0,
                    bytes: 0,
                    content: KfdOpaqueCheckpointRangeContentV1::Empty,
                });
            }
        }
        ranges[0] = KfdOpaqueCheckpointRangeSlotV1 {
            xcc_ordinal: 0,
            kind: KfdOpaqueCheckpointRangeKindV1::ControlStack,
            offset: 64,
            bytes: 128,
            content: KfdOpaqueCheckpointRangeContentV1::Complete {
                content_identity: identity(7),
            },
        };
        ranges[1] = KfdOpaqueCheckpointRangeSlotV1 {
            xcc_ordinal: 0,
            kind: KfdOpaqueCheckpointRangeKindV1::WaveState,
            offset: 4096,
            bytes: 256,
            content: KfdOpaqueCheckpointRangeContentV1::Complete {
                content_identity: identity(8),
            },
        };
        KfdOpaqueCheckpointQualificationObservationV1 {
            producer_manifest_sha256: identity_from_bytes(PRODUCER_MANIFEST_SHA256_V1),
            gfx_target_version: KFD_OPAQUE_CHECKPOINT_GFX942_TARGET_VERSION_V1,
            xcc_count: KFD_OPAQUE_CHECKPOINT_GFX942_XCC_COUNT_V1,
            context_bytes_per_xcc: KFD_OPAQUE_CHECKPOINT_GFX942_CONTEXT_BYTES_PER_XCC_V1,
            stopped_snapshot_identity: identity(1),
            queue_observation_identity: identity(2),
            device_observation_identity: identity(3),
            context_save_identity: identity(4),
            checkpoint_identity: identity(5),
            checkpoint_content_identity: identity(6),
            capture_limit_bytes: 1024,
            captured_bytes: 384,
            ranges,
            unavailable: REQUIRED_UNAVAILABLE_V1.to_vec(),
            truth: KfdOpaqueCheckpointQualificationTruthV1::caller_bound_direct_kfd(),
        }
    }

    fn identity_from_bytes(bytes: [u8; 32]) -> OpaqueIdentityV1 {
        OpaqueIdentityV1::new(bytes).unwrap()
    }

    fn receipt() -> KfdOpaqueCheckpointQualificationReceiptV1 {
        KfdOpaqueCheckpointQualificationReceiptV1::new(observation()).unwrap()
    }

    #[test]
    fn canonical_round_trip_binds_the_complete_redacted_observation() {
        let receipt = receipt();
        let bytes = encode_kfd_opaque_checkpoint_qualification_v1(&receipt).unwrap();
        assert_eq!(
            decode_kfd_opaque_checkpoint_qualification_v1(&bytes).unwrap(),
            receipt
        );
        assert!(bytes.len() < MAX_KFD_OPAQUE_CHECKPOINT_QUALIFICATION_BYTES_V1);
        assert!(!receipt.grants_observation_authority());
        assert!(!receipt.grants_execution_authority());
        assert!(!receipt.grants_resume_authority());
        assert!(!receipt.grants_memory_authority());

        let mut noncanonical = bytes;
        noncanonical.push(b'\n');
        assert_eq!(
            decode_kfd_opaque_checkpoint_qualification_v1(&noncanonical),
            Err(KfdOpaqueCheckpointQualificationErrorV1::NonCanonicalEncoding)
        );
    }

    #[test]
    fn receipt_identity_rejects_public_field_substitution() {
        let receipt = receipt();
        let bytes = encode_kfd_opaque_checkpoint_qualification_v1(&receipt).unwrap();
        let mut value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        value["observation"]["ranges"][0]["offset"] = serde_json::json!(80);
        assert_eq!(
            decode_kfd_opaque_checkpoint_qualification_v1(&serde_json::to_vec(&value).unwrap()),
            Err(KfdOpaqueCheckpointQualificationErrorV1::ReceiptIdentityMismatch)
        );
    }

    #[test]
    fn target_truth_unavailable_and_identity_substitutions_fail_closed() {
        let mut target = observation();
        target.gfx_target_version -= 1;
        assert_eq!(
            target.validate(),
            Err(KfdOpaqueCheckpointQualificationErrorV1::UnsupportedTarget)
        );

        let mut truth = observation();
        truth.truth.physical_execution_authenticated = true;
        assert_eq!(
            truth.validate(),
            Err(KfdOpaqueCheckpointQualificationErrorV1::InvalidTruthBoundary)
        );

        let mut unavailable = observation();
        unavailable.unavailable.swap(0, 1);
        assert_eq!(
            unavailable.validate(),
            Err(KfdOpaqueCheckpointQualificationErrorV1::InvalidUnavailableRoster)
        );

        let mut duplicate = observation();
        duplicate.device_observation_identity = duplicate.queue_observation_identity;
        assert_eq!(
            duplicate.validate(),
            Err(KfdOpaqueCheckpointQualificationErrorV1::DuplicateIdentity)
        );

        let mut segment_duplicate = observation();
        segment_duplicate.ranges[1].content = KfdOpaqueCheckpointRangeContentV1::Complete {
            content_identity: identity(7),
        };
        assert_eq!(
            segment_duplicate.validate(),
            Err(KfdOpaqueCheckpointQualificationErrorV1::DuplicateIdentity)
        );
    }

    #[test]
    fn hostile_range_rosters_and_bounds_fail_closed() {
        let mut missing = observation();
        missing.ranges.pop();
        assert_eq!(
            missing.validate(),
            Err(KfdOpaqueCheckpointQualificationErrorV1::InvalidRangeRoster)
        );

        let mut reordered = observation();
        reordered.ranges.swap(0, 1);
        assert_eq!(
            reordered.validate(),
            Err(KfdOpaqueCheckpointQualificationErrorV1::InvalidRangeRoster)
        );

        let mut below_header = observation();
        below_header.ranges[0].offset = KFD_CONTEXT_HEADER_BYTES_V1 - 1;
        assert_eq!(
            below_header.validate(),
            Err(KfdOpaqueCheckpointQualificationErrorV1::InvalidRange)
        );

        let mut overflow = observation();
        overflow.ranges[0].offset = u32::MAX;
        assert_eq!(
            overflow.validate(),
            Err(KfdOpaqueCheckpointQualificationErrorV1::InvalidRange)
        );

        let mut overlap = observation();
        overlap.ranges[1].offset = 100;
        assert_eq!(
            overlap.validate(),
            Err(KfdOpaqueCheckpointQualificationErrorV1::OverlappingRanges)
        );

        let mut malformed_empty = observation();
        malformed_empty.ranges[2].offset = 1;
        assert_eq!(
            malformed_empty.validate(),
            Err(KfdOpaqueCheckpointQualificationErrorV1::InvalidRange)
        );
    }

    #[test]
    fn content_shapes_and_capture_totals_fail_closed() {
        let mut missing_content = observation();
        missing_content.ranges[0].content = KfdOpaqueCheckpointRangeContentV1::Empty;
        assert_eq!(
            missing_content.validate(),
            Err(KfdOpaqueCheckpointQualificationErrorV1::InvalidRangeContent)
        );

        let mut empty_claim = observation();
        empty_claim.ranges[2].content = KfdOpaqueCheckpointRangeContentV1::Complete {
            content_identity: identity(9),
        };
        assert_eq!(
            empty_claim.validate(),
            Err(KfdOpaqueCheckpointQualificationErrorV1::InvalidRangeContent)
        );

        let mut wrong_total = observation();
        wrong_total.captured_bytes += 1;
        assert_eq!(
            wrong_total.validate(),
            Err(KfdOpaqueCheckpointQualificationErrorV1::InvalidCaptureBounds)
        );

        let mut over_limit = observation();
        over_limit.capture_limit_bytes = 383;
        assert_eq!(
            over_limit.validate(),
            Err(KfdOpaqueCheckpointQualificationErrorV1::InvalidCaptureBounds)
        );

        let mut empty = observation();
        for slot in &mut empty.ranges {
            slot.offset = 0;
            slot.bytes = 0;
            slot.content = KfdOpaqueCheckpointRangeContentV1::Empty;
        }
        empty.captured_bytes = 0;
        assert_eq!(
            empty.validate(),
            Err(KfdOpaqueCheckpointQualificationErrorV1::InvalidCaptureBounds)
        );
    }

    #[test]
    fn unknown_fields_oversize_and_private_payload_shapes_are_rejected() {
        let bytes = encode_kfd_opaque_checkpoint_qualification_v1(&receipt()).unwrap();
        let mut value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        value["observation"]["native_queue_id"] = serde_json::json!(7);
        assert_eq!(
            decode_kfd_opaque_checkpoint_qualification_v1(&serde_json::to_vec(&value).unwrap()),
            Err(KfdOpaqueCheckpointQualificationErrorV1::JsonDecode)
        );

        let mut private = serde_json::from_slice::<serde_json::Value>(&bytes).unwrap();
        private["observation"]["ranges"][0]["content"]["private_bytes"] =
            serde_json::json!([1, 2, 3]);
        assert_eq!(
            decode_kfd_opaque_checkpoint_qualification_v1(&serde_json::to_vec(&private).unwrap()),
            Err(KfdOpaqueCheckpointQualificationErrorV1::JsonDecode)
        );

        assert_eq!(
            decode_kfd_opaque_checkpoint_qualification_v1(
                &[b'x'; MAX_KFD_OPAQUE_CHECKPOINT_QUALIFICATION_BYTES_V1 + 1]
            ),
            Err(KfdOpaqueCheckpointQualificationErrorV1::SizeOutOfRange)
        );
        let encoded = String::from_utf8(bytes).unwrap();
        for forbidden_key in [
            "\"pid\"",
            "\"gpu_id\"",
            "\"queue_id\"",
            "\"event_id\"",
            "\"address\"",
            "\"handle\"",
            "\"descriptor\"",
            "\"selector\"",
            "\"private_bytes\"",
        ] {
            assert!(!encoded.contains(forbidden_key), "leaked {forbidden_key}");
        }
    }
}
