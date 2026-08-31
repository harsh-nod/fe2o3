//! Authority-free bounded source/ISA observation framing for production qualification.

use std::error::Error;
use std::fmt;

use fe2o3_artifact_transaction::{BuildAttempt, BuildInvocation, BuildSession};
use sha2::{Digest, Sha256};

const FRAME_MAGIC_V1: &[u8; 8] = b"F2SISUM1";
const FRAME_VERSION_V1: u16 = 1;
const FRAME_HEADER_BYTES_V1: usize = 16;
const FRAME_IDENTITY_BYTES_V1: usize = 32;
const FRAME_PREFIX_BYTES_V1: usize = 648;
const ADMITTED_PAYLOAD_BYTES_V1: usize = 464;
const COUNT_FIELDS_V1: usize = 12;
const FRAME_IDENTITY_DOMAIN_V1: &[u8] = b"FE2O3/SOURCE-ISA-SUMMARY-FRAME/V1\0";
const MAX_SOURCE_ISA_RECORDS_V1: u64 = 528_384;
const MAX_SOURCE_ISA_REFERENCES_V1: u64 = 1_016_800;
const MAX_PRODUCTION_STRUCTURAL_OPERATIONS_V1: u64 = 4 * 1024;

pub(crate) const SOURCE_ISA_OBSERVATION_FRAME_BYTES_V1: usize =
    FRAME_PREFIX_BYTES_V1 + FRAME_IDENTITY_BYTES_V1;
pub(crate) const MAX_SOURCE_ISA_OBSERVATION_FRAME_BYTES_V1: usize = 4096;
const _: () =
    assert!(SOURCE_ISA_OBSERVATION_FRAME_BYTES_V1 <= MAX_SOURCE_ISA_OBSERVATION_FRAME_BYTES_V1);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct SourceIsaObservationContentIdentityV1 {
    sha256: [u8; 32],
    byte_len: u64,
}

impl SourceIsaObservationContentIdentityV1 {
    pub(crate) fn new(
        sha256: [u8; 32],
        byte_len: u64,
    ) -> Result<Self, SourceIsaObservationFrameErrorV1> {
        if sha256 == [0; 32] || byte_len == 0 {
            return Err(SourceIsaObservationFrameErrorV1::InvalidClaim);
        }
        Ok(Self { sha256, byte_len })
    }

    pub(crate) const fn sha256(self) -> [u8; 32] {
        self.sha256
    }

    pub(crate) const fn byte_len(self) -> u64 {
        self.byte_len
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub(crate) enum SourceIsaObservationTargetProfileV1 {
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
pub(crate) enum SourceIsaObservationKirVersionV1 {
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
pub(crate) struct SourceIsaObservationStructuralCountsV1 {
    pub(crate) functions: u64,
    pub(crate) defined_bodies: u64,
    pub(crate) blocks: u64,
    pub(crate) operations: u64,
}

impl SourceIsaObservationStructuralCountsV1 {
    fn validate(self) -> Result<(), SourceIsaObservationFrameErrorV1> {
        if self.functions == 0
            || self.defined_bodies != 1
            || self.blocks == 0
            || self.blocks > MAX_PRODUCTION_STRUCTURAL_OPERATIONS_V1
            || self.operations < self.blocks
            || self.operations > MAX_PRODUCTION_STRUCTURAL_OPERATIONS_V1
        {
            return Err(SourceIsaObservationFrameErrorV1::InvalidClaim);
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct SourceIsaObservationStructuralBindingV1 {
    identity: [u8; 32],
    target_profile: SourceIsaObservationTargetProfileV1,
    kir_version: SourceIsaObservationKirVersionV1,
    neutral_kir: SourceIsaObservationContentIdentityV1,
    target_kir: SourceIsaObservationContentIdentityV1,
    counts: SourceIsaObservationStructuralCountsV1,
}

impl SourceIsaObservationStructuralBindingV1 {
    pub(crate) fn new(
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

    pub(crate) const fn identity(self) -> [u8; 32] {
        self.identity
    }

    pub(crate) const fn target_profile(self) -> SourceIsaObservationTargetProfileV1 {
        self.target_profile
    }

    pub(crate) const fn kir_version(self) -> SourceIsaObservationKirVersionV1 {
        self.kir_version
    }

    pub(crate) const fn neutral_kir(self) -> SourceIsaObservationContentIdentityV1 {
        self.neutral_kir
    }

    pub(crate) const fn target_kir(self) -> SourceIsaObservationContentIdentityV1 {
        self.target_kir
    }

    pub(crate) const fn counts(self) -> SourceIsaObservationStructuralCountsV1 {
        self.counts
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct SourceIsaObservationRecordCountsV1 {
    pub(crate) records: u64,
    pub(crate) source_anchored: u64,
    pub(crate) eliminated: u64,
    pub(crate) no_source: u64,
    pub(crate) source_anchored_without_isa: u64,
    pub(crate) isa_references: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct SourceIsaObservationQueryCountsV1 {
    pub(crate) distinct_source_nodes: u64,
    pub(crate) distinct_source_spans: u64,
    pub(crate) distinct_isa_points: u64,
    pub(crate) max_source_node_cardinality: u64,
    pub(crate) max_source_span_cardinality: u64,
    pub(crate) max_exact_pc_cardinality: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct SourceIsaObservationCountsV1 {
    records: SourceIsaObservationRecordCountsV1,
    queries: SourceIsaObservationQueryCountsV1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct SourceIsaObservationSourceSpanV1 {
    file_identity: [u8; 32],
    byte_start: u64,
    byte_end: u64,
    line: u32,
    column: u32,
}

impl SourceIsaObservationSourceSpanV1 {
    pub(crate) fn new(
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

    pub(crate) const fn file_identity(self) -> [u8; 32] {
        self.file_identity
    }

    pub(crate) const fn byte_start(self) -> u64 {
        self.byte_start
    }

    pub(crate) const fn byte_end(self) -> u64 {
        self.byte_end
    }

    pub(crate) const fn line(self) -> u32 {
        self.line
    }

    pub(crate) const fn column(self) -> u32 {
        self.column
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct SourceIsaObservationIsaPointV1 {
    kernel_ordinal: u64,
    symbol_relative_pc: u64,
}

impl SourceIsaObservationIsaPointV1 {
    pub(crate) fn new(
        kernel_ordinal: u64,
        symbol_relative_pc: u64,
    ) -> Result<Self, SourceIsaObservationFrameErrorV1> {
        if kernel_ordinal != 0 || symbol_relative_pc % 4 != 0 {
            return Err(SourceIsaObservationFrameErrorV1::InvalidClaim);
        }
        Ok(Self {
            kernel_ordinal,
            symbol_relative_pc,
        })
    }

    pub(crate) const fn kernel_ordinal(self) -> u64 {
        self.kernel_ordinal
    }

    pub(crate) const fn symbol_relative_pc(self) -> u64 {
        self.symbol_relative_pc
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct SourceIsaObservationRoundTripWitnessV1 {
    source_node_identity: [u8; 32],
    source_span: SourceIsaObservationSourceSpanV1,
    isa_point: SourceIsaObservationIsaPointV1,
    source_node_query_matches: u64,
    source_span_query_matches: u64,
    isa_point_query_matches: u64,
}

impl SourceIsaObservationRoundTripWitnessV1 {
    pub(crate) fn new(
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

    pub(crate) const fn source_node_identity(self) -> [u8; 32] {
        self.source_node_identity
    }

    pub(crate) const fn source_span(self) -> SourceIsaObservationSourceSpanV1 {
        self.source_span
    }

    pub(crate) const fn isa_point(self) -> SourceIsaObservationIsaPointV1 {
        self.isa_point
    }

    pub(crate) const fn source_node_query_matches(self) -> u64 {
        self.source_node_query_matches
    }

    pub(crate) const fn source_span_query_matches(self) -> u64 {
        self.source_span_query_matches
    }

    pub(crate) const fn isa_point_query_matches(self) -> u64 {
        self.isa_point_query_matches
    }
}

impl SourceIsaObservationCountsV1 {
    pub(crate) fn new(
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

    pub(crate) const fn records(self) -> SourceIsaObservationRecordCountsV1 {
        self.records
    }

    pub(crate) const fn queries(self) -> SourceIsaObservationQueryCountsV1 {
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
pub(crate) enum SourceIsaObservationUnavailableReasonV1 {
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
}

impl SourceIsaObservationUnavailableReasonV1 {
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
            _ => Err(SourceIsaObservationFrameErrorV1::InvalidTag),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u16)]
pub(crate) enum SourceIsaObservationErrorCodeV1 {
    SemanticDebugMap = 1,
    SemanticAnchors = 2,
    InvalidKirToLlvmReplay = 3,
    NonExactSemanticMap = 4,
    ArtifactIdentityMismatch = 5,
    TargetKirIdentityMismatch = 6,
    CoordinateShapeMismatch = 7,
    InvalidSourceGraph = 8,
    ResourceLimit = 9,
    AllocationFailure = 10,
}

impl SourceIsaObservationErrorCodeV1 {
    fn decode(value: u16) -> Result<Self, SourceIsaObservationFrameErrorV1> {
        match value {
            1 => Ok(Self::SemanticDebugMap),
            2 => Ok(Self::SemanticAnchors),
            3 => Ok(Self::InvalidKirToLlvmReplay),
            4 => Ok(Self::NonExactSemanticMap),
            5 => Ok(Self::ArtifactIdentityMismatch),
            6 => Ok(Self::TargetKirIdentityMismatch),
            7 => Ok(Self::CoordinateShapeMismatch),
            8 => Ok(Self::InvalidSourceGraph),
            9 => Ok(Self::ResourceLimit),
            10 => Ok(Self::AllocationFailure),
            _ => Err(SourceIsaObservationFrameErrorV1::InvalidTag),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct AdmittedSourceIsaObservationV1 {
    correlation: [u8; 32],
    artifact: SourceIsaObservationContentIdentityV1,
    structural: SourceIsaObservationStructuralBindingV1,
    counts: SourceIsaObservationCountsV1,
    round_trip_witness: Option<SourceIsaObservationRoundTripWitnessV1>,
}

impl AdmittedSourceIsaObservationV1 {
    pub(crate) fn new(
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
        if correlation == [0; 32]
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

    pub(crate) const fn correlation(self) -> [u8; 32] {
        self.correlation
    }

    pub(crate) const fn artifact(self) -> SourceIsaObservationContentIdentityV1 {
        self.artifact
    }

    pub(crate) const fn structural(self) -> SourceIsaObservationStructuralBindingV1 {
        self.structural
    }

    pub(crate) const fn counts(self) -> SourceIsaObservationCountsV1 {
        self.counts
    }

    pub(crate) const fn round_trip_witness(self) -> Option<SourceIsaObservationRoundTripWitnessV1> {
        self.round_trip_witness
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum SourceIsaObservationOutcomeV1 {
    Admitted(AdmittedSourceIsaObservationV1),
    Unavailable(SourceIsaObservationUnavailableReasonV1),
    Error(SourceIsaObservationErrorCodeV1),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct SourceIsaObservationContextV1 {
    config: [u8; 32],
    unit: [u8; 32],
    attempt: BuildAttempt,
    finalization: [u8; 32],
}

impl SourceIsaObservationContextV1 {
    pub(crate) fn new(
        config: [u8; 32],
        unit: [u8; 32],
        attempt: BuildAttempt,
        finalization: [u8; 32],
    ) -> Result<Self, SourceIsaObservationFrameErrorV1> {
        if config == [0; 32]
            || unit == [0; 32]
            || attempt.session() == BuildSession::DIRECT
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

    pub(crate) const fn config(self) -> [u8; 32] {
        self.config
    }

    pub(crate) const fn unit(self) -> [u8; 32] {
        self.unit
    }

    pub(crate) const fn attempt(self) -> BuildAttempt {
        self.attempt
    }

    pub(crate) const fn finalization(self) -> [u8; 32] {
        self.finalization
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct SourceIsaObservationFrameV1 {
    context: SourceIsaObservationContextV1,
    outcome: SourceIsaObservationOutcomeV1,
    identity: [u8; 32],
}

impl SourceIsaObservationFrameV1 {
    pub(crate) fn new(
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

    pub(crate) const fn context(&self) -> SourceIsaObservationContextV1 {
        self.context
    }

    pub(crate) const fn outcome(&self) -> SourceIsaObservationOutcomeV1 {
        self.outcome
    }

    pub(crate) const fn identity(&self) -> [u8; 32] {
        self.identity
    }

    pub(crate) fn encode(&self) -> [u8; SOURCE_ISA_OBSERVATION_FRAME_BYTES_V1] {
        let prefix = self.encode_prefix();
        let mut encoded = [0; SOURCE_ISA_OBSERVATION_FRAME_BYTES_V1];
        encoded[..FRAME_PREFIX_BYTES_V1].copy_from_slice(&prefix);
        encoded[FRAME_PREFIX_BYTES_V1..].copy_from_slice(&self.identity);
        encoded
    }

    pub(crate) fn decode(encoded: &[u8]) -> Result<Self, SourceIsaObservationFrameErrorV1> {
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
        let session = BuildSession::from_bytes(decoder.array()?);
        let invocation = BuildInvocation::from_bytes(decoder.array()?);
        let attempt = BuildAttempt::new(generation, session, invocation)
            .map_err(|_| SourceIsaObservationFrameErrorV1::InvalidClaim)?;
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
        encoder.bytes(&self.context.finalization);
        match self.outcome {
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
    encoder.bytes(&admitted.correlation);
    encoder.bytes(&admitted.artifact.sha256);
    encoder.u64(admitted.artifact.byte_len);
    let structural = admitted.structural;
    encoder.bytes(&structural.identity);
    encoder.u8(structural.target_profile as u8);
    encoder.zeros(7);
    encoder.u16(structural.kir_version as u16);
    encoder.zeros(6);
    encoder.bytes(&structural.neutral_kir.sha256);
    encoder.u64(structural.neutral_kir.byte_len);
    encoder.bytes(&structural.target_kir.sha256);
    encoder.u64(structural.target_kir.byte_len);
    for count in [
        structural.counts.functions,
        structural.counts.defined_bodies,
        structural.counts.blocks,
        structural.counts.operations,
    ] {
        encoder.u64(count);
    }
    for count in admitted.counts.values() {
        encoder.u64(count);
    }
    match admitted.round_trip_witness {
        Some(witness) => {
            encoder.u8(1);
            encoder.zeros(7);
            encoder.bytes(&witness.source_node_identity);
            encoder.bytes(&witness.source_span.file_identity);
            encoder.u64(witness.source_span.byte_start);
            encoder.u64(witness.source_span.byte_end);
            encoder.u32(witness.source_span.line);
            encoder.u32(witness.source_span.column);
            encoder.u64(witness.isa_point.kernel_ordinal);
            encoder.u64(witness.isa_point.symbol_relative_pc);
            encoder.u64(witness.source_node_query_matches);
            encoder.u64(witness.source_span_query_matches);
            encoder.u64(witness.isa_point_query_matches);
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
pub(crate) enum SourceIsaObservationFrameErrorV1 {
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

    fn context() -> SourceIsaObservationContextV1 {
        SourceIsaObservationContextV1::new(
            [0x11; 32],
            [0x12; 32],
            BuildAttempt::new(
                7,
                BuildSession::from_bytes([0x13; 16]),
                BuildInvocation::from_bytes([0x14; 32]),
            )
            .unwrap(),
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
                operations: 8,
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
                records: 2,
                source_anchored: 0,
                eliminated: 2,
                no_source: 0,
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
    fn admitted_eliminated_only_summary_has_a_canonical_absent_witness() {
        let base = admitted();
        let counts = SourceIsaObservationCountsV1::new(
            SourceIsaObservationRecordCountsV1 {
                records: 2,
                source_anchored: 0,
                eliminated: 2,
                no_source: 0,
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
                records: 2,
                source_anchored: 2,
                eliminated: 0,
                no_source: 0,
                source_anchored_without_isa: 2,
                isa_references: 0,
            },
            SourceIsaObservationQueryCountsV1 {
                distinct_source_nodes: 2,
                distinct_source_spans: 2,
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
        for (offset, value) in [(384, 2_u64), (408, 9_u64)] {
            let mut changed = encoded;
            changed[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
            let identity = frame_identity(&changed[..FRAME_PREFIX_BYTES_V1]);
            changed[FRAME_PREFIX_BYTES_V1..].copy_from_slice(&identity);
            assert_eq!(
                SourceIsaObservationFrameV1::decode(&changed),
                Err(SourceIsaObservationFrameErrorV1::InvalidClaim)
            );
        }
    }
}
