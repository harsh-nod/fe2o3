use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;
use std::io::{self, BufRead, Write};

use fe2o3_semantic_import::{
    CaptureDispatchV1, CaptureIdentityV1, ContentIdentityRecordV1, ContentSchemeV1, IdentityFactV1,
    KernelIrClaimRecordV1, MAX_PROFILER_BUNDLE_BYTES_V4, ProfilerCoverageV4, TruthOriginV1,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{
    ProfilerBundleComparisonV4, ProfilerCapabilityV4, ProfilerCaptureGoalV4, ProfilerListKindV4,
    ProfilerPageRequestV4, ProfilerPageV4, ProfilerQueryContextV4, ProfilerQueryErrorV4,
    ProfilerQueryItemV4, ProfilerQueryLimitsV4, ProfilerQueryRequestV4, ProfilerQueryResponseV4,
    ProfilerQuerySessionV4, compare_profiler_bundles_v4, encode_profiler_bundle_comparison_v4,
};

pub const AGENT_PROFILER_REQUEST_SCHEMA_V1: &str = "fe2o3-agent-profiler-request-v1";
pub const AGENT_PROFILER_RESPONSE_SCHEMA_V1: &str = "fe2o3-agent-profiler-response-v1";
pub const MAX_AGENT_PROFILER_REQUEST_BYTES_V1: u64 = 34 * 1024 * 1024;
pub const MAX_AGENT_PROFILER_RESPONSE_BYTES_V1: u64 = 2 * 1024 * 1024;
pub const MAX_AGENT_PROFILER_REQUESTS_V1: u32 = 4_096;
pub const MAX_AGENT_PROFILER_OPEN_CAPTURES_V1: u8 = 16;
pub const DEFAULT_AGENT_PROFILER_OPEN_CAPTURES_V1: u8 = 4;

const AGENT_PROFILER_CONTRACT_DOMAIN_V1: &[u8] = b"fe2o3.agent-profiler.contract.v1\0";
const AGENT_PROFILER_CONTRACT_BYTES_V1: &[u8] =
    b"read-only;bundle-v4;bounded-jsonl;no-execution-authority";
const AGENT_PROFILER_RESPONSE_BINDING_DOMAIN_V1: &[u8] =
    b"fe2o3.agent-profiler.response-binding.v1\0";

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentProfilerOperationV1 {
    DiscoverCapabilities,
    OpenCapture,
    ListRuns,
    ListDevices,
    ListDispatches,
    ListAttReferences,
    ListHotspots,
    ListWaits,
    InspectDispatch,
    InspectKernel,
    InspectWorkgroup,
    InspectWave,
    InspectLane,
    ResolveSourceSite,
    CorrelateSourceIrIsa,
    ListFaults,
    InspectMemoryAccess,
    InspectBarrier,
    InspectProperty,
    CompareCaptures,
    ExplainRegression,
    PlanNextCapture,
    ExportReproducer,
}

impl AgentProfilerOperationV1 {
    pub const ALL: [Self; 23] = [
        Self::DiscoverCapabilities,
        Self::OpenCapture,
        Self::ListRuns,
        Self::ListDevices,
        Self::ListDispatches,
        Self::ListAttReferences,
        Self::ListHotspots,
        Self::ListWaits,
        Self::InspectDispatch,
        Self::InspectKernel,
        Self::InspectWorkgroup,
        Self::InspectWave,
        Self::InspectLane,
        Self::ResolveSourceSite,
        Self::CorrelateSourceIrIsa,
        Self::ListFaults,
        Self::InspectMemoryAccess,
        Self::InspectBarrier,
        Self::InspectProperty,
        Self::CompareCaptures,
        Self::ExplainRegression,
        Self::PlanNextCapture,
        Self::ExportReproducer,
    ];
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentProfilerCapabilityStateV1 {
    Available,
    CaptureDependent,
    Unavailable,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentProfilerUnavailableReasonV1 {
    NotRepresentedByProfilerBundleV4,
    DecodedAttEventsNotCaptured,
    AuthenticatedSourceCorrelationNotCaptured,
    WorkgroupWaveLaneHierarchyNotCaptured,
    CausalExplanationNotEstablished,
    CapturePlanRequirementsNotRepresented,
    ReproducerInputsNotCaptured,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub struct AgentProfilerCapabilityV1 {
    pub operation: AgentProfilerOperationV1,
    pub state: AgentProfilerCapabilityStateV1,
    pub unavailable_reason: Option<AgentProfilerUnavailableReasonV1>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub struct AgentProfilerLimitsViewV1 {
    pub max_request_bytes: u64,
    pub max_response_bytes: u64,
    pub max_requests: u32,
    pub max_open_captures: u8,
    pub max_page_items: u16,
    pub max_bundle_bytes: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AgentProfilerServiceLimitsV1 {
    pub max_requests: u32,
    pub max_open_captures: u8,
    pub query: ProfilerQueryLimitsV4,
}

impl Default for AgentProfilerServiceLimitsV1 {
    fn default() -> Self {
        Self {
            max_requests: MAX_AGENT_PROFILER_REQUESTS_V1,
            max_open_captures: DEFAULT_AGENT_PROFILER_OPEN_CAPTURES_V1,
            query: ProfilerQueryLimitsV4::default(),
        }
    }
}

impl AgentProfilerServiceLimitsV1 {
    pub fn new(
        max_requests: u32,
        max_open_captures: u8,
        query: ProfilerQueryLimitsV4,
    ) -> Result<Self, AgentProfilerServiceErrorV1> {
        if max_requests == 0
            || max_requests > MAX_AGENT_PROFILER_REQUESTS_V1
            || max_open_captures == 0
            || max_open_captures > MAX_AGENT_PROFILER_OPEN_CAPTURES_V1
        {
            return Err(AgentProfilerServiceErrorV1::LimitOutOfRange);
        }
        Ok(Self {
            max_requests,
            max_open_captures,
            query,
        })
    }

    fn view(self) -> AgentProfilerLimitsViewV1 {
        AgentProfilerLimitsViewV1 {
            max_request_bytes: MAX_AGENT_PROFILER_REQUEST_BYTES_V1,
            max_response_bytes: self
                .query
                .max_response_bytes
                .min(MAX_AGENT_PROFILER_RESPONSE_BYTES_V1),
            max_requests: self.max_requests,
            max_open_captures: self.max_open_captures,
            max_page_items: self.query.max_page_items,
            max_bundle_bytes: self.query.max_input_bytes,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(tag = "operation", rename_all = "snake_case", deny_unknown_fields)]
pub enum AgentProfilerRequestV1 {
    DiscoverCapabilities {
        schema: String,
        request_id: u64,
    },
    OpenCapture {
        schema: String,
        request_id: u64,
        bundle_hex: String,
    },
    ListRuns {
        schema: String,
        request_id: u64,
        capture: ContentIdentityRecordV1,
        page: ProfilerPageRequestV4,
    },
    ListDevices {
        schema: String,
        request_id: u64,
        capture: ContentIdentityRecordV1,
        page: ProfilerPageRequestV4,
    },
    ListDispatches {
        schema: String,
        request_id: u64,
        capture: ContentIdentityRecordV1,
        page: ProfilerPageRequestV4,
    },
    ListAttReferences {
        schema: String,
        request_id: u64,
        capture: ContentIdentityRecordV1,
        page: ProfilerPageRequestV4,
    },
    ListHotspots {
        schema: String,
        request_id: u64,
        capture: ContentIdentityRecordV1,
        page: ProfilerPageRequestV4,
    },
    ListWaits {
        schema: String,
        request_id: u64,
        capture: ContentIdentityRecordV1,
        page: ProfilerPageRequestV4,
    },
    InspectDispatch {
        schema: String,
        request_id: u64,
        capture: ContentIdentityRecordV1,
        dispatch: CaptureIdentityV1,
    },
    InspectKernel {
        schema: String,
        request_id: u64,
        capture: ContentIdentityRecordV1,
        dispatch: CaptureIdentityV1,
    },
    InspectWorkgroup {
        schema: String,
        request_id: u64,
        capture: ContentIdentityRecordV1,
        dispatch: CaptureIdentityV1,
        workgroup: [u32; 3],
    },
    InspectWave {
        schema: String,
        request_id: u64,
        capture: ContentIdentityRecordV1,
        dispatch: CaptureIdentityV1,
        workgroup: [u32; 3],
        wave: u32,
    },
    InspectLane {
        schema: String,
        request_id: u64,
        capture: ContentIdentityRecordV1,
        dispatch: CaptureIdentityV1,
        workgroup: [u32; 3],
        wave: u32,
        lane: u16,
    },
    ResolveSourceSite {
        schema: String,
        request_id: u64,
        capture: ContentIdentityRecordV1,
        dispatch: CaptureIdentityV1,
    },
    CorrelateSourceIrIsa {
        schema: String,
        request_id: u64,
        capture: ContentIdentityRecordV1,
        dispatch: CaptureIdentityV1,
    },
    ListFaults {
        schema: String,
        request_id: u64,
        capture: ContentIdentityRecordV1,
        page: ProfilerPageRequestV4,
    },
    InspectMemoryAccess {
        schema: String,
        request_id: u64,
        capture: ContentIdentityRecordV1,
        access: CaptureIdentityV1,
    },
    InspectBarrier {
        schema: String,
        request_id: u64,
        capture: ContentIdentityRecordV1,
        barrier: CaptureIdentityV1,
    },
    InspectProperty {
        schema: String,
        request_id: u64,
        capture: ContentIdentityRecordV1,
        property: CaptureIdentityV1,
    },
    CompareCaptures {
        schema: String,
        request_id: u64,
        baseline: ContentIdentityRecordV1,
        candidate: ContentIdentityRecordV1,
    },
    ExplainRegression {
        schema: String,
        request_id: u64,
        baseline: ContentIdentityRecordV1,
        candidate: ContentIdentityRecordV1,
    },
    PlanNextCapture {
        schema: String,
        request_id: u64,
        capture: ContentIdentityRecordV1,
        goal: ProfilerCaptureGoalV4,
    },
    ExportReproducer {
        schema: String,
        request_id: u64,
        capture: ContentIdentityRecordV1,
    },
}

impl AgentProfilerRequestV1 {
    pub fn request_id(&self) -> u64 {
        match self {
            Self::DiscoverCapabilities { request_id, .. }
            | Self::OpenCapture { request_id, .. }
            | Self::ListRuns { request_id, .. }
            | Self::ListDevices { request_id, .. }
            | Self::ListDispatches { request_id, .. }
            | Self::ListAttReferences { request_id, .. }
            | Self::ListHotspots { request_id, .. }
            | Self::ListWaits { request_id, .. }
            | Self::InspectDispatch { request_id, .. }
            | Self::InspectKernel { request_id, .. }
            | Self::InspectWorkgroup { request_id, .. }
            | Self::InspectWave { request_id, .. }
            | Self::InspectLane { request_id, .. }
            | Self::ResolveSourceSite { request_id, .. }
            | Self::CorrelateSourceIrIsa { request_id, .. }
            | Self::ListFaults { request_id, .. }
            | Self::InspectMemoryAccess { request_id, .. }
            | Self::InspectBarrier { request_id, .. }
            | Self::InspectProperty { request_id, .. }
            | Self::CompareCaptures { request_id, .. }
            | Self::ExplainRegression { request_id, .. }
            | Self::PlanNextCapture { request_id, .. }
            | Self::ExportReproducer { request_id, .. } => *request_id,
        }
    }

    fn schema(&self) -> &str {
        match self {
            Self::DiscoverCapabilities { schema, .. }
            | Self::OpenCapture { schema, .. }
            | Self::ListRuns { schema, .. }
            | Self::ListDevices { schema, .. }
            | Self::ListDispatches { schema, .. }
            | Self::ListAttReferences { schema, .. }
            | Self::ListHotspots { schema, .. }
            | Self::ListWaits { schema, .. }
            | Self::InspectDispatch { schema, .. }
            | Self::InspectKernel { schema, .. }
            | Self::InspectWorkgroup { schema, .. }
            | Self::InspectWave { schema, .. }
            | Self::InspectLane { schema, .. }
            | Self::ResolveSourceSite { schema, .. }
            | Self::CorrelateSourceIrIsa { schema, .. }
            | Self::ListFaults { schema, .. }
            | Self::InspectMemoryAccess { schema, .. }
            | Self::InspectBarrier { schema, .. }
            | Self::InspectProperty { schema, .. }
            | Self::CompareCaptures { schema, .. }
            | Self::ExplainRegression { schema, .. }
            | Self::PlanNextCapture { schema, .. }
            | Self::ExportReproducer { schema, .. } => schema,
        }
    }

    pub fn operation(&self) -> AgentProfilerOperationV1 {
        match self {
            Self::DiscoverCapabilities { .. } => AgentProfilerOperationV1::DiscoverCapabilities,
            Self::OpenCapture { .. } => AgentProfilerOperationV1::OpenCapture,
            Self::ListRuns { .. } => AgentProfilerOperationV1::ListRuns,
            Self::ListDevices { .. } => AgentProfilerOperationV1::ListDevices,
            Self::ListDispatches { .. } => AgentProfilerOperationV1::ListDispatches,
            Self::ListAttReferences { .. } => AgentProfilerOperationV1::ListAttReferences,
            Self::ListHotspots { .. } => AgentProfilerOperationV1::ListHotspots,
            Self::ListWaits { .. } => AgentProfilerOperationV1::ListWaits,
            Self::InspectDispatch { .. } => AgentProfilerOperationV1::InspectDispatch,
            Self::InspectKernel { .. } => AgentProfilerOperationV1::InspectKernel,
            Self::InspectWorkgroup { .. } => AgentProfilerOperationV1::InspectWorkgroup,
            Self::InspectWave { .. } => AgentProfilerOperationV1::InspectWave,
            Self::InspectLane { .. } => AgentProfilerOperationV1::InspectLane,
            Self::ResolveSourceSite { .. } => AgentProfilerOperationV1::ResolveSourceSite,
            Self::CorrelateSourceIrIsa { .. } => AgentProfilerOperationV1::CorrelateSourceIrIsa,
            Self::ListFaults { .. } => AgentProfilerOperationV1::ListFaults,
            Self::InspectMemoryAccess { .. } => AgentProfilerOperationV1::InspectMemoryAccess,
            Self::InspectBarrier { .. } => AgentProfilerOperationV1::InspectBarrier,
            Self::InspectProperty { .. } => AgentProfilerOperationV1::InspectProperty,
            Self::CompareCaptures { .. } => AgentProfilerOperationV1::CompareCaptures,
            Self::ExplainRegression { .. } => AgentProfilerOperationV1::ExplainRegression,
            Self::PlanNextCapture { .. } => AgentProfilerOperationV1::PlanNextCapture,
            Self::ExportReproducer { .. } => AgentProfilerOperationV1::ExportReproducer,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentProfilerOpenEffectV1 {
    Registered,
    AlreadyOpen,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub struct AgentProfilerOpenAuditV1 {
    pub before_open_captures: u8,
    pub after_open_captures: u8,
    pub effect: AgentProfilerOpenEffectV1,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(tag = "classification", rename_all = "snake_case")]
pub enum AgentProfilerAggregateOriginV1 {
    Homogeneous { origin: TruthOriginV1 },
    Mixed { origins: Vec<TruthOriginV1> },
    Empty,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct AgentProfilerEvidenceV1 {
    pub origin: AgentProfilerAggregateOriginV1,
    pub service_contract: ContentIdentityRecordV1,
    pub captures: Vec<ContentIdentityRecordV1>,
    pub records: Vec<CaptureIdentityV1>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentProfilerKernelScopeV1 {
    DispatchBindingOnly,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct AgentProfilerKernelInspectionV1 {
    pub context: ProfilerQueryContextV4,
    pub dispatch_identity: CaptureIdentityV1,
    pub kernel_ir: KernelIrClaimRecordV1,
    pub artifact: IdentityFactV1,
    pub source_map: IdentityFactV1,
    pub scope: AgentProfilerKernelScopeV1,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(tag = "result", rename_all = "snake_case")]
pub enum AgentProfilerResultV1 {
    Capabilities {
        capabilities: Vec<AgentProfilerCapabilityV1>,
        limits: AgentProfilerLimitsViewV1,
        evidence: AgentProfilerEvidenceV1,
    },
    CaptureOpened {
        context: ProfilerQueryContextV4,
        coverage: ProfilerCoverageV4,
        capture_capabilities: Vec<ProfilerCapabilityV4>,
        audit: AgentProfilerOpenAuditV1,
        evidence: AgentProfilerEvidenceV1,
    },
    Page {
        page: ProfilerPageV4,
        evidence: AgentProfilerEvidenceV1,
    },
    Dispatch {
        context: ProfilerQueryContextV4,
        dispatch: Box<CaptureDispatchV1>,
        evidence: AgentProfilerEvidenceV1,
    },
    Kernel {
        inspection: AgentProfilerKernelInspectionV1,
        evidence: AgentProfilerEvidenceV1,
    },
    Comparison {
        comparison: ProfilerBundleComparisonV4,
        evidence: AgentProfilerEvidenceV1,
    },
    Unavailable {
        operation: AgentProfilerOperationV1,
        reason: AgentProfilerUnavailableReasonV1,
        evidence: AgentProfilerEvidenceV1,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentProfilerErrorCodeV1 {
    InvalidRequest,
    InvalidSchema,
    InvalidRequestId,
    DuplicateRequestId,
    RequestBudgetExhausted,
    RequestTooLarge,
    InvalidBundleEncoding,
    BundleTooLarge,
    InvalidBundle,
    CaptureLimitReached,
    CaptureNotOpen,
    InvalidPage,
    InvalidSelector,
    RecordNotFound,
    ResponseTooLarge,
    InternalEvidenceMismatch,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AgentProfilerResponseBindingV1([u8; 32]);

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum AgentProfilerResponseV1 {
    Ok {
        schema: &'static str,
        request_id: u64,
        response_revision: u64,
        value: Box<AgentProfilerResultV1>,
        #[serde(skip)]
        state_binding: AgentProfilerResponseBindingV1,
    },
    Error {
        schema: &'static str,
        request_id: Option<u64>,
        response_revision: u64,
        code: AgentProfilerErrorCodeV1,
        terminal: bool,
        #[serde(skip)]
        state_binding: AgentProfilerResponseBindingV1,
    },
}

impl AgentProfilerResponseV1 {
    pub fn is_terminal(&self) -> bool {
        matches!(self, Self::Error { terminal: true, .. })
    }
}

struct OpenCaptureV1 {
    bytes: Vec<u8>,
    session: ProfilerQuerySessionV4,
    context: ProfilerQueryContextV4,
    coverage: ProfilerCoverageV4,
    capabilities: Vec<ProfilerCapabilityV4>,
}

pub struct AgentProfilerServiceV1 {
    limits: AgentProfilerServiceLimitsV1,
    contract: ContentIdentityRecordV1,
    captures: BTreeMap<CaptureIdentityV1, OpenCaptureV1>,
    request_ids: BTreeSet<u64>,
    request_count: u32,
    response_revision: u64,
    response_bindings: BTreeMap<u64, AgentProfilerResponseBindingV1>,
    terminal_response: Option<AgentProfilerResponseV1>,
}

impl AgentProfilerServiceV1 {
    pub fn new(limits: AgentProfilerServiceLimitsV1) -> Result<Self, AgentProfilerServiceErrorV1> {
        AgentProfilerServiceLimitsV1::new(
            limits.max_requests,
            limits.max_open_captures,
            limits.query,
        )?;
        Ok(Self {
            limits,
            contract: service_contract_identity()?,
            captures: BTreeMap::new(),
            request_ids: BTreeSet::new(),
            request_count: 0,
            response_revision: 0,
            response_bindings: BTreeMap::new(),
            terminal_response: None,
        })
    }

    pub fn handle(&mut self, request: AgentProfilerRequestV1) -> AgentProfilerResponseV1 {
        if let Some(response) = &self.terminal_response {
            return response.clone();
        }
        let request_id = request.request_id();
        if self.request_count >= self.limits.max_requests {
            return self.error(
                Some(request_id),
                AgentProfilerErrorCodeV1::RequestBudgetExhausted,
                true,
            );
        }
        self.request_count = self
            .request_count
            .checked_add(1)
            .expect("the configured request bound fits u32");
        if request_id == 0 {
            return self.error(None, AgentProfilerErrorCodeV1::InvalidRequestId, false);
        }
        if !self.request_ids.insert(request_id) {
            return self.error(
                Some(request_id),
                AgentProfilerErrorCodeV1::DuplicateRequestId,
                false,
            );
        }
        if request.schema() != AGENT_PROFILER_REQUEST_SCHEMA_V1 {
            return self.error(
                Some(request_id),
                AgentProfilerErrorCodeV1::InvalidSchema,
                false,
            );
        }
        if matches!(
            &request,
            AgentProfilerRequestV1::InspectLane { lane, .. } if *lane >= 64
        ) {
            return self.error(
                Some(request_id),
                AgentProfilerErrorCodeV1::InvalidSelector,
                false,
            );
        }

        let result = self.process(request);
        match result {
            Ok(value) => self.ok(request_id, value),
            Err(code) => self.error(Some(request_id), code, false),
        }
    }

    pub fn encode_response(
        &self,
        response: &AgentProfilerResponseV1,
    ) -> Result<Vec<u8>, AgentProfilerServiceErrorV1> {
        self.validate_response(response)?;
        encode_response_bounded(response, self.limits.view().max_response_bytes)
    }

    pub fn terminal_protocol_error(
        &mut self,
        mut code: AgentProfilerErrorCodeV1,
    ) -> Result<AgentProfilerResponseV1, AgentProfilerServiceErrorV1> {
        if let Some(response) = &self.terminal_response {
            return Ok(response.clone());
        }
        if !matches!(
            code,
            AgentProfilerErrorCodeV1::InvalidRequest
                | AgentProfilerErrorCodeV1::RequestTooLarge
                | AgentProfilerErrorCodeV1::ResponseTooLarge
                | AgentProfilerErrorCodeV1::InternalEvidenceMismatch
        ) {
            return Err(AgentProfilerServiceErrorV1::InvalidResponse);
        }
        if matches!(
            code,
            AgentProfilerErrorCodeV1::InvalidRequest | AgentProfilerErrorCodeV1::RequestTooLarge
        ) {
            if self.request_count >= self.limits.max_requests {
                code = AgentProfilerErrorCodeV1::RequestBudgetExhausted;
            } else {
                self.request_count = self
                    .request_count
                    .checked_add(1)
                    .ok_or(AgentProfilerServiceErrorV1::RevisionOverflow)?;
            }
        }
        Ok(self.error(None, code, true))
    }

    fn process(
        &mut self,
        request: AgentProfilerRequestV1,
    ) -> Result<AgentProfilerResultV1, AgentProfilerErrorCodeV1> {
        let operation = request.operation();
        match request {
            AgentProfilerRequestV1::DiscoverCapabilities { .. } => {
                Ok(AgentProfilerResultV1::Capabilities {
                    capabilities: agent_capabilities(),
                    limits: self.limits.view(),
                    evidence: self.evidence(TruthOriginV1::Declared, &[], &[]),
                })
            }
            AgentProfilerRequestV1::OpenCapture { bundle_hex, .. } => {
                self.open_capture(&bundle_hex)
            }
            AgentProfilerRequestV1::ListRuns { capture, page, .. } => {
                self.list(capture, ProfilerListKindV4::Runs, page)
            }
            AgentProfilerRequestV1::ListDevices { capture, page, .. } => {
                self.list(capture, ProfilerListKindV4::Devices, page)
            }
            AgentProfilerRequestV1::ListDispatches { capture, page, .. } => {
                self.list(capture, ProfilerListKindV4::Dispatches, page)
            }
            AgentProfilerRequestV1::ListAttReferences { capture, page, .. } => {
                self.list(capture, ProfilerListKindV4::AttReferences, page)
            }
            AgentProfilerRequestV1::ListHotspots { capture, page, .. } => {
                self.list(capture, ProfilerListKindV4::DurationHotspots, page)
            }
            AgentProfilerRequestV1::ListWaits { capture, page, .. } => {
                self.list(capture, ProfilerListKindV4::Waits, page)
            }
            AgentProfilerRequestV1::InspectDispatch {
                capture, dispatch, ..
            } => self.inspect_dispatch(capture, dispatch, false),
            AgentProfilerRequestV1::InspectKernel {
                capture, dispatch, ..
            } => self.inspect_dispatch(capture, dispatch, true),
            AgentProfilerRequestV1::CompareCaptures {
                baseline,
                candidate,
                ..
            } => self.compare(baseline, candidate),
            AgentProfilerRequestV1::PlanNextCapture {
                capture, goal: _, ..
            } => self.unsupported_capture(
                capture,
                AgentProfilerOperationV1::PlanNextCapture,
                AgentProfilerUnavailableReasonV1::CapturePlanRequirementsNotRepresented,
                &[],
            ),
            AgentProfilerRequestV1::ExplainRegression {
                baseline,
                candidate,
                ..
            } => {
                self.capture(baseline)?;
                self.capture(candidate)?;
                Ok(self.unavailable(
                    AgentProfilerOperationV1::ExplainRegression,
                    AgentProfilerUnavailableReasonV1::CausalExplanationNotEstablished,
                    &[baseline, candidate],
                    &[],
                ))
            }
            AgentProfilerRequestV1::InspectWorkgroup {
                capture, dispatch, ..
            }
            | AgentProfilerRequestV1::InspectWave {
                capture, dispatch, ..
            }
            | AgentProfilerRequestV1::InspectLane {
                capture, dispatch, ..
            } => {
                self.ensure_dispatch(capture, dispatch)?;
                Ok(self.unavailable(
                    operation,
                    AgentProfilerUnavailableReasonV1::WorkgroupWaveLaneHierarchyNotCaptured,
                    &[capture],
                    &[dispatch],
                ))
            }
            AgentProfilerRequestV1::ResolveSourceSite {
                capture, dispatch, ..
            }
            | AgentProfilerRequestV1::CorrelateSourceIrIsa {
                capture, dispatch, ..
            } => {
                self.ensure_dispatch(capture, dispatch)?;
                Ok(self.unavailable(
                    operation,
                    AgentProfilerUnavailableReasonV1::AuthenticatedSourceCorrelationNotCaptured,
                    &[capture],
                    &[dispatch],
                ))
            }
            AgentProfilerRequestV1::ListFaults { capture, .. } => self.unsupported_capture(
                capture,
                AgentProfilerOperationV1::ListFaults,
                AgentProfilerUnavailableReasonV1::NotRepresentedByProfilerBundleV4,
                &[],
            ),
            AgentProfilerRequestV1::InspectMemoryAccess { capture, .. } => self
                .unsupported_capture(
                    capture,
                    AgentProfilerOperationV1::InspectMemoryAccess,
                    AgentProfilerUnavailableReasonV1::DecodedAttEventsNotCaptured,
                    &[],
                ),
            AgentProfilerRequestV1::InspectBarrier { capture, .. } => self.unsupported_capture(
                capture,
                AgentProfilerOperationV1::InspectBarrier,
                AgentProfilerUnavailableReasonV1::DecodedAttEventsNotCaptured,
                &[],
            ),
            AgentProfilerRequestV1::InspectProperty { capture, .. } => self.unsupported_capture(
                capture,
                AgentProfilerOperationV1::InspectProperty,
                AgentProfilerUnavailableReasonV1::NotRepresentedByProfilerBundleV4,
                &[],
            ),
            AgentProfilerRequestV1::ExportReproducer { capture, .. } => self.unsupported_capture(
                capture,
                AgentProfilerOperationV1::ExportReproducer,
                AgentProfilerUnavailableReasonV1::ReproducerInputsNotCaptured,
                &[],
            ),
        }
    }

    fn open_capture(
        &mut self,
        bundle_hex: &str,
    ) -> Result<AgentProfilerResultV1, AgentProfilerErrorCodeV1> {
        let bytes = decode_lower_hex_bounded(bundle_hex, self.limits.query.max_input_bytes)?;
        let session = ProfilerQuerySessionV4::open(&bytes, self.limits.query)
            .map_err(|_| AgentProfilerErrorCodeV1::InvalidBundle)?;
        let (context, coverage) = match session
            .query(ProfilerQueryRequestV4::Open)
            .map_err(map_query_error)?
        {
            ProfilerQueryResponseV4::Open { context, coverage } => (context, coverage),
            _ => return Err(AgentProfilerErrorCodeV1::InternalEvidenceMismatch),
        };
        let capabilities = match session
            .query(ProfilerQueryRequestV4::Capabilities)
            .map_err(map_query_error)?
        {
            ProfilerQueryResponseV4::Capabilities { capabilities, .. } => capabilities,
            _ => return Err(AgentProfilerErrorCodeV1::InternalEvidenceMismatch),
        };
        let key = context.bundle_identity.digest;
        let before = u8::try_from(self.captures.len())
            .map_err(|_| AgentProfilerErrorCodeV1::CaptureLimitReached)?;
        let effect = if let Some(existing) = self.captures.get(&key) {
            if existing.context.bundle_identity != context.bundle_identity
                || existing.bytes != bytes
            {
                return Err(AgentProfilerErrorCodeV1::InvalidBundle);
            }
            AgentProfilerOpenEffectV1::AlreadyOpen
        } else {
            if self.captures.len() >= self.limits.max_open_captures as usize {
                return Err(AgentProfilerErrorCodeV1::CaptureLimitReached);
            }
            self.captures.insert(
                key,
                OpenCaptureV1 {
                    bytes,
                    session,
                    context,
                    coverage,
                    capabilities: capabilities.clone(),
                },
            );
            AgentProfilerOpenEffectV1::Registered
        };
        let after = u8::try_from(self.captures.len())
            .map_err(|_| AgentProfilerErrorCodeV1::CaptureLimitReached)?;
        Ok(AgentProfilerResultV1::CaptureOpened {
            context,
            coverage,
            capture_capabilities: capabilities,
            audit: AgentProfilerOpenAuditV1 {
                before_open_captures: before,
                after_open_captures: after,
                effect,
            },
            evidence: self.evidence(TruthOriginV1::Observed, &[context.bundle_identity], &[]),
        })
    }

    fn list(
        &self,
        capture: ContentIdentityRecordV1,
        kind: ProfilerListKindV4,
        page_request: ProfilerPageRequestV4,
    ) -> Result<AgentProfilerResultV1, AgentProfilerErrorCodeV1> {
        let open = self.capture(capture)?;
        let page = match open
            .session
            .query(ProfilerQueryRequestV4::List {
                kind,
                page: page_request,
            })
            .map_err(map_query_error)?
        {
            ProfilerQueryResponseV4::Page { page } => page,
            _ => return Err(AgentProfilerErrorCodeV1::InternalEvidenceMismatch),
        };
        let evidence = self.page_evidence(capture, &page);
        Ok(AgentProfilerResultV1::Page { page, evidence })
    }

    fn inspect_dispatch(
        &self,
        capture: ContentIdentityRecordV1,
        dispatch: CaptureIdentityV1,
        kernel_only: bool,
    ) -> Result<AgentProfilerResultV1, AgentProfilerErrorCodeV1> {
        let open = self.capture(capture)?;
        let (context, value) = match open
            .session
            .query(ProfilerQueryRequestV4::InspectDispatch { identity: dispatch })
            .map_err(map_query_error)?
        {
            ProfilerQueryResponseV4::InspectDispatch {
                context, dispatch, ..
            } => (context, dispatch),
            _ => return Err(AgentProfilerErrorCodeV1::InternalEvidenceMismatch),
        };
        if kernel_only {
            Ok(AgentProfilerResultV1::Kernel {
                inspection: AgentProfilerKernelInspectionV1 {
                    context,
                    dispatch_identity: value.identity,
                    kernel_ir: value.kernel_ir,
                    artifact: value.artifact,
                    source_map: value.source_map,
                    scope: AgentProfilerKernelScopeV1::DispatchBindingOnly,
                },
                evidence: self.evidence(TruthOriginV1::Declared, &[capture], &[dispatch]),
            })
        } else {
            Ok(AgentProfilerResultV1::Dispatch {
                context,
                dispatch: value,
                evidence: self.evidence(TruthOriginV1::Observed, &[capture], &[dispatch]),
            })
        }
    }

    fn compare(
        &self,
        baseline: ContentIdentityRecordV1,
        candidate: ContentIdentityRecordV1,
    ) -> Result<AgentProfilerResultV1, AgentProfilerErrorCodeV1> {
        let baseline_open = self.capture(baseline)?;
        let candidate_open = self.capture(candidate)?;
        let comparison = compare_profiler_bundles_v4(&baseline_open.bytes, &candidate_open.bytes)
            .map_err(map_query_error)?;
        encode_profiler_bundle_comparison_v4(&comparison).map_err(map_query_error)?;
        Ok(AgentProfilerResultV1::Comparison {
            comparison,
            evidence: self.evidence(TruthOriginV1::Inferred, &[baseline, candidate], &[]),
        })
    }

    fn unsupported_capture(
        &self,
        capture: ContentIdentityRecordV1,
        operation: AgentProfilerOperationV1,
        reason: AgentProfilerUnavailableReasonV1,
        records: &[CaptureIdentityV1],
    ) -> Result<AgentProfilerResultV1, AgentProfilerErrorCodeV1> {
        self.capture(capture)?;
        Ok(self.unavailable(operation, reason, &[capture], records))
    }

    fn unavailable(
        &self,
        operation: AgentProfilerOperationV1,
        reason: AgentProfilerUnavailableReasonV1,
        captures: &[ContentIdentityRecordV1],
        records: &[CaptureIdentityV1],
    ) -> AgentProfilerResultV1 {
        AgentProfilerResultV1::Unavailable {
            operation,
            reason,
            evidence: self.evidence(TruthOriginV1::Unavailable, captures, records),
        }
    }

    fn evidence(
        &self,
        origin: TruthOriginV1,
        captures: &[ContentIdentityRecordV1],
        records: &[CaptureIdentityV1],
    ) -> AgentProfilerEvidenceV1 {
        AgentProfilerEvidenceV1 {
            origin: AgentProfilerAggregateOriginV1::Homogeneous { origin },
            service_contract: self.contract,
            captures: captures.to_vec(),
            records: records.to_vec(),
        }
    }

    fn page_evidence(
        &self,
        capture: ContentIdentityRecordV1,
        page: &ProfilerPageV4,
    ) -> AgentProfilerEvidenceV1 {
        let mut origins = page
            .items
            .iter()
            .map(profiler_item_origin)
            .collect::<Vec<_>>();
        origins.sort_unstable_by_key(|origin| truth_origin_rank(*origin));
        origins.dedup();
        let origin = match origins.as_slice() {
            [] => AgentProfilerAggregateOriginV1::Empty,
            [origin] => AgentProfilerAggregateOriginV1::Homogeneous { origin: *origin },
            _ => AgentProfilerAggregateOriginV1::Mixed { origins },
        };
        AgentProfilerEvidenceV1 {
            origin,
            service_contract: self.contract,
            captures: vec![capture],
            records: Vec::new(),
        }
    }

    fn capture(
        &self,
        identity: ContentIdentityRecordV1,
    ) -> Result<&OpenCaptureV1, AgentProfilerErrorCodeV1> {
        self.captures
            .get(&identity.digest)
            .filter(|capture| capture.context.bundle_identity == identity)
            .ok_or(AgentProfilerErrorCodeV1::CaptureNotOpen)
    }

    fn ensure_dispatch(
        &self,
        capture: ContentIdentityRecordV1,
        dispatch: CaptureIdentityV1,
    ) -> Result<(), AgentProfilerErrorCodeV1> {
        self.capture(capture)?
            .session
            .query(ProfilerQueryRequestV4::InspectDispatch { identity: dispatch })
            .map_err(map_query_error)?;
        Ok(())
    }

    fn ok(&mut self, request_id: u64, value: AgentProfilerResultV1) -> AgentProfilerResponseV1 {
        let response_revision = self.next_response_revision();
        let mut response = AgentProfilerResponseV1::Ok {
            schema: AGENT_PROFILER_RESPONSE_SCHEMA_V1,
            request_id,
            response_revision,
            value: Box::new(value),
            state_binding: AgentProfilerResponseBindingV1([0; 32]),
        };
        self.bind_response(&mut response);
        response
    }

    fn error(
        &mut self,
        request_id: Option<u64>,
        code: AgentProfilerErrorCodeV1,
        terminal: bool,
    ) -> AgentProfilerResponseV1 {
        let response_revision = self.next_response_revision();
        let mut response = AgentProfilerResponseV1::Error {
            schema: AGENT_PROFILER_RESPONSE_SCHEMA_V1,
            request_id,
            response_revision,
            code,
            terminal,
            state_binding: AgentProfilerResponseBindingV1([0; 32]),
        };
        self.bind_response(&mut response);
        if terminal {
            self.terminal_response = Some(response.clone());
        }
        response
    }

    fn next_response_revision(&mut self) -> u64 {
        self.response_revision = self
            .response_revision
            .checked_add(1)
            .expect("bounded service requests keep response revisions representable");
        self.response_revision
    }

    fn bind_response(&mut self, response: &mut AgentProfilerResponseV1) {
        let binding = self.calculate_response_binding(response);
        let revision = match response {
            AgentProfilerResponseV1::Ok {
                response_revision,
                state_binding,
                ..
            }
            | AgentProfilerResponseV1::Error {
                response_revision,
                state_binding,
                ..
            } => {
                *state_binding = binding;
                *response_revision
            }
        };
        let previous = self.response_bindings.insert(revision, binding);
        debug_assert!(previous.is_none());
    }

    fn calculate_response_binding(
        &self,
        response: &AgentProfilerResponseV1,
    ) -> AgentProfilerResponseBindingV1 {
        let mut digest = Sha256::new();
        digest.update(AGENT_PROFILER_RESPONSE_BINDING_DOMAIN_V1);
        digest.update(self.contract.digest.as_bytes());
        serde_json::to_writer(AgentDigestWriterV1(&mut digest), response)
            .expect("hash-backed response writer cannot fail");
        AgentProfilerResponseBindingV1(digest.finalize().into())
    }

    fn validate_response(
        &self,
        response: &AgentProfilerResponseV1,
    ) -> Result<(), AgentProfilerServiceErrorV1> {
        let (response_revision, state_binding) = match response {
            AgentProfilerResponseV1::Ok {
                response_revision,
                state_binding,
                ..
            }
            | AgentProfilerResponseV1::Error {
                response_revision,
                state_binding,
                ..
            } => (*response_revision, *state_binding),
        };
        if self.response_bindings.get(&response_revision) != Some(&state_binding)
            || self.calculate_response_binding(response) != state_binding
        {
            return Err(AgentProfilerServiceErrorV1::InvalidResponse);
        }
        match response {
            AgentProfilerResponseV1::Error {
                schema,
                request_id,
                response_revision,
                code,
                terminal,
                ..
            } => {
                let terminal_valid = !*terminal
                    || matches!(
                        code,
                        AgentProfilerErrorCodeV1::RequestBudgetExhausted
                            | AgentProfilerErrorCodeV1::RequestTooLarge
                            | AgentProfilerErrorCodeV1::InvalidRequest
                            | AgentProfilerErrorCodeV1::ResponseTooLarge
                            | AgentProfilerErrorCodeV1::InternalEvidenceMismatch
                    );
                if *schema != AGENT_PROFILER_RESPONSE_SCHEMA_V1
                    || *response_revision == 0
                    || request_id.is_some_and(|id| id == 0)
                    || !terminal_valid
                {
                    return Err(AgentProfilerServiceErrorV1::InvalidResponse);
                }
            }
            AgentProfilerResponseV1::Ok {
                schema,
                request_id,
                response_revision,
                value,
                ..
            } => {
                if *schema != AGENT_PROFILER_RESPONSE_SCHEMA_V1
                    || *request_id == 0
                    || *response_revision == 0
                {
                    return Err(AgentProfilerServiceErrorV1::InvalidResponse);
                }
                self.validate_result(value.as_ref())?;
            }
        }
        Ok(())
    }

    fn validate_result(
        &self,
        value: &AgentProfilerResultV1,
    ) -> Result<(), AgentProfilerServiceErrorV1> {
        let evidence = match value {
            AgentProfilerResultV1::Capabilities {
                capabilities,
                limits,
                evidence,
            } => {
                if *capabilities != agent_capabilities()
                    || *limits != self.limits.view()
                    || !evidence_has_origin(evidence, TruthOriginV1::Declared)
                    || !evidence.captures.is_empty()
                    || !evidence.records.is_empty()
                {
                    return Err(AgentProfilerServiceErrorV1::InvalidResponse);
                }
                evidence
            }
            AgentProfilerResultV1::CaptureOpened {
                context,
                coverage,
                capture_capabilities,
                audit,
                evidence,
            } => {
                let open = self
                    .capture(context.bundle_identity)
                    .map_err(|_| AgentProfilerServiceErrorV1::InvalidResponse)?;
                let audit_valid = match audit.effect {
                    AgentProfilerOpenEffectV1::Registered => {
                        audit.before_open_captures.checked_add(1) == Some(audit.after_open_captures)
                    }
                    AgentProfilerOpenEffectV1::AlreadyOpen => {
                        audit.before_open_captures == audit.after_open_captures
                    }
                };
                if open.context != *context
                    || open.coverage != *coverage
                    || open.capabilities != *capture_capabilities
                    || !audit_valid
                    || audit.after_open_captures as usize > self.captures.len()
                    || evidence.captures.as_slice() != [context.bundle_identity]
                    || !evidence_has_origin(evidence, TruthOriginV1::Observed)
                    || !evidence.records.is_empty()
                {
                    return Err(AgentProfilerServiceErrorV1::InvalidResponse);
                }
                evidence
            }
            AgentProfilerResultV1::Page { page, evidence } => {
                let capture = exactly_one_capture(evidence)?;
                let open = self
                    .capture(capture)
                    .map_err(|_| AgentProfilerServiceErrorV1::InvalidResponse)?;
                open.session
                    .encode_response(&ProfilerQueryResponseV4::Page { page: page.clone() })
                    .map_err(|_| AgentProfilerServiceErrorV1::InvalidResponse)?;
                if *evidence != self.page_evidence(capture, page) {
                    return Err(AgentProfilerServiceErrorV1::InvalidResponse);
                }
                evidence
            }
            AgentProfilerResultV1::Dispatch {
                context,
                dispatch,
                evidence,
            } => {
                let capture = exactly_one_capture(evidence)?;
                let open = self
                    .capture(capture)
                    .map_err(|_| AgentProfilerServiceErrorV1::InvalidResponse)?;
                open.session
                    .encode_response(&ProfilerQueryResponseV4::InspectDispatch {
                        context: *context,
                        dispatch: dispatch.clone(),
                        evidence: crate::ProfilerEvidenceV4 {
                            origin: TruthOriginV1::Observed,
                            bundle: capture.digest,
                            record: Some(dispatch.identity),
                        },
                    })
                    .map_err(|_| AgentProfilerServiceErrorV1::InvalidResponse)?;
                if evidence.records.as_slice() != [dispatch.identity]
                    || !evidence_has_origin(evidence, TruthOriginV1::Observed)
                {
                    return Err(AgentProfilerServiceErrorV1::InvalidResponse);
                }
                evidence
            }
            AgentProfilerResultV1::Kernel {
                inspection,
                evidence,
            } => {
                let capture = exactly_one_capture(evidence)?;
                let open = self
                    .capture(capture)
                    .map_err(|_| AgentProfilerServiceErrorV1::InvalidResponse)?;
                let response = open
                    .session
                    .query(ProfilerQueryRequestV4::InspectDispatch {
                        identity: inspection.dispatch_identity,
                    })
                    .map_err(|_| AgentProfilerServiceErrorV1::InvalidResponse)?;
                let ProfilerQueryResponseV4::InspectDispatch {
                    context, dispatch, ..
                } = response
                else {
                    return Err(AgentProfilerServiceErrorV1::InvalidResponse);
                };
                if inspection.context != context
                    || inspection.kernel_ir != dispatch.kernel_ir
                    || inspection.artifact != dispatch.artifact
                    || inspection.source_map != dispatch.source_map
                    || evidence.records.as_slice() != [dispatch.identity]
                    || !evidence_has_origin(evidence, TruthOriginV1::Declared)
                {
                    return Err(AgentProfilerServiceErrorV1::InvalidResponse);
                }
                evidence
            }
            AgentProfilerResultV1::Comparison {
                comparison,
                evidence,
            } => {
                if evidence.captures.len() != 2 {
                    return Err(AgentProfilerServiceErrorV1::InvalidResponse);
                }
                let baseline = self
                    .capture(evidence.captures[0])
                    .map_err(|_| AgentProfilerServiceErrorV1::InvalidResponse)?;
                let candidate = self
                    .capture(evidence.captures[1])
                    .map_err(|_| AgentProfilerServiceErrorV1::InvalidResponse)?;
                let expected = compare_profiler_bundles_v4(&baseline.bytes, &candidate.bytes)
                    .map_err(|_| AgentProfilerServiceErrorV1::InvalidResponse)?;
                if *comparison != expected {
                    return Err(AgentProfilerServiceErrorV1::InvalidResponse);
                }
                if !evidence_has_origin(evidence, TruthOriginV1::Inferred)
                    || !evidence.records.is_empty()
                {
                    return Err(AgentProfilerServiceErrorV1::InvalidResponse);
                }
                evidence
            }
            AgentProfilerResultV1::Unavailable {
                operation,
                reason,
                evidence,
            } => {
                for capture in &evidence.captures {
                    self.capture(*capture)
                        .map_err(|_| AgentProfilerServiceErrorV1::InvalidResponse)?;
                }
                if !evidence_has_origin(evidence, TruthOriginV1::Unavailable)
                    || evidence.captures.is_empty()
                    || unavailable_reason(*operation) != Some(*reason)
                {
                    return Err(AgentProfilerServiceErrorV1::InvalidResponse);
                }
                match operation {
                    AgentProfilerOperationV1::InspectWorkgroup
                    | AgentProfilerOperationV1::InspectWave
                    | AgentProfilerOperationV1::InspectLane
                    | AgentProfilerOperationV1::ResolveSourceSite
                    | AgentProfilerOperationV1::CorrelateSourceIrIsa => {
                        let capture = exactly_one_capture(evidence)?;
                        let [dispatch] = evidence.records.as_slice() else {
                            return Err(AgentProfilerServiceErrorV1::InvalidResponse);
                        };
                        self.ensure_dispatch(capture, *dispatch)
                            .map_err(|_| AgentProfilerServiceErrorV1::InvalidResponse)?;
                    }
                    _ if !evidence.records.is_empty() => {
                        return Err(AgentProfilerServiceErrorV1::InvalidResponse);
                    }
                    _ => {}
                }
                evidence
            }
        };
        if evidence.service_contract != self.contract
            || evidence.captures.len() > usize::from(MAX_AGENT_PROFILER_OPEN_CAPTURES_V1)
            || evidence.records.len() > usize::from(self.limits.query.max_page_items)
        {
            return Err(AgentProfilerServiceErrorV1::InvalidResponse);
        }
        Ok(())
    }
}

fn exactly_one_capture(
    evidence: &AgentProfilerEvidenceV1,
) -> Result<ContentIdentityRecordV1, AgentProfilerServiceErrorV1> {
    match evidence.captures.as_slice() {
        [capture] => Ok(*capture),
        _ => Err(AgentProfilerServiceErrorV1::InvalidResponse),
    }
}

fn evidence_has_origin(evidence: &AgentProfilerEvidenceV1, expected: TruthOriginV1) -> bool {
    evidence.origin == AgentProfilerAggregateOriginV1::Homogeneous { origin: expected }
}

fn profiler_item_origin(item: &ProfilerQueryItemV4) -> TruthOriginV1 {
    match item {
        ProfilerQueryItemV4::Run { evidence, .. }
        | ProfilerQueryItemV4::Device { evidence, .. }
        | ProfilerQueryItemV4::AttReference { evidence, .. }
        | ProfilerQueryItemV4::Unavailable { evidence, .. } => evidence.origin,
        ProfilerQueryItemV4::Dispatch { dispatch } => dispatch.evidence.origin,
        ProfilerQueryItemV4::DurationHotspot { hotspot } => hotspot.origin,
    }
}

const fn truth_origin_rank(origin: TruthOriginV1) -> u8 {
    match origin {
        TruthOriginV1::Declared => 0,
        TruthOriginV1::Proved => 1,
        TruthOriginV1::Observed => 2,
        TruthOriginV1::Inferred => 3,
        TruthOriginV1::Unavailable => 4,
    }
}

fn agent_capabilities() -> Vec<AgentProfilerCapabilityV1> {
    AgentProfilerOperationV1::ALL
        .into_iter()
        .map(|operation| {
            let (state, unavailable_reason) = match operation {
                AgentProfilerOperationV1::DiscoverCapabilities
                | AgentProfilerOperationV1::OpenCapture => {
                    (AgentProfilerCapabilityStateV1::Available, None)
                }
                AgentProfilerOperationV1::ListRuns
                | AgentProfilerOperationV1::ListDevices
                | AgentProfilerOperationV1::ListDispatches
                | AgentProfilerOperationV1::ListAttReferences
                | AgentProfilerOperationV1::ListHotspots
                | AgentProfilerOperationV1::InspectDispatch
                | AgentProfilerOperationV1::InspectKernel
                | AgentProfilerOperationV1::CompareCaptures => {
                    (AgentProfilerCapabilityStateV1::CaptureDependent, None)
                }
                AgentProfilerOperationV1::ListWaits
                | AgentProfilerOperationV1::InspectMemoryAccess
                | AgentProfilerOperationV1::InspectBarrier => (
                    AgentProfilerCapabilityStateV1::Unavailable,
                    Some(AgentProfilerUnavailableReasonV1::DecodedAttEventsNotCaptured),
                ),
                AgentProfilerOperationV1::InspectWorkgroup
                | AgentProfilerOperationV1::InspectWave
                | AgentProfilerOperationV1::InspectLane => (
                    AgentProfilerCapabilityStateV1::Unavailable,
                    Some(AgentProfilerUnavailableReasonV1::WorkgroupWaveLaneHierarchyNotCaptured),
                ),
                AgentProfilerOperationV1::ResolveSourceSite
                | AgentProfilerOperationV1::CorrelateSourceIrIsa => (
                    AgentProfilerCapabilityStateV1::Unavailable,
                    Some(
                        AgentProfilerUnavailableReasonV1::AuthenticatedSourceCorrelationNotCaptured,
                    ),
                ),
                AgentProfilerOperationV1::ExplainRegression => (
                    AgentProfilerCapabilityStateV1::Unavailable,
                    Some(AgentProfilerUnavailableReasonV1::CausalExplanationNotEstablished),
                ),
                AgentProfilerOperationV1::PlanNextCapture => (
                    AgentProfilerCapabilityStateV1::Unavailable,
                    Some(AgentProfilerUnavailableReasonV1::CapturePlanRequirementsNotRepresented),
                ),
                AgentProfilerOperationV1::ExportReproducer => (
                    AgentProfilerCapabilityStateV1::Unavailable,
                    Some(AgentProfilerUnavailableReasonV1::ReproducerInputsNotCaptured),
                ),
                AgentProfilerOperationV1::ListFaults
                | AgentProfilerOperationV1::InspectProperty => (
                    AgentProfilerCapabilityStateV1::Unavailable,
                    Some(AgentProfilerUnavailableReasonV1::NotRepresentedByProfilerBundleV4),
                ),
            };
            AgentProfilerCapabilityV1 {
                operation,
                state,
                unavailable_reason,
            }
        })
        .collect()
}

fn unavailable_reason(
    operation: AgentProfilerOperationV1,
) -> Option<AgentProfilerUnavailableReasonV1> {
    match operation {
        AgentProfilerOperationV1::ListWaits
        | AgentProfilerOperationV1::InspectMemoryAccess
        | AgentProfilerOperationV1::InspectBarrier => {
            Some(AgentProfilerUnavailableReasonV1::DecodedAttEventsNotCaptured)
        }
        AgentProfilerOperationV1::InspectWorkgroup
        | AgentProfilerOperationV1::InspectWave
        | AgentProfilerOperationV1::InspectLane => {
            Some(AgentProfilerUnavailableReasonV1::WorkgroupWaveLaneHierarchyNotCaptured)
        }
        AgentProfilerOperationV1::ResolveSourceSite
        | AgentProfilerOperationV1::CorrelateSourceIrIsa => {
            Some(AgentProfilerUnavailableReasonV1::AuthenticatedSourceCorrelationNotCaptured)
        }
        AgentProfilerOperationV1::ExplainRegression => {
            Some(AgentProfilerUnavailableReasonV1::CausalExplanationNotEstablished)
        }
        AgentProfilerOperationV1::PlanNextCapture => {
            Some(AgentProfilerUnavailableReasonV1::CapturePlanRequirementsNotRepresented)
        }
        AgentProfilerOperationV1::ExportReproducer => {
            Some(AgentProfilerUnavailableReasonV1::ReproducerInputsNotCaptured)
        }
        AgentProfilerOperationV1::ListFaults | AgentProfilerOperationV1::InspectProperty => {
            Some(AgentProfilerUnavailableReasonV1::NotRepresentedByProfilerBundleV4)
        }
        _ => None,
    }
}

fn service_contract_identity() -> Result<ContentIdentityRecordV1, AgentProfilerServiceErrorV1> {
    let mut hasher = Sha256::new();
    hasher.update(AGENT_PROFILER_CONTRACT_DOMAIN_V1);
    hasher.update(AGENT_PROFILER_CONTRACT_BYTES_V1);
    let digest = CaptureIdentityV1::new(hasher.finalize().into())
        .map_err(|_| AgentProfilerServiceErrorV1::Identity)?;
    Ok(ContentIdentityRecordV1 {
        scheme: ContentSchemeV1::DomainSeparatedSha256,
        format_version: 1,
        digest,
        canonical_len: AGENT_PROFILER_CONTRACT_BYTES_V1.len() as u64,
    })
}

fn decode_lower_hex_bounded(
    value: &str,
    max_bundle_bytes: u64,
) -> Result<Vec<u8>, AgentProfilerErrorCodeV1> {
    if value.is_empty() || !value.len().is_multiple_of(2) {
        return Err(AgentProfilerErrorCodeV1::InvalidBundleEncoding);
    }
    let decoded_len = value.len() / 2;
    if u64::try_from(decoded_len).map_or(true, |length| {
        length > max_bundle_bytes || length > MAX_PROFILER_BUNDLE_BYTES_V4
    }) {
        return Err(AgentProfilerErrorCodeV1::BundleTooLarge);
    }
    let mut output = Vec::new();
    output
        .try_reserve_exact(decoded_len)
        .map_err(|_| AgentProfilerErrorCodeV1::BundleTooLarge)?;
    for pair in value.as_bytes().chunks_exact(2) {
        let high =
            lower_hex_nibble(pair[0]).ok_or(AgentProfilerErrorCodeV1::InvalidBundleEncoding)?;
        let low =
            lower_hex_nibble(pair[1]).ok_or(AgentProfilerErrorCodeV1::InvalidBundleEncoding)?;
        output.push((high << 4) | low);
    }
    Ok(output)
}

fn lower_hex_nibble(value: u8) -> Option<u8> {
    match value {
        b'0'..=b'9' => Some(value - b'0'),
        b'a'..=b'f' => Some(value - b'a' + 10),
        _ => None,
    }
}

fn map_query_error(error: ProfilerQueryErrorV4) -> AgentProfilerErrorCodeV1 {
    match error {
        ProfilerQueryErrorV4::PageLimitOutOfRange
        | ProfilerQueryErrorV4::CursorMismatch
        | ProfilerQueryErrorV4::CursorOutOfRange => AgentProfilerErrorCodeV1::InvalidPage,
        ProfilerQueryErrorV4::DispatchNotFound => AgentProfilerErrorCodeV1::RecordNotFound,
        ProfilerQueryErrorV4::ResponseTooLarge => AgentProfilerErrorCodeV1::ResponseTooLarge,
        ProfilerQueryErrorV4::Bundle => AgentProfilerErrorCodeV1::InvalidBundle,
        ProfilerQueryErrorV4::InputTooLarge => AgentProfilerErrorCodeV1::BundleTooLarge,
        _ => AgentProfilerErrorCodeV1::InternalEvidenceMismatch,
    }
}

pub fn decode_agent_profiler_request_line_v1(
    line: &[u8],
) -> Result<AgentProfilerRequestV1, AgentProfilerServiceErrorV1> {
    if line.is_empty() || line.len() as u64 > MAX_AGENT_PROFILER_REQUEST_BYTES_V1 {
        return Err(AgentProfilerServiceErrorV1::RequestTooLarge);
    }
    let payload = line.strip_suffix(b"\n").unwrap_or(line);
    if payload.is_empty()
        || payload.contains(&b'\n')
        || payload.contains(&b'\r')
        || payload.len() as u64 >= MAX_AGENT_PROFILER_REQUEST_BYTES_V1
    {
        return Err(AgentProfilerServiceErrorV1::InvalidRequest);
    }
    serde_json::from_slice(payload).map_err(|_| AgentProfilerServiceErrorV1::InvalidRequest)
}

pub fn read_agent_profiler_request_line_v1<R: BufRead>(
    reader: &mut R,
) -> Result<Option<Vec<u8>>, AgentProfilerServiceErrorV1> {
    let mut line = Vec::new();
    let max = usize::try_from(MAX_AGENT_PROFILER_REQUEST_BYTES_V1)
        .map_err(|_| AgentProfilerServiceErrorV1::SizeOverflow)?;
    loop {
        let buffer = reader
            .fill_buf()
            .map_err(|_| AgentProfilerServiceErrorV1::Io)?;
        if buffer.is_empty() {
            return if line.is_empty() {
                Ok(None)
            } else {
                Err(AgentProfilerServiceErrorV1::InvalidRequest)
            };
        }
        let newline = buffer.iter().position(|byte| *byte == b'\n');
        let available = newline.map_or(buffer.len(), |position| position + 1);
        let remaining = max.saturating_add(1).saturating_sub(line.len());
        let consumed = available.min(remaining);
        line.extend_from_slice(&buffer[..consumed]);
        reader.consume(consumed);
        if line.len() > max {
            return Err(AgentProfilerServiceErrorV1::RequestTooLarge);
        }
        if newline.is_some() && consumed == available {
            return Ok(Some(line));
        }
    }
}

fn encode_response_bounded(
    response: &AgentProfilerResponseV1,
    max_response_bytes: u64,
) -> Result<Vec<u8>, AgentProfilerServiceErrorV1> {
    let mut output = Vec::new();
    let mut writer = AgentBoundedWriterV1 {
        output: &mut output,
        max: max_response_bytes,
        exceeded: false,
    };
    serde_json::to_writer(&mut writer, response).map_err(|_| {
        if writer.exceeded {
            AgentProfilerServiceErrorV1::ResponseTooLarge
        } else {
            AgentProfilerServiceErrorV1::JsonEncode
        }
    })?;
    output.push(b'\n');
    Ok(output)
}

struct AgentBoundedWriterV1<'a> {
    output: &'a mut Vec<u8>,
    max: u64,
    exceeded: bool,
}

struct AgentDigestWriterV1<'a>(&'a mut Sha256);

impl Write for AgentDigestWriterV1<'_> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.0.update(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl Write for AgentBoundedWriterV1<'_> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.write_all(bytes)?;
        Ok(bytes.len())
    }

    fn write_all(&mut self, bytes: &[u8]) -> io::Result<()> {
        let max = usize::try_from(self.max).unwrap_or(usize::MAX);
        if self
            .output
            .len()
            .checked_add(bytes.len())
            .is_none_or(|len| len >= max)
        {
            self.exceeded = true;
            return Err(io::Error::other("agent profiler response limit exceeded"));
        }
        self.output.extend_from_slice(bytes);
        Ok(())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[derive(Debug)]
pub enum AgentProfilerServiceErrorV1 {
    LimitOutOfRange,
    RequestTooLarge,
    InvalidRequest,
    InvalidResponse,
    ResponseTooLarge,
    JsonEncode,
    Identity,
    RevisionOverflow,
    SizeOverflow,
    Io,
}

impl fmt::Display for AgentProfilerServiceErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "agent profiler service rejected input: {self:?}")
    }
}

impl Error for AgentProfilerServiceErrorV1 {}
