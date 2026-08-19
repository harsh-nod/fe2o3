//! Authenticated schedule-model proof evidence for issue #138 general GEMM.
//!
//! This module executes one digest-pinned Verus binary over embedded, exact
//! proof sources. It returns property-local evidence for the schedule model;
//! it does not promote that evidence across the source-to-KIR correspondence
//! or the post-artifact machine-refinement boundary.

use std::{
    fmt, fs,
    path::{Path, PathBuf},
    process::{Command, ExitStatus, Stdio},
    sync::atomic::{AtomicU64, Ordering},
    thread,
    time::{Duration, Instant},
};

use sha2::{Digest as _, Sha256};

/// Pinned Verus release used by the issue #138 schedule-model proofs.
pub const GENERAL_GEMM_VERUS_VERSION_V1: &str = "0.2026.08.02.b677dd5";
/// SHA-256 of the exact pinned Verus launcher executable.
pub const GENERAL_GEMM_VERUS_SHA256_V1: [u8; 32] = [
    0xad, 0x26, 0x69, 0xf5, 0x79, 0xd8, 0x98, 0xed, 0xe5, 0x3f, 0x2b, 0xf8, 0x4e, 0x80, 0xa1, 0xda,
    0xf4, 0xe3, 0x57, 0x87, 0x39, 0xb0, 0xf5, 0x80, 0x7e, 0xf2, 0x09, 0xa0, 0xc9, 0xf3, 0x82, 0xdd,
];
/// Maximum accepted output from one proof process.
pub const MAX_GENERAL_GEMM_PROOF_OUTPUT_BYTES_V1: usize = 1024 * 1024;
/// Maximum accepted proof deadline.
pub const MAX_GENERAL_GEMM_PROOF_TIMEOUT_SECONDS_V1: u32 = 300;

const PROOF_IDENTITY_DOMAIN_V1: &[u8] = b"fe2o3-general-gemm-schedule-proof-v1\0";
const PROPERTY_IDENTITY_DOMAIN_V1: &[u8] = b"fe2o3-general-gemm-property-evidence-v1\0";
const SOURCE_CLOSURE_DOMAIN_V1: &[u8] = b"fe2o3-general-gemm-proof-source-closure-v1\0";
const EXECUTION_OUTPUT_DOMAIN_V1: &[u8] = b"fe2o3-general-gemm-proof-output-v1\0";

const MODEL_SOURCE: &[u8] = include_bytes!("../verus/general_gemm_schedule_model_v1.rs");
const REFERENCE_SOURCE: &[u8] = include_bytes!("../verus/general_gemm_reference_schedule_v1.rs");
const VECTORIZED_SOURCE: &[u8] = include_bytes!("../verus/general_gemm_vectorized_schedule_v1.rs");
const VECTOR_TAIL_WRONG_SOURCE: &[u8] =
    include_bytes!("../verus/negative/general_gemm_vector_tail_wrong.rs");
const EPILOGUE_WRONG_SOURCE: &[u8] =
    include_bytes!("../verus/negative/general_gemm_epilogue_wrong.rs");
const MACHINE_CLAIM_WRONG_SOURCE: &[u8] =
    include_bytes!("../verus/negative/general_gemm_machine_claim_wrong.rs");

/// A raw SHA-256 domain identity. Construction authenticates no producer.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(transparent)]
pub struct GeneralGemmEvidenceIdentityV1([u8; 32]);

impl GeneralGemmEvidenceIdentityV1 {
    /// Wraps caller-observed identity bytes without granting authority.
    pub const fn from_untrusted_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    /// Returns the exact identity bytes.
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    const fn is_valid(self) -> bool {
        let mut index = 0;
        while index < self.0.len() {
            if self.0[index] != 0 {
                return true;
            }
            index += 1;
        }
        false
    }
}

/// Schedule model instantiated by one independent Verus execution.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(u8)]
pub enum GeneralGemmProofScheduleV1 {
    /// Scalar masked global transfers and single-buffered XOR4 LDS.
    ReferenceWave64Xor4V1 = 1,
    /// Aligned full-vector A transfers with scalar A-tail fallback and scalar B.
    VectorizedAOnlyBf16GlobalTransferV1 = 2,
}

impl GeneralGemmProofScheduleV1 {
    const fn positive_source(self) -> (&'static str, &'static [u8], usize) {
        match self {
            Self::ReferenceWave64Xor4V1 => (
                "general_gemm_reference_schedule_v1.rs",
                REFERENCE_SOURCE,
                28,
            ),
            Self::VectorizedAOnlyBf16GlobalTransferV1 => (
                "general_gemm_vectorized_schedule_v1.rs",
                VECTORIZED_SOURCE,
                28,
            ),
        }
    }
}

/// Independent proof-required general GEMM properties.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(u8)]
pub enum GeneralGemmProofPropertyV1 {
    /// Valid allocation and region provenance in the schedule model.
    MemorySafe = 1,
    /// Guarded accesses are in bounds in the schedule model.
    BoundsSafe = 2,
    /// Staged values are defined before model reads.
    Initialized = 3,
    /// Model effect ownership excludes conflicting writes.
    RaceFree = 4,
    /// Modeled barrier participation is uniform.
    BarrierConvergent = 5,
    /// Modeled C ownership is injective.
    OutputRegionInjective = 6,
    /// Modeled LDS publish/read/reuse epochs are ordered.
    LdsEpochCorrect = 7,
    /// Modeled accumulators preserve every K prefix.
    AccumulatorPhaseRefinement = 8,
    /// Modeled masked tails stage zero and suppress invalid accesses.
    TailRefinement = 9,
    /// Modeled epilogue is `alpha * accumulator + beta * C`.
    EpilogueRefinement = 10,
    /// Schedule operation order refines the declared exact-value recurrence.
    NumericalContract = 11,
    /// Exact emitted machine boundary, unavailable before an artifact exists.
    MachineRefinementBoundary = 12,
}

/// Fixed independent property order used by proof evidence.
pub const GENERAL_GEMM_PROOF_PROPERTIES_V1: [GeneralGemmProofPropertyV1; 12] = [
    GeneralGemmProofPropertyV1::MemorySafe,
    GeneralGemmProofPropertyV1::BoundsSafe,
    GeneralGemmProofPropertyV1::Initialized,
    GeneralGemmProofPropertyV1::RaceFree,
    GeneralGemmProofPropertyV1::BarrierConvergent,
    GeneralGemmProofPropertyV1::OutputRegionInjective,
    GeneralGemmProofPropertyV1::LdsEpochCorrect,
    GeneralGemmProofPropertyV1::AccumulatorPhaseRefinement,
    GeneralGemmProofPropertyV1::TailRefinement,
    GeneralGemmProofPropertyV1::EpilogueRefinement,
    GeneralGemmProofPropertyV1::NumericalContract,
    GeneralGemmProofPropertyV1::MachineRefinementBoundary,
];

/// Exact identities supplied by compiler and planner integration.
///
/// These values become proof-evidence inputs. Their producers remain
/// responsible for deriving them from authenticated source, KIR, and compiler
/// state and for matching them before consuming any later admission.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GeneralGemmProofRequestV1 {
    schedule: GeneralGemmProofScheduleV1,
    schedule_identity: GeneralGemmEvidenceIdentityV1,
    plan_identity: GeneralGemmEvidenceIdentityV1,
    kir_identity: GeneralGemmEvidenceIdentityV1,
    compilation_binding_identity: GeneralGemmEvidenceIdentityV1,
    compile_request_identity: GeneralGemmEvidenceIdentityV1,
    obligation_set_identity: GeneralGemmEvidenceIdentityV1,
    compiler_identity: GeneralGemmEvidenceIdentityV1,
    target_identity: GeneralGemmEvidenceIdentityV1,
    toolchain_identity: GeneralGemmEvidenceIdentityV1,
    runtime_abi_identity: GeneralGemmEvidenceIdentityV1,
    source_semantics_identity: GeneralGemmEvidenceIdentityV1,
    numerical_policy_identity: GeneralGemmEvidenceIdentityV1,
}

impl GeneralGemmProofRequestV1 {
    /// Checks nonzero, domain-distinct identities for one schedule proof.
    #[allow(clippy::too_many_arguments)]
    pub fn checked(
        schedule: GeneralGemmProofScheduleV1,
        schedule_identity: GeneralGemmEvidenceIdentityV1,
        plan_identity: GeneralGemmEvidenceIdentityV1,
        kir_identity: GeneralGemmEvidenceIdentityV1,
        compilation_binding_identity: GeneralGemmEvidenceIdentityV1,
        compile_request_identity: GeneralGemmEvidenceIdentityV1,
        obligation_set_identity: GeneralGemmEvidenceIdentityV1,
        compiler_identity: GeneralGemmEvidenceIdentityV1,
        target_identity: GeneralGemmEvidenceIdentityV1,
        toolchain_identity: GeneralGemmEvidenceIdentityV1,
        runtime_abi_identity: GeneralGemmEvidenceIdentityV1,
        source_semantics_identity: GeneralGemmEvidenceIdentityV1,
        numerical_policy_identity: GeneralGemmEvidenceIdentityV1,
    ) -> Result<Self, GeneralGemmProofExecutionErrorV1> {
        let identities = [
            schedule_identity,
            plan_identity,
            kir_identity,
            compilation_binding_identity,
            compile_request_identity,
            obligation_set_identity,
            compiler_identity,
            target_identity,
            toolchain_identity,
            runtime_abi_identity,
            source_semantics_identity,
            numerical_policy_identity,
        ];
        if identities.iter().any(|identity| !identity.is_valid()) {
            return Err(GeneralGemmProofExecutionErrorV1::InvalidIdentity);
        }
        if identities
            .iter()
            .enumerate()
            .any(|(index, identity)| identities[..index].contains(identity))
        {
            return Err(GeneralGemmProofExecutionErrorV1::DuplicateIdentity);
        }
        Ok(Self {
            schedule,
            schedule_identity,
            plan_identity,
            kir_identity,
            compilation_binding_identity,
            compile_request_identity,
            obligation_set_identity,
            compiler_identity,
            target_identity,
            toolchain_identity,
            runtime_abi_identity,
            source_semantics_identity,
            numerical_policy_identity,
        })
    }

    /// Returns the independently instantiated schedule model.
    pub const fn schedule(self) -> GeneralGemmProofScheduleV1 {
        self.schedule
    }

    /// Returns the exact schedule identity.
    pub const fn schedule_identity(self) -> GeneralGemmEvidenceIdentityV1 {
        self.schedule_identity
    }

    /// Returns the exact host-plan identity.
    pub const fn plan_identity(self) -> GeneralGemmEvidenceIdentityV1 {
        self.plan_identity
    }

    /// Returns the exact semantic-KIR identity.
    pub const fn kir_identity(self) -> GeneralGemmEvidenceIdentityV1 {
        self.kir_identity
    }

    /// Returns the exact aggregate compilation binding identity.
    pub const fn compilation_binding_identity(self) -> GeneralGemmEvidenceIdentityV1 {
        self.compilation_binding_identity
    }

    /// Returns the exact compiler request identity.
    pub const fn compile_request_identity(self) -> GeneralGemmEvidenceIdentityV1 {
        self.compile_request_identity
    }

    /// Returns the exact required obligation-set identity.
    pub const fn obligation_set_identity(self) -> GeneralGemmEvidenceIdentityV1 {
        self.obligation_set_identity
    }

    /// Returns the exact compiler identity.
    pub const fn compiler_identity(self) -> GeneralGemmEvidenceIdentityV1 {
        self.compiler_identity
    }

    /// Returns the exact target identity.
    pub const fn target_identity(self) -> GeneralGemmEvidenceIdentityV1 {
        self.target_identity
    }

    /// Returns the exact compiler toolchain identity.
    pub const fn toolchain_identity(self) -> GeneralGemmEvidenceIdentityV1 {
        self.toolchain_identity
    }

    /// Returns the exact dynamic runtime ABI identity.
    pub const fn runtime_abi_identity(self) -> GeneralGemmEvidenceIdentityV1 {
        self.runtime_abi_identity
    }

    /// Returns the exact source-semantics identity.
    pub const fn source_semantics_identity(self) -> GeneralGemmEvidenceIdentityV1 {
        self.source_semantics_identity
    }

    /// Returns the exact numerical-policy identity.
    pub const fn numerical_policy_identity(self) -> GeneralGemmEvidenceIdentityV1 {
        self.numerical_policy_identity
    }

    fn identities(self) -> [GeneralGemmEvidenceIdentityV1; 12] {
        [
            self.schedule_identity,
            self.plan_identity,
            self.kir_identity,
            self.compilation_binding_identity,
            self.compile_request_identity,
            self.obligation_set_identity,
            self.compiler_identity,
            self.target_identity,
            self.toolchain_identity,
            self.runtime_abi_identity,
            self.source_semantics_identity,
            self.numerical_policy_identity,
        ]
    }
}

/// Honest authority level of one property result.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum GeneralGemmPropertyEvidenceStatusV1 {
    /// Verus proved the named theorem over the exact embedded schedule model.
    /// Authenticated KIR-to-model correspondence remains required.
    ScheduleModelTheoremVerified,
    /// The model defines the desired fact but does not derive it from source
    /// or KIR control/effect structure.
    ModelDefinitionOnly,
    /// Verus proved exact-real recurrence/epilogue equivalence, but not the
    /// declared BF16 decode and FP32 rounding policy.
    WeakerExactRealTheoremVerified,
    /// The schedule model lacks a theorem for this imported-kernel property.
    OpenCorrespondenceRequired,
    /// No emitted artifact exists to inspect against the model.
    OpenArtifactRequired,
}

/// Exact theorem, definition, or open obligation behind one property result.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum GeneralGemmPropertyEvidenceBasisV1 {
    /// A named theorem in the schedule-specific positive Verus source.
    VerifiedTheorem(&'static str),
    /// A named model definition or ordering fact that still needs import proof.
    ModelDefinition(&'static str),
    /// A named proof obligation not discharged by this model execution.
    OpenObligation(&'static str),
}

/// One independently identified property result.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GeneralGemmPropertyEvidenceV1 {
    property: GeneralGemmProofPropertyV1,
    status: GeneralGemmPropertyEvidenceStatusV1,
    basis: GeneralGemmPropertyEvidenceBasisV1,
    identity: GeneralGemmEvidenceIdentityV1,
}

impl GeneralGemmPropertyEvidenceV1 {
    /// Returns the exact property.
    pub const fn property(self) -> GeneralGemmProofPropertyV1 {
        self.property
    }

    /// Returns the exact authority boundary of this result.
    pub const fn status(self) -> GeneralGemmPropertyEvidenceStatusV1 {
        self.status
    }

    /// Returns the exact theorem, definition, or open obligation name.
    pub const fn basis(self) -> GeneralGemmPropertyEvidenceBasisV1 {
        self.basis
    }

    /// Returns the domain-separated property-evidence identity.
    pub const fn identity(self) -> GeneralGemmEvidenceIdentityV1 {
        self.identity
    }
}

/// Digest and size of one exact proof-process output.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GeneralGemmProofOutputEvidenceV1 {
    identity: GeneralGemmEvidenceIdentityV1,
    stdout_bytes: u64,
    stderr_bytes: u64,
}

impl GeneralGemmProofOutputEvidenceV1 {
    /// Returns the exact status/stdout/stderr identity.
    pub const fn identity(self) -> GeneralGemmEvidenceIdentityV1 {
        self.identity
    }

    /// Returns captured stdout bytes.
    pub const fn stdout_bytes(self) -> u64 {
        self.stdout_bytes
    }

    /// Returns captured stderr bytes.
    pub const fn stderr_bytes(self) -> u64 {
        self.stderr_bytes
    }
}

/// Privately constructed evidence from one real pinned Verus execution.
///
/// This value is intentionally not `Clone`. It grants no compiler proof gate,
/// artifact, publication, load, or launch authority.
#[derive(Debug)]
#[must_use = "schedule proof evidence must be joined to authenticated KIR correspondence"]
pub struct AuthenticatedGeneralGemmScheduleProofV1 {
    request: GeneralGemmProofRequestV1,
    identity: GeneralGemmEvidenceIdentityV1,
    source_closure_identity: GeneralGemmEvidenceIdentityV1,
    tool_identity: GeneralGemmEvidenceIdentityV1,
    positive_output: GeneralGemmProofOutputEvidenceV1,
    negative_outputs: Vec<GeneralGemmProofOutputEvidenceV1>,
    properties: [GeneralGemmPropertyEvidenceV1; 12],
}

impl AuthenticatedGeneralGemmScheduleProofV1 {
    /// Returns the exact proof input bindings.
    pub const fn request(&self) -> GeneralGemmProofRequestV1 {
        self.request
    }

    /// Returns the aggregate schedule-proof identity.
    pub const fn identity(&self) -> GeneralGemmEvidenceIdentityV1 {
        self.identity
    }

    /// Returns the exact embedded source-closure identity.
    pub const fn source_closure_identity(&self) -> GeneralGemmEvidenceIdentityV1 {
        self.source_closure_identity
    }

    /// Returns the pinned Verus launcher identity.
    pub const fn tool_identity(&self) -> GeneralGemmEvidenceIdentityV1 {
        self.tool_identity
    }

    /// Returns the successful schedule-proof output evidence.
    pub const fn positive_output(&self) -> GeneralGemmProofOutputEvidenceV1 {
        self.positive_output
    }

    /// Returns expected-negative proof outputs.
    pub fn negative_outputs(&self) -> &[GeneralGemmProofOutputEvidenceV1] {
        &self.negative_outputs
    }

    /// Returns all 12 independently identified property results.
    pub const fn properties(&self) -> &[GeneralGemmPropertyEvidenceV1; 12] {
        &self.properties
    }

    /// Returns false until KIR correspondence and post-artifact machine
    /// refinement are authenticated by their owning layers.
    pub const fn can_enter_compiler_proof_gate(&self) -> bool {
        false
    }
}

/// Failure while producing bounded schedule-proof evidence.
#[derive(Debug)]
pub enum GeneralGemmProofExecutionErrorV1 {
    /// One required identity was all-zero.
    InvalidIdentity,
    /// Two independently owned identity domains reused the same bytes.
    DuplicateIdentity,
    /// Proof timeout was zero or above the hard bound.
    InvalidTimeout,
    /// The Verus path was not absolute or did not name a regular file.
    InvalidVerusPath,
    /// The pinned Verus bytes changed.
    VerusDigestMismatch,
    /// The pinned Verus version output changed.
    VerusVersionMismatch,
    /// A proof process exceeded the deadline.
    TimedOut,
    /// A proof process emitted too much output.
    OutputTooLarge,
    /// A positive proof did not verify the exact expected obligation count.
    PositiveProofFailed,
    /// An expected-negative proof unexpectedly passed or failed elsewhere.
    NegativeProofMismatch,
    /// Filesystem or process execution failed.
    Io(std::io::Error),
}

impl fmt::Display for GeneralGemmProofExecutionErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "general GEMM proof execution failed: {self:?}")
    }
}

impl std::error::Error for GeneralGemmProofExecutionErrorV1 {}

impl From<std::io::Error> for GeneralGemmProofExecutionErrorV1 {
    fn from(value: std::io::Error) -> Self {
        Self::Io(value)
    }
}

struct ProofDirectory(PathBuf);

impl ProofDirectory {
    fn create() -> Result<Self, GeneralGemmProofExecutionErrorV1> {
        static NEXT: AtomicU64 = AtomicU64::new(1);
        let path = std::env::temp_dir().join(format!(
            "fe2o3-general-gemm-proof-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed),
        ));
        fs::create_dir(&path)?;
        fs::create_dir(path.join("negative"))?;
        Ok(Self(path))
    }

    fn write_sources(&self) -> Result<(), GeneralGemmProofExecutionErrorV1> {
        for (relative, bytes) in [
            ("general_gemm_schedule_model_v1.rs", MODEL_SOURCE),
            ("general_gemm_reference_schedule_v1.rs", REFERENCE_SOURCE),
            ("general_gemm_vectorized_schedule_v1.rs", VECTORIZED_SOURCE),
            (
                "negative/general_gemm_vector_tail_wrong.rs",
                VECTOR_TAIL_WRONG_SOURCE,
            ),
            (
                "negative/general_gemm_epilogue_wrong.rs",
                EPILOGUE_WRONG_SOURCE,
            ),
            (
                "negative/general_gemm_machine_claim_wrong.rs",
                MACHINE_CLAIM_WRONG_SOURCE,
            ),
        ] {
            fs::write(self.0.join(relative), bytes)?;
        }
        Ok(())
    }
}

impl Drop for ProofDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

struct ProcessObservation {
    status: ExitStatus,
    stdout: Vec<u8>,
    stderr: Vec<u8>,
}

/// Executes the exact embedded schedule proof and its mutation checks.
///
/// `verus_path` is accepted only when its bytes and version match the pinned
/// constants above. The returned evidence remains model-local and explicitly
/// records the open machine boundary.
pub fn execute_general_gemm_schedule_proof_v1(
    request: GeneralGemmProofRequestV1,
    verus_path: &Path,
    timeout_seconds: u32,
) -> Result<AuthenticatedGeneralGemmScheduleProofV1, GeneralGemmProofExecutionErrorV1> {
    if timeout_seconds == 0 || timeout_seconds > MAX_GENERAL_GEMM_PROOF_TIMEOUT_SECONDS_V1 {
        return Err(GeneralGemmProofExecutionErrorV1::InvalidTimeout);
    }
    if !verus_path.is_absolute() || !verus_path.is_file() {
        return Err(GeneralGemmProofExecutionErrorV1::InvalidVerusPath);
    }
    let verus_bytes = fs::read(verus_path)?;
    let tool_digest: [u8; 32] = Sha256::digest(&verus_bytes).into();
    if tool_digest != GENERAL_GEMM_VERUS_SHA256_V1 {
        return Err(GeneralGemmProofExecutionErrorV1::VerusDigestMismatch);
    }
    let version = run_bounded(
        verus_path,
        &["--version"],
        verus_path
            .parent()
            .ok_or(GeneralGemmProofExecutionErrorV1::InvalidVerusPath)?,
        timeout_seconds,
    )?;
    let version_text = String::from_utf8_lossy(&version.stdout);
    if !version.status.success()
        || !version_text.contains(&format!("Version: {GENERAL_GEMM_VERUS_VERSION_V1}"))
    {
        return Err(GeneralGemmProofExecutionErrorV1::VerusVersionMismatch);
    }

    let directory = ProofDirectory::create()?;
    directory.write_sources()?;
    let (positive_path, _, expected_verified) = request.schedule.positive_source();
    let positive = run_bounded(verus_path, &[positive_path], &directory.0, timeout_seconds)?;
    let expected_summary = format!("verification results:: {expected_verified} verified, 0 errors");
    if !positive.status.success()
        || !String::from_utf8_lossy(&positive.stdout).contains(&expected_summary)
    {
        return Err(GeneralGemmProofExecutionErrorV1::PositiveProofFailed);
    }

    let mut negative_paths = vec![
        "negative/general_gemm_epilogue_wrong.rs",
        "negative/general_gemm_machine_claim_wrong.rs",
    ];
    if request.schedule == GeneralGemmProofScheduleV1::VectorizedAOnlyBf16GlobalTransferV1 {
        negative_paths.insert(0, "negative/general_gemm_vector_tail_wrong.rs");
    }
    let mut negative_observations = Vec::with_capacity(negative_paths.len());
    for path in negative_paths {
        let observed = run_bounded(verus_path, &[path], &directory.0, timeout_seconds)?;
        let stderr = String::from_utf8_lossy(&observed.stderr);
        if observed.status.success()
            || !stderr.contains("postcondition not satisfied")
            || !stderr.contains(path)
        {
            return Err(GeneralGemmProofExecutionErrorV1::NegativeProofMismatch);
        }
        negative_observations.push(observed);
    }

    let source_closure_identity = source_closure_identity(request.schedule);
    let tool_identity = GeneralGemmEvidenceIdentityV1(tool_digest);
    let positive_output = output_evidence(&positive);
    let negative_outputs: Vec<_> = negative_observations.iter().map(output_evidence).collect();
    let identity = proof_identity(
        request,
        source_closure_identity,
        tool_identity,
        positive_output,
        &negative_outputs,
    );
    let properties = GENERAL_GEMM_PROOF_PROPERTIES_V1.map(|property| {
        let (status, basis) = property_evidence_basis(request.schedule, property);
        GeneralGemmPropertyEvidenceV1 {
            property,
            status,
            basis,
            identity: property_identity(identity, property, status, basis),
        }
    });
    Ok(AuthenticatedGeneralGemmScheduleProofV1 {
        request,
        identity,
        source_closure_identity,
        tool_identity,
        positive_output,
        negative_outputs,
        properties,
    })
}

fn run_bounded(
    program: &Path,
    arguments: &[&str],
    current_dir: &Path,
    timeout_seconds: u32,
) -> Result<ProcessObservation, GeneralGemmProofExecutionErrorV1> {
    let mut child = Command::new(program)
        .args(arguments)
        .current_dir(current_dir)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    let deadline = Instant::now() + Duration::from_secs(u64::from(timeout_seconds));
    loop {
        if child.try_wait()?.is_some() {
            break;
        }
        if Instant::now() >= deadline {
            child.kill()?;
            let _ = child.wait();
            return Err(GeneralGemmProofExecutionErrorV1::TimedOut);
        }
        thread::sleep(Duration::from_millis(10));
    }
    let output = child.wait_with_output()?;
    if output.stdout.len() > MAX_GENERAL_GEMM_PROOF_OUTPUT_BYTES_V1
        || output.stderr.len() > MAX_GENERAL_GEMM_PROOF_OUTPUT_BYTES_V1
    {
        return Err(GeneralGemmProofExecutionErrorV1::OutputTooLarge);
    }
    Ok(ProcessObservation {
        status: output.status,
        stdout: output.stdout,
        stderr: output.stderr,
    })
}

fn source_closure_identity(schedule: GeneralGemmProofScheduleV1) -> GeneralGemmEvidenceIdentityV1 {
    let (_, positive, _) = schedule.positive_source();
    let mut hasher = Sha256::new();
    hasher.update(SOURCE_CLOSURE_DOMAIN_V1);
    put_blob(&mut hasher, MODEL_SOURCE);
    put_blob(&mut hasher, positive);
    if schedule == GeneralGemmProofScheduleV1::VectorizedAOnlyBf16GlobalTransferV1 {
        put_blob(&mut hasher, VECTOR_TAIL_WRONG_SOURCE);
    }
    put_blob(&mut hasher, EPILOGUE_WRONG_SOURCE);
    put_blob(&mut hasher, MACHINE_CLAIM_WRONG_SOURCE);
    GeneralGemmEvidenceIdentityV1(hasher.finalize().into())
}

fn output_evidence(observed: &ProcessObservation) -> GeneralGemmProofOutputEvidenceV1 {
    let mut hasher = Sha256::new();
    hasher.update(EXECUTION_OUTPUT_DOMAIN_V1);
    hasher.update(observed.status.code().unwrap_or(-1).to_le_bytes());
    put_blob(&mut hasher, &observed.stdout);
    put_blob(&mut hasher, &observed.stderr);
    GeneralGemmProofOutputEvidenceV1 {
        identity: GeneralGemmEvidenceIdentityV1(hasher.finalize().into()),
        stdout_bytes: observed.stdout.len() as u64,
        stderr_bytes: observed.stderr.len() as u64,
    }
}

fn proof_identity(
    request: GeneralGemmProofRequestV1,
    source: GeneralGemmEvidenceIdentityV1,
    tool: GeneralGemmEvidenceIdentityV1,
    positive: GeneralGemmProofOutputEvidenceV1,
    negatives: &[GeneralGemmProofOutputEvidenceV1],
) -> GeneralGemmEvidenceIdentityV1 {
    let mut hasher = Sha256::new();
    hasher.update(PROOF_IDENTITY_DOMAIN_V1);
    hasher.update([request.schedule as u8]);
    for identity in request.identities() {
        hasher.update(identity.as_bytes());
    }
    hasher.update(source.as_bytes());
    hasher.update(tool.as_bytes());
    hasher.update(positive.identity.as_bytes());
    hasher.update((negatives.len() as u32).to_le_bytes());
    for negative in negatives {
        hasher.update(negative.identity.as_bytes());
    }
    GeneralGemmEvidenceIdentityV1(hasher.finalize().into())
}

fn property_identity(
    proof: GeneralGemmEvidenceIdentityV1,
    property: GeneralGemmProofPropertyV1,
    status: GeneralGemmPropertyEvidenceStatusV1,
    basis: GeneralGemmPropertyEvidenceBasisV1,
) -> GeneralGemmEvidenceIdentityV1 {
    let mut hasher = Sha256::new();
    hasher.update(PROPERTY_IDENTITY_DOMAIN_V1);
    hasher.update(proof.as_bytes());
    hasher.update([property as u8]);
    hasher.update([match status {
        GeneralGemmPropertyEvidenceStatusV1::ScheduleModelTheoremVerified => 1,
        GeneralGemmPropertyEvidenceStatusV1::ModelDefinitionOnly => 2,
        GeneralGemmPropertyEvidenceStatusV1::WeakerExactRealTheoremVerified => 3,
        GeneralGemmPropertyEvidenceStatusV1::OpenCorrespondenceRequired => 4,
        GeneralGemmPropertyEvidenceStatusV1::OpenArtifactRequired => 5,
    }]);
    match basis {
        GeneralGemmPropertyEvidenceBasisV1::VerifiedTheorem(name) => {
            hasher.update([1]);
            put_blob(&mut hasher, name.as_bytes());
        }
        GeneralGemmPropertyEvidenceBasisV1::ModelDefinition(name) => {
            hasher.update([2]);
            put_blob(&mut hasher, name.as_bytes());
        }
        GeneralGemmPropertyEvidenceBasisV1::OpenObligation(name) => {
            hasher.update([3]);
            put_blob(&mut hasher, name.as_bytes());
        }
    }
    GeneralGemmEvidenceIdentityV1(hasher.finalize().into())
}

fn property_evidence_basis(
    schedule: GeneralGemmProofScheduleV1,
    property: GeneralGemmProofPropertyV1,
) -> (
    GeneralGemmPropertyEvidenceStatusV1,
    GeneralGemmPropertyEvidenceBasisV1,
) {
    use GeneralGemmProofPropertyV1::*;
    use GeneralGemmPropertyEvidenceBasisV1::{ModelDefinition, OpenObligation, VerifiedTheorem};
    use GeneralGemmPropertyEvidenceStatusV1::{
        ModelDefinitionOnly, OpenArtifactRequired, OpenCorrespondenceRequired,
        ScheduleModelTheoremVerified, WeakerExactRealTheoremVerified,
    };

    let schedule_theorem = |reference, vectorized| match schedule {
        GeneralGemmProofScheduleV1::ReferenceWave64Xor4V1 => reference,
        GeneralGemmProofScheduleV1::VectorizedAOnlyBf16GlobalTransferV1 => vectorized,
    };
    match property {
        MemorySafe => (
            OpenCorrespondenceRequired,
            OpenObligation("allocation_provenance_from_authenticated_kir_v1"),
        ),
        BoundsSafe => (
            ScheduleModelTheoremVerified,
            VerifiedTheorem(schedule_theorem(
                "reference_modeled_global_accesses_are_bounded_v1",
                "vectorized_a_only_modeled_global_accesses_are_bounded_v1",
            )),
        ),
        Initialized => (
            OpenCorrespondenceRequired,
            OpenObligation("lds_write_read_initialization_from_authenticated_kir_v1"),
        ),
        RaceFree => (
            OpenCorrespondenceRequired,
            OpenObligation("global_and_lds_effect_conflict_freedom_from_authenticated_kir_v1"),
        ),
        BarrierConvergent => (
            ModelDefinitionOnly,
            ModelDefinition("lane_reaches_barrier_v1"),
        ),
        OutputRegionInjective => (
            ScheduleModelTheoremVerified,
            VerifiedTheorem(schedule_theorem(
                "reference_output_region_is_injective_v1",
                "vectorized_a_only_output_region_is_injective_v1",
            )),
        ),
        LdsEpochCorrect => (
            ModelDefinitionOnly,
            ModelDefinition("schedule_lds_epoch_correct_v1"),
        ),
        AccumulatorPhaseRefinement => (
            ScheduleModelTheoremVerified,
            VerifiedTheorem(schedule_theorem(
                "reference_accumulator_refines_contract_v1",
                "vectorized_accumulator_refines_contract_v1",
            )),
        ),
        TailRefinement => (
            ScheduleModelTheoremVerified,
            VerifiedTheorem(schedule_theorem(
                "reference_scalar_tail_zero_fills_v1",
                "vectorized_full_transfer_and_scalar_tail_refine_v1",
            )),
        ),
        EpilogueRefinement => (
            ScheduleModelTheoremVerified,
            VerifiedTheorem(schedule_theorem(
                "reference_epilogue_refines_exact_real_contract_v1",
                "vectorized_a_only_epilogue_refines_exact_real_contract_v1",
            )),
        ),
        NumericalContract => (
            WeakerExactRealTheoremVerified,
            VerifiedTheorem(schedule_theorem(
                "reference_numerical_contract_v1",
                "vectorized_numerical_contract_v1",
            )),
        ),
        MachineRefinementBoundary => (
            OpenArtifactRequired,
            OpenObligation("emitted_gfx942_machine_refinement_v1"),
        ),
    }
}

fn put_blob(hasher: &mut Sha256, bytes: &[u8]) {
    hasher.update((bytes.len() as u64).to_le_bytes());
    hasher.update(bytes);
}
