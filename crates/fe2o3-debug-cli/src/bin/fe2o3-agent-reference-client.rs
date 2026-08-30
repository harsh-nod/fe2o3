#![deny(unsafe_code)]

use std::collections::BTreeSet;
use std::env;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Write};
use std::os::fd::AsRawFd;
use std::os::unix::fs::{FileExt, MetadataExt, OpenOptionsExt};
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{
    Child, ChildStderr, ChildStdin, ChildStdout, Command, ExitCode, ExitStatus, Stdio,
};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{Receiver, SyncSender, TryRecvError, sync_channel};
use std::thread;
use std::time::{Duration, Instant};

use fe2o3_debug_protocol::{
    CapabilityAvailabilityV1, DebugCapabilityNameV1, DebugOperationNameV1, DebugResponseV1,
    DebugResultV1, DiagnosisClassV2, DiagnosisFactV2, DiagnosisOperationV2, DiagnosisResponseV2,
    DiagnosisViewV2, ProtocolLimitsV1, decode_diagnosis_response_line_v2, decode_response_line_v1,
};
use fe2o3_kernel_ir::VerifiedCanonicalKernelIrV7;
use fe2o3_semantic_query::{
    AGENT_PROFILER_PLAN_REQUEST_SCHEMA_V1, AGENT_PROFILER_PLAN_SCHEMA_V1,
    AGENT_PROFILER_REQUEST_SCHEMA_V1, AGENT_PROFILER_RESPONSE_SCHEMA_V1,
    AGENT_PROFILER_VARIANT_COMPARISON_SCHEMA_V1, AGENT_PROFILER_VARIANT_REQUEST_SCHEMA_V1,
    AGENT_PROFILER_VARIANT_RESPONSE_SCHEMA_V1, MAX_AGENT_PROFILER_REQUEST_BYTES_V1,
    MAX_AGENT_PROFILER_RESPONSE_BYTES_V1, MAX_AGENT_PROFILER_VARIANT_REQUEST_BYTES_V1,
    MAX_AGENT_PROFILER_VARIANT_RESPONSE_BYTES_V1, MAX_PROFILER_VARIANT_TREATMENT_BYTES_V1,
    validate_agent_profiler_variant_response_line_v1,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

const WORKFLOW_SCHEMA_V1: &str = "fe2o3-agent-reference-workflow-v1";
const REPORT_SCHEMA_V1: &str = "fe2o3-agent-reference-report-v1";
const MAX_WORKFLOW_BYTES_V1: u64 = 64 * 1024;
const MAX_DEBUG_INPUT_BYTES_V1: u64 = 64 * 1024 * 1024;
const MAX_EXECUTABLE_BYTES_V1: u64 = 512 * 1024 * 1024;
const MAX_CHILD_STDERR_BYTES_V1: u64 = 64 * 1024;
const MAX_REFERENCE_REPORT_BYTES_V1: usize = 16 * 1024 * 1024;
const CHILD_SESSION_TIMEOUT_V1: Duration = Duration::from_secs(10);
const CHILD_REAP_TIMEOUT_V1: Duration = Duration::from_secs(2);
const CHILD_POLL_INTERVAL_V1: Duration = Duration::from_millis(2);
const REQUIRED_VARIANT_GAPS_V1: [&str; 6] = [
    "decoded_att_events",
    "runtime_api_events",
    "copy_events",
    "pc_to_semantic_or_isa_correlation",
    "semantic_ir_isa_change_localization",
    "causal_regression_attribution",
];

static NEXT_SNAPSHOT_V1: AtomicU64 = AtomicU64::new(1);

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct WorkflowV1 {
    schema: String,
    trusted_debugger_executable: PathBuf,
    trusted_profiler_service_executable: PathBuf,
    out_of_bounds: SimulatorCaseV1,
    barrier_divergence: SimulatorCaseV1,
    baseline: TreatmentFilesV1,
    candidate: TreatmentFilesV1,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SimulatorCaseV1 {
    kernel: PathBuf,
    request: PathBuf,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct TreatmentFilesV1 {
    manifest: PathBuf,
    semantic_workload: PathBuf,
    raw_profiler_source: PathBuf,
    bundle: PathBuf,
    schedule: PathBuf,
    artifact: PathBuf,
    isa_projection: Option<PathBuf>,
    counters: Option<PathBuf>,
    pc_samples: Option<PathBuf>,
}

struct LoadedWorkflowV1 {
    debugger_executable: PinnedExecutableV1,
    profiler_service_executable: PinnedExecutableV1,
    out_of_bounds: LoadedSimulatorCaseV1,
    barrier_divergence: LoadedSimulatorCaseV1,
    baseline: LoadedTreatmentV1,
    candidate: LoadedTreatmentV1,
}

struct LoadedSimulatorCaseV1 {
    kernel: Vec<u8>,
    request: Vec<u8>,
}

struct LoadedTreatmentV1 {
    manifest: Vec<u8>,
    semantic_workload: Vec<u8>,
    raw_profiler_source: Vec<u8>,
    bundle: Vec<u8>,
    schedule: Vec<u8>,
    artifact: Vec<u8>,
    isa_projection: Option<Vec<u8>>,
    counters: Option<Vec<u8>>,
    pc_samples: Option<Vec<u8>>,
}

#[derive(Serialize)]
struct ReferenceReportV1 {
    schema: &'static str,
    authority: &'static str,
    executable_manifest: ExecutableManifestV1,
    out_of_bounds: SimulatorDiagnosisReportV1,
    barrier_divergence: SimulatorDiagnosisReportV1,
    variant: VariantReportV1,
    next_capture: CapturePlanReportV1,
}

#[derive(Serialize)]
struct ExecutableManifestV1 {
    debugger: ExecutableContentIdentityV1,
    profiler_service: ExecutableContentIdentityV1,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
struct ExecutableContentIdentityV1 {
    scheme: &'static str,
    sha256: String,
    bytes: u64,
}

#[derive(Serialize)]
struct SimulatorDiagnosisReportV1 {
    class: &'static str,
    claim_truth: &'static str,
    simulated: bool,
    hardware_observed: bool,
    completeness: Value,
    input_manifest_identity: Value,
    response_binding_identity: Value,
    terminal_record_identity: Value,
    transcript_record_identity: Value,
    evidence_manifest_identity: Value,
    diagnosis: Value,
    citations: Value,
    citation_count: usize,
}

#[derive(Serialize)]
struct VariantReportV1 {
    claim_truth: &'static str,
    request_identity: Value,
    baseline_manifest: Value,
    candidate_manifest: Value,
    ranked_explanations: Value,
    unavailable: Value,
    response_identity: Value,
}

#[derive(Serialize)]
struct CapturePlanReportV1 {
    claim_truth: &'static str,
    request_identity: Value,
    disposition: Value,
    minimum_additional_captures: Value,
    selected_missing_evidence: Value,
    provenance: Value,
    evidence: Value,
    first_page_returned: Value,
    second_page_returned: Value,
}

fn main() -> ExitCode {
    match run() {
        Ok(report) => {
            let Ok(bytes) = encode_json_line(&report, MAX_REFERENCE_REPORT_BYTES_V1 as u64) else {
                return ExitCode::from(1);
            };
            if std::io::stdout().lock().write_all(&bytes).is_err() {
                ExitCode::from(1)
            } else {
                ExitCode::SUCCESS
            }
        }
        Err(error) => {
            eprintln!("fe2o3 agent reference workflow failed: {error}");
            ExitCode::from(1)
        }
    }
}

fn run() -> Result<ReferenceReportV1, String> {
    let arguments = env::args_os().skip(1).collect::<Vec<_>>();
    let [flag, workflow_path] = arguments.as_slice() else {
        return Err("expected --workflow WORKFLOW.json".into());
    };
    if flag != "--workflow" {
        return Err("expected --workflow WORKFLOW.json".into());
    }
    let workflow_bytes = read_bounded(workflow_path, MAX_WORKFLOW_BYTES_V1, "workflow")?;
    let workflow: WorkflowV1 =
        serde_json::from_slice(&workflow_bytes).map_err(|_| "invalid workflow JSON")?;
    if workflow.schema != WORKFLOW_SCHEMA_V1 {
        return Err("invalid workflow schema".into());
    }
    let loaded = LoadedWorkflowV1::load(workflow)?;
    let out_of_bounds = diagnose_simulator(
        &loaded.debugger_executable,
        &loaded.out_of_bounds,
        DiagnosisClassV2::MemoryOutOfBounds,
        "memory_out_of_bounds",
    )?;
    let barrier_divergence = diagnose_simulator(
        &loaded.debugger_executable,
        &loaded.barrier_divergence,
        DiagnosisClassV2::WorkgroupBarrierDivergence,
        "workgroup_barrier_divergence",
    )?;
    let variant = compare_variants(
        &loaded.profiler_service_executable,
        &loaded.baseline,
        &loaded.candidate,
    )?;
    let next_capture =
        plan_next_capture(&loaded.profiler_service_executable, &loaded.baseline.bundle)?;
    Ok(ReferenceReportV1 {
        schema: REPORT_SCHEMA_V1,
        authority: "read_only_no_execution_attach_scheduling_or_collection_authority",
        executable_manifest: ExecutableManifestV1 {
            debugger: loaded.debugger_executable.identity().clone(),
            profiler_service: loaded.profiler_service_executable.identity().clone(),
        },
        out_of_bounds,
        barrier_divergence,
        variant,
        next_capture,
    })
}

impl LoadedWorkflowV1 {
    fn load(workflow: WorkflowV1) -> Result<Self, String> {
        Ok(Self {
            debugger_executable: PinnedExecutableV1::open(
                &workflow.trusted_debugger_executable,
                "debugger executable",
            )?,
            profiler_service_executable: PinnedExecutableV1::open(
                &workflow.trusted_profiler_service_executable,
                "profiler service executable",
            )?,
            out_of_bounds: LoadedSimulatorCaseV1::load(workflow.out_of_bounds)?,
            barrier_divergence: LoadedSimulatorCaseV1::load(workflow.barrier_divergence)?,
            baseline: LoadedTreatmentV1::load(workflow.baseline)?,
            candidate: LoadedTreatmentV1::load(workflow.candidate)?,
        })
    }
}

impl LoadedSimulatorCaseV1 {
    fn load(value: SimulatorCaseV1) -> Result<Self, String> {
        Ok(Self {
            kernel: read_bounded(&value.kernel, MAX_DEBUG_INPUT_BYTES_V1, "simulator kernel")?,
            request: read_bounded(
                &value.request,
                MAX_DEBUG_INPUT_BYTES_V1,
                "simulator request",
            )?,
        })
    }
}

impl LoadedTreatmentV1 {
    fn load(value: TreatmentFilesV1) -> Result<Self, String> {
        let result = Self {
            manifest: read_bounded(
                &value.manifest,
                MAX_PROFILER_VARIANT_TREATMENT_BYTES_V1,
                "manifest",
            )?,
            semantic_workload: read_bounded(
                &value.semantic_workload,
                MAX_PROFILER_VARIANT_TREATMENT_BYTES_V1,
                "semantic workload",
            )?,
            raw_profiler_source: read_bounded(
                &value.raw_profiler_source,
                MAX_PROFILER_VARIANT_TREATMENT_BYTES_V1,
                "raw profiler source",
            )?,
            bundle: read_bounded(
                &value.bundle,
                MAX_PROFILER_VARIANT_TREATMENT_BYTES_V1,
                "profiler bundle",
            )?,
            schedule: read_bounded(
                &value.schedule,
                MAX_PROFILER_VARIANT_TREATMENT_BYTES_V1,
                "schedule",
            )?,
            artifact: read_bounded(
                &value.artifact,
                MAX_PROFILER_VARIANT_TREATMENT_BYTES_V1,
                "artifact",
            )?,
            isa_projection: read_optional(&value.isa_projection, "ISA projection")?,
            counters: read_optional(&value.counters, "counter capture")?,
            pc_samples: read_optional(&value.pc_samples, "PC sample capture")?,
        };
        let total = result
            .parts()
            .try_fold(0_u64, |sum, bytes| sum.checked_add(bytes.len() as u64))
            .ok_or("treatment size overflow")?;
        if total > MAX_PROFILER_VARIANT_TREATMENT_BYTES_V1 {
            return Err("treatment exceeds service bound".into());
        }
        Ok(result)
    }

    fn parts(&self) -> impl Iterator<Item = &[u8]> {
        [
            Some(self.manifest.as_slice()),
            Some(self.semantic_workload.as_slice()),
            Some(self.raw_profiler_source.as_slice()),
            Some(self.bundle.as_slice()),
            Some(self.schedule.as_slice()),
            Some(self.artifact.as_slice()),
            self.isa_projection.as_deref(),
            self.counters.as_deref(),
            self.pc_samples.as_deref(),
        ]
        .into_iter()
        .flatten()
    }

    fn json(&self) -> Value {
        json!({
            "manifest_hex": lower_hex(&self.manifest),
            "semantic_workload_hex": lower_hex(&self.semantic_workload),
            "raw_profiler_source_hex": lower_hex(&self.raw_profiler_source),
            "bundle_hex": lower_hex(&self.bundle),
            "schedule_hex": lower_hex(&self.schedule),
            "artifact_hex": lower_hex(&self.artifact),
            "isa_projection_hex": self.isa_projection.as_deref().map(lower_hex),
            "counters_hex": self.counters.as_deref().map(lower_hex),
            "pc_samples_hex": self.pc_samples.as_deref().map(lower_hex),
        })
    }
}

fn diagnose_simulator(
    executable: &PinnedExecutableV1,
    case: &LoadedSimulatorCaseV1,
    expected_class: DiagnosisClassV2,
    class_wire: &'static str,
) -> Result<SimulatorDiagnosisReportV1, String> {
    let kernel = TempSnapshotV1::new("kir", &case.kernel)?;
    let request = TempSnapshotV1::new("json", &case.request)?;
    let limits = ProtocolLimitsV1::default();
    let max_total = limits
        .max_response_line_bytes
        .checked_mul(3)
        .ok_or("debugger response bound overflow")?;
    let mut command = Command::new(executable.proc_path());
    command
        .args(["sim", "--kir-v7"])
        .arg(kernel.path())
        .arg("--request")
        .arg(request.path())
        .args(["--protocol", "jsonl"]);
    let mut child = BoundedChildV1::spawn(
        command,
        executable,
        "debugger",
        limits.max_response_line_bytes,
        max_total,
    )?;
    let requests = [
        json!({"operation":"discover_capabilities","schema":"fe2o3-debug-request-v1","request_id":1,"expected_revision":0}),
        json!({"operation":"continue","schema":"fe2o3-debug-request-v1","request_id":2,"expected_revision":0,"max_events":1_000_000}),
        json!({"operation":"diagnose","schema":"fe2o3-debug-diagnosis-request-v2","request_id":3,"expected_revision":1,"filter":{"class":class_wire},"page":{"limit":1}}),
    ];
    let mut lines = Vec::new();
    lines
        .try_reserve_exact(requests.len())
        .map_err(|_| "could not reserve debugger response slots")?;
    for request in requests {
        lines.push(child.exchange_line(&request, 64 * 1024)?);
    }
    child.finish()?;
    let capabilities = decode_response_line_v1(&lines[0], limits)
        .map_err(|_| "invalid debugger capability response")?;
    let DebugResponseV1::Ok {
        request_id: 1,
        operation: DebugOperationNameV1::DiscoverCapabilities,
        session: capability_session,
        result,
        ..
    } = capabilities
    else {
        return Err("debugger capability discovery failed".into());
    };
    if capability_session.revision != 0 {
        return Err("debugger capability response revision mismatch".into());
    }
    let DebugResultV1::Capabilities { capabilities } = result.as_ref() else {
        return Err("debugger capability result missing".into());
    };
    require_debug_capability(
        capabilities,
        DebugCapabilityNameV1::SemanticTrace,
        CapabilityAvailabilityV1::Available,
    )?;
    require_debug_capability(
        capabilities,
        DebugCapabilityNameV1::KfdDispatchControl,
        CapabilityAvailabilityV1::Unavailable,
    )?;
    let continued = decode_response_line_v1(&lines[1], limits)
        .map_err(|_| "invalid debugger control response")?;
    let DebugResponseV1::Ok {
        request_id: 2,
        operation: DebugOperationNameV1::Continue,
        session: continued_session,
        ..
    } = continued
    else {
        return Err("debugger control response association mismatch".into());
    };
    if continued_session.revision != 1 {
        return Err("debugger control response revision mismatch".into());
    }
    let diagnosis = decode_diagnosis_response_line_v2(&lines[2], limits)
        .map_err(|_| "invalid or unauthenticated diagnosis response")?;
    let DiagnosisResponseV2::Ok {
        request_id: 3,
        operation: DiagnosisOperationV2::Diagnose,
        session,
        completeness,
        diagnoses,
        next_cursor: None,
        ..
    } = diagnosis
    else {
        return Err("diagnosis request failed".into());
    };
    let [diagnosis] = diagnoses.as_slice() else {
        return Err("diagnosis page did not contain exactly one result".into());
    };
    if diagnosis.class != expected_class || !session.simulated || session.hardware_observed {
        return Err("diagnosis truth classification mismatch".into());
    }
    let expected_kir = VerifiedCanonicalKernelIrV7::from_canonical_bytes(case.kernel.clone())
        .map_err(|_| "loaded simulator KIR is not canonical V7")?;
    let DiagnosisFactV2::Declared {
        value: admitted_kir,
    } = diagnosis.input.canonical_kir_v7
    else {
        return Err("diagnosis omitted its admitted KIR identity".into());
    };
    if admitted_kir.sha256.as_bytes() != *expected_kir.identity().digest()
        || admitted_kir.canonical_bytes != case.kernel.len() as u64
    {
        return Err("debugger did not use the exact loaded KIR bytes".into());
    }
    let DiagnosisFactV2::Declared {
        value: admitted_request,
    } = diagnosis.input.dispatch_request
    else {
        return Err("diagnosis omitted its admitted request identity".into());
    };
    let expected_request: [u8; 32] = Sha256::digest(&case.request).into();
    if admitted_request.sha256.as_bytes() != expected_request
        || admitted_request.canonical_bytes != case.request.len() as u64
    {
        return Err("debugger did not use the exact loaded request bytes".into());
    }
    if diagnosis.evidence.citations.is_empty() {
        return Err("diagnosis has no exact citations".into());
    }
    validate_diagnosis_citation_coverage(diagnosis, expected_class)?;
    Ok(SimulatorDiagnosisReportV1 {
        class: class_wire,
        claim_truth: "inferred_from_simulated_semantic_trace",
        simulated: session.simulated,
        hardware_observed: session.hardware_observed,
        completeness: serde_json::to_value(completeness).map_err(|_| "encode completeness")?,
        input_manifest_identity: serde_json::to_value(diagnosis.evidence.input_manifest_identity)
            .map_err(|_| "encode identity")?,
        response_binding_identity: serde_json::to_value(
            diagnosis.evidence.response_binding_identity,
        )
        .map_err(|_| "encode identity")?,
        terminal_record_identity: serde_json::to_value(diagnosis.evidence.terminal_record_identity)
            .map_err(|_| "encode identity")?,
        transcript_record_identity: serde_json::to_value(
            diagnosis.evidence.transcript_record_identity,
        )
        .map_err(|_| "encode identity")?,
        evidence_manifest_identity: serde_json::to_value(diagnosis.evidence.manifest_identity)
            .map_err(|_| "encode identity")?,
        diagnosis: serde_json::to_value(diagnosis).map_err(|_| "encode diagnosis")?,
        citations: serde_json::to_value(&diagnosis.evidence.citations)
            .map_err(|_| "encode citations")?,
        citation_count: diagnosis.evidence.citations.len(),
    })
}

fn require_debug_capability(
    capabilities: &[fe2o3_debug_protocol::CapabilityViewV1],
    name: DebugCapabilityNameV1,
    availability: CapabilityAvailabilityV1,
) -> Result<(), String> {
    if capabilities
        .iter()
        .any(|value| value.name == name && value.availability == availability)
    {
        Ok(())
    } else {
        Err("required debugger capability was not discovered".into())
    }
}

fn validate_diagnosis_citation_coverage(
    diagnosis: &DiagnosisViewV2,
    expected_class: DiagnosisClassV2,
) -> Result<(), String> {
    let fields = diagnosis
        .evidence
        .citations
        .iter()
        .map(|citation| citation.field.as_str())
        .collect::<BTreeSet<_>>();
    if fields.len() != diagnosis.evidence.citations.len() {
        return Err("diagnosis contains duplicate material-claim citations".into());
    }
    let common = [
        "input.dispatch_request",
        "input.canonical_kir_v7",
        "input.simulation_bundle",
        "input.production_kir",
        "input.kernel_abi_identity",
        "input.source_lineage",
        "input.source_map_v2",
        "input.finalized_artifact",
        "input.property_proof",
        "context.dispatch",
        "context.workgroup",
        "context.workitem",
        "context.wave",
        "context.lane",
        "site",
        "source_operation",
        "memory_region",
        "barrier",
    ];
    if common.iter().any(|field| !fields.contains(field)) {
        return Err("diagnosis omitted a common material-claim citation".into());
    }
    let specific: &[&str] = match expected_class {
        DiagnosisClassV2::MemoryOutOfBounds => &[
            "memory_region.allocation_contract",
            "memory_region.abi_argument",
            "memory_region.logical_element",
            "memory_region.legal_bounds",
        ],
        DiagnosisClassV2::WorkgroupBarrierDivergence => &[
            "barrier.phase",
            "barrier.semantics",
            "barrier.lds_epoch.current",
            "barrier.lds_epoch.after_release",
            "barrier.observed_arrivals",
            "barrier.expected_participants",
            "barrier.expected_participant_set",
            "barrier.arrived_participants",
            "barrier.waiting_participants",
            "barrier.exited_participants",
        ],
        _ => return Err("reference workflow received an unscoped diagnosis class".into()),
    };
    if specific.iter().any(|field| !fields.contains(field)) {
        return Err("diagnosis omitted a class-specific material-claim citation".into());
    }
    Ok(())
}

#[derive(Clone, Copy)]
enum ExpectedAgentResultV1 {
    Capabilities,
    CaptureOpened,
    Page,
    CapturePlan,
}

impl ExpectedAgentResultV1 {
    fn wire(self) -> &'static str {
        match self {
            Self::Capabilities => "capabilities",
            Self::CaptureOpened => "capture_opened",
            Self::Page => "page",
            Self::CapturePlan => "capture_plan",
        }
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AgentOkWireV1<T> {
    status: String,
    schema: String,
    request_id: u64,
    response_revision: u64,
    value: T,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AgentCapabilitiesWireV1 {
    result: String,
    capabilities: Vec<Value>,
    limits: AgentLimitsWireV1,
    evidence: AgentEvidenceWireV1,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AgentLimitsWireV1 {
    max_request_bytes: u64,
    max_response_bytes: u64,
    max_requests: u32,
    max_open_captures: u8,
    max_page_items: u16,
    max_bundle_bytes: u64,
    max_plan_missing_facts: u8,
    max_plan_compute_units: u8,
    max_plan_storage_bytes: u64,
    max_plan_records: u64,
    max_plan_overhead_basis_points: u32,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AgentCaptureOpenedWireV1 {
    result: String,
    context: AgentContextWireV1,
    coverage: Value,
    capture_capabilities: Vec<Value>,
    audit: Value,
    evidence: AgentEvidenceWireV1,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AgentPageResultWireV1 {
    result: String,
    page: AgentPageWireV1,
    evidence: AgentEvidenceWireV1,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AgentCapturePlanWireV1 {
    result: String,
    plan: Value,
    evidence: AgentEvidenceWireV1,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AgentContextWireV1 {
    bundle_identity: Value,
    run_identity: Value,
    source_kind: String,
    device_count: u64,
    dispatch_count: u64,
    att_reference_count: u64,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AgentPageWireV1 {
    context: AgentContextWireV1,
    kind: String,
    returned: u16,
    next_cursor: Option<Value>,
    items: Vec<Value>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AgentEvidenceWireV1 {
    origin: Value,
    service_contract: Value,
    captures: Vec<Value>,
    records: Vec<Value>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct VariantOkWireV1 {
    status: String,
    schema: String,
    request_id: u64,
    response_revision: u64,
    value: VariantValueWireV1,
    response_identity: Value,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct VariantValueWireV1 {
    result: String,
    #[serde(default)]
    capabilities: Option<Value>,
    #[serde(default)]
    comparison: Option<Value>,
}

fn validate_agent_ok_envelope<T>(
    response: &AgentOkWireV1<T>,
    request_id: u64,
    response_revision: u64,
) -> Result<(), String> {
    if response.status != "ok"
        || response.schema != AGENT_PROFILER_RESPONSE_SCHEMA_V1
        || response.request_id != request_id
        || response.response_revision != response_revision
    {
        return Err("Agent V1 response envelope does not match the issued request".into());
    }
    Ok(())
}

fn validate_agent_context_wire_v1(context: &AgentContextWireV1) -> Result<(), String> {
    if context.bundle_identity.is_null()
        || context.run_identity.is_null()
        || context.source_kind.is_empty()
    {
        return Err("Agent V1 context identity is incomplete".into());
    }
    let _ = (
        context.device_count,
        context.dispatch_count,
        context.att_reference_count,
    );
    Ok(())
}

fn validate_agent_evidence_wire_v1(evidence: &AgentEvidenceWireV1) -> Result<(), String> {
    if evidence.origin.is_null() || evidence.service_contract.is_null() {
        return Err("Agent V1 evidence identity is incomplete".into());
    }
    let _ = (&evidence.captures, &evidence.records);
    Ok(())
}

fn validate_agent_page_wire_v1(page: &AgentPageWireV1) -> Result<(), String> {
    validate_agent_context_wire_v1(&page.context)?;
    if page.kind.is_empty() || usize::from(page.returned) != page.items.len() {
        return Err("Agent V1 page shape is inconsistent".into());
    }
    let _ = &page.next_cursor;
    Ok(())
}

fn validate_agent_response_line_v1(
    line: &[u8],
    request_id: u64,
    response_revision: u64,
    expected: ExpectedAgentResultV1,
) -> Result<Value, String> {
    match expected {
        ExpectedAgentResultV1::Capabilities => {
            let response: AgentOkWireV1<AgentCapabilitiesWireV1> =
                serde_json::from_slice(line).map_err(|_| "invalid Agent V1 capabilities wire")?;
            validate_agent_ok_envelope(&response, request_id, response_revision)?;
            if response.value.result != expected.wire() {
                return Err("Agent V1 capabilities result kind mismatch".into());
            }
            validate_agent_evidence_wire_v1(&response.value.evidence)?;
            let _ = (
                response.value.capabilities,
                response.value.limits.max_request_bytes,
                response.value.limits.max_response_bytes,
                response.value.limits.max_requests,
                response.value.limits.max_open_captures,
                response.value.limits.max_page_items,
                response.value.limits.max_bundle_bytes,
                response.value.limits.max_plan_missing_facts,
                response.value.limits.max_plan_compute_units,
                response.value.limits.max_plan_storage_bytes,
                response.value.limits.max_plan_records,
                response.value.limits.max_plan_overhead_basis_points,
            );
        }
        ExpectedAgentResultV1::CaptureOpened => {
            let response: AgentOkWireV1<AgentCaptureOpenedWireV1> =
                serde_json::from_slice(line).map_err(|_| "invalid Agent V1 open wire")?;
            validate_agent_ok_envelope(&response, request_id, response_revision)?;
            if response.value.result != expected.wire() {
                return Err("Agent V1 open result kind mismatch".into());
            }
            validate_agent_context_wire_v1(&response.value.context)?;
            validate_agent_evidence_wire_v1(&response.value.evidence)?;
            let _ = (
                response.value.coverage,
                response.value.capture_capabilities,
                response.value.audit,
            );
        }
        ExpectedAgentResultV1::Page => {
            let response: AgentOkWireV1<AgentPageResultWireV1> =
                serde_json::from_slice(line).map_err(|_| "invalid Agent V1 page wire")?;
            validate_agent_ok_envelope(&response, request_id, response_revision)?;
            if response.value.result != expected.wire() {
                return Err("Agent V1 page result kind mismatch".into());
            }
            validate_agent_page_wire_v1(&response.value.page)?;
            validate_agent_evidence_wire_v1(&response.value.evidence)?;
        }
        ExpectedAgentResultV1::CapturePlan => {
            let response: AgentOkWireV1<AgentCapturePlanWireV1> =
                serde_json::from_slice(line).map_err(|_| "invalid Agent V1 plan wire")?;
            validate_agent_ok_envelope(&response, request_id, response_revision)?;
            if response.value.result != expected.wire() {
                return Err("Agent V1 plan result kind mismatch".into());
            }
            validate_agent_evidence_wire_v1(&response.value.evidence)?;
            let _ = response.value.plan;
        }
    }
    serde_json::from_slice(line).map_err(|_| "invalid Agent V1 JSON response".into())
}

fn exchange_agent_v1(
    session: &mut JsonlChildV1<'_>,
    request: &Value,
    request_id: u64,
    response_revision: u64,
    expected: ExpectedAgentResultV1,
) -> Result<Value, String> {
    if request["schema"] != AGENT_PROFILER_REQUEST_SCHEMA_V1 || request["request_id"] != request_id
    {
        return Err("issued Agent V1 request association is invalid".into());
    }
    let line = session.exchange_line(request, MAX_AGENT_PROFILER_REQUEST_BYTES_V1)?;
    validate_agent_response_line_v1(&line, request_id, response_revision, expected)
}

fn exchange_variant_v1(
    session: &mut JsonlChildV1<'_>,
    request: &Value,
    request_id: u64,
    response_revision: u64,
    expected_result: &str,
) -> Result<Value, String> {
    if request["schema"] != AGENT_PROFILER_VARIANT_REQUEST_SCHEMA_V1
        || request["request_id"] != request_id
        || request["expected_revision"] != response_revision - 1
    {
        return Err("issued Variant V1 request association is invalid".into());
    }
    let line = session.exchange_line(request, MAX_AGENT_PROFILER_VARIANT_REQUEST_BYTES_V1)?;
    validate_agent_profiler_variant_response_line_v1(&line)
        .map_err(|_| "profiler response identity mismatch")?;
    let response: VariantOkWireV1 =
        serde_json::from_slice(&line).map_err(|_| "invalid Variant V1 response wire")?;
    if response.status != "ok"
        || response.schema != AGENT_PROFILER_VARIANT_RESPONSE_SCHEMA_V1
        || response.request_id != request_id
        || response.response_revision != response_revision
        || response.value.result != expected_result
        || response.response_identity.is_null()
        || (expected_result == "capabilities"
            && (response.value.capabilities.is_none() || response.value.comparison.is_some()))
        || (expected_result == "comparison"
            && (response.value.comparison.is_none() || response.value.capabilities.is_some()))
    {
        return Err("Variant V1 response association mismatch".into());
    }
    serde_json::from_slice(&line).map_err(|_| "invalid Variant V1 JSON response".into())
}

fn compare_variants(
    executable: &PinnedExecutableV1,
    baseline: &LoadedTreatmentV1,
    candidate: &LoadedTreatmentV1,
) -> Result<VariantReportV1, String> {
    let mut session = JsonlChildV1::spawn(
        executable,
        "variant-jsonl",
        MAX_AGENT_PROFILER_VARIANT_RESPONSE_BYTES_V1,
        2,
    )?;
    let capabilities_request = json!({
        "operation":"discover_capabilities",
        "schema":AGENT_PROFILER_VARIANT_REQUEST_SCHEMA_V1,
        "request_id":1,
        "expected_revision":0
    });
    let capabilities =
        exchange_variant_v1(&mut session, &capabilities_request, 1, 1, "capabilities")?;
    require_eq(&capabilities["status"], "ok", "variant discovery")?;
    let variant_capabilities = &capabilities["value"]["capabilities"];
    require_eq(
        &variant_capabilities["authority"],
        "read_only_no_execution_attach_scheduling_or_collection_authority",
        "variant authority",
    )?;
    require_eq(
        &variant_capabilities["exact_input_encoding"],
        "canonical_lowercase_hex_of_exact_bytes",
        "variant input encoding",
    )?;
    let compare_capability = variant_capabilities["operations"]
        .as_array()
        .ok_or("missing Variant operation capabilities")?
        .iter()
        .find(|entry| entry["operation"] == "compare_variants" && entry["available"] == true)
        .ok_or("Variant comparison capability absent")?;
    require_eq(
        &compare_capability["request_schema"],
        AGENT_PROFILER_VARIANT_REQUEST_SCHEMA_V1,
        "Variant comparison request schema",
    )?;
    require_eq(
        &compare_capability["result_schema"],
        AGENT_PROFILER_VARIANT_COMPARISON_SCHEMA_V1,
        "Variant comparison result schema",
    )?;
    if variant_capabilities["max_request_bytes"].as_u64()
        != Some(MAX_AGENT_PROFILER_VARIANT_REQUEST_BYTES_V1)
        || variant_capabilities["max_response_bytes"].as_u64()
            != Some(MAX_AGENT_PROFILER_VARIANT_RESPONSE_BYTES_V1)
        || variant_capabilities["max_requests"]
            .as_u64()
            .is_none_or(|count| count < 2)
    {
        return Err("Variant discovered bounds cannot serve this workflow".into());
    }
    let comparison_request = json!({
        "operation":"compare_variants",
        "schema":AGENT_PROFILER_VARIANT_REQUEST_SCHEMA_V1,
        "request_id":2,
        "expected_revision":1,
        "baseline":baseline.json(),
        "candidate":candidate.json(),
    });
    let compared = exchange_variant_v1(&mut session, &comparison_request, 2, 2, "comparison")?;
    session.finish()?;
    require_eq(&compared["status"], "ok", "variant comparison")?;
    let comparison = &compared["value"]["comparison"];
    let explanations = comparison["ranked_explanations"]
        .as_array()
        .ok_or("missing ranked explanations")?;
    if explanations.is_empty()
        || explanations
            .iter()
            .any(|entry| entry["evidence"].as_array().is_none_or(Vec::is_empty))
    {
        return Err("ranked explanation lacks exact evidence".into());
    }
    let unavailable = comparison["unavailable"]
        .as_array()
        .ok_or("missing typed unavailable results")?;
    let kinds = unavailable
        .iter()
        .filter_map(|entry| entry["kind"].as_str())
        .collect::<BTreeSet<_>>();
    if REQUIRED_VARIANT_GAPS_V1
        .iter()
        .any(|kind| !kinds.contains(kind))
        || compared["response_identity"].is_null()
    {
        return Err("Variant truth-boundary gaps are incomplete or uncited".into());
    }
    Ok(VariantReportV1 {
        claim_truth: "conservative_co_observation_not_causal_attribution",
        request_identity: comparison["request_identity"].clone(),
        baseline_manifest: comparison["baseline_treatment"]["manifest"].clone(),
        candidate_manifest: comparison["candidate_treatment"]["manifest"].clone(),
        ranked_explanations: comparison["ranked_explanations"].clone(),
        unavailable: comparison["unavailable"].clone(),
        response_identity: compared["response_identity"].clone(),
    })
}

fn validate_dispatch_page_v1(
    response: &Value,
    capture: &Value,
    prior_dispatch: Option<&Value>,
    require_next_cursor: bool,
) -> Result<(Value, Value), String> {
    let page = &response["value"]["page"];
    let evidence = &response["value"]["evidence"];
    let items = page["items"]
        .as_array()
        .ok_or("Agent V1 dispatch page items missing")?;
    if page["kind"] != "dispatches"
        || page["context"]["bundle_identity"] != *capture
        || page["context"]["dispatch_count"] != 2
        || page["returned"] != 1
        || items.len() != 1
        || items[0]["item"] != "dispatch"
        || !evidence["captures"]
            .as_array()
            .is_some_and(|values| values.contains(capture))
    {
        return Err("Agent V1 dispatch page is not bound to the selected capture".into());
    }
    let dispatch = items[0]["dispatch"]["identity"].clone();
    if dispatch.is_null()
        || prior_dispatch.is_some_and(|prior| *prior == dispatch)
        || items[0]["dispatch"]["evidence"]["record"] != dispatch
        || items[0]["dispatch"]["evidence"]["bundle"] != capture["digest"]
        || evidence["records"]
            .as_array()
            .is_none_or(|values| !values.is_empty())
    {
        return Err("Agent V1 dispatch page repeated or omitted its record identity".into());
    }
    let next_cursor = page["next_cursor"].clone();
    if require_next_cursor {
        if next_cursor.is_null()
            || next_cursor["query_binding"].is_null()
            || next_cursor["position"] != 1
        {
            return Err("Agent V1 first dispatch cursor did not advance exactly once".into());
        }
    } else if !next_cursor.is_null() {
        return Err("Agent V1 second dispatch page did not exhaust the capture".into());
    }
    Ok((dispatch, next_cursor))
}

fn plan_next_capture(
    executable: &PinnedExecutableV1,
    bundle: &[u8],
) -> Result<CapturePlanReportV1, String> {
    let mut session =
        JsonlChildV1::spawn(executable, "jsonl", MAX_AGENT_PROFILER_RESPONSE_BYTES_V1, 5)?;
    let capabilities_request = json!({
        "operation":"discover_capabilities","schema":AGENT_PROFILER_REQUEST_SCHEMA_V1,"request_id":1
    });
    let capabilities = exchange_agent_v1(
        &mut session,
        &capabilities_request,
        1,
        1,
        ExpectedAgentResultV1::Capabilities,
    )?;
    let capability = capabilities["value"]["capabilities"]
        .as_array()
        .ok_or("missing Agent V1 capabilities")?
        .iter()
        .find(|entry| entry["operation"] == "plan_next_capture")
        .ok_or("capture planning capability absent")?;
    let limits = &capabilities["value"]["limits"];
    if limits["max_requests"]
        .as_u64()
        .is_none_or(|count| count < 5)
        || limits["max_page_items"]
            .as_u64()
            .is_none_or(|count| count < 1)
        || limits["max_bundle_bytes"]
            .as_u64()
            .is_none_or(|count| count < bundle.len() as u64)
    {
        return Err("Agent V1 discovered bounds cannot serve this workflow".into());
    }
    require_eq(
        &capability["request_contract_schema"],
        AGENT_PROFILER_PLAN_REQUEST_SCHEMA_V1,
        "plan request schema",
    )?;
    require_eq(
        &capability["result_contract_schema"],
        AGENT_PROFILER_PLAN_SCHEMA_V1,
        "plan result schema",
    )?;
    let open_request = json!({
        "operation":"open_capture","schema":AGENT_PROFILER_REQUEST_SCHEMA_V1,"request_id":2,"bundle_hex":lower_hex(bundle)
    });
    let opened = exchange_agent_v1(
        &mut session,
        &open_request,
        2,
        2,
        ExpectedAgentResultV1::CaptureOpened,
    )?;
    let capture = opened["value"]["context"]["bundle_identity"].clone();
    if capture.is_null()
        || opened["value"]["context"]["dispatch_count"] != 2
        || !opened["value"]["evidence"]["captures"]
            .as_array()
            .is_some_and(|values| values.contains(&capture))
        || opened["value"]["audit"]["effect"] != "registered"
    {
        return Err("opened capture identity or audit is not bound".into());
    }
    let first_request = json!({
        "operation":"list_dispatches","schema":AGENT_PROFILER_REQUEST_SCHEMA_V1,"request_id":3,"capture":capture,"page":{"limit":1,"cursor":null}
    });
    let first = exchange_agent_v1(
        &mut session,
        &first_request,
        3,
        3,
        ExpectedAgentResultV1::Page,
    )?;
    let (dispatch, cursor) = validate_dispatch_page_v1(&first, &capture, None, true)?;
    let second_request = json!({
        "operation":"list_dispatches","schema":AGENT_PROFILER_REQUEST_SCHEMA_V1,"request_id":4,"capture":capture,"page":{"limit":1,"cursor":cursor}
    });
    let second = exchange_agent_v1(
        &mut session,
        &second_request,
        4,
        4,
        ExpectedAgentResultV1::Page,
    )?;
    let (second_dispatch, second_cursor) =
        validate_dispatch_page_v1(&second, &capture, Some(&dispatch), false)?;
    if !second_cursor.is_null() || second_dispatch == dispatch {
        return Err("Agent V1 dispatch cursor repeated a page".into());
    }
    let plan_request = json!({
        "operation":"plan_next_capture","schema":AGENT_PROFILER_REQUEST_SCHEMA_V1,"request_id":5,"capture":capture,
        "planning":{
            "schema":AGENT_PROFILER_PLAN_REQUEST_SCHEMA_V1,
            "goal":"schedule_resource_regression",
            "ambiguity":"scheduling_delay_vs_resource_pressure",
            "missing_evidence":["hardware_counter_measurements"],
            "target":{"compute_units":[],"kernel_ir":null,"dispatch":dispatch},
            "constraints":{"maximum_overhead_basis_points":1_000_000,"maximum_storage_bytes":16_777_216,"maximum_records":100_000}
        }
    });
    let planned = exchange_agent_v1(
        &mut session,
        &plan_request,
        5,
        5,
        ExpectedAgentResultV1::CapturePlan,
    )?;
    session.finish()?;
    require_eq(&planned["status"], "ok", "capture plan")?;
    let plan = &planned["value"]["plan"];
    let evidence = &planned["value"]["evidence"];
    if plan["provenance"].as_array().is_none_or(Vec::is_empty)
        || plan["selected_missing_evidence"]
            .as_array()
            .is_none_or(Vec::is_empty)
        || evidence.is_null()
    {
        return Err("capture plan lacks provenance or selected evidence".into());
    }
    if plan["selected_missing_evidence"] != json!(["hardware_counter_measurements"])
        || plan["minimum_additional_captures"] != 1
        || !evidence["captures"]
            .as_array()
            .is_some_and(|values| values.contains(&capture))
        || !evidence["records"]
            .as_array()
            .is_some_and(|values| values.contains(&dispatch))
        || evidence["service_contract"].is_null()
    {
        return Err("minimum capture plan is not covered by exact evidence".into());
    }
    let provenance = plan["provenance"]
        .as_array()
        .ok_or("capture plan provenance is not an array")?;
    let has_subject = |kind: &str, origin: &str, field: &str, expected: &Value| {
        provenance.iter().any(|entry| {
            entry["kind"] == kind
                && entry["origin"] == origin
                && entry["subject"][field] == *expected
        })
    };
    if !has_subject(
        "planning_request",
        "declared",
        "identity",
        &plan["request_identity"],
    ) || !has_subject("capture_bundle", "observed", "content", &capture)
        || !has_subject("dispatch_record", "observed", "identity", &dispatch)
    {
        return Err("capture plan provenance is not covered by selected identities".into());
    }
    Ok(CapturePlanReportV1 {
        claim_truth: "inferred_minimum_next_capture_plan",
        request_identity: plan["request_identity"].clone(),
        disposition: plan["disposition"].clone(),
        minimum_additional_captures: plan["minimum_additional_captures"].clone(),
        selected_missing_evidence: plan["selected_missing_evidence"].clone(),
        provenance: plan["provenance"].clone(),
        evidence: evidence.clone(),
        first_page_returned: first["value"]["page"]["returned"].clone(),
        second_page_returned: second["value"]["page"]["returned"].clone(),
    })
}

struct JsonlChildV1<'a> {
    inner: BoundedChildV1<'a>,
}

impl<'a> JsonlChildV1<'a> {
    fn spawn(
        executable: &'a PinnedExecutableV1,
        mode: &str,
        max_response: u64,
        response_count: usize,
    ) -> Result<Self, String> {
        let max_line = usize::try_from(max_response).map_err(|_| "response bound is too large")?;
        let max_total = max_line
            .checked_mul(response_count)
            .ok_or("response total bound overflow")?;
        let mut command = Command::new(executable.proc_path());
        command.arg(mode);
        Ok(Self {
            inner: BoundedChildV1::spawn(
                command,
                executable,
                "profiler service",
                max_line,
                max_total,
            )?,
        })
    }

    fn exchange_line(&mut self, request: &Value, max_request: u64) -> Result<Vec<u8>, String> {
        self.inner.exchange_line(request, max_request)
    }

    fn finish(self) -> Result<(), String> {
        self.inner.finish()
    }
}

enum WriterCommandV1 {
    Write {
        bytes: Vec<u8>,
        completed: SyncSender<Result<(), ()>>,
    },
}

enum StdoutEventV1 {
    Line(Vec<u8>),
    Failure(&'static str),
    Eof,
}

struct StderrCaptureV1 {
    bytes: Vec<u8>,
    overflow: bool,
    io_failed: bool,
}

struct OwnedChildGuardV1 {
    child: Child,
    process_group: u32,
    armed: bool,
}

impl OwnedChildGuardV1 {
    fn new(child: Child) -> Self {
        let process_group = child.id();
        Self {
            child,
            process_group,
            armed: true,
        }
    }

    fn try_wait(&mut self) -> Result<Option<ExitStatus>, String> {
        self.child
            .try_wait()
            .map_err(|_| "owned child wait failed".into())
    }

    fn terminate_and_reap(&mut self) -> Result<ExitStatus, String> {
        let observed = self.try_wait();
        let termination = self.signal_owned_process_group();
        let observed = observed?;
        termination?;
        let status = if let Some(status) = observed {
            status
        } else {
            let deadline = Instant::now() + CHILD_REAP_TIMEOUT_V1;
            loop {
                if let Some(status) = self.try_wait()? {
                    break status;
                }
                if Instant::now() >= deadline {
                    return Err("owned child bounded reap timed out".into());
                }
                thread::sleep(CHILD_POLL_INTERVAL_V1);
            }
        };
        self.wait_for_process_group_absence()?;
        Ok(status)
    }

    fn terminate_remaining_process_group(&self) -> Result<(), String> {
        self.signal_owned_process_group()?;
        self.wait_for_process_group_absence()
    }

    fn wait_for_process_group_absence(&self) -> Result<(), String> {
        let deadline = Instant::now() + CHILD_REAP_TIMEOUT_V1;
        loop {
            if !self.process_group_exists()? {
                return Ok(());
            }
            if Instant::now() >= deadline {
                return Err("owned child process group survived termination".into());
            }
            thread::sleep(CHILD_POLL_INTERVAL_V1);
        }
    }

    #[allow(unsafe_code)]
    fn signal_owned_process_group(&self) -> Result<(), String> {
        let process_group = i32::try_from(self.process_group)
            .map_err(|_| "owned child process group is invalid")?;
        // SAFETY: `process_group` is captured from the successfully spawned
        // child after `process_group(0)`. A negative PID targets only that
        // owned group; no pointers or borrowed memory cross the syscall.
        let result = unsafe { libc::kill(-process_group, libc::SIGKILL) };
        if result == 0 {
            return Ok(());
        }
        let error = std::io::Error::last_os_error();
        if error.raw_os_error() == Some(libc::ESRCH) {
            Ok(())
        } else {
            Err("owned child process-group termination failed".into())
        }
    }

    #[allow(unsafe_code)]
    fn process_group_exists(&self) -> Result<bool, String> {
        let process_group = i32::try_from(self.process_group)
            .map_err(|_| "owned child process group is invalid")?;
        // SAFETY: signal zero observes only the existence/permission state of
        // the positive process group captured from this owned child.
        let result = unsafe { libc::kill(-process_group, 0) };
        if result == 0 {
            return Ok(true);
        }
        let error = std::io::Error::last_os_error();
        match error.raw_os_error() {
            Some(libc::ESRCH) => Ok(false),
            Some(libc::EPERM) => Err("owned child process-group visibility was lost".into()),
            _ => Err("owned child process-group observation failed".into()),
        }
    }

    fn disarm(&mut self) {
        self.armed = false;
    }
}

impl Drop for OwnedChildGuardV1 {
    fn drop(&mut self) {
        if self.armed {
            let _ = self.terminate_and_reap();
        }
    }
}

struct BoundedChildV1<'a> {
    owned: OwnedChildGuardV1,
    writer: Option<SyncSender<WriterCommandV1>>,
    stdout: Receiver<StdoutEventV1>,
    stderr: Receiver<StderrCaptureV1>,
    deadline: Instant,
    label: &'static str,
    executable: &'a PinnedExecutableV1,
}

impl<'a> BoundedChildV1<'a> {
    fn spawn(
        command: Command,
        executable: &'a PinnedExecutableV1,
        label: &'static str,
        max_stdout_line: usize,
        max_stdout_total: usize,
    ) -> Result<Self, String> {
        Self::spawn_with_timeout(
            command,
            executable,
            label,
            max_stdout_line,
            max_stdout_total,
            CHILD_SESSION_TIMEOUT_V1,
        )
    }

    fn spawn_with_timeout(
        mut command: Command,
        executable: &'a PinnedExecutableV1,
        label: &'static str,
        max_stdout_line: usize,
        max_stdout_total: usize,
        timeout: Duration,
    ) -> Result<Self, String> {
        if max_stdout_line == 0 || max_stdout_total < max_stdout_line {
            return Err("invalid child output bounds".into());
        }
        if timeout.is_zero() {
            return Err("invalid child deadline".into());
        }
        executable.revalidate(label)?;
        command
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .process_group(0);
        let child = command
            .spawn()
            .map_err(|_| format!("could not spawn {label}"))?;
        let mut owned = OwnedChildGuardV1::new(child);
        let input = owned
            .child
            .stdin
            .take()
            .ok_or_else(|| format!("{label} stdin unavailable"))?;
        let output = owned
            .child
            .stdout
            .take()
            .ok_or_else(|| format!("{label} stdout unavailable"))?;
        let error = owned
            .child
            .stderr
            .take()
            .ok_or_else(|| format!("{label} stderr unavailable"))?;
        let writer = spawn_child_writer_v1(input);
        let stdout = spawn_stdout_reader_v1(output, max_stdout_line, max_stdout_total)?;
        let stderr = spawn_stderr_reader_v1(error, MAX_CHILD_STDERR_BYTES_V1)?;
        let result = Self {
            owned,
            writer: Some(writer),
            stdout,
            stderr,
            deadline: Instant::now() + timeout,
            label,
            executable,
        };
        result.executable.revalidate(label)?;
        Ok(result)
    }

    fn remaining(&self) -> Result<Duration, String> {
        self.deadline
            .checked_duration_since(Instant::now())
            .ok_or_else(|| format!("{} deadline expired", self.label))
    }

    fn exchange_line(&mut self, request: &Value, max_request: u64) -> Result<Vec<u8>, String> {
        match self.stdout.try_recv() {
            Err(TryRecvError::Empty) => {}
            Ok(_) => return Err(format!("{} emitted an unsolicited response", self.label)),
            Err(TryRecvError::Disconnected) => {
                return Err(format!("{} stdout closed before request", self.label));
            }
        }
        let bytes = encode_json_line(request, max_request)?;
        let (completed_tx, completed_rx) = sync_channel(1);
        self.writer
            .as_ref()
            .ok_or_else(|| format!("{} input closed", self.label))?
            .send(WriterCommandV1::Write {
                bytes,
                completed: completed_tx,
            })
            .map_err(|_| format!("{} request writer stopped", self.label))?;
        match completed_rx.recv_timeout(self.remaining()?) {
            Ok(Ok(())) => {}
            Ok(Err(())) => return Err(format!("{} request write failed", self.label)),
            Err(_) => return Err(format!("{} request write timed out", self.label)),
        }
        match self.stdout.recv_timeout(self.remaining()?) {
            Ok(StdoutEventV1::Line(line)) => Ok(line),
            Ok(StdoutEventV1::Failure(reason)) => Err(format!("{} {reason}", self.label)),
            Ok(StdoutEventV1::Eof) => Err(format!("{} stdout ended before response", self.label)),
            Err(_) => Err(format!("{} response timed out", self.label)),
        }
    }

    fn finish(mut self) -> Result<(), String> {
        self.writer.take();
        let mut trailing_output = false;
        let mut stdout_eof = false;
        let status = loop {
            loop {
                match self.stdout.try_recv() {
                    Ok(StdoutEventV1::Line(_)) => trailing_output = true,
                    Ok(StdoutEventV1::Failure(reason)) => {
                        return Err(format!("{} {reason}", self.label));
                    }
                    Ok(StdoutEventV1::Eof) => stdout_eof = true,
                    Err(TryRecvError::Empty) => break,
                    Err(TryRecvError::Disconnected) if stdout_eof => break,
                    Err(TryRecvError::Disconnected) => {
                        return Err(format!("{} stdout reader stopped", self.label));
                    }
                }
            }
            if let Some(status) = self.owned.try_wait()? {
                break status;
            }
            self.remaining()?;
            thread::sleep(CHILD_POLL_INTERVAL_V1);
        };
        while !stdout_eof {
            match self.stdout.recv_timeout(self.remaining()?) {
                Ok(StdoutEventV1::Line(_)) => trailing_output = true,
                Ok(StdoutEventV1::Failure(reason)) => {
                    return Err(format!("{} {reason}", self.label));
                }
                Ok(StdoutEventV1::Eof) => stdout_eof = true,
                Err(_) => return Err(format!("{} stdout EOF timed out", self.label)),
            }
        }
        let stderr = self
            .stderr
            .recv_timeout(self.remaining()?)
            .map_err(|_| format!("{} stderr drain timed out", self.label))?;
        if !status.success() {
            return Err(format!("{} failed", self.label));
        }
        if trailing_output {
            return Err(format!("{} emitted trailing output", self.label));
        }
        if stderr.io_failed {
            return Err(format!("{} stderr read failed", self.label));
        }
        if stderr.overflow {
            return Err(format!("{} stderr exceeded bound", self.label));
        }
        if !stderr.bytes.is_empty() {
            return Err(format!("{} wrote unexpected stderr", self.label));
        }
        self.executable.revalidate(self.label)?;
        self.owned.terminate_remaining_process_group()?;
        self.owned.disarm();
        Ok(())
    }
}

fn spawn_child_writer_v1(mut input: ChildStdin) -> SyncSender<WriterCommandV1> {
    let (sender, receiver) = sync_channel::<WriterCommandV1>(1);
    thread::spawn(move || {
        while let Ok(WriterCommandV1::Write { bytes, completed }) = receiver.recv() {
            let result = input.write_all(&bytes).and_then(|()| input.flush());
            let _ = completed.send(result.map_err(|_| ()));
        }
    });
    sender
}

fn spawn_stdout_reader_v1(
    mut output: ChildStdout,
    max_line: usize,
    max_total: usize,
) -> Result<Receiver<StdoutEventV1>, String> {
    let line_capacity = max_line
        .checked_add(1)
        .ok_or("stdout line bound overflow")?;
    let mut line = Vec::new();
    line.try_reserve_exact(line_capacity)
        .map_err(|_| "could not reserve bounded stdout line")?;
    let (sender, receiver) = sync_channel(1);
    thread::spawn(move || {
        let mut total = 0_usize;
        let mut buffer = [0_u8; 8 * 1024];
        loop {
            let read_limit = max_total
                .saturating_sub(total)
                .saturating_add(1)
                .min(buffer.len());
            let read = match output.read(&mut buffer[..read_limit]) {
                Ok(0) => {
                    let event = if line.is_empty() {
                        StdoutEventV1::Eof
                    } else {
                        StdoutEventV1::Failure("stdout ended without a newline")
                    };
                    let _ = sender.send(event);
                    return;
                }
                Ok(read) => read,
                Err(_) => {
                    let _ = sender.send(StdoutEventV1::Failure("stdout read failed"));
                    return;
                }
            };
            total = match total.checked_add(read) {
                Some(total) if total <= max_total => total,
                _ => {
                    let _ = sender.send(StdoutEventV1::Failure("stdout exceeded total bound"));
                    return;
                }
            };
            for byte in &buffer[..read] {
                if line.capacity() == 0 && line.try_reserve_exact(line_capacity).is_err() {
                    let _ = sender.send(StdoutEventV1::Failure(
                        "could not reserve bounded stdout line",
                    ));
                    return;
                }
                line.push(*byte);
                if line.len() > max_line {
                    let _ = sender.send(StdoutEventV1::Failure("stdout line exceeded bound"));
                    return;
                }
                if *byte == b'\n'
                    && sender
                        .send(StdoutEventV1::Line(std::mem::take(&mut line)))
                        .is_err()
                {
                    return;
                }
            }
        }
    });
    Ok(receiver)
}

fn spawn_stderr_reader_v1(
    mut error: ChildStderr,
    max_bytes: u64,
) -> Result<Receiver<StderrCaptureV1>, String> {
    let max = usize::try_from(max_bytes).map_err(|_| "stderr bound is too large")?;
    let capacity = max.checked_add(1).ok_or("stderr bound overflow")?;
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(capacity)
        .map_err(|_| "could not reserve bounded stderr")?;
    let (sender, receiver) = sync_channel(1);
    thread::spawn(move || {
        let mut overflow = false;
        let mut io_failed = false;
        let mut buffer = [0_u8; 8 * 1024];
        loop {
            match error.read(&mut buffer) {
                Ok(0) => break,
                Ok(read) => {
                    let retained = capacity.saturating_sub(bytes.len()).min(read);
                    bytes.extend_from_slice(&buffer[..retained]);
                    overflow |= retained < read || bytes.len() > max;
                }
                Err(_) => {
                    io_failed = true;
                    break;
                }
            }
        }
        let _ = sender.send(StderrCaptureV1 {
            bytes,
            overflow,
            io_failed,
        });
    });
    Ok(receiver)
}

struct BoundedVecWriterV1 {
    bytes: Vec<u8>,
    max_bytes: usize,
}

impl BoundedVecWriterV1 {
    fn new(max_bytes: u64) -> Result<Self, String> {
        let max_bytes = usize::try_from(max_bytes)
            .map_err(|_| "JSON byte bound does not fit this host".to_owned())?;
        Ok(Self {
            bytes: Vec::new(),
            max_bytes,
        })
    }

    fn into_inner(self) -> Vec<u8> {
        self.bytes
    }
}

impl Write for BoundedVecWriterV1 {
    fn write(&mut self, input: &[u8]) -> io::Result<usize> {
        let required = self
            .bytes
            .len()
            .checked_add(input.len())
            .ok_or_else(|| io::Error::other("JSON byte count overflowed"))?;
        if required > self.max_bytes {
            return Err(io::Error::other(format!(
                "JSON exceeds the compiled {}-byte bound",
                self.max_bytes
            )));
        }
        self.bytes
            .try_reserve_exact(input.len())
            .map_err(|error| io::Error::other(format!("JSON allocation failed: {error}")))?;
        self.bytes.extend_from_slice(input);
        Ok(input.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

fn encode_json_line<T: Serialize>(value: &T, max_bytes: u64) -> Result<Vec<u8>, String> {
    let mut writer = BoundedVecWriterV1::new(max_bytes)?;
    serde_json::to_writer(&mut writer, value).map_err(|error| error.to_string())?;
    writer.write_all(b"\n").map_err(|error| error.to_string())?;
    Ok(writer.into_inner())
}

fn require_eq(actual: &Value, expected: &str, field: &str) -> Result<(), String> {
    if actual == expected {
        Ok(())
    } else {
        Err(format!("unexpected {field}"))
    }
}

struct PinnedExecutableV1 {
    file: File,
    metadata: StableFileMetadataV1,
    identity: ExecutableContentIdentityV1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct StableFileMetadataV1 {
    device: u64,
    inode: u64,
    mode: u32,
    links: u64,
    bytes: u64,
    modified_seconds: i64,
    modified_nanoseconds: i64,
    changed_seconds: i64,
    changed_nanoseconds: i64,
}

impl StableFileMetadataV1 {
    fn from_metadata(metadata: &fs::Metadata) -> Self {
        Self {
            device: metadata.dev(),
            inode: metadata.ino(),
            mode: metadata.mode(),
            links: metadata.nlink(),
            bytes: metadata.len(),
            modified_seconds: metadata.mtime(),
            modified_nanoseconds: metadata.mtime_nsec(),
            changed_seconds: metadata.ctime(),
            changed_nanoseconds: metadata.ctime_nsec(),
        }
    }
}

impl PinnedExecutableV1 {
    fn open(path: &Path, label: &str) -> Result<Self, String> {
        let file = OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC | libc::O_NONBLOCK)
            .open(path)
            .map_err(|_| format!("could not securely open {label}"))?;
        let before = file
            .metadata()
            .map_err(|_| format!("could not inspect opened {label}"))?;
        if !before.file_type().is_file()
            || before.nlink() != 1
            || before.len() == 0
            || before.len() > MAX_EXECUTABLE_BYTES_V1
            || before.mode() & 0o111 == 0
        {
            return Err(format!("{label} is not a bounded executable regular file"));
        }
        let metadata = StableFileMetadataV1::from_metadata(&before);
        let identity = executable_content_identity(&file, metadata.bytes, label)?;
        let after = file
            .metadata()
            .map_err(|_| format!("could not revalidate opened {label}"))?;
        if metadata != StableFileMetadataV1::from_metadata(&after) {
            return Err(format!("{label} changed during admission"));
        }
        Ok(Self {
            file,
            metadata,
            identity,
        })
    }

    fn proc_path(&self) -> String {
        format!("/proc/self/fd/{}", self.file.as_raw_fd())
    }

    fn identity(&self) -> &ExecutableContentIdentityV1 {
        &self.identity
    }

    fn revalidate(&self, label: &str) -> Result<(), String> {
        let before = self
            .file
            .metadata()
            .map_err(|_| format!("could not inspect pinned {label}"))?;
        if self.metadata != StableFileMetadataV1::from_metadata(&before) {
            return Err(format!("pinned {label} metadata changed"));
        }
        let identity = executable_content_identity(&self.file, self.metadata.bytes, label)?;
        let after = self
            .file
            .metadata()
            .map_err(|_| format!("could not revalidate pinned {label}"))?;
        if self.metadata != StableFileMetadataV1::from_metadata(&after) || identity != self.identity
        {
            return Err(format!("pinned {label} content changed"));
        }
        Ok(())
    }
}

fn executable_content_identity(
    file: &File,
    bytes: u64,
    label: &str,
) -> Result<ExecutableContentIdentityV1, String> {
    let mut digest = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    let mut offset = 0_u64;
    while offset < bytes {
        let remaining = usize::try_from((bytes - offset).min(buffer.len() as u64))
            .map_err(|_| format!("{label} size conversion failed"))?;
        let read = file
            .read_at(&mut buffer[..remaining], offset)
            .map_err(|_| format!("could not hash pinned {label}"))?;
        if read == 0 {
            return Err(format!("pinned {label} was truncated"));
        }
        digest.update(&buffer[..read]);
        offset = offset
            .checked_add(read as u64)
            .ok_or_else(|| format!("{label} size overflow"))?;
    }
    let digest: [u8; 32] = digest.finalize().into();
    Ok(ExecutableContentIdentityV1 {
        scheme: "sha256_of_exact_executable_bytes",
        sha256: lower_hex(&digest),
        bytes,
    })
}

fn read_optional(path: &Option<PathBuf>, label: &str) -> Result<Option<Vec<u8>>, String> {
    path.as_ref()
        .map(|path| read_bounded(path, MAX_PROFILER_VARIANT_TREATMENT_BYTES_V1, label))
        .transpose()
}

fn read_bounded(path: impl AsRef<Path>, max: u64, label: &str) -> Result<Vec<u8>, String> {
    let mut file = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC | libc::O_NONBLOCK)
        .open(path.as_ref())
        .map_err(|_| format!("could not securely open {label}"))?;
    let before = file
        .metadata()
        .map_err(|_| format!("could not inspect opened {label}"))?;
    if !before.file_type().is_file()
        || before.nlink() != 1
        || before.len() == 0
        || before.len() > max
    {
        return Err(format!("{label} is not a bounded regular file"));
    }
    let capacity = usize::try_from(before.len()).map_err(|_| format!("{label} is too large"))?;
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(capacity)
        .map_err(|_| format!("could not reserve bounded {label}"))?;
    Read::by_ref(&mut file)
        .take(max.saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(|_| format!("could not read opened {label}"))?;
    let after = file
        .metadata()
        .map_err(|_| format!("could not revalidate opened {label}"))?;
    let stable = before.dev() == after.dev()
        && before.ino() == after.ino()
        && before.mode() == after.mode()
        && before.nlink() == after.nlink()
        && before.len() == after.len()
        && before.mtime() == after.mtime()
        && before.mtime_nsec() == after.mtime_nsec()
        && before.ctime() == after.ctime()
        && before.ctime_nsec() == after.ctime_nsec();
    if !stable || bytes.len() as u64 != before.len() || bytes.len() as u64 > max {
        return Err(format!("{label} changed during admission"));
    }
    Ok(bytes)
}

fn lower_hex(bytes: &[u8]) -> String {
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        use std::fmt::Write as _;
        write!(&mut output, "{byte:02x}").expect("writing to String cannot fail");
    }
    output
}

struct TempSnapshotV1(PathBuf);

impl TempSnapshotV1 {
    fn new(extension: &str, bytes: &[u8]) -> Result<Self, String> {
        for _ in 0..32 {
            let ordinal = NEXT_SNAPSHOT_V1.fetch_add(1, Ordering::Relaxed);
            let path = env::temp_dir().join(format!(
                "fe2o3-agent-reference-{}-{ordinal}.{extension}",
                std::process::id()
            ));
            let file = OpenOptions::new()
                .write(true)
                .create_new(true)
                .mode(0o600)
                .open(&path);
            let Ok(mut file) = file else { continue };
            file.write_all(bytes).map_err(|_| "snapshot write failed")?;
            file.sync_all().map_err(|_| "snapshot sync failed")?;
            return Ok(Self(path));
        }
        Err("could not create private input snapshot".into())
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TempSnapshotV1 {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}

#[cfg(test)]
mod tests {
    use std::os::unix::fs::{PermissionsExt, symlink};

    use super::*;

    fn executable_file(path: &Path, bytes: &[u8]) {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o700)
            .open(path)
            .unwrap();
        file.write_all(bytes).unwrap();
        file.sync_all().unwrap();
    }

    fn helper_shell(root: &Path) -> PinnedExecutableV1 {
        let shell = fs::canonicalize("/bin/sh").unwrap();
        let installed = root.join("test-producer");
        fs::copy(shell, &installed).unwrap();
        fs::set_permissions(&installed, fs::Permissions::from_mode(0o700)).unwrap();
        PinnedExecutableV1::open(&installed, "test producer").unwrap()
    }

    fn helper_session<'a>(
        executable: &'a PinnedExecutableV1,
        script: &str,
        timeout: Duration,
    ) -> BoundedChildV1<'a> {
        let mut command = Command::new(executable.proc_path());
        command.args(["-c", script]);
        BoundedChildV1::spawn_with_timeout(command, executable, "test producer", 32, 64, timeout)
            .unwrap()
    }

    fn wait_for_pid(path: &Path) -> u32 {
        let deadline = Instant::now() + Duration::from_secs(2);
        loop {
            if let Ok(bytes) = fs::read(path)
                && let Ok(text) = std::str::from_utf8(&bytes)
                && let Ok(pid) = text.trim().parse()
            {
                return pid;
            }
            assert!(
                Instant::now() < deadline,
                "test producer did not publish PID"
            );
            thread::sleep(Duration::from_millis(2));
        }
    }

    fn assert_pid_reaped(pid: u32) {
        let path = PathBuf::from(format!("/proc/{pid}"));
        let deadline = Instant::now() + Duration::from_secs(2);
        while path.exists() && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(2));
        }
        assert!(!path.exists(), "owned producer PID {pid} survived cleanup");
    }

    fn page_response(
        capture: &Value,
        dispatch: &str,
        next_cursor: Value,
        request_id: u64,
        revision: u64,
    ) -> Value {
        json!({
            "status":"ok",
            "schema":AGENT_PROFILER_RESPONSE_SCHEMA_V1,
            "request_id":request_id,
            "response_revision":revision,
            "value":{
                "result":"page",
                "page":{
                    "context":{
                        "bundle_identity":capture,
                        "run_identity":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                        "source_kind":"rocprofv3_json",
                        "device_count":1,
                        "dispatch_count":2,
                        "att_reference_count":0
                    },
                    "kind":"dispatches",
                    "returned":1,
                    "next_cursor":next_cursor,
                    "items":[{
                        "item":"dispatch",
                        "dispatch":{
                            "identity":dispatch,
                            "evidence":{
                                "origin":"observed",
                                "bundle":capture["digest"],
                                "record":dispatch
                            }
                        }
                    }]
                },
                "evidence":{
                    "origin":{"classification":"homogeneous","origin":"observed"},
                    "service_contract":{
                        "scheme":"domain_separated_sha256",
                        "format_version":1,
                        "digest":"bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
                        "canonical_len":1
                    },
                    "captures":[capture],
                    "records":[]
                }
            }
        })
    }

    #[test]
    fn executable_descriptor_retains_renamed_bytes_and_rejects_substitution() {
        let ordinal = NEXT_SNAPSHOT_V1.fetch_add(1, Ordering::Relaxed);
        let root = env::temp_dir().join(format!(
            "fe2o3-pinned-executable-test-{}-{ordinal}",
            std::process::id()
        ));
        fs::create_dir(&root).unwrap();
        let selected = root.join("selected");
        let retained_name = root.join("retained");
        let original = b"original executable bytes";
        let replacement = b"replacement executable bytes";
        executable_file(&selected, original);
        let pinned = PinnedExecutableV1::open(&selected, "test executable").unwrap();
        let original_digest: [u8; 32] = Sha256::digest(original).into();
        assert_eq!(pinned.identity().sha256, lower_hex(&original_digest));

        fs::rename(&selected, &retained_name).unwrap();
        executable_file(&selected, replacement);
        assert_eq!(fs::read(pinned.proc_path()).unwrap(), original);
        assert_ne!(fs::read(&selected).unwrap(), original);

        let mut permissions = fs::metadata(&retained_name).unwrap().permissions();
        permissions.set_mode(0o500);
        fs::set_permissions(&retained_name, permissions).unwrap();
        assert!(pinned.revalidate("test executable").is_err());

        let symlink_path = root.join("symlink");
        symlink(&selected, &symlink_path).unwrap();
        assert!(PinnedExecutableV1::open(&symlink_path, "symlink executable").is_err());
        let hardlink_path = root.join("hardlink");
        fs::hard_link(&selected, &hardlink_path).unwrap();
        assert!(PinnedExecutableV1::open(&selected, "hard-linked executable").is_err());

        drop(pinned);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn bounded_child_rejects_hostile_streams_and_reaps_owned_process_groups() {
        let ordinal = NEXT_SNAPSHOT_V1.fetch_add(1, Ordering::Relaxed);
        let root = env::temp_dir().join(format!(
            "fe2o3-bounded-child-test-{}-{ordinal}",
            std::process::id()
        ));
        fs::create_dir(&root).unwrap();
        let executable = helper_shell(&root);
        let request = json!({"request":"test"});

        let oversized_pid = root.join("oversized.pid");
        let script = format!(
            "printf '%s\\n' $$ > {}; IFS= read -r line; printf 012345678901234567890123456789012; while :; do :; done",
            oversized_pid.display()
        );
        let mut child = helper_session(&executable, &script, Duration::from_secs(2));
        let pid = wait_for_pid(&oversized_pid);
        assert!(child.exchange_line(&request, 1024).is_err());
        drop(child);
        assert_pid_reaped(pid);

        let stderr_pid = root.join("stderr.pid");
        let script = format!(
            "printf '%s\\n' $$ > {}; IFS= read -r line; printf 'ok\\n'; i=0; while [ $i -le 65536 ]; do printf x >&2; i=$((i+1)); done",
            stderr_pid.display()
        );
        let mut child = helper_session(&executable, &script, Duration::from_secs(5));
        let pid = wait_for_pid(&stderr_pid);
        assert_eq!(child.exchange_line(&request, 1024).unwrap(), b"ok\n");
        assert!(child.finish().is_err());
        assert_pid_reaped(pid);

        let trailing_pid = root.join("trailing.pid");
        let script = format!(
            "printf '%s\\n' $$ > {}; IFS= read -r line; printf 'ok\\ntrailing\\n'",
            trailing_pid.display()
        );
        let mut child = helper_session(&executable, &script, Duration::from_secs(2));
        let pid = wait_for_pid(&trailing_pid);
        assert_eq!(child.exchange_line(&request, 1024).unwrap(), b"ok\n");
        assert!(child.finish().is_err());
        assert_pid_reaped(pid);

        let trailing_bytes_pid = root.join("trailing-bytes.pid");
        let script = format!(
            "printf '%s\\n' $$ > {}; IFS= read -r line; printf 'ok\\ntrailing'",
            trailing_bytes_pid.display()
        );
        let mut child = helper_session(&executable, &script, Duration::from_secs(2));
        let pid = wait_for_pid(&trailing_bytes_pid);
        assert_eq!(child.exchange_line(&request, 1024).unwrap(), b"ok\n");
        assert!(child.finish().is_err());
        assert_pid_reaped(pid);

        let successful_leader_pid = root.join("successful-leader.pid");
        let detached_descendant_pid = root.join("detached-descendant.pid");
        let script = format!(
            "printf '%s\\n' $$ > {}; IFS= read -r line; sh -c 'exec >/dev/null 2>&1; while :; do :; done' & printf '%s\\n' $! > {}; printf 'ok\\n'",
            successful_leader_pid.display(),
            detached_descendant_pid.display()
        );
        let mut child = helper_session(&executable, &script, Duration::from_secs(2));
        let pid = wait_for_pid(&successful_leader_pid);
        assert_eq!(child.exchange_line(&request, 1024).unwrap(), b"ok\n");
        let descendant = wait_for_pid(&detached_descendant_pid);
        child.finish().unwrap();
        assert_pid_reaped(pid);
        assert_pid_reaped(descendant);

        let hung_pid = root.join("hung.pid");
        let script = format!(
            "printf '%s\\n' $$ > {}; IFS= read -r line; while :; do :; done",
            hung_pid.display()
        );
        let mut child = helper_session(&executable, &script, Duration::from_millis(100));
        let pid = wait_for_pid(&hung_pid);
        assert!(child.exchange_line(&request, 1024).is_err());
        drop(child);
        assert_pid_reaped(pid);

        let invalid_pid = root.join("invalid.pid");
        let descendant_pid = root.join("descendant.pid");
        let script = format!(
            "printf '%s\\n' $$ > {}; IFS= read -r line; sh -c 'while :; do :; done' & printf '%s\\n' $! > {}; printf 'not-json\\n'; while :; do :; done",
            invalid_pid.display(),
            descendant_pid.display()
        );
        let mut child = helper_session(&executable, &script, Duration::from_secs(2));
        let pid = wait_for_pid(&invalid_pid);
        let line = child.exchange_line(&request, 1024).unwrap();
        assert!(serde_json::from_slice::<Value>(&line).is_err());
        let descendant = wait_for_pid(&descendant_pid);
        drop(child);
        assert_pid_reaped(pid);
        assert_pid_reaped(descendant);

        drop(executable);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn agent_envelopes_and_dispatch_pages_fail_closed_on_substitution() {
        let capture = json!({
            "scheme":"domain_separated_sha256",
            "format_version":1,
            "digest":"1111111111111111111111111111111111111111111111111111111111111111",
            "canonical_len":64
        });
        let first_dispatch = "2222222222222222222222222222222222222222222222222222222222222222";
        let second_dispatch = "3333333333333333333333333333333333333333333333333333333333333333";
        let first_cursor = json!({
            "query_binding":"4444444444444444444444444444444444444444444444444444444444444444",
            "position":1
        });
        let first = page_response(&capture, first_dispatch, first_cursor.clone(), 3, 3);
        let mut line = serde_json::to_vec(&first).unwrap();
        line.push(b'\n');
        validate_agent_response_line_v1(&line, 3, 3, ExpectedAgentResultV1::Page).unwrap();
        let (selected, cursor) = validate_dispatch_page_v1(&first, &capture, None, true).unwrap();
        assert_eq!(cursor, first_cursor);

        for hostile in [
            ("request_id", json!(4)),
            ("response_revision", json!(2)),
            ("schema", json!("wrong-schema")),
            ("status", json!("error")),
        ] {
            let mut changed = first.clone();
            changed[hostile.0] = hostile.1;
            let mut bytes = serde_json::to_vec(&changed).unwrap();
            bytes.push(b'\n');
            assert!(
                validate_agent_response_line_v1(&bytes, 3, 3, ExpectedAgentResultV1::Page).is_err()
            );
        }
        let mut wrong_result = first.clone();
        wrong_result["value"]["result"] = json!("capture_plan");
        let mut bytes = serde_json::to_vec(&wrong_result).unwrap();
        bytes.push(b'\n');
        assert!(
            validate_agent_response_line_v1(&bytes, 3, 3, ExpectedAgentResultV1::Page).is_err()
        );

        let second = page_response(&capture, second_dispatch, Value::Null, 4, 4);
        validate_dispatch_page_v1(&second, &capture, Some(&selected), false).unwrap();
        let repeated = page_response(&capture, first_dispatch, Value::Null, 4, 4);
        assert!(validate_dispatch_page_v1(&repeated, &capture, Some(&selected), false).is_err());
        let repeated_cursor = page_response(&capture, second_dispatch, first_cursor, 4, 4);
        assert!(
            validate_dispatch_page_v1(&repeated_cursor, &capture, Some(&selected), false).is_err()
        );
        let wrong_capture = json!({
            "scheme":"domain_separated_sha256",
            "format_version":1,
            "digest":"5555555555555555555555555555555555555555555555555555555555555555",
            "canonical_len":64
        });
        assert!(validate_dispatch_page_v1(&first, &wrong_capture, None, true).is_err());
        let mut stale_cursor = first;
        stale_cursor["value"]["page"]["next_cursor"]["position"] = json!(2);
        assert!(validate_dispatch_page_v1(&stale_cursor, &capture, None, true).is_err());
    }
}
