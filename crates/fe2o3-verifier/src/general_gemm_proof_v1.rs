//! Authenticated schedule-model proof evidence for issue #138 general GEMM.
//!
//! The stock Verus release delegates through a launcher, rustup, rust_verify,
//! solver and dynamic-library closure. That closure is not currently supplied
//! as reviewed, retainable inputs, so execution fails closed before touching a
//! caller path or creating any proof-authority evidence.

use std::{fmt, path::Path};

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
