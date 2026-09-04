//! Authority-free bounded source/ISA observation framing for production qualification.

use std::error::Error;
use std::fmt;

use fe2o3_kernel_ir::MAX_FUNCTIONS_V1;
use sha2::{Digest, Sha256};

const FRAME_MAGIC_V1: &[u8; 8] = b"F2SISUM1";
const FRAME_VERSION_V1: u16 = 1;
const FRAME_HEADER_BYTES_V1: usize = 16;
const FRAME_IDENTITY_BYTES_V1: usize = 32;
const FRAME_PREFIX_BYTES_V1: usize = 648;
const ADMITTED_PAYLOAD_BYTES_V1: usize = 464;
const COUNT_FIELDS_V1: usize = 12;
const FRAME_IDENTITY_DOMAIN_V1: &[u8] = b"FE2O3/SOURCE-ISA-SUMMARY-FRAME/V1\0";
const MAX_SOURCE_ISA_RECORDS_V1: u64 = 540_672;
const MAX_SOURCE_ISA_REFERENCES_V1: u64 = 1_016_800;
const MAX_PRODUCTION_STRUCTURAL_OPERATIONS_V1: u64 = 16 * 1024;

/// Inert fixed-width identity for the producer session carried by an observation.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct SourceIsaObservationSessionV1([u8; 16]);

impl SourceIsaObservationSessionV1 {
    /// The all-zero value, which is never valid in an admitted observation.
    pub const DIRECT: Self = Self([0; 16]);

    pub const fn from_bytes(bytes: [u8; 16]) -> Self {
        Self(bytes)
    }

    pub const fn as_bytes(&self) -> &[u8; 16] {
        &self.0
    }
}

impl fmt::Display for SourceIsaObservationSessionV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write_lower_hex(formatter, &self.0)
    }
}

/// Inert fixed-width identity for the producer invocation carried by an observation.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct SourceIsaObservationInvocationV1([u8; 32]);

impl SourceIsaObservationInvocationV1 {
    pub const DIRECT: Self = Self([0; 32]);

    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

impl fmt::Display for SourceIsaObservationInvocationV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write_lower_hex(formatter, &self.0)
    }
}

/// Inert attempt coordinates copied from a producer after authority release.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct SourceIsaObservationAttemptV1 {
    generation: u64,
    session: SourceIsaObservationSessionV1,
    invocation: SourceIsaObservationInvocationV1,
}

impl SourceIsaObservationAttemptV1 {
    pub fn new(
        generation: u64,
        session: SourceIsaObservationSessionV1,
        invocation: SourceIsaObservationInvocationV1,
    ) -> Result<Self, SourceIsaObservationFrameErrorV1> {
        if generation == 0
            || (session == SourceIsaObservationSessionV1::DIRECT)
                != (invocation == SourceIsaObservationInvocationV1::DIRECT)
        {
            return Err(SourceIsaObservationFrameErrorV1::InvalidClaim);
        }
        Ok(Self {
            generation,
            session,
            invocation,
        })
    }

    pub const fn generation(self) -> u64 {
        self.generation
    }

    pub const fn session(self) -> SourceIsaObservationSessionV1 {
        self.session
    }

    pub const fn invocation(self) -> SourceIsaObservationInvocationV1 {
        self.invocation
    }
}

fn write_lower_hex(formatter: &mut fmt::Formatter<'_>, bytes: &[u8]) -> fmt::Result {
    for byte in bytes {
        write!(formatter, "{byte:02x}")?;
    }
    Ok(())
}

pub const SOURCE_ISA_OBSERVATION_FRAME_BYTES_V1: usize =
    FRAME_PREFIX_BYTES_V1 + FRAME_IDENTITY_BYTES_V1;
pub const MAX_SOURCE_ISA_OBSERVATION_FRAME_BYTES_V1: usize = 4096;
const _: () =
    assert!(SOURCE_ISA_OBSERVATION_FRAME_BYTES_V1 <= MAX_SOURCE_ISA_OBSERVATION_FRAME_BYTES_V1);

const MAX_SOURCE_ISA_OBSERVATION_UNITS_V1: usize = 1024;
pub const SOURCE_ISA_COLLECTION_MAGIC_V1: &[u8; 8] = b"F2SICOL1";
const SOURCE_ISA_COLLECTION_VERSION_V1: u16 = 1;
pub const SOURCE_ISA_COLLECTION_HEADER_BYTES_V1: usize = 80;
pub const SOURCE_ISA_COLLECTION_IDENTITY_BYTES_V1: usize = 32;
pub const SOURCE_ISA_COLLECTION_IDENTITY_DOMAIN_V1: &[u8] =
    b"FE2O3/SOURCE-ISA-OBSERVATION-COLLECTION/V1\0";
pub const MAX_SOURCE_ISA_OBSERVATION_COLLECTION_BYTES_V1: usize =
    SOURCE_ISA_COLLECTION_HEADER_BYTES_V1
        + MAX_SOURCE_ISA_OBSERVATION_UNITS_V1 * SOURCE_ISA_OBSERVATION_FRAME_BYTES_V1
        + SOURCE_ISA_COLLECTION_IDENTITY_BYTES_V1;
pub const MAX_SOURCE_ISA_OBSERVATION_COLLECTION_HEX_BYTES_V1: usize =
    MAX_SOURCE_ISA_OBSERVATION_COLLECTION_BYTES_V1 * 2;
const _: () = assert!(MAX_SOURCE_ISA_OBSERVATION_COLLECTION_BYTES_V1 == 696_432);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u16)]
pub enum SourceIsaObservationTransportFailureV1 {
    #[allow(dead_code)]
    CollectorAlreadyFailed = 1,
    UnitBound = 2,
    AggregateByteBound = 3,
    ConflictingDuplicate = 4,
    RejectedFrame = 5,
    MissingSelectedUnits = 6,
    BrokerWorkerPanic = 7,
}

impl fmt::Display for SourceIsaObservationTransportFailureV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "source/ISA observation transport failure code {}",
            *self as u16
        )
    }
}

impl Error for SourceIsaObservationTransportFailureV1 {}

impl SourceIsaObservationTransportFailureV1 {
    pub const fn code(self) -> u16 {
        self as u16
    }

    fn from_code(code: u16) -> Result<Option<Self>, String> {
        match code {
            0 => Ok(None),
            1 => Ok(Some(Self::CollectorAlreadyFailed)),
            2 => Ok(Some(Self::UnitBound)),
            3 => Ok(Some(Self::AggregateByteBound)),
            4 => Ok(Some(Self::ConflictingDuplicate)),
            5 => Ok(Some(Self::RejectedFrame)),
            6 => Ok(Some(Self::MissingSelectedUnits)),
            7 => Ok(Some(Self::BrokerWorkerPanic)),
            _ => Err("source/ISA collection has an unknown failure code".to_owned()),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceIsaObservationCollectionV1 {
    config_identity: [u8; 32],
    session: SourceIsaObservationSessionV1,
    frames: Vec<([u8; 32], SourceIsaObservationFrameV1)>,
    missing_units: Vec<[u8; 32]>,
    failure: Option<SourceIsaObservationTransportFailureV1>,
}

impl SourceIsaObservationCollectionV1 {
    pub fn from_collected(
        config_identity: [u8; 32],
        session: SourceIsaObservationSessionV1,
        frames: Vec<([u8; 32], SourceIsaObservationFrameV1)>,
        missing_units: Vec<[u8; 32]>,
        failure: Option<SourceIsaObservationTransportFailureV1>,
    ) -> Self {
        Self {
            config_identity,
            session,
            frames,
            missing_units,
            failure,
        }
    }

    pub const fn config_identity(&self) -> [u8; 32] {
        self.config_identity
    }

    pub const fn session(&self) -> SourceIsaObservationSessionV1 {
        self.session
    }

    pub fn frames(&self) -> impl ExactSizeIterator<Item = &SourceIsaObservationFrameV1> {
        self.frames.iter().map(|(_, frame)| frame)
    }

    pub fn missing_units(&self) -> &[[u8; 32]] {
        &self.missing_units
    }

    pub const fn failure(&self) -> Option<SourceIsaObservationTransportFailureV1> {
        self.failure
    }

    pub fn encode_canonical(&self) -> Result<Vec<u8>, String> {
        self.validate_canonical()?;
        let total =
            source_isa_collection_encoded_length(self.frames.len(), self.missing_units.len())?;
        let total_u32 = u32::try_from(total)
            .map_err(|_| "source/ISA collection length is not representable".to_owned())?;
        let frame_count = u32::try_from(self.frames.len())
            .map_err(|_| "source/ISA collection frame count is not representable".to_owned())?;
        let missing_count = u32::try_from(self.missing_units.len()).map_err(|_| {
            "source/ISA collection missing-unit count is not representable".to_owned()
        })?;
        let mut encoded = Vec::new();
        encoded
            .try_reserve_exact(total)
            .map_err(|_| "cannot allocate bounded source/ISA collection bytes".to_owned())?;
        encoded.extend_from_slice(SOURCE_ISA_COLLECTION_MAGIC_V1);
        encoded.extend_from_slice(&SOURCE_ISA_COLLECTION_VERSION_V1.to_le_bytes());
        encoded.extend_from_slice(&(SOURCE_ISA_COLLECTION_HEADER_BYTES_V1 as u16).to_le_bytes());
        encoded.extend_from_slice(&total_u32.to_le_bytes());
        encoded.extend_from_slice(&frame_count.to_le_bytes());
        encoded.extend_from_slice(&missing_count.to_le_bytes());
        encoded.extend_from_slice(
            &self
                .failure
                .map_or(0, |failure| failure.code())
                .to_le_bytes(),
        );
        encoded.extend_from_slice(&0_u16.to_le_bytes());
        encoded.extend_from_slice(&0_u32.to_le_bytes());
        encoded.extend_from_slice(&self.config_identity());
        encoded.extend_from_slice(self.session().as_bytes());
        for (_, frame) in &self.frames {
            encoded.extend_from_slice(&frame.encode());
        }
        for unit in &self.missing_units {
            encoded.extend_from_slice(unit);
        }
        let mut digest = Sha256::new();
        digest.update(SOURCE_ISA_COLLECTION_IDENTITY_DOMAIN_V1);
        digest.update(&encoded);
        encoded.extend_from_slice(&digest.finalize());
        debug_assert_eq!(encoded.len(), total);
        Ok(encoded)
    }

    pub fn decode_canonical(encoded: &[u8]) -> Result<Self, String> {
        let minimum =
            SOURCE_ISA_COLLECTION_HEADER_BYTES_V1 + SOURCE_ISA_COLLECTION_IDENTITY_BYTES_V1;
        if encoded.len() < minimum
            || encoded.len() > MAX_SOURCE_ISA_OBSERVATION_COLLECTION_BYTES_V1
            || &encoded[..8] != SOURCE_ISA_COLLECTION_MAGIC_V1
            || u16::from_le_bytes(
                encoded[8..10]
                    .try_into()
                    .expect("fixed collection version field"),
            ) != SOURCE_ISA_COLLECTION_VERSION_V1
            || usize::from(u16::from_le_bytes(
                encoded[10..12]
                    .try_into()
                    .expect("fixed collection header field"),
            )) != SOURCE_ISA_COLLECTION_HEADER_BYTES_V1
            || usize::try_from(u32::from_le_bytes(
                encoded[12..16]
                    .try_into()
                    .expect("fixed collection length field"),
            ))
            .ok()
                != Some(encoded.len())
        {
            return Err("source/ISA collection has malformed framing".to_owned());
        }
        let frame_count = usize::try_from(u32::from_le_bytes(
            encoded[16..20]
                .try_into()
                .expect("fixed collection frame-count field"),
        ))
        .map_err(|_| "source/ISA collection frame count is not representable".to_owned())?;
        let missing_count = usize::try_from(u32::from_le_bytes(
            encoded[20..24]
                .try_into()
                .expect("fixed collection missing-count field"),
        ))
        .map_err(|_| "source/ISA collection missing count is not representable".to_owned())?;
        let expected = source_isa_collection_encoded_length(frame_count, missing_count)?;
        if expected != encoded.len() || encoded[26..32].iter().any(|byte| *byte != 0) {
            return Err("source/ISA collection has noncanonical bounds or truth claims".to_owned());
        }
        let failure = SourceIsaObservationTransportFailureV1::from_code(u16::from_le_bytes(
            encoded[24..26]
                .try_into()
                .expect("fixed collection failure field"),
        ))?;
        let config_identity = encoded[32..64]
            .try_into()
            .expect("fixed collection config field");
        let session = SourceIsaObservationSessionV1::from_bytes(
            encoded[64..80]
                .try_into()
                .expect("fixed collection session field"),
        );
        let identity_start = encoded.len() - SOURCE_ISA_COLLECTION_IDENTITY_BYTES_V1;
        let mut digest = Sha256::new();
        digest.update(SOURCE_ISA_COLLECTION_IDENTITY_DOMAIN_V1);
        digest.update(&encoded[..identity_start]);
        let identity: [u8; 32] = digest.finalize().into();
        let retained_identity: [u8; 32] = encoded[identity_start..]
            .try_into()
            .expect("fixed collection identity field");
        if retained_identity == [0; 32] || identity != retained_identity {
            return Err("source/ISA collection identity differs from its bytes".to_owned());
        }

        let mut frames = Vec::new();
        frames
            .try_reserve_exact(frame_count)
            .map_err(|_| "cannot allocate decoded source/ISA frames".to_owned())?;
        let mut cursor = SOURCE_ISA_COLLECTION_HEADER_BYTES_V1;
        for _ in 0..frame_count {
            let end = cursor + SOURCE_ISA_OBSERVATION_FRAME_BYTES_V1;
            let frame = SourceIsaObservationFrameV1::decode(&encoded[cursor..end])
                .map_err(|error| format!("invalid source/ISA collection frame: {error}"))?;
            frames.push((frame.context().unit(), frame));
            cursor = end;
        }
        let mut missing_units = Vec::new();
        missing_units
            .try_reserve_exact(missing_count)
            .map_err(|_| "cannot allocate decoded source/ISA missing units".to_owned())?;
        for _ in 0..missing_count {
            let end = cursor + 32;
            missing_units.push(
                encoded[cursor..end]
                    .try_into()
                    .expect("bounded missing-unit field"),
            );
            cursor = end;
        }
        if cursor != identity_start {
            return Err("source/ISA collection has trailing payload bytes".to_owned());
        }
        let collection = Self {
            config_identity,
            session,
            frames,
            missing_units,
            failure,
        };
        collection.validate_canonical()?;
        Ok(collection)
    }

    fn validate_canonical(&self) -> Result<(), String> {
        if self.config_identity == [0; 32]
            || self.session == SourceIsaObservationSessionV1::DIRECT
            || self.frames.windows(2).any(|pair| pair[0].0 >= pair[1].0)
            || self.frames.iter().any(|(unit, frame)| {
                *unit != frame.context().unit()
                    || frame.context().config() != self.config_identity
                    || frame.context().attempt().session() != self.session
            })
            || self.missing_units.contains(&[0; 32])
            || self.missing_units.windows(2).any(|pair| pair[0] >= pair[1])
            || self.missing_units.iter().any(|unit| {
                self.frames
                    .binary_search_by_key(unit, |(observed, _)| *observed)
                    .is_ok()
            })
            || (!self.missing_units.is_empty() && self.failure.is_none())
        {
            return Err("source/ISA collection is not canonical".to_owned());
        }
        source_isa_collection_encoded_length(self.frames.len(), self.missing_units.len())?;
        Ok(())
    }

    pub const fn grants_compiler_authority(&self) -> bool {
        false
    }

    pub const fn grants_publication_authority(&self) -> bool {
        false
    }

    pub const fn grants_runtime_authority(&self) -> bool {
        false
    }
}

pub fn source_isa_collection_encoded_length(
    frame_count: usize,
    missing_count: usize,
) -> Result<usize, String> {
    let unit_count = frame_count
        .checked_add(missing_count)
        .ok_or_else(|| "source/ISA collection unit count overflowed".to_owned())?;
    if unit_count > MAX_SOURCE_ISA_OBSERVATION_UNITS_V1 {
        return Err("source/ISA collection exceeds its unit bound".to_owned());
    }
    let frame_bytes = frame_count
        .checked_mul(SOURCE_ISA_OBSERVATION_FRAME_BYTES_V1)
        .ok_or_else(|| "source/ISA collection frame length overflowed".to_owned())?;
    let missing_bytes = missing_count
        .checked_mul(32)
        .ok_or_else(|| "source/ISA collection missing-unit length overflowed".to_owned())?;
    SOURCE_ISA_COLLECTION_HEADER_BYTES_V1
        .checked_add(frame_bytes)
        .and_then(|bytes| bytes.checked_add(missing_bytes))
        .and_then(|bytes| bytes.checked_add(SOURCE_ISA_COLLECTION_IDENTITY_BYTES_V1))
        .filter(|bytes| *bytes <= MAX_SOURCE_ISA_OBSERVATION_COLLECTION_BYTES_V1)
        .ok_or_else(|| "source/ISA collection exceeds its canonical bound".to_owned())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SourceIsaObservationContentIdentityV1 {
    sha256: [u8; 32],
    byte_len: u64,
}

impl SourceIsaObservationContentIdentityV1 {
    pub fn new(sha256: [u8; 32], byte_len: u64) -> Result<Self, SourceIsaObservationFrameErrorV1> {
        if sha256 == [0; 32] || byte_len == 0 {
            return Err(SourceIsaObservationFrameErrorV1::InvalidClaim);
        }
        Ok(Self { sha256, byte_len })
    }

    pub const fn sha256(self) -> [u8; 32] {
        self.sha256
    }

    pub const fn byte_len(self) -> u64 {
        self.byte_len
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum SourceIsaObservationTargetProfileV1 {
    Gfx942 = 1,
    Gfx950 = 2,
}

impl SourceIsaObservationTargetProfileV1 {
    fn decode(value: u8) -> Result<Self, SourceIsaObservationFrameErrorV1> {
        match value {
            1 => Ok(Self::Gfx942),
            2 => Ok(Self::Gfx950),
            _ => Err(SourceIsaObservationFrameErrorV1::InvalidTag),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u16)]
pub enum SourceIsaObservationKirVersionV1 {
    V8 = 8,
    V9 = 9,
}

impl SourceIsaObservationKirVersionV1 {
    fn decode(value: u16) -> Result<Self, SourceIsaObservationFrameErrorV1> {
        match value {
            8 => Ok(Self::V8),
            9 => Ok(Self::V9),
            _ => Err(SourceIsaObservationFrameErrorV1::InvalidTag),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SourceIsaObservationStructuralCountsV1 {
    pub functions: u64,
    pub defined_bodies: u64,
    pub blocks: u64,
    pub operations: u64,
}

impl SourceIsaObservationStructuralCountsV1 {
    fn validate(self) -> Result<(), SourceIsaObservationFrameErrorV1> {
        if self.functions == 0
            || self.functions > MAX_FUNCTIONS_V1 as u64
            || self.defined_bodies != 1
            || self.blocks == 0
            || self.blocks > MAX_PRODUCTION_STRUCTURAL_OPERATIONS_V1
            || self.operations == 0
            || self.operations > MAX_PRODUCTION_STRUCTURAL_OPERATIONS_V1
        {
            return Err(SourceIsaObservationFrameErrorV1::InvalidClaim);
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SourceIsaObservationStructuralBindingV1 {
    identity: [u8; 32],
    target_profile: SourceIsaObservationTargetProfileV1,
    kir_version: SourceIsaObservationKirVersionV1,
    neutral_kir: SourceIsaObservationContentIdentityV1,
    target_kir: SourceIsaObservationContentIdentityV1,
    counts: SourceIsaObservationStructuralCountsV1,
}

impl SourceIsaObservationStructuralBindingV1 {
    pub fn new(
        identity: [u8; 32],
        target_profile: SourceIsaObservationTargetProfileV1,
        kir_version: SourceIsaObservationKirVersionV1,
        neutral_kir: SourceIsaObservationContentIdentityV1,
        target_kir: SourceIsaObservationContentIdentityV1,
        counts: SourceIsaObservationStructuralCountsV1,
    ) -> Result<Self, SourceIsaObservationFrameErrorV1> {
        if identity == [0; 32] {
            return Err(SourceIsaObservationFrameErrorV1::InvalidClaim);
        }
        counts.validate()?;
        Ok(Self {
            identity,
            target_profile,
            kir_version,
            neutral_kir,
            target_kir,
            counts,
        })
    }

    pub const fn identity(self) -> [u8; 32] {
        self.identity
    }

    pub const fn target_profile(self) -> SourceIsaObservationTargetProfileV1 {
        self.target_profile
    }

    pub const fn kir_version(self) -> SourceIsaObservationKirVersionV1 {
        self.kir_version
    }

    pub const fn neutral_kir(self) -> SourceIsaObservationContentIdentityV1 {
        self.neutral_kir
    }

    pub const fn target_kir(self) -> SourceIsaObservationContentIdentityV1 {
        self.target_kir
    }

    pub const fn counts(self) -> SourceIsaObservationStructuralCountsV1 {
        self.counts
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SourceIsaObservationRecordCountsV1 {
    pub records: u64,
    pub source_anchored: u64,
    pub eliminated: u64,
    pub no_source: u64,
    pub source_anchored_without_isa: u64,
    pub isa_references: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SourceIsaObservationQueryCountsV1 {
    pub distinct_source_nodes: u64,
    pub distinct_source_spans: u64,
    pub distinct_isa_points: u64,
    pub max_source_node_cardinality: u64,
    pub max_source_span_cardinality: u64,
    pub max_exact_pc_cardinality: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SourceIsaObservationCountsV1 {
    records: SourceIsaObservationRecordCountsV1,
    queries: SourceIsaObservationQueryCountsV1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SourceIsaObservationSourceSpanV1 {
    file_identity: [u8; 32],
    byte_start: u64,
    byte_end: u64,
    line: u32,
    column: u32,
}

impl SourceIsaObservationSourceSpanV1 {
    pub fn new(
        file_identity: [u8; 32],
        byte_start: u64,
        byte_end: u64,
        line: u32,
        column: u32,
    ) -> Result<Self, SourceIsaObservationFrameErrorV1> {
        if file_identity == [0; 32] || byte_start >= byte_end || line == 0 || column == 0 {
            return Err(SourceIsaObservationFrameErrorV1::InvalidClaim);
        }
        Ok(Self {
            file_identity,
            byte_start,
            byte_end,
            line,
            column,
        })
    }

    pub const fn file_identity(self) -> [u8; 32] {
        self.file_identity
    }

    pub const fn byte_start(self) -> u64 {
        self.byte_start
    }

    pub const fn byte_end(self) -> u64 {
        self.byte_end
    }

    pub const fn line(self) -> u32 {
        self.line
    }

    pub const fn column(self) -> u32 {
        self.column
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SourceIsaObservationIsaPointV1 {
    kernel_ordinal: u64,
    symbol_relative_pc: u64,
}

impl SourceIsaObservationIsaPointV1 {
    pub fn new(
        kernel_ordinal: u64,
        symbol_relative_pc: u64,
    ) -> Result<Self, SourceIsaObservationFrameErrorV1> {
        if kernel_ordinal != 0 || !symbol_relative_pc.is_multiple_of(4) {
            return Err(SourceIsaObservationFrameErrorV1::InvalidClaim);
        }
        Ok(Self {
            kernel_ordinal,
            symbol_relative_pc,
        })
    }

    pub const fn kernel_ordinal(self) -> u64 {
        self.kernel_ordinal
    }

    pub const fn symbol_relative_pc(self) -> u64 {
        self.symbol_relative_pc
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SourceIsaObservationRoundTripWitnessV1 {
    source_node_identity: [u8; 32],
    source_span: SourceIsaObservationSourceSpanV1,
    isa_point: SourceIsaObservationIsaPointV1,
    source_node_query_matches: u64,
    source_span_query_matches: u64,
    isa_point_query_matches: u64,
}

impl SourceIsaObservationRoundTripWitnessV1 {
    pub fn new(
        source_node_identity: [u8; 32],
        source_span: SourceIsaObservationSourceSpanV1,
        isa_point: SourceIsaObservationIsaPointV1,
        source_node_query_matches: u64,
        source_span_query_matches: u64,
        isa_point_query_matches: u64,
    ) -> Result<Self, SourceIsaObservationFrameErrorV1> {
        if source_node_identity == [0; 32]
            || source_node_query_matches == 0
            || source_span_query_matches == 0
            || isa_point_query_matches == 0
        {
            return Err(SourceIsaObservationFrameErrorV1::InvalidClaim);
        }
        Ok(Self {
            source_node_identity,
            source_span,
            isa_point,
            source_node_query_matches,
            source_span_query_matches,
            isa_point_query_matches,
        })
    }

    pub const fn source_node_identity(self) -> [u8; 32] {
        self.source_node_identity
    }

    pub const fn source_span(self) -> SourceIsaObservationSourceSpanV1 {
        self.source_span
    }

    pub const fn isa_point(self) -> SourceIsaObservationIsaPointV1 {
        self.isa_point
    }

    pub const fn source_node_query_matches(self) -> u64 {
        self.source_node_query_matches
    }

    pub const fn source_span_query_matches(self) -> u64 {
        self.source_span_query_matches
    }

    pub const fn isa_point_query_matches(self) -> u64 {
        self.isa_point_query_matches
    }
}

impl SourceIsaObservationCountsV1 {
    pub fn new(
        records: SourceIsaObservationRecordCountsV1,
        queries: SourceIsaObservationQueryCountsV1,
    ) -> Result<Self, SourceIsaObservationFrameErrorV1> {
        let mapped = records
            .source_anchored
            .checked_add(records.eliminated)
            .and_then(|count| count.checked_add(records.no_source))
            .ok_or(SourceIsaObservationFrameErrorV1::InvalidClaim)?;
        let anchored_with_isa = records
            .source_anchored
            .checked_sub(records.source_anchored_without_isa)
            .ok_or(SourceIsaObservationFrameErrorV1::InvalidClaim)?;
        let source_records = records
            .source_anchored
            .checked_add(records.eliminated)
            .ok_or(SourceIsaObservationFrameErrorV1::InvalidClaim)?;
        let query_pairs = [
            (
                queries.distinct_source_nodes,
                queries.max_source_node_cardinality,
                source_records,
            ),
            (
                queries.distinct_source_spans,
                queries.max_source_span_cardinality,
                source_records,
            ),
            (
                queries.distinct_isa_points,
                queries.max_exact_pc_cardinality,
                records.isa_references,
            ),
        ];
        if records.records != mapped
            || records.records > MAX_SOURCE_ISA_RECORDS_V1
            || records.isa_references < anchored_with_isa
            || records.isa_references > MAX_SOURCE_ISA_REFERENCES_V1
            || query_pairs.iter().any(|(distinct, maximum, population)| {
                let feasible_lower = maximum
                    .checked_add(distinct.saturating_sub(1))
                    .is_some_and(|minimum| minimum <= *population);
                let feasible_upper = distinct
                    .checked_mul(*maximum)
                    .is_some_and(|maximum_population| *population <= maximum_population);
                *distinct > *population
                    || *distinct > MAX_SOURCE_ISA_REFERENCES_V1
                    || *maximum > *population
                    || if *population == 0 {
                        *distinct != 0 || *maximum != 0
                    } else {
                        *distinct == 0 || *maximum == 0 || !feasible_lower || !feasible_upper
                    }
            })
        {
            return Err(SourceIsaObservationFrameErrorV1::InvalidClaim);
        }
        Ok(Self { records, queries })
    }

    pub const fn records(self) -> SourceIsaObservationRecordCountsV1 {
        self.records
    }

    pub const fn queries(self) -> SourceIsaObservationQueryCountsV1 {
        self.queries
    }

    const fn values(self) -> [u64; COUNT_FIELDS_V1] {
        [
            self.records.records,
            self.records.source_anchored,
            self.records.eliminated,
            self.records.no_source,
            self.records.source_anchored_without_isa,
            self.records.isa_references,
            self.queries.distinct_source_nodes,
            self.queries.distinct_source_spans,
            self.queries.distinct_isa_points,
            self.queries.max_source_node_cardinality,
            self.queries.max_source_span_cardinality,
            self.queries.max_exact_pc_cardinality,
        ]
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u16)]
pub enum SourceIsaObservationUnavailableReasonV1 {
    CarrierMultipleKirFunctionBodies = 1,
    CarrierNoStatementCorrespondence = 2,
    CarrierSourceMapUnavailable = 3,
    CarrierResourceLimit = 4,
    CarrierCanonicalKirV7ProjectionUnavailable = 5,
    CarrierSourceObservationUnrepresentable = 6,
    CarrierSemanticMapConstructionUnavailable = 7,
    CarrierSemanticMapEncodingUnavailable = 8,
    CarrierFragmentConstructionUnavailable = 9,
    CarrierConstructionUnavailable = 10,
    CarrierReceiptExtensionConstructionUnavailable = 11,
    CarrierCorrespondenceValidationUnavailable = 12,
    CarrierCanonicalKirModuleMismatch = 13,
    CarrierLegacyBareAssociationNoAttachment = 14,
    AnchorLegacySemanticAttachment = 101,
    AnchorLegacyUninstrumentedReplay = 102,
    AnchorNoOperations = 103,
    AnchorMultipleDefinedBodies = 104,
    AnchorCompilerInstrumentationAbsent = 105,
    SourceProjectionForKirV9 = 201,
    FinalizedEvidenceUnavailableFromReadyState = 202,
}

impl SourceIsaObservationUnavailableReasonV1 {
    pub const fn label(self) -> &'static str {
        match self {
            Self::CarrierMultipleKirFunctionBodies => "carrier-multiple-kir-function-bodies",
            Self::CarrierNoStatementCorrespondence => "carrier-no-statement-correspondence",
            Self::CarrierSourceMapUnavailable => "carrier-source-map-unavailable",
            Self::CarrierResourceLimit => "carrier-resource-limit",
            Self::CarrierCanonicalKirV7ProjectionUnavailable => {
                "carrier-canonical-kir-v7-projection-unavailable"
            }
            Self::CarrierSourceObservationUnrepresentable => {
                "carrier-source-observation-unrepresentable"
            }
            Self::CarrierSemanticMapConstructionUnavailable => {
                "carrier-semantic-map-construction-unavailable"
            }
            Self::CarrierSemanticMapEncodingUnavailable => {
                "carrier-semantic-map-encoding-unavailable"
            }
            Self::CarrierFragmentConstructionUnavailable => {
                "carrier-fragment-construction-unavailable"
            }
            Self::CarrierConstructionUnavailable => "carrier-construction-unavailable",
            Self::CarrierReceiptExtensionConstructionUnavailable => {
                "carrier-receipt-extension-construction-unavailable"
            }
            Self::CarrierCorrespondenceValidationUnavailable => {
                "carrier-correspondence-validation-unavailable"
            }
            Self::CarrierCanonicalKirModuleMismatch => "carrier-canonical-kir-module-mismatch",
            Self::CarrierLegacyBareAssociationNoAttachment => {
                "carrier-legacy-bare-association-no-attachment"
            }
            Self::AnchorLegacySemanticAttachment => "anchor-legacy-semantic-attachment",
            Self::AnchorLegacyUninstrumentedReplay => "anchor-legacy-uninstrumented-replay",
            Self::AnchorNoOperations => "anchor-no-operations",
            Self::AnchorMultipleDefinedBodies => "anchor-multiple-defined-bodies",
            Self::AnchorCompilerInstrumentationAbsent => "anchor-compiler-instrumentation-absent",
            Self::SourceProjectionForKirV9 => "source-projection-for-kir-v9",
            Self::FinalizedEvidenceUnavailableFromReadyState => {
                "finalized-evidence-unavailable-from-ready-state"
            }
        }
    }

    fn decode(value: u16) -> Result<Self, SourceIsaObservationFrameErrorV1> {
        match value {
            1 => Ok(Self::CarrierMultipleKirFunctionBodies),
            2 => Ok(Self::CarrierNoStatementCorrespondence),
            3 => Ok(Self::CarrierSourceMapUnavailable),
            4 => Ok(Self::CarrierResourceLimit),
            5 => Ok(Self::CarrierCanonicalKirV7ProjectionUnavailable),
            6 => Ok(Self::CarrierSourceObservationUnrepresentable),
            7 => Ok(Self::CarrierSemanticMapConstructionUnavailable),
            8 => Ok(Self::CarrierSemanticMapEncodingUnavailable),
            9 => Ok(Self::CarrierFragmentConstructionUnavailable),
            10 => Ok(Self::CarrierConstructionUnavailable),
            11 => Ok(Self::CarrierReceiptExtensionConstructionUnavailable),
            12 => Ok(Self::CarrierCorrespondenceValidationUnavailable),
            13 => Ok(Self::CarrierCanonicalKirModuleMismatch),
            14 => Ok(Self::CarrierLegacyBareAssociationNoAttachment),
            101 => Ok(Self::AnchorLegacySemanticAttachment),
            102 => Ok(Self::AnchorLegacyUninstrumentedReplay),
            103 => Ok(Self::AnchorNoOperations),
            104 => Ok(Self::AnchorMultipleDefinedBodies),
            105 => Ok(Self::AnchorCompilerInstrumentationAbsent),
            201 => Ok(Self::SourceProjectionForKirV9),
            202 => Ok(Self::FinalizedEvidenceUnavailableFromReadyState),
            _ => Err(SourceIsaObservationFrameErrorV1::InvalidTag),
        }
    }

    pub fn from_code(value: u16) -> Result<Self, SourceIsaObservationFrameErrorV1> {
        Self::decode(value)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u16)]
pub enum SourceIsaObservationErrorCodeV1 {
    InvalidKirToLlvmReplay = 3,
    NonExactSemanticMap = 4,
    ArtifactIdentityMismatch = 5,
    TargetKirIdentityMismatch = 6,
    CoordinateShapeMismatch = 7,
    InvalidSourceGraph = 8,
    ResourceLimit = 9,
    AllocationFailure = 10,
    FinalizedMapProductionAssociation = 0x1001,
    FinalizedMapProductionAssociationMismatch = 0x1002,
    FinalizedMapInvalidKirToLlvmReplay = 0x1003,
    FinalizedMapKirToLlvmReplayTargetMismatch = 0x1004,
    FinalizedMapInvalidLlvmToHsacoCustody = 0x1005,
    FinalizedMapInvalidBoundSourceMap = 0x1006,
    FinalizedMapInvalidBoundSemanticMir = 0x1007,
    FinalizedMapInvalidBoundCorrespondenceV4 = 0x1008,
    FinalizedMapInvalidBoundCanonicalKirV8 = 0x1009,
    FinalizedMapInvalidBoundCanonicalKirV7 = 0x100a,
    FinalizedMapCanonicalKirProjectionMismatch = 0x100b,
    FinalizedMapCorrespondenceIdentityMismatch = 0x100c,
    FinalizedMapInvalidSemanticCorrespondence = 0x100d,
    FinalizedMapArtifactInspection = 0x100e,
    FinalizedMapAllocationFailure = 0x100f,
    FinalizedMapInvalidBoundCorrespondenceV5 = 0x1010,
    FinalizedMapInvalidBoundMultiRootCorrespondenceV2 = 0x1011,
    FinalizedMapInvalidInstanceCustodyV1 = 0x1012,
    SemanticMapInvalidLength = 0x1101,
    SemanticMapInvalidJson = 0x1102,
    SemanticMapNonCanonicalEncoding = 0x1103,
    SemanticMapEncoding = 0x1104,
    SemanticMapInvalidBinding = 0x1105,
    SemanticMapInvalidKernelOrdinalBasis = 0x1106,
    SemanticMapInvalidNode = 0x1107,
    SemanticMapInvalidMapping = 0x1108,
    SemanticMapDuplicateNode = 0x1109,
    SemanticMapDuplicateMapping = 0x110a,
    SemanticMapDuplicateReference = 0x110b,
    SemanticMapUnknownNode = 0x110c,
    SemanticMapLayerMismatch = 0x110d,
    SemanticMapContradictoryMapping = 0x110e,
    SemanticMapOrphanNode = 0x110f,
    SemanticMapInvalidBoundary = 0x1110,
    SemanticMapUntypedBoundary = 0x1111,
    SemanticMapResourceLimit = 0x1112,
    SemanticMapAllocationFailure = 0x1113,
    SemanticMapContentBindingMismatch = 0x1114,
    SemanticMapArtifactBindingMismatch = 0x1115,
    SemanticMapInvalidBoundSourceMap = 0x1116,
    SemanticMapInvalidBoundCanonicalKir = 0x1117,
    SemanticMapSourceMapKirBindingMismatch = 0x1118,
    SemanticMapInvalidSourceLocation = 0x1119,
    SemanticMapInvalidMirLocation = 0x111a,
    SemanticMapInvalidKirLocation = 0x111b,
    SemanticMapInvalidIsaInterval = 0x111c,
    ProductionFragmentInvalidEncoding = 0x1201,
    ProductionFragmentInvalidAssociation = 0x1202,
    ProductionFragmentInvalidGap = 0x1203,
    ProductionFragmentInvalidScheduleStatus = 0x1204,
    ProductionFragmentInvalidSourceMap = 0x1205,
    ProductionFragmentInvalidCanonicalKir = 0x1206,
    ProductionFragmentInvalidSemanticMap = 0x1207,
    ProductionFragmentAxisMismatch = 0x1208,
    ProductionFragmentResourceLimit = 0x1209,
    ProductionFragmentAllocationFailure = 0x120a,
    SemanticAnchorInvalidCompilerAttachment = 0x2001,
    SemanticAnchorInvalidProductionAssociation = 0x2002,
    SemanticAnchorInvalidKirToLlvmReplay = 0x2003,
    SemanticAnchorTargetMismatch = 0x2004,
    SemanticAnchorInvalidLlvm = 0x2005,
    SemanticAnchorContradictoryLlvm = 0x2006,
    SemanticAnchorBindingMismatch = 0x2007,
    SemanticAnchorKirCoordinateMismatch = 0x2008,
    SemanticAnchorKirToLlvmAnchorMismatch = 0x2009,
    SemanticAnchorInvalidArtifact = 0x200a,
    SemanticAnchorMissingProbeSection = 0x200b,
    SemanticAnchorAmbiguousProbeSection = 0x200c,
    SemanticAnchorInvalidProbeEncoding = 0x200d,
    SemanticAnchorProbeDescriptorMismatch = 0x200e,
    SemanticAnchorAmbiguousEntrySymbol = 0x200f,
    SemanticAnchorUnexpectedProbe = 0x2010,
    SemanticAnchorProbeOutsideKernel = 0x2011,
    SemanticAnchorResourceLimit = 0x2012,
    SemanticAnchorAllocationFailure = 0x2013,
}

impl SourceIsaObservationErrorCodeV1 {
    pub const fn label(self) -> &'static str {
        match self {
            Self::InvalidKirToLlvmReplay => "invalid-kir-to-llvm-replay",
            Self::NonExactSemanticMap => "non-exact-semantic-map",
            Self::ArtifactIdentityMismatch => "artifact-identity-mismatch",
            Self::TargetKirIdentityMismatch => "target-kir-identity-mismatch",
            Self::CoordinateShapeMismatch => "coordinate-shape-mismatch",
            Self::InvalidSourceGraph => "invalid-source-graph",
            Self::ResourceLimit => "resource-limit",
            Self::AllocationFailure => "allocation-failure",
            Self::FinalizedMapProductionAssociation => "finalized-map-production-association",
            Self::FinalizedMapProductionAssociationMismatch => {
                "finalized-map-production-association-mismatch"
            }
            Self::FinalizedMapInvalidKirToLlvmReplay => "finalized-map-invalid-kir-to-llvm-replay",
            Self::FinalizedMapKirToLlvmReplayTargetMismatch => {
                "finalized-map-kir-to-llvm-replay-target-mismatch"
            }
            Self::FinalizedMapInvalidLlvmToHsacoCustody => {
                "finalized-map-invalid-llvm-to-hsaco-custody"
            }
            Self::FinalizedMapInvalidBoundSourceMap => "finalized-map-invalid-bound-source-map",
            Self::FinalizedMapInvalidBoundSemanticMir => "finalized-map-invalid-bound-semantic-mir",
            Self::FinalizedMapInvalidBoundCorrespondenceV4 => {
                "finalized-map-invalid-bound-correspondence-v4"
            }
            Self::FinalizedMapInvalidBoundCanonicalKirV8 => {
                "finalized-map-invalid-bound-canonical-kir-v8"
            }
            Self::FinalizedMapInvalidBoundCanonicalKirV7 => {
                "finalized-map-invalid-bound-canonical-kir-v7"
            }
            Self::FinalizedMapCanonicalKirProjectionMismatch => {
                "finalized-map-canonical-kir-projection-mismatch"
            }
            Self::FinalizedMapCorrespondenceIdentityMismatch => {
                "finalized-map-correspondence-identity-mismatch"
            }
            Self::FinalizedMapInvalidSemanticCorrespondence => {
                "finalized-map-invalid-semantic-correspondence"
            }
            Self::FinalizedMapArtifactInspection => "finalized-map-artifact-inspection",
            Self::FinalizedMapAllocationFailure => "finalized-map-allocation-failure",
            Self::FinalizedMapInvalidBoundCorrespondenceV5 => {
                "finalized-map-invalid-bound-correspondence-v5"
            }
            Self::FinalizedMapInvalidBoundMultiRootCorrespondenceV2 => {
                "finalized-map-invalid-bound-multi-root-correspondence-v2"
            }
            Self::FinalizedMapInvalidInstanceCustodyV1 => {
                "finalized-map-invalid-instance-custody-v1"
            }
            Self::SemanticMapInvalidLength => "semantic-map-invalid-length",
            Self::SemanticMapInvalidJson => "semantic-map-invalid-json",
            Self::SemanticMapNonCanonicalEncoding => "semantic-map-noncanonical-encoding",
            Self::SemanticMapEncoding => "semantic-map-encoding",
            Self::SemanticMapInvalidBinding => "semantic-map-invalid-binding",
            Self::SemanticMapInvalidKernelOrdinalBasis => {
                "semantic-map-invalid-kernel-ordinal-basis"
            }
            Self::SemanticMapInvalidNode => "semantic-map-invalid-node",
            Self::SemanticMapInvalidMapping => "semantic-map-invalid-mapping",
            Self::SemanticMapDuplicateNode => "semantic-map-duplicate-node",
            Self::SemanticMapDuplicateMapping => "semantic-map-duplicate-mapping",
            Self::SemanticMapDuplicateReference => "semantic-map-duplicate-reference",
            Self::SemanticMapUnknownNode => "semantic-map-unknown-node",
            Self::SemanticMapLayerMismatch => "semantic-map-layer-mismatch",
            Self::SemanticMapContradictoryMapping => "semantic-map-contradictory-mapping",
            Self::SemanticMapOrphanNode => "semantic-map-orphan-node",
            Self::SemanticMapInvalidBoundary => "semantic-map-invalid-boundary",
            Self::SemanticMapUntypedBoundary => "semantic-map-untyped-boundary",
            Self::SemanticMapResourceLimit => "semantic-map-resource-limit",
            Self::SemanticMapAllocationFailure => "semantic-map-allocation-failure",
            Self::SemanticMapContentBindingMismatch => "semantic-map-content-binding-mismatch",
            Self::SemanticMapArtifactBindingMismatch => "semantic-map-artifact-binding-mismatch",
            Self::SemanticMapInvalidBoundSourceMap => "semantic-map-invalid-bound-source-map",
            Self::SemanticMapInvalidBoundCanonicalKir => "semantic-map-invalid-bound-canonical-kir",
            Self::SemanticMapSourceMapKirBindingMismatch => {
                "semantic-map-source-map-kir-binding-mismatch"
            }
            Self::SemanticMapInvalidSourceLocation => "semantic-map-invalid-source-location",
            Self::SemanticMapInvalidMirLocation => "semantic-map-invalid-mir-location",
            Self::SemanticMapInvalidKirLocation => "semantic-map-invalid-kir-location",
            Self::SemanticMapInvalidIsaInterval => "semantic-map-invalid-isa-interval",
            Self::ProductionFragmentInvalidEncoding => "production-fragment-invalid-encoding",
            Self::ProductionFragmentInvalidAssociation => "production-fragment-invalid-association",
            Self::ProductionFragmentInvalidGap => "production-fragment-invalid-gap",
            Self::ProductionFragmentInvalidScheduleStatus => {
                "production-fragment-invalid-schedule-status"
            }
            Self::ProductionFragmentInvalidSourceMap => "production-fragment-invalid-source-map",
            Self::ProductionFragmentInvalidCanonicalKir => {
                "production-fragment-invalid-canonical-kir"
            }
            Self::ProductionFragmentInvalidSemanticMap => {
                "production-fragment-invalid-semantic-map"
            }
            Self::ProductionFragmentAxisMismatch => "production-fragment-axis-mismatch",
            Self::ProductionFragmentResourceLimit => "production-fragment-resource-limit",
            Self::ProductionFragmentAllocationFailure => "production-fragment-allocation-failure",
            Self::SemanticAnchorInvalidCompilerAttachment => {
                "semantic-anchor-invalid-compiler-attachment"
            }
            Self::SemanticAnchorInvalidProductionAssociation => {
                "semantic-anchor-invalid-production-association"
            }
            Self::SemanticAnchorInvalidKirToLlvmReplay => {
                "semantic-anchor-invalid-kir-to-llvm-replay"
            }
            Self::SemanticAnchorTargetMismatch => "semantic-anchor-target-mismatch",
            Self::SemanticAnchorInvalidLlvm => "semantic-anchor-invalid-llvm",
            Self::SemanticAnchorContradictoryLlvm => "semantic-anchor-contradictory-llvm",
            Self::SemanticAnchorBindingMismatch => "semantic-anchor-binding-mismatch",
            Self::SemanticAnchorKirCoordinateMismatch => "semantic-anchor-kir-coordinate-mismatch",
            Self::SemanticAnchorKirToLlvmAnchorMismatch => {
                "semantic-anchor-kir-to-llvm-anchor-mismatch"
            }
            Self::SemanticAnchorInvalidArtifact => "semantic-anchor-invalid-artifact",
            Self::SemanticAnchorMissingProbeSection => "semantic-anchor-missing-probe-section",
            Self::SemanticAnchorAmbiguousProbeSection => "semantic-anchor-ambiguous-probe-section",
            Self::SemanticAnchorInvalidProbeEncoding => "semantic-anchor-invalid-probe-encoding",
            Self::SemanticAnchorProbeDescriptorMismatch => {
                "semantic-anchor-probe-descriptor-mismatch"
            }
            Self::SemanticAnchorAmbiguousEntrySymbol => "semantic-anchor-ambiguous-entry-symbol",
            Self::SemanticAnchorUnexpectedProbe => "semantic-anchor-unexpected-probe",
            Self::SemanticAnchorProbeOutsideKernel => "semantic-anchor-probe-outside-kernel",
            Self::SemanticAnchorResourceLimit => "semantic-anchor-resource-limit",
            Self::SemanticAnchorAllocationFailure => "semantic-anchor-allocation-failure",
        }
    }

    fn decode(value: u16) -> Result<Self, SourceIsaObservationFrameErrorV1> {
        match value {
            3 => Ok(Self::InvalidKirToLlvmReplay),
            4 => Ok(Self::NonExactSemanticMap),
            5 => Ok(Self::ArtifactIdentityMismatch),
            6 => Ok(Self::TargetKirIdentityMismatch),
            7 => Ok(Self::CoordinateShapeMismatch),
            8 => Ok(Self::InvalidSourceGraph),
            9 => Ok(Self::ResourceLimit),
            10 => Ok(Self::AllocationFailure),
            0x1001 => Ok(Self::FinalizedMapProductionAssociation),
            0x1002 => Ok(Self::FinalizedMapProductionAssociationMismatch),
            0x1003 => Ok(Self::FinalizedMapInvalidKirToLlvmReplay),
            0x1004 => Ok(Self::FinalizedMapKirToLlvmReplayTargetMismatch),
            0x1005 => Ok(Self::FinalizedMapInvalidLlvmToHsacoCustody),
            0x1006 => Ok(Self::FinalizedMapInvalidBoundSourceMap),
            0x1007 => Ok(Self::FinalizedMapInvalidBoundSemanticMir),
            0x1008 => Ok(Self::FinalizedMapInvalidBoundCorrespondenceV4),
            0x1009 => Ok(Self::FinalizedMapInvalidBoundCanonicalKirV8),
            0x100a => Ok(Self::FinalizedMapInvalidBoundCanonicalKirV7),
            0x100b => Ok(Self::FinalizedMapCanonicalKirProjectionMismatch),
            0x100c => Ok(Self::FinalizedMapCorrespondenceIdentityMismatch),
            0x100d => Ok(Self::FinalizedMapInvalidSemanticCorrespondence),
            0x100e => Ok(Self::FinalizedMapArtifactInspection),
            0x100f => Ok(Self::FinalizedMapAllocationFailure),
            0x1010 => Ok(Self::FinalizedMapInvalidBoundCorrespondenceV5),
            0x1011 => Ok(Self::FinalizedMapInvalidBoundMultiRootCorrespondenceV2),
            0x1012 => Ok(Self::FinalizedMapInvalidInstanceCustodyV1),
            0x1101 => Ok(Self::SemanticMapInvalidLength),
            0x1102 => Ok(Self::SemanticMapInvalidJson),
            0x1103 => Ok(Self::SemanticMapNonCanonicalEncoding),
            0x1104 => Ok(Self::SemanticMapEncoding),
            0x1105 => Ok(Self::SemanticMapInvalidBinding),
            0x1106 => Ok(Self::SemanticMapInvalidKernelOrdinalBasis),
            0x1107 => Ok(Self::SemanticMapInvalidNode),
            0x1108 => Ok(Self::SemanticMapInvalidMapping),
            0x1109 => Ok(Self::SemanticMapDuplicateNode),
            0x110a => Ok(Self::SemanticMapDuplicateMapping),
            0x110b => Ok(Self::SemanticMapDuplicateReference),
            0x110c => Ok(Self::SemanticMapUnknownNode),
            0x110d => Ok(Self::SemanticMapLayerMismatch),
            0x110e => Ok(Self::SemanticMapContradictoryMapping),
            0x110f => Ok(Self::SemanticMapOrphanNode),
            0x1110 => Ok(Self::SemanticMapInvalidBoundary),
            0x1111 => Ok(Self::SemanticMapUntypedBoundary),
            0x1112 => Ok(Self::SemanticMapResourceLimit),
            0x1113 => Ok(Self::SemanticMapAllocationFailure),
            0x1114 => Ok(Self::SemanticMapContentBindingMismatch),
            0x1115 => Ok(Self::SemanticMapArtifactBindingMismatch),
            0x1116 => Ok(Self::SemanticMapInvalidBoundSourceMap),
            0x1117 => Ok(Self::SemanticMapInvalidBoundCanonicalKir),
            0x1118 => Ok(Self::SemanticMapSourceMapKirBindingMismatch),
            0x1119 => Ok(Self::SemanticMapInvalidSourceLocation),
            0x111a => Ok(Self::SemanticMapInvalidMirLocation),
            0x111b => Ok(Self::SemanticMapInvalidKirLocation),
            0x111c => Ok(Self::SemanticMapInvalidIsaInterval),
            0x1201 => Ok(Self::ProductionFragmentInvalidEncoding),
            0x1202 => Ok(Self::ProductionFragmentInvalidAssociation),
            0x1203 => Ok(Self::ProductionFragmentInvalidGap),
            0x1204 => Ok(Self::ProductionFragmentInvalidScheduleStatus),
            0x1205 => Ok(Self::ProductionFragmentInvalidSourceMap),
            0x1206 => Ok(Self::ProductionFragmentInvalidCanonicalKir),
            0x1207 => Ok(Self::ProductionFragmentInvalidSemanticMap),
            0x1208 => Ok(Self::ProductionFragmentAxisMismatch),
            0x1209 => Ok(Self::ProductionFragmentResourceLimit),
            0x120a => Ok(Self::ProductionFragmentAllocationFailure),
            0x2001 => Ok(Self::SemanticAnchorInvalidCompilerAttachment),
            0x2002 => Ok(Self::SemanticAnchorInvalidProductionAssociation),
            0x2003 => Ok(Self::SemanticAnchorInvalidKirToLlvmReplay),
            0x2004 => Ok(Self::SemanticAnchorTargetMismatch),
            0x2005 => Ok(Self::SemanticAnchorInvalidLlvm),
            0x2006 => Ok(Self::SemanticAnchorContradictoryLlvm),
            0x2007 => Ok(Self::SemanticAnchorBindingMismatch),
            0x2008 => Ok(Self::SemanticAnchorKirCoordinateMismatch),
            0x2009 => Ok(Self::SemanticAnchorKirToLlvmAnchorMismatch),
            0x200a => Ok(Self::SemanticAnchorInvalidArtifact),
            0x200b => Ok(Self::SemanticAnchorMissingProbeSection),
            0x200c => Ok(Self::SemanticAnchorAmbiguousProbeSection),
            0x200d => Ok(Self::SemanticAnchorInvalidProbeEncoding),
            0x200e => Ok(Self::SemanticAnchorProbeDescriptorMismatch),
            0x200f => Ok(Self::SemanticAnchorAmbiguousEntrySymbol),
            0x2010 => Ok(Self::SemanticAnchorUnexpectedProbe),
            0x2011 => Ok(Self::SemanticAnchorProbeOutsideKernel),
            0x2012 => Ok(Self::SemanticAnchorResourceLimit),
            0x2013 => Ok(Self::SemanticAnchorAllocationFailure),
            _ => Err(SourceIsaObservationFrameErrorV1::InvalidTag),
        }
    }

    pub fn from_code(value: u16) -> Result<Self, SourceIsaObservationFrameErrorV1> {
        Self::decode(value)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AdmittedSourceIsaObservationV1 {
    correlation: [u8; 32],
    artifact: SourceIsaObservationContentIdentityV1,
    structural: SourceIsaObservationStructuralBindingV1,
    counts: SourceIsaObservationCountsV1,
    round_trip_witness: Option<SourceIsaObservationRoundTripWitnessV1>,
}

impl AdmittedSourceIsaObservationV1 {
    pub fn new(
        correlation: [u8; 32],
        artifact: SourceIsaObservationContentIdentityV1,
        structural: SourceIsaObservationStructuralBindingV1,
        counts: SourceIsaObservationCountsV1,
        round_trip_witness: Option<SourceIsaObservationRoundTripWitnessV1>,
    ) -> Result<Self, SourceIsaObservationFrameErrorV1> {
        let records = counts.records();
        let queries = counts.queries();
        let source_anchored_with_isa = records
            .source_anchored
            .checked_sub(records.source_anchored_without_isa)
            .ok_or(SourceIsaObservationFrameErrorV1::InvalidClaim)?;
        let uncovered_operations = records.no_source;
        let covered_operations = structural
            .counts()
            .operations
            .checked_sub(uncovered_operations)
            .ok_or(SourceIsaObservationFrameErrorV1::InvalidClaim)?;
        if correlation == [0; 32]
            || (covered_operations == 0) != (records.source_anchored == 0)
            || records.source_anchored < covered_operations
            || round_trip_witness.is_some() != (source_anchored_with_isa != 0)
            || round_trip_witness.is_some_and(|witness| {
                witness.source_node_query_matches > queries.max_source_node_cardinality
                    || witness.source_span_query_matches > queries.max_source_span_cardinality
                    || witness.isa_point_query_matches > queries.max_exact_pc_cardinality
            })
        {
            return Err(SourceIsaObservationFrameErrorV1::InvalidClaim);
        }
        Ok(Self {
            correlation,
            artifact,
            structural,
            counts,
            round_trip_witness,
        })
    }

    pub const fn correlation(self) -> [u8; 32] {
        self.correlation
    }

    pub const fn artifact(self) -> SourceIsaObservationContentIdentityV1 {
        self.artifact
    }

    pub const fn structural(self) -> SourceIsaObservationStructuralBindingV1 {
        self.structural
    }

    pub const fn counts(self) -> SourceIsaObservationCountsV1 {
        self.counts
    }

    pub const fn round_trip_witness(self) -> Option<SourceIsaObservationRoundTripWitnessV1> {
        self.round_trip_witness
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
// The admitted payload is intentionally allocation-free inside the fixed 680-byte wire frame.
#[allow(clippy::large_enum_variant)]
pub enum SourceIsaObservationOutcomeV1 {
    Admitted(AdmittedSourceIsaObservationV1),
    Unavailable(SourceIsaObservationUnavailableReasonV1),
    Error(SourceIsaObservationErrorCodeV1),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SourceIsaObservationContextV1 {
    config: [u8; 32],
    unit: [u8; 32],
    attempt: SourceIsaObservationAttemptV1,
    finalization: [u8; 32],
}

impl SourceIsaObservationContextV1 {
    pub fn new(
        config: [u8; 32],
        unit: [u8; 32],
        attempt: SourceIsaObservationAttemptV1,
        finalization: [u8; 32],
    ) -> Result<Self, SourceIsaObservationFrameErrorV1> {
        if config == [0; 32]
            || unit == [0; 32]
            || attempt.session() == SourceIsaObservationSessionV1::DIRECT
            || finalization == [0; 32]
        {
            return Err(SourceIsaObservationFrameErrorV1::InvalidClaim);
        }
        Ok(Self {
            config,
            unit,
            attempt,
            finalization,
        })
    }

    pub const fn config(self) -> [u8; 32] {
        self.config
    }

    pub const fn unit(self) -> [u8; 32] {
        self.unit
    }

    pub const fn attempt(self) -> SourceIsaObservationAttemptV1 {
        self.attempt
    }

    pub const fn finalization(self) -> [u8; 32] {
        self.finalization
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceIsaObservationFrameV1 {
    context: SourceIsaObservationContextV1,
    outcome: SourceIsaObservationOutcomeV1,
    identity: [u8; 32],
}

impl SourceIsaObservationFrameV1 {
    pub fn new(
        context: SourceIsaObservationContextV1,
        outcome: SourceIsaObservationOutcomeV1,
    ) -> Self {
        let mut frame = Self {
            context,
            outcome,
            identity: [0; 32],
        };
        frame.identity = frame_identity(&frame.encode_prefix());
        frame
    }

    pub const fn context(&self) -> SourceIsaObservationContextV1 {
        self.context
    }

    pub const fn outcome(&self) -> SourceIsaObservationOutcomeV1 {
        self.outcome
    }

    pub const fn identity(&self) -> [u8; 32] {
        self.identity
    }

    pub fn encode(&self) -> [u8; SOURCE_ISA_OBSERVATION_FRAME_BYTES_V1] {
        let prefix = self.encode_prefix();
        let mut encoded = [0; SOURCE_ISA_OBSERVATION_FRAME_BYTES_V1];
        encoded[..FRAME_PREFIX_BYTES_V1].copy_from_slice(&prefix);
        encoded[FRAME_PREFIX_BYTES_V1..].copy_from_slice(&self.identity());
        encoded
    }

    pub fn decode(encoded: &[u8]) -> Result<Self, SourceIsaObservationFrameErrorV1> {
        if encoded.len() != SOURCE_ISA_OBSERVATION_FRAME_BYTES_V1
            || &encoded[..8] != FRAME_MAGIC_V1
            || u16::from_le_bytes(encoded[8..10].try_into().expect("fixed version"))
                != FRAME_VERSION_V1
            || usize::from(u16::from_le_bytes(
                encoded[10..12].try_into().expect("fixed header"),
            )) != FRAME_HEADER_BYTES_V1
            || usize::try_from(u32::from_le_bytes(
                encoded[12..16].try_into().expect("fixed length"),
            ))
            .ok()
                != Some(SOURCE_ISA_OBSERVATION_FRAME_BYTES_V1)
        {
            return Err(SourceIsaObservationFrameErrorV1::Malformed);
        }
        let identity: [u8; 32] = encoded[FRAME_PREFIX_BYTES_V1..]
            .try_into()
            .expect("fixed frame identity");
        if identity == [0; 32] || frame_identity(&encoded[..FRAME_PREFIX_BYTES_V1]) != identity {
            return Err(SourceIsaObservationFrameErrorV1::Identity);
        }

        let mut decoder = FrameDecoder::new(&encoded[FRAME_HEADER_BYTES_V1..FRAME_PREFIX_BYTES_V1]);
        let config = decoder.array()?;
        let unit = decoder.array()?;
        let generation = decoder.u64()?;
        let session = SourceIsaObservationSessionV1::from_bytes(decoder.array()?);
        let invocation = SourceIsaObservationInvocationV1::from_bytes(decoder.array()?);
        let attempt = SourceIsaObservationAttemptV1::new(generation, session, invocation)?;
        let finalization = decoder.array()?;
        let context = SourceIsaObservationContextV1::new(config, unit, attempt, finalization)?;
        let outcome_tag = decoder.u8()?;
        let reason_code = decoder.u16()?;
        decoder.zeros(5)?;
        let admitted_bytes = decoder.take(ADMITTED_PAYLOAD_BYTES_V1)?;
        let outcome = match outcome_tag {
            1 if reason_code == 0 => {
                SourceIsaObservationOutcomeV1::Admitted(decode_admitted(admitted_bytes)?)
            }
            2 if admitted_bytes.iter().all(|byte| *byte == 0) => {
                SourceIsaObservationOutcomeV1::Unavailable(
                    SourceIsaObservationUnavailableReasonV1::decode(reason_code)?,
                )
            }
            3 if admitted_bytes.iter().all(|byte| *byte == 0) => {
                SourceIsaObservationOutcomeV1::Error(SourceIsaObservationErrorCodeV1::decode(
                    reason_code,
                )?)
            }
            _ => return Err(SourceIsaObservationFrameErrorV1::InvalidTag),
        };
        if decoder.u64()? != 0 {
            return Err(SourceIsaObservationFrameErrorV1::TruthClaim);
        }
        decoder.finish()?;
        Ok(Self {
            context,
            outcome,
            identity,
        })
    }

    fn encode_prefix(&self) -> [u8; FRAME_PREFIX_BYTES_V1] {
        let mut encoded = [0; FRAME_PREFIX_BYTES_V1];
        let mut encoder = FrameEncoder::new(&mut encoded);
        encoder.bytes(FRAME_MAGIC_V1);
        encoder.u16(FRAME_VERSION_V1);
        encoder.u16(FRAME_HEADER_BYTES_V1 as u16);
        encoder.u32(SOURCE_ISA_OBSERVATION_FRAME_BYTES_V1 as u32);
        encoder.bytes(&self.context.config);
        encoder.bytes(&self.context.unit);
        encoder.u64(self.context.attempt.generation());
        encoder.bytes(self.context.attempt.session().as_bytes());
        encoder.bytes(self.context.attempt.invocation().as_bytes());
        encoder.bytes(&self.context.finalization());
        match self.outcome() {
            SourceIsaObservationOutcomeV1::Admitted(admitted) => {
                encoder.u8(1);
                encoder.u16(0);
                encoder.zeros(5);
                encode_admitted(&mut encoder, admitted);
            }
            SourceIsaObservationOutcomeV1::Unavailable(reason) => {
                encoder.u8(2);
                encoder.u16(reason as u16);
                encoder.zeros(5 + ADMITTED_PAYLOAD_BYTES_V1);
            }
            SourceIsaObservationOutcomeV1::Error(code) => {
                encoder.u8(3);
                encoder.u16(code as u16);
                encoder.zeros(5 + ADMITTED_PAYLOAD_BYTES_V1);
            }
        }
        encoder.u64(0);
        encoder.finish();
        encoded
    }
}

fn encode_admitted(encoder: &mut FrameEncoder<'_>, admitted: AdmittedSourceIsaObservationV1) {
    encoder.bytes(&admitted.correlation());
    let artifact = admitted.artifact();
    encoder.bytes(&artifact.sha256());
    encoder.u64(artifact.byte_len());
    let structural = admitted.structural();
    encoder.bytes(&structural.identity());
    encoder.u8(structural.target_profile() as u8);
    encoder.zeros(7);
    encoder.u16(structural.kir_version() as u16);
    encoder.zeros(6);
    let neutral_kir = structural.neutral_kir();
    encoder.bytes(&neutral_kir.sha256());
    encoder.u64(neutral_kir.byte_len());
    let target_kir = structural.target_kir();
    encoder.bytes(&target_kir.sha256());
    encoder.u64(target_kir.byte_len());
    let structural_counts = structural.counts();
    for count in [
        structural_counts.functions,
        structural_counts.defined_bodies,
        structural_counts.blocks,
        structural_counts.operations,
    ] {
        encoder.u64(count);
    }
    for count in admitted.counts().values() {
        encoder.u64(count);
    }
    match admitted.round_trip_witness() {
        Some(witness) => {
            encoder.u8(1);
            encoder.zeros(7);
            encoder.bytes(&witness.source_node_identity());
            let source_span = witness.source_span();
            encoder.bytes(&source_span.file_identity());
            encoder.u64(source_span.byte_start());
            encoder.u64(source_span.byte_end());
            encoder.u32(source_span.line());
            encoder.u32(source_span.column());
            let isa_point = witness.isa_point();
            encoder.u64(isa_point.kernel_ordinal());
            encoder.u64(isa_point.symbol_relative_pc());
            encoder.u64(witness.source_node_query_matches());
            encoder.u64(witness.source_span_query_matches());
            encoder.u64(witness.isa_point_query_matches());
        }
        None => encoder.zeros(136),
    }
}

fn decode_admitted(
    encoded: &[u8],
) -> Result<AdmittedSourceIsaObservationV1, SourceIsaObservationFrameErrorV1> {
    let mut decoder = FrameDecoder::new(encoded);
    let correlation = decoder.array()?;
    let artifact = SourceIsaObservationContentIdentityV1::new(decoder.array()?, decoder.u64()?)?;
    let structural_identity = decoder.array()?;
    let target_profile = SourceIsaObservationTargetProfileV1::decode(decoder.u8()?)?;
    decoder.zeros(7)?;
    let kir_version = SourceIsaObservationKirVersionV1::decode(decoder.u16()?)?;
    decoder.zeros(6)?;
    let neutral_kir = SourceIsaObservationContentIdentityV1::new(decoder.array()?, decoder.u64()?)?;
    let target_kir = SourceIsaObservationContentIdentityV1::new(decoder.array()?, decoder.u64()?)?;
    let structural_counts = SourceIsaObservationStructuralCountsV1 {
        functions: decoder.u64()?,
        defined_bodies: decoder.u64()?,
        blocks: decoder.u64()?,
        operations: decoder.u64()?,
    };
    let structural = SourceIsaObservationStructuralBindingV1::new(
        structural_identity,
        target_profile,
        kir_version,
        neutral_kir,
        target_kir,
        structural_counts,
    )?;
    let mut values = [0; COUNT_FIELDS_V1];
    for value in &mut values {
        *value = decoder.u64()?;
    }
    let counts = SourceIsaObservationCountsV1::new(
        SourceIsaObservationRecordCountsV1 {
            records: values[0],
            source_anchored: values[1],
            eliminated: values[2],
            no_source: values[3],
            source_anchored_without_isa: values[4],
            isa_references: values[5],
        },
        SourceIsaObservationQueryCountsV1 {
            distinct_source_nodes: values[6],
            distinct_source_spans: values[7],
            distinct_isa_points: values[8],
            max_source_node_cardinality: values[9],
            max_source_span_cardinality: values[10],
            max_exact_pc_cardinality: values[11],
        },
    )?;
    let round_trip_witness = match decoder.u8()? {
        0 => {
            decoder.zeros(135)?;
            None
        }
        1 => {
            decoder.zeros(7)?;
            let source_node_identity = decoder.array()?;
            let source_span = SourceIsaObservationSourceSpanV1::new(
                decoder.array()?,
                decoder.u64()?,
                decoder.u64()?,
                decoder.u32()?,
                decoder.u32()?,
            )?;
            let isa_point = SourceIsaObservationIsaPointV1::new(decoder.u64()?, decoder.u64()?)?;
            Some(SourceIsaObservationRoundTripWitnessV1::new(
                source_node_identity,
                source_span,
                isa_point,
                decoder.u64()?,
                decoder.u64()?,
                decoder.u64()?,
            )?)
        }
        _ => return Err(SourceIsaObservationFrameErrorV1::InvalidTag),
    };
    decoder.finish()?;
    AdmittedSourceIsaObservationV1::new(
        correlation,
        artifact,
        structural,
        counts,
        round_trip_witness,
    )
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SourceIsaObservationFrameErrorV1 {
    Malformed,
    InvalidClaim,
    InvalidTag,
    Identity,
    TruthClaim,
}

impl fmt::Display for SourceIsaObservationFrameErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Malformed => formatter.write_str("malformed source/ISA observation frame"),
            Self::InvalidClaim => formatter.write_str("invalid source/ISA observation claim"),
            Self::InvalidTag => formatter.write_str("invalid source/ISA observation tag"),
            Self::Identity => formatter.write_str("source/ISA observation identity differs"),
            Self::TruthClaim => {
                formatter.write_str("source/ISA observation truth claims are nonzero")
            }
        }
    }
}

impl Error for SourceIsaObservationFrameErrorV1 {}

fn frame_identity(encoded_without_identity: &[u8]) -> [u8; 32] {
    let mut digest = Sha256::new();
    digest.update(FRAME_IDENTITY_DOMAIN_V1);
    digest.update((encoded_without_identity.len() as u64).to_le_bytes());
    digest.update(encoded_without_identity);
    digest.finalize().into()
}

struct FrameEncoder<'encoded> {
    encoded: &'encoded mut [u8],
    offset: usize,
}

impl<'encoded> FrameEncoder<'encoded> {
    fn new(encoded: &'encoded mut [u8]) -> Self {
        Self { encoded, offset: 0 }
    }

    fn bytes(&mut self, value: &[u8]) {
        let end = self.offset + value.len();
        self.encoded[self.offset..end].copy_from_slice(value);
        self.offset = end;
    }

    fn u8(&mut self, value: u8) {
        self.bytes(&[value]);
    }

    fn u16(&mut self, value: u16) {
        self.bytes(&value.to_le_bytes());
    }

    fn u32(&mut self, value: u32) {
        self.bytes(&value.to_le_bytes());
    }

    fn u64(&mut self, value: u64) {
        self.bytes(&value.to_le_bytes());
    }

    fn zeros(&mut self, count: usize) {
        self.offset += count;
    }

    fn finish(self) {
        debug_assert_eq!(self.offset, self.encoded.len());
    }
}

struct FrameDecoder<'encoded> {
    remaining: &'encoded [u8],
}

impl<'encoded> FrameDecoder<'encoded> {
    const fn new(encoded: &'encoded [u8]) -> Self {
        Self { remaining: encoded }
    }

    fn take(&mut self, count: usize) -> Result<&'encoded [u8], SourceIsaObservationFrameErrorV1> {
        if self.remaining.len() < count {
            return Err(SourceIsaObservationFrameErrorV1::Malformed);
        }
        let (value, remaining) = self.remaining.split_at(count);
        self.remaining = remaining;
        Ok(value)
    }

    fn array<const N: usize>(&mut self) -> Result<[u8; N], SourceIsaObservationFrameErrorV1> {
        Ok(self.take(N)?.try_into().expect("fixed decoded field"))
    }

    fn u8(&mut self) -> Result<u8, SourceIsaObservationFrameErrorV1> {
        Ok(self.array::<1>()?[0])
    }

    fn u16(&mut self) -> Result<u16, SourceIsaObservationFrameErrorV1> {
        Ok(u16::from_le_bytes(self.array()?))
    }

    fn u32(&mut self) -> Result<u32, SourceIsaObservationFrameErrorV1> {
        Ok(u32::from_le_bytes(self.array()?))
    }

    fn u64(&mut self) -> Result<u64, SourceIsaObservationFrameErrorV1> {
        Ok(u64::from_le_bytes(self.array()?))
    }

    fn zeros(&mut self, count: usize) -> Result<(), SourceIsaObservationFrameErrorV1> {
        if self.take(count)?.iter().any(|byte| *byte != 0) {
            return Err(SourceIsaObservationFrameErrorV1::Malformed);
        }
        Ok(())
    }

    fn finish(self) -> Result<(), SourceIsaObservationFrameErrorV1> {
        self.remaining
            .is_empty()
            .then_some(())
            .ok_or(SourceIsaObservationFrameErrorV1::Malformed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn attempt(
        generation: u64,
        session: [u8; 16],
        invocation: [u8; 32],
    ) -> SourceIsaObservationAttemptV1 {
        SourceIsaObservationAttemptV1::new(
            generation,
            SourceIsaObservationSessionV1::from_bytes(session),
            SourceIsaObservationInvocationV1::from_bytes(invocation),
        )
        .unwrap()
    }

    fn context() -> SourceIsaObservationContextV1 {
        SourceIsaObservationContextV1::new(
            [0x11; 32],
            [0x12; 32],
            attempt(7, [0x13; 16], [0x14; 32]),
            [0x15; 32],
        )
        .unwrap()
    }

    fn admitted() -> AdmittedSourceIsaObservationV1 {
        let content =
            |byte, length| SourceIsaObservationContentIdentityV1::new([byte; 32], length).unwrap();
        let structural = SourceIsaObservationStructuralBindingV1::new(
            [0x19; 32],
            SourceIsaObservationTargetProfileV1::Gfx942,
            SourceIsaObservationKirVersionV1::V8,
            content(0x20, 100),
            content(0x21, 110),
            SourceIsaObservationStructuralCountsV1 {
                functions: 2,
                defined_bodies: 1,
                blocks: 3,
                operations: 6,
            },
        )
        .unwrap();
        let counts = SourceIsaObservationCountsV1::new(
            SourceIsaObservationRecordCountsV1 {
                records: 8,
                source_anchored: 5,
                eliminated: 2,
                no_source: 1,
                source_anchored_without_isa: 1,
                isa_references: 6,
            },
            SourceIsaObservationQueryCountsV1 {
                distinct_source_nodes: 4,
                distinct_source_spans: 3,
                distinct_isa_points: 5,
                max_source_node_cardinality: 2,
                max_source_span_cardinality: 3,
                max_exact_pc_cardinality: 2,
            },
        )
        .unwrap();
        let witness = SourceIsaObservationRoundTripWitnessV1::new(
            [0x22; 32],
            SourceIsaObservationSourceSpanV1::new([0x23; 32], 10, 14, 2, 3).unwrap(),
            SourceIsaObservationIsaPointV1::new(0, 16).unwrap(),
            2,
            3,
            2,
        )
        .unwrap();
        AdmittedSourceIsaObservationV1::new(
            [0x16; 32],
            content(0x17, 1234),
            structural,
            counts,
            Some(witness),
        )
        .unwrap()
    }

    fn frame(outcome: SourceIsaObservationOutcomeV1) -> SourceIsaObservationFrameV1 {
        SourceIsaObservationFrameV1::new(context(), outcome)
    }

    fn admitted_shape(
        operations: u64,
        source_anchored: u64,
        eliminated: u64,
        no_source: u64,
    ) -> Result<AdmittedSourceIsaObservationV1, SourceIsaObservationFrameErrorV1> {
        let base = admitted();
        let structural = SourceIsaObservationStructuralBindingV1::new(
            [0x39; 32],
            SourceIsaObservationTargetProfileV1::Gfx942,
            SourceIsaObservationKirVersionV1::V8,
            base.structural().neutral_kir(),
            base.structural().target_kir(),
            SourceIsaObservationStructuralCountsV1 {
                functions: 1,
                defined_bodies: 1,
                blocks: 3,
                operations,
            },
        )?;
        let source_records = source_anchored
            .checked_add(eliminated)
            .ok_or(SourceIsaObservationFrameErrorV1::InvalidClaim)?;
        let counts = SourceIsaObservationCountsV1::new(
            SourceIsaObservationRecordCountsV1 {
                records: source_records
                    .checked_add(no_source)
                    .ok_or(SourceIsaObservationFrameErrorV1::InvalidClaim)?,
                source_anchored,
                eliminated,
                no_source,
                source_anchored_without_isa: source_anchored,
                isa_references: 0,
            },
            SourceIsaObservationQueryCountsV1 {
                distinct_source_nodes: u64::from(source_records != 0),
                distinct_source_spans: u64::from(source_records != 0),
                distinct_isa_points: 0,
                max_source_node_cardinality: source_records,
                max_source_span_cardinality: source_records,
                max_exact_pc_cardinality: 0,
            },
        )?;
        AdmittedSourceIsaObservationV1::new([0x38; 32], base.artifact(), structural, counts, None)
    }

    #[test]
    fn all_typed_outcomes_round_trip_with_canonical_zeroing() {
        for expected in [
            frame(SourceIsaObservationOutcomeV1::Admitted(admitted())),
            frame(SourceIsaObservationOutcomeV1::Unavailable(
                SourceIsaObservationUnavailableReasonV1::SourceProjectionForKirV9,
            )),
            frame(SourceIsaObservationOutcomeV1::Error(
                SourceIsaObservationErrorCodeV1::AllocationFailure,
            )),
        ] {
            let encoded = expected.encode();
            assert_eq!(encoded.len(), 680);
            assert!(encoded.len() <= MAX_SOURCE_ISA_OBSERVATION_FRAME_BYTES_V1);
            assert_eq!(&encoded[..8], b"F2SISUM1");
            if !matches!(
                expected.outcome(),
                SourceIsaObservationOutcomeV1::Admitted(_)
            ) {
                assert!(encoded[176..640].iter().all(|byte| *byte == 0));
            }
            assert_eq!(SourceIsaObservationFrameV1::decode(&encoded), Ok(expected));
        }
    }

    #[test]
    fn frozen_pre_extraction_frame_and_collection_identities_are_stable() {
        // Captured from the frozen cargo-fe2o3 codec before it moved into this crate.
        let hex = |bytes: &[u8]| {
            bytes
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect::<String>()
        };
        let assert_frame = |frame: &SourceIsaObservationFrameV1,
                            expected_identity: &str,
                            expected_encoded_sha256: &str| {
            let encoded = frame.encode();
            assert_eq!(hex(&frame.identity()), expected_identity);
            assert_eq!(hex(&Sha256::digest(encoded)), expected_encoded_sha256);
        };
        let admitted = frame(SourceIsaObservationOutcomeV1::Admitted(admitted()));
        let unavailable = frame(SourceIsaObservationOutcomeV1::Unavailable(
            SourceIsaObservationUnavailableReasonV1::SourceProjectionForKirV9,
        ));
        let error = frame(SourceIsaObservationOutcomeV1::Error(
            SourceIsaObservationErrorCodeV1::AllocationFailure,
        ));
        assert_frame(
            &admitted,
            "ae1b38e52062cc4cc66e4cc44b64df7298f93aaba88d6c34a1d9d6fda481cfb1",
            "95264c97f69e2ff8c89ef6d2d60d8772d89b0a4d3457fc608ee5a62cb9119335",
        );
        assert_frame(
            &unavailable,
            "161cfb07d06bd7c234ad154f994437cd8f386d1bfd2048964c649d6bee61f572",
            "6d954a44306662fdc510a1b9a094d2cc81df3107ff0d92ff863ea7aa09d87d2c",
        );
        assert_frame(
            &error,
            "51862e25119fab532f4538ceaef92691c3fe380317f9d5a4ff81698551181c99",
            "d360ecf588981e81212940b3ae59177a55a8755c6e6188b807c3062f4eacc9dc",
        );

        let collection = SourceIsaObservationCollectionV1::from_collected(
            [0x11; 32],
            SourceIsaObservationSessionV1::from_bytes([0x13; 16]),
            vec![([0x12; 32], unavailable)],
            vec![[0x16; 32]],
            Some(SourceIsaObservationTransportFailureV1::MissingSelectedUnits),
        );
        let encoded = collection.encode_canonical().unwrap();
        assert_eq!(
            hex(&encoded[encoded.len() - SOURCE_ISA_COLLECTION_IDENTITY_BYTES_V1..]),
            "fb07983f01b7d71f5d4ef9946747f50983892aea1009b5189b5cfaa0d03bfc1e"
        );
        assert_eq!(
            hex(&Sha256::digest(&encoded)),
            "ed4cfaad0c268c3d786ce3fa5bc6ee800c125e43b1c1e4c2c017f4252be25423"
        );
        assert_eq!(
            SourceIsaObservationCollectionV1::decode_canonical(&encoded),
            Ok(collection)
        );
    }

    #[test]
    fn lossy_and_unassigned_error_codes_are_rejected() {
        let encoded = frame(SourceIsaObservationOutcomeV1::Error(
            SourceIsaObservationErrorCodeV1::InvalidKirToLlvmReplay,
        ))
        .encode();
        for code in [
            0,
            1,
            2,
            0x1000,
            0x1013,
            0x1100,
            0x111d,
            0x1200,
            0x120b,
            0x2000,
            0x2014,
            u16::MAX,
        ] {
            let mut changed = encoded;
            changed[169..171].copy_from_slice(&code.to_le_bytes());
            let identity = frame_identity(&changed[..FRAME_PREFIX_BYTES_V1]);
            changed[FRAME_PREFIX_BYTES_V1..].copy_from_slice(&identity);
            assert_eq!(
                SourceIsaObservationFrameV1::decode(&changed),
                Err(SourceIsaObservationFrameErrorV1::InvalidTag),
                "reserved error code {code:#06x} was accepted"
            );
        }
    }

    #[test]
    fn finalized_v5_correspondence_error_has_a_distinct_canonical_code() {
        let expected = frame(SourceIsaObservationOutcomeV1::Error(
            SourceIsaObservationErrorCodeV1::FinalizedMapInvalidBoundCorrespondenceV5,
        ));
        let encoded = expected.encode();

        assert_eq!(
            u16::from_le_bytes(encoded[169..171].try_into().unwrap()),
            0x1010
        );
        assert_eq!(SourceIsaObservationFrameV1::decode(&encoded), Ok(expected));
        assert_eq!(
            SourceIsaObservationErrorCodeV1::FinalizedMapInvalidBoundCorrespondenceV5.label(),
            "finalized-map-invalid-bound-correspondence-v5"
        );

        let multi_root = frame(SourceIsaObservationOutcomeV1::Error(
            SourceIsaObservationErrorCodeV1::FinalizedMapInvalidBoundMultiRootCorrespondenceV2,
        ));
        let encoded = multi_root.encode();
        assert_eq!(
            u16::from_le_bytes(encoded[169..171].try_into().unwrap()),
            0x1011
        );
        assert_eq!(
            SourceIsaObservationFrameV1::decode(&encoded),
            Ok(multi_root)
        );

        let instance_custody = frame(SourceIsaObservationOutcomeV1::Error(
            SourceIsaObservationErrorCodeV1::FinalizedMapInvalidInstanceCustodyV1,
        ));
        let encoded = instance_custody.encode();
        assert_eq!(
            u16::from_le_bytes(encoded[169..171].try_into().unwrap()),
            0x1012
        );
        assert_eq!(
            SourceIsaObservationFrameV1::decode(&encoded),
            Ok(instance_custody)
        );
    }

    #[test]
    fn admitted_structure_round_trips_with_empty_blocks() {
        let base = admitted();
        let structural = SourceIsaObservationStructuralBindingV1::new(
            [0x29; 32],
            SourceIsaObservationTargetProfileV1::Gfx942,
            SourceIsaObservationKirVersionV1::V8,
            base.structural().neutral_kir(),
            base.structural().target_kir(),
            SourceIsaObservationStructuralCountsV1 {
                functions: 2,
                defined_bodies: 1,
                blocks: 4,
                operations: 2,
            },
        )
        .unwrap();
        let counts = SourceIsaObservationCountsV1::new(
            SourceIsaObservationRecordCountsV1 {
                records: 2,
                source_anchored: 0,
                eliminated: 0,
                no_source: 2,
                source_anchored_without_isa: 0,
                isa_references: 0,
            },
            SourceIsaObservationQueryCountsV1 {
                distinct_source_nodes: 0,
                distinct_source_spans: 0,
                distinct_isa_points: 0,
                max_source_node_cardinality: 0,
                max_source_span_cardinality: 0,
                max_exact_pc_cardinality: 0,
            },
        )
        .unwrap();
        let admitted = AdmittedSourceIsaObservationV1::new(
            base.correlation(),
            base.artifact(),
            structural,
            counts,
            None,
        )
        .unwrap();
        let expected = frame(SourceIsaObservationOutcomeV1::Admitted(admitted));
        let decoded = SourceIsaObservationFrameV1::decode(&expected.encode()).unwrap();
        assert_eq!(decoded, expected);
        let SourceIsaObservationOutcomeV1::Admitted(decoded) = decoded.outcome() else {
            panic!("expected admitted observation");
        };
        assert_eq!(decoded.structural().counts().blocks, 4);
        assert_eq!(decoded.structural().counts().operations, 2);
    }

    #[test]
    fn admitted_record_classes_cover_structural_operations_exactly() {
        for admitted in [
            admitted_shape(1, 1, 0, 0).unwrap(),
            admitted_shape(2, 0, 0, 2).unwrap(),
            admitted_shape(1, 2, 0, 0).unwrap(),
            admitted_shape(1, 0, 2, 1).unwrap(),
        ] {
            let expected = frame(SourceIsaObservationOutcomeV1::Admitted(admitted));
            assert_eq!(
                SourceIsaObservationFrameV1::decode(&expected.encode()),
                Ok(expected)
            );
        }

        for rejected in [
            admitted_shape(1, 0, 0, 2),
            admitted_shape(1, 0, 0, 0),
            admitted_shape(1, 0, 2, 0),
            admitted_shape(1, 1, 0, 1),
            admitted_shape(2, 1, 0, 0),
            admitted_shape(0, 0, 0, 0),
        ] {
            assert_eq!(
                rejected,
                Err(SourceIsaObservationFrameErrorV1::InvalidClaim)
            );
        }
    }

    #[test]
    fn structural_function_bound_matches_canonical_kir() {
        let base = admitted().structural();
        for functions in [1, MAX_FUNCTIONS_V1 as u64] {
            assert!(
                SourceIsaObservationStructuralBindingV1::new(
                    [0x49; 32],
                    base.target_profile(),
                    base.kir_version(),
                    base.neutral_kir(),
                    base.target_kir(),
                    SourceIsaObservationStructuralCountsV1 {
                        functions,
                        defined_bodies: 1,
                        blocks: 1,
                        operations: 1,
                    },
                )
                .is_ok()
            );
        }
        for functions in [0, MAX_FUNCTIONS_V1 as u64 + 1, u64::MAX] {
            assert!(
                SourceIsaObservationStructuralBindingV1::new(
                    [0x49; 32],
                    base.target_profile(),
                    base.kir_version(),
                    base.neutral_kir(),
                    base.target_kir(),
                    SourceIsaObservationStructuralCountsV1 {
                        functions,
                        defined_bodies: 1,
                        blocks: 1,
                        operations: 1,
                    },
                )
                .is_err()
            );
        }
    }

    #[test]
    fn valid_zero_population_and_eliminated_only_summaries_are_representable() {
        for records in [
            SourceIsaObservationRecordCountsV1 {
                records: 0,
                source_anchored: 0,
                eliminated: 0,
                no_source: 0,
                source_anchored_without_isa: 0,
                isa_references: 0,
            },
            SourceIsaObservationRecordCountsV1 {
                records: 8,
                source_anchored: 0,
                eliminated: 2,
                no_source: 6,
                source_anchored_without_isa: 0,
                isa_references: 0,
            },
            SourceIsaObservationRecordCountsV1 {
                records: 1,
                source_anchored: 1,
                eliminated: 0,
                no_source: 0,
                source_anchored_without_isa: 0,
                isa_references: 5,
            },
        ] {
            let duplicate_pc = records.isa_references == 5;
            let eliminated_only = records.eliminated != 0;
            assert!(
                SourceIsaObservationCountsV1::new(
                    records,
                    SourceIsaObservationQueryCountsV1 {
                        distinct_source_nodes: if eliminated_only || duplicate_pc {
                            1
                        } else {
                            0
                        },
                        distinct_source_spans: if eliminated_only || duplicate_pc {
                            1
                        } else {
                            0
                        },
                        distinct_isa_points: if duplicate_pc { 1 } else { 0 },
                        max_source_node_cardinality: if eliminated_only {
                            2
                        } else {
                            if duplicate_pc { 1 } else { 0 }
                        },
                        max_source_span_cardinality: if eliminated_only {
                            2
                        } else {
                            if duplicate_pc { 1 } else { 0 }
                        },
                        max_exact_pc_cardinality: if duplicate_pc { 5 } else { 0 },
                    }
                )
                .is_ok()
            );
        }
    }

    #[test]
    fn admitted_eliminated_source_records_have_a_canonical_absent_witness() {
        let base = admitted();
        let operations = base.structural().counts().operations;
        let counts = SourceIsaObservationCountsV1::new(
            SourceIsaObservationRecordCountsV1 {
                records: operations.checked_add(2).unwrap(),
                source_anchored: 0,
                eliminated: 2,
                no_source: operations,
                source_anchored_without_isa: 0,
                isa_references: 0,
            },
            SourceIsaObservationQueryCountsV1 {
                distinct_source_nodes: 1,
                distinct_source_spans: 1,
                distinct_isa_points: 0,
                max_source_node_cardinality: 2,
                max_source_span_cardinality: 2,
                max_exact_pc_cardinality: 0,
            },
        )
        .unwrap();
        let admitted = AdmittedSourceIsaObservationV1::new(
            base.correlation(),
            base.artifact(),
            base.structural(),
            counts,
            None,
        )
        .unwrap();
        let expected = frame(SourceIsaObservationOutcomeV1::Admitted(admitted));
        let encoded = expected.encode();
        assert!(encoded[504..640].iter().all(|byte| *byte == 0));
        assert_eq!(SourceIsaObservationFrameV1::decode(&encoded), Ok(expected));
    }

    #[test]
    fn admitted_backend_eliminated_anchors_have_no_round_trip_witness() {
        let base = admitted();
        let counts = SourceIsaObservationCountsV1::new(
            SourceIsaObservationRecordCountsV1 {
                records: 6,
                source_anchored: 6,
                eliminated: 0,
                no_source: 0,
                source_anchored_without_isa: 6,
                isa_references: 0,
            },
            SourceIsaObservationQueryCountsV1 {
                distinct_source_nodes: 6,
                distinct_source_spans: 6,
                distinct_isa_points: 0,
                max_source_node_cardinality: 1,
                max_source_span_cardinality: 1,
                max_exact_pc_cardinality: 0,
            },
        )
        .unwrap();
        let admitted = AdmittedSourceIsaObservationV1::new(
            base.correlation(),
            base.artifact(),
            base.structural(),
            counts,
            None,
        )
        .unwrap();
        assert_eq!(admitted.round_trip_witness(), None);
        assert_eq!(
            SourceIsaObservationFrameV1::decode(
                &frame(SourceIsaObservationOutcomeV1::Admitted(admitted)).encode()
            )
            .unwrap()
            .outcome(),
            SourceIsaObservationOutcomeV1::Admitted(admitted)
        );
    }

    #[test]
    fn impossible_query_cardinality_distributions_are_rejected() {
        let records = SourceIsaObservationRecordCountsV1 {
            records: 5,
            source_anchored: 5,
            eliminated: 0,
            no_source: 0,
            source_anchored_without_isa: 0,
            isa_references: 5,
        };
        for queries in [
            SourceIsaObservationQueryCountsV1 {
                distinct_source_nodes: 3,
                distinct_source_spans: 1,
                distinct_isa_points: 1,
                max_source_node_cardinality: 1,
                max_source_span_cardinality: 5,
                max_exact_pc_cardinality: 5,
            },
            SourceIsaObservationQueryCountsV1 {
                distinct_source_nodes: 3,
                distinct_source_spans: 1,
                distinct_isa_points: 1,
                max_source_node_cardinality: 4,
                max_source_span_cardinality: 5,
                max_exact_pc_cardinality: 5,
            },
        ] {
            assert_eq!(
                SourceIsaObservationCountsV1::new(records, queries),
                Err(SourceIsaObservationFrameErrorV1::InvalidClaim)
            );
        }
    }

    #[test]
    fn frame_rejects_every_single_byte_mutation() {
        let expected = frame(SourceIsaObservationOutcomeV1::Admitted(admitted()));
        let encoded = expected.encode();
        for index in 0..encoded.len() {
            let mut changed = encoded;
            changed[index] ^= 1;
            assert_ne!(
                SourceIsaObservationFrameV1::decode(&changed),
                Ok(expected.clone()),
                "byte {index} was not bound"
            );
        }
    }

    #[test]
    fn reserved_tags_payload_zeroing_and_truth_claims_are_exhaustive() {
        let unavailable = frame(SourceIsaObservationOutcomeV1::Unavailable(
            SourceIsaObservationUnavailableReasonV1::CarrierSourceMapUnavailable,
        ));
        for offset in [171, 176, 639, 640] {
            let mut encoded = unavailable.encode();
            encoded[offset] = 1;
            let identity = frame_identity(&encoded[..FRAME_PREFIX_BYTES_V1]);
            encoded[FRAME_PREFIX_BYTES_V1..].copy_from_slice(&identity);
            assert!(SourceIsaObservationFrameV1::decode(&encoded).is_err());
        }
    }

    #[test]
    fn witness_presence_and_cardinalities_are_bound_to_counts() {
        let admitted = admitted();
        let encoded = frame(SourceIsaObservationOutcomeV1::Admitted(admitted)).encode();
        let decoded = SourceIsaObservationFrameV1::decode(&encoded).unwrap();
        let SourceIsaObservationOutcomeV1::Admitted(decoded) = decoded.outcome() else {
            panic!("expected admitted outcome");
        };
        assert_eq!(decoded.round_trip_witness(), admitted.round_trip_witness());

        for offset in [504, 505, 600, 608, 616, 624, 632] {
            let mut changed = encoded;
            changed[offset] = changed[offset].wrapping_add(1);
            let identity = frame_identity(&changed[..FRAME_PREFIX_BYTES_V1]);
            changed[FRAME_PREFIX_BYTES_V1..].copy_from_slice(&identity);
            assert!(SourceIsaObservationFrameV1::decode(&changed).is_err());
        }
    }

    #[test]
    fn kir_replay_v7_is_not_a_valid_structural_version() {
        let mut encoded = frame(SourceIsaObservationOutcomeV1::Admitted(admitted())).encode();
        encoded[288..290].copy_from_slice(&7_u16.to_le_bytes());
        let identity = frame_identity(&encoded[..FRAME_PREFIX_BYTES_V1]);
        encoded[FRAME_PREFIX_BYTES_V1..].copy_from_slice(&identity);
        assert_eq!(
            SourceIsaObservationFrameV1::decode(&encoded),
            Err(SourceIsaObservationFrameErrorV1::InvalidTag)
        );
    }

    #[test]
    fn impossible_structural_and_count_claims_fail_after_rehashing() {
        let encoded = frame(SourceIsaObservationOutcomeV1::Admitted(admitted())).encode();
        for (offset, value) in [(376, u64::MAX), (384, 2_u64), (400, 7_u64), (408, 9_u64)] {
            let mut changed = encoded;
            changed[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
            let identity = frame_identity(&changed[..FRAME_PREFIX_BYTES_V1]);
            changed[FRAME_PREFIX_BYTES_V1..].copy_from_slice(&identity);
            assert_eq!(
                SourceIsaObservationFrameV1::decode(&changed),
                Err(SourceIsaObservationFrameErrorV1::InvalidClaim)
            );
        }

        let mut zero_records = encoded;
        zero_records[408..640].fill(0);
        let identity = frame_identity(&zero_records[..FRAME_PREFIX_BYTES_V1]);
        zero_records[FRAME_PREFIX_BYTES_V1..].copy_from_slice(&identity);
        assert_eq!(
            SourceIsaObservationFrameV1::decode(&zero_records),
            Err(SourceIsaObservationFrameErrorV1::InvalidClaim)
        );
    }
}
