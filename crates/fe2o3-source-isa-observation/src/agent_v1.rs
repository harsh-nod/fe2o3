//! Bounded, authority-free source/ISA inspection shared by humans and agents.

use std::collections::BTreeSet;
use std::io::{BufRead, Write};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::wire_v1::{
    AdmittedSourceIsaObservationV1, MAX_SOURCE_ISA_OBSERVATION_COLLECTION_BYTES_V1,
    MAX_SOURCE_ISA_OBSERVATION_COLLECTION_HEX_BYTES_V1, SourceIsaObservationCollectionV1,
    SourceIsaObservationErrorCodeV1, SourceIsaObservationKirVersionV1,
    SourceIsaObservationOutcomeV1, SourceIsaObservationTargetProfileV1,
    SourceIsaObservationTransportFailureV1, SourceIsaObservationUnavailableReasonV1,
};

pub const AGENT_SOURCE_ISA_REQUEST_SCHEMA_V1: &str = "fe2o3-agent-source-isa-request-v1";
pub const AGENT_SOURCE_ISA_RESPONSE_SCHEMA_V1: &str = "fe2o3-agent-source-isa-response-v1";
pub const MAX_AGENT_SOURCE_ISA_REQUEST_BYTES_V1: usize = 2 * 1024 * 1024;
pub const MAX_AGENT_SOURCE_ISA_RESPONSE_BYTES_V1: usize = 2 * 1024 * 1024;
pub const MAX_AGENT_SOURCE_ISA_PAGE_ITEMS_V1: u16 = 64;
pub const MAX_AGENT_SOURCE_ISA_UNITS_V1: usize = 1024;
pub const MAX_AGENT_SOURCE_ISA_REQUESTS_V1: u32 = 4_096;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentSourceIsaOperationV1 {
    DiscoverCapabilities,
    InspectSourceIsaCollection,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(tag = "operation", rename_all = "snake_case", deny_unknown_fields)]
pub enum AgentSourceIsaRequestV1 {
    DiscoverCapabilities {
        schema: String,
        request_id: u64,
    },
    InspectSourceIsaCollection {
        schema: String,
        request_id: u64,
        collection_hex: String,
        page: AgentSourceIsaPageRequestV1,
    },
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct AgentSourceIsaPageRequestV1 {
    pub after: Option<String>,
    pub limit: u16,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub struct SourceIsaAuthorityViewV1 {
    pub observation_only: bool,
    pub compiler_authority: bool,
    pub proof_authority: bool,
    pub artifact_authority: bool,
    pub runtime_authority: bool,
    pub hardware_execution_observed: bool,
    pub complete_machine_coverage_proved: bool,
    pub semantic_refinement_proved: bool,
}

impl SourceIsaAuthorityViewV1 {
    const OBSERVATION_ONLY: Self = Self {
        observation_only: true,
        compiler_authority: false,
        proof_authority: false,
        artifact_authority: false,
        runtime_authority: false,
        hardware_execution_observed: false,
        complete_machine_coverage_proved: false,
        semantic_refinement_proved: false,
    };
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct SourceIsaCollectionSummaryV1 {
    pub collection_evidence: SourceIsaEvidenceIdentityV1,
    pub format: &'static str,
    pub configuration_identity: String,
    pub session: String,
    pub frame_count: u16,
    pub missing_unit_count: u16,
    pub transport_failure: Option<SourceIsaTypedCodeV1>,
    pub completeness: SourceIsaCollectionCompletenessV1,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct SourceIsaEvidenceIdentityV1 {
    pub scheme: &'static str,
    pub digest: String,
    pub canonical_byte_len: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum SourceIsaCollectionCompletenessV1 {
    Complete,
    Incomplete {
        missing_unit_count: u16,
        transport_failure: Option<SourceIsaTypedCodeV1>,
    },
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct SourceIsaTypedCodeV1 {
    pub code: u16,
    pub label: &'static str,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct SourceIsaFrameViewV1 {
    pub frame_evidence: SourceIsaEvidenceIdentityV1,
    pub unit_identity: String,
    pub attempt: SourceIsaBuildAttemptViewV1,
    pub finalization_identity: String,
    pub outcome: SourceIsaFrameOutcomeViewV1,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct SourceIsaBuildAttemptViewV1 {
    pub generation: u64,
    pub session: String,
    pub invocation_identity: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(tag = "outcome", rename_all = "snake_case")]
pub enum SourceIsaFrameOutcomeViewV1 {
    Admitted { evidence: SourceIsaAdmittedViewV1 },
    Unavailable { reason: SourceIsaTypedCodeV1 },
    Error { error: SourceIsaTypedCodeV1 },
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct SourceIsaAdmittedViewV1 {
    pub correlation_identity: String,
    pub artifact: SourceIsaContentViewV1,
    pub structural: SourceIsaStructuralViewV1,
    pub records: SourceIsaRecordCountsViewV1,
    pub queries: SourceIsaQueryCountsViewV1,
    pub round_trip: Option<SourceIsaRoundTripViewV1>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct SourceIsaContentViewV1 {
    pub sha256: String,
    pub byte_len: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct SourceIsaStructuralViewV1 {
    pub identity: String,
    pub target: SourceIsaTargetViewV1,
    pub kir_version: u16,
    pub functions: u64,
    pub defined_bodies: u64,
    pub blocks: u64,
    pub operations: u64,
    pub neutral_kir: SourceIsaContentViewV1,
    pub target_kir: SourceIsaContentViewV1,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct SourceIsaTargetViewV1 {
    pub architecture: &'static str,
    pub features: [&'static str; 2],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub struct SourceIsaRecordCountsViewV1 {
    pub total: u64,
    pub source_anchored: u64,
    pub eliminated: u64,
    pub no_source: u64,
    pub source_anchored_without_isa: u64,
    pub isa_references: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub struct SourceIsaQueryCountsViewV1 {
    pub distinct_source_nodes: u64,
    pub distinct_source_spans: u64,
    pub distinct_isa_points: u64,
    pub max_source_node_cardinality: u64,
    pub max_source_span_cardinality: u64,
    pub max_exact_pc_cardinality: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct SourceIsaRoundTripViewV1 {
    pub source_node_identity: String,
    pub source_span: SourceIsaSourceSpanViewV1,
    pub isa_point: SourceIsaPointViewV1,
    pub source_node_query_matches: u64,
    pub source_span_query_matches: u64,
    pub isa_point_query_matches: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct SourceIsaSourceSpanViewV1 {
    pub file_identity: String,
    pub byte_start: u64,
    pub byte_end: u64,
    pub line: u32,
    pub column: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub struct SourceIsaPointViewV1 {
    pub kernel_ordinal: u64,
    pub symbol_relative_pc: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(tag = "unit_state", rename_all = "snake_case")]
pub enum SourceIsaUnitViewV1 {
    Observed {
        frame: SourceIsaFrameViewV1,
    },
    Missing {
        missing_evidence_id: String,
        unit_identity: String,
    },
}

impl SourceIsaUnitViewV1 {
    fn unit_identity(&self) -> &str {
        match self {
            Self::Observed { frame } => &frame.unit_identity,
            Self::Missing { unit_identity, .. } => unit_identity,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct SourceIsaPageV1 {
    pub after: Option<String>,
    pub next_after: Option<String>,
    pub page_exhausted: bool,
    pub items: Vec<SourceIsaUnitViewV1>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceIsaInspectionV1 {
    pub authority: SourceIsaAuthorityViewV1,
    pub collection: SourceIsaCollectionSummaryV1,
    admitted: SourceIsaObservationCollectionV1,
    collection_identity: [u8; 32],
    canonical_byte_len: u64,
}

impl SourceIsaInspectionV1 {
    pub fn decode_canonical(encoded: &[u8]) -> Result<Self, String> {
        if encoded.len() > MAX_SOURCE_ISA_OBSERVATION_COLLECTION_BYTES_V1 {
            return Err("source/ISA collection exceeds its binary byte bound".to_owned());
        }
        let collection = SourceIsaObservationCollectionV1::decode_canonical(encoded)?;
        let collection_identity = encoded
            .get(encoded.len().saturating_sub(32)..)
            .and_then(|identity| <[u8; 32]>::try_from(identity).ok())
            .ok_or_else(|| "source/ISA collection identity is absent".to_owned())?;
        Self::from_admitted_collection(
            collection,
            collection_identity,
            u64::try_from(encoded.len())
                .map_err(|_| "source/ISA collection byte length does not fit V1".to_owned())?,
        )
    }

    fn from_admitted_collection(
        collection: SourceIsaObservationCollectionV1,
        collection_identity: [u8; 32],
        canonical_byte_len: u64,
    ) -> Result<Self, String> {
        let unit_count = collection
            .frames()
            .len()
            .checked_add(collection.missing_units().len())
            .ok_or_else(|| "source/ISA inspection unit count overflowed".to_owned())?;
        if unit_count > MAX_AGENT_SOURCE_ISA_UNITS_V1 {
            return Err("source/ISA inspection exceeds its unit bound".to_owned());
        }
        let transport_failure = collection.failure().map(transport_failure);
        let frame_count = u16::try_from(collection.frames().len())
            .map_err(|_| "source/ISA frame count does not fit V1".to_owned())?;
        let missing_unit_count = u16::try_from(collection.missing_units().len())
            .map_err(|_| "source/ISA missing count does not fit V1".to_owned())?;
        let completeness = if missing_unit_count == 0 && transport_failure.is_none() {
            SourceIsaCollectionCompletenessV1::Complete
        } else {
            SourceIsaCollectionCompletenessV1::Incomplete {
                missing_unit_count,
                transport_failure: transport_failure.clone(),
            }
        };
        Ok(Self {
            authority: SourceIsaAuthorityViewV1::OBSERVATION_ONLY,
            collection: SourceIsaCollectionSummaryV1 {
                collection_evidence: SourceIsaEvidenceIdentityV1 {
                    scheme: "sha256",
                    digest: hex_bytes(&collection_identity),
                    canonical_byte_len,
                },
                format: "fe2o3-source-isa-observation-collection-v1",
                configuration_identity: hex_bytes(&collection.config_identity()),
                session: collection.session().to_string(),
                frame_count,
                missing_unit_count,
                transport_failure,
                completeness,
            },
            admitted: collection,
            collection_identity,
            canonical_byte_len,
        })
    }

    pub fn frames(&self) -> impl ExactSizeIterator<Item = SourceIsaFrameViewV1> + '_ {
        self.admitted.frames().map(project_frame)
    }

    pub fn missing_units(&self) -> impl ExactSizeIterator<Item = String> + '_ {
        self.admitted
            .missing_units()
            .iter()
            .map(|unit| hex_bytes(unit))
    }

    fn merged_units(&self) -> impl Iterator<Item = RawSourceIsaUnitV1<'_>> {
        let mut frames = self.admitted.frames().peekable();
        let mut missing = self.admitted.missing_units().iter().peekable();
        std::iter::from_fn(move || match (frames.peek(), missing.peek()) {
            (Some(frame), Some(unit)) if frame.context().unit() < **unit => {
                frames.next().map(RawSourceIsaUnitV1::Observed)
            }
            (Some(_), Some(_)) => missing.next().map(RawSourceIsaUnitV1::Missing),
            (Some(_), None) => frames.next().map(RawSourceIsaUnitV1::Observed),
            (None, Some(_)) => missing.next().map(RawSourceIsaUnitV1::Missing),
            (None, None) => None,
        })
    }

    pub fn page(
        &self,
        request: &AgentSourceIsaPageRequestV1,
    ) -> Result<SourceIsaPageV1, AgentSourceIsaErrorCodeV1> {
        if request.limit == 0 || request.limit > MAX_AGENT_SOURCE_ISA_PAGE_ITEMS_V1 {
            return Err(AgentSourceIsaErrorCodeV1::InvalidPage);
        }
        let unit_count = usize::from(self.collection.frame_count)
            .checked_add(usize::from(self.collection.missing_unit_count))
            .ok_or(AgentSourceIsaErrorCodeV1::InvalidPage)?;
        let start = match &request.after {
            None => 0,
            Some(after) => parse_page_cursor(
                after,
                &self.collection_identity,
                self.canonical_byte_len,
                unit_count,
                self.merged_units()
                    .nth(cursor_position(after)?.saturating_sub(1))
                    .ok_or(AgentSourceIsaErrorCodeV1::InvalidPage)?
                    .unit_identity(),
            )?,
        };
        let end = start
            .checked_add(usize::from(request.limit))
            .unwrap_or(usize::MAX)
            .min(unit_count);
        let items = self
            .merged_units()
            .skip(start)
            .take(end.saturating_sub(start))
            .map(|unit| unit.project(&self.collection_identity, self.canonical_byte_len))
            .collect::<Vec<_>>();
        let page_exhausted = end == unit_count;
        let next_after = if page_exhausted {
            None
        } else {
            Some(page_cursor(
                &self.collection_identity,
                self.canonical_byte_len,
                unit_count,
                end,
                items
                    .last()
                    .expect("a nonexhausted nonzero page has one item")
                    .unit_identity(),
            ))
        };
        Ok(SourceIsaPageV1 {
            after: request.after.clone(),
            next_after,
            page_exhausted,
            items,
        })
    }
}

enum RawSourceIsaUnitV1<'collection> {
    Observed(&'collection crate::wire_v1::SourceIsaObservationFrameV1),
    Missing(&'collection [u8; 32]),
}

impl RawSourceIsaUnitV1<'_> {
    fn unit_identity(&self) -> String {
        match self {
            Self::Observed(frame) => hex_bytes(&frame.context().unit()),
            Self::Missing(unit) => hex_bytes(*unit),
        }
    }

    fn project(
        self,
        collection_identity: &[u8; 32],
        canonical_byte_len: u64,
    ) -> SourceIsaUnitViewV1 {
        match self {
            Self::Observed(frame) => SourceIsaUnitViewV1::Observed {
                frame: project_frame(frame),
            },
            Self::Missing(unit) => {
                let unit_identity = hex_bytes(unit);
                SourceIsaUnitViewV1::Missing {
                    missing_evidence_id: missing_evidence_id(
                        collection_identity,
                        canonical_byte_len,
                        &unit_identity,
                    ),
                    unit_identity,
                }
            }
        }
    }
}

fn project_frame(frame: &crate::wire_v1::SourceIsaObservationFrameV1) -> SourceIsaFrameViewV1 {
    let context = frame.context();
    SourceIsaFrameViewV1 {
        frame_evidence: SourceIsaEvidenceIdentityV1 {
            scheme: "sha256",
            digest: hex_bytes(&frame.identity()),
            canonical_byte_len: u64::try_from(
                crate::wire_v1::SOURCE_ISA_OBSERVATION_FRAME_BYTES_V1,
            )
            .expect("source/ISA frame byte bound fits u64"),
        },
        unit_identity: hex_bytes(&context.unit()),
        attempt: SourceIsaBuildAttemptViewV1 {
            generation: context.attempt().generation(),
            session: context.attempt().session().to_string(),
            invocation_identity: context.attempt().invocation().to_string(),
        },
        finalization_identity: hex_bytes(&context.finalization()),
        outcome: match frame.outcome() {
            SourceIsaObservationOutcomeV1::Admitted(admitted) => {
                SourceIsaFrameOutcomeViewV1::Admitted {
                    evidence: project_admitted(admitted),
                }
            }
            SourceIsaObservationOutcomeV1::Unavailable(reason) => {
                SourceIsaFrameOutcomeViewV1::Unavailable {
                    reason: SourceIsaTypedCodeV1 {
                        code: reason as u16,
                        label: unavailable_reason(reason),
                    },
                }
            }
            SourceIsaObservationOutcomeV1::Error(error) => SourceIsaFrameOutcomeViewV1::Error {
                error: SourceIsaTypedCodeV1 {
                    code: error as u16,
                    label: observation_error(error),
                },
            },
        },
    }
}

fn project_admitted(admitted: AdmittedSourceIsaObservationV1) -> SourceIsaAdmittedViewV1 {
    let artifact = admitted.artifact();
    let structural = admitted.structural();
    let structural_counts = structural.counts();
    let records = admitted.counts().records();
    let queries = admitted.counts().queries();
    SourceIsaAdmittedViewV1 {
        correlation_identity: hex_bytes(&admitted.correlation()),
        artifact: SourceIsaContentViewV1 {
            sha256: hex_bytes(&artifact.sha256()),
            byte_len: artifact.byte_len(),
        },
        structural: SourceIsaStructuralViewV1 {
            identity: hex_bytes(&structural.identity()),
            target: source_isa_target(structural.target_profile()),
            kir_version: source_isa_kir_version(structural.kir_version()),
            functions: structural_counts.functions,
            defined_bodies: structural_counts.defined_bodies,
            blocks: structural_counts.blocks,
            operations: structural_counts.operations,
            neutral_kir: SourceIsaContentViewV1 {
                sha256: hex_bytes(&structural.neutral_kir().sha256()),
                byte_len: structural.neutral_kir().byte_len(),
            },
            target_kir: SourceIsaContentViewV1 {
                sha256: hex_bytes(&structural.target_kir().sha256()),
                byte_len: structural.target_kir().byte_len(),
            },
        },
        records: SourceIsaRecordCountsViewV1 {
            total: records.records,
            source_anchored: records.source_anchored,
            eliminated: records.eliminated,
            no_source: records.no_source,
            source_anchored_without_isa: records.source_anchored_without_isa,
            isa_references: records.isa_references,
        },
        queries: SourceIsaQueryCountsViewV1 {
            distinct_source_nodes: queries.distinct_source_nodes,
            distinct_source_spans: queries.distinct_source_spans,
            distinct_isa_points: queries.distinct_isa_points,
            max_source_node_cardinality: queries.max_source_node_cardinality,
            max_source_span_cardinality: queries.max_source_span_cardinality,
            max_exact_pc_cardinality: queries.max_exact_pc_cardinality,
        },
        round_trip: admitted.round_trip_witness().map(|witness| {
            let span = witness.source_span();
            let isa = witness.isa_point();
            SourceIsaRoundTripViewV1 {
                source_node_identity: hex_bytes(&witness.source_node_identity()),
                source_span: SourceIsaSourceSpanViewV1 {
                    file_identity: hex_bytes(&span.file_identity()),
                    byte_start: span.byte_start(),
                    byte_end: span.byte_end(),
                    line: span.line(),
                    column: span.column(),
                },
                isa_point: SourceIsaPointViewV1 {
                    kernel_ordinal: isa.kernel_ordinal(),
                    symbol_relative_pc: isa.symbol_relative_pc(),
                },
                source_node_query_matches: witness.source_node_query_matches(),
                source_span_query_matches: witness.source_span_query_matches(),
                isa_point_query_matches: witness.isa_point_query_matches(),
            }
        }),
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentSourceIsaErrorCodeV1 {
    InvalidRequest,
    RequestTooLarge,
    InvalidRequestId,
    DuplicateRequestId,
    RequestBudgetExhausted,
    SchemaMismatch,
    CollectionHexTooLarge,
    InvalidLowercaseHex,
    InvalidCollection,
    InvalidPage,
    ResponseTooLarge,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub struct AgentSourceIsaLimitsV1 {
    pub max_request_bytes: usize,
    pub max_response_bytes: usize,
    pub max_collection_binary_bytes: usize,
    pub max_collection_hex_bytes: usize,
    pub max_units: usize,
    pub max_page_items: u16,
    pub max_requests: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentSourceIsaCapabilityStateV1 {
    Available,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub struct AgentSourceIsaCapabilityV1 {
    pub operation: AgentSourceIsaOperationV1,
    pub state: AgentSourceIsaCapabilityStateV1,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(tag = "result", rename_all = "snake_case")]
pub enum AgentSourceIsaResultV1 {
    Capabilities {
        authority: SourceIsaAuthorityViewV1,
        limits: AgentSourceIsaLimitsV1,
        capabilities: Vec<AgentSourceIsaCapabilityV1>,
    },
    CollectionPage {
        authority: SourceIsaAuthorityViewV1,
        collection: SourceIsaCollectionSummaryV1,
        page: SourceIsaPageV1,
    },
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum AgentSourceIsaResponseV1 {
    Ok {
        schema: &'static str,
        request_id: u64,
        response_revision: u64,
        operation: AgentSourceIsaOperationV1,
        result: AgentSourceIsaResultV1,
    },
    Error {
        schema: &'static str,
        request_id: Option<u64>,
        response_revision: u64,
        operation: Option<AgentSourceIsaOperationV1>,
        error: AgentSourceIsaErrorCodeV1,
        terminal: bool,
    },
}

impl AgentSourceIsaResponseV1 {
    fn error(
        request_id: Option<u64>,
        response_revision: u64,
        operation: Option<AgentSourceIsaOperationV1>,
        error: AgentSourceIsaErrorCodeV1,
        terminal: bool,
    ) -> Self {
        Self::Error {
            schema: AGENT_SOURCE_ISA_RESPONSE_SCHEMA_V1,
            request_id,
            response_revision,
            operation,
            error,
            terminal,
        }
    }
}

pub fn execute_agent_source_isa_request_line_v1(line: &[u8]) -> String {
    let mut service = AgentSourceIsaServiceV1::new();
    encode_response(service.handle_line(line))
}

pub struct AgentSourceIsaServiceV1 {
    request_ids: BTreeSet<u64>,
    request_count: u32,
    response_revision: u64,
    terminal: bool,
}

impl Default for AgentSourceIsaServiceV1 {
    fn default() -> Self {
        Self::new()
    }
}

impl AgentSourceIsaServiceV1 {
    pub fn new() -> Self {
        Self {
            request_ids: BTreeSet::new(),
            request_count: 0,
            response_revision: 0,
            terminal: false,
        }
    }

    pub fn handle_line(&mut self, line: &[u8]) -> AgentSourceIsaResponseV1 {
        self.response_revision = self.response_revision.saturating_add(1);
        if self.terminal || self.request_count >= MAX_AGENT_SOURCE_ISA_REQUESTS_V1 {
            self.terminal = true;
            return AgentSourceIsaResponseV1::error(
                None,
                self.response_revision,
                None,
                AgentSourceIsaErrorCodeV1::RequestBudgetExhausted,
                true,
            );
        }
        if line.len() > MAX_AGENT_SOURCE_ISA_REQUEST_BYTES_V1 {
            self.terminal = true;
            return AgentSourceIsaResponseV1::error(
                None,
                self.response_revision,
                None,
                AgentSourceIsaErrorCodeV1::RequestTooLarge,
                true,
            );
        }
        self.request_count += 1;
        let request = match decode_request(line) {
            Ok(request) => request,
            Err(error) => {
                return AgentSourceIsaResponseV1::error(
                    None,
                    self.response_revision,
                    None,
                    error,
                    false,
                );
            }
        };
        let (request_id, operation) = request.identity();
        if request_id == 0 {
            return AgentSourceIsaResponseV1::error(
                None,
                self.response_revision,
                Some(operation),
                AgentSourceIsaErrorCodeV1::InvalidRequestId,
                false,
            );
        }
        if !self.request_ids.insert(request_id) {
            return AgentSourceIsaResponseV1::error(
                Some(request_id),
                self.response_revision,
                Some(operation),
                AgentSourceIsaErrorCodeV1::DuplicateRequestId,
                false,
            );
        }
        execute_request(request, self.response_revision)
    }
}

impl AgentSourceIsaRequestV1 {
    fn identity(&self) -> (u64, AgentSourceIsaOperationV1) {
        match self {
            Self::DiscoverCapabilities { request_id, .. } => {
                (*request_id, AgentSourceIsaOperationV1::DiscoverCapabilities)
            }
            Self::InspectSourceIsaCollection { request_id, .. } => (
                *request_id,
                AgentSourceIsaOperationV1::InspectSourceIsaCollection,
            ),
        }
    }
}

pub fn run_agent_source_isa_jsonl_v1<R: BufRead, W: Write>(
    input: &mut R,
    output: &mut W,
) -> Result<(), std::io::Error> {
    let mut service = AgentSourceIsaServiceV1::new();
    loop {
        let Some(line) = read_request_line(input)? else {
            return Ok(());
        };
        let terminal_input = line.len() > MAX_AGENT_SOURCE_ISA_REQUEST_BYTES_V1;
        let response = service.handle_line(&line);
        write_response(output, response)?;
        if terminal_input {
            return Ok(());
        }
    }
}

pub fn inspect_source_isa_agent_json_v1(encoded: &[u8]) -> Result<String, String> {
    let inspection = SourceIsaInspectionV1::decode_canonical(encoded)?;
    let page = inspection
        .page(&AgentSourceIsaPageRequestV1 {
            after: None,
            limit: MAX_AGENT_SOURCE_ISA_PAGE_ITEMS_V1,
        })
        .map_err(|_| "source/ISA first page is invalid".to_owned())?;
    Ok(encode_response(AgentSourceIsaResponseV1::Ok {
        schema: AGENT_SOURCE_ISA_RESPONSE_SCHEMA_V1,
        request_id: 1,
        response_revision: 1,
        operation: AgentSourceIsaOperationV1::InspectSourceIsaCollection,
        result: AgentSourceIsaResultV1::CollectionPage {
            authority: inspection.authority,
            collection: inspection.collection,
            page,
        },
    }))
}

fn decode_request(line: &[u8]) -> Result<AgentSourceIsaRequestV1, AgentSourceIsaErrorCodeV1> {
    if line.is_empty() || line.len() > MAX_AGENT_SOURCE_ISA_REQUEST_BYTES_V1 {
        return Err(AgentSourceIsaErrorCodeV1::RequestTooLarge);
    }
    let payload = line.strip_suffix(b"\n").unwrap_or(line);
    if payload.is_empty()
        || payload.contains(&b'\n')
        || payload.contains(&b'\r')
        || payload.len() >= MAX_AGENT_SOURCE_ISA_REQUEST_BYTES_V1
    {
        return Err(AgentSourceIsaErrorCodeV1::InvalidRequest);
    }
    serde_json::from_slice(payload).map_err(|_| AgentSourceIsaErrorCodeV1::InvalidRequest)
}

fn execute_request(
    request: AgentSourceIsaRequestV1,
    response_revision: u64,
) -> AgentSourceIsaResponseV1 {
    match request {
        AgentSourceIsaRequestV1::DiscoverCapabilities { schema, request_id } => {
            let operation = AgentSourceIsaOperationV1::DiscoverCapabilities;
            if schema != AGENT_SOURCE_ISA_REQUEST_SCHEMA_V1 {
                return AgentSourceIsaResponseV1::error(
                    Some(request_id),
                    response_revision,
                    Some(operation),
                    AgentSourceIsaErrorCodeV1::SchemaMismatch,
                    false,
                );
            }
            AgentSourceIsaResponseV1::Ok {
                schema: AGENT_SOURCE_ISA_RESPONSE_SCHEMA_V1,
                request_id,
                response_revision,
                operation,
                result: AgentSourceIsaResultV1::Capabilities {
                    authority: SourceIsaAuthorityViewV1::OBSERVATION_ONLY,
                    limits: limits(),
                    capabilities: vec![
                        AgentSourceIsaCapabilityV1 {
                            operation: AgentSourceIsaOperationV1::DiscoverCapabilities,
                            state: AgentSourceIsaCapabilityStateV1::Available,
                        },
                        AgentSourceIsaCapabilityV1 {
                            operation: AgentSourceIsaOperationV1::InspectSourceIsaCollection,
                            state: AgentSourceIsaCapabilityStateV1::Available,
                        },
                    ],
                },
            }
        }
        AgentSourceIsaRequestV1::InspectSourceIsaCollection {
            schema,
            request_id,
            collection_hex,
            page,
        } => {
            let operation = AgentSourceIsaOperationV1::InspectSourceIsaCollection;
            if schema != AGENT_SOURCE_ISA_REQUEST_SCHEMA_V1 {
                return AgentSourceIsaResponseV1::error(
                    Some(request_id),
                    response_revision,
                    Some(operation),
                    AgentSourceIsaErrorCodeV1::SchemaMismatch,
                    false,
                );
            }
            let encoded = match decode_lowercase_hex(&collection_hex) {
                Ok(encoded) => encoded,
                Err(error) => {
                    return AgentSourceIsaResponseV1::error(
                        Some(request_id),
                        response_revision,
                        Some(operation),
                        error,
                        false,
                    );
                }
            };
            let inspection = match SourceIsaInspectionV1::decode_canonical(&encoded) {
                Ok(inspection) => inspection,
                Err(_) => {
                    return AgentSourceIsaResponseV1::error(
                        Some(request_id),
                        response_revision,
                        Some(operation),
                        AgentSourceIsaErrorCodeV1::InvalidCollection,
                        false,
                    );
                }
            };
            let page = match inspection.page(&page) {
                Ok(page) => page,
                Err(error) => {
                    return AgentSourceIsaResponseV1::error(
                        Some(request_id),
                        response_revision,
                        Some(operation),
                        error,
                        false,
                    );
                }
            };
            AgentSourceIsaResponseV1::Ok {
                schema: AGENT_SOURCE_ISA_RESPONSE_SCHEMA_V1,
                request_id,
                response_revision,
                operation,
                result: AgentSourceIsaResultV1::CollectionPage {
                    authority: inspection.authority,
                    collection: inspection.collection,
                    page,
                },
            }
        }
    }
}

fn limits() -> AgentSourceIsaLimitsV1 {
    AgentSourceIsaLimitsV1 {
        max_request_bytes: MAX_AGENT_SOURCE_ISA_REQUEST_BYTES_V1,
        max_response_bytes: MAX_AGENT_SOURCE_ISA_RESPONSE_BYTES_V1,
        max_collection_binary_bytes: MAX_SOURCE_ISA_OBSERVATION_COLLECTION_BYTES_V1,
        max_collection_hex_bytes: MAX_SOURCE_ISA_OBSERVATION_COLLECTION_HEX_BYTES_V1,
        max_units: MAX_AGENT_SOURCE_ISA_UNITS_V1,
        max_page_items: MAX_AGENT_SOURCE_ISA_PAGE_ITEMS_V1,
        max_requests: MAX_AGENT_SOURCE_ISA_REQUESTS_V1,
    }
}

fn encode_response(response: AgentSourceIsaResponseV1) -> String {
    let revision = response.revision();
    serialize_response_bounded(
        &response,
        MAX_AGENT_SOURCE_ISA_RESPONSE_BYTES_V1.saturating_sub(1),
    )
    .unwrap_or_else(|| {
        serialize_response_bounded(
            &AgentSourceIsaResponseV1::error(
                None,
                revision,
                None,
                AgentSourceIsaErrorCodeV1::ResponseTooLarge,
                false,
            ),
            MAX_AGENT_SOURCE_ISA_RESPONSE_BYTES_V1.saturating_sub(1),
        )
        .expect("bounded source/ISA error response is serializable")
    })
}

impl AgentSourceIsaResponseV1 {
    fn revision(&self) -> u64 {
        match self {
            Self::Ok {
                response_revision, ..
            }
            | Self::Error {
                response_revision, ..
            } => *response_revision,
        }
    }
}

fn serialize_response_bounded(response: &AgentSourceIsaResponseV1, limit: usize) -> Option<String> {
    let mut writer = BoundedWriter::new(limit);
    serde_json::to_writer(&mut writer, response).ok()?;
    String::from_utf8(writer.bytes).ok()
}

fn write_response<W: Write>(
    output: &mut W,
    response: AgentSourceIsaResponseV1,
) -> Result<(), std::io::Error> {
    let encoded = encode_response(response);
    if encoded.len().saturating_add(1) > MAX_AGENT_SOURCE_ISA_RESPONSE_BYTES_V1 {
        return Err(std::io::Error::new(
            std::io::ErrorKind::FileTooLarge,
            "source/ISA agent response exceeds its newline-inclusive bound",
        ));
    }
    output.write_all(encoded.as_bytes())?;
    output.write_all(b"\n")
}

fn read_request_line<R: BufRead>(input: &mut R) -> Result<Option<Vec<u8>>, std::io::Error> {
    let mut line = Vec::new();
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
        let remaining = MAX_AGENT_SOURCE_ISA_REQUEST_BYTES_V1
            .saturating_add(1)
            .saturating_sub(line.len());
        let consumed = available.min(remaining);
        line.extend_from_slice(&buffer[..consumed]);
        input.consume(consumed);
        if line.len() > MAX_AGENT_SOURCE_ISA_REQUEST_BYTES_V1
            || (newline.is_some() && consumed == available)
        {
            return Ok(Some(line));
        }
    }
}

struct BoundedWriter {
    bytes: Vec<u8>,
    limit: usize,
}

impl BoundedWriter {
    fn new(limit: usize) -> Self {
        Self {
            bytes: Vec::new(),
            limit,
        }
    }
}

impl Write for BoundedWriter {
    fn write(&mut self, bytes: &[u8]) -> Result<usize, std::io::Error> {
        if bytes.len() > self.limit.saturating_sub(self.bytes.len()) {
            return Err(std::io::Error::new(
                std::io::ErrorKind::FileTooLarge,
                "bounded source/ISA JSON serialization exceeded its limit",
            ));
        }
        self.bytes.extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> Result<(), std::io::Error> {
        Ok(())
    }
}

fn decode_lowercase_hex(value: &str) -> Result<Vec<u8>, AgentSourceIsaErrorCodeV1> {
    if value.len() > MAX_SOURCE_ISA_OBSERVATION_COLLECTION_HEX_BYTES_V1 {
        return Err(AgentSourceIsaErrorCodeV1::CollectionHexTooLarge);
    }
    if value.is_empty() || value.len() % 2 != 0 {
        return Err(AgentSourceIsaErrorCodeV1::InvalidLowercaseHex);
    }
    let mut decoded = Vec::new();
    decoded
        .try_reserve_exact(value.len() / 2)
        .map_err(|_| AgentSourceIsaErrorCodeV1::CollectionHexTooLarge)?;
    for pair in value.as_bytes().chunks_exact(2) {
        let high = lowercase_nibble(pair[0])?;
        let low = lowercase_nibble(pair[1])?;
        decoded.push((high << 4) | low);
    }
    Ok(decoded)
}

fn lowercase_nibble(value: u8) -> Result<u8, AgentSourceIsaErrorCodeV1> {
    match value {
        b'0'..=b'9' => Ok(value - b'0'),
        b'a'..=b'f' => Ok(value - b'a' + 10),
        _ => Err(AgentSourceIsaErrorCodeV1::InvalidLowercaseHex),
    }
}

const PAGE_CURSOR_DOMAIN_V1: &[u8] = b"FE2O3/AGENT-SOURCE-ISA-PAGE-CURSOR/V1\0";
const MISSING_EVIDENCE_DOMAIN_V1: &[u8] = b"FE2O3/SOURCE-ISA-MISSING-EVIDENCE/V1\0";

fn page_cursor(
    collection_identity: &[u8; 32],
    canonical_byte_len: u64,
    unit_count: usize,
    position: usize,
    preceding_unit_identity: &str,
) -> String {
    debug_assert!(position > 0 && position < unit_count);
    let position_u32 = u32::try_from(position).expect("source/ISA unit bound fits u32");
    let unit_count = u32::try_from(unit_count).expect("source/ISA unit bound fits u32");
    let mut digest = Sha256::new();
    digest.update(PAGE_CURSOR_DOMAIN_V1);
    digest.update(collection_identity);
    digest.update(canonical_byte_len.to_le_bytes());
    digest.update(AGENT_SOURCE_ISA_REQUEST_SCHEMA_V1.as_bytes());
    digest.update(b"inspect_source_isa_collection\0all_units\0");
    digest.update(unit_count.to_le_bytes());
    digest.update(position_u32.to_le_bytes());
    digest.update(preceding_unit_identity.as_bytes());
    format!("v1:{position_u32}:{}", hex_bytes(&digest.finalize()))
}

fn parse_page_cursor(
    cursor: &str,
    collection_identity: &[u8; 32],
    canonical_byte_len: u64,
    unit_count: usize,
    preceding_unit_identity: String,
) -> Result<usize, AgentSourceIsaErrorCodeV1> {
    let position = cursor_position(cursor)?;
    if position >= unit_count {
        return Err(AgentSourceIsaErrorCodeV1::InvalidPage);
    }
    let expected = page_cursor(
        collection_identity,
        canonical_byte_len,
        unit_count,
        position,
        &preceding_unit_identity,
    );
    (expected == cursor)
        .then_some(position)
        .ok_or(AgentSourceIsaErrorCodeV1::InvalidPage)
}

fn cursor_position(cursor: &str) -> Result<usize, AgentSourceIsaErrorCodeV1> {
    let mut parts = cursor.split(':');
    if parts.next() != Some("v1") {
        return Err(AgentSourceIsaErrorCodeV1::InvalidPage);
    }
    let position_text = parts.next().ok_or(AgentSourceIsaErrorCodeV1::InvalidPage)?;
    let binding = parts.next().ok_or(AgentSourceIsaErrorCodeV1::InvalidPage)?;
    if parts.next().is_some()
        || position_text.is_empty()
        || (position_text.starts_with('0') && position_text != "0")
        || binding.len() != 64
        || !binding
            .as_bytes()
            .iter()
            .all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f'))
    {
        return Err(AgentSourceIsaErrorCodeV1::InvalidPage);
    }
    let position = position_text
        .parse::<usize>()
        .map_err(|_| AgentSourceIsaErrorCodeV1::InvalidPage)?;
    if position == 0 {
        return Err(AgentSourceIsaErrorCodeV1::InvalidPage);
    }
    Ok(position)
}

fn missing_evidence_id(
    collection_identity: &[u8; 32],
    canonical_byte_len: u64,
    unit_identity: &str,
) -> String {
    let mut digest = Sha256::new();
    digest.update(MISSING_EVIDENCE_DOMAIN_V1);
    digest.update(collection_identity);
    digest.update(canonical_byte_len.to_le_bytes());
    digest.update(unit_identity.as_bytes());
    hex_bytes(&digest.finalize())
}

pub fn hex_bytes(value: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(value.len().saturating_mul(2));
    for byte in value {
        output.push(char::from(DIGITS[usize::from(byte >> 4)]));
        output.push(char::from(DIGITS[usize::from(byte & 0x0f)]));
    }
    output
}

const fn source_isa_target(target: SourceIsaObservationTargetProfileV1) -> SourceIsaTargetViewV1 {
    match target {
        SourceIsaObservationTargetProfileV1::Gfx942 => SourceIsaTargetViewV1 {
            architecture: "gfx942",
            features: ["wavefrontsize64", "xnack-"],
        },
        SourceIsaObservationTargetProfileV1::Gfx950 => SourceIsaTargetViewV1 {
            architecture: "gfx950",
            features: ["wavefrontsize64", "xnack-"],
        },
    }
}

const fn source_isa_kir_version(version: SourceIsaObservationKirVersionV1) -> u16 {
    match version {
        SourceIsaObservationKirVersionV1::V8 => 8,
        SourceIsaObservationKirVersionV1::V9 => 9,
    }
}

const fn unavailable_reason(reason: SourceIsaObservationUnavailableReasonV1) -> &'static str {
    reason.label()
}

const fn observation_error(error: SourceIsaObservationErrorCodeV1) -> &'static str {
    error.label()
}

const fn transport_failure(
    failure: SourceIsaObservationTransportFailureV1,
) -> SourceIsaTypedCodeV1 {
    let label = match failure {
        SourceIsaObservationTransportFailureV1::CollectorAlreadyFailed => {
            "collector_already_failed"
        }
        SourceIsaObservationTransportFailureV1::UnitBound => "unit_bound",
        SourceIsaObservationTransportFailureV1::AggregateByteBound => "aggregate_byte_bound",
        SourceIsaObservationTransportFailureV1::ConflictingDuplicate => "conflicting_duplicate",
        SourceIsaObservationTransportFailureV1::RejectedFrame => "rejected_frame",
        SourceIsaObservationTransportFailureV1::MissingSelectedUnits => "missing_selected_units",
        SourceIsaObservationTransportFailureV1::BrokerWorkerPanic => "broker_worker_panic",
    };
    SourceIsaTypedCodeV1 {
        code: failure.code(),
        label,
    }
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use fe2o3_artifact_transaction::{BuildAttempt, BuildInvocation, BuildSession};
    use serde_json::{Value, json};

    use super::*;
    use crate::wire_v1::{
        SourceIsaObservationContextV1, SourceIsaObservationFrameV1, SourceIsaObservationOutcomeV1,
        SourceIsaObservationUnavailableReasonV1,
    };

    fn attempt(session: BuildSession, invocation: u8) -> BuildAttempt {
        BuildAttempt::new(
            u64::from(invocation),
            session,
            BuildInvocation::from_bytes([invocation; 32]),
        )
        .expect("valid test attempt")
    }

    fn collection(config: u8, observed: &[u8], missing: &[u8]) -> Vec<u8> {
        let session = BuildSession::from_bytes([0x77; 16]);
        let frames = observed
            .iter()
            .map(|unit| {
                let frame = SourceIsaObservationFrameV1::new(
                    SourceIsaObservationContextV1::new(
                        [config; 32],
                        [*unit; 32],
                        attempt(session, *unit),
                        [unit.wrapping_add(1); 32],
                    )
                    .expect("valid context"),
                    SourceIsaObservationOutcomeV1::Unavailable(
                        SourceIsaObservationUnavailableReasonV1::SourceProjectionForKirV9,
                    ),
                );
                ([*unit; 32], frame)
            })
            .collect();
        SourceIsaObservationCollectionV1::from_collected(
            [config; 32],
            session,
            frames,
            missing.iter().map(|unit| [*unit; 32]).collect(),
            (!missing.is_empty())
                .then_some(SourceIsaObservationTransportFailureV1::MissingSelectedUnits),
        )
        .encode_canonical()
        .expect("canonical test collection")
    }

    fn request(id: u64, bytes: &[u8], after: Option<String>, limit: u16) -> Vec<u8> {
        let mut line = serde_json::to_vec(&json!({
            "operation": "inspect_source_isa_collection",
            "schema": AGENT_SOURCE_ISA_REQUEST_SCHEMA_V1,
            "request_id": id,
            "collection_hex": hex_bytes(bytes),
            "page": {"after": after, "limit": limit}
        }))
        .expect("serialize request");
        line.push(b'\n');
        line
    }

    #[test]
    fn capabilities_are_versioned_bounded_and_explicitly_non_authoritative() {
        let output = execute_agent_source_isa_request_line_v1(
            br#"{"operation":"discover_capabilities","schema":"fe2o3-agent-source-isa-request-v1","request_id":1}
"#,
        );
        let response: Value = serde_json::from_str(&output).expect("typed response");
        assert_eq!(response["status"], "ok");
        assert_eq!(response["response_revision"], 1);
        assert_eq!(response["result"]["result"], "capabilities");
        assert_eq!(response["result"]["authority"]["observation_only"], true);
        for denied in [
            "compiler_authority",
            "proof_authority",
            "artifact_authority",
            "runtime_authority",
            "hardware_execution_observed",
            "complete_machine_coverage_proved",
            "semantic_refinement_proved",
        ] {
            assert_eq!(response["result"]["authority"][denied], false);
        }
        assert_eq!(
            response["result"]["limits"]["max_collection_binary_bytes"],
            696_432
        );
        assert_eq!(
            response["result"]["limits"]["max_collection_hex_bytes"],
            1_392_864
        );
        assert_eq!(response["result"]["limits"]["max_units"], 1_024);
        assert_eq!(response["result"]["limits"]["max_page_items"], 64);
    }

    #[test]
    fn paging_merges_units_and_binds_cursor_and_missing_evidence_to_collection() {
        let first = collection(0x41, &[0x20], &[0x10, 0x30]);
        let second = collection(0x42, &[0x20], &[0x10, 0x30]);
        let first_inspection = SourceIsaInspectionV1::decode_canonical(&first).unwrap();
        let first_page = first_inspection
            .page(&AgentSourceIsaPageRequestV1 {
                after: None,
                limit: 2,
            })
            .unwrap();
        assert!(!first_page.page_exhausted);
        assert_eq!(first_page.items[0].unit_identity(), hex_bytes(&[0x10; 32]));
        assert_eq!(first_page.items[1].unit_identity(), hex_bytes(&[0x20; 32]));
        let cursor = first_page.next_after.expect("resumable page");
        let resumed = first_inspection
            .page(&AgentSourceIsaPageRequestV1 {
                after: Some(cursor.clone()),
                limit: 2,
            })
            .unwrap();
        assert!(resumed.page_exhausted);
        assert_eq!(resumed.items.len(), 1);
        assert_eq!(resumed.items[0].unit_identity(), hex_bytes(&[0x30; 32]));
        assert!(resumed.next_after.is_none());

        let second_inspection = SourceIsaInspectionV1::decode_canonical(&second).unwrap();
        assert_eq!(
            second_inspection.page(&AgentSourceIsaPageRequestV1 {
                after: Some(cursor),
                limit: 1,
            }),
            Err(AgentSourceIsaErrorCodeV1::InvalidPage)
        );
        assert!(
            first_inspection
                .page(&AgentSourceIsaPageRequestV1 {
                    after: Some(format!("v1:999:{}", "0".repeat(64))),
                    limit: 1,
                })
                .is_err()
        );

        let SourceIsaUnitViewV1::Missing {
            missing_evidence_id: first_missing,
            ..
        } = &first_page.items[0]
        else {
            panic!("expected missing unit")
        };
        let second_page = second_inspection
            .page(&AgentSourceIsaPageRequestV1 {
                after: None,
                limit: 1,
            })
            .unwrap();
        let SourceIsaUnitViewV1::Missing {
            missing_evidence_id: second_missing,
            ..
        } = &second_page.items[0]
        else {
            panic!("expected missing unit")
        };
        assert_ne!(first_missing, second_missing);
    }

    #[test]
    fn service_rejects_zero_duplicate_uppercase_and_unknown_fields() {
        let bytes = collection(0x41, &[0x20], &[]);
        let mut service = AgentSourceIsaServiceV1::new();
        let zero: Value = serde_json::from_str(&encode_response(
            service.handle_line(&request(0, &bytes, None, 1)),
        ))
        .unwrap();
        assert_eq!(zero["error"], "invalid_request_id");

        let first: Value = serde_json::from_str(&encode_response(
            service.handle_line(&request(7, &bytes, None, 1)),
        ))
        .unwrap();
        assert_eq!(first["status"], "ok");
        let duplicate: Value = serde_json::from_str(&encode_response(
            service.handle_line(&request(7, &bytes, None, 1)),
        ))
        .unwrap();
        assert_eq!(duplicate["error"], "duplicate_request_id");
        assert_eq!(duplicate["response_revision"], 3);

        let mut uppercase = serde_json::to_vec(&json!({
            "operation": "inspect_source_isa_collection",
            "schema": AGENT_SOURCE_ISA_REQUEST_SCHEMA_V1,
            "request_id": 8,
            "collection_hex": hex_bytes(&bytes).to_uppercase(),
            "page": {"after": null, "limit": 1}
        }))
        .unwrap();
        uppercase.push(b'\n');
        let uppercase: Value =
            serde_json::from_str(&encode_response(service.handle_line(&uppercase))).unwrap();
        assert_eq!(uppercase["error"], "invalid_lowercase_hex");

        let unknown = br#"{"operation":"discover_capabilities","schema":"fe2o3-agent-source-isa-request-v1","request_id":9,"unknown":true}
"#;
        let unknown: Value =
            serde_json::from_str(&encode_response(service.handle_line(unknown))).unwrap();
        assert_eq!(unknown["error"], "invalid_request");
    }

    #[test]
    fn jsonl_stream_preserves_revisions_and_newline_in_response_bound() {
        let input = concat!(
            "{\"operation\":\"discover_capabilities\",\"schema\":\"fe2o3-agent-source-isa-request-v1\",\"request_id\":1}\n",
            "{\"operation\":\"discover_capabilities\",\"schema\":\"fe2o3-agent-source-isa-request-v1\",\"request_id\":2}\n"
        );
        let mut output = Vec::new();
        run_agent_source_isa_jsonl_v1(&mut Cursor::new(input), &mut output).unwrap();
        let lines = output.split(|byte| *byte == b'\n').collect::<Vec<_>>();
        assert_eq!(lines.len(), 3);
        for (index, line) in lines[..2].iter().enumerate() {
            assert!(line.len() + 1 <= MAX_AGENT_SOURCE_ISA_RESPONSE_BYTES_V1);
            let response: Value = serde_json::from_slice(line).unwrap();
            assert_eq!(response["response_revision"], (index + 1) as u64);
        }
        assert!(lines[2].is_empty());
    }
}
