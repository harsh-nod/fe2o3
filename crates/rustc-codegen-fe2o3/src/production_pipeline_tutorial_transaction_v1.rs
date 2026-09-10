//! Compiler-owned assembly of one tutorial production qualification transaction.
//!
//! This boundary accepts only a completed native V5 transaction and exact evidence bytes. The
//! producer drives the exact requested fixture through protected Cargo, then recovers its durable
//! result carrier; an extraction-only Bundle V8 is never promoted to production evidence.

use std::collections::{BTreeMap, BTreeSet};
use std::ffi::CString;
use std::fmt;
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::os::fd::OwnedFd;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt};
use std::path::{Component, Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};

use fe2o3_artifact_transaction::{
    RetainedDurableDirectoryV1, authenticated_compiler_capability_evidence_identity_v5,
};
use fe2o3_compiler_ffi::InertProductionCapabilityResultV5;
use fe2o3_kernel_ir::{VerifiedCanonicalKernelIrV13, VerifiedSimulationBundleV8};
use fe2o3_runtime_protocol::{
    MAX_WORKER_V3_CAPABILITY_RESULT_CARRIER_BYTES_V1, RecoveredWorkerV3CapabilityResultCarrierV1,
    RecoveredWorkerV3LoadEnvelopeV2, WorkerV3CapabilityAncillaryEvidenceV1,
    WorkerV3CapabilityResultCarrierWireV1,
    recover_worker_v3_capability_ancillary_evidence_for_recovered_v1,
    recover_worker_v3_capability_result_carrier_v1, recover_worker_v3_load_envelope_v2,
};
use serde::de::{self, Deserialize, Deserializer, MapAccess, SeqAccess, Visitor};
use serde_json::{Map, Number, Value};
use sha2::{Digest, Sha256};

const REQUEST_SCHEMA: &str = "fe2o3-tutorial-production-transaction-request-v1";
const PIPELINE_ENTRY: &str = "rustc-codegen-fe2o3::production_pipeline";
const ROADMAP_ISSUE: &str = "https://github.com/harsh-nod/fe2o3/issues/272";
const MANIFEST_PATH: &str = "config/tutorial-kernel-manifest-v1.json";
const OBJECT_PREFIX: &str = "objects/sha256";
const REQUEST_DOMAIN: &[u8] = b"fe2o3-tutorial-production-transaction-request-v1\0";
const RECORD_DOMAIN: &[u8] = b"fe2o3-tutorial-capability-record-v1\0";
const SOURCE_CLOSURE_DOMAIN: &[u8] = b"fe2o3-tutorial-package-rust-source-closure-v1\0";
const NEGATIVE_FIXTURE_DOMAIN: &[u8] = b"fe2o3-tutorial-capability-negative-fixtures-v1\0";
const COMMAND_DOMAIN: &[u8] = b"fe2o3-tutorial-semantic-command-v1\0";
const CORPUS_DOMAIN: &[u8] = b"fe2o3-tutorial-kernel-corpus-contract-v1\0";
const FIXTURE_INPUT_DOMAIN: &[u8] = b"fe2o3-tutorial-fixture-compiler-input-v1\0";
const MAX_JSON_BYTES: u64 = 64 * 1024 * 1024;
const MAX_EVIDENCE_BYTES: usize = 2 * 1024 * 1024 * 1024;
const MAX_SOURCE_FILES: usize = 4096;
const MAX_SOURCE_BYTES: usize = 64 * 1024 * 1024;
const MAX_PROTECTED_BUILD_STDERR_BYTES: u64 = 8 * 1024 * 1024;
const CAPABILITY_CARRIER_PREFIX: &str = ".fe2o3-worker-v3-capability-result-v1-";
const CAPABILITY_CARRIER_SUFFIX: &str = ".carrier";
const RELEASE_ENVIRONMENT: &[&str] = &[
    "CARGO",
    "FE2O3_AUTHORITY_BACKEND_SHA256_V1",
    "FE2O3_AUTHORITY_CARGO_SHA256_V1",
    "FE2O3_AUTHORITY_CARGO_BINDING_TRAMPOLINE_PATH_V1",
    "FE2O3_AUTHORITY_CARGO_BINDING_TRAMPOLINE_SHA256_V1",
    "FE2O3_AUTHORITY_RUSTC_PATH_V1",
    "FE2O3_AUTHORITY_RUSTC_RUNTIME_SHA256_V1",
    "FE2O3_AUTHORITY_RUSTC_SHA256_V1",
    "FE2O3_BACKEND",
    "FE2O3_PRODUCTION_BUILD_CONFIG_V1",
    "FE2O3_PRODUCTION_BUILD_CONFIG_V2",
];

const REQUEST_KEYS: &[&str] = &[
    "candidate",
    "capabilityKernel",
    "fixture",
    "hardwareReceiptChallenge",
    "manifest",
    "productionTransaction",
    "requestBindingSha256",
    "roadmapIssue",
    "schema",
    "simulatorEvidence",
];
const RECORD_KEYS: &[&str] = &[
    "capabilityClosure",
    "compilerInput",
    "evidenceFiles",
    "fixtureId",
    "graph",
    "hardware",
    "kernelSymbol",
    "lessonIds",
    "negativeFixtures",
    "productionEvidence",
    "productionTransaction",
    "proof",
    "recordBindingSha256",
    "simulator",
    "target",
    "targetDecision",
];
const EVIDENCE_KINDS: &[&str] = &[
    "artifact",
    "artifact-inspection",
    "capability-analysis",
    "capability-closure",
    "compiler-input",
    "compiler-policy",
    "driver-identity",
    "hardware",
    "host-admission",
    "launch-contract",
    "llvm-module",
    "lowering",
    "machine-refinement",
    "negative-fixture-set",
    "numerical-policy",
    "optimized-kir-v13",
    "proof-checker",
    "proof-evidence",
    "proof-obligation-set",
    "runtime-identity",
    "sealed-production-receipt",
    "semantic-mir",
    "simulation-bundle-v8",
    "simulator",
    "source-closure",
    "source-mir-to-kir-refinement",
    "target-capability-decision",
    "target-identity",
];
const LOCAL_EVIDENCE_KINDS: &[&str] = &[
    "artifact",
    "artifact-inspection",
    "capability-analysis",
    "capability-closure",
    "compiler-input",
    "compiler-policy",
    "driver-identity",
    "hardware",
    "host-admission",
    "launch-contract",
    "llvm-module",
    "lowering",
    "machine-refinement",
    "negative-fixture-set",
    "numerical-policy",
    "optimized-kir-v13",
    "proof-checker",
    "proof-evidence",
    "proof-obligation-set",
    "runtime-identity",
    "sealed-production-receipt",
    "semantic-mir",
    "simulation-bundle-v8",
    "source-closure",
    "source-mir-to-kir-refinement",
    "target-capability-decision",
    "target-identity",
];
const PRODUCTION_EVIDENCE_JOINS: &[(&str, &str)] = &[
    ("artifactSha256", "artifact"),
    ("artifactInspectionSha256", "artifact-inspection"),
    ("capabilityAnalysisSha256", "capability-analysis"),
    ("capabilityClosureSha256", "capability-closure"),
    ("compilerPolicySha256", "compiler-policy"),
    ("hardwareEvidenceSha256", "hardware"),
    ("hostAdmissionSha256", "host-admission"),
    ("launchContractSha256", "launch-contract"),
    ("negativeFixtureSetSha256", "negative-fixture-set"),
    ("numericalPolicySha256", "numerical-policy"),
    ("simulatorEvidenceSha256", "simulator"),
    (
        "targetCapabilityDecisionSha256",
        "target-capability-decision",
    ),
    ("targetIdentitySha256", "target-identity"),
];
const PRODUCTION_EVIDENCE_KEYS: &[&str] = &[
    "artifactSha256",
    "artifactInspectionSha256",
    "capabilityAnalysisSha256",
    "capabilityClosureSha256",
    "compilerCommit",
    "compilerPolicySha256",
    "compilerTree",
    "finalOptimizedKirSha256",
    "hardwareEvidenceSha256",
    "hostAdmissionSha256",
    "launchContractSha256",
    "loweringIdentitySha256",
    "machineRefinementSha256",
    "negativeFixtureSetSha256",
    "numericalPolicySha256",
    "proofCheckerSha256",
    "proofEvidenceSha256",
    "proofObligationSetSha256",
    "simulatorEvidenceSha256",
    "sourceMirIdentitySha256",
    "sourceMirToKirRefinementSha256",
    "targetCapabilityDecisionSha256",
    "targetIdentitySha256",
];
const GRAPH_KEYS: &[&str] = &[
    "bundleContentIdentitySha256",
    "bundleSubjectIdentitySha256",
    "canonicalKirBytes",
    "canonicalKirVersion",
    "finalGraphEpoch",
    "kernelAbiIdentitySha256",
    "kernelCount",
    "productionKirIdentitySha256",
    "semanticMirIdentitySha256",
    "sourceInventoryReceiptSha256",
    "sourcePreflightReceiptSha256",
];
const PRODUCTION_PROOF_PROPERTIES: &[&str] = &[
    "capability-provenance",
    "functional-refinement",
    "machine-refinement",
    "source-mir-kir-refinement",
];

static STAGING_SEQUENCE: AtomicU64 = AtomicU64::new(0);

/// Stable machine-readable failure classes for the compiler-owned producer.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum TutorialProductionTransactionErrorCodeV1 {
    RequestIo,
    NonCanonicalJson,
    RequestSchema,
    RequestBinding,
    SourceMismatch,
    WrongTarget,
    ProtectedCompletionUnavailable,
    InvalidProductionResult,
    ArtifactMismatch,
    MissingEvidence,
    EvidenceMismatch,
    HardwareReceipt,
    OutputPath,
    Publication,
}

impl TutorialProductionTransactionErrorCodeV1 {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::RequestIo => "FE2O3-TUTORIAL-TXN-001",
            Self::NonCanonicalJson => "FE2O3-TUTORIAL-TXN-002",
            Self::RequestSchema => "FE2O3-TUTORIAL-TXN-003",
            Self::RequestBinding => "FE2O3-TUTORIAL-TXN-004",
            Self::SourceMismatch => "FE2O3-TUTORIAL-TXN-005",
            Self::WrongTarget => "FE2O3-TUTORIAL-TXN-006",
            Self::ProtectedCompletionUnavailable => "FE2O3-TUTORIAL-TXN-007",
            Self::InvalidProductionResult => "FE2O3-TUTORIAL-TXN-008",
            Self::ArtifactMismatch => "FE2O3-TUTORIAL-TXN-009",
            Self::MissingEvidence => "FE2O3-TUTORIAL-TXN-010",
            Self::EvidenceMismatch => "FE2O3-TUTORIAL-TXN-011",
            Self::HardwareReceipt => "FE2O3-TUTORIAL-TXN-012",
            Self::OutputPath => "FE2O3-TUTORIAL-TXN-013",
            Self::Publication => "FE2O3-TUTORIAL-TXN-014",
        }
    }
}

/// Fail-closed diagnostic returned before any output directory is published.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TutorialProductionTransactionErrorV1 {
    code: TutorialProductionTransactionErrorCodeV1,
    message: String,
}

impl TutorialProductionTransactionErrorV1 {
    fn new(code: TutorialProductionTransactionErrorCodeV1, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }

    pub const fn code(&self) -> TutorialProductionTransactionErrorCodeV1 {
        self.code
    }

    pub fn message(&self) -> &str {
        &self.message
    }
}

impl fmt::Display for TutorialProductionTransactionErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}: {}", self.code.as_str(), self.message)
    }
}

impl std::error::Error for TutorialProductionTransactionErrorV1 {}

type ResultV1<T> = Result<T, TutorialProductionTransactionErrorV1>;

/// Identity of one atomically published transaction export.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TutorialProductionTransactionReceiptV1 {
    fixture_id: String,
    request_binding_sha256: String,
    transaction_sha256: String,
    export_sha256: String,
    output_directory: PathBuf,
}

impl TutorialProductionTransactionReceiptV1 {
    pub fn fixture_id(&self) -> &str {
        &self.fixture_id
    }

    pub fn request_binding_sha256(&self) -> &str {
        &self.request_binding_sha256
    }

    pub fn transaction_sha256(&self) -> &str {
        &self.transaction_sha256
    }

    pub fn export_sha256(&self) -> &str {
        &self.export_sha256
    }

    pub fn output_directory(&self) -> &Path {
        &self.output_directory
    }
}

#[derive(Debug)]
struct RequestContext {
    document: Value,
    canonical_bytes: Vec<u8>,
    fixture_id: String,
    target: String,
    kernel_symbol: String,
    request_binding_sha256: String,
    source_closure_preimage: Vec<u8>,
    simulator_command_sha256: String,
    hardware_command_sha256: String,
    hardware_lane: String,
    hardware_timeout_seconds: u64,
    repository: PathBuf,
}

/// Prepares one transaction through the sole protected Cargo/rustc entrypoint.
pub fn prepare_tutorial_capability_qualification_transaction_v1(
    cargo_fe2o3: &Path,
    request: &Path,
    output_directory: &Path,
) -> ResultV1<TutorialProductionTransactionReceiptV1> {
    let repository = std::env::current_dir().map_err(|error| {
        TutorialProductionTransactionErrorV1::new(
            TutorialProductionTransactionErrorCodeV1::RequestIo,
            format!("cannot identify the compiler repository: {error}"),
        )
    })?;
    let repository = real_directory(&repository, "compiler repository")?;
    let request_bytes = read_regular(request, MAX_JSON_BYTES, "transaction request")?;
    let context = preflight_request(&repository, request_bytes, true)?;
    require_external_output_path(&repository, output_directory)?;
    let build = run_protected_fixture_build_v1(
        cargo_fe2o3,
        &context.repository,
        &context.target,
        &context.document,
        output_directory
            .parent()
            .expect("the external output path has a checked parent"),
    )?;
    let recovered = recover_protected_fixture_result_v1(&build.output_root)?;
    recovered.revalidate_currentness()?;
    assemble_pre_hardware_transaction_v1(&context, &recovered, output_directory)
}

#[derive(Debug)]
struct ProtectedFixtureBuildV1 {
    _staging: ScopedBuildDirectoryV1,
    output_root: PathBuf,
}

#[derive(Debug)]
struct ScopedBuildDirectoryV1(PathBuf);

impl Drop for ScopedBuildDirectoryV1 {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

struct RecoveredProtectedFixtureResultV1 {
    directory: RetainedDurableDirectoryV1,
    envelope: RecoveredWorkerV3LoadEnvelopeV2,
    carrier: RecoveredWorkerV3CapabilityResultCarrierV1,
    ancillary: WorkerV3CapabilityAncillaryEvidenceV1,
}

impl RecoveredProtectedFixtureResultV1 {
    fn production_result(&self) -> &InertProductionCapabilityResultV5 {
        self.carrier.production_result()
    }

    fn revalidate_currentness(&self) -> ResultV1<()> {
        let current_carrier =
            recover_worker_v3_capability_result_carrier_v1(&self.directory, &self.envelope)
                .map_err(|error| {
                    TutorialProductionTransactionErrorV1::new(
                        TutorialProductionTransactionErrorCodeV1::InvalidProductionResult,
                        format!("native V5 capability carrier is no longer current: {error}"),
                    )
                })?;
        if current_carrier.identity() != self.carrier.identity()
            || current_carrier.production_result().canonical_bytes()
                != self.carrier.production_result().canonical_bytes()
        {
            return fail(
                TutorialProductionTransactionErrorCodeV1::InvalidProductionResult,
                "native V5 capability carrier changed between recovery and transaction admission",
            );
        }
        let current_ancillary = recover_worker_v3_capability_ancillary_evidence_for_recovered_v1(
            &self.directory,
            &self.envelope,
            current_carrier.production_result(),
        )
        .map_err(|error| {
            TutorialProductionTransactionErrorV1::new(
                TutorialProductionTransactionErrorCodeV1::InvalidProductionResult,
                format!("native V5 ancillary evidence is no longer current: {error}"),
            )
        })?;
        let retained_checker = self
            .ancillary
            .checker_evidence_identity()
            .map_err(|error| {
                TutorialProductionTransactionErrorV1::new(
                    TutorialProductionTransactionErrorCodeV1::InvalidProductionResult,
                    format!("retained native V5 checker identity is invalid: {error}"),
                )
            })?;
        let current_checker = current_ancillary
            .checker_evidence_identity()
            .map_err(|error| {
                TutorialProductionTransactionErrorV1::new(
                    TutorialProductionTransactionErrorCodeV1::InvalidProductionResult,
                    format!("current native V5 checker identity is invalid: {error}"),
                )
            })?;
        if current_ancillary.canonical_bytes() != self.ancillary.canonical_bytes()
            || current_checker != retained_checker
            || current_ancillary.checker_evidence_bytes() != self.ancillary.checker_evidence_bytes()
            || current_ancillary.object_bytes() != self.ancillary.object_bytes()
            || current_ancillary.production_result_bytes()
                != self.ancillary.production_result_bytes()
        {
            return fail(
                TutorialProductionTransactionErrorCodeV1::InvalidProductionResult,
                "native V5 ancillary evidence changed between recovery and transaction admission",
            );
        }
        Ok(())
    }
}

fn run_protected_fixture_build_v1(
    cargo_fe2o3: &Path,
    repository: &Path,
    requested_target: &str,
    request_document: &Value,
    staging_parent: &Path,
) -> ResultV1<ProtectedFixtureBuildV1> {
    let cargo_fe2o3 = exact_executable(cargo_fe2o3, "cargo-fe2o3 production executable")?;
    let request = object(request_document, "request")?;
    let fixture = object(required(request, "fixture", "request")?, "request.fixture")?;
    let input = object(
        required(fixture, "compilerInput", "request.fixture")?,
        "request.fixture.compilerInput",
    )?;
    let manifest = protected_relative(
        repository,
        required(input, "packageManifest", "request.fixture.compilerInput")?
            .as_str()
            .ok_or_else(|| {
                TutorialProductionTransactionErrorV1::new(
                    TutorialProductionTransactionErrorCodeV1::RequestSchema,
                    "request.fixture.compilerInput.packageManifest must be a string",
                )
            })?,
        "package manifest",
    )?;
    let target = object(
        required(input, "cargoTarget", "request.fixture.compilerInput")?,
        "request.fixture.compilerInput.cargoTarget",
    )?;
    if string_field(
        &Value::Object(target.clone()),
        "kind",
        "request.fixture.compilerInput.cargoTarget",
    )? != "lib"
    {
        return fail(
            TutorialProductionTransactionErrorCodeV1::RequestSchema,
            "the tutorial production lifecycle currently requires an exact Cargo library target",
        );
    }
    let features = required(input, "features", "request.fixture.compilerInput")?
        .as_array()
        .ok_or_else(|| {
            TutorialProductionTransactionErrorV1::new(
                TutorialProductionTransactionErrorCodeV1::RequestSchema,
                "request.fixture.compilerInput.features must be an array",
            )
        })?;
    let mut feature_names = Vec::with_capacity(features.len());
    for feature in features {
        let feature = feature.as_str().ok_or_else(|| {
            TutorialProductionTransactionErrorV1::new(
                TutorialProductionTransactionErrorCodeV1::RequestSchema,
                "request.fixture.compilerInput.features must contain strings",
            )
        })?;
        if feature.is_empty() || feature.contains(',') {
            return fail(
                TutorialProductionTransactionErrorCodeV1::RequestSchema,
                "request.fixture.compilerInput.features contains an invalid Cargo feature",
            );
        }
        feature_names.push(feature);
    }

    let sequence = STAGING_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let staging = staging_parent.join(format!(
        ".fe2o3-tutorial-protected-build-{}-{sequence}",
        std::process::id()
    ));
    let mut builder = fs::DirBuilder::new();
    builder.mode(0o700);
    builder.create(&staging).map_err(|error| {
        TutorialProductionTransactionErrorV1::new(
            TutorialProductionTransactionErrorCodeV1::OutputPath,
            format!("cannot create protected-build staging directory: {error}"),
        )
    })?;
    let staging_guard = ScopedBuildDirectoryV1(staging.clone());
    let target_directory = staging.join("target");
    let stderr_path = staging.join("cargo-fe2o3.stderr");
    let stderr = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .custom_flags(libc::O_CLOEXEC | libc::O_NOFOLLOW)
        .open(&stderr_path)
        .map_err(|error| io_error("cannot create protected-build stderr", error))?;

    let mut command = Command::new(cargo_fe2o3);
    command
        .env_clear()
        .env("LANG", "C")
        .env("LC_ALL", "C")
        .env("TZ", "UTC")
        .env("FE2O3_TARGET", requested_target)
        .current_dir(repository)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::from(stderr))
        .args([
            "authority",
            "release",
            "build",
            "--locked",
            "--manifest-path",
        ])
        .arg(&manifest)
        .arg("--target-dir")
        .arg(&target_directory)
        .arg("--lib");
    if input.get("defaultFeatures").and_then(Value::as_bool) == Some(false) {
        command.arg("--no-default-features");
    }
    if !feature_names.is_empty() {
        command.arg("--features").arg(feature_names.join(","));
    }
    for name in RELEASE_ENVIRONMENT {
        if let Some(value) = std::env::var_os(name) {
            command.env(name, value);
        }
    }
    let status = command.status().map_err(|error| {
        TutorialProductionTransactionErrorV1::new(
            TutorialProductionTransactionErrorCodeV1::ProtectedCompletionUnavailable,
            format!("cannot start protected cargo-fe2o3 fixture build: {error}"),
        )
    })?;
    let stderr = read_bounded_optional_file(
        &stderr_path,
        MAX_PROTECTED_BUILD_STDERR_BYTES,
        "protected cargo-fe2o3 stderr",
    )?;
    if !status.success() {
        let detail = String::from_utf8_lossy(&stderr);
        let detail = detail.trim();
        return fail(
            TutorialProductionTransactionErrorCodeV1::ProtectedCompletionUnavailable,
            format!(
                "protected cargo-fe2o3 fixture build failed with {status}{}",
                if detail.is_empty() {
                    String::new()
                } else {
                    format!(": {detail}")
                }
            ),
        );
    }
    Ok(ProtectedFixtureBuildV1 {
        _staging: staging_guard,
        output_root: target_directory.join("fe2o3"),
    })
}

fn recover_protected_fixture_result_v1(
    output_root: &Path,
) -> ResultV1<RecoveredProtectedFixtureResultV1> {
    let directory = real_directory(output_root, "protected build output root")?;
    let mut carrier_paths = Vec::new();
    for entry in fs::read_dir(&directory)
        .map_err(|error| io_error("cannot enumerate protected build output root", error))?
    {
        let entry = entry
            .map_err(|error| io_error("cannot inspect protected build output entry", error))?;
        let name = entry.file_name();
        let Some(name) = name.to_str() else {
            continue;
        };
        if name.starts_with(CAPABILITY_CARRIER_PREFIX) && name.ends_with(CAPABILITY_CARRIER_SUFFIX)
        {
            carrier_paths.push(entry.path());
        }
    }
    if carrier_paths.len() != 1 {
        return fail(
            TutorialProductionTransactionErrorCodeV1::InvalidProductionResult,
            format!(
                "protected build must publish exactly one native V5 capability carrier; found {}",
                carrier_paths.len()
            ),
        );
    }
    let carrier_bytes = read_regular(
        &carrier_paths[0],
        MAX_WORKER_V3_CAPABILITY_RESULT_CARRIER_BYTES_V1 as u64,
        "native V5 capability carrier",
    )?;
    let wire = WorkerV3CapabilityResultCarrierWireV1::decode_canonical(&carrier_bytes).map_err(
        |error| {
            TutorialProductionTransactionErrorV1::new(
                TutorialProductionTransactionErrorCodeV1::InvalidProductionResult,
                format!("native V5 capability carrier failed typed decoding: {error}"),
            )
        },
    )?;
    let envelope =
        recover_worker_v3_load_envelope_v2(&directory, wire.attempt()).map_err(|error| {
            TutorialProductionTransactionErrorV1::new(
                TutorialProductionTransactionErrorCodeV1::InvalidProductionResult,
                format!("native V5 load-envelope recovery failed: {error}"),
            )
        })?;
    let retained = RetainedDurableDirectoryV1::admit_service_owned(OwnedFd::from(
        File::open(&directory)
            .map_err(|error| io_error("cannot retain protected build output root", error))?,
    ))
    .map_err(|error| {
        TutorialProductionTransactionErrorV1::new(
            TutorialProductionTransactionErrorCodeV1::InvalidProductionResult,
            format!("native V5 durable-root admission failed: {error}"),
        )
    })?;
    let carrier =
        recover_worker_v3_capability_result_carrier_v1(&retained, &envelope).map_err(|error| {
            TutorialProductionTransactionErrorV1::new(
                TutorialProductionTransactionErrorCodeV1::InvalidProductionResult,
                format!("native V5 capability-carrier recovery failed: {error}"),
            )
        })?;
    let ancillary = recover_worker_v3_capability_ancillary_evidence_for_recovered_v1(
        &retained,
        &envelope,
        carrier.production_result(),
    )
    .map_err(|error| {
        TutorialProductionTransactionErrorV1::new(
            TutorialProductionTransactionErrorCodeV1::InvalidProductionResult,
            format!("native V5 ancillary-evidence recovery failed: {error}"),
        )
    })?;
    Ok(RecoveredProtectedFixtureResultV1 {
        directory: retained,
        envelope,
        carrier,
        ancillary,
    })
}

fn exact_executable(path: &Path, label: &str) -> ResultV1<PathBuf> {
    if !path.is_absolute() {
        return fail(
            TutorialProductionTransactionErrorCodeV1::RequestIo,
            format!("{label} must be absolute"),
        );
    }
    let metadata = fs::symlink_metadata(path).map_err(|error| io_error(label, error))?;
    let canonical = path
        .canonicalize()
        .map_err(|error| io_error(label, error))?;
    if canonical != path
        || metadata.file_type().is_symlink()
        || !metadata.file_type().is_file()
        || metadata.mode() & 0o111 == 0
    {
        return fail(
            TutorialProductionTransactionErrorCodeV1::RequestIo,
            format!("{label} must be an exact executable regular non-symlink file"),
        );
    }
    Ok(canonical)
}

fn read_bounded_optional_file(path: &Path, maximum: u64, label: &str) -> ResultV1<Vec<u8>> {
    let mut file = File::open(path).map_err(|error| io_error(label, error))?;
    let length = file
        .metadata()
        .map_err(|error| io_error(label, error))?
        .len();
    if length > maximum {
        return fail(
            TutorialProductionTransactionErrorCodeV1::ProtectedCompletionUnavailable,
            format!("{label} exceeds its byte bound"),
        );
    }
    file.seek(SeekFrom::Start(0))
        .map_err(|error| io_error(label, error))?;
    let mut bytes = Vec::with_capacity(length as usize);
    file.read_to_end(&mut bytes)
        .map_err(|error| io_error(label, error))?;
    Ok(bytes)
}

#[path = "production_pipeline_tutorial_transaction_negative_v1.rs"]
mod negative_fixture;

include!("production_pipeline_tutorial_transaction_request_v1.rs");
include!("production_pipeline_tutorial_transaction_evidence_v1.rs");
include!("production_pipeline_tutorial_transaction_pre_hardware_v1.rs");
include!("production_pipeline_tutorial_transaction_io_v1.rs");
#[cfg(test)]
include!("production_pipeline_tutorial_transaction_tests_v1.rs");
