use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;
use std::io::{self, BufRead, Write};

use fe2o3_semantic_import::{
    CaptureDispatchV1, CaptureIdentityV1, ContentIdentityRecordV1, ContentSchemeV1, IdentityFactV1,
    KernelIrClaimRecordV1, MAX_PROFILER_BUNDLE_BYTES_V4, ProfilerCoverageV4, ProfilerSourceKindV4,
    TruthOriginV1,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{
    ProfilerBundleComparisonV4, ProfilerCapabilityV4, ProfilerListKindV4, ProfilerPageRequestV4,
    ProfilerPageV4, ProfilerQueryContextV4, ProfilerQueryErrorV4, ProfilerQueryItemV4,
    ProfilerQueryLimitsV4, ProfilerQueryRequestV4, ProfilerQueryResponseV4, ProfilerQuerySessionV4,
    compare_profiler_bundles_v4, encode_profiler_bundle_comparison_v4,
};

pub const AGENT_PROFILER_REQUEST_SCHEMA_V1: &str = "fe2o3-agent-profiler-request-v1";
pub const AGENT_PROFILER_RESPONSE_SCHEMA_V1: &str = "fe2o3-agent-profiler-response-v1";
pub const MAX_AGENT_PROFILER_REQUEST_BYTES_V1: u64 = 34 * 1024 * 1024;
pub const MAX_AGENT_PROFILER_RESPONSE_BYTES_V1: u64 = 2 * 1024 * 1024;
pub const MAX_AGENT_PROFILER_REQUESTS_V1: u32 = 4_096;
pub const MAX_AGENT_PROFILER_OPEN_CAPTURES_V1: u8 = 16;
pub const DEFAULT_AGENT_PROFILER_OPEN_CAPTURES_V1: u8 = 4;
pub const AGENT_PROFILER_PLAN_REQUEST_SCHEMA_V1: &str = "fe2o3-agent-profiler-plan-request-v1";
pub const AGENT_PROFILER_PLAN_SCHEMA_V1: &str = "fe2o3-agent-profiler-plan-v1";
pub const MAX_AGENT_PROFILER_PLAN_MISSING_FACTS_V1: usize = 8;
pub const MAX_AGENT_PROFILER_PLAN_COMPUTE_UNITS_V1: usize = 64;
pub const MAX_AGENT_PROFILER_PLAN_STORAGE_BYTES_V1: u64 = 1 << 40;
pub const MAX_AGENT_PROFILER_PLAN_RECORDS_V1: u64 = 1 << 32;
pub const MAX_AGENT_PROFILER_PLAN_OVERHEAD_BASIS_POINTS_V1: u32 = 1_000_000;

const AGENT_PROFILER_CONTRACT_DOMAIN_V1: &[u8] = b"fe2o3.agent-profiler.contract.v1\0";
const AGENT_PROFILER_CONTRACT_BYTES_V1: &[u8] =
    b"read-only;bundle-v4;bounded-jsonl;plan-v1;no-execution-authority";
const AGENT_PROFILER_RESPONSE_BINDING_DOMAIN_V1: &[u8] =
    b"fe2o3.agent-profiler.response-binding.v1\0";
const AGENT_PROFILER_PLAN_REQUEST_DOMAIN_V1: &[u8] = b"fe2o3.agent-profiler.plan-request.v1\0";

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
    RankedExplanationRequiresCausalCounterOrDecodedEventEvidence,
    CapturePlanRequirementsNotRepresented,
    ReproducerInputsNotCaptured,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub struct AgentProfilerCapabilityV1 {
    pub operation: AgentProfilerOperationV1,
    pub state: AgentProfilerCapabilityStateV1,
    pub unavailable_reason: Option<AgentProfilerUnavailableReasonV1>,
    pub request_contract_schema: Option<&'static str>,
    pub result_contract_schema: Option<&'static str>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub struct AgentProfilerLimitsViewV1 {
    pub max_request_bytes: u64,
    pub max_response_bytes: u64,
    pub max_requests: u32,
    pub max_open_captures: u8,
    pub max_page_items: u16,
    pub max_bundle_bytes: u64,
    pub max_plan_missing_facts: u8,
    pub max_plan_compute_units: u8,
    pub max_plan_storage_bytes: u64,
    pub max_plan_records: u64,
    pub max_plan_overhead_basis_points: u32,
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
            max_plan_missing_facts: MAX_AGENT_PROFILER_PLAN_MISSING_FACTS_V1 as u8,
            max_plan_compute_units: MAX_AGENT_PROFILER_PLAN_COMPUTE_UNITS_V1 as u8,
            max_plan_storage_bytes: MAX_AGENT_PROFILER_PLAN_STORAGE_BYTES_V1,
            max_plan_records: MAX_AGENT_PROFILER_PLAN_RECORDS_V1,
            max_plan_overhead_basis_points: MAX_AGENT_PROFILER_PLAN_OVERHEAD_BASIS_POINTS_V1,
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentProfilerPlanGoalV1 {
    AmbiguousCorrectnessDiagnosis,
    ScheduleResourceRegression,
    ExplainWaits,
    DecodeAttCoverage,
    RankDispatchDurations,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentProfilerAmbiguityV1 {
    MemoryFaultVsBarrierDivergence,
    SchedulingDelayVsResourcePressure,
    UnknownWaitCause,
    MissingVsUndecodedAttCoverage,
    DispatchDurationOrdering,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentProfilerPlanEvidenceClassV1 {
    StableEnvironmentBinding,
    KernelIrBinding,
    DispatchEnvelope,
    DispatchTiming,
    AttManifest,
    DecodedMemoryEvents,
    DecodedBarrierEvents,
    DecodedWaitEvents,
    HardwareCounterMeasurements,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AgentProfilerPlanTargetV1 {
    pub compute_units: Vec<u32>,
    pub kernel_ir: Option<CaptureIdentityV1>,
    pub dispatch: Option<CaptureIdentityV1>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentProfilerSelectorValidationV1 {
    NotSpecified,
    ValidatedAgainstCapture,
    CallerDeclaredNotValidatedByBundle,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub struct AgentProfilerTargetValidationV1 {
    pub compute_units: AgentProfilerSelectorValidationV1,
    pub kernel_ir: AgentProfilerSelectorValidationV1,
    pub dispatch: AgentProfilerSelectorValidationV1,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AgentProfilerPlanConstraintsV1 {
    pub maximum_overhead_basis_points: u32,
    pub maximum_storage_bytes: u64,
    pub maximum_records: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AgentProfilerPlanRequestV1 {
    pub schema: String,
    pub goal: AgentProfilerPlanGoalV1,
    pub ambiguity: AgentProfilerAmbiguityV1,
    pub missing_evidence: Vec<AgentProfilerPlanEvidenceClassV1>,
    pub target: AgentProfilerPlanTargetV1,
    pub constraints: AgentProfilerPlanConstraintsV1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentProfilerPlanDispositionV1 {
    AdditionalCaptureRequired,
    AdditionalCaptureRequiredWithUnavailableConfigurationOrPostprocessing,
    ExistingCaptureRequiresUnavailablePostprocessing,
    ExistingEvidenceSufficient,
    BlockedByDeclaredOverheadLimit,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentProfilerDiscriminationMethodV1 {
    DecodedMemoryVsBarrierEventClassification,
    AggregateSchedulerVsResourceCounterContrast,
    DecodedWaitEventClassification,
    AttManifestVsDecodedEventCoverage,
    ObservedDispatchDurationOrdering,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentProfilerCaptureDataClassV1 {
    KernelDispatchEnvelope,
    KernelDispatchTiming,
    AttThreadTrace,
    DecodedMemoryEvents,
    DecodedBarrierEvents,
    DecodedWaitEvents,
    DispatchHardwareCounters,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentProfilerLogicalCounterV1 {
    ActiveWaveOccupancy,
    SchedulerIssueUtilization,
    VectorMemoryStallPressure,
    CacheMissPressure,
    ScratchResourcePressure,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentProfilerCollectorToolV1 {
    Rocprofv3,
    Rocprofv3ComputeViewer,
    Fe2o3SemanticImporter,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentProfilerCollectorCapabilityV1 {
    KernelDispatchCollection,
    LogicalCounterResolution,
    DispatchCounterCollection,
    AttThreadTraceCollection,
    DecodedEventExport,
    StrictDecodedEventImport,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentProfilerCollectorCapabilityStatusV1 {
    RequiredNotVerifiedByCapture,
    RequiredUnavailableInCurrentBuild,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub struct AgentProfilerCollectorRequirementV1 {
    pub tool: AgentProfilerCollectorToolV1,
    pub capability: AgentProfilerCollectorCapabilityV1,
    pub status: AgentProfilerCollectorCapabilityStatusV1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub struct AgentProfilerBoundedU32RangeV1 {
    pub minimum: u32,
    pub maximum: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub struct AgentProfilerBoundedU64RangeV1 {
    pub minimum: u64,
    pub maximum: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentProfilerOverheadMethodV1 {
    ContractDeclaredConservativeEnvelope,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentProfilerOverheadLimitationV1 {
    NotMeasured,
    WorkloadDeviceAndCollectorDependent,
    NotAPerformancePrediction,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct AgentProfilerExpectedOverheadV1 {
    pub origin: TruthOriginV1,
    pub additional_runtime_basis_points: AgentProfilerBoundedU32RangeV1,
    pub method: AgentProfilerOverheadMethodV1,
    pub limitations: Vec<AgentProfilerOverheadLimitationV1>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentProfilerStorageEstimateMethodV1 {
    ExistingBundleBytesScaledBySelectedDataClass,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentProfilerStorageLimitationV1 {
    EstimateNotMeasurement,
    EncodingAndWorkloadDependent,
    MaximumMayTruncateCapture,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct AgentProfilerStoragePlanV1 {
    pub maximum_bytes: u64,
    pub estimated_bytes: AgentProfilerBoundedU64RangeV1,
    pub estimate_origin: TruthOriginV1,
    pub estimate_method: AgentProfilerStorageEstimateMethodV1,
    pub estimate_scale_multiplier: u16,
    pub limitations: Vec<AgentProfilerStorageLimitationV1>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentProfilerPrivilegeRequirementV1 {
    None,
    ProfilerAccess,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentProfilerServiceAuthorityV1 {
    ReadOnlyPlanningOnly,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentProfilerExecutionAuthorizationV1 {
    SeparateExplicitAuthorizationRequired,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentProfilerAttachAuthorityV1 {
    NotAvailableToService,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub struct AgentProfilerAuthorizationBoundaryV1 {
    pub service_authority: AgentProfilerServiceAuthorityV1,
    pub stateful_execution: AgentProfilerExecutionAuthorizationV1,
    pub attach_authority: AgentProfilerAttachAuthorityV1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentProfilerMutualExclusionReasonV1 {
    SeparateInstrumentationCaptureRequired,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub struct AgentProfilerMutualExclusionV1 {
    pub excluded_data_class: AgentProfilerCaptureDataClassV1,
    pub reason: AgentProfilerMutualExclusionReasonV1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentProfilerSamplingModeV1 {
    CollectorReportedDispatchRecords,
    TargetedThreadTrace,
    ExistingThreadTracePostprocessing,
    DispatchCounterAggregate,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentProfilerCompletenessLimitV1 {
    CollectorReportedRecordsOnly,
    SelectedDispatchOnly,
    SelectedComputeUnitsOnly,
    NoFullGridAttClaim,
    AggregateCountersDoNotEstablishCausality,
    StorageCeilingMayTruncate,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct AgentProfilerSamplingCompletenessV1 {
    pub mode: AgentProfilerSamplingModeV1,
    pub maximum_records: u64,
    pub limitations: Vec<AgentProfilerCompletenessLimitV1>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct AgentProfilerCaptureRecipeV1 {
    pub action: AgentProfilerCaptureActionV1,
    pub target: AgentProfilerPlanTargetV1,
    pub target_validation: AgentProfilerTargetValidationV1,
    pub requested_data_classes: Vec<AgentProfilerCaptureDataClassV1>,
    pub requested_logical_counters: Vec<AgentProfilerLogicalCounterV1>,
    pub collector_requirements: Vec<AgentProfilerCollectorRequirementV1>,
    pub expected_overhead: AgentProfilerExpectedOverheadV1,
    pub storage: AgentProfilerStoragePlanV1,
    pub required_privilege: AgentProfilerPrivilegeRequirementV1,
    pub mutual_exclusions: Vec<AgentProfilerMutualExclusionV1>,
    pub sampling_and_completeness: AgentProfilerSamplingCompletenessV1,
    pub authorization: AgentProfilerAuthorizationBoundaryV1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentProfilerCaptureActionV1 {
    NewCaptureRequired,
    PostprocessExistingCapture,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentProfilerPlanProvenanceKindV1 {
    PlanningRequest,
    CaptureBundle,
    DispatchRecord,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub struct AgentProfilerPlanProvenanceV1 {
    pub kind: AgentProfilerPlanProvenanceKindV1,
    pub origin: TruthOriginV1,
    pub identity: CaptureIdentityV1,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct AgentProfilerNextCapturePlanV1 {
    pub schema: &'static str,
    pub request_identity: CaptureIdentityV1,
    pub goal: AgentProfilerPlanGoalV1,
    pub ambiguity: AgentProfilerAmbiguityV1,
    pub discrimination_method: AgentProfilerDiscriminationMethodV1,
    pub target: AgentProfilerPlanTargetV1,
    pub declared_constraints: AgentProfilerPlanConstraintsV1,
    pub disposition: AgentProfilerPlanDispositionV1,
    pub minimum_additional_captures: u8,
    pub already_available_evidence: Vec<AgentProfilerPlanEvidenceClassV1>,
    pub selected_missing_evidence: Vec<AgentProfilerPlanEvidenceClassV1>,
    pub recipe: Option<Box<AgentProfilerCaptureRecipeV1>>,
    pub provenance: Vec<AgentProfilerPlanProvenanceV1>,
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
        planning: AgentProfilerPlanRequestV1,
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
    CapturePlan {
        plan: Box<AgentProfilerNextCapturePlanV1>,
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
    InvalidPlanRequest,
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
                capture, planning, ..
            } => self.plan_next_capture(capture, planning),
            AgentProfilerRequestV1::ExplainRegression {
                baseline,
                candidate,
                ..
            } => {
                self.capture(baseline)?;
                self.capture(candidate)?;
                Ok(self.unavailable(
                    AgentProfilerOperationV1::ExplainRegression,
                    AgentProfilerUnavailableReasonV1::RankedExplanationRequiresCausalCounterOrDecodedEventEvidence,
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

    fn plan_next_capture(
        &self,
        capture: ContentIdentityRecordV1,
        planning: AgentProfilerPlanRequestV1,
    ) -> Result<AgentProfilerResultV1, AgentProfilerErrorCodeV1> {
        let planning = canonicalize_plan_request(planning)?;
        let open = self.capture(capture)?;
        let dispatch = match planning.target.dispatch {
            Some(identity) => {
                let response = open
                    .session
                    .query(ProfilerQueryRequestV4::InspectDispatch { identity })
                    .map_err(map_query_error)?;
                let ProfilerQueryResponseV4::InspectDispatch { dispatch, .. } = response else {
                    return Err(AgentProfilerErrorCodeV1::InternalEvidenceMismatch);
                };
                Some(dispatch)
            }
            None => None,
        };
        if let Some(kernel_ir) = planning.target.kernel_ir {
            let Some(dispatch) = &dispatch else {
                return Err(AgentProfilerErrorCodeV1::InvalidSelector);
            };
            if dispatch.kernel_ir.digest != kernel_ir {
                return Err(AgentProfilerErrorCodeV1::InvalidSelector);
            }
        }
        if goal_requires_dispatch(planning.goal) && dispatch.is_none() {
            return Err(AgentProfilerErrorCodeV1::InvalidSelector);
        }

        let required = required_plan_evidence(planning.goal);
        let available = available_plan_evidence(open, dispatch.as_deref());
        let selected_missing = required
            .iter()
            .copied()
            .filter(|fact| !available.contains(fact))
            .collect::<Vec<_>>();
        if planning.missing_evidence != selected_missing {
            return Err(AgentProfilerErrorCodeV1::InvalidPlanRequest);
        }

        let request_identity = plan_request_identity(&planning)?;
        let (disposition, minimum_additional_captures, recipe) = if selected_missing.is_empty() {
            (
                AgentProfilerPlanDispositionV1::ExistingEvidenceSufficient,
                0,
                None,
            )
        } else {
            let recipe = capture_recipe(
                &planning,
                &selected_missing,
                u64::try_from(open.bytes.len())
                    .map_err(|_| AgentProfilerErrorCodeV1::InvalidPlanRequest)?,
            );
            let minimum_additional_captures = u8::from(matches!(
                recipe.action,
                AgentProfilerCaptureActionV1::NewCaptureRequired
            ));
            let disposition = if recipe
                .expected_overhead
                .additional_runtime_basis_points
                .maximum
                > planning.constraints.maximum_overhead_basis_points
            {
                AgentProfilerPlanDispositionV1::BlockedByDeclaredOverheadLimit
            } else if recipe.collector_requirements.iter().any(|requirement| {
                requirement.status
                    == AgentProfilerCollectorCapabilityStatusV1::RequiredUnavailableInCurrentBuild
            }) {
                if minimum_additional_captures == 0 {
                    AgentProfilerPlanDispositionV1::ExistingCaptureRequiresUnavailablePostprocessing
                } else {
                    AgentProfilerPlanDispositionV1::AdditionalCaptureRequiredWithUnavailableConfigurationOrPostprocessing
                }
            } else {
                AgentProfilerPlanDispositionV1::AdditionalCaptureRequired
            };
            (
                disposition,
                minimum_additional_captures,
                Some(Box::new(recipe)),
            )
        };
        let mut provenance = vec![
            AgentProfilerPlanProvenanceV1 {
                kind: AgentProfilerPlanProvenanceKindV1::PlanningRequest,
                origin: TruthOriginV1::Declared,
                identity: request_identity,
            },
            AgentProfilerPlanProvenanceV1 {
                kind: AgentProfilerPlanProvenanceKindV1::CaptureBundle,
                origin: TruthOriginV1::Observed,
                identity: capture.digest,
            },
        ];
        let mut records = Vec::new();
        if let Some(dispatch) = dispatch {
            provenance.push(AgentProfilerPlanProvenanceV1 {
                kind: AgentProfilerPlanProvenanceKindV1::DispatchRecord,
                origin: TruthOriginV1::Observed,
                identity: dispatch.identity,
            });
            records.push(dispatch.identity);
        }
        let already_available_evidence = required
            .iter()
            .copied()
            .filter(|fact| available.contains(fact))
            .collect();
        Ok(AgentProfilerResultV1::CapturePlan {
            plan: Box::new(AgentProfilerNextCapturePlanV1 {
                schema: AGENT_PROFILER_PLAN_SCHEMA_V1,
                request_identity,
                goal: planning.goal,
                ambiguity: planning.ambiguity,
                discrimination_method: discrimination_method(planning.goal),
                target: planning.target.clone(),
                declared_constraints: planning.constraints,
                disposition,
                minimum_additional_captures,
                already_available_evidence,
                selected_missing_evidence: selected_missing,
                recipe,
                provenance,
            }),
            evidence: AgentProfilerEvidenceV1 {
                origin: AgentProfilerAggregateOriginV1::Mixed {
                    origins: vec![
                        TruthOriginV1::Declared,
                        TruthOriginV1::Observed,
                        TruthOriginV1::Inferred,
                    ],
                },
                service_contract: self.contract,
                captures: vec![capture],
                records,
            },
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
            AgentProfilerResultV1::CapturePlan { plan, evidence } => {
                let capture = exactly_one_capture(evidence)?;
                let expected = self
                    .plan_next_capture(
                        capture,
                        AgentProfilerPlanRequestV1 {
                            schema: AGENT_PROFILER_PLAN_REQUEST_SCHEMA_V1.to_owned(),
                            goal: plan.goal,
                            ambiguity: plan.ambiguity,
                            missing_evidence: plan.selected_missing_evidence.clone(),
                            target: plan.target.clone(),
                            constraints: plan.declared_constraints,
                        },
                    )
                    .map_err(|_| AgentProfilerServiceErrorV1::InvalidResponse)?;
                let AgentProfilerResultV1::CapturePlan {
                    plan: expected_plan,
                    evidence: expected_evidence,
                } = expected
                else {
                    return Err(AgentProfilerServiceErrorV1::InvalidResponse);
                };
                if plan.as_ref() != expected_plan.as_ref() || *evidence != expected_evidence {
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

fn canonicalize_plan_request(
    mut planning: AgentProfilerPlanRequestV1,
) -> Result<AgentProfilerPlanRequestV1, AgentProfilerErrorCodeV1> {
    if planning.schema != AGENT_PROFILER_PLAN_REQUEST_SCHEMA_V1
        || planning.ambiguity != ambiguity_for_goal(planning.goal)
        || planning.missing_evidence.len() > MAX_AGENT_PROFILER_PLAN_MISSING_FACTS_V1
        || planning.target.compute_units.len() > MAX_AGENT_PROFILER_PLAN_COMPUTE_UNITS_V1
        || planning.constraints.maximum_overhead_basis_points
            > MAX_AGENT_PROFILER_PLAN_OVERHEAD_BASIS_POINTS_V1
        || planning.constraints.maximum_storage_bytes == 0
        || planning.constraints.maximum_storage_bytes > MAX_AGENT_PROFILER_PLAN_STORAGE_BYTES_V1
        || planning.constraints.maximum_records == 0
        || planning.constraints.maximum_records > MAX_AGENT_PROFILER_PLAN_RECORDS_V1
    {
        return Err(AgentProfilerErrorCodeV1::InvalidPlanRequest);
    }
    planning.missing_evidence.sort_unstable();
    let missing_count = planning.missing_evidence.len();
    planning.missing_evidence.dedup();
    planning.target.compute_units.sort_unstable();
    let compute_unit_count = planning.target.compute_units.len();
    planning.target.compute_units.dedup();
    if planning.missing_evidence.len() != missing_count
        || planning.target.compute_units.len() != compute_unit_count
    {
        return Err(AgentProfilerErrorCodeV1::InvalidPlanRequest);
    }
    Ok(planning)
}

const fn ambiguity_for_goal(goal: AgentProfilerPlanGoalV1) -> AgentProfilerAmbiguityV1 {
    match goal {
        AgentProfilerPlanGoalV1::AmbiguousCorrectnessDiagnosis => {
            AgentProfilerAmbiguityV1::MemoryFaultVsBarrierDivergence
        }
        AgentProfilerPlanGoalV1::ScheduleResourceRegression => {
            AgentProfilerAmbiguityV1::SchedulingDelayVsResourcePressure
        }
        AgentProfilerPlanGoalV1::ExplainWaits => AgentProfilerAmbiguityV1::UnknownWaitCause,
        AgentProfilerPlanGoalV1::DecodeAttCoverage => {
            AgentProfilerAmbiguityV1::MissingVsUndecodedAttCoverage
        }
        AgentProfilerPlanGoalV1::RankDispatchDurations => {
            AgentProfilerAmbiguityV1::DispatchDurationOrdering
        }
    }
}

const fn discrimination_method(
    goal: AgentProfilerPlanGoalV1,
) -> AgentProfilerDiscriminationMethodV1 {
    match goal {
        AgentProfilerPlanGoalV1::AmbiguousCorrectnessDiagnosis => {
            AgentProfilerDiscriminationMethodV1::DecodedMemoryVsBarrierEventClassification
        }
        AgentProfilerPlanGoalV1::ScheduleResourceRegression => {
            AgentProfilerDiscriminationMethodV1::AggregateSchedulerVsResourceCounterContrast
        }
        AgentProfilerPlanGoalV1::ExplainWaits => {
            AgentProfilerDiscriminationMethodV1::DecodedWaitEventClassification
        }
        AgentProfilerPlanGoalV1::DecodeAttCoverage => {
            AgentProfilerDiscriminationMethodV1::AttManifestVsDecodedEventCoverage
        }
        AgentProfilerPlanGoalV1::RankDispatchDurations => {
            AgentProfilerDiscriminationMethodV1::ObservedDispatchDurationOrdering
        }
    }
}

const fn goal_requires_dispatch(goal: AgentProfilerPlanGoalV1) -> bool {
    matches!(
        goal,
        AgentProfilerPlanGoalV1::AmbiguousCorrectnessDiagnosis
            | AgentProfilerPlanGoalV1::ScheduleResourceRegression
            | AgentProfilerPlanGoalV1::RankDispatchDurations
    )
}

const fn required_plan_evidence(
    goal: AgentProfilerPlanGoalV1,
) -> &'static [AgentProfilerPlanEvidenceClassV1] {
    match goal {
        AgentProfilerPlanGoalV1::AmbiguousCorrectnessDiagnosis => &[
            AgentProfilerPlanEvidenceClassV1::StableEnvironmentBinding,
            AgentProfilerPlanEvidenceClassV1::KernelIrBinding,
            AgentProfilerPlanEvidenceClassV1::DispatchEnvelope,
            AgentProfilerPlanEvidenceClassV1::AttManifest,
            AgentProfilerPlanEvidenceClassV1::DecodedMemoryEvents,
            AgentProfilerPlanEvidenceClassV1::DecodedBarrierEvents,
        ],
        AgentProfilerPlanGoalV1::ScheduleResourceRegression => &[
            AgentProfilerPlanEvidenceClassV1::StableEnvironmentBinding,
            AgentProfilerPlanEvidenceClassV1::KernelIrBinding,
            AgentProfilerPlanEvidenceClassV1::DispatchTiming,
            AgentProfilerPlanEvidenceClassV1::HardwareCounterMeasurements,
        ],
        AgentProfilerPlanGoalV1::ExplainWaits => &[
            AgentProfilerPlanEvidenceClassV1::AttManifest,
            AgentProfilerPlanEvidenceClassV1::DecodedWaitEvents,
        ],
        AgentProfilerPlanGoalV1::DecodeAttCoverage => &[
            AgentProfilerPlanEvidenceClassV1::AttManifest,
            AgentProfilerPlanEvidenceClassV1::DecodedMemoryEvents,
            AgentProfilerPlanEvidenceClassV1::DecodedBarrierEvents,
            AgentProfilerPlanEvidenceClassV1::DecodedWaitEvents,
        ],
        AgentProfilerPlanGoalV1::RankDispatchDurations => {
            &[AgentProfilerPlanEvidenceClassV1::DispatchTiming]
        }
    }
}

fn available_plan_evidence(
    open: &OpenCaptureV1,
    dispatch: Option<&CaptureDispatchV1>,
) -> BTreeSet<AgentProfilerPlanEvidenceClassV1> {
    let mut available =
        BTreeSet::from([AgentProfilerPlanEvidenceClassV1::StableEnvironmentBinding]);
    if dispatch.is_some() {
        available.extend([
            AgentProfilerPlanEvidenceClassV1::KernelIrBinding,
            AgentProfilerPlanEvidenceClassV1::DispatchEnvelope,
            AgentProfilerPlanEvidenceClassV1::DispatchTiming,
        ]);
    }
    if open.context.source_kind == ProfilerSourceKindV4::Rocprofv3AttComputeViewerManifest {
        available.insert(AgentProfilerPlanEvidenceClassV1::AttManifest);
    }
    available
}

fn plan_request_identity(
    planning: &AgentProfilerPlanRequestV1,
) -> Result<CaptureIdentityV1, AgentProfilerErrorCodeV1> {
    let mut digest = Sha256::new();
    digest.update(AGENT_PROFILER_PLAN_REQUEST_DOMAIN_V1);
    serde_json::to_writer(AgentDigestWriterV1(&mut digest), planning)
        .map_err(|_| AgentProfilerErrorCodeV1::InvalidPlanRequest)?;
    CaptureIdentityV1::new(digest.finalize().into())
        .map_err(|_| AgentProfilerErrorCodeV1::InvalidPlanRequest)
}

fn capture_recipe(
    planning: &AgentProfilerPlanRequestV1,
    selected_missing: &[AgentProfilerPlanEvidenceClassV1],
    existing_bundle_bytes: u64,
) -> AgentProfilerCaptureRecipeV1 {
    let (
        requested_data_classes,
        requested_logical_counters,
        collector_requirements,
        action,
        overhead_max,
        storage_multiplier,
        sampling_mode,
        required_privilege,
        excluded_data_class,
    ) = match planning.goal {
        AgentProfilerPlanGoalV1::AmbiguousCorrectnessDiagnosis => (
            vec![
                AgentProfilerCaptureDataClassV1::AttThreadTrace,
                AgentProfilerCaptureDataClassV1::DecodedMemoryEvents,
                AgentProfilerCaptureDataClassV1::DecodedBarrierEvents,
            ],
            Vec::new(),
            att_collector_requirements(true),
            AgentProfilerCaptureActionV1::NewCaptureRequired,
            MAX_AGENT_PROFILER_PLAN_OVERHEAD_BASIS_POINTS_V1,
            64,
            AgentProfilerSamplingModeV1::TargetedThreadTrace,
            AgentProfilerPrivilegeRequirementV1::ProfilerAccess,
            Some(AgentProfilerCaptureDataClassV1::DispatchHardwareCounters),
        ),
        AgentProfilerPlanGoalV1::ScheduleResourceRegression => (
            vec![AgentProfilerCaptureDataClassV1::DispatchHardwareCounters],
            vec![
                AgentProfilerLogicalCounterV1::ActiveWaveOccupancy,
                AgentProfilerLogicalCounterV1::SchedulerIssueUtilization,
                AgentProfilerLogicalCounterV1::VectorMemoryStallPressure,
                AgentProfilerLogicalCounterV1::CacheMissPressure,
                AgentProfilerLogicalCounterV1::ScratchResourcePressure,
            ],
            vec![
                AgentProfilerCollectorRequirementV1 {
                    tool: AgentProfilerCollectorToolV1::Rocprofv3,
                    capability: AgentProfilerCollectorCapabilityV1::LogicalCounterResolution,
                    status:
                        AgentProfilerCollectorCapabilityStatusV1::RequiredUnavailableInCurrentBuild,
                },
                collector_requirement(
                    AgentProfilerCollectorToolV1::Rocprofv3,
                    AgentProfilerCollectorCapabilityV1::DispatchCounterCollection,
                ),
            ],
            AgentProfilerCaptureActionV1::NewCaptureRequired,
            50_000,
            8,
            AgentProfilerSamplingModeV1::DispatchCounterAggregate,
            AgentProfilerPrivilegeRequirementV1::ProfilerAccess,
            Some(AgentProfilerCaptureDataClassV1::AttThreadTrace),
        ),
        AgentProfilerPlanGoalV1::ExplainWaits | AgentProfilerPlanGoalV1::DecodeAttCoverage => {
            let capture_required =
                selected_missing.contains(&AgentProfilerPlanEvidenceClassV1::AttManifest);
            let mut data_classes = capture_required
                .then_some(AgentProfilerCaptureDataClassV1::AttThreadTrace)
                .into_iter()
                .collect::<Vec<_>>();
            for fact in selected_missing {
                let class = match fact {
                    AgentProfilerPlanEvidenceClassV1::DecodedMemoryEvents => {
                        Some(AgentProfilerCaptureDataClassV1::DecodedMemoryEvents)
                    }
                    AgentProfilerPlanEvidenceClassV1::DecodedBarrierEvents => {
                        Some(AgentProfilerCaptureDataClassV1::DecodedBarrierEvents)
                    }
                    AgentProfilerPlanEvidenceClassV1::DecodedWaitEvents => {
                        Some(AgentProfilerCaptureDataClassV1::DecodedWaitEvents)
                    }
                    _ => None,
                };
                if let Some(class) = class {
                    data_classes.push(class);
                }
            }
            (
                data_classes,
                Vec::new(),
                att_collector_requirements(capture_required),
                if capture_required {
                    AgentProfilerCaptureActionV1::NewCaptureRequired
                } else {
                    AgentProfilerCaptureActionV1::PostprocessExistingCapture
                },
                if capture_required {
                    MAX_AGENT_PROFILER_PLAN_OVERHEAD_BASIS_POINTS_V1
                } else {
                    0
                },
                if capture_required { 64 } else { 8 },
                if capture_required {
                    AgentProfilerSamplingModeV1::TargetedThreadTrace
                } else {
                    AgentProfilerSamplingModeV1::ExistingThreadTracePostprocessing
                },
                if capture_required {
                    AgentProfilerPrivilegeRequirementV1::ProfilerAccess
                } else {
                    AgentProfilerPrivilegeRequirementV1::None
                },
                capture_required
                    .then_some(AgentProfilerCaptureDataClassV1::DispatchHardwareCounters),
            )
        }
        AgentProfilerPlanGoalV1::RankDispatchDurations => (
            vec![
                AgentProfilerCaptureDataClassV1::KernelDispatchEnvelope,
                AgentProfilerCaptureDataClassV1::KernelDispatchTiming,
            ],
            Vec::new(),
            vec![collector_requirement(
                AgentProfilerCollectorToolV1::Rocprofv3,
                AgentProfilerCollectorCapabilityV1::KernelDispatchCollection,
            )],
            AgentProfilerCaptureActionV1::NewCaptureRequired,
            10_000,
            2,
            AgentProfilerSamplingModeV1::CollectorReportedDispatchRecords,
            AgentProfilerPrivilegeRequirementV1::ProfilerAccess,
            None,
        ),
    };
    let estimated_maximum = existing_bundle_bytes
        .saturating_mul(storage_multiplier)
        .min(planning.constraints.maximum_storage_bytes);
    let mut completeness_limitations = vec![
        AgentProfilerCompletenessLimitV1::CollectorReportedRecordsOnly,
        AgentProfilerCompletenessLimitV1::StorageCeilingMayTruncate,
    ];
    if planning.target.dispatch.is_some() {
        completeness_limitations.push(AgentProfilerCompletenessLimitV1::SelectedDispatchOnly);
    }
    if !planning.target.compute_units.is_empty() {
        completeness_limitations.push(AgentProfilerCompletenessLimitV1::SelectedComputeUnitsOnly);
    }
    if matches!(
        sampling_mode,
        AgentProfilerSamplingModeV1::TargetedThreadTrace
    ) {
        completeness_limitations.push(AgentProfilerCompletenessLimitV1::NoFullGridAttClaim);
    }
    if matches!(
        sampling_mode,
        AgentProfilerSamplingModeV1::DispatchCounterAggregate
    ) {
        completeness_limitations
            .push(AgentProfilerCompletenessLimitV1::AggregateCountersDoNotEstablishCausality);
    }
    AgentProfilerCaptureRecipeV1 {
        action,
        target: planning.target.clone(),
        target_validation: AgentProfilerTargetValidationV1 {
            compute_units: if planning.target.compute_units.is_empty() {
                AgentProfilerSelectorValidationV1::NotSpecified
            } else {
                AgentProfilerSelectorValidationV1::CallerDeclaredNotValidatedByBundle
            },
            kernel_ir: if planning.target.kernel_ir.is_some() {
                AgentProfilerSelectorValidationV1::ValidatedAgainstCapture
            } else {
                AgentProfilerSelectorValidationV1::NotSpecified
            },
            dispatch: if planning.target.dispatch.is_some() {
                AgentProfilerSelectorValidationV1::ValidatedAgainstCapture
            } else {
                AgentProfilerSelectorValidationV1::NotSpecified
            },
        },
        requested_data_classes,
        requested_logical_counters,
        collector_requirements,
        expected_overhead: AgentProfilerExpectedOverheadV1 {
            origin: TruthOriginV1::Declared,
            additional_runtime_basis_points: AgentProfilerBoundedU32RangeV1 {
                minimum: 0,
                maximum: overhead_max,
            },
            method: AgentProfilerOverheadMethodV1::ContractDeclaredConservativeEnvelope,
            limitations: vec![
                AgentProfilerOverheadLimitationV1::NotMeasured,
                AgentProfilerOverheadLimitationV1::WorkloadDeviceAndCollectorDependent,
                AgentProfilerOverheadLimitationV1::NotAPerformancePrediction,
            ],
        },
        storage: AgentProfilerStoragePlanV1 {
            maximum_bytes: planning.constraints.maximum_storage_bytes,
            estimated_bytes: AgentProfilerBoundedU64RangeV1 {
                minimum: 0,
                maximum: estimated_maximum,
            },
            estimate_origin: TruthOriginV1::Inferred,
            estimate_method:
                AgentProfilerStorageEstimateMethodV1::ExistingBundleBytesScaledBySelectedDataClass,
            estimate_scale_multiplier: u16::try_from(storage_multiplier)
                .expect("the contract-declared storage multipliers fit u16"),
            limitations: vec![
                AgentProfilerStorageLimitationV1::EstimateNotMeasurement,
                AgentProfilerStorageLimitationV1::EncodingAndWorkloadDependent,
                AgentProfilerStorageLimitationV1::MaximumMayTruncateCapture,
            ],
        },
        required_privilege,
        mutual_exclusions: excluded_data_class
            .map(|excluded_data_class| {
                vec![AgentProfilerMutualExclusionV1 {
                    excluded_data_class,
                    reason:
                        AgentProfilerMutualExclusionReasonV1::SeparateInstrumentationCaptureRequired,
                }]
            })
            .unwrap_or_default(),
        sampling_and_completeness: AgentProfilerSamplingCompletenessV1 {
            mode: sampling_mode,
            maximum_records: planning.constraints.maximum_records,
            limitations: completeness_limitations,
        },
        authorization: AgentProfilerAuthorizationBoundaryV1 {
            service_authority: AgentProfilerServiceAuthorityV1::ReadOnlyPlanningOnly,
            stateful_execution:
                AgentProfilerExecutionAuthorizationV1::SeparateExplicitAuthorizationRequired,
            attach_authority: AgentProfilerAttachAuthorityV1::NotAvailableToService,
        },
    }
}

fn att_collector_requirements(capture_required: bool) -> Vec<AgentProfilerCollectorRequirementV1> {
    let mut requirements = Vec::new();
    if capture_required {
        requirements.push(collector_requirement(
            AgentProfilerCollectorToolV1::Rocprofv3,
            AgentProfilerCollectorCapabilityV1::AttThreadTraceCollection,
        ));
    }
    requirements.extend([
        collector_requirement(
            AgentProfilerCollectorToolV1::Rocprofv3ComputeViewer,
            AgentProfilerCollectorCapabilityV1::DecodedEventExport,
        ),
        AgentProfilerCollectorRequirementV1 {
            tool: AgentProfilerCollectorToolV1::Fe2o3SemanticImporter,
            capability: AgentProfilerCollectorCapabilityV1::StrictDecodedEventImport,
            status: AgentProfilerCollectorCapabilityStatusV1::RequiredUnavailableInCurrentBuild,
        },
    ]);
    requirements
}

const fn collector_requirement(
    tool: AgentProfilerCollectorToolV1,
    capability: AgentProfilerCollectorCapabilityV1,
) -> AgentProfilerCollectorRequirementV1 {
    AgentProfilerCollectorRequirementV1 {
        tool,
        capability,
        status: AgentProfilerCollectorCapabilityStatusV1::RequiredNotVerifiedByCapture,
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
                | AgentProfilerOperationV1::CompareCaptures
                | AgentProfilerOperationV1::PlanNextCapture => {
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
                    Some(
                        AgentProfilerUnavailableReasonV1::RankedExplanationRequiresCausalCounterOrDecodedEventEvidence,
                    ),
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
                request_contract_schema: (operation
                    == AgentProfilerOperationV1::PlanNextCapture)
                    .then_some(AGENT_PROFILER_PLAN_REQUEST_SCHEMA_V1),
                result_contract_schema: (operation
                    == AgentProfilerOperationV1::PlanNextCapture)
                    .then_some(AGENT_PROFILER_PLAN_SCHEMA_V1),
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
            Some(
                AgentProfilerUnavailableReasonV1::RankedExplanationRequiresCausalCounterOrDecodedEventEvidence,
            )
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
