#![deny(unsafe_code)]

use std::collections::BTreeSet;
use std::env;
use std::ffi::OsStr;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::os::fd::{AsRawFd, BorrowedFd};
use std::os::unix::fs::{FileExt, MetadataExt, OpenOptionsExt};
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{
    Child, ChildStderr, ChildStdin, ChildStdout, Command, ExitCode, ExitStatus, Stdio,
};
#[cfg(test)]
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{Receiver, SyncSender, TryRecvError, sync_channel};
#[cfg(test)]
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use fe2o3_debug_cli::reference_archive_v1::{
    MAX_REFERENCE_EVIDENCE_ARCHIVE_BYTES_V1, REFERENCE_EVIDENCE_ARCHIVE_REPORT_SCHEMA_V1,
    REFERENCE_EVIDENCE_ARCHIVE_SCHEMA_V1, ReferenceArchiveContentIdentityV1,
    ReferenceArchiveMemberIdentityV1, ReferenceEvidenceArchiveV1,
    decode_reference_evidence_archive_v1,
};
use fe2o3_debug_protocol::{
    CapabilityAvailabilityV1, DebugCapabilityNameV1, DebugOperationNameV1, DebugResponseV1,
    DebugResultV1, DiagnosisClassV2, DiagnosisFactV2, DiagnosisOperationV2, DiagnosisResponseV2,
    DiagnosisViewV2, PageCursorV1, ProtocolLimitsV1, SessionViewV1,
    decode_diagnosis_response_line_v2, decode_response_line_v1,
};
use fe2o3_kernel_ir::VerifiedCanonicalKernelIrV7;
use fe2o3_semantic_import::{
    CaptureIdentityV1, ContentIdentityRecordV1, ProfilerCoverageV4, TruthOriginV1,
};
use fe2o3_semantic_query::{
    AGENT_PROFILER_PLAN_REQUEST_SCHEMA_V1, AGENT_PROFILER_PLAN_SCHEMA_V1,
    AGENT_PROFILER_REQUEST_SCHEMA_V1, AGENT_PROFILER_RESPONSE_SCHEMA_V1,
    AGENT_PROFILER_VARIANT_COMPARISON_SCHEMA_V1, AGENT_PROFILER_VARIANT_REQUEST_SCHEMA_V1,
    AGENT_PROFILER_VARIANT_RESPONSE_SCHEMA_V1, MAX_AGENT_PROFILER_REQUEST_BYTES_V1,
    MAX_AGENT_PROFILER_RESPONSE_BYTES_V1, MAX_AGENT_PROFILER_VARIANT_REQUEST_BYTES_V1,
    MAX_AGENT_PROFILER_VARIANT_RESPONSE_BYTES_V1, MAX_PROFILER_VARIANT_RESULT_BYTES_V1,
    MAX_PROFILER_VARIANT_TREATMENT_BYTES_V1, ProfilerCursorV4, ProfilerDispatchSummaryV4,
    ProfilerListKindV4, ProfilerPageRequestV4, ProfilerPageV4, ProfilerQueryContextV4,
    ProfilerQueryItemV4, ProfilerQueryLimitsV4, ProfilerQueryRequestV4, ProfilerQueryResponseV4,
    ProfilerQuerySessionV4, ProfilerVariantComparisonV1, ProfilerVariantTreatmentInputV1,
    ProfilerVariantUnavailableKindV1, build_profiler_variant_request_v1,
    compare_profiler_variants_v1, decode_profiler_variant_comparison_v1,
    validate_agent_profiler_variant_response_line_v1,
};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
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
const REQUIRED_VARIANT_GAPS_V1: [ProfilerVariantUnavailableKindV1; 6] = [
    ProfilerVariantUnavailableKindV1::DecodedAttEvents,
    ProfilerVariantUnavailableKindV1::RuntimeApiEvents,
    ProfilerVariantUnavailableKindV1::CopyEvents,
    ProfilerVariantUnavailableKindV1::PcToSemanticOrIsaCorrelation,
    ProfilerVariantUnavailableKindV1::SemanticIrIsaChangeLocalization,
    ProfilerVariantUnavailableKindV1::CausalRegressionAttribution,
];

#[cfg(test)]
static NEXT_SNAPSHOT_V1: AtomicU64 = AtomicU64::new(1);

const REQUIRED_ARCHIVE_MEMFD_SEALS_V1: rustix::fs::SealFlags = rustix::fs::SealFlags::WRITE
    .union(rustix::fs::SealFlags::GROW)
    .union(rustix::fs::SealFlags::SHRINK)
    .union(rustix::fs::SealFlags::SEAL);

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
#[serde(untagged)]
enum ReferenceClientOutputV1 {
    Workflow(ReferenceReportV1),
    Archive(ArchiveReferenceReportV1),
}

#[derive(Serialize)]
struct ArchiveReferenceReportV1 {
    schema: &'static str,
    authority: &'static str,
    archive_schema: &'static str,
    archive: ReferenceArchiveContentIdentityV1,
    members: Vec<ReferenceArchiveMemberIdentityV1>,
    workflow: ReferenceReportV1,
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

fn run() -> Result<ReferenceClientOutputV1, String> {
    let arguments = env::args_os().skip(1).collect::<Vec<_>>();
    match arguments.as_slice() {
        [flag, workflow_path] if flag == "--workflow" => {
            let workflow_bytes = read_bounded(workflow_path, MAX_WORKFLOW_BYTES_V1, "workflow")?;
            let workflow: WorkflowV1 = serde_json::from_slice(&workflow_bytes)
                .map_err(|_| "invalid workflow JSON")?;
            if workflow.schema != WORKFLOW_SCHEMA_V1 {
                return Err("invalid workflow schema".into());
            }
            Ok(ReferenceClientOutputV1::Workflow(run_loaded(
                LoadedWorkflowV1::load(workflow)?,
            )?))
        }
        [archive_flag, archive_path, digest_flag, expected_digest, debugger_flag, debugger_path, profiler_flag, profiler_path]
            if archive_flag == "--archive"
                && digest_flag == "--archive-sha256"
                && debugger_flag == "--debugger"
                && profiler_flag == "--profiler-service" =>
        {
            let archive_bytes = read_archive_bounded(
                archive_path,
                MAX_REFERENCE_EVIDENCE_ARCHIVE_BYTES_V1,
                "reference evidence archive",
            )?;
            let expected_digest = decode_sha256(expected_digest)?;
            let archive = decode_reference_evidence_archive_v1(&archive_bytes, expected_digest)
                .map_err(|error| error.to_string())?;
            drop(archive_bytes);
            let identity = archive.identity.clone();
            let members = archive.members.clone();
            let workflow = run_loaded(LoadedWorkflowV1::load_archive(
                debugger_path,
                profiler_path,
                archive,
            )?)?;
            Ok(ReferenceClientOutputV1::Archive(ArchiveReferenceReportV1 {
                schema: REFERENCE_EVIDENCE_ARCHIVE_REPORT_SCHEMA_V1,
                authority: "read_only_no_execution_attach_scheduling_or_collection_authority",
                archive_schema: REFERENCE_EVIDENCE_ARCHIVE_SCHEMA_V1,
                archive: identity,
                members,
                workflow,
            }))
        }
        _ => Err("expected --workflow WORKFLOW.json or --archive ARCHIVE --archive-sha256 SHA256 --debugger FILE --profiler-service FILE".into()),
    }
}

fn run_loaded(loaded: LoadedWorkflowV1) -> Result<ReferenceReportV1, String> {
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

    fn load_archive(
        debugger_path: &OsStr,
        profiler_service_path: &OsStr,
        archive: ReferenceEvidenceArchiveV1,
    ) -> Result<Self, String> {
        Ok(Self {
            debugger_executable: PinnedExecutableV1::open_archive(
                Path::new(debugger_path),
                "debugger executable",
            )?,
            profiler_service_executable: PinnedExecutableV1::open_archive(
                Path::new(profiler_service_path),
                "profiler service executable",
            )?,
            out_of_bounds: LoadedSimulatorCaseV1 {
                kernel: archive.out_of_bounds.kernel,
                request: archive.out_of_bounds.request,
            },
            barrier_divergence: LoadedSimulatorCaseV1 {
                kernel: archive.barrier_divergence.kernel,
                request: archive.barrier_divergence.request,
            },
            baseline: LoadedTreatmentV1::from_archive(archive.baseline),
            candidate: LoadedTreatmentV1::from_archive(archive.candidate),
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
        Self::load_with_budget(value, MAX_PROFILER_VARIANT_TREATMENT_BYTES_V1)
    }

    fn load_with_budget(value: TreatmentFilesV1, mut remaining: u64) -> Result<Self, String> {
        Ok(Self {
            manifest: read_budgeted(&value.manifest, &mut remaining, "manifest")?,
            semantic_workload: read_budgeted(
                &value.semantic_workload,
                &mut remaining,
                "semantic workload",
            )?,
            raw_profiler_source: read_budgeted(
                &value.raw_profiler_source,
                &mut remaining,
                "raw profiler source",
            )?,
            bundle: read_budgeted(&value.bundle, &mut remaining, "profiler bundle")?,
            schedule: read_budgeted(&value.schedule, &mut remaining, "schedule")?,
            artifact: read_budgeted(&value.artifact, &mut remaining, "artifact")?,
            isa_projection: read_optional_budgeted(
                &value.isa_projection,
                &mut remaining,
                "ISA projection",
            )?,
            counters: read_optional_budgeted(&value.counters, &mut remaining, "counter capture")?,
            pc_samples: read_optional_budgeted(
                &value.pc_samples,
                &mut remaining,
                "PC sample capture",
            )?,
        })
    }

    fn input(&self) -> ProfilerVariantTreatmentInputV1<'_> {
        ProfilerVariantTreatmentInputV1 {
            manifest: &self.manifest,
            semantic_workload: &self.semantic_workload,
            raw_profiler_source: &self.raw_profiler_source,
            bundle: &self.bundle,
            schedule: &self.schedule,
            artifact: &self.artifact,
            isa_projection: self.isa_projection.as_deref(),
            counters: self.counters.as_deref(),
            pc_samples: self.pc_samples.as_deref(),
        }
    }

    fn from_archive(value: fe2o3_debug_cli::reference_archive_v1::ReferenceTreatmentV1) -> Self {
        Self {
            manifest: value.manifest,
            semantic_workload: value.semantic_workload,
            raw_profiler_source: value.raw_profiler_source,
            bundle: value.bundle,
            schedule: value.schedule,
            artifact: value.artifact,
            isa_projection: value.isa_projection,
            counters: value.counters,
            pc_samples: value.pc_samples,
        }
    }
}

fn decode_sha256(value: &OsStr) -> Result<[u8; 32], String> {
    let value = value
        .to_str()
        .ok_or("archive SHA-256 is not canonical UTF-8 hexadecimal")?;
    if value.len() != 64
        || value
            .as_bytes()
            .iter()
            .any(|byte| !matches!(byte, b'0'..=b'9' | b'a'..=b'f'))
    {
        return Err("archive SHA-256 is not canonical lowercase hexadecimal".into());
    }
    let mut decoded = [0_u8; 32];
    for (destination, pair) in decoded.iter_mut().zip(value.as_bytes().chunks_exact(2)) {
        *destination = (hex_nibble(pair[0]) << 4) | hex_nibble(pair[1]);
    }
    Ok(decoded)
}

fn hex_nibble(value: u8) -> u8 {
    match value {
        b'0'..=b'9' => value - b'0',
        b'a'..=b'f' => value - b'a' + 10,
        _ => unreachable!("decode_sha256 validated each nibble"),
    }
}

fn diagnose_simulator(
    executable: &PinnedExecutableV1,
    case: &LoadedSimulatorCaseV1,
    expected_class: DiagnosisClassV2,
    class_wire: &'static str,
) -> Result<SimulatorDiagnosisReportV1, String> {
    let kernel = SealedInputV1::new("canonical KIR V7", &case.kernel)?;
    let request = SealedInputV1::new("simulation request", &case.request)?;
    let limits = ProtocolLimitsV1::default();
    let max_total = limits
        .max_response_line_bytes
        .checked_mul(3)
        .ok_or("debugger response bound overflow")?;
    let mut command = Command::new(executable.proc_path());
    command
        .args(["sim", "--kir-v7-fd"])
        .arg(kernel.file.as_raw_fd().to_string())
        .arg("--request-fd")
        .arg(request.file.as_raw_fd().to_string())
        .args(["--protocol", "jsonl"]);
    inherit_sealed_inputs_v1(&mut command, &kernel, &request)?;
    let mut child = BoundedChildV1::spawn(
        command,
        executable,
        "debugger",
        limits.max_response_line_bytes,
        max_total,
    )?;
    kernel.revalidate("canonical KIR V7")?;
    request.revalidate("simulation request")?;
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
        result: continued_result,
        ..
    } = continued
    else {
        return Err("debugger control response association mismatch".into());
    };
    if !matches!(continued_result.as_ref(), DebugResultV1::Control { .. }) {
        return Err("debugger control result kind mismatch".into());
    }
    let diagnosis = decode_diagnosis_response_line_v2(&lines[2], limits)
        .map_err(|_| "invalid or unauthenticated diagnosis response")?;
    let DiagnosisResponseV2::Ok {
        request_id: 3,
        operation: DiagnosisOperationV2::Diagnose,
        session,
        completeness,
        diagnoses,
        next_cursor,
        ..
    } = diagnosis
    else {
        return Err("diagnosis request failed".into());
    };
    validate_simulator_session_chain_v1(
        capability_session,
        continued_session,
        session,
        next_cursor,
    )?;
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

fn validate_simulator_session_chain_v1(
    capability: SessionViewV1,
    continued: SessionViewV1,
    diagnosis: SessionViewV1,
    next_cursor: Option<PageCursorV1>,
) -> Result<(), String> {
    if capability.revision != 0
        || continued.revision != 1
        || capability.configuration_identity != continued.configuration_identity
        || diagnosis != continued
        || next_cursor.is_some()
    {
        return Err("simulator response session/configuration/cursor association mismatch".into());
    }
    Ok(())
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

#[derive(Debug, Deserialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
struct AgentCapabilitiesWireV1 {
    result: String,
    capabilities: Vec<Value>,
    limits: AgentLimitsWireV1,
    evidence: AgentEvidenceWireV1,
}

#[derive(Debug, Deserialize, Eq, PartialEq)]
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

#[derive(Debug, Deserialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
struct AgentCaptureOpenedWireV1 {
    result: String,
    context: ProfilerQueryContextV4,
    coverage: ProfilerCoverageV4,
    capture_capabilities: Vec<Value>,
    audit: AgentOpenAuditWireV1,
    evidence: AgentEvidenceWireV1,
}

#[derive(Debug, Deserialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
struct AgentOpenAuditWireV1 {
    before_open_captures: u8,
    after_open_captures: u8,
    effect: AgentOpenEffectWireV1,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
enum AgentOpenEffectWireV1 {
    Registered,
    AlreadyOpen,
}

#[derive(Debug, Deserialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
struct AgentPageResultWireV1 {
    result: String,
    page: AgentPageWireV1,
    evidence: AgentEvidenceWireV1,
}

#[derive(Debug, Deserialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
struct AgentCapturePlanWireV1 {
    result: String,
    plan: Value,
    evidence: AgentEvidenceWireV1,
}

#[derive(Debug, Deserialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
struct AgentPageWireV1 {
    context: ProfilerQueryContextV4,
    kind: ProfilerListKindV4,
    returned: u16,
    next_cursor: Option<ProfilerCursorV4>,
    items: Vec<AgentDispatchItemWireV1>,
}

#[derive(Debug, Deserialize, Eq, PartialEq)]
#[serde(tag = "item", rename_all = "snake_case", deny_unknown_fields)]
enum AgentDispatchItemWireV1 {
    Dispatch { dispatch: ProfilerDispatchSummaryV4 },
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "classification", rename_all = "snake_case", deny_unknown_fields)]
enum AgentAggregateOriginWireV1 {
    Homogeneous { origin: TruthOriginV1 },
    Mixed { origins: Vec<TruthOriginV1> },
    Empty,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct AgentEvidenceWireV1 {
    origin: AgentAggregateOriginWireV1,
    service_contract: ContentIdentityRecordV1,
    captures: Vec<ContentIdentityRecordV1>,
    records: Vec<CaptureIdentityV1>,
}

enum ValidatedAgentResponseV1 {
    Capabilities(AgentCapabilitiesWireV1),
    CaptureOpened(AgentCaptureOpenedWireV1),
    Page(AgentPageResultWireV1),
    CapturePlan(AgentCapturePlanWireV1),
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct VariantOkWireV1<T> {
    status: String,
    schema: String,
    request_id: u64,
    response_revision: u64,
    value: T,
    response_identity: ContentIdentityRecordV1,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct VariantCapabilitiesValueWireV1 {
    result: String,
    capabilities: Value,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct VariantComparisonValueWireV1 {
    result: String,
    comparison: ProfilerVariantComparisonV1,
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

fn validate_agent_page_wire_v1(page: &AgentPageWireV1) -> Result<(), String> {
    if usize::from(page.returned) != page.items.len() {
        return Err("Agent V1 page shape is inconsistent".into());
    }
    Ok(())
}

fn validate_agent_response_line_v1(
    line: &[u8],
    request_id: u64,
    response_revision: u64,
    expected: ExpectedAgentResultV1,
) -> Result<ValidatedAgentResponseV1, String> {
    match expected {
        ExpectedAgentResultV1::Capabilities => {
            let response: AgentOkWireV1<AgentCapabilitiesWireV1> =
                serde_json::from_slice(line).map_err(|_| "invalid Agent V1 capabilities wire")?;
            validate_agent_ok_envelope(&response, request_id, response_revision)?;
            if response.value.result != expected.wire() {
                return Err("Agent V1 capabilities result kind mismatch".into());
            }
            Ok(ValidatedAgentResponseV1::Capabilities(response.value))
        }
        ExpectedAgentResultV1::CaptureOpened => {
            let response: AgentOkWireV1<AgentCaptureOpenedWireV1> =
                serde_json::from_slice(line).map_err(|_| "invalid Agent V1 open wire")?;
            validate_agent_ok_envelope(&response, request_id, response_revision)?;
            if response.value.result != expected.wire() {
                return Err("Agent V1 open result kind mismatch".into());
            }
            Ok(ValidatedAgentResponseV1::CaptureOpened(response.value))
        }
        ExpectedAgentResultV1::Page => {
            let response: AgentOkWireV1<AgentPageResultWireV1> =
                serde_json::from_slice(line).map_err(|_| "invalid Agent V1 page wire")?;
            validate_agent_ok_envelope(&response, request_id, response_revision)?;
            if response.value.result != expected.wire() {
                return Err("Agent V1 page result kind mismatch".into());
            }
            validate_agent_page_wire_v1(&response.value.page)?;
            Ok(ValidatedAgentResponseV1::Page(response.value))
        }
        ExpectedAgentResultV1::CapturePlan => {
            let response: AgentOkWireV1<AgentCapturePlanWireV1> =
                serde_json::from_slice(line).map_err(|_| "invalid Agent V1 plan wire")?;
            validate_agent_ok_envelope(&response, request_id, response_revision)?;
            if response.value.result != expected.wire() {
                return Err("Agent V1 plan result kind mismatch".into());
            }
            Ok(ValidatedAgentResponseV1::CapturePlan(response.value))
        }
    }
}

fn exchange_agent_v1(
    session: &mut JsonlChildV1<'_>,
    request: &Value,
    request_id: u64,
    response_revision: u64,
    expected: ExpectedAgentResultV1,
) -> Result<ValidatedAgentResponseV1, String> {
    if request["schema"] != AGENT_PROFILER_REQUEST_SCHEMA_V1 || request["request_id"] != request_id
    {
        return Err("issued Agent V1 request association is invalid".into());
    }
    let line = session.exchange_line(request, MAX_AGENT_PROFILER_REQUEST_BYTES_V1)?;
    validate_agent_response_line_v1(&line, request_id, response_revision, expected)
}

fn exchange_encoded_agent_v1(
    session: &mut JsonlChildV1<'_>,
    request: Vec<u8>,
    request_id: u64,
    response_revision: u64,
    expected: ExpectedAgentResultV1,
) -> Result<ValidatedAgentResponseV1, String> {
    let line = session.exchange_encoded_line(request, MAX_AGENT_PROFILER_REQUEST_BYTES_V1)?;
    validate_agent_response_line_v1(&line, request_id, response_revision, expected)
}

fn validate_variant_response_v1<T: DeserializeOwned>(
    line: &[u8],
    request_id: u64,
    response_revision: u64,
) -> Result<VariantOkWireV1<T>, String> {
    validate_agent_profiler_variant_response_line_v1(line)
        .map_err(|_| "profiler response identity mismatch")?;
    let response: VariantOkWireV1<T> =
        serde_json::from_slice(line).map_err(|_| "invalid Variant V1 response wire")?;
    if response.status != "ok"
        || response.schema != AGENT_PROFILER_VARIANT_RESPONSE_SCHEMA_V1
        || response.request_id != request_id
        || response.response_revision != response_revision
    {
        return Err("Variant V1 response association mismatch".into());
    }
    Ok(response)
}

fn exchange_variant_v1<T: DeserializeOwned>(
    session: &mut JsonlChildV1<'_>,
    request: &Value,
    request_id: u64,
    response_revision: u64,
) -> Result<VariantOkWireV1<T>, String> {
    if request["schema"] != AGENT_PROFILER_VARIANT_REQUEST_SCHEMA_V1
        || request["request_id"] != request_id
        || request["expected_revision"] != response_revision - 1
    {
        return Err("issued Variant V1 request association is invalid".into());
    }
    let line = session.exchange_line(request, MAX_AGENT_PROFILER_VARIANT_REQUEST_BYTES_V1)?;
    validate_variant_response_v1(&line, request_id, response_revision)
}

fn exchange_encoded_variant_v1<T: DeserializeOwned>(
    session: &mut JsonlChildV1<'_>,
    request: Vec<u8>,
    request_id: u64,
    response_revision: u64,
) -> Result<VariantOkWireV1<T>, String> {
    let line =
        session.exchange_encoded_line(request, MAX_AGENT_PROFILER_VARIANT_REQUEST_BYTES_V1)?;
    validate_variant_response_v1(&line, request_id, response_revision)
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
    let capabilities: VariantOkWireV1<VariantCapabilitiesValueWireV1> =
        exchange_variant_v1(&mut session, &capabilities_request, 1, 1)?;
    if capabilities.value.result != "capabilities" {
        return Err("Variant capability result kind mismatch".into());
    }
    let variant_capabilities = &capabilities.value.capabilities;
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
    let comparison_request = build_profiler_variant_request_v1(
        &baseline.semantic_workload,
        &baseline.manifest,
        &candidate.manifest,
    )
    .map_err(|_| "could not build retained Variant request")?;
    let expected =
        compare_profiler_variants_v1(comparison_request, baseline.input(), candidate.input())
            .map_err(|_| "retained Variant inputs failed production comparison")?;
    let encoded_request = encode_variant_comparison_request_v1(baseline, candidate)?;
    let compared: VariantOkWireV1<VariantComparisonValueWireV1> =
        exchange_encoded_variant_v1(&mut session, encoded_request, 2, 2)?;
    session.finish()?;
    if compared.value.result != "comparison" {
        return Err("Variant comparison result kind mismatch".into());
    }
    let comparison = compared.value.comparison;
    let mut canonical = encode_json_line(
        &comparison,
        MAX_PROFILER_VARIANT_RESULT_BYTES_V1.saturating_add(1),
    )?;
    if canonical.pop() != Some(b'\n') {
        return Err("typed Variant comparison encoding is not canonical JSONL".into());
    }
    let decoded = decode_profiler_variant_comparison_v1(
        &canonical,
        comparison_request,
        baseline.input(),
        candidate.input(),
    )
    .map_err(|_| "Variant comparison does not match the retained exact inputs")?;
    if decoded != expected || comparison != expected {
        return Err("Variant comparison differs from independent production output".into());
    }
    if comparison.ranked_explanations.is_empty()
        || comparison
            .ranked_explanations
            .iter()
            .any(|entry| entry.evidence.is_empty())
    {
        return Err("ranked explanation lacks exact evidence".into());
    }
    let kinds = comparison
        .unavailable
        .iter()
        .map(|entry| entry.kind)
        .collect::<BTreeSet<_>>();
    if REQUIRED_VARIANT_GAPS_V1
        .iter()
        .any(|kind| !kinds.contains(kind))
    {
        return Err("Variant truth-boundary gaps are incomplete or uncited".into());
    }
    Ok(VariantReportV1 {
        claim_truth: "conservative_co_observation_not_causal_attribution",
        request_identity: serde_json::to_value(comparison.request_identity)
            .map_err(|_| "encode Variant request identity")?,
        baseline_manifest: serde_json::to_value(comparison.baseline_treatment.manifest)
            .map_err(|_| "encode baseline manifest identity")?,
        candidate_manifest: serde_json::to_value(comparison.candidate_treatment.manifest)
            .map_err(|_| "encode candidate manifest identity")?,
        ranked_explanations: serde_json::to_value(&comparison.ranked_explanations)
            .map_err(|_| "encode Variant explanations")?,
        unavailable: serde_json::to_value(&comparison.unavailable)
            .map_err(|_| "encode Variant unavailables")?,
        response_identity: serde_json::to_value(compared.response_identity)
            .map_err(|_| "encode Variant response identity")?,
    })
}

fn validate_dispatch_page_v1(
    response: &AgentPageResultWireV1,
    expected: &ProfilerPageV4,
    expected_evidence: &AgentEvidenceWireV1,
) -> Result<CaptureIdentityV1, String> {
    if response.result != "page"
        || response.page.context != expected.context
        || response.page.kind != expected.kind
        || response.page.returned != expected.returned
        || response.page.next_cursor != expected.next_cursor
        || response.evidence != *expected_evidence
        || response.page.items.len() != expected.items.len()
    {
        return Err("Agent V1 dispatch page differs from the independent bundle query".into());
    }
    let [wire_item] = response.page.items.as_slice() else {
        return Err("Agent V1 dispatch page did not contain exactly one item".into());
    };
    let [expected_item] = expected.items.as_slice() else {
        return Err("independent dispatch page did not contain exactly one item".into());
    };
    let (
        AgentDispatchItemWireV1::Dispatch { dispatch },
        ProfilerQueryItemV4::Dispatch {
            dispatch: expected_dispatch,
        },
    ) = (wire_item, expected_item)
    else {
        return Err("Agent V1 page item kind differs from the independent query".into());
    };
    if dispatch != expected_dispatch {
        return Err("Agent V1 dispatch record differs from the independent bundle query".into());
    }
    Ok(dispatch.identity)
}

fn plan_next_capture(
    executable: &PinnedExecutableV1,
    bundle: &[u8],
) -> Result<CapturePlanReportV1, String> {
    let independent = ProfilerQuerySessionV4::open(bundle, ProfilerQueryLimitsV4::default())
        .map_err(|_| "retained bundle failed independent production admission")?;
    let (expected_context, expected_coverage) = match independent
        .query(ProfilerQueryRequestV4::Open)
        .map_err(|_| "independent bundle open failed")?
    {
        ProfilerQueryResponseV4::Open { context, coverage } => (context, coverage),
        _ => return Err("independent bundle open returned the wrong result".into()),
    };
    let expected_capture_capabilities = match independent
        .query(ProfilerQueryRequestV4::Capabilities)
        .map_err(|_| "independent bundle capabilities failed")?
    {
        ProfilerQueryResponseV4::Capabilities { capabilities, .. } => capabilities,
        _ => return Err("independent bundle capabilities returned the wrong result".into()),
    };
    if expected_context.dispatch_count != 2 {
        return Err("seeded capture no longer contains exactly two dispatches".into());
    }
    let expected_first = match independent
        .query(ProfilerQueryRequestV4::List {
            kind: ProfilerListKindV4::Dispatches,
            page: ProfilerPageRequestV4 {
                limit: 1,
                cursor: None,
            },
        })
        .map_err(|_| "independent first dispatch page failed")?
    {
        ProfilerQueryResponseV4::Page { page } => page,
        _ => return Err("independent first dispatch page returned the wrong result".into()),
    };
    let expected_cursor = expected_first
        .next_cursor
        .ok_or("independent first dispatch page did not advance")?;
    let expected_second = match independent
        .query(ProfilerQueryRequestV4::List {
            kind: ProfilerListKindV4::Dispatches,
            page: ProfilerPageRequestV4 {
                limit: 1,
                cursor: Some(expected_cursor),
            },
        })
        .map_err(|_| "independent second dispatch page failed")?
    {
        ProfilerQueryResponseV4::Page { page } => page,
        _ => return Err("independent second dispatch page returned the wrong result".into()),
    };
    if expected_cursor.position != 1 || expected_second.next_cursor.is_some() {
        return Err("independent dispatch cursor contract changed".into());
    }

    let mut session =
        JsonlChildV1::spawn(executable, "jsonl", MAX_AGENT_PROFILER_RESPONSE_BYTES_V1, 5)?;
    let capabilities_request = json!({
        "operation":"discover_capabilities","schema":AGENT_PROFILER_REQUEST_SCHEMA_V1,"request_id":1
    });
    let ValidatedAgentResponseV1::Capabilities(capabilities) = exchange_agent_v1(
        &mut session,
        &capabilities_request,
        1,
        1,
        ExpectedAgentResultV1::Capabilities,
    )?
    else {
        return Err("Agent V1 capabilities response kind changed".into());
    };
    if capabilities.evidence.origin
        != (AgentAggregateOriginWireV1::Homogeneous {
            origin: TruthOriginV1::Declared,
        })
        || !capabilities.evidence.captures.is_empty()
        || !capabilities.evidence.records.is_empty()
    {
        return Err("Agent V1 capability evidence is not exact".into());
    }
    let service_contract = capabilities.evidence.service_contract;
    let capability = capabilities
        .capabilities
        .iter()
        .find(|entry| entry["operation"] == "plan_next_capture")
        .ok_or("capture planning capability absent")?;
    let limits = &capabilities.limits;
    if limits.max_requests < 5
        || limits.max_page_items < 1
        || limits.max_bundle_bytes < bundle.len() as u64
    {
        return Err("Agent V1 discovered bounds cannot serve this workflow".into());
    }
    let _ = (
        limits.max_request_bytes,
        limits.max_response_bytes,
        limits.max_open_captures,
        limits.max_plan_missing_facts,
        limits.max_plan_compute_units,
        limits.max_plan_storage_bytes,
        limits.max_plan_records,
        limits.max_plan_overhead_basis_points,
    );
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
    let open_request = encode_open_capture_request_v1(bundle)?;
    let ValidatedAgentResponseV1::CaptureOpened(opened) = exchange_encoded_agent_v1(
        &mut session,
        open_request,
        2,
        2,
        ExpectedAgentResultV1::CaptureOpened,
    )?
    else {
        return Err("Agent V1 open response kind changed".into());
    };
    let capture = expected_context.bundle_identity;
    let expected_capture_capabilities = serde_json::to_value(expected_capture_capabilities)
        .map_err(|_| "encode independent capture capabilities")?;
    if opened.context != expected_context
        || opened.coverage != expected_coverage
        || Value::Array(opened.capture_capabilities.clone()) != expected_capture_capabilities
        || opened.audit
            != (AgentOpenAuditWireV1 {
                before_open_captures: 0,
                after_open_captures: 1,
                effect: AgentOpenEffectWireV1::Registered,
            })
        || opened.evidence
            != (AgentEvidenceWireV1 {
                origin: AgentAggregateOriginWireV1::Homogeneous {
                    origin: TruthOriginV1::Observed,
                },
                service_contract,
                captures: vec![capture],
                records: Vec::new(),
            })
    {
        return Err("opened capture differs from independent admission".into());
    }
    let first_request = json!({
        "operation":"list_dispatches","schema":AGENT_PROFILER_REQUEST_SCHEMA_V1,"request_id":3,"capture":capture,"page":{"limit":1,"cursor":null}
    });
    let ValidatedAgentResponseV1::Page(first) = exchange_agent_v1(
        &mut session,
        &first_request,
        3,
        3,
        ExpectedAgentResultV1::Page,
    )?
    else {
        return Err("Agent V1 first page response kind changed".into());
    };
    let page_evidence = AgentEvidenceWireV1 {
        origin: AgentAggregateOriginWireV1::Homogeneous {
            origin: TruthOriginV1::Observed,
        },
        service_contract,
        captures: vec![capture],
        records: Vec::new(),
    };
    let dispatch = validate_dispatch_page_v1(&first, &expected_first, &page_evidence)?;
    let second_request = json!({
        "operation":"list_dispatches","schema":AGENT_PROFILER_REQUEST_SCHEMA_V1,"request_id":4,"capture":capture,"page":{"limit":1,"cursor":expected_cursor}
    });
    let ValidatedAgentResponseV1::Page(second) = exchange_agent_v1(
        &mut session,
        &second_request,
        4,
        4,
        ExpectedAgentResultV1::Page,
    )?
    else {
        return Err("Agent V1 second page response kind changed".into());
    };
    let second_dispatch = validate_dispatch_page_v1(&second, &expected_second, &page_evidence)?;
    if second_dispatch == dispatch {
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
    let ValidatedAgentResponseV1::CapturePlan(planned) = exchange_agent_v1(
        &mut session,
        &plan_request,
        5,
        5,
        ExpectedAgentResultV1::CapturePlan,
    )?
    else {
        return Err("Agent V1 plan response kind changed".into());
    };
    session.finish()?;
    let plan = &planned.plan;
    let evidence = &planned.evidence;
    if plan["provenance"].as_array().is_none_or(Vec::is_empty)
        || plan["selected_missing_evidence"]
            .as_array()
            .is_none_or(Vec::is_empty)
    {
        return Err("capture plan lacks provenance or selected evidence".into());
    }
    if plan["selected_missing_evidence"] != json!(["hardware_counter_measurements"])
        || plan["minimum_additional_captures"] != 1
        || evidence
            != &(AgentEvidenceWireV1 {
                origin: AgentAggregateOriginWireV1::Mixed {
                    origins: vec![
                        TruthOriginV1::Declared,
                        TruthOriginV1::Observed,
                        TruthOriginV1::Inferred,
                    ],
                },
                service_contract,
                captures: vec![capture],
                records: vec![dispatch],
            })
    {
        return Err("minimum capture plan is not covered by exact evidence".into());
    }
    let provenance = plan["provenance"]
        .as_array()
        .ok_or("capture plan provenance is not an array")?;
    let capture_value = serde_json::to_value(capture).map_err(|_| "encode capture identity")?;
    let dispatch_value = serde_json::to_value(dispatch).map_err(|_| "encode dispatch identity")?;
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
    ) || !has_subject("capture_bundle", "observed", "content", &capture_value)
        || !has_subject("dispatch_record", "observed", "identity", &dispatch_value)
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
        evidence: serde_json::to_value(evidence).map_err(|_| "encode capture plan evidence")?,
        first_page_returned: json!(first.page.returned),
        second_page_returned: json!(second.page.returned),
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

    fn exchange_encoded_line(
        &mut self,
        request: Vec<u8>,
        max_request: u64,
    ) -> Result<Vec<u8>, String> {
        self.inner.exchange_encoded_line(request, max_request)
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
    group_signal_authority: ProcessGroupSignalAuthorityV1,
    armed: bool,
    #[cfg(test)]
    lifecycle_test: Option<LifecycleTestControlV1>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ProcessGroupSignalAuthorityV1 {
    Authorized,
    Revoked,
}

#[cfg(test)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum LifecycleEventV1 {
    ObservedLeaderExit,
    SignaledProcessGroup,
    ReapedDirectChild,
    VerifiedProcessGroupAbsent,
}

#[cfg(test)]
struct LifecycleTestControlV1 {
    events: Arc<Mutex<Vec<LifecycleEventV1>>>,
    fail_signal_after_delivery: bool,
    fail_absence_after_reap: bool,
}

impl OwnedChildGuardV1 {
    fn new(child: Child) -> Self {
        let process_group = child.id();
        Self {
            child,
            process_group,
            group_signal_authority: ProcessGroupSignalAuthorityV1::Authorized,
            armed: true,
            #[cfg(test)]
            lifecycle_test: None,
        }
    }

    #[cfg(test)]
    fn set_lifecycle_test_control(&mut self, control: LifecycleTestControlV1) {
        self.lifecycle_test = Some(control);
    }

    #[cfg(test)]
    fn record_lifecycle(&self, event: LifecycleEventV1) {
        if let Some(control) = &self.lifecycle_test {
            control.events.lock().unwrap().push(event);
        }
    }

    #[allow(unsafe_code)]
    fn observe_leader_exit_without_reaping(&self) -> Result<bool, String> {
        let pid =
            libc::id_t::try_from(self.process_group).map_err(|_| "owned child PID is invalid")?;
        let mut information = std::mem::MaybeUninit::<libc::siginfo_t>::zeroed();
        // SAFETY: waitid writes one siginfo_t to the initialized storage. P_PID
        // targets the exact still-owned child PID, WNOHANG is nonblocking, and
        // WNOWAIT deliberately retains the zombie until the ordered reap.
        let result = unsafe {
            libc::waitid(
                libc::P_PID,
                pid,
                information.as_mut_ptr(),
                libc::WEXITED | libc::WNOHANG | libc::WNOWAIT,
            )
        };
        if result != 0 {
            return Err("owned child non-reaping observation failed".into());
        }
        // SAFETY: successful waitid initialized the siginfo_t output.
        let information = unsafe { information.assume_init() };
        let exited = unsafe { information.si_pid() } != 0;
        if exited {
            #[cfg(test)]
            self.record_lifecycle(LifecycleEventV1::ObservedLeaderExit);
        }
        Ok(exited)
    }

    fn terminate_and_reap(&mut self) -> Result<ExitStatus, String> {
        let termination = self.signal_owned_process_group();
        if termination.is_err() {
            let _ = self.child.kill();
        }
        let reaped = self.reap_direct_child();
        let absent = self.wait_for_process_group_absence();
        combine_lifecycle_results(termination, reaped, absent)
    }

    fn finalize_observed_leader_exit(&mut self) -> Result<ExitStatus, String> {
        let termination = self.signal_owned_process_group();
        let reaped = self.reap_direct_child();
        let absent = self.wait_for_process_group_absence();
        combine_lifecycle_results(termination, reaped, absent)
    }

    fn reap_direct_child(&mut self) -> Result<ExitStatus, String> {
        let deadline = Instant::now() + CHILD_REAP_TIMEOUT_V1;
        loop {
            match self.child.try_wait() {
                Ok(Some(status)) => {
                    #[cfg(test)]
                    self.record_lifecycle(LifecycleEventV1::ReapedDirectChild);
                    return Ok(status);
                }
                Ok(None) => {}
                Err(_) => return Err("owned child wait failed".into()),
            }
            if Instant::now() >= deadline {
                return Err("owned child bounded reap timed out".into());
            }
            thread::sleep(CHILD_POLL_INTERVAL_V1);
        }
    }

    fn wait_for_process_group_absence(&self) -> Result<(), String> {
        let deadline = Instant::now() + CHILD_REAP_TIMEOUT_V1;
        loop {
            if !self.process_group_exists()? {
                #[cfg(test)]
                {
                    self.record_lifecycle(LifecycleEventV1::VerifiedProcessGroupAbsent);
                    if self
                        .lifecycle_test
                        .as_ref()
                        .is_some_and(|control| control.fail_absence_after_reap)
                    {
                        return Err("injected process-group absence failure".into());
                    }
                }
                return Ok(());
            }
            if Instant::now() >= deadline {
                return Err("owned child process group survived termination".into());
            }
            thread::sleep(CHILD_POLL_INTERVAL_V1);
        }
    }

    #[allow(unsafe_code)]
    fn signal_owned_process_group(&mut self) -> Result<(), String> {
        if self.group_signal_authority == ProcessGroupSignalAuthorityV1::Revoked {
            return Ok(());
        }
        self.group_signal_authority = ProcessGroupSignalAuthorityV1::Revoked;
        let process_group = i32::try_from(self.process_group)
            .map_err(|_| "owned child process group is invalid")?;
        #[cfg(test)]
        self.record_lifecycle(LifecycleEventV1::SignaledProcessGroup);
        // SAFETY: `process_group` is captured from the successfully spawned
        // child after `process_group(0)`. A negative PID targets only that
        // owned group; no pointers or borrowed memory cross the syscall.
        let result = unsafe { libc::kill(-process_group, libc::SIGKILL) };
        if result == 0 {
            #[cfg(test)]
            {
                if self
                    .lifecycle_test
                    .as_ref()
                    .is_some_and(|control| control.fail_signal_after_delivery)
                {
                    return Err("injected process-group signal failure".into());
                }
            }
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

fn combine_lifecycle_results(
    termination: Result<(), String>,
    reaped: Result<ExitStatus, String>,
    absent: Result<(), String>,
) -> Result<ExitStatus, String> {
    let status = reaped?;
    termination?;
    absent?;
    Ok(status)
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
        if executable.is_archive_sealed() {
            command.env_clear();
        }
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
        let bytes = encode_json_line(request, max_request)?;
        self.exchange_encoded_line(bytes, max_request)
    }

    fn exchange_encoded_line(
        &mut self,
        bytes: Vec<u8>,
        max_request: u64,
    ) -> Result<Vec<u8>, String> {
        match self.stdout.try_recv() {
            Err(TryRecvError::Empty) => {}
            Ok(_) => return Err(format!("{} emitted an unsolicited response", self.label)),
            Err(TryRecvError::Disconnected) => {
                return Err(format!("{} stdout closed before request", self.label));
            }
        }
        if bytes.is_empty()
            || bytes.len() as u64 > max_request
            || !bytes.ends_with(b"\n")
            || bytes[..bytes.len() - 1]
                .iter()
                .any(|byte| matches!(byte, b'\n' | b'\r'))
        {
            return Err(format!(
                "{} encoded request is not bounded JSONL",
                self.label
            ));
        }
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
            if self.owned.observe_leader_exit_without_reaping()? {
                break self.owned.finalize_observed_leader_exit()?;
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

fn write_lower_hex_json_string_v1(
    writer: &mut BoundedVecWriterV1,
    bytes: &[u8],
) -> Result<(), String> {
    bytes.len().checked_mul(2).ok_or("hex length overflow")?;
    writer.write_all(b"\"").map_err(|error| error.to_string())?;
    let mut encoded = [0_u8; 8 * 1024];
    for chunk in bytes.chunks(encoded.len() / 2) {
        for (index, byte) in chunk.iter().copied().enumerate() {
            encoded[index * 2] = hex_digit_v1(byte >> 4);
            encoded[index * 2 + 1] = hex_digit_v1(byte & 0x0f);
        }
        writer
            .write_all(&encoded[..chunk.len() * 2])
            .map_err(|error| error.to_string())?;
    }
    writer.write_all(b"\"").map_err(|error| error.to_string())
}

fn write_optional_lower_hex_json_string_v1(
    writer: &mut BoundedVecWriterV1,
    bytes: Option<&[u8]>,
) -> Result<(), String> {
    match bytes {
        Some(bytes) => write_lower_hex_json_string_v1(writer, bytes),
        None => writer.write_all(b"null").map_err(|error| error.to_string()),
    }
}

fn write_variant_treatment_v1(
    writer: &mut BoundedVecWriterV1,
    treatment: &LoadedTreatmentV1,
) -> Result<(), String> {
    writer
        .write_all(b"{\"manifest_hex\":")
        .map_err(|error| error.to_string())?;
    write_lower_hex_json_string_v1(writer, &treatment.manifest)?;
    writer
        .write_all(b",\"semantic_workload_hex\":")
        .map_err(|error| error.to_string())?;
    write_lower_hex_json_string_v1(writer, &treatment.semantic_workload)?;
    writer
        .write_all(b",\"raw_profiler_source_hex\":")
        .map_err(|error| error.to_string())?;
    write_lower_hex_json_string_v1(writer, &treatment.raw_profiler_source)?;
    writer
        .write_all(b",\"bundle_hex\":")
        .map_err(|error| error.to_string())?;
    write_lower_hex_json_string_v1(writer, &treatment.bundle)?;
    writer
        .write_all(b",\"schedule_hex\":")
        .map_err(|error| error.to_string())?;
    write_lower_hex_json_string_v1(writer, &treatment.schedule)?;
    writer
        .write_all(b",\"artifact_hex\":")
        .map_err(|error| error.to_string())?;
    write_lower_hex_json_string_v1(writer, &treatment.artifact)?;
    writer
        .write_all(b",\"isa_projection_hex\":")
        .map_err(|error| error.to_string())?;
    write_optional_lower_hex_json_string_v1(writer, treatment.isa_projection.as_deref())?;
    writer
        .write_all(b",\"counters_hex\":")
        .map_err(|error| error.to_string())?;
    write_optional_lower_hex_json_string_v1(writer, treatment.counters.as_deref())?;
    writer
        .write_all(b",\"pc_samples_hex\":")
        .map_err(|error| error.to_string())?;
    write_optional_lower_hex_json_string_v1(writer, treatment.pc_samples.as_deref())?;
    writer.write_all(b"}").map_err(|error| error.to_string())
}

fn encode_variant_comparison_request_v1(
    baseline: &LoadedTreatmentV1,
    candidate: &LoadedTreatmentV1,
) -> Result<Vec<u8>, String> {
    let mut writer = BoundedVecWriterV1::new(MAX_AGENT_PROFILER_VARIANT_REQUEST_BYTES_V1)?;
    writer
        .write_all(b"{\"operation\":\"compare_variants\",\"schema\":\"fe2o3-agent-profiler-variant-request-v1\",\"request_id\":2,\"expected_revision\":1,\"baseline\":")
        .map_err(|error| error.to_string())?;
    write_variant_treatment_v1(&mut writer, baseline)?;
    writer
        .write_all(b",\"candidate\":")
        .map_err(|error| error.to_string())?;
    write_variant_treatment_v1(&mut writer, candidate)?;
    writer
        .write_all(b"}\n")
        .map_err(|error| error.to_string())?;
    Ok(writer.into_inner())
}

fn encode_open_capture_request_v1(bundle: &[u8]) -> Result<Vec<u8>, String> {
    let mut writer = BoundedVecWriterV1::new(MAX_AGENT_PROFILER_REQUEST_BYTES_V1)?;
    writer
        .write_all(b"{\"operation\":\"open_capture\",\"schema\":\"fe2o3-agent-profiler-request-v1\",\"request_id\":2,\"bundle_hex\":")
        .map_err(|error| error.to_string())?;
    write_lower_hex_json_string_v1(&mut writer, bundle)?;
    writer
        .write_all(b"}\n")
        .map_err(|error| error.to_string())?;
    Ok(writer.into_inner())
}

const fn hex_digit_v1(value: u8) -> u8 {
    if value < 10 {
        b'0' + value
    } else {
        b'a' + value - 10
    }
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
    custody: ExecutableCustodyV1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ExecutableCustodyV1 {
    LegacyDescriptor,
    ArchiveSealedMemfd,
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

    fn same_legacy_descriptor_object(self, other: Self) -> bool {
        self.device == other.device
            && self.inode == other.inode
            && self.mode == other.mode
            && other.links <= self.links
            && self.bytes == other.bytes
            && self.modified_seconds == other.modified_seconds
            && self.modified_nanoseconds == other.modified_nanoseconds
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
            custody: ExecutableCustodyV1::LegacyDescriptor,
        })
    }

    fn open_archive(path: &Path, label: &str) -> Result<Self, String> {
        let source = OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC | libc::O_NONBLOCK)
            .open(path)
            .map_err(|_| format!("could not securely open {label}"))?;
        let before = source
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
        let source_metadata = StableFileMetadataV1::from_metadata(&before);
        let descriptor = rustix::fs::memfd_create(
            "fe2o3-reference-executable-v1",
            rustix::fs::MemfdFlags::CLOEXEC | rustix::fs::MemfdFlags::ALLOW_SEALING,
        )
        .map_err(|_| format!("could not create immutable {label} image"))?;
        let mut writable = File::from(descriptor);
        rustix::fs::fchmod(&writable, rustix::fs::Mode::from_raw_mode(0o500))
            .map_err(|_| format!("could not protect immutable {label} image mode"))?;
        let source_identity =
            copy_executable_to_memfd_v1(&source, &mut writable, source_metadata.bytes, label)?;
        let after = source
            .metadata()
            .map_err(|_| format!("could not revalidate opened {label}"))?;
        if source_metadata != StableFileMetadataV1::from_metadata(&after) {
            return Err(format!("{label} changed during immutable capture"));
        }
        writable
            .sync_all()
            .map_err(|_| format!("could not synchronize immutable {label} image"))?;
        seal_archive_memfd_v1(&writable, label)?;
        writable
            .seek(SeekFrom::Start(0))
            .map_err(|_| format!("could not rewind immutable {label} image"))?;
        let writable_path = format!("/proc/self/fd/{}", writable.as_raw_fd());
        let read_only = rustix::fs::open(
            writable_path,
            rustix::fs::OFlags::RDONLY | rustix::fs::OFlags::CLOEXEC,
            rustix::fs::Mode::empty(),
        )
        .map(File::from)
        .map_err(|_| format!("could not bind read-only immutable {label} image"))?;
        drop(writable);
        let metadata = read_only
            .metadata()
            .map_err(|_| format!("could not inspect immutable {label} image"))?;
        let metadata = StableFileMetadataV1::from_metadata(&metadata);
        let identity = executable_content_identity(&read_only, metadata.bytes, label)?;
        if identity != source_identity {
            return Err(format!("immutable {label} image identity mismatch"));
        }
        let result = Self {
            file: read_only,
            metadata,
            identity,
            custody: ExecutableCustodyV1::ArchiveSealedMemfd,
        };
        result.revalidate(label)?;
        Ok(result)
    }

    fn proc_path(&self) -> String {
        format!("/proc/self/fd/{}", self.file.as_raw_fd())
    }

    fn identity(&self) -> &ExecutableContentIdentityV1 {
        &self.identity
    }

    fn is_archive_sealed(&self) -> bool {
        self.custody == ExecutableCustodyV1::ArchiveSealedMemfd
    }

    fn revalidate(&self, label: &str) -> Result<(), String> {
        let before = self
            .file
            .metadata()
            .map_err(|_| format!("could not inspect pinned {label}"))?;
        let before = StableFileMetadataV1::from_metadata(&before);
        let metadata_matches = match self.custody {
            ExecutableCustodyV1::LegacyDescriptor => {
                self.metadata.same_legacy_descriptor_object(before)
            }
            ExecutableCustodyV1::ArchiveSealedMemfd => self.metadata == before,
        };
        if !metadata_matches {
            return Err(format!("pinned {label} metadata changed"));
        }
        let identity = executable_content_identity(&self.file, self.metadata.bytes, label)?;
        let after = self
            .file
            .metadata()
            .map_err(|_| format!("could not revalidate pinned {label}"))?;
        let after = StableFileMetadataV1::from_metadata(&after);
        let metadata_matches = match self.custody {
            ExecutableCustodyV1::LegacyDescriptor => {
                self.metadata.same_legacy_descriptor_object(after)
            }
            ExecutableCustodyV1::ArchiveSealedMemfd => self.metadata == after,
        };
        if !metadata_matches || identity != self.identity {
            return Err(format!("pinned {label} content changed"));
        }
        if self.is_archive_sealed() {
            validate_archive_executable_memfd_v1(&self.file, self.metadata, label)?;
        }
        Ok(())
    }
}

fn copy_executable_to_memfd_v1(
    source: &File,
    destination: &mut File,
    bytes: u64,
    label: &str,
) -> Result<ExecutableContentIdentityV1, String> {
    let mut digest = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    let mut offset = 0_u64;
    while offset < bytes {
        let remaining = usize::try_from((bytes - offset).min(buffer.len() as u64))
            .map_err(|_| format!("{label} size conversion failed"))?;
        let read = source
            .read_at(&mut buffer[..remaining], offset)
            .map_err(|_| format!("could not copy admitted {label}"))?;
        if read == 0 {
            return Err(format!("admitted {label} was truncated"));
        }
        destination
            .write_all(&buffer[..read])
            .map_err(|_| format!("could not populate immutable {label} image"))?;
        digest.update(&buffer[..read]);
        offset = offset
            .checked_add(read as u64)
            .ok_or_else(|| format!("{label} size overflow"))?;
    }
    Ok(ExecutableContentIdentityV1 {
        scheme: "sha256_of_exact_executable_bytes",
        sha256: lower_hex(&digest.finalize())?,
        bytes,
    })
}

fn seal_archive_memfd_v1(file: &File, label: &str) -> Result<(), String> {
    rustix::fs::fcntl_add_seals(
        file,
        rustix::fs::SealFlags::WRITE | rustix::fs::SealFlags::GROW | rustix::fs::SealFlags::SHRINK,
    )
    .and_then(|()| rustix::fs::fcntl_add_seals(file, rustix::fs::SealFlags::SEAL))
    .map_err(|_| format!("could not seal immutable {label} image"))?;
    if rustix::fs::fcntl_get_seals(file)
        .map_err(|_| format!("could not inspect immutable {label} image seals"))?
        != REQUIRED_ARCHIVE_MEMFD_SEALS_V1
    {
        return Err(format!("immutable {label} image has unexpected seals"));
    }
    Ok(())
}

fn validate_archive_executable_memfd_v1(
    file: &File,
    expected: StableFileMetadataV1,
    label: &str,
) -> Result<(), String> {
    let metadata = file
        .metadata()
        .map_err(|_| format!("could not inspect immutable {label} image"))?;
    let status = rustix::fs::fcntl_getfl(file)
        .map_err(|_| format!("could not inspect immutable {label} status"))?;
    if StableFileMetadataV1::from_metadata(&metadata) != expected
        || !metadata.file_type().is_file()
        || metadata.nlink() != 0
        || metadata.mode() & 0o7777 != 0o500
        || status & rustix::fs::OFlags::ACCMODE != rustix::fs::OFlags::RDONLY
        || status.contains(rustix::fs::OFlags::PATH)
        || rustix::fs::fcntl_get_seals(file)
            .map_err(|_| format!("could not inspect immutable {label} image seals"))?
            != REQUIRED_ARCHIVE_MEMFD_SEALS_V1
    {
        return Err(format!("immutable {label} image changed"));
    }
    let link = fs::read_link(format!("/proc/self/fd/{}", file.as_raw_fd()))
        .map_err(|_| format!("could not inspect immutable {label} descriptor"))?;
    let link = link.as_os_str().as_encoded_bytes();
    if !(link.starts_with(b"/memfd:") || link.starts_with(b"memfd:"))
        || !link.ends_with(b" (deleted)")
    {
        return Err(format!("immutable {label} image is not a memfd"));
    }
    Ok(())
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
        sha256: lower_hex(&digest)?,
        bytes,
    })
}

fn read_optional_budgeted(
    path: &Option<PathBuf>,
    remaining: &mut u64,
    label: &str,
) -> Result<Option<Vec<u8>>, String> {
    path.as_ref()
        .map(|path| read_budgeted(path, remaining, label))
        .transpose()
}

fn read_budgeted(path: &Path, remaining: &mut u64, label: &str) -> Result<Vec<u8>, String> {
    let bytes = read_bounded(path, *remaining, label)?;
    let admitted = u64::try_from(bytes.len()).map_err(|_| format!("{label} is too large"))?;
    *remaining = remaining
        .checked_sub(admitted)
        .ok_or_else(|| format!("{label} exceeds the remaining treatment budget"))?;
    Ok(bytes)
}

fn read_bounded(path: impl AsRef<Path>, max: u64, label: &str) -> Result<Vec<u8>, String> {
    read_bounded_descriptor_with_post_read(path, max, label, || {})
}

fn read_archive_bounded(path: impl AsRef<Path>, max: u64, label: &str) -> Result<Vec<u8>, String> {
    read_archive_bounded_with_post_read(path, max, label, || {})
}

fn read_bounded_descriptor_with_post_read(
    path: impl AsRef<Path>,
    max: u64,
    label: &str,
    post_read: impl FnOnce(),
) -> Result<Vec<u8>, String> {
    read_bounded_impl_v1(path, max, label, post_read, false)
}

fn read_archive_bounded_with_post_read(
    path: impl AsRef<Path>,
    max: u64,
    label: &str,
    post_read: impl FnOnce(),
) -> Result<Vec<u8>, String> {
    read_bounded_impl_v1(path, max, label, post_read, true)
}

fn read_bounded_impl_v1(
    path: impl AsRef<Path>,
    max: u64,
    label: &str,
    post_read: impl FnOnce(),
    require_persistent_path: bool,
) -> Result<Vec<u8>, String> {
    let path = path.as_ref();
    let mut file = OpenOptions::new()
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
    post_read();
    let after = file
        .metadata()
        .map_err(|_| format!("could not revalidate opened {label}"))?;
    let before_metadata = StableFileMetadataV1::from_metadata(&before);
    let after_metadata = StableFileMetadataV1::from_metadata(&after);
    let stable = if require_persistent_path {
        before_metadata == after_metadata
    } else {
        before_metadata.same_legacy_descriptor_object(after_metadata)
    };
    let content_stable = descriptor_matches_bytes_v1(&file, &bytes, label)?;
    let persistent = !require_persistent_path
        || OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC | libc::O_NONBLOCK)
            .open(path)
            .and_then(|file| file.metadata())
            .as_ref()
            .is_ok_and(|metadata| StableFileMetadataV1::from_metadata(metadata) == before_metadata);
    if !stable
        || !content_stable
        || !persistent
        || bytes.len() as u64 != before.len()
        || bytes.len() as u64 > max
    {
        return Err(format!("{label} changed during admission"));
    }
    Ok(bytes)
}

fn descriptor_matches_bytes_v1(file: &File, expected: &[u8], label: &str) -> Result<bool, String> {
    let mut buffer = [0_u8; 64 * 1024];
    let mut offset = 0_usize;
    while offset < expected.len() {
        let count = (expected.len() - offset).min(buffer.len());
        let file_offset =
            u64::try_from(offset).map_err(|_| format!("{label} validation offset is too large"))?;
        let read = file
            .read_at(&mut buffer[..count], file_offset)
            .map_err(|_| format!("could not revalidate opened {label} content"))?;
        if read == 0 || buffer[..read] != expected[offset..offset + read] {
            return Ok(false);
        }
        offset = offset
            .checked_add(read)
            .ok_or_else(|| format!("{label} validation offset overflow"))?;
    }
    let trailing_offset = u64::try_from(expected.len())
        .map_err(|_| format!("{label} validation offset is too large"))?;
    let mut trailing = [0_u8; 1];
    file.read_at(&mut trailing, trailing_offset)
        .map(|read| read == 0)
        .map_err(|_| format!("could not revalidate opened {label} content"))
}

fn lower_hex(bytes: &[u8]) -> Result<String, String> {
    let capacity = bytes.len().checked_mul(2).ok_or("hex length overflow")?;
    let mut output = String::new();
    output
        .try_reserve_exact(capacity)
        .map_err(|_| "hex allocation failed")?;
    for byte in bytes.iter().copied() {
        output.push(char::from(hex_digit_v1(byte >> 4)));
        output.push(char::from(hex_digit_v1(byte & 0x0f)));
    }
    Ok(output)
}

struct SealedInputV1 {
    file: File,
    object: (u64, u64, u64),
    identity: [u8; 32],
}

impl SealedInputV1 {
    fn new(label: &str, bytes: &[u8]) -> Result<Self, String> {
        if bytes.is_empty() || bytes.len() as u64 > MAX_DEBUG_INPUT_BYTES_V1 {
            return Err(format!("{label} exceeds its bounded sealed-input contract"));
        }
        let descriptor = rustix::fs::memfd_create(
            "fe2o3-reference-debug-input-v1",
            rustix::fs::MemfdFlags::CLOEXEC | rustix::fs::MemfdFlags::ALLOW_SEALING,
        )
        .map_err(|_| format!("could not create sealed {label}"))?;
        let writable = File::from(descriptor);
        rustix::fs::fchmod(&writable, rustix::fs::Mode::from_raw_mode(0o400))
            .map_err(|_| format!("could not protect sealed {label} mode"))?;
        writable
            .write_all_at(bytes, 0)
            .and_then(|()| writable.sync_all())
            .map_err(|_| format!("could not populate sealed {label}"))?;
        seal_archive_memfd_v1(&writable, label)?;
        let writable_path = format!("/proc/self/fd/{}", writable.as_raw_fd());
        let file = rustix::fs::open(
            writable_path,
            rustix::fs::OFlags::RDONLY | rustix::fs::OFlags::CLOEXEC,
            rustix::fs::Mode::empty(),
        )
        .map(File::from)
        .map_err(|_| format!("could not bind read-only sealed {label}"))?;
        drop(writable);
        let stat =
            rustix::fs::fstat(&file).map_err(|_| format!("could not inspect sealed {label}"))?;
        let object = (
            stat.st_dev,
            stat.st_ino,
            u64::try_from(stat.st_size).map_err(|_| format!("invalid sealed {label} size"))?,
        );
        let result = Self {
            file,
            object,
            identity: Sha256::digest(bytes).into(),
        };
        result.revalidate(label)?;
        Ok(result)
    }

    fn revalidate(&self, label: &str) -> Result<(), String> {
        let stat = rustix::fs::fstat(&self.file)
            .map_err(|_| format!("could not inspect sealed {label}"))?;
        let length = usize::try_from(stat.st_size)
            .ok()
            .filter(|length| *length > 0 && *length as u64 <= MAX_DEBUG_INPUT_BYTES_V1)
            .ok_or_else(|| format!("sealed {label} has an invalid size"))?;
        let status = rustix::fs::fcntl_getfl(&self.file)
            .map_err(|_| format!("could not inspect sealed {label} status"))?;
        if rustix::fs::FileType::from_raw_mode(stat.st_mode) != rustix::fs::FileType::RegularFile
            || stat.st_nlink != 0
            || stat.st_mode & 0o7777 != 0o400
            || status & rustix::fs::OFlags::ACCMODE != rustix::fs::OFlags::RDONLY
            || status.contains(rustix::fs::OFlags::PATH)
            || rustix::fs::fcntl_get_seals(&self.file)
                .map_err(|_| format!("could not inspect sealed {label} seals"))?
                != REQUIRED_ARCHIVE_MEMFD_SEALS_V1
            || self.object != (stat.st_dev, stat.st_ino, stat.st_size as u64)
        {
            return Err(format!("sealed {label} changed"));
        }
        let mut bytes = Vec::new();
        bytes
            .try_reserve_exact(length)
            .map_err(|_| format!("could not reserve sealed {label} validation"))?;
        bytes.resize(length, 0);
        self.file
            .read_exact_at(&mut bytes, 0)
            .map_err(|_| format!("could not validate sealed {label}"))?;
        if <[u8; 32]>::from(Sha256::digest(&bytes)) != self.identity {
            return Err(format!("sealed {label} identity changed"));
        }
        Ok(())
    }
}

#[allow(unsafe_code)]
fn inherit_sealed_inputs_v1(
    command: &mut Command,
    kernel: &SealedInputV1,
    request: &SealedInputV1,
) -> Result<(), String> {
    kernel.revalidate("canonical KIR V7")?;
    request.revalidate("simulation request")?;
    if kernel.file.as_raw_fd() == request.file.as_raw_fd() || kernel.object == request.object {
        return Err("sealed debugger inputs are not distinct objects".into());
    }
    let kernel_fd = kernel.file.as_raw_fd();
    let request_fd = request.file.as_raw_fd();
    let kernel_object = kernel.object;
    let request_object = request.object;
    // SAFETY: both sealed descriptors remain owned by `diagnose_simulator`
    // through spawn. The callback validates each object and changes only the
    // child descriptor flags needed for the two explicit debugger fd options.
    unsafe {
        command.pre_exec(move || {
            for (descriptor, expected) in [(kernel_fd, kernel_object), (request_fd, request_object)]
            {
                let descriptor = BorrowedFd::borrow_raw(descriptor);
                let stat = rustix::fs::fstat(descriptor).map_err(io::Error::from)?;
                let actual = (
                    stat.st_dev,
                    stat.st_ino,
                    u64::try_from(stat.st_size)
                        .map_err(|_| io::Error::from_raw_os_error(libc::EINVAL))?,
                );
                let status = rustix::fs::fcntl_getfl(descriptor).map_err(io::Error::from)?;
                if actual != expected
                    || rustix::fs::FileType::from_raw_mode(stat.st_mode)
                        != rustix::fs::FileType::RegularFile
                    || stat.st_nlink != 0
                    || stat.st_mode & 0o7777 != 0o400
                    || status & rustix::fs::OFlags::ACCMODE != rustix::fs::OFlags::RDONLY
                    || status.contains(rustix::fs::OFlags::PATH)
                    || rustix::fs::fcntl_get_seals(descriptor).map_err(io::Error::from)?
                        != REQUIRED_ARCHIVE_MEMFD_SEALS_V1
                {
                    return Err(io::Error::from_raw_os_error(libc::ESTALE));
                }
                rustix::io::fcntl_setfd(descriptor, rustix::io::FdFlags::empty())
                    .map_err(io::Error::from)?;
            }
            Ok(())
        });
    }
    Ok(())
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

    fn helper_archive_shell(root: &Path) -> (PinnedExecutableV1, PathBuf) {
        let shell = fs::canonicalize("/bin/sh").unwrap();
        let installed = root.join("archive-test-producer");
        fs::copy(shell, &installed).unwrap();
        fs::set_permissions(&installed, fs::Permissions::from_mode(0o700)).unwrap();
        (
            PinnedExecutableV1::open_archive(&installed, "archive test producer").unwrap(),
            installed,
        )
    }

    fn route_test_workflow(executable: &Path, evidence: &Path) -> WorkflowV1 {
        let simulator_case = || SimulatorCaseV1 {
            kernel: evidence.to_owned(),
            request: evidence.to_owned(),
        };
        let treatment = || TreatmentFilesV1 {
            manifest: evidence.to_owned(),
            semantic_workload: evidence.to_owned(),
            raw_profiler_source: evidence.to_owned(),
            bundle: evidence.to_owned(),
            schedule: evidence.to_owned(),
            artifact: evidence.to_owned(),
            isa_projection: None,
            counters: None,
            pc_samples: None,
        };
        WorkflowV1 {
            schema: WORKFLOW_SCHEMA_V1.to_owned(),
            trusted_debugger_executable: executable.to_owned(),
            trusted_profiler_service_executable: executable.to_owned(),
            out_of_bounds: simulator_case(),
            barrier_divergence: simulator_case(),
            baseline: treatment(),
            candidate: treatment(),
        }
    }

    fn route_test_archive() -> ReferenceEvidenceArchiveV1 {
        use fe2o3_debug_cli::reference_archive_v1::{
            ReferenceEvidenceArchiveInputV1, ReferenceSimulatorCaseInputV1,
            ReferenceTreatmentInputV1, encode_reference_evidence_archive_v1,
            reference_evidence_archive_sha256_v1,
        };

        let treatment = || ReferenceTreatmentInputV1 {
            manifest: b"manifest",
            semantic_workload: b"workload",
            raw_profiler_source: b"profiler",
            bundle: b"bundle",
            schedule: b"schedule",
            artifact: b"artifact",
            isa_projection: None,
            counters: None,
            pc_samples: None,
        };
        let bytes = encode_reference_evidence_archive_v1(ReferenceEvidenceArchiveInputV1 {
            out_of_bounds: ReferenceSimulatorCaseInputV1 {
                kernel: b"out-of-bounds KIR",
                request: b"out-of-bounds request",
            },
            barrier_divergence: ReferenceSimulatorCaseInputV1 {
                kernel: b"barrier KIR",
                request: b"barrier request",
            },
            baseline: treatment(),
            candidate: treatment(),
        })
        .unwrap();
        decode_reference_evidence_archive_v1(&bytes, reference_evidence_archive_sha256_v1(&bytes))
            .unwrap()
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
                        "source_kind":"rocprofv3_kernel_dispatch_json",
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
                            "device_identity":"cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc",
                            "process_index":0,
                            "dispatch_index":0,
                            "launch":{
                                "logical_grid":[64,1,1],
                                "grid_workgroups":[1,1,1],
                                "workgroup_size":[64,1,1],
                                "wave_width":64
                            },
                            "start_timestamp":10,
                            "end_timestamp":20,
                            "duration_ticks":10,
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

    fn decoded_page(response: &Value, request_id: u64, revision: u64) -> AgentPageResultWireV1 {
        let mut line = serde_json::to_vec(response).unwrap();
        line.push(b'\n');
        let ValidatedAgentResponseV1::Page(page) = validate_agent_response_line_v1(
            &line,
            request_id,
            revision,
            ExpectedAgentResultV1::Page,
        )
        .unwrap() else {
            panic!("response was not a page")
        };
        page
    }

    fn expected_page(response: &AgentPageResultWireV1) -> ProfilerPageV4 {
        ProfilerPageV4 {
            context: response.page.context,
            kind: response.page.kind,
            returned: response.page.returned,
            next_cursor: response.page.next_cursor,
            items: response
                .page
                .items
                .iter()
                .map(|item| match item {
                    AgentDispatchItemWireV1::Dispatch { dispatch } => {
                        ProfilerQueryItemV4::Dispatch {
                            dispatch: *dispatch,
                        }
                    }
                })
                .collect(),
        }
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
        assert_eq!(
            pinned.identity().sha256,
            lower_hex(&original_digest).unwrap()
        );

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
    fn archive_executable_is_immutable_and_child_environment_is_empty() {
        let ordinal = NEXT_SNAPSHOT_V1.fetch_add(1, Ordering::Relaxed);
        let root = env::temp_dir().join(format!(
            "fe2o3-archive-executable-test-{}-{ordinal}",
            std::process::id()
        ));
        fs::create_dir(&root).unwrap();
        let (executable, installed) = helper_archive_shell(&root);
        assert!(executable.is_archive_sealed());
        assert_eq!(
            rustix::fs::fcntl_get_seals(&executable.file).unwrap(),
            REQUIRED_ARCHIVE_MEMFD_SEALS_V1
        );
        let retained = root.join("retained-producer");
        fs::rename(&installed, &retained).unwrap();
        fs::copy("/bin/false", &installed).unwrap();
        fs::set_permissions(&installed, fs::Permissions::from_mode(0o700)).unwrap();
        executable.revalidate("archive test producer").unwrap();
        let linked = root.join("linked-producer");
        symlink(&installed, &linked).unwrap();
        assert!(PinnedExecutableV1::open_archive(&linked, "linked producer").is_err());
        fs::remove_file(&linked).unwrap();
        fs::hard_link(&installed, &linked).unwrap();
        assert!(PinnedExecutableV1::open_archive(&installed, "linked producer").is_err());
        fs::remove_file(&linked).unwrap();

        let mut command = Command::new(executable.proc_path());
        command
            .args([
                "-c",
                "IFS= read -r line; environment_bytes=$(/usr/bin/wc -c < /proc/$$/environ); if [ \"$environment_bytes\" -ne 0 ]; then printf 'hostile\\n'; else printf 'ok\\n'; fi",
            ])
            .env("LD_PRELOAD", "/hostile/preload.so")
            .env("LD_LIBRARY_PATH", "/hostile/loader")
            .env("LANG", "hostile_LOCALE")
            .env("PATH", "/hostile/path")
            .env("TMPDIR", "/hostile/tmp")
            .env("ROCM_PATH", "/hostile/rocm")
            .env("ASAN_OPTIONS", "hostile=1")
            .env("FE2O3_HOSTILE", "1");
        let mut child =
            BoundedChildV1::spawn(command, &executable, "archive test producer", 32, 64).unwrap();
        assert_eq!(
            child
                .exchange_line(&json!({"request":"test"}), 1024)
                .unwrap(),
            b"ok\n"
        );
        child.finish().unwrap();
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn archive_route_seals_exact_executables_and_clears_child_environment() {
        let ordinal = NEXT_SNAPSHOT_V1.fetch_add(1, Ordering::Relaxed);
        let root = env::temp_dir().join(format!(
            "fe2o3-archive-route-test-{}-{ordinal}",
            std::process::id()
        ));
        fs::create_dir(&root).unwrap();
        let installed = root.join("installed-producer");
        fs::copy(fs::canonicalize("/bin/sh").unwrap(), &installed).unwrap();
        fs::set_permissions(&installed, fs::Permissions::from_mode(0o700)).unwrap();
        let source_bytes = fs::read(&installed).unwrap();
        let source_digest: [u8; 32] = Sha256::digest(&source_bytes).into();

        let loaded = LoadedWorkflowV1::load_archive(
            installed.as_os_str(),
            installed.as_os_str(),
            route_test_archive(),
        )
        .unwrap();
        for executable in [
            &loaded.debugger_executable,
            &loaded.profiler_service_executable,
        ] {
            assert_eq!(executable.custody, ExecutableCustodyV1::ArchiveSealedMemfd);
            assert_eq!(
                executable.identity().sha256,
                lower_hex(&source_digest).unwrap()
            );
            assert_eq!(fs::read(executable.proc_path()).unwrap(), source_bytes);
        }

        let retained = root.join("retained-producer");
        fs::rename(&installed, &retained).unwrap();
        fs::copy("/bin/false", &installed).unwrap();
        fs::set_permissions(&installed, fs::Permissions::from_mode(0o700)).unwrap();
        fs::remove_file(&retained).unwrap();
        loaded
            .debugger_executable
            .revalidate("archive route debugger")
            .unwrap();

        let mut command = Command::new(loaded.debugger_executable.proc_path());
        command
            .args([
                "-c",
                "IFS= read -r line; environment_bytes=$(/usr/bin/wc -c < /proc/$$/environ); if [ \"$environment_bytes\" -ne 0 ]; then printf 'hostile\\n'; else printf 'empty\\n'; fi",
            ])
            .env("LD_PRELOAD", "/hostile/preload.so")
            .env("LD_LIBRARY_PATH", "/hostile/loader")
            .env("PATH", "/hostile/path")
            .env("TMPDIR", "/hostile/tmp")
            .env("ROCM_PATH", "/hostile/rocm")
            .env("FE2O3_HOSTILE", "1");
        let mut child = BoundedChildV1::spawn(
            command,
            &loaded.debugger_executable,
            "archive route debugger",
            32,
            64,
        )
        .unwrap();
        assert_eq!(
            child
                .exchange_line(&json!({"request":"test"}), 1024)
                .unwrap(),
            b"empty\n"
        );
        child.finish().unwrap();
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn legacy_route_retains_descriptor_environment_and_loaded_loose_inputs() {
        let ordinal = NEXT_SNAPSHOT_V1.fetch_add(1, Ordering::Relaxed);
        let root = env::temp_dir().join(format!(
            "fe2o3-legacy-route-test-{}-{ordinal}",
            std::process::id()
        ));
        fs::create_dir(&root).unwrap();
        let installed = root.join("installed-producer");
        fs::copy(fs::canonicalize("/bin/sh").unwrap(), &installed).unwrap();
        fs::set_permissions(&installed, fs::Permissions::from_mode(0o700)).unwrap();
        let source_bytes = fs::read(&installed).unwrap();
        let source_digest: [u8; 32] = Sha256::digest(&source_bytes).into();
        let evidence = root.join("loose-evidence");
        fs::write(&evidence, b"legacy evidence bytes").unwrap();

        let loaded = LoadedWorkflowV1::load(route_test_workflow(&installed, &evidence)).unwrap();
        for executable in [
            &loaded.debugger_executable,
            &loaded.profiler_service_executable,
        ] {
            assert_eq!(executable.custody, ExecutableCustodyV1::LegacyDescriptor);
            assert_eq!(
                executable.identity().sha256,
                lower_hex(&source_digest).unwrap()
            );
        }

        let retained_executable = root.join("retained-producer");
        fs::rename(&installed, &retained_executable).unwrap();
        fs::copy("/bin/false", &installed).unwrap();
        fs::set_permissions(&installed, fs::Permissions::from_mode(0o700)).unwrap();
        assert_eq!(
            fs::read(loaded.debugger_executable.proc_path()).unwrap(),
            source_bytes
        );

        let retained_evidence = root.join("retained-evidence");
        fs::rename(&evidence, &retained_evidence).unwrap();
        fs::write(&evidence, b"replacement evidence").unwrap();
        fs::remove_file(&evidence).unwrap();
        fs::remove_file(&retained_evidence).unwrap();
        assert_eq!(loaded.out_of_bounds.kernel, b"legacy evidence bytes");

        let mut command = Command::new(loaded.debugger_executable.proc_path());
        command
            .args([
                "-c",
                "IFS= read -r line; printf '%s\\n' \"${FE2O3_LEGACY_MARKER-unset}\"",
            ])
            .env("FE2O3_LEGACY_MARKER", "retained");
        let mut child = BoundedChildV1::spawn(
            command,
            &loaded.debugger_executable,
            "legacy route debugger",
            32,
            64,
        )
        .unwrap();
        assert_eq!(
            child
                .exchange_line(&json!({"request":"test"}), 1024)
                .unwrap(),
            b"retained\n"
        );
        child.finish().unwrap();
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn sealed_debug_input_rejects_alias_and_closes_without_named_cleanup() {
        let input = SealedInputV1::new("test input", b"exact input bytes").unwrap();
        let descriptor = input.file.as_raw_fd();
        let retained = input.file.try_clone().unwrap();
        let object = input.object;
        assert!(fs::symlink_metadata(format!("/proc/self/fd/{descriptor}")).is_ok());
        assert!(input.file.write_all_at(b"x", 0).is_err());
        assert!(input.file.set_len(0).is_err());
        let mut command = Command::new("/bin/false");
        assert!(inherit_sealed_inputs_v1(&mut command, &input, &input).is_err());
        drop(input);
        match fs::metadata(format!("/proc/self/fd/{descriptor}")) {
            Ok(metadata) => assert_ne!((metadata.dev(), metadata.ino(), metadata.len()), object),
            Err(error) => assert_eq!(error.kind(), io::ErrorKind::NotFound),
        }
        drop(retained);
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
        let lifecycle_events = Arc::new(Mutex::new(Vec::new()));
        child
            .owned
            .set_lifecycle_test_control(LifecycleTestControlV1 {
                events: Arc::clone(&lifecycle_events),
                fail_signal_after_delivery: false,
                fail_absence_after_reap: false,
            });
        let pid = wait_for_pid(&successful_leader_pid);
        assert_eq!(child.exchange_line(&request, 1024).unwrap(), b"ok\n");
        let descendant = wait_for_pid(&detached_descendant_pid);
        child.finish().unwrap();
        assert_eq!(
            *lifecycle_events.lock().unwrap(),
            [
                LifecycleEventV1::ObservedLeaderExit,
                LifecycleEventV1::SignaledProcessGroup,
                LifecycleEventV1::ReapedDirectChild,
                LifecycleEventV1::VerifiedProcessGroupAbsent,
            ]
        );
        assert_pid_reaped(pid);
        assert_pid_reaped(descendant);

        for (label, fail_signal, fail_absence) in [
            ("signal-failure", true, false),
            ("absence-failure", false, true),
        ] {
            let pid_path = root.join(format!("{label}.pid"));
            let script = format!(
                "printf '%s\\n' $$ > {}; IFS= read -r line; printf 'ok\\n'",
                pid_path.display()
            );
            let mut child = helper_session(&executable, &script, Duration::from_secs(2));
            let events = Arc::new(Mutex::new(Vec::new()));
            child
                .owned
                .set_lifecycle_test_control(LifecycleTestControlV1 {
                    events: Arc::clone(&events),
                    fail_signal_after_delivery: fail_signal,
                    fail_absence_after_reap: fail_absence,
                });
            let pid = wait_for_pid(&pid_path);
            assert_eq!(child.exchange_line(&request, 1024).unwrap(), b"ok\n");
            assert!(child.finish().is_err());
            assert_pid_reaped(pid);
            let events = events.lock().unwrap();
            assert_eq!(
                events[..4],
                [
                    LifecycleEventV1::ObservedLeaderExit,
                    LifecycleEventV1::SignaledProcessGroup,
                    LifecycleEventV1::ReapedDirectChild,
                    LifecycleEventV1::VerifiedProcessGroupAbsent,
                ]
            );
            assert_eq!(
                events
                    .iter()
                    .filter(|event| **event == LifecycleEventV1::SignaledProcessGroup)
                    .count(),
                1,
                "Drop must not signal a revoked PGID after {label}"
            );
        }

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

    fn simulator_session(
        revision: u64,
        state: &str,
        configuration: &str,
        event: u64,
    ) -> SessionViewV1 {
        serde_json::from_value(json!({
            "backend":"cpu_kir_simulator",
            "execution_kind":"cpu_kir_simulation",
            "state":state,
            "revision":revision,
            "configuration_identity":configuration,
            "cursor":{
                "configuration_identity":configuration,
                "event_sequence":event,
                "state_revision":revision
            },
            "simulated":true,
            "hardware_observed":false,
            "performance_prediction":false
        }))
        .unwrap()
    }

    #[test]
    fn simulator_session_chain_rejects_stale_config_cursor_and_state() {
        let first = "1111111111111111111111111111111111111111111111111111111111111111";
        let second = "2222222222222222222222222222222222222222222222222222222222222222";
        let capability = simulator_session(0, "stopped", first, 0);
        let continued = simulator_session(1, "stopped", first, 9);
        validate_simulator_session_chain_v1(capability, continued, continued, None).unwrap();

        for hostile in [
            simulator_session(0, "stopped", first, 9),
            simulator_session(1, "stopped", second, 9),
            simulator_session(1, "stopped", first, 10),
            simulator_session(1, "running", first, 9),
        ] {
            assert!(
                validate_simulator_session_chain_v1(capability, continued, hostile, None).is_err()
            );
        }
        let cursor: PageCursorV1 = serde_json::from_value(json!({
            "query_identity":"3333333333333333333333333333333333333333333333333333333333333333",
            "position":1
        }))
        .unwrap();
        assert!(
            validate_simulator_session_chain_v1(capability, continued, continued, Some(cursor))
                .is_err()
        );
    }

    #[test]
    fn treatment_admission_rejects_aggregate_overflow_before_excess_read() {
        let ordinal = NEXT_SNAPSHOT_V1.fetch_add(1, Ordering::Relaxed);
        let root = env::temp_dir().join(format!(
            "fe2o3-treatment-budget-test-{}-{ordinal}",
            std::process::id()
        ));
        fs::create_dir(&root).unwrap();
        let paths = (0..7)
            .map(|index| root.join(format!("part-{index}")))
            .collect::<Vec<_>>();
        for path in &paths {
            fs::write(path, b"x").unwrap();
        }
        let files = TreatmentFilesV1 {
            manifest: paths[0].clone(),
            semantic_workload: paths[1].clone(),
            raw_profiler_source: paths[2].clone(),
            bundle: paths[3].clone(),
            schedule: paths[4].clone(),
            artifact: paths[5].clone(),
            isa_projection: Some(paths[6].clone()),
            counters: None,
            pc_samples: None,
        };
        assert!(LoadedTreatmentV1::load_with_budget(files, 6).is_err());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn evidence_reader_rejects_path_rename_and_replacement_during_admission() {
        let ordinal = NEXT_SNAPSHOT_V1.fetch_add(1, Ordering::Relaxed);
        let root = env::temp_dir().join(format!(
            "fe2o3-archive-rename-test-{}-{ordinal}",
            std::process::id()
        ));
        fs::create_dir(&root).unwrap();
        let selected = root.join("selected.fe2archive");
        let retained = root.join("retained.fe2archive");
        let replacement = root.join("replacement.fe2archive");
        fs::write(&selected, b"selected archive bytes").unwrap();
        fs::write(&replacement, b"replacement archive bytes").unwrap();
        let result = read_archive_bounded_with_post_read(
            &selected,
            1_024,
            "reference evidence archive",
            || {
                fs::rename(&selected, &retained).unwrap();
                fs::rename(&replacement, &selected).unwrap();
            },
        );
        assert_eq!(
            result.unwrap_err(),
            "reference evidence archive changed during admission"
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn legacy_descriptor_reader_retains_renamed_snapshot() {
        let ordinal = NEXT_SNAPSHOT_V1.fetch_add(1, Ordering::Relaxed);
        let root = env::temp_dir().join(format!(
            "fe2o3-legacy-rename-test-{}-{ordinal}",
            std::process::id()
        ));
        fs::create_dir(&root).unwrap();
        let selected = root.join("selected.json");
        let retained = root.join("retained.json");
        let replacement = root.join("replacement.json");
        fs::write(&selected, b"selected legacy bytes").unwrap();
        fs::write(&replacement, b"replacement legacy bytes").unwrap();
        let admitted =
            read_bounded_descriptor_with_post_read(&selected, 1_024, "legacy descriptor", || {
                fs::rename(&selected, &retained).unwrap();
                fs::rename(&replacement, &selected).unwrap();
            })
            .unwrap();
        assert_eq!(admitted, b"selected legacy bytes");

        let unlinked = root.join("unlinked.json");
        fs::write(&unlinked, b"unlinked legacy bytes").unwrap();
        let admitted =
            read_bounded_descriptor_with_post_read(&unlinked, 1_024, "legacy descriptor", || {
                fs::remove_file(&unlinked).unwrap()
            })
            .unwrap();
        assert_eq!(admitted, b"unlinked legacy bytes");
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn archive_reader_rejects_same_inode_mutation_during_admission() {
        let ordinal = NEXT_SNAPSHOT_V1.fetch_add(1, Ordering::Relaxed);
        let root = env::temp_dir().join(format!(
            "fe2o3-archive-mutation-test-{}-{ordinal}",
            std::process::id()
        ));
        fs::create_dir(&root).unwrap();
        let selected = root.join("selected.fe2archive");
        fs::write(&selected, b"selected archive bytes").unwrap();
        let result = read_archive_bounded_with_post_read(
            &selected,
            1_024,
            "reference evidence archive",
            || {
                let mut file = OpenOptions::new().write(true).open(&selected).unwrap();
                file.write_all(b"mutated!").unwrap();
                file.sync_all().unwrap();
            },
        );
        assert_eq!(
            result.unwrap_err(),
            "reference evidence archive changed during admission"
        );
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
        let first_wire = decoded_page(&first, 3, 3);
        let expected_first = expected_page(&first_wire);
        let expected_evidence = first_wire.evidence.clone();
        let selected =
            validate_dispatch_page_v1(&first_wire, &expected_first, &expected_evidence).unwrap();

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
        let second_wire = decoded_page(&second, 4, 4);
        let expected_second = expected_page(&second_wire);
        let second_selected =
            validate_dispatch_page_v1(&second_wire, &expected_second, &expected_evidence).unwrap();
        assert_ne!(selected, second_selected);
        assert!(
            validate_dispatch_page_v1(&first_wire, &expected_second, &expected_evidence).is_err()
        );

        for (pointer, replacement) in [
            (
                "/value/page/context/bundle_identity/digest",
                json!("5555555555555555555555555555555555555555555555555555555555555555"),
            ),
            ("/value/page/context/dispatch_count", json!(3)),
            (
                "/value/page/next_cursor/query_binding",
                json!("6666666666666666666666666666666666666666666666666666666666666666"),
            ),
            ("/value/page/next_cursor/position", json!(2)),
            (
                "/value/page/items/0/dispatch/identity",
                json!(second_dispatch),
            ),
        ] {
            let mut hostile = first.clone();
            *hostile.pointer_mut(pointer).unwrap() = replacement;
            let hostile = decoded_page(&hostile, 3, 3);
            assert!(
                validate_dispatch_page_v1(&hostile, &expected_first, &expected_evidence).is_err()
            );
        }

        let mut extra_evidence = first.clone();
        extra_evidence["value"]["evidence"]["records"] = json!([first_dispatch]);
        let extra_evidence = decoded_page(&extra_evidence, 3, 3);
        assert!(
            validate_dispatch_page_v1(&extra_evidence, &expected_first, &expected_evidence)
                .is_err()
        );

        for (pointer, replacement) in [
            ("/value/page/context/run_identity", json!(0)),
            (
                "/value/page/context/run_identity",
                json!("AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA"),
            ),
            (
                "/value/page/context/run_identity",
                json!("0000000000000000000000000000000000000000000000000000000000000000"),
            ),
        ] {
            let mut hostile = first.clone();
            *hostile.pointer_mut(pointer).unwrap() = replacement;
            let mut bytes = serde_json::to_vec(&hostile).unwrap();
            bytes.push(b'\n');
            assert!(
                validate_agent_response_line_v1(&bytes, 3, 3, ExpectedAgentResultV1::Page).is_err()
            );
        }

        let mut unknown_evidence = first;
        unknown_evidence["value"]["evidence"]["unexpected"] = json!(true);
        let mut bytes = serde_json::to_vec(&unknown_evidence).unwrap();
        bytes.push(b'\n');
        assert!(
            validate_agent_response_line_v1(&bytes, 3, 3, ExpectedAgentResultV1::Page).is_err()
        );
    }
}
