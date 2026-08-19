//! Authenticated schedule-model proof evidence for issue #138 general GEMM.
//!
//! The legacy launcher-path API remains fail-closed. A separate V2 entry point
//! accepts only the exact retained runtime closure admitted beneath the protected
//! `/opt` installation root, executes exact retained proof sources, and returns model-local
//! evidence that grants no compiler, artifact, load, or launch authority.

use std::{
    fmt,
    path::Path,
    time::{Duration, Instant},
};

use sha2::{Digest as _, Sha256};

use crate::general_gemm_runtime_closure_v2::{
    GeneralGemmProofSourceV2, GeneralGemmRuntimeClosureErrorKindV2,
    GeneralGemmRuntimeClosureErrorV2, GeneralGemmRuntimeProcessOutputV2,
    GeneralGemmVerusRuntimeClosureLeaseV2,
};

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

const PROOF_IDENTITY_DOMAIN_V2: &[u8] = b"fe2o3-general-gemm-symbolic-schedule-proof-v2\0";
const PROPERTY_IDENTITY_DOMAIN_V1: &[u8] = b"fe2o3-general-gemm-property-evidence-v1\0";
const SOURCE_CLOSURE_DOMAIN_V2: &[u8] = b"fe2o3-general-gemm-proof-source-closure-v2\0";
const EXECUTION_OUTPUT_DOMAIN_V2: &[u8] = b"fe2o3-general-gemm-proof-output-v2\0";
const MODEL_INPUT_IDENTITY_DOMAIN_V1: &[u8] = b"fe2o3-general-gemm-verus-model-input-v1\0";
const POSITIVE_SOURCE_IDENTITY_DOMAIN_V1: &[u8] = b"fe2o3-general-gemm-verus-positive-source-v1\0";
const THEOREM_SET_IDENTITY_DOMAIN_V1: &[u8] = b"fe2o3-general-gemm-verus-theorem-set-v1\0";

const MODEL_SOURCE: &[u8] = include_bytes!("../verus/general_gemm_schedule_model_v1.rs");
#[cfg(test)]
const REFERENCE_SOURCE: &[u8] = include_bytes!("../verus/general_gemm_reference_schedule_v1.rs");
#[cfg(test)]
const VECTORIZED_SOURCE: &[u8] = include_bytes!("../verus/general_gemm_vectorized_schedule_v1.rs");
#[cfg(test)]
const VECTOR_TAIL_WRONG_SOURCE: &[u8] =
    include_bytes!("../verus/negative/general_gemm_vector_tail_wrong.rs");
#[cfg(test)]
const EPILOGUE_WRONG_SOURCE: &[u8] =
    include_bytes!("../verus/negative/general_gemm_epilogue_wrong.rs");
#[cfg(test)]
const MACHINE_CLAIM_WRONG_SOURCE: &[u8] =
    include_bytes!("../verus/negative/general_gemm_machine_claim_wrong.rs");

const POSITIVE_STDOUT: &[u8] = b"verification results:: 28 verified, 0 errors\n";
const MODEL_NEGATIVE_STDOUT: &[u8] = b"verification results:: 22 verified, 1 errors\n";
const EPILOGUE_NEGATIVE_STDOUT: &[u8] = b"verification results:: 1 verified, 1 errors\n";

const VECTOR_TAIL_WRONG_STDERR: &[u8] = br#"error: postcondition not satisfied
  --> /proc/self/fd/186/negative/general_gemm_vector_tail_wrong.rs:24:9
   |
10 | / pub proof fn mutated_unguarded_vector_tail_is_bounded_v1(
11 | |     group_y: nat,
12 | |     phase: nat,
13 | |     lane: nat,
...  |
16 | |     lda: nat,
17 | | )
   | |_- at the end of the function body
...
24 |           model::phase_depth_v1(phase, lane, 3) < k,
   |           ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ failed this postcondition

error: aborting due to 1 previous error

"#;
const EPILOGUE_WRONG_STDERR: &[u8] = br#"error: postcondition not satisfied
  --> /proc/self/fd/186/negative/general_gemm_epilogue_wrong.rs:11:13
   |
 5 | / pub proof fn mutated_epilogue_omits_beta_v1(
 6 | |     alpha: real,
 7 | |     accumulator: real,
 8 | |     beta: real,
 9 | |     c: real,
10 | | )
   | |_- at the end of the function body
11 |       ensures alpha * accumulator + c == alpha * accumulator + beta * c,
   |               ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ failed this postcondition

error: aborting due to 1 previous error

"#;
const MACHINE_CLAIM_WRONG_STDERR: &[u8] = br#"error: postcondition not satisfied
  --> /proc/self/fd/186/negative/general_gemm_machine_claim_wrong.rs:11:13
   |
10 | pub proof fn mutated_symbolic_proof_claims_machine_refinement_v1()
   | ------------------------------------------------------------------ at the end of the function body
11 |     ensures model::machine_refinement_complete_v1(),
   |             ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ failed this postcondition

error: aborting due to 1 previous error

"#;

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
    const fn positive_source(self) -> GeneralGemmProofSourceV2 {
        match self {
            Self::ReferenceWave64Xor4V1 => GeneralGemmProofSourceV2::Reference,
            Self::VectorizedAOnlyBf16GlobalTransferV1 => GeneralGemmProofSourceV2::Vectorized,
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

/// Symbolic template identities supplied by compiler integration.
///
/// The request contains no concrete dimensions, strides, coefficients, plan,
/// KIR instance, or runtime ABI. Those belong only to a later checked launch
/// instantiation and cannot be inherited by this parameterized model proof.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GeneralGemmProofRequestV1 {
    schedule: GeneralGemmProofScheduleV1,
    schedule_identity: GeneralGemmEvidenceIdentityV1,
    symbolic_plan_identity: GeneralGemmEvidenceIdentityV1,
    symbolic_kir_identity: GeneralGemmEvidenceIdentityV1,
    symbolic_compilation_identity: GeneralGemmEvidenceIdentityV1,
    compile_request_identity: GeneralGemmEvidenceIdentityV1,
    obligation_set_identity: GeneralGemmEvidenceIdentityV1,
    compiler_identity: GeneralGemmEvidenceIdentityV1,
    target_identity: GeneralGemmEvidenceIdentityV1,
    toolchain_identity: GeneralGemmEvidenceIdentityV1,
    source_template_identity: GeneralGemmEvidenceIdentityV1,
    numerical_policy_identity: GeneralGemmEvidenceIdentityV1,
}

impl GeneralGemmProofRequestV1 {
    /// Checks nonzero, domain-distinct identities for one schedule proof.
    #[allow(clippy::too_many_arguments)]
    pub fn checked(
        schedule: GeneralGemmProofScheduleV1,
        schedule_identity: GeneralGemmEvidenceIdentityV1,
        symbolic_plan_identity: GeneralGemmEvidenceIdentityV1,
        symbolic_kir_identity: GeneralGemmEvidenceIdentityV1,
        symbolic_compilation_identity: GeneralGemmEvidenceIdentityV1,
        compile_request_identity: GeneralGemmEvidenceIdentityV1,
        obligation_set_identity: GeneralGemmEvidenceIdentityV1,
        compiler_identity: GeneralGemmEvidenceIdentityV1,
        target_identity: GeneralGemmEvidenceIdentityV1,
        toolchain_identity: GeneralGemmEvidenceIdentityV1,
        source_template_identity: GeneralGemmEvidenceIdentityV1,
        numerical_policy_identity: GeneralGemmEvidenceIdentityV1,
    ) -> Result<Self, GeneralGemmProofExecutionErrorV1> {
        let identities = [
            schedule_identity,
            symbolic_plan_identity,
            symbolic_kir_identity,
            symbolic_compilation_identity,
            compile_request_identity,
            obligation_set_identity,
            compiler_identity,
            target_identity,
            toolchain_identity,
            source_template_identity,
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
            symbolic_plan_identity,
            symbolic_kir_identity,
            symbolic_compilation_identity,
            compile_request_identity,
            obligation_set_identity,
            compiler_identity,
            target_identity,
            toolchain_identity,
            source_template_identity,
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

    /// Returns the canonical runtime-parameterized plan-schema identity.
    pub const fn symbolic_plan_identity(self) -> GeneralGemmEvidenceIdentityV1 {
        self.symbolic_plan_identity
    }

    /// Returns the canonical runtime-parameterized KIR-template identity.
    pub const fn symbolic_kir_identity(self) -> GeneralGemmEvidenceIdentityV1 {
        self.symbolic_kir_identity
    }

    /// Returns the aggregate symbolic compilation identity.
    pub const fn symbolic_compilation_identity(self) -> GeneralGemmEvidenceIdentityV1 {
        self.symbolic_compilation_identity
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

    /// Returns the frontend semantic binding for the source template.
    pub const fn source_template_identity(self) -> GeneralGemmEvidenceIdentityV1 {
        self.source_template_identity
    }

    /// Returns the exact numerical-policy identity.
    pub const fn numerical_policy_identity(self) -> GeneralGemmEvidenceIdentityV1 {
        self.numerical_policy_identity
    }

    /// Symbolic proof inputs never authorize one concrete launch instance.
    pub const fn grants_concrete_launch_authority(self) -> bool {
        false
    }

    pub(crate) fn identities(self) -> [GeneralGemmEvidenceIdentityV1; 11] {
        [
            self.schedule_identity,
            self.symbolic_plan_identity,
            self.symbolic_kir_identity,
            self.symbolic_compilation_identity,
            self.compile_request_identity,
            self.obligation_set_identity,
            self.compiler_identity,
            self.target_identity,
            self.toolchain_identity,
            self.source_template_identity,
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
///
/// ```compile_fail
/// fn duplicate(evidence: fe2o3_verifier::AuthenticatedGeneralGemmScheduleProofV1) {
///     let _copy = evidence.clone();
/// }
/// ```
///
/// ```compile_fail
/// fn cannot_publish(evidence: fe2o3_verifier::AuthenticatedGeneralGemmScheduleProofV1) {
///     evidence.publish();
/// }
/// ```
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

    /// Returns the exact reviewed Verus runtime-closure identity.
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
    /// No reviewed, retained closure exists for the stock Verus runtime.
    AuthenticatedRuntimeClosureUnavailable,
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
    /// Executed retained source evidence did not match checked KIR/model correspondence.
    RetainedSourceCorrespondenceMismatch,
    /// The reviewed retained Verus runtime closure could not be used safely.
    RuntimeClosure(GeneralGemmRuntimeClosureErrorV2),
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

/// Refuses to mint schedule-proof evidence without a closed runtime manifest.
///
/// The current arguments can name only one launcher. They cannot bind the
/// launcher-selected `rustup`, `rust_verify`, solver, vstd, rlibs, macro DSOs,
/// Rust toolchain, dynamic loader, shared libraries, environment, or executed
/// proof-source objects. Until those inputs have an exact reviewed manifest and
/// are retained immutably through a supervised process tree, this function
/// deliberately does not inspect `verus_path`, create files, or spawn a child.
pub fn execute_general_gemm_schedule_proof_v1(
    request: GeneralGemmProofRequestV1,
    verus_path: &Path,
    timeout_seconds: u32,
) -> Result<AuthenticatedGeneralGemmScheduleProofV1, GeneralGemmProofExecutionErrorV1> {
    if timeout_seconds == 0 || timeout_seconds > MAX_GENERAL_GEMM_PROOF_TIMEOUT_SECONDS_V1 {
        return Err(GeneralGemmProofExecutionErrorV1::InvalidTimeout);
    }
    let _ = (request, verus_path);
    Err(GeneralGemmProofExecutionErrorV1::AuthenticatedRuntimeClosureUnavailable)
}

/// Executes the exact retained schedule proof suite through one retained V2 runtime closure.
///
/// The lease can be opened only from one protected root-owned version directory beneath
/// `/opt/fe2o3/verus-runtime-v2`. Every proof source is selected by a closed typed enum from the
/// retained root-owned manifest; the returned evidence remains model-local and grants no compiler,
/// artifact, load, or launch gate.
pub fn execute_general_gemm_schedule_proof_with_runtime_closure_v2(
    request: GeneralGemmProofRequestV1,
    runtime: &GeneralGemmVerusRuntimeClosureLeaseV2,
    timeout_seconds: u32,
) -> Result<AuthenticatedGeneralGemmScheduleProofV1, GeneralGemmProofExecutionErrorV1> {
    if timeout_seconds == 0 || timeout_seconds > MAX_GENERAL_GEMM_PROOF_TIMEOUT_SECONDS_V1 {
        return Err(GeneralGemmProofExecutionErrorV1::InvalidTimeout);
    }
    runtime.revalidate().map_err(map_runtime_error)?;
    let deadline = Instant::now()
        .checked_add(Duration::from_secs(u64::from(timeout_seconds)))
        .ok_or(GeneralGemmProofExecutionErrorV1::InvalidTimeout)?;

    let positive = request.schedule.positive_source();

    let negatives = negative_cases(request.schedule);

    let source_closure_identity = source_closure_identity(positive, &negatives);
    let positive_observation = runtime
        .execute_rust_verify(positive, deadline, MAX_GENERAL_GEMM_PROOF_OUTPUT_BYTES_V1)
        .map_err(map_runtime_error)?;
    require_positive_output(&positive_observation)?;

    let mut negative_observations = Vec::with_capacity(negatives.len());
    for negative in &negatives {
        let observation = runtime
            .execute_rust_verify(
                negative.source,
                deadline,
                MAX_GENERAL_GEMM_PROOF_OUTPUT_BYTES_V1,
            )
            .map_err(map_runtime_error)?;
        require_negative_output(
            &observation,
            negative.expected_stdout,
            negative.expected_stderr,
        )?;
        negative_observations.push(observation);
    }
    runtime.revalidate().map_err(map_runtime_error)?;
    if Instant::now() >= deadline {
        return Err(GeneralGemmProofExecutionErrorV1::TimedOut);
    }

    let tool_identity = GeneralGemmEvidenceIdentityV1(runtime.identity().as_bytes());
    let positive_output = output_evidence(&positive_observation);
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

struct NegativeCaseV2 {
    source: GeneralGemmProofSourceV2,
    expected_stdout: &'static [u8],
    expected_stderr: &'static [u8],
}

impl NegativeCaseV2 {
    const fn new(
        source: GeneralGemmProofSourceV2,
        expected_stdout: &'static [u8],
        expected_stderr: &'static [u8],
    ) -> Self {
        Self {
            source,
            expected_stdout,
            expected_stderr,
        }
    }
}

fn negative_cases(schedule: GeneralGemmProofScheduleV1) -> Vec<NegativeCaseV2> {
    let mut negatives = Vec::with_capacity(
        if schedule == GeneralGemmProofScheduleV1::VectorizedAOnlyBf16GlobalTransferV1 {
            3
        } else {
            2
        },
    );
    if schedule == GeneralGemmProofScheduleV1::VectorizedAOnlyBf16GlobalTransferV1 {
        negatives.push(NegativeCaseV2::new(
            GeneralGemmProofSourceV2::VectorTailWrong,
            MODEL_NEGATIVE_STDOUT,
            VECTOR_TAIL_WRONG_STDERR,
        ));
    }
    negatives.push(NegativeCaseV2::new(
        GeneralGemmProofSourceV2::EpilogueWrong,
        EPILOGUE_NEGATIVE_STDOUT,
        EPILOGUE_WRONG_STDERR,
    ));
    negatives.push(NegativeCaseV2::new(
        GeneralGemmProofSourceV2::MachineClaimWrong,
        MODEL_NEGATIVE_STDOUT,
        MACHINE_CLAIM_WRONG_STDERR,
    ));
    negatives
}

pub(crate) struct GeneralGemmDerivedVerusInputIdentitiesV1 {
    pub(crate) model_identity: GeneralGemmEvidenceIdentityV1,
    pub(crate) positive_source_identity: GeneralGemmEvidenceIdentityV1,
    pub(crate) theorem_set_identity: GeneralGemmEvidenceIdentityV1,
    pub(crate) source_closure_identity: GeneralGemmEvidenceIdentityV1,
}

pub(crate) fn derive_general_gemm_verus_input_identities_v1(
    schedule: GeneralGemmProofScheduleV1,
) -> GeneralGemmDerivedVerusInputIdentitiesV1 {
    let positive = schedule.positive_source();
    let model_identity = model_input_identity_with_bytes(MODEL_SOURCE);
    let positive_source_identity = positive_source_identity_with_bytes(
        schedule,
        positive.relative_to_proof_directory(),
        positive.embedded_bytes(),
    );

    let mut theorem_set = Sha256::new();
    theorem_set.update(THEOREM_SET_IDENTITY_DOMAIN_V1);
    theorem_set.update([schedule as u8]);
    theorem_set.update(model_identity.as_bytes());
    theorem_set.update(positive_source_identity.as_bytes());
    for property in GENERAL_GEMM_PROOF_PROPERTIES_V1 {
        let (status, basis) = property_evidence_basis(schedule, property);
        theorem_set.update([property as u8, property_status_tag(status)]);
        match basis {
            GeneralGemmPropertyEvidenceBasisV1::VerifiedTheorem(name) => {
                theorem_set.update([1]);
                put_blob(&mut theorem_set, name.as_bytes());
            }
            GeneralGemmPropertyEvidenceBasisV1::ModelDefinition(name) => {
                theorem_set.update([2]);
                put_blob(&mut theorem_set, name.as_bytes());
            }
            GeneralGemmPropertyEvidenceBasisV1::OpenObligation(name) => {
                theorem_set.update([3]);
                put_blob(&mut theorem_set, name.as_bytes());
            }
        }
    }
    let theorem_set_identity = GeneralGemmEvidenceIdentityV1(theorem_set.finalize().into());
    let negatives = negative_cases(schedule);
    let source_closure_identity = source_closure_identity(positive, &negatives);

    GeneralGemmDerivedVerusInputIdentitiesV1 {
        model_identity,
        positive_source_identity,
        theorem_set_identity,
        source_closure_identity,
    }
}

fn model_input_identity_with_bytes(bytes: &[u8]) -> GeneralGemmEvidenceIdentityV1 {
    let mut model = Sha256::new();
    model.update(MODEL_INPUT_IDENTITY_DOMAIN_V1);
    put_blob(&mut model, b"general_gemm_schedule_model_v1.rs");
    put_blob(&mut model, bytes);
    GeneralGemmEvidenceIdentityV1(model.finalize().into())
}

fn positive_source_identity_with_bytes(
    schedule: GeneralGemmProofScheduleV1,
    relative_path: &str,
    bytes: &[u8],
) -> GeneralGemmEvidenceIdentityV1 {
    let mut source = Sha256::new();
    source.update(POSITIVE_SOURCE_IDENTITY_DOMAIN_V1);
    source.update([schedule as u8]);
    put_blob(&mut source, relative_path.as_bytes());
    put_blob(&mut source, bytes);
    GeneralGemmEvidenceIdentityV1(source.finalize().into())
}

const fn property_status_tag(status: GeneralGemmPropertyEvidenceStatusV1) -> u8 {
    match status {
        GeneralGemmPropertyEvidenceStatusV1::ScheduleModelTheoremVerified => 1,
        GeneralGemmPropertyEvidenceStatusV1::ModelDefinitionOnly => 2,
        GeneralGemmPropertyEvidenceStatusV1::WeakerExactRealTheoremVerified => 3,
        GeneralGemmPropertyEvidenceStatusV1::OpenCorrespondenceRequired => 4,
        GeneralGemmPropertyEvidenceStatusV1::OpenArtifactRequired => 5,
    }
}

fn require_positive_output(
    observed: &GeneralGemmRuntimeProcessOutputV2,
) -> Result<(), GeneralGemmProofExecutionErrorV1> {
    if observed.exit_code != Some(0)
        || observed.signal.is_some()
        || observed.stdout != POSITIVE_STDOUT
        || !observed.stderr.is_empty()
    {
        return Err(GeneralGemmProofExecutionErrorV1::PositiveProofFailed);
    }
    Ok(())
}

fn require_negative_output(
    observed: &GeneralGemmRuntimeProcessOutputV2,
    expected_stdout: &[u8],
    expected_stderr: &[u8],
) -> Result<(), GeneralGemmProofExecutionErrorV1> {
    if observed.exit_code != Some(1)
        || observed.signal.is_some()
        || observed.stdout != expected_stdout
        || observed.stderr != expected_stderr
    {
        return Err(GeneralGemmProofExecutionErrorV1::NegativeProofMismatch);
    }
    Ok(())
}

fn map_runtime_error(error: GeneralGemmRuntimeClosureErrorV2) -> GeneralGemmProofExecutionErrorV1 {
    match error.kind() {
        GeneralGemmRuntimeClosureErrorKindV2::TimedOut => {
            GeneralGemmProofExecutionErrorV1::TimedOut
        }
        GeneralGemmRuntimeClosureErrorKindV2::OutputTooLarge => {
            GeneralGemmProofExecutionErrorV1::OutputTooLarge
        }
        _ => GeneralGemmProofExecutionErrorV1::RuntimeClosure(error),
    }
}

fn source_closure_identity(
    positive: GeneralGemmProofSourceV2,
    negatives: &[NegativeCaseV2],
) -> GeneralGemmEvidenceIdentityV1 {
    source_closure_identity_with_positive_bytes(positive, positive.embedded_bytes(), negatives)
}

fn source_closure_identity_with_positive_bytes(
    positive: GeneralGemmProofSourceV2,
    embedded_positive: &[u8],
    negatives: &[NegativeCaseV2],
) -> GeneralGemmEvidenceIdentityV1 {
    let mut hasher = Sha256::new();
    hasher.update(SOURCE_CLOSURE_DOMAIN_V2);
    put_blob(&mut hasher, b"general_gemm_schedule_model_v1.rs");
    put_blob(&mut hasher, MODEL_SOURCE);
    put_blob(
        &mut hasher,
        positive.relative_to_proof_directory().as_bytes(),
    );
    put_blob(&mut hasher, embedded_positive);
    hasher.update((negatives.len() as u32).to_le_bytes());
    for negative in negatives {
        put_blob(
            &mut hasher,
            negative.source.relative_to_proof_directory().as_bytes(),
        );
        put_blob(&mut hasher, negative.source.embedded_bytes());
    }
    GeneralGemmEvidenceIdentityV1(hasher.finalize().into())
}

fn output_evidence(
    observed: &GeneralGemmRuntimeProcessOutputV2,
) -> GeneralGemmProofOutputEvidenceV1 {
    let mut hasher = Sha256::new();
    hasher.update(EXECUTION_OUTPUT_DOMAIN_V2);
    hasher.update(observed.exit_code.unwrap_or(-1).to_le_bytes());
    hasher.update(observed.signal.unwrap_or(0).to_le_bytes());
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
    hasher.update(PROOF_IDENTITY_DOMAIN_V2);
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

#[cfg(test)]
mod tests {
    use super::*;

    fn output(exit_code: i32, stdout: &[u8], stderr: &[u8]) -> GeneralGemmRuntimeProcessOutputV2 {
        GeneralGemmRuntimeProcessOutputV2 {
            exit_code: Some(exit_code),
            signal: None,
            stdout: stdout.to_vec(),
            stderr: stderr.to_vec(),
        }
    }

    #[test]
    fn typed_sources_are_the_exact_embedded_proof_bodies() {
        for (source, embedded) in [
            (GeneralGemmProofSourceV2::Reference, REFERENCE_SOURCE),
            (GeneralGemmProofSourceV2::Vectorized, VECTORIZED_SOURCE),
            (
                GeneralGemmProofSourceV2::VectorTailWrong,
                VECTOR_TAIL_WRONG_SOURCE,
            ),
            (
                GeneralGemmProofSourceV2::EpilogueWrong,
                EPILOGUE_WRONG_SOURCE,
            ),
            (
                GeneralGemmProofSourceV2::MachineClaimWrong,
                MACHINE_CLAIM_WRONG_SOURCE,
            ),
        ] {
            assert_eq!(source.embedded_bytes(), embedded);
        }
    }

    #[test]
    fn source_closure_identity_rejects_embedded_source_substitution() {
        let negatives = [NegativeCaseV2::new(
            GeneralGemmProofSourceV2::EpilogueWrong,
            EPILOGUE_NEGATIVE_STDOUT,
            EPILOGUE_WRONG_STDERR,
        )];
        let exact = source_closure_identity_with_positive_bytes(
            GeneralGemmProofSourceV2::Reference,
            REFERENCE_SOURCE,
            &negatives,
        );
        let mut substituted = REFERENCE_SOURCE.to_vec();
        substituted[0] ^= 1;
        assert_ne!(
            exact,
            source_closure_identity_with_positive_bytes(
                GeneralGemmProofSourceV2::Reference,
                &substituted,
                &negatives,
            )
        );
    }

    #[test]
    fn model_and_positive_source_identities_reject_stale_bytes() {
        let schedule = GeneralGemmProofScheduleV1::ReferenceWave64Xor4V1;
        let positive = schedule.positive_source();
        let exact = derive_general_gemm_verus_input_identities_v1(schedule);

        let mut stale_model = MODEL_SOURCE.to_vec();
        stale_model[0] ^= 1;
        assert_ne!(
            exact.model_identity,
            model_input_identity_with_bytes(&stale_model)
        );

        let mut stale_source = positive.embedded_bytes().to_vec();
        stale_source[0] ^= 1;
        assert_ne!(
            exact.positive_source_identity,
            positive_source_identity_with_bytes(
                schedule,
                positive.relative_to_proof_directory(),
                &stale_source,
            )
        );
    }

    #[test]
    fn positive_output_parser_requires_exact_typed_result() {
        require_positive_output(&output(0, POSITIVE_STDOUT, b"")).unwrap();
        for altered in [
            output(1, POSITIVE_STDOUT, b""),
            output(0, b"verification results:: 29 verified, 0 errors\n", b""),
            output(0, POSITIVE_STDOUT, b"warning\n"),
        ] {
            assert!(matches!(
                require_positive_output(&altered),
                Err(GeneralGemmProofExecutionErrorV1::PositiveProofFailed)
            ));
        }
        let mut signaled = output(0, POSITIVE_STDOUT, b"");
        signaled.signal = Some(9);
        assert!(require_positive_output(&signaled).is_err());
    }

    #[test]
    fn negative_output_parser_rejects_every_untyped_substitution() {
        require_negative_output(
            &output(1, MODEL_NEGATIVE_STDOUT, VECTOR_TAIL_WRONG_STDERR),
            MODEL_NEGATIVE_STDOUT,
            VECTOR_TAIL_WRONG_STDERR,
        )
        .unwrap();
        require_negative_output(
            &output(1, EPILOGUE_NEGATIVE_STDOUT, EPILOGUE_WRONG_STDERR),
            EPILOGUE_NEGATIVE_STDOUT,
            EPILOGUE_WRONG_STDERR,
        )
        .unwrap();
        for altered in [
            output(0, MODEL_NEGATIVE_STDOUT, VECTOR_TAIL_WRONG_STDERR),
            output(2, MODEL_NEGATIVE_STDOUT, VECTOR_TAIL_WRONG_STDERR),
            output(
                1,
                b"verification results:: 21 verified, 1 errors\n",
                VECTOR_TAIL_WRONG_STDERR,
            ),
            output(1, MODEL_NEGATIVE_STDOUT, b""),
            output(1, MODEL_NEGATIVE_STDOUT, EPILOGUE_WRONG_STDERR),
            output(
                1,
                MODEL_NEGATIVE_STDOUT,
                b"error: postcondition not satisfied\n",
            ),
        ] {
            assert!(matches!(
                require_negative_output(&altered, MODEL_NEGATIVE_STDOUT, VECTOR_TAIL_WRONG_STDERR),
                Err(GeneralGemmProofExecutionErrorV1::NegativeProofMismatch)
            ));
        }
        let mut signaled = output(1, MODEL_NEGATIVE_STDOUT, VECTOR_TAIL_WRONG_STDERR);
        signaled.signal = Some(9);
        assert!(
            require_negative_output(&signaled, MODEL_NEGATIVE_STDOUT, VECTOR_TAIL_WRONG_STDERR)
                .is_err()
        );
    }
}
