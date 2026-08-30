#![forbid(unsafe_code)]
//! Bounded, deterministic, read-only queries over Semantic Trace V1.
//!
//! This crate is an observation surface. It grants no compiler, debugger,
//! runtime, KFD, address, handle, or execution-control authority.
//! Capture plans describe minimum missing evidence and qualitative collection
//! cost; they neither execute tools nor claim a diagnosis or performance
//! prediction.

mod capture_query;
pub use capture_query::*;
mod capture_compare;
pub use capture_compare::*;
mod counter_query;
pub use counter_query::*;
mod pc_sample_query;
pub use pc_sample_query::*;
mod profiler_query;
pub use profiler_query::*;
mod profiler_variant_v1;
pub use profiler_variant_v1::*;
mod distributed_overlap_v1;
pub use distributed_overlap_v1::*;
mod agent_service_v1;
pub use agent_service_v1::*;

use std::error::Error;
use std::fmt;
use std::io::{self, Write};

use fe2o3_semantic_trace::*;
use serde::{Serialize, Serializer};
use sha2::{Digest, Sha256};

mod capture_plan;

pub use capture_plan::*;

pub const QUERY_SCHEMA_V1: &str = "fe2o3-semantic-query-v1";
pub const MAX_QUERY_PAGE_ITEMS_V1: u16 = 4_096;
pub const MAX_QUERY_RESPONSE_BYTES_V1: u64 = 16 * 1024 * 1024;
pub const MIN_QUERY_RESPONSE_BYTES_V1: u64 = 16 * 1024;

const RESPONSE_ENVELOPE_BUDGET_V1: u64 = 8 * 1024;
const RESPONSE_ITEM_BUDGET_V1: u64 = 8 * 1024;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct QueryLimitsV1 {
    max_input_bytes: u64,
    max_page_items: u16,
    max_response_bytes: u64,
}

impl QueryLimitsV1 {
    pub fn new(
        max_input_bytes: u64,
        max_page_items: u16,
        max_response_bytes: u64,
    ) -> Result<Self, QueryErrorV1> {
        if max_input_bytes == 0 || max_input_bytes > MAX_TRACE_BYTES_V1 {
            return Err(QueryErrorV1::InputLimitOutOfRange {
                actual: max_input_bytes,
                max: MAX_TRACE_BYTES_V1,
            });
        }
        if max_page_items == 0 || max_page_items > MAX_QUERY_PAGE_ITEMS_V1 {
            return Err(QueryErrorV1::PageLimitOutOfRange {
                actual: u64::from(max_page_items),
                max: u64::from(MAX_QUERY_PAGE_ITEMS_V1),
            });
        }
        if !(MIN_QUERY_RESPONSE_BYTES_V1..=MAX_QUERY_RESPONSE_BYTES_V1)
            .contains(&max_response_bytes)
        {
            return Err(QueryErrorV1::ResponseLimitOutOfRange {
                actual: max_response_bytes,
                min: MIN_QUERY_RESPONSE_BYTES_V1,
                max: MAX_QUERY_RESPONSE_BYTES_V1,
            });
        }
        Ok(Self {
            max_input_bytes,
            max_page_items,
            max_response_bytes,
        })
    }

    pub const fn max_input_bytes(self) -> u64 {
        self.max_input_bytes
    }

    pub const fn max_page_items(self) -> u16 {
        self.max_page_items
    }

    pub const fn max_response_bytes(self) -> u64 {
        self.max_response_bytes
    }
}

impl Default for QueryLimitsV1 {
    fn default() -> Self {
        Self {
            max_input_bytes: MAX_TRACE_BYTES_V1,
            max_page_items: 128,
            max_response_bytes: 2 * 1024 * 1024,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub struct PageRequestV1 {
    /// Trace-bound cursor returned by a prior page. `None` starts a query.
    pub cursor: Option<QueryCursorV1>,
    pub limit: u16,
}

impl PageRequestV1 {
    pub const fn new(cursor: Option<QueryCursorV1>, limit: u16) -> Self {
        Self { cursor, limit }
    }
}

impl Default for PageRequestV1 {
    fn default() -> Self {
        Self {
            cursor: None,
            limit: 64,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub struct QueryCursorV1 {
    /// Binds the exact trace, page kind, and filters, but not page size.
    pub query_binding: OpaqueIdentityViewV1,
    pub event_position: u64,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct QueryFilterV1 {
    pub sequence_start: Option<u64>,
    pub sequence_end: Option<u64>,
    pub workgroup: Option<[u32; 3]>,
    pub wave: Option<u32>,
    pub lane: Option<u16>,
    pub function_ordinal: Option<u64>,
    pub block_ordinal: Option<u64>,
    pub operation_ordinal: Option<u64>,
    pub allocation: Option<(u64, u64)>,
    pub memory_access: Option<MemoryAccessFilterV1>,
    pub provenance: Option<ProvenanceFilterV1>,
    pub evidence_kind: Option<EvidenceKindFilterV1>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MemoryAccessFilterV1 {
    Read,
    Write,
    Atomic,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProvenanceFilterV1 {
    Declared,
    Proved,
    Observed,
    Inferred,
    Unavailable,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EvidenceKindFilterV1 {
    Declaration,
    Proof,
    InferenceRule,
    RuntimeObservation,
    Artifact,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PageKindV1 {
    Workgroups,
    Waves,
    Lanes,
    Sites,
    OperationOccurrences,
    MemoryAccesses,
    MemoryRegions,
    Faults,
    ProvenanceAndEvidence,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum QueryRequestV1 {
    Capabilities,
    DispatchSummary,
    PlanNextCapture {
        goal: CaptureGoalV1,
    },
    DiagnosisStatus {
        goal: CaptureGoalV1,
    },
    Page {
        kind: PageKindV1,
        page: PageRequestV1,
        filter: QueryFilterV1,
    },
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(tag = "response", rename_all = "snake_case")]
pub enum QueryResponseV1 {
    Capabilities {
        context: TraceContextViewV1,
        capabilities: Vec<CapabilityViewV1>,
    },
    DispatchSummary {
        context: TraceContextViewV1,
        summary: DispatchSummaryV1,
    },
    PlanNextCapture {
        context: TraceContextViewV1,
        plan: NextCapturePlanV1,
    },
    DiagnosisStatus {
        context: TraceContextViewV1,
        status: DiagnosisStatusV1,
    },
    Page {
        page: QueryPageV1,
    },
}

#[derive(Debug)]
pub struct TraceQuerySessionV1 {
    trace: TraceV1,
    input_bytes: u64,
    trace_binding: OpaqueIdentityViewV1,
    limits: QueryLimitsV1,
}

impl TraceQuerySessionV1 {
    /// Opens and fully validates one canonical Trace V1 byte stream.
    pub fn open(bytes: &[u8], limits: QueryLimitsV1) -> Result<Self, QueryErrorV1> {
        let input_bytes = u64::try_from(bytes.len()).map_err(|_| QueryErrorV1::SizeOverflow)?;
        if input_bytes > limits.max_input_bytes {
            return Err(QueryErrorV1::InputTooLarge {
                actual: input_bytes,
                max: limits.max_input_bytes,
            });
        }
        let trace_binding = trace_binding(bytes);
        let trace = decode_trace_v1(bytes).map_err(QueryErrorV1::TraceDecode)?;
        Ok(Self {
            trace,
            input_bytes,
            trace_binding,
            limits,
        })
    }

    /// Adopts an already constructed and validated Trace V1 value.
    pub fn from_trace(trace: TraceV1, limits: QueryLimitsV1) -> Result<Self, QueryErrorV1> {
        let encoded = encode_trace_v1(&trace).map_err(QueryErrorV1::TraceEncode)?;
        let input_bytes = u64::try_from(encoded.len()).map_err(|_| QueryErrorV1::SizeOverflow)?;
        if input_bytes > limits.max_input_bytes {
            return Err(QueryErrorV1::InputTooLarge {
                actual: input_bytes,
                max: limits.max_input_bytes,
            });
        }
        let trace_binding = trace_binding(&encoded);
        Ok(Self {
            trace,
            input_bytes,
            trace_binding,
            limits,
        })
    }

    pub const fn limits(&self) -> QueryLimitsV1 {
        self.limits
    }

    pub fn query(&self, request: QueryRequestV1) -> Result<QueryResponseV1, QueryErrorV1> {
        match request {
            QueryRequestV1::Capabilities => self.capabilities(),
            QueryRequestV1::DispatchSummary => Ok(QueryResponseV1::DispatchSummary {
                context: self.context()?,
                summary: self.dispatch_summary(),
            }),
            QueryRequestV1::PlanNextCapture { goal } => Ok(QueryResponseV1::PlanNextCapture {
                context: self.context()?,
                plan: capture_plan::plan_next_capture(&self.trace, self.trace_binding, goal)?,
            }),
            QueryRequestV1::DiagnosisStatus { goal } => Ok(QueryResponseV1::DiagnosisStatus {
                context: self.context()?,
                status: capture_plan::diagnosis_status(&self.trace, goal)?,
            }),
            QueryRequestV1::Page { kind, page, filter } => {
                self.validate_page_request(page)?;
                self.validate_filter(filter)?;
                let query_binding = query_binding(self.trace_binding, kind, filter);
                if let Some(cursor) = page.cursor
                    && cursor.query_binding != query_binding
                {
                    return Err(QueryErrorV1::CursorQueryMismatch);
                }
                Ok(QueryResponseV1::Page {
                    page: self.page(kind, page, filter, query_binding)?,
                })
            }
        }
    }

    /// Produces canonical compact JSON terminated by one newline.
    pub fn encode_json(&self, response: &QueryResponseV1) -> Result<Vec<u8>, QueryErrorV1> {
        let capacity = usize::try_from(self.limits.max_response_bytes)
            .map_err(|_| QueryErrorV1::SizeOverflow)?;
        let mut output = Vec::new();
        let mut writer = BoundedWriterV1 {
            output: &mut output,
            max: capacity.saturating_sub(1),
            limit_exceeded: false,
        };
        if serde_json::to_writer(&mut writer, response).is_err() {
            return Err(if writer.limit_exceeded {
                QueryErrorV1::ResponseTooLarge {
                    max: self.limits.max_response_bytes,
                }
            } else {
                QueryErrorV1::JsonEncodingFailure
            });
        }
        reserve_bounded(writer.output, 1, capacity)?;
        output.push(b'\n');
        Ok(output)
    }

    pub fn query_json(&self, request: QueryRequestV1) -> Result<Vec<u8>, QueryErrorV1> {
        let response = self.query(request)?;
        self.encode_json(&response)
    }

    fn validate_page_request(&self, page: PageRequestV1) -> Result<(), QueryErrorV1> {
        if page.limit == 0 || page.limit > self.limits.max_page_items {
            return Err(QueryErrorV1::PageLimitOutOfRange {
                actual: u64::from(page.limit),
                max: u64::from(self.limits.max_page_items),
            });
        }
        let event_count =
            u64::try_from(self.trace.events().len()).map_err(|_| QueryErrorV1::SizeOverflow)?;
        let event_position = page.cursor.map_or(0, |cursor| cursor.event_position);
        if event_position > event_count {
            return Err(QueryErrorV1::CursorOutOfRange {
                cursor: event_position,
                event_count,
            });
        }
        let response_upper_bound = RESPONSE_ENVELOPE_BUDGET_V1
            .checked_add(
                u64::from(page.limit)
                    .checked_mul(RESPONSE_ITEM_BUDGET_V1)
                    .ok_or(QueryErrorV1::SizeOverflow)?,
            )
            .ok_or(QueryErrorV1::SizeOverflow)?;
        if response_upper_bound > self.limits.max_response_bytes {
            return Err(QueryErrorV1::PageExceedsResponseBudget {
                requested_items: page.limit,
                conservative_bytes: response_upper_bound,
                max: self.limits.max_response_bytes,
            });
        }
        Ok(())
    }

    fn validate_filter(&self, filter: QueryFilterV1) -> Result<(), QueryErrorV1> {
        if matches!((filter.sequence_start, filter.sequence_end), (Some(start), Some(end)) if start > end)
        {
            return Err(QueryErrorV1::InvalidSequenceRange);
        }
        let launch = self.trace.header().launch();
        if let Some(workgroup) = filter.workgroup
            && launch.linear_workgroup(workgroup).is_none()
        {
            return Err(QueryErrorV1::WorkgroupOutsideLaunch { workgroup });
        }
        if let Some(wave) = filter.wave {
            let maximum = launch.waves_per_workgroup();
            if u64::from(wave) >= maximum {
                return Err(QueryErrorV1::WaveOutsideWorkgroup { wave, maximum });
            }
        }
        if let Some(lane) = filter.lane
            && lane >= launch.wave_width().lanes()
        {
            return Err(QueryErrorV1::LaneOutsideWave {
                lane,
                width: launch.wave_width().lanes(),
            });
        }
        Ok(())
    }

    fn context(&self) -> Result<TraceContextViewV1, QueryErrorV1> {
        let header = self.trace.header();
        let producer = header.producer();
        Ok(TraceContextViewV1 {
            schema: QUERY_SCHEMA_V1,
            trace_binding: self.trace_binding,
            input_bytes: self.input_bytes,
            event_count: u64::try_from(self.trace.events().len())
                .map_err(|_| QueryErrorV1::SizeOverflow)?,
            producer: ProducerViewV1 {
                kind: producer_kind_label(producer.kind()),
                name: clone_bounded(producer.name().as_str())?,
                version: clone_bounded(producer.version().as_str())?,
                executable: producer.executable().map(identity_view),
            },
            execution_kind: execution_kind_label(header.execution_kind()),
            kernel_ir: kernel_ir_view(header.kernel_ir_claim()),
            semantic_mir: header.semantic_mir().map(content_identity_view),
            lineage: header.lineage().map(content_identity_view),
            artifact: header.artifact().map(content_identity_view),
            dispatch: dispatch_view(header.dispatch()),
            launch: launch_view(header.launch()),
            capture: capture_view(header.completeness(), header.boundaries()),
        })
    }

    fn capabilities(&self) -> Result<QueryResponseV1, QueryErrorV1> {
        const SPECS: &[(
            CapabilityNameV1,
            CapabilityAvailabilityV1,
            Option<CapabilityUnavailableReasonV1>,
        )] = &[
            (
                CapabilityNameV1::DispatchSummary,
                CapabilityAvailabilityV1::Available,
                None,
            ),
            (
                CapabilityNameV1::WorkgroupObservations,
                CapabilityAvailabilityV1::Available,
                None,
            ),
            (
                CapabilityNameV1::WaveObservations,
                CapabilityAvailabilityV1::Available,
                None,
            ),
            (
                CapabilityNameV1::LaneObservations,
                CapabilityAvailabilityV1::Available,
                None,
            ),
            (
                CapabilityNameV1::SemanticSiteClaims,
                CapabilityAvailabilityV1::Available,
                None,
            ),
            (
                CapabilityNameV1::OperationOccurrences,
                CapabilityAvailabilityV1::Available,
                None,
            ),
            (
                CapabilityNameV1::MemoryAccesses,
                CapabilityAvailabilityV1::Available,
                None,
            ),
            (
                CapabilityNameV1::MemoryRegions,
                CapabilityAvailabilityV1::Available,
                None,
            ),
            (
                CapabilityNameV1::DiagnosticsAndFaults,
                CapabilityAvailabilityV1::Available,
                None,
            ),
            (
                CapabilityNameV1::ProvenanceAndEvidence,
                CapabilityAvailabilityV1::Available,
                None,
            ),
            (
                CapabilityNameV1::CaptureLoss,
                CapabilityAvailabilityV1::Available,
                None,
            ),
            (
                CapabilityNameV1::NextCapturePlanning,
                CapabilityAvailabilityV1::Available,
                None,
            ),
            (
                CapabilityNameV1::DiagnosisStatus,
                CapabilityAvailabilityV1::Available,
                None,
            ),
            (
                CapabilityNameV1::SourceLocations,
                CapabilityAvailabilityV1::Unavailable,
                Some(CapabilityUnavailableReasonV1::RequiresAuthenticatedCatalog),
            ),
            (
                CapabilityNameV1::RegisterValues,
                CapabilityAvailabilityV1::Unavailable,
                Some(CapabilityUnavailableReasonV1::NotRepresentedByTraceV1),
            ),
            (
                CapabilityNameV1::SourceVariableValues,
                CapabilityAvailabilityV1::Unavailable,
                Some(CapabilityUnavailableReasonV1::NotRepresentedByTraceV1),
            ),
            (
                CapabilityNameV1::RawNativeAddresses,
                CapabilityAvailabilityV1::Unavailable,
                Some(CapabilityUnavailableReasonV1::ForbiddenAuthority),
            ),
            (
                CapabilityNameV1::RuntimeHandles,
                CapabilityAvailabilityV1::Unavailable,
                Some(CapabilityUnavailableReasonV1::ForbiddenAuthority),
            ),
            (
                CapabilityNameV1::BreakpointsAndStepping,
                CapabilityAvailabilityV1::Unavailable,
                Some(CapabilityUnavailableReasonV1::ReadOnlySurface),
            ),
            (
                CapabilityNameV1::ExecutionMutation,
                CapabilityAvailabilityV1::Unavailable,
                Some(CapabilityUnavailableReasonV1::ReadOnlySurface),
            ),
            (
                CapabilityNameV1::PerformancePrediction,
                CapabilityAvailabilityV1::Unavailable,
                Some(CapabilityUnavailableReasonV1::OutsideCurrentScope),
            ),
            (
                CapabilityNameV1::HardwareCounterValues,
                CapabilityAvailabilityV1::Unavailable,
                Some(CapabilityUnavailableReasonV1::NotRepresentedByTraceV1),
            ),
            (
                CapabilityNameV1::PcSamples,
                CapabilityAvailabilityV1::Unavailable,
                Some(CapabilityUnavailableReasonV1::NotRepresentedByTraceV1),
            ),
            (
                CapabilityNameV1::DecodedAttWaveTimeline,
                CapabilityAvailabilityV1::Unavailable,
                Some(CapabilityUnavailableReasonV1::NotRepresentedByTraceV1),
            ),
            (
                CapabilityNameV1::DirectKfdDispatchObservation,
                CapabilityAvailabilityV1::Unavailable,
                Some(CapabilityUnavailableReasonV1::OutsideCurrentScope),
            ),
        ];
        let mut capabilities = Vec::new();
        capabilities.try_reserve_exact(SPECS.len()).map_err(|_| {
            QueryErrorV1::AllocationFailure {
                requested: SPECS.len(),
            }
        })?;
        capabilities.extend(
            SPECS
                .iter()
                .map(|(name, availability, reason)| CapabilityViewV1 {
                    name: *name,
                    availability: *availability,
                    reason: *reason,
                }),
        );
        Ok(QueryResponseV1::Capabilities {
            context: self.context()?,
            capabilities,
        })
    }

    fn dispatch_summary(&self) -> DispatchSummaryV1 {
        let mut summary = DispatchSummaryV1::default();
        for event in self.trace.events() {
            match event.scope().level() {
                ExecutionLevelV1::Dispatch => summary.dispatch_scoped_events += 1,
                ExecutionLevelV1::Workgroup { .. } => summary.workgroup_scoped_events += 1,
                ExecutionLevelV1::Wave { .. } => summary.wave_scoped_events += 1,
                ExecutionLevelV1::Lane { .. } => summary.lane_scoped_events += 1,
            }
            if event.site().is_some() {
                summary.site_claim_events += 1;
            }
            if matches!(event.provenance(), FactProvenanceV1::Unavailable { .. }) {
                summary.unavailable_fact_events += 1;
            }
            match event.kind() {
                TraceEventKindV1::Dispatch(DispatchEventV1::Begin) => {
                    summary.dispatch_begin_sequence = Some(event.sequence());
                }
                TraceEventKindV1::Dispatch(DispatchEventV1::End(outcome)) => {
                    summary.dispatch_end_sequence = Some(event.sequence());
                    summary.dispatch_outcome = Some(dispatch_outcome_label(outcome));
                }
                TraceEventKindV1::Invocation(_) => summary.invocation_events += 1,
                TraceEventKindV1::BlockEnter => summary.block_entry_events += 1,
                TraceEventKindV1::Operation(OperationEventV1::Begin(_)) => {
                    summary.operation_occurrences += 1;
                }
                TraceEventKindV1::Operation(OperationEventV1::End(_)) => {}
                TraceEventKindV1::Branch { .. } => summary.branch_events += 1,
                TraceEventKindV1::Memory(memory) => {
                    summary.memory_accesses += 1;
                    match memory.outcome() {
                        MemoryOutcomeV1::Fault(_) => summary.memory_faults += 1,
                        MemoryOutcomeV1::Unavailable(_) => summary.unavailable_memory_events += 1,
                        MemoryOutcomeV1::Completed => {}
                    }
                }
                TraceEventKindV1::Barrier(_) => summary.barrier_events += 1,
                TraceEventKindV1::Allocation(_) => summary.memory_region_events += 1,
                TraceEventKindV1::Diagnostic(_) => summary.diagnostic_events += 1,
            }
        }
        summary
    }

    fn page(
        &self,
        kind: PageKindV1,
        page: PageRequestV1,
        filter: QueryFilterV1,
        query_binding: OpaqueIdentityViewV1,
    ) -> Result<QueryPageV1, QueryErrorV1> {
        let start = usize::try_from(page.cursor.map_or(0, |cursor| cursor.event_position))
            .map_err(|_| QueryErrorV1::SizeOverflow)?;
        let limit = usize::from(page.limit);
        let mut items = Vec::new();
        items
            .try_reserve_exact(limit)
            .map_err(|_| QueryErrorV1::AllocationFailure { requested: limit })?;
        let mut position = start;
        let events = self.trace.events();
        while position < events.len() && items.len() < limit {
            let event = &events[position];
            if matches_filter(event, filter)
                && let Some(item) = query_item(kind, event)?
            {
                items.push(item);
            }
            position += 1;
        }
        let mut next_cursor = None;
        while position < events.len() {
            if matches_filter(&events[position], filter)
                && query_kind_matches(kind, &events[position])
            {
                next_cursor = Some(QueryCursorV1 {
                    query_binding,
                    event_position: u64::try_from(position)
                        .map_err(|_| QueryErrorV1::SizeOverflow)?,
                });
                break;
            }
            position += 1;
        }
        Ok(QueryPageV1 {
            context: self.context()?,
            kind,
            request: page,
            returned: u16::try_from(items.len()).map_err(|_| QueryErrorV1::SizeOverflow)?,
            next_cursor,
            items,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct TraceContextViewV1 {
    pub schema: &'static str,
    pub trace_binding: OpaqueIdentityViewV1,
    pub input_bytes: u64,
    pub event_count: u64,
    pub producer: ProducerViewV1,
    pub execution_kind: &'static str,
    pub kernel_ir: KernelIrClaimViewV1,
    pub semantic_mir: Option<ContentIdentityViewV1>,
    pub lineage: Option<ContentIdentityViewV1>,
    pub artifact: Option<ContentIdentityViewV1>,
    pub dispatch: DispatchIdentityViewV1,
    pub launch: LaunchViewV1,
    pub capture: CaptureViewV1,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ProducerViewV1 {
    pub kind: &'static str,
    pub name: String,
    pub version: String,
    pub executable: Option<OpaqueIdentityViewV1>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OpaqueIdentityViewV1 {
    /// Inert digest/identity bytes, never a native pointer or handle.
    pub bytes: [u8; 32],
}

impl Serialize for OpaqueIdentityViewV1 {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        const HEX: &[u8; 16] = b"0123456789abcdef";
        let mut encoded = [0_u8; 64];
        for (index, byte) in self.bytes.iter().copied().enumerate() {
            encoded[index * 2] = HEX[usize::from(byte >> 4)];
            encoded[index * 2 + 1] = HEX[usize::from(byte & 0x0f)];
        }
        serializer.serialize_str(std::str::from_utf8(&encoded).expect("hex is valid UTF-8"))
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub struct DispatchIdentityViewV1 {
    pub domain: &'static str,
    pub identity: OpaqueIdentityViewV1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub struct KernelIrClaimViewV1 {
    pub wire_version: u16,
    pub identity_policy: u16,
    pub digest: OpaqueIdentityViewV1,
    pub canonical_len: u64,
    pub authenticated: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub struct ContentIdentityViewV1 {
    pub scheme: &'static str,
    pub format_version: u16,
    pub digest: OpaqueIdentityViewV1,
    pub canonical_len: u64,
    pub authenticated: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub struct LaunchViewV1 {
    pub logical_grid: [u64; 3],
    pub grid_workgroups: [u32; 3],
    pub workgroup_size: [u32; 3],
    pub wave_width: u16,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub struct CaptureViewV1 {
    pub completeness: &'static str,
    pub truncation_reason: Option<&'static str>,
    pub emitted_events: Option<u64>,
    pub dropped_events: Option<u64>,
    pub dropped_events_known: bool,
    pub start_boundary: &'static str,
    pub end_boundary: &'static str,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CapabilityNameV1 {
    DispatchSummary,
    WorkgroupObservations,
    WaveObservations,
    LaneObservations,
    SemanticSiteClaims,
    OperationOccurrences,
    MemoryAccesses,
    MemoryRegions,
    DiagnosticsAndFaults,
    ProvenanceAndEvidence,
    CaptureLoss,
    NextCapturePlanning,
    DiagnosisStatus,
    SourceLocations,
    RegisterValues,
    SourceVariableValues,
    RawNativeAddresses,
    RuntimeHandles,
    BreakpointsAndStepping,
    ExecutionMutation,
    PerformancePrediction,
    HardwareCounterValues,
    PcSamples,
    DecodedAttWaveTimeline,
    DirectKfdDispatchObservation,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CapabilityAvailabilityV1 {
    Available,
    Unavailable,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CapabilityUnavailableReasonV1 {
    NotRepresentedByTraceV1,
    RequiresAuthenticatedCatalog,
    ForbiddenAuthority,
    ReadOnlySurface,
    OutsideCurrentScope,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub struct CapabilityViewV1 {
    pub name: CapabilityNameV1,
    pub availability: CapabilityAvailabilityV1,
    pub reason: Option<CapabilityUnavailableReasonV1>,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize)]
pub struct DispatchSummaryV1 {
    pub dispatch_begin_sequence: Option<u64>,
    pub dispatch_end_sequence: Option<u64>,
    pub dispatch_outcome: Option<&'static str>,
    pub dispatch_scoped_events: u64,
    pub workgroup_scoped_events: u64,
    pub wave_scoped_events: u64,
    pub lane_scoped_events: u64,
    pub invocation_events: u64,
    pub block_entry_events: u64,
    pub operation_occurrences: u64,
    pub branch_events: u64,
    pub memory_accesses: u64,
    pub memory_faults: u64,
    pub memory_region_events: u64,
    pub barrier_events: u64,
    pub diagnostic_events: u64,
    pub site_claim_events: u64,
    pub unavailable_fact_events: u64,
    pub unavailable_memory_events: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct QueryPageV1 {
    pub context: TraceContextViewV1,
    pub kind: PageKindV1,
    pub request: PageRequestV1,
    pub returned: u16,
    pub next_cursor: Option<QueryCursorV1>,
    pub items: Vec<QueryItemV1>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(tag = "item", rename_all = "snake_case")]
pub enum QueryItemV1 {
    ScopeObservation {
        event: EventContextViewV1,
        event_kind: EventKindViewV1,
    },
    SemanticSite {
        event: EventContextViewV1,
        site: SiteViewV1,
        event_kind: EventKindViewV1,
    },
    OperationOccurrence {
        event: EventContextViewV1,
        phase: &'static str,
        frame: u64,
        occurrence: u64,
    },
    MemoryAccess {
        event: EventContextViewV1,
        access: &'static str,
        allocation: AllocationIdentityViewV1,
        byte_offset: u64,
        byte_len: u64,
        address_space: &'static str,
        outcome: &'static str,
        unavailable_reason: Option<&'static str>,
        fault: Option<&'static str>,
    },
    MemoryRegion {
        event: EventContextViewV1,
        action: &'static str,
        allocation: AllocationIdentityViewV1,
        byte_len: Option<u64>,
        address_space: Option<&'static str>,
        layout_available: bool,
    },
    Fault {
        event: EventContextViewV1,
        source: &'static str,
        kind: &'static str,
        code: Option<u32>,
        allocation: Option<AllocationIdentityViewV1>,
    },
    ProvenanceAndEvidence {
        event: EventContextViewV1,
        event_kind: EventKindViewV1,
    },
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct EventContextViewV1 {
    pub sequence: u64,
    pub timestamp: TimestampViewV1,
    pub provenance: ProvenanceViewV1,
    pub scope: ScopeViewV1,
    pub site: Option<SiteViewV1>,
    pub evidence: Vec<EvidenceViewV1>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub struct TimestampViewV1 {
    pub kind: &'static str,
    pub logical_step: Option<u64>,
    pub clock_domain: Option<OpaqueIdentityViewV1>,
    pub clock_ticks: Option<u64>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub struct ProvenanceViewV1 {
    pub kind: &'static str,
    pub unavailable_reason: Option<&'static str>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub struct ScopeViewV1 {
    pub level: &'static str,
    pub workgroup: Option<[u32; 3]>,
    pub wave: Option<u32>,
    pub lane: Option<u16>,
    pub logical_workitem: Option<[u64; 3]>,
    pub active_mask: Option<u64>,
    pub wave_width: Option<u16>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub struct SiteViewV1 {
    pub function_ordinal: u64,
    pub block_ordinal: u64,
    pub point: &'static str,
    pub operation_ordinal: Option<u64>,
    pub resolved: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub struct EvidenceViewV1 {
    pub kind: &'static str,
    pub identity: OpaqueIdentityViewV1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub struct AllocationIdentityViewV1 {
    pub ordinal: u64,
    pub generation: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub struct EventKindViewV1 {
    pub category: &'static str,
    pub action: Option<&'static str>,
}

fn matches_filter(event: &TraceEventV1, filter: QueryFilterV1) -> bool {
    if filter
        .sequence_start
        .is_some_and(|start| event.sequence() < start)
        || filter
            .sequence_end
            .is_some_and(|end| event.sequence() >= end)
    {
        return false;
    }
    let scope = event.scope();
    if filter
        .workgroup
        .is_some_and(|workgroup| scope.workgroup_coordinate() != Some(workgroup))
        || filter
            .wave
            .is_some_and(|wave| scope.wave_ordinal() != Some(wave))
        || filter
            .lane
            .is_some_and(|lane| scope.lane_ordinal() != Some(lane))
    {
        return false;
    }
    let site = event.site();
    if filter
        .function_ordinal
        .is_some_and(|ordinal| site.is_none_or(|site| site.function_ordinal() != ordinal))
        || filter
            .block_ordinal
            .is_some_and(|ordinal| site.is_none_or(|site| site.block_ordinal() != ordinal))
        || filter.operation_ordinal.is_some_and(|ordinal| {
            !matches!(site.map(KirSiteClaimV1::point), Some(KirSitePointV1::Operation(actual)) if actual == ordinal)
        })
    {
        return false;
    }
    if filter
        .allocation
        .is_some_and(|expected| event_allocation(event) != Some(expected))
    {
        return false;
    }
    if filter.memory_access.is_some_and(|expected| {
        !matches!(event.kind(), TraceEventKindV1::Memory(memory) if memory_access_matches(memory.kind(), expected))
    }) {
        return false;
    }
    if filter
        .provenance
        .is_some_and(|expected| !provenance_matches(event.provenance(), expected))
    {
        return false;
    }
    if filter.evidence_kind.is_some_and(|expected| {
        !event
            .evidence_refs()
            .iter()
            .any(|evidence| evidence_kind_matches(evidence.kind(), expected))
    }) {
        return false;
    }
    true
}

fn query_kind_matches(kind: PageKindV1, event: &TraceEventV1) -> bool {
    match kind {
        PageKindV1::Workgroups => event.scope().workgroup_coordinate().is_some(),
        PageKindV1::Waves => event.scope().wave_ordinal().is_some(),
        PageKindV1::Lanes => event.scope().lane_ordinal().is_some(),
        PageKindV1::Sites => event.site().is_some(),
        PageKindV1::OperationOccurrences => {
            matches!(event.kind(), TraceEventKindV1::Operation(_))
        }
        PageKindV1::MemoryAccesses => matches!(event.kind(), TraceEventKindV1::Memory(_)),
        PageKindV1::MemoryRegions => matches!(event.kind(), TraceEventKindV1::Allocation(_)),
        PageKindV1::Faults => is_fault(event.kind()),
        PageKindV1::ProvenanceAndEvidence => true,
    }
}

fn query_item(kind: PageKindV1, event: &TraceEventV1) -> Result<Option<QueryItemV1>, QueryErrorV1> {
    if !query_kind_matches(kind, event) {
        return Ok(None);
    }
    let context = event_context(event)?;
    Ok(Some(match kind {
        PageKindV1::Workgroups | PageKindV1::Waves | PageKindV1::Lanes => {
            QueryItemV1::ScopeObservation {
                event: context,
                event_kind: event_kind_view(event.kind()),
            }
        }
        PageKindV1::Sites => QueryItemV1::SemanticSite {
            event: context,
            site: site_view(event.site().expect("site query matched a site")),
            event_kind: event_kind_view(event.kind()),
        },
        PageKindV1::OperationOccurrences => {
            let (phase, occurrence) = match event.kind() {
                TraceEventKindV1::Operation(OperationEventV1::Begin(occurrence)) => {
                    ("begin", occurrence)
                }
                TraceEventKindV1::Operation(OperationEventV1::End(occurrence)) => {
                    ("end", occurrence)
                }
                _ => unreachable!("operation query matched an operation"),
            };
            QueryItemV1::OperationOccurrence {
                event: context,
                phase,
                frame: occurrence.frame(),
                occurrence: occurrence.occurrence(),
            }
        }
        PageKindV1::MemoryAccesses => {
            let TraceEventKindV1::Memory(memory) = event.kind() else {
                unreachable!("memory query matched memory")
            };
            let (outcome, unavailable_reason, fault) = memory_outcome_view(memory.outcome());
            QueryItemV1::MemoryAccess {
                event: context,
                access: memory_access_label(memory.kind()),
                allocation: allocation_view(memory.allocation()),
                byte_offset: memory.byte_offset(),
                byte_len: memory.byte_len(),
                address_space: address_space_label(memory.address_space()),
                outcome,
                unavailable_reason,
                fault,
            }
        }
        PageKindV1::MemoryRegions => {
            let TraceEventKindV1::Allocation(allocation) = event.kind() else {
                unreachable!("region query matched allocation")
            };
            let (action, identity, byte_len, address_space, layout_available) =
                allocation_event_view(allocation);
            QueryItemV1::MemoryRegion {
                event: context,
                action,
                allocation: identity,
                byte_len,
                address_space,
                layout_available,
            }
        }
        PageKindV1::Faults => fault_item(context, event.kind()),
        PageKindV1::ProvenanceAndEvidence => QueryItemV1::ProvenanceAndEvidence {
            event: context,
            event_kind: event_kind_view(event.kind()),
        },
    }))
}

fn event_context(event: &TraceEventV1) -> Result<EventContextViewV1, QueryErrorV1> {
    let mut evidence = Vec::new();
    evidence
        .try_reserve_exact(event.evidence_refs().len())
        .map_err(|_| QueryErrorV1::AllocationFailure {
            requested: event.evidence_refs().len(),
        })?;
    evidence.extend(
        event
            .evidence_refs()
            .iter()
            .map(|reference| EvidenceViewV1 {
                kind: evidence_kind_label(reference.kind()),
                identity: identity_view(reference.identity()),
            }),
    );
    Ok(EventContextViewV1 {
        sequence: event.sequence(),
        timestamp: timestamp_view(event.timestamp()),
        provenance: provenance_view(event.provenance()),
        scope: scope_view(event.scope()),
        site: event.site().map(site_view),
        evidence,
    })
}

fn fault_item(context: EventContextViewV1, kind: TraceEventKindV1) -> QueryItemV1 {
    match kind {
        TraceEventKindV1::Diagnostic(diagnostic) => QueryItemV1::Fault {
            event: context,
            source: "diagnostic",
            kind: diagnostic_kind_label(diagnostic.kind()),
            code: Some(diagnostic.code()),
            allocation: None,
        },
        TraceEventKindV1::Memory(memory) => {
            let MemoryOutcomeV1::Fault(fault) = memory.outcome() else {
                unreachable!("fault query matched a memory fault")
            };
            QueryItemV1::Fault {
                event: context,
                source: "memory",
                kind: memory_fault_label(fault),
                code: None,
                allocation: Some(allocation_view(memory.allocation())),
            }
        }
        TraceEventKindV1::Dispatch(DispatchEventV1::End(outcome)) => QueryItemV1::Fault {
            event: context,
            source: "dispatch",
            kind: dispatch_outcome_label(outcome),
            code: None,
            allocation: None,
        },
        _ => unreachable!("fault query matched a fault"),
    }
}

fn is_fault(kind: TraceEventKindV1) -> bool {
    matches!(
        kind,
        TraceEventKindV1::Diagnostic(_)
            | TraceEventKindV1::Memory(MemoryEventV1 { .. })
            | TraceEventKindV1::Dispatch(DispatchEventV1::End(
                DispatchOutcomeV1::Failed | DispatchOutcomeV1::Cancelled
            ))
    ) && match kind {
        TraceEventKindV1::Memory(memory) => matches!(memory.outcome(), MemoryOutcomeV1::Fault(_)),
        TraceEventKindV1::Diagnostic(_) | TraceEventKindV1::Dispatch(DispatchEventV1::End(_)) => {
            true
        }
        _ => false,
    }
}

fn event_allocation(event: &TraceEventV1) -> Option<(u64, u64)> {
    let allocation = match event.kind() {
        TraceEventKindV1::Memory(memory) => memory.allocation(),
        TraceEventKindV1::Allocation(allocation) => allocation.allocation(),
        _ => return None,
    };
    Some((allocation.ordinal(), allocation.generation()))
}

fn event_kind_view(kind: TraceEventKindV1) -> EventKindViewV1 {
    let (category, action) = match kind {
        TraceEventKindV1::Dispatch(DispatchEventV1::Begin) => ("dispatch", Some("begin")),
        TraceEventKindV1::Dispatch(DispatchEventV1::End(_)) => ("dispatch", Some("end")),
        TraceEventKindV1::Invocation(InvocationEventV1::Begin) => ("invocation", Some("begin")),
        TraceEventKindV1::Invocation(InvocationEventV1::End) => ("invocation", Some("end")),
        TraceEventKindV1::BlockEnter => ("block", Some("enter")),
        TraceEventKindV1::Operation(OperationEventV1::Begin(_)) => ("operation", Some("begin")),
        TraceEventKindV1::Operation(OperationEventV1::End(_)) => ("operation", Some("end")),
        TraceEventKindV1::Branch { .. } => ("branch", None),
        TraceEventKindV1::Memory(memory) => ("memory", Some(memory_access_label(memory.kind()))),
        TraceEventKindV1::Barrier(barrier) => {
            ("barrier", Some(barrier_action_label(barrier.action())))
        }
        TraceEventKindV1::Allocation(allocation) => {
            ("allocation", Some(allocation_action_label(allocation)))
        }
        TraceEventKindV1::Diagnostic(_) => ("diagnostic", None),
    };
    EventKindViewV1 { category, action }
}

fn timestamp_view(timestamp: TimestampV1) -> TimestampViewV1 {
    match timestamp {
        TimestampV1::LogicalStep(step) => TimestampViewV1 {
            kind: "logical_step",
            logical_step: Some(step),
            clock_domain: None,
            clock_ticks: None,
        },
        TimestampV1::Clock { domain, ticks } => TimestampViewV1 {
            kind: "clock",
            logical_step: None,
            clock_domain: Some(identity_view(domain)),
            clock_ticks: Some(ticks),
        },
    }
}

fn provenance_view(provenance: FactProvenanceV1) -> ProvenanceViewV1 {
    match provenance {
        FactProvenanceV1::Declared => ProvenanceViewV1 {
            kind: "declared",
            unavailable_reason: None,
        },
        FactProvenanceV1::Proved => ProvenanceViewV1 {
            kind: "proved",
            unavailable_reason: None,
        },
        FactProvenanceV1::Observed => ProvenanceViewV1 {
            kind: "observed",
            unavailable_reason: None,
        },
        FactProvenanceV1::Inferred => ProvenanceViewV1 {
            kind: "inferred",
            unavailable_reason: None,
        },
        FactProvenanceV1::Unavailable { reason } => ProvenanceViewV1 {
            kind: "unavailable",
            unavailable_reason: Some(unavailable_reason_label(reason)),
        },
    }
}

fn scope_view(scope: ExecutionScopeV1) -> ScopeViewV1 {
    match scope.level() {
        ExecutionLevelV1::Dispatch => ScopeViewV1 {
            level: "dispatch",
            workgroup: None,
            wave: None,
            lane: None,
            logical_workitem: None,
            active_mask: None,
            wave_width: None,
        },
        ExecutionLevelV1::Workgroup { workgroup } => ScopeViewV1 {
            level: "workgroup",
            workgroup: Some(workgroup),
            wave: None,
            lane: None,
            logical_workitem: None,
            active_mask: None,
            wave_width: None,
        },
        ExecutionLevelV1::Wave {
            workgroup,
            wave,
            active_mask,
        } => ScopeViewV1 {
            level: "wave",
            workgroup: Some(workgroup),
            wave: Some(wave),
            lane: None,
            logical_workitem: None,
            active_mask: Some(active_mask.bits()),
            wave_width: Some(active_mask.width().lanes()),
        },
        ExecutionLevelV1::Lane {
            workgroup,
            wave,
            lane,
            logical_workitem,
            active_mask,
        } => ScopeViewV1 {
            level: "lane",
            workgroup: Some(workgroup),
            wave: Some(wave),
            lane: Some(lane),
            logical_workitem: Some(logical_workitem),
            active_mask: Some(active_mask.bits()),
            wave_width: Some(active_mask.width().lanes()),
        },
    }
}

fn site_view(site: KirSiteClaimV1) -> SiteViewV1 {
    let (point, operation_ordinal) = match site.point() {
        KirSitePointV1::BlockEntry => ("block_entry", None),
        KirSitePointV1::Operation(operation) => ("operation", Some(operation)),
        KirSitePointV1::Terminator => ("terminator", None),
    };
    SiteViewV1 {
        function_ordinal: site.function_ordinal(),
        block_ordinal: site.block_ordinal(),
        point,
        operation_ordinal,
        resolved: false,
    }
}

fn allocation_event_view(
    allocation: AllocationEventV1,
) -> (
    &'static str,
    AllocationIdentityViewV1,
    Option<u64>,
    Option<&'static str>,
    bool,
) {
    match allocation {
        AllocationEventV1::Create {
            allocation,
            byte_len,
            address_space,
        } => (
            "create",
            allocation_view(allocation),
            Some(byte_len),
            Some(address_space_label(address_space)),
            true,
        ),
        AllocationEventV1::Preexisting {
            allocation,
            byte_len,
            address_space,
        } => (
            "preexisting",
            allocation_view(allocation),
            Some(byte_len),
            Some(address_space_label(address_space)),
            true,
        ),
        AllocationEventV1::UnknownLifecycle { allocation } => (
            "unknown_lifecycle",
            allocation_view(allocation),
            None,
            None,
            false,
        ),
        AllocationEventV1::Release { allocation } => {
            ("release", allocation_view(allocation), None, None, false)
        }
    }
}

fn memory_outcome_view(
    outcome: MemoryOutcomeV1,
) -> (&'static str, Option<&'static str>, Option<&'static str>) {
    match outcome {
        MemoryOutcomeV1::Completed => ("completed", None, None),
        MemoryOutcomeV1::Fault(fault) => ("fault", None, Some(memory_fault_label(fault))),
        MemoryOutcomeV1::Unavailable(reason) => {
            ("unavailable", Some(unavailable_reason_label(reason)), None)
        }
    }
}

fn identity_view(identity: OpaqueIdentityV1) -> OpaqueIdentityViewV1 {
    OpaqueIdentityViewV1 {
        bytes: *identity.as_bytes(),
    }
}

fn dispatch_view(dispatch: DispatchIdentityV1) -> DispatchIdentityViewV1 {
    DispatchIdentityViewV1 {
        domain: dispatch_domain_label(dispatch.domain()),
        identity: identity_view(dispatch.identity()),
    }
}

fn kernel_ir_view(claim: KernelIrIdentityClaimV1) -> KernelIrClaimViewV1 {
    KernelIrClaimViewV1 {
        wire_version: claim.wire_version(),
        identity_policy: claim.identity_policy(),
        digest: identity_view(claim.digest()),
        canonical_len: claim.canonical_len(),
        authenticated: false,
    }
}

fn content_identity_view(identity: ContentIdentityV1) -> ContentIdentityViewV1 {
    ContentIdentityViewV1 {
        scheme: match identity.scheme() {
            ContentIdentitySchemeV1::RawCanonicalSha256 => "raw_canonical_sha256",
            ContentIdentitySchemeV1::DomainSeparatedSha256 => "domain_separated_sha256",
        },
        format_version: identity.format_version(),
        digest: identity_view(identity.digest()),
        canonical_len: identity.canonical_len(),
        authenticated: false,
    }
}

fn launch_view(launch: LaunchGeometryV1) -> LaunchViewV1 {
    LaunchViewV1 {
        logical_grid: launch.logical_grid(),
        grid_workgroups: launch.grid_workgroups(),
        workgroup_size: launch.workgroup_size(),
        wave_width: launch.wave_width().lanes(),
    }
}

fn capture_view(
    completeness: TraceCompletenessV1,
    boundaries: CaptureBoundariesV1,
) -> CaptureViewV1 {
    let (state, reason, emitted, dropped, known) = match completeness {
        TraceCompletenessV1::Complete => ("complete", None, None, None, true),
        TraceCompletenessV1::Truncated {
            reason,
            emitted_events,
            dropped_events,
        } => {
            let (dropped, known) = match dropped_events {
                DroppedEventCountV1::Known(count) => (Some(count), true),
                DroppedEventCountV1::Unknown => (None, false),
            };
            (
                "truncated",
                Some(truncation_reason_label(reason)),
                Some(emitted_events),
                dropped,
                known,
            )
        }
    };
    CaptureViewV1 {
        completeness: state,
        truncation_reason: reason,
        emitted_events: emitted,
        dropped_events: dropped,
        dropped_events_known: known,
        start_boundary: match boundaries.start() {
            CaptureStartBoundaryV1::DispatchBeginIncluded => "dispatch_begin_included",
            CaptureStartBoundaryV1::DispatchAlreadyActive => "dispatch_already_active",
        },
        end_boundary: match boundaries.end() {
            CaptureEndBoundaryV1::DispatchEndIncluded => "dispatch_end_included",
            CaptureEndBoundaryV1::DispatchContinuesAfterCapture => {
                "dispatch_continues_after_capture"
            }
        },
    }
}

fn allocation_view(allocation: TraceAllocationIdV1) -> AllocationIdentityViewV1 {
    AllocationIdentityViewV1 {
        ordinal: allocation.ordinal(),
        generation: allocation.generation(),
    }
}

fn clone_bounded(value: &str) -> Result<String, QueryErrorV1> {
    let mut output = String::new();
    output
        .try_reserve_exact(value.len())
        .map_err(|_| QueryErrorV1::AllocationFailure {
            requested: value.len(),
        })?;
    output.push_str(value);
    Ok(output)
}

fn trace_binding(bytes: &[u8]) -> OpaqueIdentityViewV1 {
    let mut hasher = Sha256::new();
    hasher.update(b"fe2o3-semantic-query/cursor/v1\0");
    hasher.update(bytes);
    OpaqueIdentityViewV1 {
        bytes: hasher.finalize().into(),
    }
}

fn query_binding(
    trace: OpaqueIdentityViewV1,
    kind: PageKindV1,
    filter: QueryFilterV1,
) -> OpaqueIdentityViewV1 {
    let mut hasher = Sha256::new();
    hasher.update(b"fe2o3-semantic-query/page/v1\0");
    hasher.update(trace.bytes);
    hasher.update([page_kind_tag(kind)]);
    hash_option_u64(&mut hasher, filter.sequence_start);
    hash_option_u64(&mut hasher, filter.sequence_end);
    match filter.workgroup {
        Some(workgroup) => {
            hasher.update([1]);
            for coordinate in workgroup {
                hasher.update(coordinate.to_le_bytes());
            }
        }
        None => hasher.update([0]),
    }
    hash_option_u32(&mut hasher, filter.wave);
    hash_option_u16(&mut hasher, filter.lane);
    hash_option_u64(&mut hasher, filter.function_ordinal);
    hash_option_u64(&mut hasher, filter.block_ordinal);
    hash_option_u64(&mut hasher, filter.operation_ordinal);
    match filter.allocation {
        Some((ordinal, generation)) => {
            hasher.update([1]);
            hasher.update(ordinal.to_le_bytes());
            hasher.update(generation.to_le_bytes());
        }
        None => hasher.update([0]),
    }
    hash_option_tag(
        &mut hasher,
        filter.memory_access.map(memory_access_filter_tag),
    );
    hash_option_tag(&mut hasher, filter.provenance.map(provenance_filter_tag));
    hash_option_tag(
        &mut hasher,
        filter.evidence_kind.map(evidence_kind_filter_tag),
    );
    OpaqueIdentityViewV1 {
        bytes: hasher.finalize().into(),
    }
}

fn hash_option_u64(hasher: &mut Sha256, value: Option<u64>) {
    match value {
        Some(value) => {
            hasher.update([1]);
            hasher.update(value.to_le_bytes());
        }
        None => hasher.update([0]),
    }
}

fn hash_option_u32(hasher: &mut Sha256, value: Option<u32>) {
    match value {
        Some(value) => {
            hasher.update([1]);
            hasher.update(value.to_le_bytes());
        }
        None => hasher.update([0]),
    }
}

fn hash_option_u16(hasher: &mut Sha256, value: Option<u16>) {
    match value {
        Some(value) => {
            hasher.update([1]);
            hasher.update(value.to_le_bytes());
        }
        None => hasher.update([0]),
    }
}

fn hash_option_tag(hasher: &mut Sha256, value: Option<u8>) {
    match value {
        Some(value) => hasher.update([1, value]),
        None => hasher.update([0]),
    }
}

const fn page_kind_tag(kind: PageKindV1) -> u8 {
    match kind {
        PageKindV1::Workgroups => 0,
        PageKindV1::Waves => 1,
        PageKindV1::Lanes => 2,
        PageKindV1::Sites => 3,
        PageKindV1::OperationOccurrences => 4,
        PageKindV1::MemoryAccesses => 5,
        PageKindV1::MemoryRegions => 6,
        PageKindV1::Faults => 7,
        PageKindV1::ProvenanceAndEvidence => 8,
    }
}

const fn memory_access_filter_tag(kind: MemoryAccessFilterV1) -> u8 {
    match kind {
        MemoryAccessFilterV1::Read => 0,
        MemoryAccessFilterV1::Write => 1,
        MemoryAccessFilterV1::Atomic => 2,
    }
}

const fn provenance_filter_tag(kind: ProvenanceFilterV1) -> u8 {
    match kind {
        ProvenanceFilterV1::Declared => 0,
        ProvenanceFilterV1::Proved => 1,
        ProvenanceFilterV1::Observed => 2,
        ProvenanceFilterV1::Inferred => 3,
        ProvenanceFilterV1::Unavailable => 4,
    }
}

const fn evidence_kind_filter_tag(kind: EvidenceKindFilterV1) -> u8 {
    match kind {
        EvidenceKindFilterV1::Declaration => 0,
        EvidenceKindFilterV1::Proof => 1,
        EvidenceKindFilterV1::InferenceRule => 2,
        EvidenceKindFilterV1::RuntimeObservation => 3,
        EvidenceKindFilterV1::Artifact => 4,
    }
}

fn producer_kind_label(kind: ProducerKindV1) -> &'static str {
    match kind {
        ProducerKindV1::CpuKirSimulator => "cpu_kir_simulator",
        ProducerKindV1::KfdHardwareCollector => "kfd_hardware_collector",
        ProducerKindV1::RocgdbImporter => "rocgdb_importer",
        ProducerKindV1::RocprofImporter => "rocprof_importer",
    }
}

fn execution_kind_label(kind: ExecutionKindV1) -> &'static str {
    match kind {
        ExecutionKindV1::CpuKirSimulation => "cpu_kir_simulation",
        ExecutionKindV1::KfdHardware => "kfd_hardware",
        ExecutionKindV1::RocgdbImport => "rocgdb_import",
        ExecutionKindV1::RocprofImport => "rocprof_import",
    }
}

fn dispatch_domain_label(domain: DispatchIdentityDomainV1) -> &'static str {
    match domain {
        DispatchIdentityDomainV1::TraceLocal => "trace_local",
        DispatchIdentityDomainV1::RuntimeModel => "runtime_model",
        DispatchIdentityDomainV1::ImportedCollector => "imported_collector",
    }
}

fn truncation_reason_label(reason: TruncationReasonV1) -> &'static str {
    match reason {
        TruncationReasonV1::EventLimit => "event_limit",
        TruncationReasonV1::ByteLimit => "byte_limit",
        TruncationReasonV1::CollectorLoss => "collector_loss",
        TruncationReasonV1::ProducerFailure => "producer_failure",
        TruncationReasonV1::UserStopped => "user_stopped",
    }
}

fn unavailable_reason_label(reason: UnavailableReasonV1) -> &'static str {
    match reason {
        UnavailableReasonV1::Unsupported => "unsupported",
        UnavailableReasonV1::NotCaptured => "not_captured",
        UnavailableReasonV1::OptimizedOut => "optimized_out",
        UnavailableReasonV1::OutsideCaptureScope => "outside_capture_scope",
        UnavailableReasonV1::Truncated => "truncated",
    }
}

fn evidence_kind_label(kind: EvidenceKindV1) -> &'static str {
    match kind {
        EvidenceKindV1::Declaration => "declaration",
        EvidenceKindV1::Proof => "proof",
        EvidenceKindV1::InferenceRule => "inference_rule",
        EvidenceKindV1::RuntimeObservation => "runtime_observation",
        EvidenceKindV1::Artifact => "artifact",
    }
}

fn memory_access_label(kind: MemoryAccessKindV1) -> &'static str {
    match kind {
        MemoryAccessKindV1::Read => "read",
        MemoryAccessKindV1::Write => "write",
        MemoryAccessKindV1::Atomic => "atomic",
    }
}

fn address_space_label(space: AddressSpaceV1) -> &'static str {
    match space {
        AddressSpaceV1::Private => "private",
        AddressSpaceV1::Workgroup => "workgroup",
        AddressSpaceV1::Global => "global",
        AddressSpaceV1::Constant => "constant",
        AddressSpaceV1::Generic => "generic",
    }
}

fn memory_fault_label(kind: MemoryFaultKindV1) -> &'static str {
    match kind {
        MemoryFaultKindV1::OutOfBounds => "out_of_bounds",
        MemoryFaultKindV1::Misaligned => "misaligned",
        MemoryFaultKindV1::InvalidAddressSpace => "invalid_address_space",
        MemoryFaultKindV1::UseAfterRelease => "use_after_release",
        MemoryFaultKindV1::Uninitialized => "uninitialized",
        MemoryFaultKindV1::PermissionDenied => "permission_denied",
        MemoryFaultKindV1::Unknown => "unknown",
    }
}

fn diagnostic_kind_label(kind: DiagnosticKindV1) -> &'static str {
    match kind {
        DiagnosticKindV1::Trap => "trap",
        DiagnosticKindV1::Assert => "assert",
        DiagnosticKindV1::Fault => "fault",
    }
}

fn dispatch_outcome_label(outcome: DispatchOutcomeV1) -> &'static str {
    match outcome {
        DispatchOutcomeV1::Completed => "completed",
        DispatchOutcomeV1::Failed => "failed",
        DispatchOutcomeV1::Cancelled => "cancelled",
    }
}

fn barrier_action_label(action: BarrierActionV1) -> &'static str {
    match action {
        BarrierActionV1::Arrive => "arrive",
        BarrierActionV1::Release => "release",
    }
}

fn allocation_action_label(event: AllocationEventV1) -> &'static str {
    match event {
        AllocationEventV1::Create { .. } => "create",
        AllocationEventV1::Preexisting { .. } => "preexisting",
        AllocationEventV1::UnknownLifecycle { .. } => "unknown_lifecycle",
        AllocationEventV1::Release { .. } => "release",
    }
}

fn memory_access_matches(actual: MemoryAccessKindV1, expected: MemoryAccessFilterV1) -> bool {
    matches!(
        (actual, expected),
        (MemoryAccessKindV1::Read, MemoryAccessFilterV1::Read)
            | (MemoryAccessKindV1::Write, MemoryAccessFilterV1::Write)
            | (MemoryAccessKindV1::Atomic, MemoryAccessFilterV1::Atomic)
    )
}

fn provenance_matches(actual: FactProvenanceV1, expected: ProvenanceFilterV1) -> bool {
    matches!(
        (actual, expected),
        (FactProvenanceV1::Declared, ProvenanceFilterV1::Declared)
            | (FactProvenanceV1::Proved, ProvenanceFilterV1::Proved)
            | (FactProvenanceV1::Observed, ProvenanceFilterV1::Observed)
            | (FactProvenanceV1::Inferred, ProvenanceFilterV1::Inferred)
            | (
                FactProvenanceV1::Unavailable { .. },
                ProvenanceFilterV1::Unavailable
            )
    )
}

fn evidence_kind_matches(actual: EvidenceKindV1, expected: EvidenceKindFilterV1) -> bool {
    matches!(
        (actual, expected),
        (
            EvidenceKindV1::Declaration,
            EvidenceKindFilterV1::Declaration
        ) | (EvidenceKindV1::Proof, EvidenceKindFilterV1::Proof)
            | (
                EvidenceKindV1::InferenceRule,
                EvidenceKindFilterV1::InferenceRule
            )
            | (
                EvidenceKindV1::RuntimeObservation,
                EvidenceKindFilterV1::RuntimeObservation
            )
            | (EvidenceKindV1::Artifact, EvidenceKindFilterV1::Artifact)
    )
}

struct BoundedWriterV1<'a> {
    output: &'a mut Vec<u8>,
    max: usize,
    limit_exceeded: bool,
}

impl Write for BoundedWriterV1<'_> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        let next = self
            .output
            .len()
            .checked_add(bytes.len())
            .ok_or_else(response_too_large_io_error)?;
        if next > self.max {
            self.limit_exceeded = true;
            return Err(response_too_large_io_error());
        }
        reserve_bounded_io(self.output, bytes.len(), self.max)?;
        self.output.extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

fn reserve_bounded(
    output: &mut Vec<u8>,
    additional: usize,
    max: usize,
) -> Result<(), QueryErrorV1> {
    let required = output
        .len()
        .checked_add(additional)
        .ok_or(QueryErrorV1::SizeOverflow)?;
    if required > max {
        return Err(QueryErrorV1::ResponseTooLarge {
            max: u64::try_from(max).unwrap_or(u64::MAX),
        });
    }
    if required <= output.capacity() {
        return Ok(());
    }
    let doubled = output.capacity().checked_mul(2).unwrap_or(max);
    let target = required.max(doubled).min(max);
    output
        .try_reserve_exact(target.saturating_sub(output.capacity()))
        .map_err(|_| QueryErrorV1::AllocationFailure { requested: target })?;
    if output.capacity() > max {
        return Err(QueryErrorV1::ResponseTooLarge {
            max: u64::try_from(max).unwrap_or(u64::MAX),
        });
    }
    Ok(())
}

fn reserve_bounded_io(output: &mut Vec<u8>, additional: usize, max: usize) -> io::Result<()> {
    reserve_bounded(output, additional, max)
        .map_err(|_| io::Error::other("could not grow bounded query response"))
}

fn response_too_large_io_error() -> io::Error {
    io::Error::new(
        io::ErrorKind::FileTooLarge,
        "query response exceeds configured bound",
    )
}

#[derive(Debug)]
pub enum QueryErrorV1 {
    InputLimitOutOfRange {
        actual: u64,
        max: u64,
    },
    ResponseLimitOutOfRange {
        actual: u64,
        min: u64,
        max: u64,
    },
    InputTooLarge {
        actual: u64,
        max: u64,
    },
    PageLimitOutOfRange {
        actual: u64,
        max: u64,
    },
    PageExceedsResponseBudget {
        requested_items: u16,
        conservative_bytes: u64,
        max: u64,
    },
    CursorOutOfRange {
        cursor: u64,
        event_count: u64,
    },
    CursorQueryMismatch,
    InvalidSequenceRange,
    WorkgroupOutsideLaunch {
        workgroup: [u32; 3],
    },
    WaveOutsideWorkgroup {
        wave: u32,
        maximum: u64,
    },
    LaneOutsideWave {
        lane: u16,
        width: u16,
    },
    SizeOverflow,
    AllocationFailure {
        requested: usize,
    },
    PlanLimitExceeded {
        field: &'static str,
        max: usize,
    },
    TraceDecode(TraceDecodeErrorV1),
    TraceEncode(TraceEncodeErrorV1),
    JsonEncodingFailure,
    ResponseTooLarge {
        max: u64,
    },
}

impl fmt::Display for QueryErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "semantic trace query failed: {self:?}")
    }
}

impl Error for QueryErrorV1 {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bounded_writer_never_retains_or_emits_past_limit() {
        let mut output = Vec::new();
        let mut writer = BoundedWriterV1 {
            output: &mut output,
            max: 3,
            limit_exceeded: false,
        };
        writer.write_all(b"abc").unwrap();
        assert!(writer.write_all(b"d").is_err());
        assert!(writer.limit_exceeded);
        assert_eq!(writer.output.as_slice(), b"abc");
        assert!(writer.output.capacity() <= 3);
    }

    #[test]
    fn opaque_identity_json_is_fixed_lowercase_hex() {
        let identity = OpaqueIdentityViewV1 { bytes: [0xab; 32] };
        let encoded = serde_json::to_string(&identity).unwrap();
        assert_eq!(encoded, format!("\"{}\"", "ab".repeat(32)));
    }
}
