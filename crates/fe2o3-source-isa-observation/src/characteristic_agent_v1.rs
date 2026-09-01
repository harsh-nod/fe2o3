//! Bounded JSONL inspection of one preloaded, exactly admitted characteristic collection.

use std::error::Error;
use std::fmt;
use std::io::{BufRead, Write};

use serde::de::{self, IgnoredAny, MapAccess, Visitor};
use serde::{Deserialize, Deserializer, Serialize};

use crate::characteristic_v1::*;

pub const AGENT_SOURCE_ISA_CHARACTERISTIC_REQUEST_SCHEMA_V1: &str =
    "fe2o3-agent-source-isa-characteristic-request-v1";
pub const AGENT_SOURCE_ISA_CHARACTERISTIC_RESPONSE_SCHEMA_V1: &str =
    "fe2o3-agent-source-isa-characteristic-response-v1";
pub const MAX_AGENT_SOURCE_ISA_CHARACTERISTIC_REQUEST_BYTES_V1: usize = 2 * 1024 * 1024;
pub const MAX_AGENT_SOURCE_ISA_CHARACTERISTIC_RESPONSE_BYTES_V1: usize = 2 * 1024 * 1024;
pub const MAX_AGENT_SOURCE_ISA_CHARACTERISTIC_REQUESTS_V1: u32 = 4_096;
pub const MAX_AGENT_SOURCE_ISA_CHARACTERISTIC_PAGE_ITEMS_V1: u16 = 64;

const HEX_IDENTITY_BYTES_V1: usize = 64;
const HEX_CURSOR_BYTES_V1: usize = SOURCE_ISA_CHARACTERISTIC_CURSOR_BYTES_V1 * 2;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentSourceIsaCharacteristicOperationV1 {
    DiscoverCapabilities,
    QueryTargets,
    QueryFacts,
    QueryIntervals,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "operation", rename_all = "snake_case", deny_unknown_fields)]
pub enum AgentSourceIsaCharacteristicRequestV1 {
    DiscoverCapabilities {
        schema: String,
        request_id: u64,
    },
    QueryTargets {
        schema: String,
        request_id: u64,
        collection_identity: String,
        query: AgentSourceIsaCharacteristicTargetQueryV1,
        cursor: Option<String>,
        limit: u16,
    },
    QueryFacts {
        schema: String,
        request_id: u64,
        collection_identity: String,
        query: AgentSourceIsaCharacteristicQueryV1,
        cursor: Option<String>,
        limit: u16,
    },
    QueryIntervals {
        schema: String,
        request_id: u64,
        collection_identity: String,
        occurrence_identity: String,
        cursor: Option<String>,
        limit: u16,
    },
}

impl AgentSourceIsaCharacteristicRequestV1 {
    fn identity(&self) -> (u64, AgentSourceIsaCharacteristicOperationV1) {
        match self {
            Self::DiscoverCapabilities { request_id, .. } => (
                *request_id,
                AgentSourceIsaCharacteristicOperationV1::DiscoverCapabilities,
            ),
            Self::QueryTargets { request_id, .. } => (
                *request_id,
                AgentSourceIsaCharacteristicOperationV1::QueryTargets,
            ),
            Self::QueryFacts { request_id, .. } => (
                *request_id,
                AgentSourceIsaCharacteristicOperationV1::QueryFacts,
            ),
            Self::QueryIntervals { request_id, .. } => (
                *request_id,
                AgentSourceIsaCharacteristicOperationV1::QueryIntervals,
            ),
        }
    }

    fn schema(&self) -> &str {
        match self {
            Self::DiscoverCapabilities { schema, .. }
            | Self::QueryTargets { schema, .. }
            | Self::QueryFacts { schema, .. }
            | Self::QueryIntervals { schema, .. } => schema,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(tag = "selector", rename_all = "snake_case", deny_unknown_fields)]
pub enum AgentSourceIsaCharacteristicTargetQueryV1 {
    All,
    CharacteristicIdentity {
        identity: String,
    },
    Category {
        code: u16,
    },
    Kind {
        code: u16,
    },
    TargetKir {
        coordinate: AgentSourceIsaKirCoordinateV1,
    },
}

#[derive(Clone, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(tag = "selector", rename_all = "snake_case", deny_unknown_fields)]
pub enum AgentSourceIsaCharacteristicQueryV1 {
    All,
    CharacteristicIdentity {
        identity: String,
    },
    Category {
        code: u16,
    },
    Kind {
        code: u16,
    },
    SourceNode {
        identity: String,
    },
    SourceSpan {
        span: AgentSourceIsaSourceSpanV1,
    },
    RecordKind {
        code: u8,
    },
    MirNode {
        identity: String,
    },
    Mir {
        coordinate: AgentSourceIsaMirCoordinateV1,
    },
    NeutralKirNode {
        identity: String,
    },
    NeutralKir {
        coordinate: AgentSourceIsaKirCoordinateV1,
    },
    TargetKir {
        coordinate: AgentSourceIsaKirCoordinateV1,
    },
    SemanticOperation {
        identity: String,
    },
    CompilerHandoffLlvm {
        coordinate: AgentSourceIsaLlvmCoordinateV1,
    },
    Transformation {
        code: u8,
    },
    ExactPc {
        kernel_ordinal: u64,
        symbol_relative_pc: u64,
    },
    PreKirOnly,
}

#[derive(Clone, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AgentSourceIsaSourceSpanV1 {
    pub file_identity: String,
    pub byte_start: u64,
    pub byte_end: u64,
    pub line: u32,
    pub column: u32,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AgentSourceIsaMirCoordinateV1 {
    pub body_ordinal: u64,
    pub block_ordinal: u64,
    pub statement_ordinal: u64,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AgentSourceIsaKirCoordinateV1 {
    pub function_ordinal: u64,
    pub block_ordinal: u64,
    pub operation_ordinal: u64,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AgentSourceIsaLlvmCoordinateV1 {
    pub function_ordinal: u64,
    pub block_ordinal: u64,
    pub instruction_ordinal: u64,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct AgentSourceIsaCharacteristicAuthorityV1 {
    pub observation_only: bool,
    pub compiler_authority: bool,
    pub proof_authority: bool,
    pub publication_authority: bool,
    pub runtime_authority: bool,
    pub hardware_observation_authority: bool,
    pub complete_machine_instruction_coverage_proved: bool,
    pub schedule_proved: bool,
    pub semantic_refinement_proved: bool,
    pub final_llvm_classification_proved: bool,
    pub final_isa_opcode_classification_proved: bool,
    pub decoded_isa: bool,
    pub sparse_final_hsaco_anchors_present: bool,
}

impl AgentSourceIsaCharacteristicAuthorityV1 {
    fn for_collection(collection: &SourceIsaCharacteristicCollectionV1) -> Self {
        Self {
            observation_only: true,
            compiler_authority: false,
            proof_authority: false,
            publication_authority: false,
            runtime_authority: false,
            hardware_observation_authority: false,
            complete_machine_instruction_coverage_proved: false,
            schedule_proved: false,
            semantic_refinement_proved: false,
            final_llvm_classification_proved: false,
            final_isa_opcode_classification_proved: false,
            decoded_isa: false,
            sparse_final_hsaco_anchors_present: collection.has_sparse_final_hsaco_anchors(),
        }
    }

    fn has_valid_nonclaims(self) -> bool {
        self.observation_only
            && !self.compiler_authority
            && !self.proof_authority
            && !self.publication_authority
            && !self.runtime_authority
            && !self.hardware_observation_authority
            && !self.complete_machine_instruction_coverage_proved
            && !self.schedule_proved
            && !self.semantic_refinement_proved
            && !self.final_llvm_classification_proved
            && !self.final_isa_opcode_classification_proved
            && !self.decoded_isa
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct AgentSourceIsaCharacteristicLimitsV1 {
    pub max_request_bytes: usize,
    pub max_response_bytes: usize,
    pub max_collection_binary_bytes: usize,
    pub max_catalog_records: usize,
    pub max_targets: usize,
    pub max_target_correlations: usize,
    pub max_correlations_per_target: usize,
    pub max_pre_kir_eliminations: usize,
    pub max_sparse_intervals: usize,
    pub max_page_items: u16,
    pub max_requests: u32,
}

fn limits() -> AgentSourceIsaCharacteristicLimitsV1 {
    AgentSourceIsaCharacteristicLimitsV1 {
        max_request_bytes: MAX_AGENT_SOURCE_ISA_CHARACTERISTIC_REQUEST_BYTES_V1,
        max_response_bytes: MAX_AGENT_SOURCE_ISA_CHARACTERISTIC_RESPONSE_BYTES_V1,
        max_collection_binary_bytes: MAX_SOURCE_ISA_CHARACTERISTIC_COLLECTION_BYTES_V1,
        max_catalog_records: MAX_SOURCE_ISA_CHARACTERISTIC_CATALOG_RECORDS_V1,
        max_targets: MAX_SOURCE_ISA_CHARACTERISTIC_TARGETS_V1,
        max_target_correlations: MAX_SOURCE_ISA_CHARACTERISTIC_TARGET_CORRELATIONS_V1,
        max_correlations_per_target: MAX_SOURCE_ISA_CHARACTERISTIC_CORRELATIONS_PER_TARGET_V1,
        max_pre_kir_eliminations: MAX_SOURCE_ISA_CHARACTERISTIC_PRE_KIR_ELIMINATIONS_V1,
        max_sparse_intervals: MAX_SOURCE_ISA_CHARACTERISTIC_INTERVALS_V1,
        max_page_items: MAX_AGENT_SOURCE_ISA_CHARACTERISTIC_PAGE_ITEMS_V1,
        max_requests: MAX_AGENT_SOURCE_ISA_CHARACTERISTIC_REQUESTS_V1,
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentSourceIsaCharacteristicCapabilityStateV1 {
    Available,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct AgentSourceIsaCharacteristicCapabilityV1 {
    pub operation: AgentSourceIsaCharacteristicOperationV1,
    pub state: AgentSourceIsaCharacteristicCapabilityStateV1,
}

fn capabilities() -> [AgentSourceIsaCharacteristicCapabilityV1; 4] {
    [
        AgentSourceIsaCharacteristicCapabilityV1 {
            operation: AgentSourceIsaCharacteristicOperationV1::DiscoverCapabilities,
            state: AgentSourceIsaCharacteristicCapabilityStateV1::Available,
        },
        AgentSourceIsaCharacteristicCapabilityV1 {
            operation: AgentSourceIsaCharacteristicOperationV1::QueryTargets,
            state: AgentSourceIsaCharacteristicCapabilityStateV1::Available,
        },
        AgentSourceIsaCharacteristicCapabilityV1 {
            operation: AgentSourceIsaCharacteristicOperationV1::QueryFacts,
            state: AgentSourceIsaCharacteristicCapabilityStateV1::Available,
        },
        AgentSourceIsaCharacteristicCapabilityV1 {
            operation: AgentSourceIsaCharacteristicOperationV1::QueryIntervals,
            state: AgentSourceIsaCharacteristicCapabilityStateV1::Available,
        },
    ]
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct AgentSourceIsaContentIdentityV1 {
    pub sha256: String,
    pub byte_len: u64,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct AgentSourceIsaStructuralCountsV1 {
    pub functions: u64,
    pub defined_bodies: u64,
    pub blocks: u64,
    pub operations: u64,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentSourceIsaTargetProfileLabelV1 {
    Gfx942,
    Gfx950,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct AgentSourceIsaTargetProfileV1 {
    pub code: u8,
    pub label: AgentSourceIsaTargetProfileLabelV1,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct AgentSourceIsaCharacteristicBindingV1 {
    pub target_profile: AgentSourceIsaTargetProfileV1,
    pub kir_version: u8,
    pub structural_identity: String,
    pub structural_counts: AgentSourceIsaStructuralCountsV1,
    pub source_map_v2: AgentSourceIsaContentIdentityV1,
    pub neutral_kir: AgentSourceIsaContentIdentityV1,
    pub target_kir: AgentSourceIsaContentIdentityV1,
    pub artifact: AgentSourceIsaContentIdentityV1,
    pub catalog: AgentSourceIsaContentIdentityV1,
    pub structural_bridge: AgentSourceIsaContentIdentityV1,
    pub correlation_identity: String,
    pub semantic_map_identity: String,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentSourceIsaMissingReasonV1 {
    NoQualifyingTargetOperation,
    NoAdmittedCorrelation,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentSourceIsaUnavailableReasonV1 {
    CatalogUnavailable,
    StructuralBridgeUnavailable,
    ClassifierUnavailable,
    SourceProjectionUnavailable,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentSourceIsaObservationErrorV1 {
    InvalidCatalog,
    InvalidStructuralBridge,
    InvalidClassification,
    ConflictingEvidence,
    ResourceLimit,
    AllocationFailure,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum AgentSourceIsaCharacteristicScanStateV1 {
    Complete,
    Missing {
        code: u16,
        reason: AgentSourceIsaMissingReasonV1,
    },
    Unavailable {
        code: u16,
        reason: AgentSourceIsaUnavailableReasonV1,
    },
    Error {
        code: u16,
        error: AgentSourceIsaObservationErrorV1,
    },
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct AgentSourceIsaCharacteristicScanSummaryV1 {
    pub catalog_record_count: u64,
    pub catalog_records_scanned: u64,
    pub target_operation_count: u64,
    pub target_operations_scanned: u64,
    pub classified_target_count: u64,
    pub retained_target_correlation_count: u64,
    pub pre_kir_elimination_count: u64,
    pub correlation_count: u64,
    pub state: AgentSourceIsaCharacteristicScanStateV1,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct AgentSourceIsaCharacteristicCollectionSummaryV1 {
    pub format: String,
    pub identity: String,
    pub canonical_byte_len: u64,
    pub binding: AgentSourceIsaCharacteristicBindingV1,
    pub scan: AgentSourceIsaCharacteristicScanSummaryV1,
    pub target_count: u64,
    pub pre_kir_elimination_count: u64,
    pub sparse_final_hsaco_anchors_present: bool,
    pub decoded_isa: bool,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentSourceIsaCharacteristicCategoryLabelV1 {
    TargetKirGlobalStore,
    TargetKirWorkgroupLdsRead,
    TargetKirWorkgroupLdsWrite,
    TargetKirWorkgroupBarrier,
    TargetKirBf16MfmaExact,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct AgentSourceIsaCharacteristicCategoryV1 {
    pub code: u16,
    pub label: AgentSourceIsaCharacteristicCategoryLabelV1,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentSourceIsaMemoryFormLabelV1 {
    Plain,
    Guarded,
    MatrixTile,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct AgentSourceIsaMemoryFormV1 {
    pub code: u8,
    pub label: AgentSourceIsaMemoryFormLabelV1,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentSourceIsaCharacteristicKindLabelV1 {
    GlobalStore,
    WorkgroupLoad,
    WorkgroupStore,
    WorkgroupBarrier,
    Bf16F32M16N16K16Wave64MatrixMultiplyAccumulate,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct AgentSourceIsaCharacteristicKindV1 {
    pub code: u16,
    pub label: AgentSourceIsaCharacteristicKindLabelV1,
    pub memory_form: Option<AgentSourceIsaMemoryFormV1>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentSourceIsaRecordKindLabelV1 {
    EliminatedBeforeKir,
    SourceAnchored,
    NoSourceProvenance,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct AgentSourceIsaRecordKindV1 {
    pub code: u8,
    pub label: AgentSourceIsaRecordKindLabelV1,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentSourceIsaTransformationLabelV1 {
    Preserved,
    Duplicated,
    Coalesced,
    DuplicatedAndCoalesced,
    Eliminated,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct AgentSourceIsaTransformationV1 {
    pub code: u8,
    pub label: AgentSourceIsaTransformationLabelV1,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct AgentSourceIsaSourceCoordinateV1 {
    pub node_identity: String,
    pub span: AgentSourceIsaSourceSpanV1,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct AgentSourceIsaTargetCorrelationV1 {
    pub catalog_record_ordinal: u64,
    pub record_kind: AgentSourceIsaRecordKindV1,
    pub source: Option<AgentSourceIsaSourceCoordinateV1>,
    pub mir_node_identity: Option<String>,
    pub mir: Option<AgentSourceIsaMirCoordinateV1>,
    pub neutral_kir_node_identity: Option<String>,
    pub neutral_kir: Option<AgentSourceIsaKirCoordinateV1>,
    pub target_kir: AgentSourceIsaKirCoordinateV1,
    pub semantic_operation_identity: String,
    pub compiler_handoff_llvm: AgentSourceIsaLlvmCoordinateV1,
    pub interval_count: u64,
    pub transformation: AgentSourceIsaTransformationV1,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct AgentSourceIsaPreKirEliminationV1 {
    pub catalog_record_ordinal: u64,
    pub record_kind: AgentSourceIsaRecordKindV1,
    pub source: AgentSourceIsaSourceCoordinateV1,
    pub mir_node_identity: String,
    pub mir: AgentSourceIsaMirCoordinateV1,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "fact_kind", rename_all = "snake_case")]
pub enum AgentSourceIsaCharacteristicFactOutcomeV1 {
    TargetCorrelation {
        correlation: AgentSourceIsaTargetCorrelationV1,
    },
    PreKirElimination {
        elimination: AgentSourceIsaPreKirEliminationV1,
    },
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct AgentSourceIsaCharacteristicFactV1 {
    pub occurrence_identity: String,
    pub characteristic_identity: Option<String>,
    pub category: Option<AgentSourceIsaCharacteristicCategoryV1>,
    pub kind: Option<AgentSourceIsaCharacteristicKindV1>,
    pub target_kir: Option<AgentSourceIsaKirCoordinateV1>,
    pub outcome: AgentSourceIsaCharacteristicFactOutcomeV1,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct AgentSourceIsaCharacteristicFactPageV1 {
    pub query: AgentSourceIsaCharacteristicQueryV1,
    pub query_identity: String,
    pub cursor: Option<String>,
    pub next_cursor: Option<String>,
    pub total_matches: u64,
    pub page_exhausted: bool,
    pub facts: Vec<AgentSourceIsaCharacteristicFactV1>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct AgentSourceIsaCharacteristicTargetV1 {
    pub occurrence_identity: String,
    pub characteristic_identity: String,
    pub category: AgentSourceIsaCharacteristicCategoryV1,
    pub kind: AgentSourceIsaCharacteristicKindV1,
    pub target_kir: AgentSourceIsaKirCoordinateV1,
    pub correlation_count: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct AgentSourceIsaCharacteristicTargetPageV1 {
    pub query: AgentSourceIsaCharacteristicTargetQueryV1,
    pub query_identity: String,
    pub cursor: Option<String>,
    pub next_cursor: Option<String>,
    pub total_matches: u64,
    pub page_exhausted: bool,
    pub targets: Vec<AgentSourceIsaCharacteristicTargetV1>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct AgentSourceIsaIsaIntervalV1 {
    pub kernel_ordinal: u64,
    pub symbol_relative_start: u64,
    pub symbol_relative_end: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct AgentSourceIsaIntervalItemV1 {
    pub identity: String,
    pub ordinal: u64,
    pub interval: AgentSourceIsaIsaIntervalV1,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct AgentSourceIsaCharacteristicIntervalPageV1 {
    pub occurrence_identity: String,
    pub cursor: Option<String>,
    pub next_cursor: Option<String>,
    pub total_intervals: u64,
    pub page_exhausted: bool,
    pub intervals: Vec<AgentSourceIsaIntervalItemV1>,
}

#[derive(Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "result", rename_all = "snake_case", deny_unknown_fields)]
pub enum AgentSourceIsaCharacteristicResultV1 {
    Capabilities {
        authority: AgentSourceIsaCharacteristicAuthorityV1,
        limits: AgentSourceIsaCharacteristicLimitsV1,
        capabilities: [AgentSourceIsaCharacteristicCapabilityV1; 4],
        collection: AgentSourceIsaCharacteristicCollectionSummaryV1,
    },
    TargetPage {
        authority: AgentSourceIsaCharacteristicAuthorityV1,
        collection: AgentSourceIsaCharacteristicCollectionSummaryV1,
        page: AgentSourceIsaCharacteristicTargetPageV1,
    },
    FactPage {
        authority: AgentSourceIsaCharacteristicAuthorityV1,
        collection: AgentSourceIsaCharacteristicCollectionSummaryV1,
        page: AgentSourceIsaCharacteristicFactPageV1,
    },
    IntervalPage {
        authority: AgentSourceIsaCharacteristicAuthorityV1,
        collection: AgentSourceIsaCharacteristicCollectionSummaryV1,
        page: AgentSourceIsaCharacteristicIntervalPageV1,
    },
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentSourceIsaCharacteristicErrorCodeV1 {
    InvalidRequest,
    RequestTooLarge,
    InvalidRequestId,
    DuplicateRequestId,
    RequestBudgetExhausted,
    SchemaMismatch,
    CollectionIdentityMismatch,
    InvalidLowercaseHex,
    InvalidQuery,
    InvalidPageLimit,
    InvalidCursor,
    UnknownOccurrence,
    InvalidCollection,
    AllocationFailure,
    ResponseTooLarge,
}

impl fmt::Display for AgentSourceIsaCharacteristicErrorCodeV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "source/ISA characteristic agent error: {self:?}")
    }
}

impl Error for AgentSourceIsaCharacteristicErrorCodeV1 {}

#[derive(Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "status", rename_all = "snake_case", deny_unknown_fields)]
#[allow(clippy::large_enum_variant)]
pub enum AgentSourceIsaCharacteristicResponseV1 {
    Ok {
        schema: String,
        request_id: u64,
        response_revision: u64,
        operation: AgentSourceIsaCharacteristicOperationV1,
        result: AgentSourceIsaCharacteristicResultV1,
    },
    Error {
        schema: String,
        request_id: Option<u64>,
        response_revision: u64,
        operation: Option<AgentSourceIsaCharacteristicOperationV1>,
        error: AgentSourceIsaCharacteristicErrorCodeV1,
        terminal: bool,
    },
}

impl AgentSourceIsaCharacteristicResponseV1 {
    fn error(
        request_id: Option<u64>,
        response_revision: u64,
        operation: Option<AgentSourceIsaCharacteristicOperationV1>,
        error: AgentSourceIsaCharacteristicErrorCodeV1,
        terminal: bool,
    ) -> Self {
        Self::Error {
            schema: AGENT_SOURCE_ISA_CHARACTERISTIC_RESPONSE_SCHEMA_V1.to_owned(),
            request_id,
            response_revision,
            operation,
            error,
            terminal,
        }
    }

    pub const fn is_terminal(&self) -> bool {
        matches!(self, Self::Error { terminal: true, .. })
    }

    fn context(
        &self,
    ) -> (
        Option<u64>,
        u64,
        Option<AgentSourceIsaCharacteristicOperationV1>,
    ) {
        match self {
            Self::Ok {
                request_id,
                response_revision,
                operation,
                ..
            } => (Some(*request_id), *response_revision, Some(*operation)),
            Self::Error {
                request_id,
                response_revision,
                operation,
                ..
            } => (*request_id, *response_revision, *operation),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AgentSourceIsaCharacteristicDecodeErrorV1 {
    LineTooLarge,
    InvalidCanonicalJsonl,
}

impl fmt::Display for AgentSourceIsaCharacteristicDecodeErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::LineTooLarge => "source/ISA characteristic agent line exceeds its byte bound",
            Self::InvalidCanonicalJsonl => {
                "source/ISA characteristic agent line is not canonical JSONL"
            }
        })
    }
}

impl Error for AgentSourceIsaCharacteristicDecodeErrorV1 {}

pub struct AgentSourceIsaCharacteristicServiceV1 {
    collection: SourceIsaCharacteristicCollectionV1,
    request_ids: Vec<u64>,
    request_count: u32,
    response_revision: u64,
    terminal: bool,
}

impl AgentSourceIsaCharacteristicServiceV1 {
    pub fn new(
        collection: SourceIsaCharacteristicCollectionV1,
    ) -> Result<Self, AgentSourceIsaCharacteristicErrorCodeV1> {
        collection
            .canonical_byte_len()
            .map_err(map_collection_error)?;
        let mut request_ids = Vec::new();
        let request_capacity = usize::try_from(MAX_AGENT_SOURCE_ISA_CHARACTERISTIC_REQUESTS_V1)
            .map_err(|_| AgentSourceIsaCharacteristicErrorCodeV1::AllocationFailure)?;
        request_ids
            .try_reserve_exact(request_capacity)
            .map_err(|_| AgentSourceIsaCharacteristicErrorCodeV1::AllocationFailure)?;
        Ok(Self {
            collection,
            request_ids,
            request_count: 0,
            response_revision: 0,
            terminal: false,
        })
    }

    pub const fn collection(&self) -> &SourceIsaCharacteristicCollectionV1 {
        &self.collection
    }

    pub fn handle_line(&mut self, line: &[u8]) -> AgentSourceIsaCharacteristicResponseV1 {
        let Some(revision) = self.response_revision.checked_add(1) else {
            self.terminal = true;
            return AgentSourceIsaCharacteristicResponseV1::error(
                None,
                self.response_revision,
                None,
                AgentSourceIsaCharacteristicErrorCodeV1::RequestBudgetExhausted,
                true,
            );
        };
        self.response_revision = revision;
        if self.terminal || self.request_count >= MAX_AGENT_SOURCE_ISA_CHARACTERISTIC_REQUESTS_V1 {
            self.terminal = true;
            return AgentSourceIsaCharacteristicResponseV1::error(
                None,
                self.response_revision,
                None,
                AgentSourceIsaCharacteristicErrorCodeV1::RequestBudgetExhausted,
                true,
            );
        }
        if line.len() > MAX_AGENT_SOURCE_ISA_CHARACTERISTIC_REQUEST_BYTES_V1 {
            self.terminal = true;
            return AgentSourceIsaCharacteristicResponseV1::error(
                None,
                self.response_revision,
                None,
                AgentSourceIsaCharacteristicErrorCodeV1::RequestTooLarge,
                true,
            );
        }
        self.request_count += 1;
        let request = match decode_agent_source_isa_characteristic_request_line_v1(line) {
            Ok(request) => request,
            Err(error) => {
                let (request_id, operation) = request_identity_hint(line);
                if request_id.is_some_and(|value| self.request_ids.contains(&value)) {
                    return AgentSourceIsaCharacteristicResponseV1::error(
                        request_id,
                        self.response_revision,
                        operation,
                        AgentSourceIsaCharacteristicErrorCodeV1::DuplicateRequestId,
                        false,
                    );
                }
                if let Some(value) = request_id {
                    self.request_ids.push(value);
                }
                return AgentSourceIsaCharacteristicResponseV1::error(
                    request_id,
                    self.response_revision,
                    operation,
                    match error {
                        AgentSourceIsaCharacteristicDecodeErrorV1::LineTooLarge => {
                            AgentSourceIsaCharacteristicErrorCodeV1::RequestTooLarge
                        }
                        AgentSourceIsaCharacteristicDecodeErrorV1::InvalidCanonicalJsonl => {
                            AgentSourceIsaCharacteristicErrorCodeV1::InvalidRequest
                        }
                    },
                    false,
                );
            }
        };
        let (request_id, operation) = request.identity();
        if request_id == 0 {
            return AgentSourceIsaCharacteristicResponseV1::error(
                None,
                self.response_revision,
                Some(operation),
                AgentSourceIsaCharacteristicErrorCodeV1::InvalidRequestId,
                false,
            );
        }
        if self.request_ids.contains(&request_id) {
            return AgentSourceIsaCharacteristicResponseV1::error(
                Some(request_id),
                self.response_revision,
                Some(operation),
                AgentSourceIsaCharacteristicErrorCodeV1::DuplicateRequestId,
                false,
            );
        }
        self.request_ids.push(request_id);
        if request.schema() != AGENT_SOURCE_ISA_CHARACTERISTIC_REQUEST_SCHEMA_V1 {
            return AgentSourceIsaCharacteristicResponseV1::error(
                Some(request_id),
                self.response_revision,
                Some(operation),
                AgentSourceIsaCharacteristicErrorCodeV1::SchemaMismatch,
                false,
            );
        }
        match execute_request(&self.collection, request) {
            Ok(result) => AgentSourceIsaCharacteristicResponseV1::Ok {
                schema: AGENT_SOURCE_ISA_CHARACTERISTIC_RESPONSE_SCHEMA_V1.to_owned(),
                request_id,
                response_revision: self.response_revision,
                operation,
                result,
            },
            Err(error) => AgentSourceIsaCharacteristicResponseV1::error(
                Some(request_id),
                self.response_revision,
                Some(operation),
                error,
                false,
            ),
        }
    }
}

fn execute_request(
    collection: &SourceIsaCharacteristicCollectionV1,
    request: AgentSourceIsaCharacteristicRequestV1,
) -> Result<AgentSourceIsaCharacteristicResultV1, AgentSourceIsaCharacteristicErrorCodeV1> {
    let authority = AgentSourceIsaCharacteristicAuthorityV1::for_collection(collection);
    match request {
        AgentSourceIsaCharacteristicRequestV1::DiscoverCapabilities { .. } => {
            Ok(AgentSourceIsaCharacteristicResultV1::Capabilities {
                authority,
                limits: limits(),
                capabilities: capabilities(),
                collection: project_collection(collection)?,
            })
        }
        AgentSourceIsaCharacteristicRequestV1::QueryTargets {
            collection_identity,
            query,
            cursor,
            limit,
            ..
        } => {
            require_collection_identity(collection, &collection_identity)?;
            if limit == 0 || limit > MAX_AGENT_SOURCE_ISA_CHARACTERISTIC_PAGE_ITEMS_V1 {
                return Err(AgentSourceIsaCharacteristicErrorCodeV1::InvalidPageLimit);
            }
            let core_query = decode_target_query(&query)?;
            let core_cursor = cursor.as_deref().map(decode_cursor).transpose()?;
            let core_page = collection
                .query_targets_page(&core_query, core_cursor.as_ref(), limit)
                .map_err(map_query_error)?;
            let mut targets = Vec::new();
            targets
                .try_reserve_exact(core_page.targets().len())
                .map_err(|_| AgentSourceIsaCharacteristicErrorCodeV1::AllocationFailure)?;
            for target in core_page.targets() {
                targets.push(AgentSourceIsaCharacteristicTargetV1 {
                    occurrence_identity: hex_bytes(target.identity())?,
                    characteristic_identity: hex_bytes(target.characteristic_identity())?,
                    category: project_category(target.category()),
                    kind: project_kind(target.kind()),
                    target_kir: project_kir(target.target_kir()),
                    correlation_count: target.correlation_count(),
                });
            }
            Ok(AgentSourceIsaCharacteristicResultV1::TargetPage {
                authority,
                collection: project_collection(collection)?,
                page: AgentSourceIsaCharacteristicTargetPageV1 {
                    query,
                    query_identity: hex_bytes(core_page.query_identity())?,
                    cursor,
                    next_cursor: core_page.next_cursor().map(encode_cursor).transpose()?,
                    total_matches: core_page.total_matches(),
                    page_exhausted: core_page.page_exhausted(),
                    targets,
                },
            })
        }
        AgentSourceIsaCharacteristicRequestV1::QueryFacts {
            collection_identity,
            query,
            cursor,
            limit,
            ..
        } => {
            require_collection_identity(collection, &collection_identity)?;
            if limit == 0 || limit > MAX_AGENT_SOURCE_ISA_CHARACTERISTIC_PAGE_ITEMS_V1 {
                return Err(AgentSourceIsaCharacteristicErrorCodeV1::InvalidPageLimit);
            }
            let core_query = decode_query(&query)?;
            let core_cursor = cursor.as_deref().map(decode_cursor).transpose()?;
            let core_page = collection
                .query_page(&core_query, core_cursor.as_ref(), limit)
                .map_err(map_query_error)?;
            let mut facts = Vec::new();
            facts
                .try_reserve_exact(core_page.matches().len())
                .map_err(|_| AgentSourceIsaCharacteristicErrorCodeV1::AllocationFailure)?;
            for item in core_page.matches() {
                facts.push(project_fact(*item)?);
            }
            Ok(AgentSourceIsaCharacteristicResultV1::FactPage {
                authority,
                collection: project_collection(collection)?,
                page: AgentSourceIsaCharacteristicFactPageV1 {
                    query,
                    query_identity: hex_bytes(core_page.query_identity())?,
                    cursor,
                    next_cursor: core_page.next_cursor().map(encode_cursor).transpose()?,
                    total_matches: core_page.total_matches(),
                    page_exhausted: core_page.page_exhausted(),
                    facts,
                },
            })
        }
        AgentSourceIsaCharacteristicRequestV1::QueryIntervals {
            collection_identity,
            occurrence_identity,
            cursor,
            limit,
            ..
        } => {
            require_collection_identity(collection, &collection_identity)?;
            if limit == 0 || limit > MAX_AGENT_SOURCE_ISA_CHARACTERISTIC_PAGE_ITEMS_V1 {
                return Err(AgentSourceIsaCharacteristicErrorCodeV1::InvalidPageLimit);
            }
            let occurrence = decode_identity(&occurrence_identity)?;
            let core_cursor = cursor.as_deref().map(decode_cursor).transpose()?;
            let core_page = collection
                .interval_page(occurrence, core_cursor.as_ref(), limit)
                .map_err(map_interval_error)?;
            let mut intervals = Vec::new();
            intervals
                .try_reserve_exact(core_page.intervals().len())
                .map_err(|_| AgentSourceIsaCharacteristicErrorCodeV1::AllocationFailure)?;
            for item in core_page.intervals() {
                let interval = item.interval();
                intervals.push(AgentSourceIsaIntervalItemV1 {
                    identity: hex_bytes(item.identity())?,
                    ordinal: item.ordinal(),
                    interval: AgentSourceIsaIsaIntervalV1 {
                        kernel_ordinal: interval.kernel_ordinal(),
                        symbol_relative_start: interval.symbol_relative_start(),
                        symbol_relative_end: interval.symbol_relative_end(),
                    },
                });
            }
            Ok(AgentSourceIsaCharacteristicResultV1::IntervalPage {
                authority,
                collection: project_collection(collection)?,
                page: AgentSourceIsaCharacteristicIntervalPageV1 {
                    occurrence_identity,
                    cursor,
                    next_cursor: core_page.next_cursor().map(encode_cursor).transpose()?,
                    total_intervals: core_page.total_intervals(),
                    page_exhausted: core_page.page_exhausted(),
                    intervals,
                },
            })
        }
    }
}

fn decode_target_query(
    query: &AgentSourceIsaCharacteristicTargetQueryV1,
) -> Result<SourceIsaCharacteristicTargetQueryV1, AgentSourceIsaCharacteristicErrorCodeV1> {
    Ok(match query {
        AgentSourceIsaCharacteristicTargetQueryV1::All => SourceIsaCharacteristicTargetQueryV1::All,
        AgentSourceIsaCharacteristicTargetQueryV1::CharacteristicIdentity { identity } => {
            SourceIsaCharacteristicTargetQueryV1::CharacteristicIdentity(decode_identity(identity)?)
        }
        AgentSourceIsaCharacteristicTargetQueryV1::Category { code } => {
            SourceIsaCharacteristicTargetQueryV1::Category(decode_category(*code)?)
        }
        AgentSourceIsaCharacteristicTargetQueryV1::Kind { code } => {
            SourceIsaCharacteristicTargetQueryV1::Kind(decode_kind(*code)?)
        }
        AgentSourceIsaCharacteristicTargetQueryV1::TargetKir { coordinate } => {
            SourceIsaCharacteristicTargetQueryV1::TargetKir(decode_kir(*coordinate)?)
        }
    })
}

fn require_collection_identity(
    collection: &SourceIsaCharacteristicCollectionV1,
    identity: &str,
) -> Result<(), AgentSourceIsaCharacteristicErrorCodeV1> {
    if decode_identity(identity)? != collection.identity() {
        Err(AgentSourceIsaCharacteristicErrorCodeV1::CollectionIdentityMismatch)
    } else {
        Ok(())
    }
}

fn decode_query(
    query: &AgentSourceIsaCharacteristicQueryV1,
) -> Result<SourceIsaCharacteristicQueryV1, AgentSourceIsaCharacteristicErrorCodeV1> {
    Ok(match query {
        AgentSourceIsaCharacteristicQueryV1::All => SourceIsaCharacteristicQueryV1::All,
        AgentSourceIsaCharacteristicQueryV1::CharacteristicIdentity { identity } => {
            SourceIsaCharacteristicQueryV1::CharacteristicIdentity(decode_identity(identity)?)
        }
        AgentSourceIsaCharacteristicQueryV1::Category { code } => {
            SourceIsaCharacteristicQueryV1::Category(decode_category(*code)?)
        }
        AgentSourceIsaCharacteristicQueryV1::Kind { code } => {
            SourceIsaCharacteristicQueryV1::Kind(decode_kind(*code)?)
        }
        AgentSourceIsaCharacteristicQueryV1::SourceNode { identity } => {
            SourceIsaCharacteristicQueryV1::SourceNode(decode_identity(identity)?)
        }
        AgentSourceIsaCharacteristicQueryV1::SourceSpan { span } => {
            SourceIsaCharacteristicQueryV1::SourceSpan(
                SourceIsaCharacteristicSourceSpanV1::new(
                    decode_identity(&span.file_identity)?,
                    span.byte_start,
                    span.byte_end,
                    span.line,
                    span.column,
                )
                .map_err(map_query_error)?,
            )
        }
        AgentSourceIsaCharacteristicQueryV1::RecordKind { code } => {
            SourceIsaCharacteristicQueryV1::RecordKind(decode_record_kind(*code)?)
        }
        AgentSourceIsaCharacteristicQueryV1::MirNode { identity } => {
            SourceIsaCharacteristicQueryV1::MirNode(decode_identity(identity)?)
        }
        AgentSourceIsaCharacteristicQueryV1::Mir { coordinate } => {
            SourceIsaCharacteristicQueryV1::Mir(decode_mir(*coordinate)?)
        }
        AgentSourceIsaCharacteristicQueryV1::NeutralKirNode { identity } => {
            SourceIsaCharacteristicQueryV1::NeutralKirNode(decode_identity(identity)?)
        }
        AgentSourceIsaCharacteristicQueryV1::NeutralKir { coordinate } => {
            SourceIsaCharacteristicQueryV1::NeutralKir(decode_kir(*coordinate)?)
        }
        AgentSourceIsaCharacteristicQueryV1::TargetKir { coordinate } => {
            SourceIsaCharacteristicQueryV1::TargetKir(decode_kir(*coordinate)?)
        }
        AgentSourceIsaCharacteristicQueryV1::SemanticOperation { identity } => {
            SourceIsaCharacteristicQueryV1::SemanticOperation(decode_identity(identity)?)
        }
        AgentSourceIsaCharacteristicQueryV1::CompilerHandoffLlvm { coordinate } => {
            SourceIsaCharacteristicQueryV1::CompilerHandoffLlvm(decode_llvm(*coordinate)?)
        }
        AgentSourceIsaCharacteristicQueryV1::Transformation { code } => {
            SourceIsaCharacteristicQueryV1::Transformation(decode_transformation(*code)?)
        }
        AgentSourceIsaCharacteristicQueryV1::ExactPc {
            kernel_ordinal,
            symbol_relative_pc,
        } => SourceIsaCharacteristicQueryV1::ExactPc {
            kernel_ordinal: *kernel_ordinal,
            symbol_relative_pc: *symbol_relative_pc,
        },
        AgentSourceIsaCharacteristicQueryV1::PreKirOnly => {
            SourceIsaCharacteristicQueryV1::PreKirOnly
        }
    })
}

fn decode_category(
    code: u16,
) -> Result<SourceIsaCharacteristicCategoryV1, AgentSourceIsaCharacteristicErrorCodeV1> {
    match code {
        1 => Ok(SourceIsaCharacteristicCategoryV1::TargetKirGlobalStore),
        2 => Ok(SourceIsaCharacteristicCategoryV1::TargetKirWorkgroupLdsRead),
        3 => Ok(SourceIsaCharacteristicCategoryV1::TargetKirWorkgroupLdsWrite),
        4 => Ok(SourceIsaCharacteristicCategoryV1::TargetKirWorkgroupBarrier),
        5 => Ok(SourceIsaCharacteristicCategoryV1::TargetKirBf16MfmaExact),
        _ => Err(AgentSourceIsaCharacteristicErrorCodeV1::InvalidQuery),
    }
}

fn decode_kind(
    code: u16,
) -> Result<SourceIsaCharacteristicKindV1, AgentSourceIsaCharacteristicErrorCodeV1> {
    let memory = |code| match code {
        1 => Ok(SourceIsaCharacteristicMemoryFormV1::Plain),
        2 => Ok(SourceIsaCharacteristicMemoryFormV1::Guarded),
        3 => Ok(SourceIsaCharacteristicMemoryFormV1::MatrixTile),
        _ => Err(AgentSourceIsaCharacteristicErrorCodeV1::InvalidQuery),
    };
    match code {
        1..=3 => Ok(SourceIsaCharacteristicKindV1::GlobalStore {
            form: memory(code)?,
        }),
        4..=6 => Ok(SourceIsaCharacteristicKindV1::WorkgroupLoad {
            form: memory(code - 3)?,
        }),
        7..=9 => Ok(SourceIsaCharacteristicKindV1::WorkgroupStore {
            form: memory(code - 6)?,
        }),
        10 => Ok(SourceIsaCharacteristicKindV1::WorkgroupBarrier),
        11 => Ok(SourceIsaCharacteristicKindV1::Bf16F32M16N16K16Wave64MatrixMultiplyAccumulate),
        _ => Err(AgentSourceIsaCharacteristicErrorCodeV1::InvalidQuery),
    }
}

fn decode_record_kind(
    code: u8,
) -> Result<SourceIsaCharacteristicRecordKindV1, AgentSourceIsaCharacteristicErrorCodeV1> {
    match code {
        1 => Ok(SourceIsaCharacteristicRecordKindV1::EliminatedBeforeKir),
        2 => Ok(SourceIsaCharacteristicRecordKindV1::SourceAnchored),
        3 => Ok(SourceIsaCharacteristicRecordKindV1::NoSourceProvenance),
        _ => Err(AgentSourceIsaCharacteristicErrorCodeV1::InvalidQuery),
    }
}

fn decode_transformation(
    code: u8,
) -> Result<SourceIsaCharacteristicTransformationV1, AgentSourceIsaCharacteristicErrorCodeV1> {
    match code {
        1 => Ok(SourceIsaCharacteristicTransformationV1::Preserved),
        2 => Ok(SourceIsaCharacteristicTransformationV1::Duplicated),
        3 => Ok(SourceIsaCharacteristicTransformationV1::Coalesced),
        4 => Ok(SourceIsaCharacteristicTransformationV1::DuplicatedAndCoalesced),
        5 => Ok(SourceIsaCharacteristicTransformationV1::Eliminated),
        _ => Err(AgentSourceIsaCharacteristicErrorCodeV1::InvalidQuery),
    }
}

fn decode_mir(
    value: AgentSourceIsaMirCoordinateV1,
) -> Result<SourceIsaCharacteristicMirCoordinateV1, AgentSourceIsaCharacteristicErrorCodeV1> {
    SourceIsaCharacteristicMirCoordinateV1::new(
        value.body_ordinal,
        value.block_ordinal,
        value.statement_ordinal,
    )
    .map_err(map_query_error)
}

fn decode_kir(
    value: AgentSourceIsaKirCoordinateV1,
) -> Result<SourceIsaCharacteristicKirCoordinateV1, AgentSourceIsaCharacteristicErrorCodeV1> {
    SourceIsaCharacteristicKirCoordinateV1::new(
        value.function_ordinal,
        value.block_ordinal,
        value.operation_ordinal,
    )
    .map_err(map_query_error)
}

fn decode_llvm(
    value: AgentSourceIsaLlvmCoordinateV1,
) -> Result<
    SourceIsaCharacteristicCompilerHandoffLlvmCoordinateV1,
    AgentSourceIsaCharacteristicErrorCodeV1,
> {
    SourceIsaCharacteristicCompilerHandoffLlvmCoordinateV1::new(
        value.function_ordinal,
        value.block_ordinal,
        value.instruction_ordinal,
    )
    .map_err(map_query_error)
}

fn project_collection(
    collection: &SourceIsaCharacteristicCollectionV1,
) -> Result<AgentSourceIsaCharacteristicCollectionSummaryV1, AgentSourceIsaCharacteristicErrorCodeV1>
{
    let binding = collection.binding();
    let counts = binding.structural_counts();
    let scan = collection.scan();
    Ok(AgentSourceIsaCharacteristicCollectionSummaryV1 {
        format: "fe2o3-source-isa-characteristic-collection-v1".to_owned(),
        identity: hex_bytes(collection.identity())?,
        canonical_byte_len: collection
            .canonical_byte_len()
            .map_err(map_collection_error)?,
        binding: AgentSourceIsaCharacteristicBindingV1 {
            target_profile: project_target_profile(binding.target_profile()),
            kir_version: binding.kir_version().code(),
            structural_identity: hex_bytes(binding.structural_identity())?,
            structural_counts: AgentSourceIsaStructuralCountsV1 {
                functions: counts.functions,
                defined_bodies: counts.defined_bodies,
                blocks: counts.blocks,
                operations: counts.operations,
            },
            source_map_v2: project_content(binding.source_map_v2())?,
            neutral_kir: project_content(binding.neutral_kir())?,
            target_kir: project_content(binding.target_kir())?,
            artifact: project_content(binding.artifact())?,
            catalog: project_content(binding.catalog())?,
            structural_bridge: project_content(binding.structural_bridge())?,
            correlation_identity: hex_bytes(binding.correlation_identity())?,
            semantic_map_identity: hex_bytes(binding.semantic_map_identity())?,
        },
        scan: AgentSourceIsaCharacteristicScanSummaryV1 {
            catalog_record_count: scan.catalog_record_count(),
            catalog_records_scanned: scan.catalog_records_scanned(),
            target_operation_count: scan.target_operation_count(),
            target_operations_scanned: scan.target_operations_scanned(),
            classified_target_count: scan.classified_target_count(),
            retained_target_correlation_count: scan.retained_target_correlation_count(),
            pre_kir_elimination_count: scan.pre_kir_elimination_count(),
            correlation_count: scan.correlation_count(),
            state: project_scan_state(scan.state()),
        },
        target_count: u64::try_from(collection.targets().len())
            .map_err(|_| AgentSourceIsaCharacteristicErrorCodeV1::InvalidCollection)?,
        pre_kir_elimination_count: u64::try_from(collection.pre_kir_eliminations().len())
            .map_err(|_| AgentSourceIsaCharacteristicErrorCodeV1::InvalidCollection)?,
        sparse_final_hsaco_anchors_present: collection.has_sparse_final_hsaco_anchors(),
        decoded_isa: false,
    })
}

fn project_target_profile(
    value: SourceIsaCharacteristicTargetProfileV1,
) -> AgentSourceIsaTargetProfileV1 {
    AgentSourceIsaTargetProfileV1 {
        code: value.code(),
        label: match value {
            SourceIsaCharacteristicTargetProfileV1::Gfx942 => {
                AgentSourceIsaTargetProfileLabelV1::Gfx942
            }
            SourceIsaCharacteristicTargetProfileV1::Gfx950 => {
                AgentSourceIsaTargetProfileLabelV1::Gfx950
            }
        },
    }
}

fn project_content(
    value: SourceIsaCharacteristicContentIdentityV1,
) -> Result<AgentSourceIsaContentIdentityV1, AgentSourceIsaCharacteristicErrorCodeV1> {
    Ok(AgentSourceIsaContentIdentityV1 {
        sha256: hex_bytes(value.sha256())?,
        byte_len: value.byte_len(),
    })
}

fn project_scan_state(
    value: SourceIsaCharacteristicScanStateV1,
) -> AgentSourceIsaCharacteristicScanStateV1 {
    match value {
        SourceIsaCharacteristicScanStateV1::Complete => {
            AgentSourceIsaCharacteristicScanStateV1::Complete
        }
        SourceIsaCharacteristicScanStateV1::Missing(reason) => {
            AgentSourceIsaCharacteristicScanStateV1::Missing {
                code: reason.code(),
                reason: match reason {
                    SourceIsaCharacteristicMissingReasonV1::NoQualifyingTargetOperation => {
                        AgentSourceIsaMissingReasonV1::NoQualifyingTargetOperation
                    }
                    SourceIsaCharacteristicMissingReasonV1::NoAdmittedCorrelation => {
                        AgentSourceIsaMissingReasonV1::NoAdmittedCorrelation
                    }
                },
            }
        }
        SourceIsaCharacteristicScanStateV1::Unavailable(reason) => {
            AgentSourceIsaCharacteristicScanStateV1::Unavailable {
                code: reason.code(),
                reason: match reason {
                    SourceIsaCharacteristicUnavailableReasonV1::CatalogUnavailable => {
                        AgentSourceIsaUnavailableReasonV1::CatalogUnavailable
                    }
                    SourceIsaCharacteristicUnavailableReasonV1::StructuralBridgeUnavailable => {
                        AgentSourceIsaUnavailableReasonV1::StructuralBridgeUnavailable
                    }
                    SourceIsaCharacteristicUnavailableReasonV1::ClassifierUnavailable => {
                        AgentSourceIsaUnavailableReasonV1::ClassifierUnavailable
                    }
                    SourceIsaCharacteristicUnavailableReasonV1::SourceProjectionUnavailable => {
                        AgentSourceIsaUnavailableReasonV1::SourceProjectionUnavailable
                    }
                },
            }
        }
        SourceIsaCharacteristicScanStateV1::Error(error) => {
            AgentSourceIsaCharacteristicScanStateV1::Error {
                code: error.code(),
                error: match error {
                    SourceIsaCharacteristicObservationErrorV1::InvalidCatalog => {
                        AgentSourceIsaObservationErrorV1::InvalidCatalog
                    }
                    SourceIsaCharacteristicObservationErrorV1::InvalidStructuralBridge => {
                        AgentSourceIsaObservationErrorV1::InvalidStructuralBridge
                    }
                    SourceIsaCharacteristicObservationErrorV1::InvalidClassification => {
                        AgentSourceIsaObservationErrorV1::InvalidClassification
                    }
                    SourceIsaCharacteristicObservationErrorV1::ConflictingEvidence => {
                        AgentSourceIsaObservationErrorV1::ConflictingEvidence
                    }
                    SourceIsaCharacteristicObservationErrorV1::ResourceLimit => {
                        AgentSourceIsaObservationErrorV1::ResourceLimit
                    }
                    SourceIsaCharacteristicObservationErrorV1::AllocationFailure => {
                        AgentSourceIsaObservationErrorV1::AllocationFailure
                    }
                },
            }
        }
    }
}

fn project_fact(
    value: SourceIsaCharacteristicMatchV1,
) -> Result<AgentSourceIsaCharacteristicFactV1, AgentSourceIsaCharacteristicErrorCodeV1> {
    Ok(AgentSourceIsaCharacteristicFactV1 {
        occurrence_identity: hex_bytes(value.identity())?,
        characteristic_identity: value.characteristic_identity().map(hex_bytes).transpose()?,
        category: value.category().map(project_category),
        kind: value.kind().map(project_kind),
        target_kir: value.target_kir().map(project_kir),
        outcome: match value.outcome() {
            SourceIsaCharacteristicMatchOutcomeV1::TargetCorrelation(summary) => {
                AgentSourceIsaCharacteristicFactOutcomeV1::TargetCorrelation {
                    correlation: AgentSourceIsaTargetCorrelationV1 {
                        catalog_record_ordinal: summary.catalog_record_ordinal,
                        record_kind: project_record_kind(summary.kind),
                        source: summary.source.map(project_source).transpose()?,
                        mir_node_identity: summary.mir_node_identity.map(hex_bytes).transpose()?,
                        mir: summary.mir.map(project_mir),
                        neutral_kir_node_identity: summary
                            .neutral_kir_node_identity
                            .map(hex_bytes)
                            .transpose()?,
                        neutral_kir: summary.neutral_kir.map(project_kir),
                        target_kir: project_kir(summary.target_kir),
                        semantic_operation_identity: hex_bytes(
                            summary.semantic_operation_identity,
                        )?,
                        compiler_handoff_llvm: project_llvm(summary.compiler_handoff_llvm),
                        interval_count: summary.interval_count,
                        transformation: project_transformation(summary.transformation),
                    },
                }
            }
            SourceIsaCharacteristicMatchOutcomeV1::PreKirElimination(fact) => {
                AgentSourceIsaCharacteristicFactOutcomeV1::PreKirElimination {
                    elimination: AgentSourceIsaPreKirEliminationV1 {
                        catalog_record_ordinal: fact.catalog_record_ordinal(),
                        record_kind: project_record_kind(
                            SourceIsaCharacteristicRecordKindV1::EliminatedBeforeKir,
                        ),
                        source: project_source(fact.source())?,
                        mir_node_identity: hex_bytes(fact.mir_node_identity())?,
                        mir: project_mir(fact.mir()),
                    },
                }
            }
        },
    })
}

fn project_source(
    value: SourceIsaCharacteristicSourceCoordinateV1,
) -> Result<AgentSourceIsaSourceCoordinateV1, AgentSourceIsaCharacteristicErrorCodeV1> {
    Ok(AgentSourceIsaSourceCoordinateV1 {
        node_identity: hex_bytes(value.node_identity())?,
        span: project_span(value.span())?,
    })
}

fn project_span(
    value: SourceIsaCharacteristicSourceSpanV1,
) -> Result<AgentSourceIsaSourceSpanV1, AgentSourceIsaCharacteristicErrorCodeV1> {
    Ok(AgentSourceIsaSourceSpanV1 {
        file_identity: hex_bytes(value.file_identity())?,
        byte_start: value.byte_start(),
        byte_end: value.byte_end(),
        line: value.line(),
        column: value.column(),
    })
}

fn project_mir(value: SourceIsaCharacteristicMirCoordinateV1) -> AgentSourceIsaMirCoordinateV1 {
    AgentSourceIsaMirCoordinateV1 {
        body_ordinal: value.body_ordinal(),
        block_ordinal: value.block_ordinal(),
        statement_ordinal: value.statement_ordinal(),
    }
}

fn project_kir(value: SourceIsaCharacteristicKirCoordinateV1) -> AgentSourceIsaKirCoordinateV1 {
    AgentSourceIsaKirCoordinateV1 {
        function_ordinal: value.function_ordinal(),
        block_ordinal: value.block_ordinal(),
        operation_ordinal: value.operation_ordinal(),
    }
}

fn project_llvm(
    value: SourceIsaCharacteristicCompilerHandoffLlvmCoordinateV1,
) -> AgentSourceIsaLlvmCoordinateV1 {
    AgentSourceIsaLlvmCoordinateV1 {
        function_ordinal: value.function_ordinal(),
        block_ordinal: value.block_ordinal(),
        instruction_ordinal: value.instruction_ordinal(),
    }
}

fn project_category(
    value: SourceIsaCharacteristicCategoryV1,
) -> AgentSourceIsaCharacteristicCategoryV1 {
    AgentSourceIsaCharacteristicCategoryV1 {
        code: value.code(),
        label: match value {
            SourceIsaCharacteristicCategoryV1::TargetKirGlobalStore => {
                AgentSourceIsaCharacteristicCategoryLabelV1::TargetKirGlobalStore
            }
            SourceIsaCharacteristicCategoryV1::TargetKirWorkgroupLdsRead => {
                AgentSourceIsaCharacteristicCategoryLabelV1::TargetKirWorkgroupLdsRead
            }
            SourceIsaCharacteristicCategoryV1::TargetKirWorkgroupLdsWrite => {
                AgentSourceIsaCharacteristicCategoryLabelV1::TargetKirWorkgroupLdsWrite
            }
            SourceIsaCharacteristicCategoryV1::TargetKirWorkgroupBarrier => {
                AgentSourceIsaCharacteristicCategoryLabelV1::TargetKirWorkgroupBarrier
            }
            SourceIsaCharacteristicCategoryV1::TargetKirBf16MfmaExact => {
                AgentSourceIsaCharacteristicCategoryLabelV1::TargetKirBf16MfmaExact
            }
        },
    }
}

fn project_memory_form(value: SourceIsaCharacteristicMemoryFormV1) -> AgentSourceIsaMemoryFormV1 {
    AgentSourceIsaMemoryFormV1 {
        code: value.code(),
        label: match value {
            SourceIsaCharacteristicMemoryFormV1::Plain => AgentSourceIsaMemoryFormLabelV1::Plain,
            SourceIsaCharacteristicMemoryFormV1::Guarded => {
                AgentSourceIsaMemoryFormLabelV1::Guarded
            }
            SourceIsaCharacteristicMemoryFormV1::MatrixTile => {
                AgentSourceIsaMemoryFormLabelV1::MatrixTile
            }
        },
    }
}

fn project_kind(value: SourceIsaCharacteristicKindV1) -> AgentSourceIsaCharacteristicKindV1 {
    let (label, memory_form) = match value {
        SourceIsaCharacteristicKindV1::GlobalStore { form } => (
            AgentSourceIsaCharacteristicKindLabelV1::GlobalStore,
            Some(project_memory_form(form)),
        ),
        SourceIsaCharacteristicKindV1::WorkgroupLoad { form } => (
            AgentSourceIsaCharacteristicKindLabelV1::WorkgroupLoad,
            Some(project_memory_form(form)),
        ),
        SourceIsaCharacteristicKindV1::WorkgroupStore { form } => (
            AgentSourceIsaCharacteristicKindLabelV1::WorkgroupStore,
            Some(project_memory_form(form)),
        ),
        SourceIsaCharacteristicKindV1::WorkgroupBarrier => (
            AgentSourceIsaCharacteristicKindLabelV1::WorkgroupBarrier,
            None,
        ),
        SourceIsaCharacteristicKindV1::Bf16F32M16N16K16Wave64MatrixMultiplyAccumulate => (
            AgentSourceIsaCharacteristicKindLabelV1::Bf16F32M16N16K16Wave64MatrixMultiplyAccumulate,
            None,
        ),
    };
    AgentSourceIsaCharacteristicKindV1 {
        code: value.code(),
        label,
        memory_form,
    }
}

fn project_record_kind(value: SourceIsaCharacteristicRecordKindV1) -> AgentSourceIsaRecordKindV1 {
    AgentSourceIsaRecordKindV1 {
        code: value.code(),
        label: match value {
            SourceIsaCharacteristicRecordKindV1::EliminatedBeforeKir => {
                AgentSourceIsaRecordKindLabelV1::EliminatedBeforeKir
            }
            SourceIsaCharacteristicRecordKindV1::SourceAnchored => {
                AgentSourceIsaRecordKindLabelV1::SourceAnchored
            }
            SourceIsaCharacteristicRecordKindV1::NoSourceProvenance => {
                AgentSourceIsaRecordKindLabelV1::NoSourceProvenance
            }
        },
    }
}

fn project_transformation(
    value: SourceIsaCharacteristicTransformationV1,
) -> AgentSourceIsaTransformationV1 {
    AgentSourceIsaTransformationV1 {
        code: value.code(),
        label: match value {
            SourceIsaCharacteristicTransformationV1::Preserved => {
                AgentSourceIsaTransformationLabelV1::Preserved
            }
            SourceIsaCharacteristicTransformationV1::Duplicated => {
                AgentSourceIsaTransformationLabelV1::Duplicated
            }
            SourceIsaCharacteristicTransformationV1::Coalesced => {
                AgentSourceIsaTransformationLabelV1::Coalesced
            }
            SourceIsaCharacteristicTransformationV1::DuplicatedAndCoalesced => {
                AgentSourceIsaTransformationLabelV1::DuplicatedAndCoalesced
            }
            SourceIsaCharacteristicTransformationV1::Eliminated => {
                AgentSourceIsaTransformationLabelV1::Eliminated
            }
        },
    }
}

fn map_collection_error(
    value: SourceIsaCharacteristicErrorV1,
) -> AgentSourceIsaCharacteristicErrorCodeV1 {
    match value {
        SourceIsaCharacteristicErrorV1::AllocationFailure => {
            AgentSourceIsaCharacteristicErrorCodeV1::AllocationFailure
        }
        _ => AgentSourceIsaCharacteristicErrorCodeV1::InvalidCollection,
    }
}

fn map_query_error(
    value: SourceIsaCharacteristicErrorV1,
) -> AgentSourceIsaCharacteristicErrorCodeV1 {
    match value {
        SourceIsaCharacteristicErrorV1::InvalidCursor => {
            AgentSourceIsaCharacteristicErrorCodeV1::InvalidCursor
        }
        SourceIsaCharacteristicErrorV1::AllocationFailure => {
            AgentSourceIsaCharacteristicErrorCodeV1::AllocationFailure
        }
        _ => AgentSourceIsaCharacteristicErrorCodeV1::InvalidQuery,
    }
}

fn map_interval_error(
    value: SourceIsaCharacteristicErrorV1,
) -> AgentSourceIsaCharacteristicErrorCodeV1 {
    match value {
        SourceIsaCharacteristicErrorV1::InvalidCursor => {
            AgentSourceIsaCharacteristicErrorCodeV1::InvalidCursor
        }
        SourceIsaCharacteristicErrorV1::InvalidClaim => {
            AgentSourceIsaCharacteristicErrorCodeV1::UnknownOccurrence
        }
        SourceIsaCharacteristicErrorV1::AllocationFailure => {
            AgentSourceIsaCharacteristicErrorCodeV1::AllocationFailure
        }
        _ => AgentSourceIsaCharacteristicErrorCodeV1::InvalidCollection,
    }
}

fn decode_identity(value: &str) -> Result<[u8; 32], AgentSourceIsaCharacteristicErrorCodeV1> {
    let decoded = decode_fixed_hex(value)?;
    if decoded == [0; 32] {
        Err(AgentSourceIsaCharacteristicErrorCodeV1::InvalidLowercaseHex)
    } else {
        Ok(decoded)
    }
}

fn decode_cursor(
    value: &str,
) -> Result<SourceIsaCharacteristicCursorV1, AgentSourceIsaCharacteristicErrorCodeV1> {
    if value.len() != HEX_CURSOR_BYTES_V1 {
        return Err(AgentSourceIsaCharacteristicErrorCodeV1::InvalidCursor);
    }
    let decoded: [u8; SOURCE_ISA_CHARACTERISTIC_CURSOR_BYTES_V1] = decode_fixed_hex(value)
        .map_err(|_| AgentSourceIsaCharacteristicErrorCodeV1::InvalidCursor)?;
    SourceIsaCharacteristicCursorV1::decode_canonical(&decoded)
        .map_err(|_| AgentSourceIsaCharacteristicErrorCodeV1::InvalidCursor)
}

fn encode_cursor(
    value: SourceIsaCharacteristicCursorV1,
) -> Result<String, AgentSourceIsaCharacteristicErrorCodeV1> {
    hex_bytes(value.encode_canonical())
}

fn decode_fixed_hex<const N: usize>(
    value: &str,
) -> Result<[u8; N], AgentSourceIsaCharacteristicErrorCodeV1> {
    if value.len()
        != N.checked_mul(2)
            .ok_or(AgentSourceIsaCharacteristicErrorCodeV1::InvalidLowercaseHex)?
        || !value
            .as_bytes()
            .iter()
            .all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f'))
    {
        return Err(AgentSourceIsaCharacteristicErrorCodeV1::InvalidLowercaseHex);
    }
    let mut decoded = [0; N];
    for (index, pair) in value.as_bytes().chunks_exact(2).enumerate() {
        decoded[index] = (lower_nibble(pair[0])? << 4) | lower_nibble(pair[1])?;
    }
    Ok(decoded)
}

fn lower_nibble(value: u8) -> Result<u8, AgentSourceIsaCharacteristicErrorCodeV1> {
    match value {
        b'0'..=b'9' => Ok(value - b'0'),
        b'a'..=b'f' => Ok(value - b'a' + 10),
        _ => Err(AgentSourceIsaCharacteristicErrorCodeV1::InvalidLowercaseHex),
    }
}

fn hex_bytes<const N: usize>(
    value: [u8; N],
) -> Result<String, AgentSourceIsaCharacteristicErrorCodeV1> {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let capacity = N
        .checked_mul(2)
        .ok_or(AgentSourceIsaCharacteristicErrorCodeV1::AllocationFailure)?;
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(capacity)
        .map_err(|_| AgentSourceIsaCharacteristicErrorCodeV1::AllocationFailure)?;
    for byte in value {
        bytes.push(HEX[usize::from(byte >> 4)]);
        bytes.push(HEX[usize::from(byte & 0x0f)]);
    }
    String::from_utf8(bytes).map_err(|_| AgentSourceIsaCharacteristicErrorCodeV1::InvalidCollection)
}

pub fn encode_agent_source_isa_characteristic_request_line_v1(
    request: &AgentSourceIsaCharacteristicRequestV1,
) -> Result<String, AgentSourceIsaCharacteristicDecodeErrorV1> {
    serialize_jsonl_bounded(
        request,
        MAX_AGENT_SOURCE_ISA_CHARACTERISTIC_REQUEST_BYTES_V1,
    )
}

pub fn decode_agent_source_isa_characteristic_request_line_v1(
    line: &[u8],
) -> Result<AgentSourceIsaCharacteristicRequestV1, AgentSourceIsaCharacteristicDecodeErrorV1> {
    let payload = canonical_payload(line, MAX_AGENT_SOURCE_ISA_CHARACTERISTIC_REQUEST_BYTES_V1)?;
    let request: AgentSourceIsaCharacteristicRequestV1 = serde_json::from_slice(payload)
        .map_err(|_| AgentSourceIsaCharacteristicDecodeErrorV1::InvalidCanonicalJsonl)?;
    let canonical = serialize_json_bounded(&request, payload.len())?;
    if canonical.as_bytes() != payload {
        return Err(AgentSourceIsaCharacteristicDecodeErrorV1::InvalidCanonicalJsonl);
    }
    Ok(request)
}

pub fn encode_agent_source_isa_characteristic_response_line_v1(
    response: &AgentSourceIsaCharacteristicResponseV1,
) -> Result<String, AgentSourceIsaCharacteristicErrorCodeV1> {
    serialize_jsonl_bounded(
        response,
        MAX_AGENT_SOURCE_ISA_CHARACTERISTIC_RESPONSE_BYTES_V1,
    )
    .map_err(|_| AgentSourceIsaCharacteristicErrorCodeV1::ResponseTooLarge)
}

pub fn decode_agent_source_isa_characteristic_response_line_v1(
    line: &[u8],
) -> Result<AgentSourceIsaCharacteristicResponseV1, AgentSourceIsaCharacteristicDecodeErrorV1> {
    let payload = canonical_payload(line, MAX_AGENT_SOURCE_ISA_CHARACTERISTIC_RESPONSE_BYTES_V1)?;
    let response: AgentSourceIsaCharacteristicResponseV1 = serde_json::from_slice(payload)
        .map_err(|_| AgentSourceIsaCharacteristicDecodeErrorV1::InvalidCanonicalJsonl)?;
    if !response.has_valid_envelope() {
        return Err(AgentSourceIsaCharacteristicDecodeErrorV1::InvalidCanonicalJsonl);
    }
    let canonical = serialize_json_bounded(&response, payload.len())?;
    if canonical.as_bytes() != payload {
        return Err(AgentSourceIsaCharacteristicDecodeErrorV1::InvalidCanonicalJsonl);
    }
    Ok(response)
}

fn canonical_payload(
    line: &[u8],
    limit: usize,
) -> Result<&[u8], AgentSourceIsaCharacteristicDecodeErrorV1> {
    if line.is_empty() || line.len() > limit {
        return Err(AgentSourceIsaCharacteristicDecodeErrorV1::LineTooLarge);
    }
    let payload = line.strip_suffix(b"\n").unwrap_or(line);
    if payload.is_empty() || payload.contains(&b'\n') || payload.contains(&b'\r') {
        return Err(AgentSourceIsaCharacteristicDecodeErrorV1::InvalidCanonicalJsonl);
    }
    Ok(payload)
}

fn serialize_json_bounded<T: Serialize>(
    value: &T,
    limit: usize,
) -> Result<String, AgentSourceIsaCharacteristicDecodeErrorV1> {
    let mut writer = BoundedJsonWriterV1::new(limit)?;
    serde_json::to_writer(&mut writer, value)
        .map_err(|_| AgentSourceIsaCharacteristicDecodeErrorV1::InvalidCanonicalJsonl)?;
    String::from_utf8(writer.bytes)
        .map_err(|_| AgentSourceIsaCharacteristicDecodeErrorV1::InvalidCanonicalJsonl)
}

fn serialize_jsonl_bounded<T: Serialize>(
    value: &T,
    limit: usize,
) -> Result<String, AgentSourceIsaCharacteristicDecodeErrorV1> {
    let payload_limit = limit
        .checked_sub(1)
        .ok_or(AgentSourceIsaCharacteristicDecodeErrorV1::LineTooLarge)?;
    let mut encoded = serialize_json_bounded(value, payload_limit)?;
    if encoded.len() >= limit {
        return Err(AgentSourceIsaCharacteristicDecodeErrorV1::LineTooLarge);
    }
    encoded
        .try_reserve_exact(1)
        .map_err(|_| AgentSourceIsaCharacteristicDecodeErrorV1::LineTooLarge)?;
    encoded.push('\n');
    Ok(encoded)
}

struct BoundedJsonWriterV1 {
    bytes: Vec<u8>,
    limit: usize,
}

impl BoundedJsonWriterV1 {
    fn new(limit: usize) -> Result<Self, AgentSourceIsaCharacteristicDecodeErrorV1> {
        let mut bytes = Vec::new();
        bytes
            .try_reserve_exact(limit)
            .map_err(|_| AgentSourceIsaCharacteristicDecodeErrorV1::LineTooLarge)?;
        Ok(Self { bytes, limit })
    }
}

impl Write for BoundedJsonWriterV1 {
    fn write(&mut self, bytes: &[u8]) -> Result<usize, std::io::Error> {
        if bytes.len() > self.limit.saturating_sub(self.bytes.len()) {
            return Err(std::io::Error::new(
                std::io::ErrorKind::FileTooLarge,
                "source/ISA characteristic JSON exceeds its byte bound",
            ));
        }
        self.bytes.extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> Result<(), std::io::Error> {
        Ok(())
    }
}

pub fn run_agent_source_isa_characteristic_jsonl_v1<R: BufRead, W: Write>(
    service: &mut AgentSourceIsaCharacteristicServiceV1,
    input: &mut R,
    output: &mut W,
) -> Result<(), std::io::Error> {
    loop {
        let Some(line) = read_request_line(input)? else {
            return Ok(());
        };
        let response = service.handle_line(&line);
        let terminal = response.is_terminal();
        write_response(output, response)?;
        if terminal {
            return Ok(());
        }
    }
}

fn write_response<W: Write>(
    output: &mut W,
    response: AgentSourceIsaCharacteristicResponseV1,
) -> Result<(), std::io::Error> {
    let context = response.context();
    let encoded = match encode_agent_source_isa_characteristic_response_line_v1(&response) {
        Ok(encoded) => encoded,
        Err(_) => {
            let fallback = AgentSourceIsaCharacteristicResponseV1::error(
                context.0,
                context.1,
                context.2,
                AgentSourceIsaCharacteristicErrorCodeV1::ResponseTooLarge,
                false,
            );
            encode_agent_source_isa_characteristic_response_line_v1(&fallback).map_err(|_| {
                std::io::Error::new(
                    std::io::ErrorKind::FileTooLarge,
                    "bounded source/ISA characteristic fallback response could not be encoded",
                )
            })?
        }
    };
    output.write_all(encoded.as_bytes())?;
    output.flush()
}

fn read_request_line<R: BufRead>(input: &mut R) -> Result<Option<Vec<u8>>, std::io::Error> {
    let mut line = Vec::new();
    let capacity = MAX_AGENT_SOURCE_ISA_CHARACTERISTIC_REQUEST_BYTES_V1
        .checked_add(1)
        .ok_or_else(|| {
            std::io::Error::new(
                std::io::ErrorKind::OutOfMemory,
                "source/ISA characteristic request bound overflowed",
            )
        })?;
    line.try_reserve_exact(capacity).map_err(|_| {
        std::io::Error::new(
            std::io::ErrorKind::OutOfMemory,
            "cannot reserve bounded source/ISA characteristic request line",
        )
    })?;
    loop {
        let buffer = input.fill_buf()?;
        if buffer.is_empty() {
            return if line.is_empty() {
                Ok(None)
            } else {
                Ok(Some(line))
            };
        }
        let newline = buffer.iter().position(|byte| *byte == b'\n');
        let available = newline.map_or(buffer.len(), |position| position + 1);
        let remaining = MAX_AGENT_SOURCE_ISA_CHARACTERISTIC_REQUEST_BYTES_V1
            .saturating_add(1)
            .saturating_sub(line.len());
        let consumed = available.min(remaining);
        line.extend_from_slice(&buffer[..consumed]);
        input.consume(consumed);
        if line.len() > MAX_AGENT_SOURCE_ISA_CHARACTERISTIC_REQUEST_BYTES_V1
            || (newline.is_some() && consumed == available)
        {
            return Ok(Some(line));
        }
    }
}

#[derive(Default)]
struct RequestIdentityHintV1 {
    request_id: Option<u64>,
    operation: Option<String>,
}

impl<'de> Deserialize<'de> for RequestIdentityHintV1 {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_map(RequestIdentityHintVisitorV1)
    }
}

struct RequestIdentityHintVisitorV1;

impl<'de> Visitor<'de> for RequestIdentityHintVisitorV1 {
    type Value = RequestIdentityHintV1;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .write_str("a source/ISA characteristic request object with unique correlation keys")
    }

    fn visit_map<M>(self, mut map: M) -> Result<Self::Value, M::Error>
    where
        M: MapAccess<'de>,
    {
        let mut hint = RequestIdentityHintV1::default();
        let mut request_id_seen = false;
        let mut operation_seen = false;
        while let Some(key) = map.next_key::<String>()? {
            match key.as_str() {
                "request_id" => {
                    if request_id_seen {
                        return Err(de::Error::duplicate_field("request_id"));
                    }
                    request_id_seen = true;
                    hint.request_id = Some(map.next_value()?);
                }
                "operation" => {
                    if operation_seen {
                        return Err(de::Error::duplicate_field("operation"));
                    }
                    operation_seen = true;
                    hint.operation = Some(map.next_value()?);
                }
                _ => {
                    map.next_value::<IgnoredAny>()?;
                }
            }
        }
        Ok(hint)
    }
}

fn request_identity_hint(
    line: &[u8],
) -> (Option<u64>, Option<AgentSourceIsaCharacteristicOperationV1>) {
    let payload = line.strip_suffix(b"\n").unwrap_or(line);
    let Ok(hint) = serde_json::from_slice::<RequestIdentityHintV1>(payload) else {
        return (None, None);
    };
    let request_id = hint.request_id.filter(|value| *value != 0);
    let operation = match hint.operation.as_deref() {
        Some("discover_capabilities") => {
            Some(AgentSourceIsaCharacteristicOperationV1::DiscoverCapabilities)
        }
        Some("query_targets") => Some(AgentSourceIsaCharacteristicOperationV1::QueryTargets),
        Some("query_facts") => Some(AgentSourceIsaCharacteristicOperationV1::QueryFacts),
        Some("query_intervals") => Some(AgentSourceIsaCharacteristicOperationV1::QueryIntervals),
        _ => None,
    };
    (request_id, operation)
}

impl AgentSourceIsaCharacteristicResponseV1 {
    fn has_valid_envelope(&self) -> bool {
        match self {
            Self::Ok {
                schema,
                request_id,
                response_revision,
                operation,
                result,
            } => {
                schema == AGENT_SOURCE_ISA_CHARACTERISTIC_RESPONSE_SCHEMA_V1
                    && *request_id != 0
                    && *response_revision != 0
                    && result.is_valid_for(*operation)
            }
            Self::Error {
                schema,
                request_id,
                response_revision,
                operation,
                error,
                terminal,
            } => {
                schema == AGENT_SOURCE_ISA_CHARACTERISTIC_RESPONSE_SCHEMA_V1
                    && request_id.is_none_or(|value| value != 0)
                    && *response_revision != 0
                    && *terminal
                        == matches!(
                            error,
                            AgentSourceIsaCharacteristicErrorCodeV1::RequestTooLarge
                                | AgentSourceIsaCharacteristicErrorCodeV1::RequestBudgetExhausted
                        )
                    && (!*terminal || (request_id.is_none() && operation.is_none()))
            }
        }
    }
}

impl AgentSourceIsaCharacteristicResultV1 {
    fn is_valid_for(&self, operation: AgentSourceIsaCharacteristicOperationV1) -> bool {
        let (authority, collection) = match self {
            Self::Capabilities {
                authority,
                limits: actual_limits,
                capabilities: actual_capabilities,
                collection,
            } => {
                if operation != AgentSourceIsaCharacteristicOperationV1::DiscoverCapabilities
                    || *actual_limits != limits()
                    || *actual_capabilities != capabilities()
                {
                    return false;
                }
                (authority, collection)
            }
            Self::TargetPage {
                authority,
                collection,
                page: _,
            } => {
                if operation != AgentSourceIsaCharacteristicOperationV1::QueryTargets {
                    return false;
                }
                (authority, collection)
            }
            Self::FactPage {
                authority,
                collection,
                page: _,
            } => {
                if operation != AgentSourceIsaCharacteristicOperationV1::QueryFacts {
                    return false;
                }
                (authority, collection)
            }
            Self::IntervalPage {
                authority,
                collection,
                page: _,
            } => {
                if operation != AgentSourceIsaCharacteristicOperationV1::QueryIntervals {
                    return false;
                }
                (authority, collection)
            }
        };
        let Ok(collection_identity) = decode_identity(&collection.identity) else {
            return false;
        };
        let page_valid = match self {
            Self::Capabilities { .. } => true,
            Self::TargetPage { page, .. } => page.is_valid_for(collection, collection_identity),
            Self::FactPage { page, .. } => page.is_valid_for(collection, collection_identity),
            Self::IntervalPage { page, .. } => page.is_valid_for(collection_identity),
        };
        authority.has_valid_nonclaims()
            && collection.is_valid()
            && page_valid
            && authority.sparse_final_hsaco_anchors_present
                == collection.sparse_final_hsaco_anchors_present
    }
}

impl AgentSourceIsaCharacteristicCollectionSummaryV1 {
    fn is_valid(&self) -> bool {
        let minimum_length =
            SOURCE_ISA_CHARACTERISTIC_HEADER_BYTES_V1 + SOURCE_ISA_CHARACTERISTIC_IDENTITY_BYTES_V1;
        let canonical_len_valid = usize::try_from(self.canonical_byte_len).is_ok_and(|length| {
            (minimum_length..=MAX_SOURCE_ISA_CHARACTERISTIC_COLLECTION_BYTES_V1).contains(&length)
        });
        self.format == "fe2o3-source-isa-characteristic-collection-v1"
            && valid_nonzero_identity(&self.identity)
            && canonical_len_valid
            && self.binding.is_valid()
            && self.scan.is_valid()
            && usize::try_from(self.target_count)
                .is_ok_and(|count| count <= MAX_SOURCE_ISA_CHARACTERISTIC_TARGETS_V1)
            && usize::try_from(self.scan.retained_target_correlation_count)
                .is_ok_and(|count| count <= MAX_SOURCE_ISA_CHARACTERISTIC_TARGET_CORRELATIONS_V1)
            && usize::try_from(self.pre_kir_elimination_count)
                .is_ok_and(|count| count <= MAX_SOURCE_ISA_CHARACTERISTIC_PRE_KIR_ELIMINATIONS_V1)
            && self.scan.target_operation_count == self.binding.structural_counts.operations
            && self.target_count == self.scan.classified_target_count
            && self.pre_kir_elimination_count == self.scan.pre_kir_elimination_count
            && !self.decoded_isa
    }
}

impl AgentSourceIsaCharacteristicBindingV1 {
    fn is_valid(&self) -> bool {
        matches!(
            (self.target_profile.code, self.target_profile.label),
            (1, AgentSourceIsaTargetProfileLabelV1::Gfx942)
                | (2, AgentSourceIsaTargetProfileLabelV1::Gfx950)
        ) && matches!(self.kir_version, 8 | 9)
            && valid_nonzero_identity(&self.structural_identity)
            && self.structural_counts.defined_bodies <= self.structural_counts.functions
            && self.structural_counts.blocks >= self.structural_counts.defined_bodies
            && self.source_map_v2.is_valid()
            && self.neutral_kir.is_valid()
            && self.target_kir.is_valid()
            && self.artifact.is_valid()
            && self.catalog.is_valid()
            && self.structural_bridge.is_valid()
            && valid_nonzero_identity(&self.correlation_identity)
            && valid_nonzero_identity(&self.semantic_map_identity)
    }
}

impl AgentSourceIsaContentIdentityV1 {
    fn is_valid(&self) -> bool {
        valid_nonzero_identity(&self.sha256) && self.byte_len != 0
    }
}

impl AgentSourceIsaCharacteristicScanSummaryV1 {
    fn is_valid(&self) -> bool {
        let Some(correlation_count) = self
            .retained_target_correlation_count
            .checked_add(self.pre_kir_elimination_count)
        else {
            return false;
        };
        if self.catalog_records_scanned > self.catalog_record_count
            || self.target_operations_scanned > self.target_operation_count
            || self.classified_target_count > self.target_operations_scanned
            || self.correlation_count != correlation_count
            || self.correlation_count > self.catalog_records_scanned
        {
            return false;
        }
        if !usize::try_from(self.catalog_record_count)
            .is_ok_and(|count| count <= MAX_SOURCE_ISA_CHARACTERISTIC_CATALOG_RECORDS_V1)
            || !usize::try_from(self.catalog_records_scanned)
                .is_ok_and(|count| count <= MAX_SOURCE_ISA_CHARACTERISTIC_CATALOG_RECORDS_V1)
        {
            return false;
        }
        let total_correlation_limit = MAX_SOURCE_ISA_CHARACTERISTIC_TARGET_CORRELATIONS_V1
            .checked_add(MAX_SOURCE_ISA_CHARACTERISTIC_PRE_KIR_ELIMINATIONS_V1);
        if !usize::try_from(self.classified_target_count)
            .is_ok_and(|count| count <= MAX_SOURCE_ISA_CHARACTERISTIC_TARGETS_V1)
            || !usize::try_from(self.retained_target_correlation_count)
                .is_ok_and(|count| count <= MAX_SOURCE_ISA_CHARACTERISTIC_TARGET_CORRELATIONS_V1)
            || !usize::try_from(self.pre_kir_elimination_count)
                .is_ok_and(|count| count <= MAX_SOURCE_ISA_CHARACTERISTIC_PRE_KIR_ELIMINATIONS_V1)
            || !usize::try_from(self.correlation_count)
                .is_ok_and(|count| total_correlation_limit.is_some_and(|limit| count <= limit))
        {
            return false;
        }
        match self.state {
            AgentSourceIsaCharacteristicScanStateV1::Complete => {
                self.catalog_records_scanned == self.catalog_record_count
                    && self.target_operations_scanned == self.target_operation_count
            }
            AgentSourceIsaCharacteristicScanStateV1::Missing { code, reason } => matches!(
                (code, reason),
                (
                    1,
                    AgentSourceIsaMissingReasonV1::NoQualifyingTargetOperation
                ) | (2, AgentSourceIsaMissingReasonV1::NoAdmittedCorrelation)
            ),
            AgentSourceIsaCharacteristicScanStateV1::Unavailable { code, reason } => matches!(
                (code, reason),
                (1, AgentSourceIsaUnavailableReasonV1::CatalogUnavailable)
                    | (
                        2,
                        AgentSourceIsaUnavailableReasonV1::StructuralBridgeUnavailable
                    )
                    | (3, AgentSourceIsaUnavailableReasonV1::ClassifierUnavailable)
                    | (
                        4,
                        AgentSourceIsaUnavailableReasonV1::SourceProjectionUnavailable
                    )
            ),
            AgentSourceIsaCharacteristicScanStateV1::Error { code, error } => matches!(
                (code, error),
                (1, AgentSourceIsaObservationErrorV1::InvalidCatalog)
                    | (2, AgentSourceIsaObservationErrorV1::InvalidStructuralBridge)
                    | (3, AgentSourceIsaObservationErrorV1::InvalidClassification)
                    | (4, AgentSourceIsaObservationErrorV1::ConflictingEvidence)
                    | (5, AgentSourceIsaObservationErrorV1::ResourceLimit)
                    | (6, AgentSourceIsaObservationErrorV1::AllocationFailure)
            ),
        }
    }
}

impl AgentSourceIsaCharacteristicTargetPageV1 {
    fn is_valid_for(
        &self,
        summary: &AgentSourceIsaCharacteristicCollectionSummaryV1,
        collection: [u8; 32],
    ) -> bool {
        self.targets.len() <= usize::from(MAX_AGENT_SOURCE_ISA_CHARACTERISTIC_PAGE_ITEMS_V1)
            && u64::try_from(self.targets.len()).is_ok_and(|count| count <= self.total_matches)
            && self.total_matches <= summary.target_count
            && usize::try_from(self.total_matches)
                .is_ok_and(|count| count <= MAX_SOURCE_ISA_CHARACTERISTIC_TARGETS_V1)
            && self.page_exhausted == self.next_cursor.is_none()
            && valid_optional_cursor(&self.cursor)
            && valid_optional_cursor(&self.next_cursor)
            && decode_target_query(&self.query).is_ok_and(|query| {
                hex_bytes(query.identity()).is_ok_and(|identity| identity == self.query_identity)
            })
            && cursors_bind(
                &self.cursor,
                &self.next_cursor,
                collection,
                decode_target_query(&self.query).map(|query| query.identity()),
            )
            && self
                .targets
                .iter()
                .all(|target| target.is_valid_for(collection))
            && unique_occurrences(
                self.targets
                    .iter()
                    .map(|target| target.occurrence_identity.as_str()),
            )
            && cursor_chain_is_valid(
                &self.cursor,
                &self.next_cursor,
                self.targets.len(),
                self.targets
                    .last()
                    .map(|target| &target.occurrence_identity),
            )
            && page_position_is_valid(
                &self.cursor,
                self.total_matches,
                self.targets.len(),
                self.page_exhausted,
            )
    }
}

impl AgentSourceIsaCharacteristicTargetV1 {
    fn is_valid_for(&self, collection: [u8; 32]) -> bool {
        let occurrence_valid = decode_identity(&self.occurrence_identity).is_ok_and(|occurrence| {
            decode_identity(&self.characteristic_identity).is_ok_and(|characteristic| {
                decode_kind(self.kind.code).is_ok_and(|kind| {
                    decode_kir(self.target_kir).is_ok_and(|target_kir| {
                        source_isa_characteristic_target_match_identity_v1(
                            collection,
                            characteristic,
                            kind,
                            target_kir,
                            self.correlation_count,
                        ) == Ok(occurrence)
                    })
                })
            })
        });
        occurrence_valid
            && valid_nonzero_identity(&self.characteristic_identity)
            && valid_category(self.category)
            && valid_kind(self.kind)
            && self.category.code == category_for_kind(self.kind).code
            && valid_kir(self.target_kir)
            && usize::try_from(self.correlation_count).is_ok_and(|count| {
                count <= MAX_SOURCE_ISA_CHARACTERISTIC_CORRELATIONS_PER_TARGET_V1
            })
    }
}

impl AgentSourceIsaCharacteristicFactPageV1 {
    fn is_valid_for(
        &self,
        summary: &AgentSourceIsaCharacteristicCollectionSummaryV1,
        collection: [u8; 32],
    ) -> bool {
        self.facts.len() <= usize::from(MAX_AGENT_SOURCE_ISA_CHARACTERISTIC_PAGE_ITEMS_V1)
            && u64::try_from(self.facts.len()).is_ok_and(|count| count <= self.total_matches)
            && self.total_matches <= summary.scan.correlation_count
            && self.page_exhausted == self.next_cursor.is_none()
            && valid_optional_cursor(&self.cursor)
            && valid_optional_cursor(&self.next_cursor)
            && decode_query(&self.query).is_ok_and(|query| {
                hex_bytes(query.identity()).is_ok_and(|identity| identity == self.query_identity)
            })
            && cursors_bind(
                &self.cursor,
                &self.next_cursor,
                collection,
                decode_query(&self.query).map(|query| query.identity()),
            )
            && self.facts.iter().all(|fact| fact.is_valid_for(collection))
            && unique_occurrences(
                self.facts
                    .iter()
                    .map(|fact| fact.occurrence_identity.as_str()),
            )
            && cursor_chain_is_valid(
                &self.cursor,
                &self.next_cursor,
                self.facts.len(),
                self.facts.last().map(|fact| &fact.occurrence_identity),
            )
            && page_position_is_valid(
                &self.cursor,
                self.total_matches,
                self.facts.len(),
                self.page_exhausted,
            )
    }
}

impl AgentSourceIsaCharacteristicFactV1 {
    fn is_valid_for(&self, collection: [u8; 32]) -> bool {
        let Ok(occurrence) = decode_identity(&self.occurrence_identity) else {
            return false;
        };
        match &self.outcome {
            AgentSourceIsaCharacteristicFactOutcomeV1::TargetCorrelation { correlation } => {
                self.characteristic_identity
                    .as_deref()
                    .is_some_and(|identity| {
                        decode_identity(identity).is_ok_and(|target| {
                            source_isa_characteristic_target_correlation_match_identity_v1(
                                collection,
                                target,
                                correlation.catalog_record_ordinal,
                            ) == occurrence
                        })
                    })
                    && self.category.is_some_and(valid_category)
                    && self.kind.is_some_and(valid_kind)
                    && self.target_kir.is_some_and(valid_kir)
                    && self.kind.is_some_and(|kind| {
                        self.category
                            .is_some_and(|category| category.code == category_for_kind(kind).code)
                    })
                    && self.target_kir == Some(correlation.target_kir)
                    && correlation.is_valid()
            }
            AgentSourceIsaCharacteristicFactOutcomeV1::PreKirElimination { elimination } => {
                self.characteristic_identity.is_none()
                    && self.category.is_none()
                    && self.kind.is_none()
                    && self.target_kir.is_none()
                    && source_isa_characteristic_pre_kir_match_identity_v1(
                        collection,
                        elimination.catalog_record_ordinal,
                    ) == occurrence
                    && elimination.is_valid()
            }
        }
    }
}

impl AgentSourceIsaTargetCorrelationV1 {
    fn is_valid(&self) -> bool {
        let attribution_shape = match self.record_kind.label {
            AgentSourceIsaRecordKindLabelV1::SourceAnchored => {
                self.source
                    .as_ref()
                    .is_some_and(|source| source.is_valid(false))
                    && self
                        .mir_node_identity
                        .as_deref()
                        .is_some_and(valid_nonzero_identity)
                    && self.mir.is_some_and(valid_mir)
                    && self
                        .neutral_kir_node_identity
                        .as_deref()
                        .is_some_and(valid_nonzero_identity)
                    && self.neutral_kir.is_some_and(valid_kir)
            }
            AgentSourceIsaRecordKindLabelV1::NoSourceProvenance => {
                self.source.is_none()
                    && self.mir_node_identity.is_none()
                    && self.mir.is_none()
                    && self.neutral_kir_node_identity.is_none()
                    && self.neutral_kir.is_none()
            }
            AgentSourceIsaRecordKindLabelV1::EliminatedBeforeKir => false,
        };
        valid_record_kind(self.record_kind)
            && self.record_kind.label != AgentSourceIsaRecordKindLabelV1::EliminatedBeforeKir
            && attribution_shape
            && valid_kir(self.target_kir)
            && valid_nonzero_identity(&self.semantic_operation_identity)
            && valid_llvm(self.compiler_handoff_llvm)
            && valid_transformation(self.transformation)
            && (self.interval_count == 0)
                == (self.transformation.label == AgentSourceIsaTransformationLabelV1::Eliminated)
            && usize::try_from(self.interval_count)
                .is_ok_and(|count| count <= MAX_SOURCE_ISA_CHARACTERISTIC_INTERVALS_V1)
    }
}

impl AgentSourceIsaPreKirEliminationV1 {
    fn is_valid(&self) -> bool {
        self.record_kind
            == project_record_kind(SourceIsaCharacteristicRecordKindV1::EliminatedBeforeKir)
            && self.source.is_valid(true)
            && valid_nonzero_identity(&self.mir_node_identity)
            && valid_mir(self.mir)
    }
}

impl AgentSourceIsaSourceCoordinateV1 {
    fn is_valid(&self, allow_empty_span: bool) -> bool {
        valid_nonzero_identity(&self.node_identity)
            && valid_nonzero_identity(&self.span.file_identity)
            && if allow_empty_span {
                self.span.byte_start <= self.span.byte_end
            } else {
                self.span.byte_start < self.span.byte_end
            }
            && self.span.line != 0
            && self.span.column != 0
    }
}

impl AgentSourceIsaCharacteristicIntervalPageV1 {
    fn is_valid_for(&self, collection: [u8; 32]) -> bool {
        let Ok(occurrence) = decode_identity(&self.occurrence_identity) else {
            return false;
        };
        let Ok(query_identity) = source_isa_characteristic_interval_query_identity_v1(occurrence)
        else {
            return false;
        };
        let start = match &self.cursor {
            None => 0,
            Some(cursor) => match decode_cursor(cursor) {
                Ok(cursor) => cursor.next_ordinal(),
                Err(_) => return false,
            },
        };
        valid_nonzero_identity(&self.occurrence_identity)
            && self.intervals.len()
                <= usize::from(MAX_AGENT_SOURCE_ISA_CHARACTERISTIC_PAGE_ITEMS_V1)
            && u64::try_from(self.intervals.len()).is_ok_and(|count| count <= self.total_intervals)
            && usize::try_from(self.total_intervals)
                .is_ok_and(|count| count <= MAX_SOURCE_ISA_CHARACTERISTIC_INTERVALS_V1)
            && self.page_exhausted == self.next_cursor.is_none()
            && valid_optional_cursor(&self.cursor)
            && valid_optional_cursor(&self.next_cursor)
            && cursors_bind(
                &self.cursor,
                &self.next_cursor,
                collection,
                Ok(query_identity),
            )
            && self.intervals.iter().enumerate().all(|(offset, item)| {
                let Ok(offset) = u64::try_from(offset) else {
                    return false;
                };
                let Some(expected_ordinal) = start.checked_add(offset) else {
                    return false;
                };
                let Ok(identity) = decode_identity(&item.identity) else {
                    return false;
                };
                let Ok(core_interval) = SourceIsaCharacteristicIsaIntervalV1::new(
                    item.interval.kernel_ordinal,
                    item.interval.symbol_relative_start,
                    item.interval.symbol_relative_end,
                ) else {
                    return false;
                };
                valid_nonzero_identity(&item.identity)
                    && item.ordinal == expected_ordinal
                    && source_isa_characteristic_interval_item_identity_v1(
                        collection,
                        occurrence,
                        item.ordinal,
                        core_interval,
                    ) == Ok(identity)
            })
            && cursor_chain_is_valid(
                &self.cursor,
                &self.next_cursor,
                self.intervals.len(),
                self.intervals.last().map(|item| &item.identity),
            )
            && page_position_is_valid(
                &self.cursor,
                self.total_intervals,
                self.intervals.len(),
                self.page_exhausted,
            )
    }
}

fn valid_nonzero_identity(value: &str) -> bool {
    value.len() == HEX_IDENTITY_BYTES_V1
        && value
            .as_bytes()
            .iter()
            .all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f'))
        && value.as_bytes().iter().any(|byte| *byte != b'0')
}

fn unique_occurrences<'a>(values: impl Iterator<Item = &'a str> + Clone) -> bool {
    for (ordinal, value) in values.clone().enumerate() {
        if values.clone().take(ordinal).any(|earlier| earlier == value) {
            return false;
        }
    }
    true
}

fn valid_optional_cursor(value: &Option<String>) -> bool {
    value
        .as_deref()
        .is_none_or(|value| decode_cursor(value).is_ok())
}

fn cursors_bind(
    cursor: &Option<String>,
    next_cursor: &Option<String>,
    collection: [u8; 32],
    query: Result<[u8; 32], AgentSourceIsaCharacteristicErrorCodeV1>,
) -> bool {
    let Ok(query) = query else {
        return false;
    };
    [cursor, next_cursor].into_iter().all(|value| {
        value.as_deref().is_none_or(|value| {
            decode_cursor(value).is_ok_and(|cursor| {
                cursor.collection_identity() == collection && cursor.query_identity() == query
            })
        })
    })
}

fn cursor_chain_is_valid(
    cursor: &Option<String>,
    next_cursor: &Option<String>,
    item_count: usize,
    last_item_identity: Option<&String>,
) -> bool {
    let start = match cursor {
        None => 0,
        Some(cursor) => match decode_cursor(cursor) {
            Ok(cursor) => cursor.next_ordinal(),
            Err(_) => return false,
        },
    };
    match next_cursor {
        None => true,
        Some(next) => {
            let Ok(next) = decode_cursor(next) else {
                return false;
            };
            let Ok(item_count) = u64::try_from(item_count) else {
                return false;
            };
            let Some(expected_next) = start.checked_add(item_count) else {
                return false;
            };
            let Some(last) = last_item_identity.and_then(|value| decode_identity(value).ok())
            else {
                return false;
            };
            next.next_ordinal() == expected_next && next.preceding_item_identity() == last
        }
    }
}

fn page_position_is_valid(
    cursor: &Option<String>,
    total: u64,
    item_count: usize,
    exhausted: bool,
) -> bool {
    let start = match cursor {
        None => 0,
        Some(cursor) => match decode_cursor(cursor) {
            Ok(cursor) if cursor.next_ordinal() < total => cursor.next_ordinal(),
            _ => return false,
        },
    };
    let Ok(item_count) = u64::try_from(item_count) else {
        return false;
    };
    let Some(end) = start.checked_add(item_count) else {
        return false;
    };
    end <= total && exhausted == (end == total)
}

fn valid_ordinal_coordinate(values: [u64; 3]) -> bool {
    values.into_iter().all(|value| value <= u64::from(u32::MAX))
}

fn valid_mir(value: AgentSourceIsaMirCoordinateV1) -> bool {
    valid_ordinal_coordinate([
        value.body_ordinal,
        value.block_ordinal,
        value.statement_ordinal,
    ])
}

fn valid_kir(value: AgentSourceIsaKirCoordinateV1) -> bool {
    valid_ordinal_coordinate([
        value.function_ordinal,
        value.block_ordinal,
        value.operation_ordinal,
    ])
}

fn valid_llvm(value: AgentSourceIsaLlvmCoordinateV1) -> bool {
    valid_ordinal_coordinate([
        value.function_ordinal,
        value.block_ordinal,
        value.instruction_ordinal,
    ])
}

fn valid_category(value: AgentSourceIsaCharacteristicCategoryV1) -> bool {
    matches!(
        (value.code, value.label),
        (
            1,
            AgentSourceIsaCharacteristicCategoryLabelV1::TargetKirGlobalStore
        ) | (
            2,
            AgentSourceIsaCharacteristicCategoryLabelV1::TargetKirWorkgroupLdsRead
        ) | (
            3,
            AgentSourceIsaCharacteristicCategoryLabelV1::TargetKirWorkgroupLdsWrite
        ) | (
            4,
            AgentSourceIsaCharacteristicCategoryLabelV1::TargetKirWorkgroupBarrier
        ) | (
            5,
            AgentSourceIsaCharacteristicCategoryLabelV1::TargetKirBf16MfmaExact
        )
    )
}

fn valid_memory_form(value: AgentSourceIsaMemoryFormV1) -> bool {
    matches!(
        (value.code, value.label),
        (1, AgentSourceIsaMemoryFormLabelV1::Plain)
            | (2, AgentSourceIsaMemoryFormLabelV1::Guarded)
            | (3, AgentSourceIsaMemoryFormLabelV1::MatrixTile)
    )
}

fn valid_kind(value: AgentSourceIsaCharacteristicKindV1) -> bool {
    match (value.code, value.label, value.memory_form) {
        (1..=3, AgentSourceIsaCharacteristicKindLabelV1::GlobalStore, Some(form)) => {
            valid_memory_form(form) && u16::from(form.code) == value.code
        }
        (4..=6, AgentSourceIsaCharacteristicKindLabelV1::WorkgroupLoad, Some(form)) => {
            valid_memory_form(form) && u16::from(form.code) + 3 == value.code
        }
        (7..=9, AgentSourceIsaCharacteristicKindLabelV1::WorkgroupStore, Some(form)) => {
            valid_memory_form(form) && u16::from(form.code) + 6 == value.code
        }
        (10, AgentSourceIsaCharacteristicKindLabelV1::WorkgroupBarrier, None)
        | (
            11,
            AgentSourceIsaCharacteristicKindLabelV1::Bf16F32M16N16K16Wave64MatrixMultiplyAccumulate,
            None,
        ) => true,
        _ => false,
    }
}

fn category_for_kind(
    value: AgentSourceIsaCharacteristicKindV1,
) -> AgentSourceIsaCharacteristicCategoryV1 {
    let category = match value.label {
        AgentSourceIsaCharacteristicKindLabelV1::GlobalStore => {
            SourceIsaCharacteristicCategoryV1::TargetKirGlobalStore
        }
        AgentSourceIsaCharacteristicKindLabelV1::WorkgroupLoad => {
            SourceIsaCharacteristicCategoryV1::TargetKirWorkgroupLdsRead
        }
        AgentSourceIsaCharacteristicKindLabelV1::WorkgroupStore => {
            SourceIsaCharacteristicCategoryV1::TargetKirWorkgroupLdsWrite
        }
        AgentSourceIsaCharacteristicKindLabelV1::WorkgroupBarrier => {
            SourceIsaCharacteristicCategoryV1::TargetKirWorkgroupBarrier
        }
        AgentSourceIsaCharacteristicKindLabelV1::Bf16F32M16N16K16Wave64MatrixMultiplyAccumulate => {
            SourceIsaCharacteristicCategoryV1::TargetKirBf16MfmaExact
        }
    };
    project_category(category)
}

fn valid_record_kind(value: AgentSourceIsaRecordKindV1) -> bool {
    matches!(
        (value.code, value.label),
        (1, AgentSourceIsaRecordKindLabelV1::EliminatedBeforeKir)
            | (2, AgentSourceIsaRecordKindLabelV1::SourceAnchored)
            | (3, AgentSourceIsaRecordKindLabelV1::NoSourceProvenance)
    )
}

fn valid_transformation(value: AgentSourceIsaTransformationV1) -> bool {
    matches!(
        (value.code, value.label),
        (1, AgentSourceIsaTransformationLabelV1::Preserved)
            | (2, AgentSourceIsaTransformationLabelV1::Duplicated)
            | (3, AgentSourceIsaTransformationLabelV1::Coalesced)
            | (
                4,
                AgentSourceIsaTransformationLabelV1::DuplicatedAndCoalesced
            )
            | (5, AgentSourceIsaTransformationLabelV1::Eliminated)
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use sha2::{Digest, Sha256};

    fn id(byte: u8) -> [u8; 32] {
        [byte; 32]
    }

    fn content(byte: u8, len: u64) -> SourceIsaCharacteristicContentIdentityV1 {
        SourceIsaCharacteristicContentIdentityV1::new(id(byte), len).unwrap()
    }

    fn kir(operation_ordinal: u64) -> SourceIsaCharacteristicKirCoordinateV1 {
        SourceIsaCharacteristicKirCoordinateV1::new(0, 0, operation_ordinal).unwrap()
    }

    fn span(byte: u8, empty: bool) -> SourceIsaCharacteristicSourceSpanV1 {
        let start = u64::from(byte);
        SourceIsaCharacteristicSourceSpanV1::new(
            id(byte + 1),
            start,
            if empty { start } else { start + 4 },
            u32::from(byte),
            1,
        )
        .unwrap()
    }

    fn collection() -> SourceIsaCharacteristicCollectionV1 {
        collection_for(SourceIsaCharacteristicTargetProfileV1::Gfx942)
    }

    fn collection_for(
        target_profile: SourceIsaCharacteristicTargetProfileV1,
    ) -> SourceIsaCharacteristicCollectionV1 {
        let binding = SourceIsaCharacteristicBindingV1::new(
            target_profile,
            SourceIsaCharacteristicKirVersionV1::V8,
            id(1),
            SourceIsaCharacteristicStructuralCountsV1 {
                functions: 1,
                defined_bodies: 1,
                blocks: 1,
                operations: 2,
            },
            content(2, 20),
            content(3, 30),
            content(4, 40),
            content(5, 50),
            content(6, 60),
            content(7, 70),
            id(8),
            id(9),
        )
        .unwrap();
        let interval = SourceIsaCharacteristicIsaIntervalV1::new(0, 0x40, 0x44).unwrap();
        let anchored = SourceIsaCharacteristicTargetCorrelationV1::new(
            0,
            SourceIsaCharacteristicRecordKindV1::SourceAnchored,
            Some(SourceIsaCharacteristicSourceCoordinateV1::new(id(20), span(20, false)).unwrap()),
            Some(id(22)),
            Some(SourceIsaCharacteristicMirCoordinateV1::new(0, 1, 2).unwrap()),
            Some(id(23)),
            Some(kir(1)),
            kir(1),
            id(24),
            SourceIsaCharacteristicCompilerHandoffLlvmCoordinateV1::new(0, 2, 3).unwrap(),
            vec![interval, interval],
            SourceIsaCharacteristicTransformationV1::Duplicated,
        )
        .unwrap();
        let eliminated = SourceIsaCharacteristicTargetCorrelationV1::new(
            1,
            SourceIsaCharacteristicRecordKindV1::NoSourceProvenance,
            None,
            None,
            None,
            None,
            None,
            kir(1),
            id(25),
            SourceIsaCharacteristicCompilerHandoffLlvmCoordinateV1::new(0, 2, 4).unwrap(),
            Vec::new(),
            SourceIsaCharacteristicTransformationV1::Eliminated,
        )
        .unwrap();
        let pre = SourceIsaCharacteristicPreKirEliminationV1::new(
            2,
            SourceIsaCharacteristicSourceCoordinateV1::new(id(30), span(30, true)).unwrap(),
            id(32),
            SourceIsaCharacteristicMirCoordinateV1::new(0, 3, 4).unwrap(),
        )
        .unwrap();
        SourceIsaCharacteristicCollectionV1::new(
            binding,
            SourceIsaCharacteristicScanSummaryV1::new(
                3,
                3,
                2,
                2,
                2,
                2,
                1,
                3,
                SourceIsaCharacteristicScanStateV1::Complete,
            )
            .unwrap(),
            vec![
                SourceIsaCharacteristicTargetV1::new(
                    SourceIsaCharacteristicKindV1::GlobalStore {
                        form: SourceIsaCharacteristicMemoryFormV1::Plain,
                    },
                    kir(1),
                    vec![anchored, eliminated],
                )
                .unwrap(),
                SourceIsaCharacteristicTargetV1::new(
                    SourceIsaCharacteristicKindV1::GlobalStore {
                        form: SourceIsaCharacteristicMemoryFormV1::Guarded,
                    },
                    kir(2),
                    Vec::new(),
                )
                .unwrap(),
            ],
            vec![pre],
        )
        .unwrap()
    }

    fn identity_string(collection: &SourceIsaCharacteristicCollectionV1) -> String {
        hex_bytes(collection.identity()).unwrap()
    }

    fn handle(
        service: &mut AgentSourceIsaCharacteristicServiceV1,
        request: &AgentSourceIsaCharacteristicRequestV1,
    ) -> AgentSourceIsaCharacteristicResponseV1 {
        let line = encode_agent_source_isa_characteristic_request_line_v1(request).unwrap();
        service.handle_line(line.as_bytes())
    }

    fn discover(request_id: u64) -> AgentSourceIsaCharacteristicRequestV1 {
        AgentSourceIsaCharacteristicRequestV1::DiscoverCapabilities {
            schema: AGENT_SOURCE_ISA_CHARACTERISTIC_REQUEST_SCHEMA_V1.to_owned(),
            request_id,
        }
    }

    fn error_code(
        response: AgentSourceIsaCharacteristicResponseV1,
    ) -> AgentSourceIsaCharacteristicErrorCodeV1 {
        let AgentSourceIsaCharacteristicResponseV1::Error { error, .. } = response else {
            panic!("expected typed agent error response");
        };
        error
    }

    fn assert_resealed_response_is_rejected(response: AgentSourceIsaCharacteristicResponseV1) {
        let encoded = encode_agent_source_isa_characteristic_response_line_v1(&response).unwrap();
        assert_eq!(
            decode_agent_source_isa_characteristic_response_line_v1(encoded.as_bytes()),
            Err(AgentSourceIsaCharacteristicDecodeErrorV1::InvalidCanonicalJsonl)
        );
    }

    fn forged_cursor(
        collection: [u8; 32],
        query: [u8; 32],
        next_ordinal: u64,
        preceding: [u8; 32],
    ) -> String {
        let mut bytes = [0; SOURCE_ISA_CHARACTERISTIC_CURSOR_BYTES_V1];
        bytes[..8].copy_from_slice(b"F2SICU1\0");
        bytes[8..10].copy_from_slice(&1_u16.to_le_bytes());
        bytes[10..12].copy_from_slice(&16_u16.to_le_bytes());
        bytes[12..16].copy_from_slice(&152_u32.to_le_bytes());
        bytes[16..48].copy_from_slice(&collection);
        bytes[48..80].copy_from_slice(&query);
        bytes[80..88].copy_from_slice(&next_ordinal.to_le_bytes());
        bytes[88..120].copy_from_slice(&preceding);
        let mut digest = Sha256::new();
        digest.update(b"FE2O3/SOURCE-ISA-CHARACTERISTIC-CURSOR/V1\0");
        digest.update(collection);
        digest.update(query);
        digest.update(next_ordinal.to_le_bytes());
        digest.update(preceding);
        let identity: [u8; 32] = digest.finalize().into();
        bytes[120..].copy_from_slice(&identity);
        hex_bytes(bytes).unwrap()
    }

    #[test]
    fn discovery_is_canonical_bound_and_explicitly_non_authoritative() {
        let exact = collection();
        let expected_identity = identity_string(&exact);
        let request = discover(1);
        let request_line =
            encode_agent_source_isa_characteristic_request_line_v1(&request).unwrap();
        assert_eq!(
            request_line,
            concat!(
                "{\"operation\":\"discover_capabilities\",",
                "\"schema\":\"fe2o3-agent-source-isa-characteristic-request-v1\",",
                "\"request_id\":1}\n"
            )
        );
        assert_eq!(
            decode_agent_source_isa_characteristic_request_line_v1(request_line.as_bytes())
                .unwrap(),
            request
        );
        let mut service = AgentSourceIsaCharacteristicServiceV1::new(exact).unwrap();
        let response = service.handle_line(request_line.as_bytes());
        let encoded = encode_agent_source_isa_characteristic_response_line_v1(&response).unwrap();
        assert!(encoded.len() <= MAX_AGENT_SOURCE_ISA_CHARACTERISTIC_RESPONSE_BYTES_V1);
        let decoded =
            decode_agent_source_isa_characteristic_response_line_v1(encoded.as_bytes()).unwrap();
        let AgentSourceIsaCharacteristicResponseV1::Ok {
            response_revision,
            result:
                AgentSourceIsaCharacteristicResultV1::Capabilities {
                    authority,
                    collection,
                    capabilities,
                    ..
                },
            ..
        } = decoded
        else {
            panic!("expected capabilities response");
        };
        assert_eq!(response_revision, 1);
        assert!(authority.has_valid_nonclaims());
        assert!(authority.sparse_final_hsaco_anchors_present);
        assert_eq!(collection.identity, expected_identity);
        assert_eq!(capabilities, super::capabilities());
    }

    #[test]
    fn target_fact_and_interval_planes_paginate_independently() {
        let exact = collection();
        let identity = identity_string(&exact);
        let mut service = AgentSourceIsaCharacteristicServiceV1::new(exact).unwrap();
        let first_target = handle(
            &mut service,
            &AgentSourceIsaCharacteristicRequestV1::QueryTargets {
                schema: AGENT_SOURCE_ISA_CHARACTERISTIC_REQUEST_SCHEMA_V1.to_owned(),
                request_id: 1,
                collection_identity: identity.clone(),
                query: AgentSourceIsaCharacteristicTargetQueryV1::All,
                cursor: None,
                limit: 1,
            },
        );
        let AgentSourceIsaCharacteristicResponseV1::Ok {
            result: AgentSourceIsaCharacteristicResultV1::TargetPage { page, .. },
            ..
        } = first_target
        else {
            panic!("expected target page");
        };
        assert_eq!(page.total_matches, 2);
        assert_eq!(page.targets[0].correlation_count, 2);
        let target_cursor = page.next_cursor.unwrap();

        let second_target = handle(
            &mut service,
            &AgentSourceIsaCharacteristicRequestV1::QueryTargets {
                schema: AGENT_SOURCE_ISA_CHARACTERISTIC_REQUEST_SCHEMA_V1.to_owned(),
                request_id: 2,
                collection_identity: identity.clone(),
                query: AgentSourceIsaCharacteristicTargetQueryV1::All,
                cursor: Some(target_cursor.clone()),
                limit: 1,
            },
        );
        let AgentSourceIsaCharacteristicResponseV1::Ok {
            result: AgentSourceIsaCharacteristicResultV1::TargetPage { page, .. },
            ..
        } = second_target
        else {
            panic!("expected second target page");
        };
        assert_eq!(page.targets[0].correlation_count, 0);
        assert!(page.page_exhausted);

        let cross_plane = handle(
            &mut service,
            &AgentSourceIsaCharacteristicRequestV1::QueryFacts {
                schema: AGENT_SOURCE_ISA_CHARACTERISTIC_REQUEST_SCHEMA_V1.to_owned(),
                request_id: 3,
                collection_identity: identity.clone(),
                query: AgentSourceIsaCharacteristicQueryV1::All,
                cursor: Some(target_cursor),
                limit: 1,
            },
        );
        assert_eq!(
            error_code(cross_plane),
            AgentSourceIsaCharacteristicErrorCodeV1::InvalidCursor
        );

        let fact = handle(
            &mut service,
            &AgentSourceIsaCharacteristicRequestV1::QueryFacts {
                schema: AGENT_SOURCE_ISA_CHARACTERISTIC_REQUEST_SCHEMA_V1.to_owned(),
                request_id: 4,
                collection_identity: identity.clone(),
                query: AgentSourceIsaCharacteristicQueryV1::ExactPc {
                    kernel_ordinal: 0,
                    symbol_relative_pc: 0x40,
                },
                cursor: None,
                limit: 1,
            },
        );
        let AgentSourceIsaCharacteristicResponseV1::Ok {
            result: AgentSourceIsaCharacteristicResultV1::FactPage { page, .. },
            ..
        } = fact
        else {
            panic!("expected fact page");
        };
        assert_eq!(page.facts.len(), 1);
        let occurrence = page.facts[0].occurrence_identity.clone();
        let first_interval = handle(
            &mut service,
            &AgentSourceIsaCharacteristicRequestV1::QueryIntervals {
                schema: AGENT_SOURCE_ISA_CHARACTERISTIC_REQUEST_SCHEMA_V1.to_owned(),
                request_id: 5,
                collection_identity: identity.clone(),
                occurrence_identity: occurrence.clone(),
                cursor: None,
                limit: 1,
            },
        );
        let AgentSourceIsaCharacteristicResponseV1::Ok {
            result: AgentSourceIsaCharacteristicResultV1::IntervalPage { page, .. },
            ..
        } = first_interval
        else {
            panic!("expected interval page");
        };
        assert_eq!(page.total_intervals, 2);
        let first_item = page.intervals[0].clone();
        let interval_cursor = page.next_cursor.unwrap();
        let second_interval = handle(
            &mut service,
            &AgentSourceIsaCharacteristicRequestV1::QueryIntervals {
                schema: AGENT_SOURCE_ISA_CHARACTERISTIC_REQUEST_SCHEMA_V1.to_owned(),
                request_id: 6,
                collection_identity: identity,
                occurrence_identity: occurrence,
                cursor: Some(interval_cursor),
                limit: 1,
            },
        );
        let AgentSourceIsaCharacteristicResponseV1::Ok {
            result: AgentSourceIsaCharacteristicResultV1::IntervalPage { page, .. },
            ..
        } = second_interval
        else {
            panic!("expected second interval page");
        };
        assert_eq!(page.intervals[0].interval, first_item.interval);
        assert_ne!(page.intervals[0].identity, first_item.identity);
    }

    #[test]
    fn malformed_identity_and_request_ids_are_typed_and_bounded() {
        let exact = collection();
        let identity = identity_string(&exact);
        let mut service = AgentSourceIsaCharacteristicServiceV1::new(exact).unwrap();
        let uppercase = handle(
            &mut service,
            &AgentSourceIsaCharacteristicRequestV1::QueryTargets {
                schema: AGENT_SOURCE_ISA_CHARACTERISTIC_REQUEST_SCHEMA_V1.to_owned(),
                request_id: 1,
                collection_identity: identity.to_uppercase(),
                query: AgentSourceIsaCharacteristicTargetQueryV1::All,
                cursor: None,
                limit: 1,
            },
        );
        assert_eq!(
            error_code(uppercase),
            AgentSourceIsaCharacteristicErrorCodeV1::InvalidLowercaseHex
        );
        for (request_id, malformed_identity) in [(2, "0".repeat(64)), (3, "a".to_owned())] {
            let response = handle(
                &mut service,
                &AgentSourceIsaCharacteristicRequestV1::QueryTargets {
                    schema: AGENT_SOURCE_ISA_CHARACTERISTIC_REQUEST_SCHEMA_V1.to_owned(),
                    request_id,
                    collection_identity: malformed_identity,
                    query: AgentSourceIsaCharacteristicTargetQueryV1::All,
                    cursor: None,
                    limit: 1,
                },
            );
            assert_eq!(
                error_code(response),
                AgentSourceIsaCharacteristicErrorCodeV1::InvalidLowercaseHex
            );
        }
        let wrong_collection = handle(
            &mut service,
            &AgentSourceIsaCharacteristicRequestV1::QueryTargets {
                schema: AGENT_SOURCE_ISA_CHARACTERISTIC_REQUEST_SCHEMA_V1.to_owned(),
                request_id: 4,
                collection_identity: hex_bytes(id(99)).unwrap(),
                query: AgentSourceIsaCharacteristicTargetQueryV1::All,
                cursor: None,
                limit: 1,
            },
        );
        assert_eq!(
            error_code(wrong_collection),
            AgentSourceIsaCharacteristicErrorCodeV1::CollectionIdentityMismatch
        );
        let invalid_limit = handle(
            &mut service,
            &AgentSourceIsaCharacteristicRequestV1::QueryTargets {
                schema: AGENT_SOURCE_ISA_CHARACTERISTIC_REQUEST_SCHEMA_V1.to_owned(),
                request_id: 5,
                collection_identity: identity.clone(),
                query: AgentSourceIsaCharacteristicTargetQueryV1::All,
                cursor: None,
                limit: 0,
            },
        );
        assert_eq!(
            error_code(invalid_limit),
            AgentSourceIsaCharacteristicErrorCodeV1::InvalidPageLimit
        );
        let malformed = format!(
            "{{\"operation\":\"discover_capabilities\",\"schema\":\"{}\",\"request_id\":7,\"extra\":true}}\n",
            AGENT_SOURCE_ISA_CHARACTERISTIC_REQUEST_SCHEMA_V1
        );
        assert_eq!(
            error_code(service.handle_line(malformed.as_bytes())),
            AgentSourceIsaCharacteristicErrorCodeV1::InvalidRequest
        );
        assert_eq!(
            error_code(handle(&mut service, &discover(7))),
            AgentSourceIsaCharacteristicErrorCodeV1::DuplicateRequestId
        );
        assert_eq!(
            error_code(service.handle_line(b"{ \"operation\":\"discover_capabilities\" }\n")),
            AgentSourceIsaCharacteristicErrorCodeV1::InvalidRequest
        );
        let well_formed_noncanonical = format!(
            "{{ \"operation\": \"discover_capabilities\", \"schema\": \"{}\", \"request_id\": 9 }}\n",
            AGENT_SOURCE_ISA_CHARACTERISTIC_REQUEST_SCHEMA_V1
        );
        assert_eq!(
            error_code(service.handle_line(well_formed_noncanonical.as_bytes())),
            AgentSourceIsaCharacteristicErrorCodeV1::InvalidRequest
        );
        assert_eq!(
            error_code(handle(&mut service, &discover(9))),
            AgentSourceIsaCharacteristicErrorCodeV1::DuplicateRequestId
        );
        let duplicate_keys = format!(
            "{{\"operation\":\"discover_capabilities\",\"operation\":\"discover_capabilities\",\"schema\":\"{}\",\"request_id\":10,\"request_id\":10}}\n",
            AGENT_SOURCE_ISA_CHARACTERISTIC_REQUEST_SCHEMA_V1
        );
        assert!(matches!(
            service.handle_line(duplicate_keys.as_bytes()),
            AgentSourceIsaCharacteristicResponseV1::Error {
                request_id: None,
                operation: None,
                error: AgentSourceIsaCharacteristicErrorCodeV1::InvalidRequest,
                ..
            }
        ));
        let max_page = handle(
            &mut service,
            &AgentSourceIsaCharacteristicRequestV1::QueryFacts {
                schema: AGENT_SOURCE_ISA_CHARACTERISTIC_REQUEST_SCHEMA_V1.to_owned(),
                request_id: 11,
                collection_identity: identity,
                query: AgentSourceIsaCharacteristicQueryV1::All,
                cursor: None,
                limit: MAX_AGENT_SOURCE_ISA_CHARACTERISTIC_PAGE_ITEMS_V1,
            },
        );
        let max_page_line =
            encode_agent_source_isa_characteristic_response_line_v1(&max_page).unwrap();
        assert!(max_page_line.len() <= MAX_AGENT_SOURCE_ISA_CHARACTERISTIC_RESPONSE_BYTES_V1);
        assert!(
            decode_agent_source_isa_characteristic_response_line_v1(max_page_line.as_bytes())
                .is_ok()
        );
        let mut oversized = vec![b' '; MAX_AGENT_SOURCE_ISA_CHARACTERISTIC_REQUEST_BYTES_V1 + 1];
        oversized[0] = b'{';
        assert_eq!(
            error_code(service.handle_line(&oversized)),
            AgentSourceIsaCharacteristicErrorCodeV1::RequestTooLarge
        );
    }

    #[test]
    fn revisions_duplicates_and_budget_state_are_monotonic() {
        let mut service = AgentSourceIsaCharacteristicServiceV1::new(collection()).unwrap();
        let first = handle(&mut service, &discover(1));
        let second = handle(&mut service, &discover(2));
        let duplicate = handle(&mut service, &discover(2));
        let revision = |response: &AgentSourceIsaCharacteristicResponseV1| match response {
            AgentSourceIsaCharacteristicResponseV1::Ok {
                response_revision, ..
            }
            | AgentSourceIsaCharacteristicResponseV1::Error {
                response_revision, ..
            } => *response_revision,
        };
        assert_eq!(
            (revision(&first), revision(&second), revision(&duplicate)),
            (1, 2, 3)
        );
        assert_eq!(
            error_code(duplicate),
            AgentSourceIsaCharacteristicErrorCodeV1::DuplicateRequestId
        );
        service.request_count = MAX_AGENT_SOURCE_ISA_CHARACTERISTIC_REQUESTS_V1;
        let exhausted = service.handle_line(b"{}");
        assert!(exhausted.is_terminal());
        assert_eq!(
            error_code(exhausted),
            AgentSourceIsaCharacteristicErrorCodeV1::RequestBudgetExhausted
        );
    }

    #[test]
    fn target_cursors_reject_cross_query_collection_predecessor_and_terminal_replay() {
        let exact = collection();
        let collection_bytes = exact.identity();
        let identity = identity_string(&exact);
        let mut service = AgentSourceIsaCharacteristicServiceV1::new(exact).unwrap();
        let first = handle(
            &mut service,
            &AgentSourceIsaCharacteristicRequestV1::QueryTargets {
                schema: AGENT_SOURCE_ISA_CHARACTERISTIC_REQUEST_SCHEMA_V1.to_owned(),
                request_id: 1,
                collection_identity: identity.clone(),
                query: AgentSourceIsaCharacteristicTargetQueryV1::All,
                cursor: None,
                limit: 1,
            },
        );
        let AgentSourceIsaCharacteristicResponseV1::Ok {
            result: AgentSourceIsaCharacteristicResultV1::TargetPage { page, .. },
            ..
        } = first
        else {
            panic!("expected first target page");
        };
        let valid_cursor = page.next_cursor.unwrap();
        let second = handle(
            &mut service,
            &AgentSourceIsaCharacteristicRequestV1::QueryTargets {
                schema: AGENT_SOURCE_ISA_CHARACTERISTIC_REQUEST_SCHEMA_V1.to_owned(),
                request_id: 2,
                collection_identity: identity.clone(),
                query: AgentSourceIsaCharacteristicTargetQueryV1::All,
                cursor: Some(valid_cursor.clone()),
                limit: 1,
            },
        );
        let AgentSourceIsaCharacteristicResponseV1::Ok {
            result: AgentSourceIsaCharacteristicResultV1::TargetPage { page, .. },
            ..
        } = second
        else {
            panic!("expected second target page");
        };
        let second_occurrence = decode_identity(&page.targets[0].occurrence_identity).unwrap();
        let query_identity = SourceIsaCharacteristicTargetQueryV1::All.identity();
        let terminal = forged_cursor(collection_bytes, query_identity, 2, second_occurrence);
        let bad_predecessor = forged_cursor(collection_bytes, query_identity, 1, id(99));
        for (request_id, cursor) in [(3, terminal), (4, bad_predecessor)] {
            let response = handle(
                &mut service,
                &AgentSourceIsaCharacteristicRequestV1::QueryTargets {
                    schema: AGENT_SOURCE_ISA_CHARACTERISTIC_REQUEST_SCHEMA_V1.to_owned(),
                    request_id,
                    collection_identity: identity.clone(),
                    query: AgentSourceIsaCharacteristicTargetQueryV1::All,
                    cursor: Some(cursor),
                    limit: 1,
                },
            );
            assert_eq!(
                error_code(response),
                AgentSourceIsaCharacteristicErrorCodeV1::InvalidCursor
            );
        }
        let cross_query = handle(
            &mut service,
            &AgentSourceIsaCharacteristicRequestV1::QueryTargets {
                schema: AGENT_SOURCE_ISA_CHARACTERISTIC_REQUEST_SCHEMA_V1.to_owned(),
                request_id: 5,
                collection_identity: identity,
                query: AgentSourceIsaCharacteristicTargetQueryV1::Kind { code: 1 },
                cursor: Some(valid_cursor.clone()),
                limit: 1,
            },
        );
        assert_eq!(
            error_code(cross_query),
            AgentSourceIsaCharacteristicErrorCodeV1::InvalidCursor
        );

        let other = collection_for(SourceIsaCharacteristicTargetProfileV1::Gfx950);
        let other_identity = identity_string(&other);
        let mut other_service = AgentSourceIsaCharacteristicServiceV1::new(other).unwrap();
        let cross_collection = handle(
            &mut other_service,
            &AgentSourceIsaCharacteristicRequestV1::QueryTargets {
                schema: AGENT_SOURCE_ISA_CHARACTERISTIC_REQUEST_SCHEMA_V1.to_owned(),
                request_id: 1,
                collection_identity: other_identity,
                query: AgentSourceIsaCharacteristicTargetQueryV1::All,
                cursor: Some(valid_cursor),
                limit: 1,
            },
        );
        assert_eq!(
            error_code(cross_collection),
            AgentSourceIsaCharacteristicErrorCodeV1::InvalidCursor
        );
    }

    #[test]
    fn jsonl_runner_writes_newline_delimited_responses_and_budget_tail() {
        let first = encode_agent_source_isa_characteristic_request_line_v1(&discover(1)).unwrap();
        let second = encode_agent_source_isa_characteristic_request_line_v1(&discover(2)).unwrap();
        let mut input = std::io::Cursor::new(format!("{first}{second}").into_bytes());
        let mut output = Vec::new();
        let mut service = AgentSourceIsaCharacteristicServiceV1::new(collection()).unwrap();
        run_agent_source_isa_characteristic_jsonl_v1(&mut service, &mut input, &mut output)
            .unwrap();
        assert!(output.ends_with(b"\n"));
        let lines = output
            .split_inclusive(|byte| *byte == b'\n')
            .collect::<Vec<_>>();
        assert_eq!(lines.len(), 2);
        for line in lines {
            assert!(decode_agent_source_isa_characteristic_response_line_v1(line).is_ok());
        }

        let mut exhausted = AgentSourceIsaCharacteristicServiceV1::new(collection()).unwrap();
        exhausted.request_count = MAX_AGENT_SOURCE_ISA_CHARACTERISTIC_REQUESTS_V1;
        let mut input = std::io::Cursor::new(b"{}\n".to_vec());
        let mut output = Vec::new();
        run_agent_source_isa_characteristic_jsonl_v1(&mut exhausted, &mut input, &mut output)
            .unwrap();
        assert!(output.ends_with(b"\n"));
        let response = decode_agent_source_isa_characteristic_response_line_v1(&output).unwrap();
        assert!(response.is_terminal());
        assert_eq!(
            error_code(response),
            AgentSourceIsaCharacteristicErrorCodeV1::RequestBudgetExhausted
        );
    }

    #[test]
    fn collection_summary_rejects_resealed_published_count_overflows() {
        let over_catalog = u64::try_from(MAX_SOURCE_ISA_CHARACTERISTIC_CATALOG_RECORDS_V1)
            .unwrap()
            .checked_add(1)
            .unwrap();
        let mut service = AgentSourceIsaCharacteristicServiceV1::new(collection()).unwrap();
        let mut response = handle(&mut service, &discover(1));
        let AgentSourceIsaCharacteristicResponseV1::Ok {
            result:
                AgentSourceIsaCharacteristicResultV1::Capabilities {
                    collection: summary,
                    ..
                },
            ..
        } = &mut response
        else {
            panic!("expected capabilities");
        };
        summary.scan.catalog_record_count = over_catalog;
        summary.scan.catalog_records_scanned = over_catalog;
        assert_resealed_response_is_rejected(response);

        let over_targets = u64::try_from(MAX_SOURCE_ISA_CHARACTERISTIC_TARGETS_V1)
            .unwrap()
            .checked_add(1)
            .unwrap();
        let mut service = AgentSourceIsaCharacteristicServiceV1::new(collection()).unwrap();
        let mut response = handle(&mut service, &discover(1));
        let AgentSourceIsaCharacteristicResponseV1::Ok {
            result:
                AgentSourceIsaCharacteristicResultV1::Capabilities {
                    collection: summary,
                    ..
                },
            ..
        } = &mut response
        else {
            panic!("expected capabilities");
        };
        summary.target_count = over_targets;
        summary.scan.classified_target_count = over_targets;
        summary.scan.target_operation_count = over_targets;
        summary.scan.target_operations_scanned = over_targets;
        summary.binding.structural_counts.operations = over_targets;
        assert_resealed_response_is_rejected(response);

        let over_target_correlations =
            u64::try_from(MAX_SOURCE_ISA_CHARACTERISTIC_TARGET_CORRELATIONS_V1)
                .unwrap()
                .checked_add(1)
                .unwrap();
        let correlation_count = over_target_correlations.checked_add(1).unwrap();
        let mut service = AgentSourceIsaCharacteristicServiceV1::new(collection()).unwrap();
        let mut response = handle(&mut service, &discover(1));
        let AgentSourceIsaCharacteristicResponseV1::Ok {
            result:
                AgentSourceIsaCharacteristicResultV1::Capabilities {
                    collection: summary,
                    ..
                },
            ..
        } = &mut response
        else {
            panic!("expected capabilities");
        };
        summary.scan.retained_target_correlation_count = over_target_correlations;
        summary.scan.correlation_count = correlation_count;
        summary.scan.catalog_record_count = correlation_count;
        summary.scan.catalog_records_scanned = correlation_count;
        assert_resealed_response_is_rejected(response);

        let over_pre_kir = u64::try_from(MAX_SOURCE_ISA_CHARACTERISTIC_PRE_KIR_ELIMINATIONS_V1)
            .unwrap()
            .checked_add(1)
            .unwrap();
        let correlation_count = over_pre_kir.checked_add(2).unwrap();
        let mut service = AgentSourceIsaCharacteristicServiceV1::new(collection()).unwrap();
        let mut response = handle(&mut service, &discover(1));
        let AgentSourceIsaCharacteristicResponseV1::Ok {
            result:
                AgentSourceIsaCharacteristicResultV1::Capabilities {
                    collection: summary,
                    ..
                },
            ..
        } = &mut response
        else {
            panic!("expected capabilities");
        };
        summary.pre_kir_elimination_count = over_pre_kir;
        summary.scan.pre_kir_elimination_count = over_pre_kir;
        summary.scan.correlation_count = correlation_count;
        summary.scan.catalog_record_count = correlation_count;
        summary.scan.catalog_records_scanned = correlation_count;
        assert_resealed_response_is_rejected(response);
    }

    #[test]
    fn target_and_fact_items_reject_resealed_published_count_overflows() {
        let exact = collection();
        let collection_identity = exact.identity();
        let identity = identity_string(&exact);
        let mut service = AgentSourceIsaCharacteristicServiceV1::new(exact).unwrap();
        let mut target_response = handle(
            &mut service,
            &AgentSourceIsaCharacteristicRequestV1::QueryTargets {
                schema: AGENT_SOURCE_ISA_CHARACTERISTIC_REQUEST_SCHEMA_V1.to_owned(),
                request_id: 1,
                collection_identity: identity.clone(),
                query: AgentSourceIsaCharacteristicTargetQueryV1::All,
                cursor: None,
                limit: 1,
            },
        );
        let AgentSourceIsaCharacteristicResponseV1::Ok {
            result: AgentSourceIsaCharacteristicResultV1::TargetPage { page, .. },
            ..
        } = &mut target_response
        else {
            panic!("expected target page");
        };
        let target = &mut page.targets[0];
        target.correlation_count =
            u64::try_from(MAX_SOURCE_ISA_CHARACTERISTIC_CORRELATIONS_PER_TARGET_V1)
                .unwrap()
                .checked_add(1)
                .unwrap();
        let occurrence = source_isa_characteristic_target_match_identity_v1(
            collection_identity,
            decode_identity(&target.characteristic_identity).unwrap(),
            decode_kind(target.kind.code).unwrap(),
            decode_kir(target.target_kir).unwrap(),
            target.correlation_count,
        )
        .unwrap();
        target.occurrence_identity = hex_bytes(occurrence).unwrap();
        assert_resealed_response_is_rejected(target_response);

        let exact = collection();
        let identity = identity_string(&exact);
        let mut service = AgentSourceIsaCharacteristicServiceV1::new(exact).unwrap();
        let mut fact_response = handle(
            &mut service,
            &AgentSourceIsaCharacteristicRequestV1::QueryFacts {
                schema: AGENT_SOURCE_ISA_CHARACTERISTIC_REQUEST_SCHEMA_V1.to_owned(),
                request_id: 1,
                collection_identity: identity,
                query: AgentSourceIsaCharacteristicQueryV1::All,
                cursor: None,
                limit: 1,
            },
        );
        let AgentSourceIsaCharacteristicResponseV1::Ok {
            result: AgentSourceIsaCharacteristicResultV1::FactPage { page, .. },
            ..
        } = &mut fact_response
        else {
            panic!("expected fact page");
        };
        let AgentSourceIsaCharacteristicFactOutcomeV1::TargetCorrelation { correlation } =
            &mut page.facts[0].outcome
        else {
            panic!("expected target correlation");
        };
        correlation.interval_count = u64::try_from(MAX_SOURCE_ISA_CHARACTERISTIC_INTERVALS_V1)
            .unwrap()
            .checked_add(1)
            .unwrap();
        assert_resealed_response_is_rejected(fact_response);
    }

    #[test]
    fn response_pages_reject_resealed_total_count_overflows() {
        let exact = collection();
        let identity = identity_string(&exact);
        let mut service = AgentSourceIsaCharacteristicServiceV1::new(exact).unwrap();
        let mut target_response = handle(
            &mut service,
            &AgentSourceIsaCharacteristicRequestV1::QueryTargets {
                schema: AGENT_SOURCE_ISA_CHARACTERISTIC_REQUEST_SCHEMA_V1.to_owned(),
                request_id: 1,
                collection_identity: identity.clone(),
                query: AgentSourceIsaCharacteristicTargetQueryV1::All,
                cursor: None,
                limit: 1,
            },
        );
        let AgentSourceIsaCharacteristicResponseV1::Ok {
            result: AgentSourceIsaCharacteristicResultV1::TargetPage { page, .. },
            ..
        } = &mut target_response
        else {
            panic!("expected target page");
        };
        page.total_matches = page.total_matches.checked_add(1).unwrap();
        assert_resealed_response_is_rejected(target_response);

        let mut fact_response = handle(
            &mut service,
            &AgentSourceIsaCharacteristicRequestV1::QueryFacts {
                schema: AGENT_SOURCE_ISA_CHARACTERISTIC_REQUEST_SCHEMA_V1.to_owned(),
                request_id: 2,
                collection_identity: identity.clone(),
                query: AgentSourceIsaCharacteristicQueryV1::All,
                cursor: None,
                limit: 1,
            },
        );
        let occurrence = {
            let AgentSourceIsaCharacteristicResponseV1::Ok {
                result: AgentSourceIsaCharacteristicResultV1::FactPage { page, .. },
                ..
            } = &mut fact_response
            else {
                panic!("expected fact page");
            };
            page.total_matches = page.total_matches.checked_add(1).unwrap();
            page.facts[0].occurrence_identity.clone()
        };
        assert_resealed_response_is_rejected(fact_response);

        let mut interval_response = handle(
            &mut service,
            &AgentSourceIsaCharacteristicRequestV1::QueryIntervals {
                schema: AGENT_SOURCE_ISA_CHARACTERISTIC_REQUEST_SCHEMA_V1.to_owned(),
                request_id: 3,
                collection_identity: identity,
                occurrence_identity: occurrence,
                cursor: None,
                limit: 1,
            },
        );
        let AgentSourceIsaCharacteristicResponseV1::Ok {
            result: AgentSourceIsaCharacteristicResultV1::IntervalPage { page, .. },
            ..
        } = &mut interval_response
        else {
            panic!("expected interval page");
        };
        page.total_intervals = u64::try_from(MAX_SOURCE_ISA_CHARACTERISTIC_INTERVALS_V1)
            .unwrap()
            .checked_add(1)
            .unwrap();
        assert_resealed_response_is_rejected(interval_response);
    }

    #[test]
    fn standalone_response_decoder_rejects_target_projection_substitution() {
        let exact = collection();
        let identity = identity_string(&exact);
        let mut service = AgentSourceIsaCharacteristicServiceV1::new(exact).unwrap();
        let mut target_response = handle(
            &mut service,
            &AgentSourceIsaCharacteristicRequestV1::QueryTargets {
                schema: AGENT_SOURCE_ISA_CHARACTERISTIC_REQUEST_SCHEMA_V1.to_owned(),
                request_id: 1,
                collection_identity: identity.clone(),
                query: AgentSourceIsaCharacteristicTargetQueryV1::All,
                cursor: None,
                limit: 2,
            },
        );
        let AgentSourceIsaCharacteristicResponseV1::Ok {
            result: AgentSourceIsaCharacteristicResultV1::TargetPage { page, .. },
            ..
        } = &mut target_response
        else {
            panic!("expected target page");
        };
        page.targets[0].kind = project_kind(SourceIsaCharacteristicKindV1::GlobalStore {
            form: SourceIsaCharacteristicMemoryFormV1::Guarded,
        });
        let encoded =
            encode_agent_source_isa_characteristic_response_line_v1(&target_response).unwrap();
        assert_eq!(
            decode_agent_source_isa_characteristic_response_line_v1(encoded.as_bytes()),
            Err(AgentSourceIsaCharacteristicDecodeErrorV1::InvalidCanonicalJsonl)
        );
    }

    #[test]
    fn standalone_response_decoder_rejects_duplicate_fact_occurrence() {
        let exact = collection();
        let identity = identity_string(&exact);
        let mut service = AgentSourceIsaCharacteristicServiceV1::new(exact).unwrap();
        let mut fact_response = handle(
            &mut service,
            &AgentSourceIsaCharacteristicRequestV1::QueryFacts {
                schema: AGENT_SOURCE_ISA_CHARACTERISTIC_REQUEST_SCHEMA_V1.to_owned(),
                request_id: 1,
                collection_identity: identity,
                query: AgentSourceIsaCharacteristicQueryV1::All,
                cursor: None,
                limit: 2,
            },
        );
        let AgentSourceIsaCharacteristicResponseV1::Ok {
            result: AgentSourceIsaCharacteristicResultV1::FactPage { page, .. },
            ..
        } = &mut fact_response
        else {
            panic!("expected fact page");
        };
        page.facts[1] = page.facts[0].clone();
        let encoded =
            encode_agent_source_isa_characteristic_response_line_v1(&fact_response).unwrap();
        assert_eq!(
            decode_agent_source_isa_characteristic_response_line_v1(encoded.as_bytes()),
            Err(AgentSourceIsaCharacteristicDecodeErrorV1::InvalidCanonicalJsonl)
        );
    }

    #[test]
    fn authority_elevation_and_duplicate_target_are_independently_rejected() {
        let exact = collection();
        let identity = identity_string(&exact);
        let mut service = AgentSourceIsaCharacteristicServiceV1::new(exact).unwrap();
        let mut elevated = handle(&mut service, &discover(1));
        let AgentSourceIsaCharacteristicResponseV1::Ok {
            result: AgentSourceIsaCharacteristicResultV1::Capabilities { authority, .. },
            ..
        } = &mut elevated
        else {
            panic!("expected capabilities");
        };
        authority.runtime_authority = true;
        let encoded = encode_agent_source_isa_characteristic_response_line_v1(&elevated).unwrap();
        assert!(
            decode_agent_source_isa_characteristic_response_line_v1(encoded.as_bytes()).is_err()
        );

        let mut duplicated = handle(
            &mut service,
            &AgentSourceIsaCharacteristicRequestV1::QueryTargets {
                schema: AGENT_SOURCE_ISA_CHARACTERISTIC_REQUEST_SCHEMA_V1.to_owned(),
                request_id: 2,
                collection_identity: identity,
                query: AgentSourceIsaCharacteristicTargetQueryV1::All,
                cursor: None,
                limit: 2,
            },
        );
        let AgentSourceIsaCharacteristicResponseV1::Ok {
            result: AgentSourceIsaCharacteristicResultV1::TargetPage { page, .. },
            ..
        } = &mut duplicated
        else {
            panic!("expected target page");
        };
        page.targets[1] = page.targets[0].clone();
        let encoded = encode_agent_source_isa_characteristic_response_line_v1(&duplicated).unwrap();
        assert!(
            decode_agent_source_isa_characteristic_response_line_v1(encoded.as_bytes()).is_err()
        );
    }

    #[test]
    fn source_shape_cursor_and_unknown_response_fields_are_rejected() {
        let exact = collection();
        let identity = identity_string(&exact);
        let mut service = AgentSourceIsaCharacteristicServiceV1::new(exact).unwrap();
        let mut fact_response = handle(
            &mut service,
            &AgentSourceIsaCharacteristicRequestV1::QueryFacts {
                schema: AGENT_SOURCE_ISA_CHARACTERISTIC_REQUEST_SCHEMA_V1.to_owned(),
                request_id: 1,
                collection_identity: identity,
                query: AgentSourceIsaCharacteristicQueryV1::All,
                cursor: None,
                limit: 1,
            },
        );
        let AgentSourceIsaCharacteristicResponseV1::Ok {
            result: AgentSourceIsaCharacteristicResultV1::FactPage { page, .. },
            ..
        } = &mut fact_response
        else {
            panic!("expected fact page");
        };
        let AgentSourceIsaCharacteristicFactOutcomeV1::TargetCorrelation { correlation } =
            &mut page.facts[0].outcome
        else {
            panic!("expected target correlation");
        };
        correlation.source = None;
        let encoded =
            encode_agent_source_isa_characteristic_response_line_v1(&fact_response).unwrap();
        assert!(
            decode_agent_source_isa_characteristic_response_line_v1(encoded.as_bytes()).is_err()
        );

        let mut capabilities = handle(&mut service, &discover(2));
        let AgentSourceIsaCharacteristicResponseV1::Ok {
            result: AgentSourceIsaCharacteristicResultV1::Capabilities { collection, .. },
            ..
        } = &mut capabilities
        else {
            panic!("expected capabilities");
        };
        collection.format = "x".repeat(MAX_AGENT_SOURCE_ISA_CHARACTERISTIC_RESPONSE_BYTES_V1);
        let mut output = Vec::new();
        write_response(&mut output, capabilities).unwrap();
        let fallback = decode_agent_source_isa_characteristic_response_line_v1(&output).unwrap();
        assert_eq!(
            error_code(fallback),
            AgentSourceIsaCharacteristicErrorCodeV1::ResponseTooLarge
        );

        let normal = handle(&mut service, &discover(3));
        let mut encoded = encode_agent_source_isa_characteristic_response_line_v1(&normal).unwrap();
        encoded.pop();
        encoded.pop();
        encoded.push_str(",\"unknown\":true}\n");
        assert!(
            decode_agent_source_isa_characteristic_response_line_v1(encoded.as_bytes()).is_err()
        );
    }
}
