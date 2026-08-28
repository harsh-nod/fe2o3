//! Deterministic next-capture planning over one already validated trace.

use fe2o3_semantic_trace::{
    AllocationEventV1, CaptureBoundariesV1, DiagnosticKindV1, DispatchEventV1, DispatchOutcomeV1,
    EvidenceKindV1, ExecutionLevelV1, FactProvenanceV1, InvocationEventV1, KirSiteClaimV1,
    KirSitePointV1, LaunchGeometryV1, MemoryOutcomeV1, OpaqueIdentityV1, ProducerKindV1,
    TimestampV1, TraceCompletenessV1, TraceEventKindV1, TraceEventV1, TraceV1,
};
use serde::Serialize;

use crate::{OpaqueIdentityViewV1, QueryErrorV1};

pub const MAX_CAPTURE_PLAN_STEPS_V1: usize = 4;
pub const MAX_CAPTURE_FACTS_PER_STEP_V1: usize = 8;
pub const MAX_CAPTURE_PLAN_UNSUPPORTED_V1: usize = 8;
pub const MAX_CAPTURE_EXISTING_EVIDENCE_REFS_V1: usize = 12;
pub const MAX_DIAGNOSIS_MISSING_FACTS_V1: usize = 8;
pub const MAX_DIAGNOSIS_OBSERVED_FAULTS_V1: usize = 8;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CaptureGoalV1 {
    MemoryFault,
    BarrierDivergence,
    PerformanceHotspot,
    CorrectnessMismatch,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CapturePlanDispositionV1 {
    AdditionalCaptureRequired,
    AdditionalCaptureRequiredWithUnsupportedFacts,
    ExistingEvidenceObserved,
    BlockedByUnsupportedFacts,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CaptureToolFamilyV1 {
    SimulatorTrace,
    Rocprofv3DispatchJson,
    Rocprofv3Counters,
    Rocprofv3PcSampling,
    Rocprofv3Att,
    FutureDirectKfd,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CaptureToolStatusV1 {
    SupportedTraceProducer,
    SupportedImporter,
    SupportedManifestOnly,
    CaptureOnlyNoTraceV1Facts,
    FutureUnavailable,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CaptureFactV1 {
    TraceContainerIdentity,
    StableEnvironmentIdentity,
    StableDeviceIdentity,
    StableCounterIdentity,
    KernelIrClaim,
    ArtifactBindingClaim,
    DispatchEnvelope,
    DispatchTiming,
    FullDispatchCapture,
    FullInvocationCoverage,
    LaneScope,
    SemanticSites,
    SimulatorControlFlow,
    MemoryAccessOutcomes,
    FaultAllocationLayout,
    ObservedMemoryFault,
    BarrierPhaseEvents,
    ObservedDiagnosticOrFault,
    AttCaptureManifest,
    HardwareCounterMeasurements,
    PcSampleDistribution,
    SelectedWaveAttTimeline,
    FullGridAttCoverage,
    OutputComparison,
    RegisterOrSourceValues,
    AuthenticatedDirectKfdDispatch,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CaptureOverheadTierV1 {
    Low,
    Moderate,
    High,
    VeryHigh,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CaptureStorageTierV1 {
    Small,
    Moderate,
    Large,
    VeryLarge,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CaptureComputeUnitSelectionV1 {
    NotApplicable,
    UnspecifiedNotRepresentedByTraceV1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CapturePrivilegeRequirementV1 {
    None,
    ProfilerAccess,
    DebuggerAccess,
    UnavailableDirectKfdAuthority,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CaptureAttachRequirementV1 {
    None,
    LaunchUnderTool,
    AttachOrLaunchUnderDebugger,
    Unavailable,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CaptureExecutionScopeV1 {
    ReproductionDispatch,
    SelectedWorkgroup {
        workgroup: [u32; 3],
    },
    SelectedWave {
        workgroup: [u32; 3],
        wave: u32,
    },
    SelectedLane {
        workgroup: [u32; 3],
        wave: u32,
        lane: u16,
        logical_workitem: [u64; 3],
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub struct CaptureSiteV1 {
    pub function_ordinal: u64,
    pub block_ordinal: u64,
    pub operation_ordinal: Option<u64>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub struct CaptureTargetV1 {
    pub scope: CaptureExecutionScopeV1,
    pub observed_site: Option<CaptureSiteV1>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CaptureMutualExclusionReasonV1 {
    SeparateInstrumentationCaptureRequired,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub struct CaptureMutualExclusionV1 {
    pub tool: CaptureToolFamilyV1,
    pub reason: CaptureMutualExclusionReasonV1,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct CapturePlanStepV1 {
    ordinal: u8,
    tool: CaptureToolFamilyV1,
    tool_status: CaptureToolStatusV1,
    target: CaptureTargetV1,
    required_facts: Vec<CaptureFactV1>,
    overhead: CaptureOverheadTierV1,
    storage: CaptureStorageTierV1,
    compute_unit_selection: CaptureComputeUnitSelectionV1,
    privilege: CapturePrivilegeRequirementV1,
    attach: CaptureAttachRequirementV1,
    mutual_exclusions: Vec<CaptureMutualExclusionV1>,
    why_it_discriminates: &'static str,
}

impl CapturePlanStepV1 {
    pub const fn ordinal(&self) -> u8 {
        self.ordinal
    }

    pub const fn tool(&self) -> CaptureToolFamilyV1 {
        self.tool
    }

    pub fn required_facts(&self) -> &[CaptureFactV1] {
        &self.required_facts
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum UnsupportedCaptureReasonV1 {
    NotRepresentedByTraceV1,
    ImporterRetainsManifestOnly,
    NormalizationRedactsRequiredScope,
    CurrentDirectKfdBoundaryHasNoAuthenticatedDispatch,
    FullGridCoverageIsNotEstablishedByAtt,
    OutsideCurrentScope,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub struct UnsupportedCaptureFactV1 {
    pub tool: CaptureToolFamilyV1,
    pub fact: CaptureFactV1,
    pub reason: UnsupportedCaptureReasonV1,
    pub detail: &'static str,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CapturePlanLimitationV1 {
    NoDiagnosisClaim,
    NoPerformancePrediction,
    NoDirectKfdDispatchAvailabilityClaim,
    NoFullGridAttCoverageClaim,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ExistingEvidenceKindV1 {
    TraceBinding,
    KernelIrClaim,
    ArtifactClaim,
    TraceEvent,
    EventEvidence,
    AggregateInvariant,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub struct ExistingEvidenceRefV1 {
    pub kind: ExistingEvidenceKindV1,
    pub fact: CaptureFactV1,
    pub event_sequence: Option<u64>,
    pub provenance: &'static str,
    pub identity: Option<OpaqueIdentityViewV1>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct NextCapturePlanV1 {
    goal: CaptureGoalV1,
    disposition: CapturePlanDispositionV1,
    steps: Vec<CapturePlanStepV1>,
    unsupported: Vec<UnsupportedCaptureFactV1>,
    existing_evidence_refs: Vec<ExistingEvidenceRefV1>,
    limitations: [CapturePlanLimitationV1; 4],
}

impl NextCapturePlanV1 {
    pub const fn goal(&self) -> CaptureGoalV1 {
        self.goal
    }

    pub const fn disposition(&self) -> CapturePlanDispositionV1 {
        self.disposition
    }

    pub fn steps(&self) -> &[CapturePlanStepV1] {
        &self.steps
    }

    pub fn unsupported(&self) -> &[UnsupportedCaptureFactV1] {
        &self.unsupported
    }

    pub fn existing_evidence_refs(&self) -> &[ExistingEvidenceRefV1] {
        &self.existing_evidence_refs
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DiagnosisObservationStatusV1 {
    ObservedFaultsPresent,
    NoObservedFaultsInCompleteCapture,
    EvidenceIncomplete,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ObservedFaultSourceV1 {
    Memory,
    Diagnostic,
    Dispatch,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub struct ObservedFaultRefV1 {
    pub sequence: u64,
    pub source: ObservedFaultSourceV1,
    pub kind: &'static str,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct DiagnosisStatusV1 {
    goal: CaptureGoalV1,
    observation_status: DiagnosisObservationStatusV1,
    observed_fault_count: u64,
    observed_faults: Vec<ObservedFaultRefV1>,
    additional_observed_faults: u64,
    missing_facts: Vec<CaptureFactV1>,
    diagnosis_reached: bool,
}

impl DiagnosisStatusV1 {
    pub const fn observation_status(&self) -> DiagnosisObservationStatusV1 {
        self.observation_status
    }

    pub const fn observed_fault_count(&self) -> u64 {
        self.observed_fault_count
    }

    pub fn missing_facts(&self) -> &[CaptureFactV1] {
        &self.missing_facts
    }

    pub const fn diagnosis_reached(&self) -> bool {
        self.diagnosis_reached
    }
}

const LIMITATIONS: [CapturePlanLimitationV1; 4] = [
    CapturePlanLimitationV1::NoDiagnosisClaim,
    CapturePlanLimitationV1::NoPerformancePrediction,
    CapturePlanLimitationV1::NoDirectKfdDispatchAvailabilityClaim,
    CapturePlanLimitationV1::NoFullGridAttCoverageClaim,
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum TraceSourceV1 {
    Simulator,
    RocprofDispatchJson,
    RocprofAttManifest,
    KfdClaim,
    Other,
}

#[derive(Clone, Copy, Debug)]
struct FactInventoryV1 {
    bits: u64,
}

impl FactInventoryV1 {
    fn inspect(trace: &TraceV1) -> Result<Self, QueryErrorV1> {
        let source = trace_source(trace);
        let mut inventory = Self { bits: 0 };
        inventory.insert(CaptureFactV1::KernelIrClaim);
        if trace.header().artifact().is_some() {
            inventory.insert(CaptureFactV1::ArtifactBindingClaim);
        }
        if source == TraceSourceV1::RocprofAttManifest {
            inventory.insert(CaptureFactV1::AttCaptureManifest);
        }

        let boundaries = trace.header().boundaries();
        let full_capture = matches!(trace.header().completeness(), TraceCompletenessV1::Complete)
            && boundaries == CaptureBoundariesV1::FULL_DISPATCH;
        if full_capture {
            inventory.insert(CaptureFactV1::FullDispatchCapture);
        }

        let mut dispatch_begin = false;
        let mut dispatch_end = false;
        let mut begin_clock = None;
        let mut end_clock = None;
        let mut first_memory_fault = None;
        for event in trace.events() {
            if event.provenance() != FactProvenanceV1::Observed {
                continue;
            }
            if event.site().is_some() {
                inventory.insert(CaptureFactV1::SemanticSites);
            }
            if matches!(event.scope().level(), ExecutionLevelV1::Lane { .. }) {
                inventory.insert(CaptureFactV1::LaneScope);
            }
            match event.kind() {
                TraceEventKindV1::Dispatch(DispatchEventV1::Begin) => {
                    dispatch_begin = true;
                    begin_clock = clock_timestamp(event);
                }
                TraceEventKindV1::Dispatch(DispatchEventV1::End(outcome)) => {
                    dispatch_end = true;
                    end_clock = clock_timestamp(event);
                    if matches!(
                        outcome,
                        DispatchOutcomeV1::Failed | DispatchOutcomeV1::Cancelled
                    ) {
                        inventory.insert(CaptureFactV1::ObservedDiagnosticOrFault);
                    }
                }
                TraceEventKindV1::BlockEnter
                | TraceEventKindV1::Operation(_)
                | TraceEventKindV1::Branch { .. }
                    if source == TraceSourceV1::Simulator =>
                {
                    inventory.insert(CaptureFactV1::SimulatorControlFlow);
                }
                TraceEventKindV1::Memory(memory) => {
                    if !matches!(memory.outcome(), MemoryOutcomeV1::Unavailable(_)) {
                        inventory.insert(CaptureFactV1::MemoryAccessOutcomes);
                    }
                    if matches!(memory.outcome(), MemoryOutcomeV1::Fault(_)) {
                        inventory.insert(CaptureFactV1::ObservedMemoryFault);
                        inventory.insert(CaptureFactV1::ObservedDiagnosticOrFault);
                        first_memory_fault.get_or_insert((event.sequence(), memory.allocation()));
                    }
                }
                TraceEventKindV1::Barrier(_) => {
                    inventory.insert(CaptureFactV1::BarrierPhaseEvents);
                }
                TraceEventKindV1::Diagnostic(_) => {
                    inventory.insert(CaptureFactV1::ObservedDiagnosticOrFault);
                }
                _ => {}
            }
        }
        if dispatch_begin && dispatch_end {
            inventory.insert(CaptureFactV1::DispatchEnvelope);
        }
        if matches!((begin_clock, end_clock), (Some((begin_domain, begin)), Some((end_domain, end))) if begin_domain == end_domain && begin <= end)
        {
            inventory.insert(CaptureFactV1::DispatchTiming);
        }
        if full_capture && has_exact_observed_invocation_coverage(trace)? {
            inventory.insert(CaptureFactV1::FullInvocationCoverage);
        }
        if let Some((fault_sequence, fault_allocation)) = first_memory_fault
            && trace.events().iter().any(|event| {
                event.sequence() < fault_sequence
                    && event.provenance() == FactProvenanceV1::Observed
                    && matches!(
                        event.kind(),
                        TraceEventKindV1::Allocation(
                            AllocationEventV1::Create { allocation, .. }
                                | AllocationEventV1::Preexisting { allocation, .. }
                        ) if allocation == fault_allocation
                    )
            })
        {
            inventory.insert(CaptureFactV1::FaultAllocationLayout);
        }
        Ok(inventory)
    }

    const fn contains(self, fact: CaptureFactV1) -> bool {
        self.bits & fact_bit(fact) != 0
    }

    fn insert(&mut self, fact: CaptureFactV1) {
        self.bits |= fact_bit(fact);
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct InvocationCoordinateV1 {
    workgroup: [u32; 3],
    wave: u32,
    lane: u16,
    logical_workitem: [u64; 3],
}

fn has_exact_observed_invocation_coverage(trace: &TraceV1) -> Result<bool, QueryErrorV1> {
    let mut begins = bounded_vec(trace.events().len())?;
    let mut ends = bounded_vec(trace.events().len())?;
    for event in trace.events() {
        let TraceEventKindV1::Invocation(action) = event.kind() else {
            continue;
        };
        if event.provenance() != FactProvenanceV1::Observed {
            return Ok(false);
        }
        let Some(coordinate) = invocation_coordinate(event) else {
            return Ok(false);
        };
        match action {
            InvocationEventV1::Begin => begins.push(coordinate),
            InvocationEventV1::End => ends.push(coordinate),
        }
    }
    begins.sort_unstable();
    ends.sort_unstable();
    if begins.windows(2).any(|pair| pair[0] == pair[1])
        || ends.windows(2).any(|pair| pair[0] == pair[1])
        || begins != ends
    {
        return Ok(false);
    }

    let launch = trace.header().launch();
    let expected = launch
        .logical_grid()
        .into_iter()
        .try_fold(1_u64, u64::checked_mul)
        .ok_or(QueryErrorV1::SizeOverflow)?;
    if u64::try_from(begins.len()).map_err(|_| QueryErrorV1::SizeOverflow)? != expected {
        return Ok(false);
    }
    let grid = launch.logical_grid();
    for z in 0..grid[2] {
        for y in 0..grid[1] {
            for x in 0..grid[0] {
                let expected = coordinate_for_logical_workitem(launch, [x, y, z])?;
                if begins.binary_search(&expected).is_err() {
                    return Ok(false);
                }
            }
        }
    }
    Ok(true)
}

fn invocation_coordinate(event: &TraceEventV1) -> Option<InvocationCoordinateV1> {
    let ExecutionLevelV1::Lane {
        workgroup,
        wave,
        lane,
        logical_workitem,
        ..
    } = event.scope().level()
    else {
        return None;
    };
    Some(InvocationCoordinateV1 {
        workgroup,
        wave,
        lane,
        logical_workitem,
    })
}

fn coordinate_for_logical_workitem(
    launch: LaunchGeometryV1,
    logical_workitem: [u64; 3],
) -> Result<InvocationCoordinateV1, QueryErrorV1> {
    let size = launch.workgroup_size();
    let mut workgroup = [0_u32; 3];
    let mut local = [0_u32; 3];
    for axis in 0..3 {
        let size = u64::from(size[axis]);
        workgroup[axis] =
            u32::try_from(logical_workitem[axis] / size).map_err(|_| QueryErrorV1::SizeOverflow)?;
        local[axis] =
            u32::try_from(logical_workitem[axis] % size).map_err(|_| QueryErrorV1::SizeOverflow)?;
    }
    let linear = launch
        .linear_local_workitem(local)
        .ok_or(QueryErrorV1::SizeOverflow)?;
    let width = u64::from(launch.wave_width().lanes());
    Ok(InvocationCoordinateV1 {
        workgroup,
        wave: u32::try_from(linear / width).map_err(|_| QueryErrorV1::SizeOverflow)?,
        lane: u16::try_from(linear % width).map_err(|_| QueryErrorV1::SizeOverflow)?,
        logical_workitem,
    })
}

pub(crate) fn plan_next_capture(
    trace: &TraceV1,
    trace_binding: OpaqueIdentityViewV1,
    goal: CaptureGoalV1,
) -> Result<NextCapturePlanV1, QueryErrorV1> {
    let inventory = FactInventoryV1::inspect(trace)?;
    let mut steps = bounded_vec(MAX_CAPTURE_PLAN_STEPS_V1)?;
    let mut unsupported = bounded_vec(MAX_CAPTURE_PLAN_UNSUPPORTED_V1)?;
    let mut planned = inventory.bits;
    let observed_site = observed_goal_site(trace, goal);

    match goal {
        CaptureGoalV1::MemoryFault => {
            add_step(
                &mut steps,
                &mut planned,
                CaptureStepSpecV1 {
                    tool: CaptureToolFamilyV1::SimulatorTrace,
                    tool_status: CaptureToolStatusV1::SupportedTraceProducer,
                    target: reproduction_target(observed_site),
                    facts: &[
                        CaptureFactV1::MemoryAccessOutcomes,
                        CaptureFactV1::FaultAllocationLayout,
                        CaptureFactV1::SemanticSites,
                        CaptureFactV1::LaneScope,
                        CaptureFactV1::FullDispatchCapture,
                    ],
                    overhead: CaptureOverheadTierV1::Moderate,
                    storage: CaptureStorageTierV1::Moderate,
                    compute_unit_selection: CaptureComputeUnitSelectionV1::NotApplicable,
                    privilege: CapturePrivilegeRequirementV1::None,
                    attach: CaptureAttachRequirementV1::None,
                    exclusions: &[],
                    why: "reproduces checked semantic memory outcomes and allocation bounds without requiring a GPU; it can distinguish a semantic bounds/lifecycle fault from an unreproduced hardware-only symptom",
                },
            )?;
            push_unsupported(
                &mut unsupported,
                direct_kfd_unsupported(CaptureFactV1::AuthenticatedDirectKfdDispatch),
            )?;
        }
        CaptureGoalV1::BarrierDivergence => {
            add_step(
                &mut steps,
                &mut planned,
                CaptureStepSpecV1 {
                    tool: CaptureToolFamilyV1::SimulatorTrace,
                    tool_status: CaptureToolStatusV1::SupportedTraceProducer,
                    target: reproduction_target(observed_site),
                    facts: &[
                        CaptureFactV1::BarrierPhaseEvents,
                        CaptureFactV1::FullInvocationCoverage,
                        CaptureFactV1::SemanticSites,
                        CaptureFactV1::LaneScope,
                    ],
                    overhead: CaptureOverheadTierV1::Moderate,
                    storage: CaptureStorageTierV1::Moderate,
                    compute_unit_selection: CaptureComputeUnitSelectionV1::NotApplicable,
                    privilege: CapturePrivilegeRequirementV1::None,
                    attach: CaptureAttachRequirementV1::None,
                    exclusions: &[],
                    why: "compares barrier phase arrivals/releases across explicitly covered simulator invocations and preserves the KIR site of each event",
                },
            )?;
            push_unsupported(
                &mut unsupported,
                UnsupportedCaptureFactV1 {
                    tool: CaptureToolFamilyV1::Rocprofv3Att,
                    fact: CaptureFactV1::SelectedWaveAttTimeline,
                    reason: UnsupportedCaptureReasonV1::ImporterRetainsManifestOnly,
                    detail: "the current ATT importer proves only a capture manifest; decoded wave timelines are not Trace V1 facts",
                },
            )?;
            push_unsupported(
                &mut unsupported,
                UnsupportedCaptureFactV1 {
                    tool: CaptureToolFamilyV1::Rocprofv3Att,
                    fact: CaptureFactV1::FullGridAttCoverage,
                    reason: UnsupportedCaptureReasonV1::FullGridCoverageIsNotEstablishedByAtt,
                    detail: "ATT must be treated as selected-wave evidence and never as full-grid coverage",
                },
            )?;
            push_unsupported(
                &mut unsupported,
                direct_kfd_unsupported(CaptureFactV1::AuthenticatedDirectKfdDispatch),
            )?;
        }
        CaptureGoalV1::PerformanceHotspot => {
            add_step(
                &mut steps,
                &mut planned,
                CaptureStepSpecV1 {
                    tool: CaptureToolFamilyV1::Rocprofv3DispatchJson,
                    tool_status: CaptureToolStatusV1::SupportedImporter,
                    target: reproduction_target(observed_site),
                    facts: &[
                        CaptureFactV1::DispatchEnvelope,
                        CaptureFactV1::DispatchTiming,
                    ],
                    overhead: CaptureOverheadTierV1::Low,
                    storage: CaptureStorageTierV1::Small,
                    compute_unit_selection:
                        CaptureComputeUnitSelectionV1::UnspecifiedNotRepresentedByTraceV1,
                    privilege: CapturePrivilegeRequirementV1::ProfilerAccess,
                    attach: CaptureAttachRequirementV1::LaunchUnderTool,
                    exclusions: &[],
                    why: "establishes an observed dispatch interval and launch geometry before collecting higher-overhead attribution evidence; it is measurement, not performance prediction",
                },
            )?;
            add_step(
                &mut steps,
                &mut planned,
                CaptureStepSpecV1 {
                    tool: CaptureToolFamilyV1::Rocprofv3PcSampling,
                    tool_status: CaptureToolStatusV1::CaptureOnlyNoTraceV1Facts,
                    target: reproduction_target(observed_site),
                    facts: &[CaptureFactV1::PcSampleDistribution],
                    overhead: CaptureOverheadTierV1::Moderate,
                    storage: CaptureStorageTierV1::Moderate,
                    compute_unit_selection:
                        CaptureComputeUnitSelectionV1::UnspecifiedNotRepresentedByTraceV1,
                    privilege: CapturePrivilegeRequirementV1::ProfilerAccess,
                    attach: CaptureAttachRequirementV1::LaunchUnderTool,
                    exclusions: &[CaptureToolFamilyV1::Rocprofv3Att],
                    why: "sampled execution attribution can identify where measured hardware time accumulates without asserting complete instruction coverage",
                },
            )?;
            add_step(
                &mut steps,
                &mut planned,
                CaptureStepSpecV1 {
                    tool: CaptureToolFamilyV1::Rocprofv3Counters,
                    tool_status: CaptureToolStatusV1::CaptureOnlyNoTraceV1Facts,
                    target: reproduction_target(observed_site),
                    facts: &[CaptureFactV1::HardwareCounterMeasurements],
                    overhead: CaptureOverheadTierV1::Moderate,
                    storage: CaptureStorageTierV1::Small,
                    compute_unit_selection:
                        CaptureComputeUnitSelectionV1::UnspecifiedNotRepresentedByTraceV1,
                    privilege: CapturePrivilegeRequirementV1::ProfilerAccess,
                    attach: CaptureAttachRequirementV1::LaunchUnderTool,
                    exclusions: &[CaptureToolFamilyV1::Rocprofv3Att],
                    why: "measured counters can discriminate memory, issue, and occupancy pressure after a hotspot is observed; they do not predict performance",
                },
            )?;
            add_step(
                &mut steps,
                &mut planned,
                CaptureStepSpecV1 {
                    tool: CaptureToolFamilyV1::Rocprofv3Att,
                    tool_status: CaptureToolStatusV1::SupportedManifestOnly,
                    target: reproduction_target(observed_site),
                    facts: &[CaptureFactV1::AttCaptureManifest],
                    overhead: CaptureOverheadTierV1::VeryHigh,
                    storage: CaptureStorageTierV1::VeryLarge,
                    compute_unit_selection:
                        CaptureComputeUnitSelectionV1::UnspecifiedNotRepresentedByTraceV1,
                    privilege: CapturePrivilegeRequirementV1::ProfilerAccess,
                    attach: CaptureAttachRequirementV1::LaunchUnderTool,
                    exclusions: &[
                        CaptureToolFamilyV1::Rocprofv3PcSampling,
                        CaptureToolFamilyV1::Rocprofv3Counters,
                    ],
                    why: "confirms that selected-wave ATT artifacts were emitted for a separate capture; the current importer does not decode their timeline or establish full-grid coverage",
                },
            )?;
            for (tool, fact, detail) in [
                (
                    CaptureToolFamilyV1::Rocprofv3PcSampling,
                    CaptureFactV1::PcSampleDistribution,
                    "Trace V1 has no PC-sample record or authenticated PC-to-KIR correlation",
                ),
                (
                    CaptureToolFamilyV1::Rocprofv3Counters,
                    CaptureFactV1::HardwareCounterMeasurements,
                    "Trace V1 has no hardware-counter value record",
                ),
                (
                    CaptureToolFamilyV1::Rocprofv3Att,
                    CaptureFactV1::SelectedWaveAttTimeline,
                    "the current ATT importer retains only the manifest, not decoded selected-wave timelines",
                ),
            ] {
                push_unsupported(
                    &mut unsupported,
                    UnsupportedCaptureFactV1 {
                        tool,
                        fact,
                        reason: UnsupportedCaptureReasonV1::NotRepresentedByTraceV1,
                        detail,
                    },
                )?;
            }
            push_unsupported(
                &mut unsupported,
                UnsupportedCaptureFactV1 {
                    tool: CaptureToolFamilyV1::Rocprofv3Att,
                    fact: CaptureFactV1::FullGridAttCoverage,
                    reason: UnsupportedCaptureReasonV1::FullGridCoverageIsNotEstablishedByAtt,
                    detail: "ATT must be treated as selected-wave evidence and never as full-grid coverage",
                },
            )?;
            push_unsupported(
                &mut unsupported,
                direct_kfd_unsupported(CaptureFactV1::AuthenticatedDirectKfdDispatch),
            )?;
        }
        CaptureGoalV1::CorrectnessMismatch => {
            add_step(
                &mut steps,
                &mut planned,
                CaptureStepSpecV1 {
                    tool: CaptureToolFamilyV1::SimulatorTrace,
                    tool_status: CaptureToolStatusV1::SupportedTraceProducer,
                    target: reproduction_target(observed_site),
                    facts: &[
                        CaptureFactV1::SimulatorControlFlow,
                        CaptureFactV1::MemoryAccessOutcomes,
                        CaptureFactV1::SemanticSites,
                        CaptureFactV1::FullInvocationCoverage,
                    ],
                    overhead: CaptureOverheadTierV1::Moderate,
                    storage: CaptureStorageTierV1::Moderate,
                    compute_unit_selection: CaptureComputeUnitSelectionV1::NotApplicable,
                    privilege: CapturePrivilegeRequirementV1::None,
                    attach: CaptureAttachRequirementV1::None,
                    exclusions: &[],
                    why: "records deterministic semantic control flow and memory outcomes for the claimed KIR and launch geometry; Trace V1 does not authenticate inputs or output equality",
                },
            )?;
            push_unsupported(
                &mut unsupported,
                UnsupportedCaptureFactV1 {
                    tool: CaptureToolFamilyV1::SimulatorTrace,
                    fact: CaptureFactV1::OutputComparison,
                    reason: UnsupportedCaptureReasonV1::NotRepresentedByTraceV1,
                    detail: "Trace V1 does not carry input/output values or a reference-output comparison",
                },
            )?;
            push_unsupported(
                &mut unsupported,
                direct_kfd_unsupported(CaptureFactV1::AuthenticatedDirectKfdDispatch),
            )?;
        }
    }

    let goal_observed = goal_evidence_observed(inventory, goal);
    let disposition = if goal_observed && steps.is_empty() {
        CapturePlanDispositionV1::ExistingEvidenceObserved
    } else if steps.is_empty() {
        CapturePlanDispositionV1::BlockedByUnsupportedFacts
    } else if unsupported.is_empty() {
        CapturePlanDispositionV1::AdditionalCaptureRequired
    } else {
        CapturePlanDispositionV1::AdditionalCaptureRequiredWithUnsupportedFacts
    };
    Ok(NextCapturePlanV1 {
        goal,
        disposition,
        steps,
        unsupported,
        existing_evidence_refs: existing_evidence(trace, trace_binding, goal)?,
        limitations: LIMITATIONS,
    })
}

pub(crate) fn diagnosis_status(
    trace: &TraceV1,
    goal: CaptureGoalV1,
) -> Result<DiagnosisStatusV1, QueryErrorV1> {
    let inventory = FactInventoryV1::inspect(trace)?;
    let mut faults = bounded_vec(MAX_DIAGNOSIS_OBSERVED_FAULTS_V1)?;
    let mut fault_count = 0_u64;
    for event in trace.events() {
        if event.provenance() != FactProvenanceV1::Observed {
            continue;
        }
        let fault = observed_fault(event);
        if let Some(fault) = fault {
            fault_count = fault_count.saturating_add(1);
            if faults.len() < MAX_DIAGNOSIS_OBSERVED_FAULTS_V1 {
                faults.push(fault);
            }
        }
    }
    let additional_observed_faults = fault_count
        .saturating_sub(u64::try_from(faults.len()).map_err(|_| QueryErrorV1::SizeOverflow)?);
    let full = inventory.contains(CaptureFactV1::FullDispatchCapture);
    let observation_status = if fault_count != 0 {
        DiagnosisObservationStatusV1::ObservedFaultsPresent
    } else if full {
        DiagnosisObservationStatusV1::NoObservedFaultsInCompleteCapture
    } else {
        DiagnosisObservationStatusV1::EvidenceIncomplete
    };
    let mut missing_facts = bounded_vec(MAX_DIAGNOSIS_MISSING_FACTS_V1)?;
    for fact in goal_facts(goal) {
        if !inventory.contains(*fact) {
            missing_facts.push(*fact);
        }
    }
    Ok(DiagnosisStatusV1 {
        goal,
        observation_status,
        observed_fault_count: fault_count,
        observed_faults: faults,
        additional_observed_faults,
        missing_facts,
        diagnosis_reached: false,
    })
}

struct CaptureStepSpecV1 {
    tool: CaptureToolFamilyV1,
    tool_status: CaptureToolStatusV1,
    target: CaptureTargetV1,
    facts: &'static [CaptureFactV1],
    overhead: CaptureOverheadTierV1,
    storage: CaptureStorageTierV1,
    compute_unit_selection: CaptureComputeUnitSelectionV1,
    privilege: CapturePrivilegeRequirementV1,
    attach: CaptureAttachRequirementV1,
    exclusions: &'static [CaptureToolFamilyV1],
    why: &'static str,
}

fn add_step(
    steps: &mut Vec<CapturePlanStepV1>,
    planned: &mut u64,
    spec: CaptureStepSpecV1,
) -> Result<(), QueryErrorV1> {
    let mut required_facts = bounded_vec(MAX_CAPTURE_FACTS_PER_STEP_V1)?;
    for fact in spec.facts {
        let bit = fact_bit(*fact);
        if *planned & bit == 0 {
            required_facts.push(*fact);
            *planned |= bit;
        }
    }
    if required_facts.is_empty() {
        return Ok(());
    }
    if steps.len() == MAX_CAPTURE_PLAN_STEPS_V1 {
        return Err(QueryErrorV1::PlanLimitExceeded {
            field: "steps",
            max: MAX_CAPTURE_PLAN_STEPS_V1,
        });
    }
    let mut mutual_exclusions = bounded_vec(spec.exclusions.len())?;
    mutual_exclusions.extend(spec.exclusions.iter().map(|tool| CaptureMutualExclusionV1 {
        tool: *tool,
        reason: CaptureMutualExclusionReasonV1::SeparateInstrumentationCaptureRequired,
    }));
    let ordinal = u8::try_from(steps.len() + 1).map_err(|_| QueryErrorV1::SizeOverflow)?;
    steps.push(CapturePlanStepV1 {
        ordinal,
        tool: spec.tool,
        tool_status: spec.tool_status,
        target: spec.target,
        required_facts,
        overhead: spec.overhead,
        storage: spec.storage,
        compute_unit_selection: spec.compute_unit_selection,
        privilege: spec.privilege,
        attach: spec.attach,
        mutual_exclusions,
        why_it_discriminates: spec.why,
    });
    Ok(())
}

fn push_unsupported(
    unsupported: &mut Vec<UnsupportedCaptureFactV1>,
    fact: UnsupportedCaptureFactV1,
) -> Result<(), QueryErrorV1> {
    if unsupported.len() == MAX_CAPTURE_PLAN_UNSUPPORTED_V1 {
        return Err(QueryErrorV1::PlanLimitExceeded {
            field: "unsupported",
            max: MAX_CAPTURE_PLAN_UNSUPPORTED_V1,
        });
    }
    unsupported.push(fact);
    Ok(())
}

fn existing_evidence(
    trace: &TraceV1,
    trace_binding: OpaqueIdentityViewV1,
    goal: CaptureGoalV1,
) -> Result<Vec<ExistingEvidenceRefV1>, QueryErrorV1> {
    let inventory = FactInventoryV1::inspect(trace)?;
    let mut refs = bounded_vec(MAX_CAPTURE_EXISTING_EVIDENCE_REFS_V1)?;
    refs.push(ExistingEvidenceRefV1 {
        kind: ExistingEvidenceKindV1::TraceBinding,
        fact: source_claim_fact(trace_source(trace)),
        event_sequence: None,
        provenance: "trace_binding",
        identity: Some(trace_binding),
    });
    refs.push(ExistingEvidenceRefV1 {
        kind: ExistingEvidenceKindV1::KernelIrClaim,
        fact: CaptureFactV1::KernelIrClaim,
        event_sequence: None,
        provenance: "untrusted_claim",
        identity: Some(identity_view(trace.header().kernel_ir_claim().digest())),
    });
    if let Some(artifact) = trace.header().artifact() {
        refs.push(ExistingEvidenceRefV1 {
            kind: ExistingEvidenceKindV1::ArtifactClaim,
            fact: CaptureFactV1::ArtifactBindingClaim,
            event_sequence: None,
            provenance: "untrusted_claim",
            identity: Some(identity_view(artifact.digest())),
        });
    }
    if inventory.contains(CaptureFactV1::FullInvocationCoverage) {
        refs.push(ExistingEvidenceRefV1 {
            kind: ExistingEvidenceKindV1::AggregateInvariant,
            fact: CaptureFactV1::FullInvocationCoverage,
            event_sequence: None,
            provenance: "observed_aggregate",
            identity: Some(trace_binding),
        });
    }
    let first_memory_fault = trace.events().iter().find_map(|event| {
        if event.provenance() != FactProvenanceV1::Observed {
            return None;
        }
        match event.kind() {
            TraceEventKindV1::Memory(memory)
                if matches!(memory.outcome(), MemoryOutcomeV1::Fault(_)) =>
            {
                Some((event.sequence(), memory.allocation()))
            }
            _ => None,
        }
    });
    for event in trace.events() {
        let Some(fact) = event_fact(event, first_memory_fault) else {
            continue;
        };
        if !fact_relevant(goal, fact) {
            continue;
        }
        if refs.len() == MAX_CAPTURE_EXISTING_EVIDENCE_REFS_V1 {
            break;
        }
        refs.push(ExistingEvidenceRefV1 {
            kind: ExistingEvidenceKindV1::TraceEvent,
            fact,
            event_sequence: Some(event.sequence()),
            provenance: provenance_label(event.provenance()),
            identity: None,
        });
        for evidence in event.evidence_refs() {
            if refs.len() == MAX_CAPTURE_EXISTING_EVIDENCE_REFS_V1 {
                break;
            }
            refs.push(ExistingEvidenceRefV1 {
                kind: ExistingEvidenceKindV1::EventEvidence,
                fact,
                event_sequence: Some(event.sequence()),
                provenance: evidence_kind_label(evidence.kind()),
                identity: Some(identity_view(evidence.identity())),
            });
        }
    }
    Ok(refs)
}

fn bounded_vec<T>(capacity: usize) -> Result<Vec<T>, QueryErrorV1> {
    let mut values = Vec::new();
    values
        .try_reserve_exact(capacity)
        .map_err(|_| QueryErrorV1::AllocationFailure {
            requested: capacity,
        })?;
    Ok(values)
}

const fn fact_bit(fact: CaptureFactV1) -> u64 {
    1_u64 << fact as u8
}

fn trace_source(trace: &TraceV1) -> TraceSourceV1 {
    let producer = trace.header().producer();
    match producer.kind() {
        ProducerKindV1::CpuKirSimulator => TraceSourceV1::Simulator,
        ProducerKindV1::RocprofImporter if producer.name().as_str() == "rocprofv3-json-import" => {
            TraceSourceV1::RocprofDispatchJson
        }
        ProducerKindV1::RocprofImporter
            if producer.name().as_str() == "rocprofv3-att-manifest-import" =>
        {
            TraceSourceV1::RocprofAttManifest
        }
        ProducerKindV1::KfdHardwareCollector => TraceSourceV1::KfdClaim,
        _ => TraceSourceV1::Other,
    }
}

const fn source_claim_fact(source: TraceSourceV1) -> CaptureFactV1 {
    match source {
        TraceSourceV1::RocprofAttManifest => CaptureFactV1::AttCaptureManifest,
        TraceSourceV1::Simulator
        | TraceSourceV1::RocprofDispatchJson
        | TraceSourceV1::KfdClaim
        | TraceSourceV1::Other => CaptureFactV1::TraceContainerIdentity,
    }
}

fn clock_timestamp(event: &TraceEventV1) -> Option<(OpaqueIdentityV1, u64)> {
    match event.timestamp() {
        TimestampV1::Clock { domain, ticks } => Some((domain, ticks)),
        TimestampV1::LogicalStep(_) => None,
    }
}

fn goal_evidence_observed(inventory: FactInventoryV1, goal: CaptureGoalV1) -> bool {
    goal_facts(goal)
        .iter()
        .all(|fact| inventory.contains(*fact))
}

const fn goal_facts(goal: CaptureGoalV1) -> &'static [CaptureFactV1] {
    match goal {
        CaptureGoalV1::MemoryFault => &[
            CaptureFactV1::ObservedMemoryFault,
            CaptureFactV1::MemoryAccessOutcomes,
            CaptureFactV1::FaultAllocationLayout,
            CaptureFactV1::SemanticSites,
            CaptureFactV1::LaneScope,
        ],
        CaptureGoalV1::BarrierDivergence => &[
            CaptureFactV1::BarrierPhaseEvents,
            CaptureFactV1::FullInvocationCoverage,
            CaptureFactV1::SemanticSites,
            CaptureFactV1::LaneScope,
        ],
        CaptureGoalV1::PerformanceHotspot => &[
            CaptureFactV1::DispatchTiming,
            CaptureFactV1::PcSampleDistribution,
            CaptureFactV1::HardwareCounterMeasurements,
        ],
        CaptureGoalV1::CorrectnessMismatch => &[
            CaptureFactV1::SimulatorControlFlow,
            CaptureFactV1::MemoryAccessOutcomes,
            CaptureFactV1::FullInvocationCoverage,
            CaptureFactV1::OutputComparison,
        ],
    }
}

fn reproduction_target(site: Option<CaptureSiteV1>) -> CaptureTargetV1 {
    CaptureTargetV1 {
        scope: CaptureExecutionScopeV1::ReproductionDispatch,
        observed_site: site,
    }
}

fn observed_goal_site(trace: &TraceV1, goal: CaptureGoalV1) -> Option<CaptureSiteV1> {
    trace.events().iter().find_map(|event| {
        if event.provenance() != FactProvenanceV1::Observed {
            return None;
        }
        let relevant = match goal {
            CaptureGoalV1::MemoryFault => matches!(
                event.kind(),
                TraceEventKindV1::Memory(memory)
                    if matches!(memory.outcome(), MemoryOutcomeV1::Fault(_))
            ),
            CaptureGoalV1::BarrierDivergence => {
                matches!(event.kind(), TraceEventKindV1::Barrier(_))
            }
            CaptureGoalV1::PerformanceHotspot | CaptureGoalV1::CorrectnessMismatch => false,
        };
        relevant.then(|| event.site().map(site_view)).flatten()
    })
}

fn site_view(site: KirSiteClaimV1) -> CaptureSiteV1 {
    CaptureSiteV1 {
        function_ordinal: site.function_ordinal(),
        block_ordinal: site.block_ordinal(),
        operation_ordinal: match site.point() {
            KirSitePointV1::Operation(ordinal) => Some(ordinal),
            KirSitePointV1::BlockEntry | KirSitePointV1::Terminator => None,
        },
    }
}

fn event_fact(
    event: &TraceEventV1,
    first_memory_fault: Option<(u64, fe2o3_semantic_trace::TraceAllocationIdV1)>,
) -> Option<CaptureFactV1> {
    match event.kind() {
        TraceEventKindV1::Dispatch(_) => Some(CaptureFactV1::DispatchEnvelope),
        TraceEventKindV1::Invocation(_) => None,
        TraceEventKindV1::BlockEnter
        | TraceEventKindV1::Operation(_)
        | TraceEventKindV1::Branch { .. } => Some(CaptureFactV1::SemanticSites),
        TraceEventKindV1::Memory(memory)
            if matches!(memory.outcome(), MemoryOutcomeV1::Fault(_)) =>
        {
            Some(CaptureFactV1::ObservedMemoryFault)
        }
        TraceEventKindV1::Memory(_) => Some(CaptureFactV1::MemoryAccessOutcomes),
        TraceEventKindV1::Barrier(_) => Some(CaptureFactV1::BarrierPhaseEvents),
        TraceEventKindV1::Allocation(allocation)
            if first_memory_fault.is_some_and(|(sequence, fault_allocation)| {
                event.sequence() < sequence && allocation.allocation() == fault_allocation
            }) =>
        {
            Some(CaptureFactV1::FaultAllocationLayout)
        }
        TraceEventKindV1::Allocation(_) => None,
        TraceEventKindV1::Diagnostic(_) => Some(CaptureFactV1::ObservedDiagnosticOrFault),
    }
}

const fn fact_relevant(goal: CaptureGoalV1, fact: CaptureFactV1) -> bool {
    match goal {
        CaptureGoalV1::MemoryFault => matches!(
            fact,
            CaptureFactV1::KernelIrClaim
                | CaptureFactV1::ArtifactBindingClaim
                | CaptureFactV1::DispatchEnvelope
                | CaptureFactV1::FullInvocationCoverage
                | CaptureFactV1::LaneScope
                | CaptureFactV1::SemanticSites
                | CaptureFactV1::MemoryAccessOutcomes
                | CaptureFactV1::FaultAllocationLayout
                | CaptureFactV1::ObservedMemoryFault
                | CaptureFactV1::ObservedDiagnosticOrFault
        ),
        CaptureGoalV1::BarrierDivergence => matches!(
            fact,
            CaptureFactV1::KernelIrClaim
                | CaptureFactV1::DispatchEnvelope
                | CaptureFactV1::FullInvocationCoverage
                | CaptureFactV1::LaneScope
                | CaptureFactV1::SemanticSites
                | CaptureFactV1::BarrierPhaseEvents
                | CaptureFactV1::AttCaptureManifest
        ),
        CaptureGoalV1::PerformanceHotspot => matches!(
            fact,
            CaptureFactV1::KernelIrClaim
                | CaptureFactV1::ArtifactBindingClaim
                | CaptureFactV1::DispatchEnvelope
                | CaptureFactV1::DispatchTiming
                | CaptureFactV1::SemanticSites
                | CaptureFactV1::AttCaptureManifest
        ),
        CaptureGoalV1::CorrectnessMismatch => matches!(
            fact,
            CaptureFactV1::KernelIrClaim
                | CaptureFactV1::ArtifactBindingClaim
                | CaptureFactV1::DispatchEnvelope
                | CaptureFactV1::FullInvocationCoverage
                | CaptureFactV1::SemanticSites
                | CaptureFactV1::SimulatorControlFlow
                | CaptureFactV1::MemoryAccessOutcomes
                | CaptureFactV1::ObservedDiagnosticOrFault
        ),
    }
}

fn observed_fault(event: &TraceEventV1) -> Option<ObservedFaultRefV1> {
    let (source, kind) = match event.kind() {
        TraceEventKindV1::Memory(memory) => {
            let MemoryOutcomeV1::Fault(fault) = memory.outcome() else {
                return None;
            };
            (ObservedFaultSourceV1::Memory, memory_fault_label(fault))
        }
        TraceEventKindV1::Diagnostic(diagnostic) => (
            ObservedFaultSourceV1::Diagnostic,
            diagnostic_kind_label(diagnostic.kind()),
        ),
        TraceEventKindV1::Dispatch(DispatchEventV1::End(DispatchOutcomeV1::Failed)) => {
            (ObservedFaultSourceV1::Dispatch, "failed")
        }
        TraceEventKindV1::Dispatch(DispatchEventV1::End(DispatchOutcomeV1::Cancelled)) => {
            (ObservedFaultSourceV1::Dispatch, "cancelled")
        }
        _ => return None,
    };
    Some(ObservedFaultRefV1 {
        sequence: event.sequence(),
        source,
        kind,
    })
}

const fn direct_kfd_unsupported(fact: CaptureFactV1) -> UnsupportedCaptureFactV1 {
    UnsupportedCaptureFactV1 {
        tool: CaptureToolFamilyV1::FutureDirectKfd,
        fact,
        reason: UnsupportedCaptureReasonV1::CurrentDirectKfdBoundaryHasNoAuthenticatedDispatch,
        detail: "the current direct-KFD boundary exposes redacted queue lifecycle only, not an authenticated dispatch, completion, timing, or semantic execution trace",
    }
}

const fn identity_view(identity: OpaqueIdentityV1) -> OpaqueIdentityViewV1 {
    OpaqueIdentityViewV1 {
        bytes: *identity.as_bytes(),
    }
}

const fn provenance_label(provenance: FactProvenanceV1) -> &'static str {
    match provenance {
        FactProvenanceV1::Declared => "declared",
        FactProvenanceV1::Proved => "proved",
        FactProvenanceV1::Observed => "observed",
        FactProvenanceV1::Inferred => "inferred",
        FactProvenanceV1::Unavailable { .. } => "unavailable",
    }
}

const fn evidence_kind_label(kind: EvidenceKindV1) -> &'static str {
    match kind {
        EvidenceKindV1::Declaration => "declaration",
        EvidenceKindV1::Proof => "proof",
        EvidenceKindV1::InferenceRule => "inference_rule",
        EvidenceKindV1::RuntimeObservation => "runtime_observation",
        EvidenceKindV1::Artifact => "artifact",
    }
}

const fn memory_fault_label(kind: fe2o3_semantic_trace::MemoryFaultKindV1) -> &'static str {
    use fe2o3_semantic_trace::MemoryFaultKindV1;
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

const fn diagnostic_kind_label(kind: DiagnosticKindV1) -> &'static str {
    match kind {
        DiagnosticKindV1::Trap => "trap",
        DiagnosticKindV1::Assert => "assert",
        DiagnosticKindV1::Fault => "fault",
    }
}
