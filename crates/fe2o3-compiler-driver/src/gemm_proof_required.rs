//! Proof-required admission for the issue #138 tiled GEMM profile.

use core::fmt;

use fe2o3_compiler_api::{
    CanonicalDiagnosticV1, CompileDispositionV1, CompileOutputV1, CompileRequestV1,
    CompilerStageV1, DiagnosticCodeV1, DiagnosticMessageV1, DiagnosticSeverityV1,
    DiagnosticSubjectIdentityV1, ObligationSetIdentityV1, RequestIdentityV1,
};

use crate::{CompilerBackendFailureV1, TransactionalCompilerBackendV1};

/// Maximum number of aggregate and unsafe-escape findings in one GEMM report.
pub const MAX_GEMM_OBLIGATION_FINDINGS_V1: usize = 64;

/// Maximum compiler-derived unsafe obligations in one GEMM request.
pub const MAX_GEMM_UNSAFE_OBLIGATIONS_V1: usize =
    MAX_GEMM_OBLIGATION_FINDINGS_V1 - GEMM_REQUIRED_SAFETY_PROPERTIES_V1.len();

/// Independently required safety and refinement properties for tiled GEMM.
///
/// No variant implies or promotes another. In particular, memory safety does
/// not imply race freedom, and barrier convergence does not establish LDS
/// epoch correctness.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(u8)]
pub enum GemmSafetyPropertyV1 {
    /// All admitted memory effects use valid allocations and provenance.
    MemorySafe = 1,
    /// Every global, LDS, and fragment access is in bounds under its mask.
    BoundsSafe = 2,
    /// Every value is completely initialized before it is read.
    Initialized = 3,
    /// Cross-lane and cross-workgroup conflicting effects are excluded.
    RaceFree = 4,
    /// Every participating invocation reaches barriers convergently.
    BarrierConvergent = 5,
    /// Workgroup and lane output mappings are injective over live stores.
    OutputRegionInjective = 6,
    /// LDS publish and reuse epochs order every staged read and overwrite.
    LdsEpochCorrect = 7,
    /// Accumulators carry the exact partial sum across all K phases.
    AccumulatorPhaseRefinement = 8,
    /// Tail masks, zero fill, and predicated accesses refine the GEMM domain.
    TailRefinement = 9,
    /// Runtime alpha and beta implement the declared output epilogue.
    EpilogueRefinement = 10,
    /// BF16 input and FP32 accumulation obey the declared numerical contract.
    NumericalContract = 11,
    /// Evidence names the exact covered source-to-machine refinement boundary.
    MachineRefinementBoundary = 12,
}

/// Exact required property order for proof-required tiled GEMM admission.
pub const GEMM_REQUIRED_SAFETY_PROPERTIES_V1: [GemmSafetyPropertyV1; 12] = [
    GemmSafetyPropertyV1::MemorySafe,
    GemmSafetyPropertyV1::BoundsSafe,
    GemmSafetyPropertyV1::Initialized,
    GemmSafetyPropertyV1::RaceFree,
    GemmSafetyPropertyV1::BarrierConvergent,
    GemmSafetyPropertyV1::OutputRegionInjective,
    GemmSafetyPropertyV1::LdsEpochCorrect,
    GemmSafetyPropertyV1::AccumulatorPhaseRefinement,
    GemmSafetyPropertyV1::TailRefinement,
    GemmSafetyPropertyV1::EpilogueRefinement,
    GemmSafetyPropertyV1::NumericalContract,
    GemmSafetyPropertyV1::MachineRefinementBoundary,
];

impl GemmSafetyPropertyV1 {
    /// Returns the stable diagnostic spelling of this independent property.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::MemorySafe => "memory_safe",
            Self::BoundsSafe => "bounds_safe",
            Self::Initialized => "initialized",
            Self::RaceFree => "race_free",
            Self::BarrierConvergent => "barrier_convergent",
            Self::OutputRegionInjective => "output_region_injective",
            Self::LdsEpochCorrect => "lds_epoch_correct",
            Self::AccumulatorPhaseRefinement => "accumulator_phase_refinement",
            Self::TailRefinement => "tail_refinement",
            Self::EpilogueRefinement => "epilogue_refinement",
            Self::NumericalContract => "numerical_contract",
            Self::MachineRefinementBoundary => "machine_refinement_boundary",
        }
    }

    /// Returns the earliest semantic stage at which this property is required.
    pub const fn verification_stage(self) -> CompilerStageV1 {
        match self {
            Self::MemorySafe | Self::Initialized | Self::RaceFree => CompilerStageV1::Gpu,
            Self::BoundsSafe | Self::OutputRegionInjective => CompilerStageV1::Tile,
            Self::BarrierConvergent | Self::LdsEpochCorrect => CompilerStageV1::Gpu,
            Self::AccumulatorPhaseRefinement
            | Self::TailRefinement
            | Self::EpilogueRefinement
            | Self::NumericalContract => CompilerStageV1::Kernel,
            Self::MachineRefinementBoundary => CompilerStageV1::Amdgcn,
        }
    }
}

/// Result of checking one required or unsafe-originated obligation.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(u8)]
pub enum GemmObligationOutcomeV1 {
    /// The exact obligation was discharged for the bound inputs.
    Discharged = 1,
    /// Verification produced a definite counterexample.
    Counterexample = 2,
    /// The selected verifier does not support the obligation.
    Unsupported = 3,
    /// Verification exceeded its admitted time budget.
    TimedOut = 4,
    /// Verification ended without complete coverage.
    Incomplete = 5,
}

impl GemmObligationOutcomeV1 {
    const fn unproved_reason(self) -> &'static str {
        match self {
            Self::Discharged => "discharged",
            Self::Counterexample => "counterexample",
            Self::Unsupported => "unsupported",
            Self::TimedOut => "timed out",
            Self::Incomplete => "incomplete",
        }
    }
}

/// Origin of one verifier finding.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum GemmObligationOriginV1 {
    /// The aggregate obligation required independently for every GEMM.
    RequiredProperty,
    /// An additional obligation derived from unsafe source behavior.
    ///
    /// Unsafe findings supplement the required property; they can never stand
    /// in for the aggregate required-property result.
    UnsafeEscape {
        /// Nonzero stable obligation identifier derived by MIR admission.
        obligation_id: u32,
    },
}

/// Why a GEMM obligation finding could not be constructed.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GemmObligationFindingErrorV1 {
    /// Zero is reserved for absence of an unsafe obligation identifier.
    ZeroUnsafeObligationId,
}

impl fmt::Display for GemmObligationFindingErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("unsafe GEMM obligation identifier must be nonzero")
    }
}

impl std::error::Error for GemmObligationFindingErrorV1 {}

/// One property-local verifier result with an optional semantic subject.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct GemmObligationFindingV1 {
    property: GemmSafetyPropertyV1,
    origin: GemmObligationOriginV1,
    outcome: GemmObligationOutcomeV1,
    subject: Option<DiagnosticSubjectIdentityV1>,
}

impl GemmObligationFindingV1 {
    /// Creates the one aggregate result required for a property.
    pub const fn required(
        property: GemmSafetyPropertyV1,
        outcome: GemmObligationOutcomeV1,
        subject: Option<DiagnosticSubjectIdentityV1>,
    ) -> Self {
        Self {
            property,
            origin: GemmObligationOriginV1::RequiredProperty,
            outcome,
            subject,
        }
    }

    /// Creates an additional result derived from an unsafe source operation.
    pub const fn unsafe_escape(
        obligation_id: u32,
        property: GemmSafetyPropertyV1,
        outcome: GemmObligationOutcomeV1,
        subject: Option<DiagnosticSubjectIdentityV1>,
    ) -> Result<Self, GemmObligationFindingErrorV1> {
        if obligation_id == 0 {
            return Err(GemmObligationFindingErrorV1::ZeroUnsafeObligationId);
        }
        Ok(Self {
            property,
            origin: GemmObligationOriginV1::UnsafeEscape { obligation_id },
            outcome,
            subject,
        })
    }

    /// Returns the property checked by this finding.
    pub const fn property(self) -> GemmSafetyPropertyV1 {
        self.property
    }

    /// Returns whether this is an aggregate or unsafe-originated obligation.
    pub const fn origin(self) -> GemmObligationOriginV1 {
        self.origin
    }

    /// Returns the verifier outcome without promoting it to proof authority.
    pub const fn outcome(self) -> GemmObligationOutcomeV1 {
        self.outcome
    }

    /// Returns the semantic operation or source subject when retained.
    pub const fn subject(self) -> Option<DiagnosticSubjectIdentityV1> {
        self.subject
    }
}

/// One unsafe obligation expected by the compiler's authenticated MIR import.
///
/// This inventory entry is separate from verifier findings. Source code and a
/// proof-result provider cannot declare that the inventory is complete.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct GemmExpectedUnsafeObligationV1 {
    obligation_id: u32,
    property: GemmSafetyPropertyV1,
    subject: Option<DiagnosticSubjectIdentityV1>,
}

impl GemmExpectedUnsafeObligationV1 {
    /// Creates one nonzero compiler-derived unsafe obligation.
    pub const fn new(
        obligation_id: u32,
        property: GemmSafetyPropertyV1,
        subject: Option<DiagnosticSubjectIdentityV1>,
    ) -> Result<Self, GemmObligationFindingErrorV1> {
        if obligation_id == 0 {
            return Err(GemmObligationFindingErrorV1::ZeroUnsafeObligationId);
        }
        Ok(Self {
            obligation_id,
            property,
            subject,
        })
    }

    /// Returns the nonzero stable MIR-derived obligation identifier.
    pub const fn obligation_id(self) -> u32 {
        self.obligation_id
    }

    /// Returns the property the unsafe operation must establish.
    pub const fn property(self) -> GemmSafetyPropertyV1 {
        self.property
    }

    /// Returns the exact semantic subject retained by MIR admission.
    pub const fn subject(self) -> Option<DiagnosticSubjectIdentityV1> {
        self.subject
    }
}

/// Why compiler-owned GEMM proof requirements were rejected.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GemmProofRequirementsErrorV1 {
    /// The unsafe inventory exceeds the hard request bound.
    TooManyUnsafeObligations {
        /// Observed unsafe-obligation count.
        actual: usize,
        /// Hard V1 unsafe-obligation limit.
        maximum: usize,
    },
    /// Two compiler-derived entries reused one obligation identifier.
    DuplicateUnsafeObligationId {
        /// Duplicated nonzero obligation identifier.
        obligation_id: u32,
    },
}

impl fmt::Display for GemmProofRequirementsErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "invalid compiler-owned GEMM requirements: {self:?}"
        )
    }
}

impl std::error::Error for GemmProofRequirementsErrorV1 {}

/// Exact compiler-owned proof requirements for one compile request.
///
/// The frontend integration must derive the unsafe inventory from authenticated
/// MIR and construct this value before invoking a proof-result provider. The
/// request and obligation-set bindings prevent reuse for another kernel or
/// obligation set. This structural record does not authenticate its producer
/// and grants no proof, artifact, publication, load, or launch authority.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GemmProofRequirementsV1 {
    request_identity: RequestIdentityV1,
    obligation_set_identity: ObligationSetIdentityV1,
    expected_unsafe: Vec<GemmExpectedUnsafeObligationV1>,
}

impl GemmProofRequirementsV1 {
    /// Binds a checked, canonically ordered unsafe inventory to one request.
    pub fn new(
        request: &CompileRequestV1,
        mut expected_unsafe: Vec<GemmExpectedUnsafeObligationV1>,
    ) -> Result<Self, GemmProofRequirementsErrorV1> {
        if expected_unsafe.len() > MAX_GEMM_UNSAFE_OBLIGATIONS_V1 {
            return Err(GemmProofRequirementsErrorV1::TooManyUnsafeObligations {
                actual: expected_unsafe.len(),
                maximum: MAX_GEMM_UNSAFE_OBLIGATIONS_V1,
            });
        }
        expected_unsafe.sort_unstable_by_key(|obligation| obligation.obligation_id);
        if let Some(pair) = expected_unsafe
            .windows(2)
            .find(|pair| pair[0].obligation_id == pair[1].obligation_id)
        {
            return Err(GemmProofRequirementsErrorV1::DuplicateUnsafeObligationId {
                obligation_id: pair[0].obligation_id,
            });
        }
        Ok(Self {
            request_identity: request.identity(),
            obligation_set_identity: request.input_obligations_identity(),
            expected_unsafe,
        })
    }

    /// Returns the exact request commitment that owns these requirements.
    pub const fn request_identity(&self) -> RequestIdentityV1 {
        self.request_identity
    }

    /// Returns the exact obligation-set commitment that owns this inventory.
    pub const fn obligation_set_identity(&self) -> ObligationSetIdentityV1 {
        self.obligation_set_identity
    }

    /// Returns the canonical compiler-derived unsafe inventory.
    pub fn expected_unsafe_obligations(&self) -> &[GemmExpectedUnsafeObligationV1] {
        &self.expected_unsafe
    }
}

/// Why a bounded GEMM proof report could not be constructed.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GemmProofReportErrorV1 {
    /// The report exceeds the hard finding-count bound.
    TooManyFindings {
        /// Observed finding count.
        actual: usize,
        /// Hard V1 finding-count limit.
        maximum: usize,
    },
}

impl fmt::Display for GemmProofReportErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "invalid GEMM proof report: {self:?}")
    }
}

impl std::error::Error for GemmProofReportErrorV1 {}

/// Bounded findings for the request's exact obligation-set commitment.
///
/// This value is verifier input, not proof evidence or artifact authority. The
/// frontend/verifier integration remains responsible for deriving both the
/// obligation-set identity and the complete unsafe-operation inventory from
/// authenticated MIR instead of accepting an inventory declared by source.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GemmProofReportV1 {
    obligation_set_identity: ObligationSetIdentityV1,
    findings: Vec<GemmObligationFindingV1>,
}

impl GemmProofReportV1 {
    /// Creates a hard-bounded, otherwise untrusted report.
    pub fn new(
        obligation_set_identity: ObligationSetIdentityV1,
        findings: Vec<GemmObligationFindingV1>,
    ) -> Result<Self, GemmProofReportErrorV1> {
        if findings.len() > MAX_GEMM_OBLIGATION_FINDINGS_V1 {
            return Err(GemmProofReportErrorV1::TooManyFindings {
                actual: findings.len(),
                maximum: MAX_GEMM_OBLIGATION_FINDINGS_V1,
            });
        }
        Ok(Self {
            obligation_set_identity,
            findings,
        })
    }

    /// Returns the exact obligation-set commitment reported by the verifier.
    pub const fn obligation_set_identity(&self) -> ObligationSetIdentityV1 {
        self.obligation_set_identity
    }

    /// Returns the untrusted findings in producer order.
    pub fn findings(&self) -> &[GemmObligationFindingV1] {
        &self.findings
    }
}

/// Stable proof-required GEMM diagnostic codes.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
#[repr(u32)]
pub enum GemmProofDiagnosticV1 {
    /// The report names a different obligation-set commitment.
    ObligationSetMismatch = 0x4647_0001,
    /// The report contains duplicate aggregate or unsafe obligations.
    DuplicateObligation = 0x4647_0002,
    /// An obligation introduced by unsafe source behavior is unresolved.
    UnsafeObligationUnresolved = 0x4647_0003,
    /// The proof evaluator did not return a complete bounded report.
    EvaluationFailed = 0x4647_0004,
    /// Reported unsafe findings do not exactly match compiler-owned MIR requirements.
    UnsafeInventoryMismatch = 0x4647_0005,
    /// Compiler-owned requirements do not bind the active request.
    RequirementsBindingMismatch = 0x4647_0006,
    /// `memory_safe` was not discharged.
    MemorySafe = 0x4647_0101,
    /// `bounds_safe` was not discharged.
    BoundsSafe = 0x4647_0102,
    /// `initialized` was not discharged.
    Initialized = 0x4647_0103,
    /// `race_free` was not discharged.
    RaceFree = 0x4647_0104,
    /// `barrier_convergent` was not discharged.
    BarrierConvergent = 0x4647_0105,
    /// `output_region_injective` was not discharged.
    OutputRegionInjective = 0x4647_0106,
    /// `lds_epoch_correct` was not discharged.
    LdsEpochCorrect = 0x4647_0107,
    /// `accumulator_phase_refinement` was not discharged.
    AccumulatorPhaseRefinement = 0x4647_0108,
    /// `tail_refinement` was not discharged.
    TailRefinement = 0x4647_0109,
    /// `epilogue_refinement` was not discharged.
    EpilogueRefinement = 0x4647_010a,
    /// `numerical_contract` was not discharged.
    NumericalContract = 0x4647_010b,
    /// `machine_refinement_boundary` was not discharged.
    MachineRefinementBoundary = 0x4647_010c,
}

impl GemmProofDiagnosticV1 {
    /// Returns the stable numeric code stored in compiler diagnostics.
    pub const fn code(self) -> u32 {
        self as u32
    }

    pub(crate) const fn for_property(property: GemmSafetyPropertyV1) -> Self {
        match property {
            GemmSafetyPropertyV1::MemorySafe => Self::MemorySafe,
            GemmSafetyPropertyV1::BoundsSafe => Self::BoundsSafe,
            GemmSafetyPropertyV1::Initialized => Self::Initialized,
            GemmSafetyPropertyV1::RaceFree => Self::RaceFree,
            GemmSafetyPropertyV1::BarrierConvergent => Self::BarrierConvergent,
            GemmSafetyPropertyV1::OutputRegionInjective => Self::OutputRegionInjective,
            GemmSafetyPropertyV1::LdsEpochCorrect => Self::LdsEpochCorrect,
            GemmSafetyPropertyV1::AccumulatorPhaseRefinement => Self::AccumulatorPhaseRefinement,
            GemmSafetyPropertyV1::TailRefinement => Self::TailRefinement,
            GemmSafetyPropertyV1::EpilogueRefinement => Self::EpilogueRefinement,
            GemmSafetyPropertyV1::NumericalContract => Self::NumericalContract,
            GemmSafetyPropertyV1::MachineRefinementBoundary => Self::MachineRefinementBoundary,
        }
    }
}

/// Failure returned when the proof evaluator itself produced no report.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GemmProofEvaluationFailureV1 {
    /// The selected proof service or implementation is unavailable.
    Unavailable,
    /// The complete evaluation exceeded its admitted time budget.
    TimedOut,
    /// Evaluation exhausted a bounded compiler resource.
    ResourceExhausted,
    /// Evaluation returned malformed or incomplete results.
    InvalidResult,
}

impl GemmProofEvaluationFailureV1 {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Unavailable => "unavailable",
            Self::TimedOut => "timed out",
            Self::ResourceExhausted => "resource exhausted",
            Self::InvalidResult => "invalid result",
        }
    }
}

/// Stable semantic reason that proof-required GEMM admission was denied.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GemmProofRejectionKindV1 {
    /// The compiler-owned requirements name a different request.
    RequirementsRequestMismatch,
    /// The compiler-owned requirements name a different obligation set.
    RequirementsObligationSetMismatch,
    /// The report does not bind the request's exact obligation set.
    ObligationSetMismatch,
    /// One independently required aggregate property result is absent.
    MissingRequiredProperty(GemmSafetyPropertyV1),
    /// A required aggregate property occurs more than once.
    DuplicateRequiredProperty(GemmSafetyPropertyV1),
    /// A required aggregate property was not discharged.
    RequiredPropertyNotDischarged {
        /// Independently failing property.
        property: GemmSafetyPropertyV1,
        /// Exact non-success outcome.
        outcome: GemmObligationOutcomeV1,
    },
    /// An unsafe obligation identifier occurs more than once.
    DuplicateUnsafeObligation {
        /// Duplicated nonzero obligation identifier.
        obligation_id: u32,
    },
    /// A compiler-derived unsafe obligation has no verifier finding.
    MissingUnsafeObligation {
        /// Missing nonzero obligation identifier.
        obligation_id: u32,
        /// Required property derived from authenticated MIR.
        property: GemmSafetyPropertyV1,
    },
    /// A verifier finding names no compiler-derived unsafe obligation.
    UnexpectedUnsafeObligation {
        /// Unexpected nonzero obligation identifier.
        obligation_id: u32,
        /// Property claimed by the unexpected finding.
        property: GemmSafetyPropertyV1,
    },
    /// A verifier finding substituted the property of an expected obligation.
    UnsafeObligationPropertyMismatch {
        /// Nonzero obligation identifier.
        obligation_id: u32,
        /// Compiler-derived property.
        expected: GemmSafetyPropertyV1,
        /// Property claimed by the verifier report.
        actual: GemmSafetyPropertyV1,
    },
    /// A verifier finding substituted the subject of an expected obligation.
    UnsafeObligationSubjectMismatch {
        /// Nonzero obligation identifier.
        obligation_id: u32,
    },
    /// Unsafe source behavior introduced an unresolved named obligation.
    UnsafeObligationNotDischarged {
        /// Nonzero unsafe obligation identifier.
        obligation_id: u32,
        /// Property that unsafe behavior must still establish.
        property: GemmSafetyPropertyV1,
        /// Exact non-success outcome.
        outcome: GemmObligationOutcomeV1,
    },
    /// The proof evaluator failed before producing a complete report.
    EvaluationFailed(GemmProofEvaluationFailureV1),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct GemmProofRejectionV1 {
    kind: GemmProofRejectionKindV1,
    subject: Option<DiagnosticSubjectIdentityV1>,
}

impl GemmProofRejectionV1 {
    const fn new(
        kind: GemmProofRejectionKindV1,
        subject: Option<DiagnosticSubjectIdentityV1>,
    ) -> Self {
        Self { kind, subject }
    }

    fn diagnostic(self, request: &CompileRequestV1) -> CanonicalDiagnosticV1 {
        let (code, stage, message) = match self.kind {
            GemmProofRejectionKindV1::RequirementsRequestMismatch => (
                GemmProofDiagnosticV1::RequirementsBindingMismatch,
                CompilerStageV1::Mir,
                "compiler-owned GEMM requirements do not match the active request".to_owned(),
            ),
            GemmProofRejectionKindV1::RequirementsObligationSetMismatch => (
                GemmProofDiagnosticV1::RequirementsBindingMismatch,
                CompilerStageV1::Mir,
                "compiler-owned GEMM requirements do not match the request obligation set"
                    .to_owned(),
            ),
            GemmProofRejectionKindV1::ObligationSetMismatch => (
                GemmProofDiagnosticV1::ObligationSetMismatch,
                CompilerStageV1::Mir,
                "proof-required GEMM report does not match the request obligation set".to_owned(),
            ),
            GemmProofRejectionKindV1::MissingRequiredProperty(property) => (
                GemmProofDiagnosticV1::for_property(property),
                property.verification_stage(),
                format!(
                    "proof-required GEMM property {} could not be proved because its result is missing",
                    property.as_str()
                ),
            ),
            GemmProofRejectionKindV1::DuplicateRequiredProperty(property) => (
                GemmProofDiagnosticV1::DuplicateObligation,
                property.verification_stage(),
                format!(
                    "proof-required GEMM report contains duplicate {} results",
                    property.as_str()
                ),
            ),
            GemmProofRejectionKindV1::RequiredPropertyNotDischarged {
                property,
                outcome: GemmObligationOutcomeV1::Counterexample,
            } => (
                GemmProofDiagnosticV1::for_property(property),
                property.verification_stage(),
                format!(
                    "proof-required GEMM property {} has a counterexample",
                    property.as_str()
                ),
            ),
            GemmProofRejectionKindV1::RequiredPropertyNotDischarged { property, outcome } => (
                GemmProofDiagnosticV1::for_property(property),
                property.verification_stage(),
                format!(
                    "proof-required GEMM property {} could not be proved: {}",
                    property.as_str(),
                    outcome.unproved_reason()
                ),
            ),
            GemmProofRejectionKindV1::DuplicateUnsafeObligation { obligation_id } => (
                GemmProofDiagnosticV1::DuplicateObligation,
                CompilerStageV1::Mir,
                format!(
                    "proof-required GEMM report contains duplicate unsafe obligation {obligation_id}"
                ),
            ),
            GemmProofRejectionKindV1::MissingUnsafeObligation {
                obligation_id,
                property,
            } => (
                GemmProofDiagnosticV1::UnsafeInventoryMismatch,
                property.verification_stage(),
                format!(
                    "compiler-derived unsafe GEMM obligation {obligation_id} for {} has no verifier result",
                    property.as_str()
                ),
            ),
            GemmProofRejectionKindV1::UnexpectedUnsafeObligation {
                obligation_id,
                property,
            } => (
                GemmProofDiagnosticV1::UnsafeInventoryMismatch,
                CompilerStageV1::Mir,
                format!(
                    "GEMM verifier reported unexpected unsafe obligation {obligation_id} for {}",
                    property.as_str()
                ),
            ),
            GemmProofRejectionKindV1::UnsafeObligationPropertyMismatch {
                obligation_id,
                expected,
                actual,
            } => (
                GemmProofDiagnosticV1::UnsafeInventoryMismatch,
                expected.verification_stage(),
                format!(
                    "unsafe GEMM obligation {obligation_id} property mismatch: expected {}, reported {}",
                    expected.as_str(),
                    actual.as_str()
                ),
            ),
            GemmProofRejectionKindV1::UnsafeObligationSubjectMismatch { obligation_id } => (
                GemmProofDiagnosticV1::UnsafeInventoryMismatch,
                CompilerStageV1::Mir,
                format!(
                    "unsafe GEMM obligation {obligation_id} semantic subject does not match compiler-owned MIR requirements"
                ),
            ),
            GemmProofRejectionKindV1::UnsafeObligationNotDischarged {
                obligation_id,
                property,
                outcome: GemmObligationOutcomeV1::Counterexample,
            } => (
                GemmProofDiagnosticV1::UnsafeObligationUnresolved,
                property.verification_stage(),
                format!(
                    "unsafe GEMM obligation {obligation_id} for {} has a counterexample",
                    property.as_str()
                ),
            ),
            GemmProofRejectionKindV1::UnsafeObligationNotDischarged {
                obligation_id,
                property,
                outcome,
            } => (
                GemmProofDiagnosticV1::UnsafeObligationUnresolved,
                property.verification_stage(),
                format!(
                    "unsafe GEMM obligation {obligation_id} for {} could not be proved: {}",
                    property.as_str(),
                    outcome.unproved_reason()
                ),
            ),
            GemmProofRejectionKindV1::EvaluationFailed(failure) => (
                GemmProofDiagnosticV1::EvaluationFailed,
                CompilerStageV1::Kernel,
                format!(
                    "proof-required GEMM evaluation failed closed: {}",
                    failure.as_str()
                ),
            ),
        };
        let subject = self.subject.or_else(|| {
            Some(DiagnosticSubjectIdentityV1::from_untrusted_bytes(
                request.kernel_instance_identity().into_bytes(),
            ))
        });
        CanonicalDiagnosticV1::new(
            0,
            DiagnosticCodeV1::new(code.code()).expect("static GEMM diagnostic code is nonzero"),
            DiagnosticSeverityV1::Error,
            Some(stage),
            subject,
            DiagnosticMessageV1::new(message)
                .expect("static bounded GEMM diagnostic presentation is canonical"),
        )
    }
}

/// Unforgeable compiler-local proof gate for one exact GEMM request.
///
/// This value only permits candidate construction to begin. It is not proof
/// evidence and grants no publication, load, dispatch, or launch authority.
#[derive(Debug)]
pub struct ProofRequiredGemmAdmissionV1 {
    request_identity: RequestIdentityV1,
    obligation_set_identity: ObligationSetIdentityV1,
    _private: (),
}

impl ProofRequiredGemmAdmissionV1 {
    /// Returns the exact compile request admitted by this compiler-local gate.
    pub const fn request_identity(&self) -> RequestIdentityV1 {
        self.request_identity
    }

    /// Returns the exact obligation set admitted by this compiler-local gate.
    pub const fn obligation_set_identity(&self) -> ObligationSetIdentityV1 {
        self.obligation_set_identity
    }
}

/// Applies the fail-closed GEMM policy to a bounded report.
///
/// Required aggregate results are checked in the fixed property order, so a
/// hostile producer cannot reorder diagnostics. Additional unsafe obligations
/// are checked by ascending obligation identifier after all required results.
pub fn admit_proof_required_gemm_v1(
    request: &CompileRequestV1,
    requirements: &GemmProofRequirementsV1,
    report: &GemmProofReportV1,
) -> Result<ProofRequiredGemmAdmissionV1, GemmProofRejectionKindV1> {
    verify_gemm_report(request, requirements, report).map_err(|rejection| rejection.kind)
}

fn verify_gemm_report(
    request: &CompileRequestV1,
    requirements: &GemmProofRequirementsV1,
    report: &GemmProofReportV1,
) -> Result<ProofRequiredGemmAdmissionV1, GemmProofRejectionV1> {
    verify_requirements_binding(request, requirements)?;
    if report.obligation_set_identity != request.input_obligations_identity() {
        return Err(GemmProofRejectionV1::new(
            GemmProofRejectionKindV1::ObligationSetMismatch,
            None,
        ));
    }

    for property in GEMM_REQUIRED_SAFETY_PROPERTIES_V1 {
        let mut matching = report.findings.iter().copied().filter(|finding| {
            finding.property == property
                && finding.origin == GemmObligationOriginV1::RequiredProperty
        });
        let Some(finding) = matching.next() else {
            return Err(GemmProofRejectionV1::new(
                GemmProofRejectionKindV1::MissingRequiredProperty(property),
                None,
            ));
        };
        if matching.next().is_some() {
            return Err(GemmProofRejectionV1::new(
                GemmProofRejectionKindV1::DuplicateRequiredProperty(property),
                finding.subject,
            ));
        }
        if finding.outcome != GemmObligationOutcomeV1::Discharged {
            return Err(GemmProofRejectionV1::new(
                GemmProofRejectionKindV1::RequiredPropertyNotDischarged {
                    property,
                    outcome: finding.outcome,
                },
                finding.subject,
            ));
        }
    }

    let mut unsafe_findings: Vec<_> = report
        .findings
        .iter()
        .copied()
        .filter_map(|finding| match finding.origin {
            GemmObligationOriginV1::RequiredProperty => None,
            GemmObligationOriginV1::UnsafeEscape { obligation_id } => {
                Some((obligation_id, finding))
            }
        })
        .collect();
    unsafe_findings.sort_unstable_by_key(|(obligation_id, _)| *obligation_id);
    for pair in unsafe_findings.windows(2) {
        if pair[0].0 == pair[1].0 {
            return Err(GemmProofRejectionV1::new(
                GemmProofRejectionKindV1::DuplicateUnsafeObligation {
                    obligation_id: pair[0].0,
                },
                pair[0].1.subject.or(pair[1].1.subject),
            ));
        }
    }

    let expected = &requirements.expected_unsafe;
    let mut expected_index = 0;
    let mut finding_index = 0;
    while expected_index < expected.len() || finding_index < unsafe_findings.len() {
        match (
            expected.get(expected_index),
            unsafe_findings.get(finding_index),
        ) {
            (Some(expected), None) => {
                return Err(GemmProofRejectionV1::new(
                    GemmProofRejectionKindV1::MissingUnsafeObligation {
                        obligation_id: expected.obligation_id,
                        property: expected.property,
                    },
                    expected.subject,
                ));
            }
            (None, Some((obligation_id, finding))) => {
                return Err(GemmProofRejectionV1::new(
                    GemmProofRejectionKindV1::UnexpectedUnsafeObligation {
                        obligation_id: *obligation_id,
                        property: finding.property,
                    },
                    finding.subject,
                ));
            }
            (Some(expected), Some((obligation_id, finding)))
                if expected.obligation_id < *obligation_id =>
            {
                return Err(GemmProofRejectionV1::new(
                    GemmProofRejectionKindV1::MissingUnsafeObligation {
                        obligation_id: expected.obligation_id,
                        property: expected.property,
                    },
                    expected.subject,
                ));
            }
            (Some(expected), Some((obligation_id, finding)))
                if *obligation_id < expected.obligation_id =>
            {
                return Err(GemmProofRejectionV1::new(
                    GemmProofRejectionKindV1::UnexpectedUnsafeObligation {
                        obligation_id: *obligation_id,
                        property: finding.property,
                    },
                    finding.subject,
                ));
            }
            (Some(expected), Some((obligation_id, finding))) => {
                debug_assert_eq!(expected.obligation_id, *obligation_id);
                if expected.property != finding.property {
                    return Err(GemmProofRejectionV1::new(
                        GemmProofRejectionKindV1::UnsafeObligationPropertyMismatch {
                            obligation_id: *obligation_id,
                            expected: expected.property,
                            actual: finding.property,
                        },
                        expected.subject.or(finding.subject),
                    ));
                }
                if expected.subject != finding.subject {
                    return Err(GemmProofRejectionV1::new(
                        GemmProofRejectionKindV1::UnsafeObligationSubjectMismatch {
                            obligation_id: *obligation_id,
                        },
                        expected.subject.or(finding.subject),
                    ));
                }
                if finding.outcome != GemmObligationOutcomeV1::Discharged {
                    return Err(GemmProofRejectionV1::new(
                        GemmProofRejectionKindV1::UnsafeObligationNotDischarged {
                            obligation_id: *obligation_id,
                            property: finding.property,
                            outcome: finding.outcome,
                        },
                        finding.subject,
                    ));
                }
                expected_index += 1;
                finding_index += 1;
            }
            (None, None) => break,
        }
    }

    Ok(ProofRequiredGemmAdmissionV1 {
        request_identity: request.identity(),
        obligation_set_identity: report.obligation_set_identity,
        _private: (),
    })
}

fn verify_requirements_binding(
    request: &CompileRequestV1,
    requirements: &GemmProofRequirementsV1,
) -> Result<(), GemmProofRejectionV1> {
    if requirements.request_identity != request.identity() {
        return Err(GemmProofRejectionV1::new(
            GemmProofRejectionKindV1::RequirementsRequestMismatch,
            None,
        ));
    }
    if requirements.obligation_set_identity != request.input_obligations_identity() {
        return Err(GemmProofRejectionV1::new(
            GemmProofRejectionKindV1::RequirementsObligationSetMismatch,
            None,
        ));
    }
    Ok(())
}

/// Supplies proof findings without constructing or publishing device artifacts.
pub trait GemmProofReportProviderV1 {
    /// Evaluates the request's exact GEMM obligations.
    fn evaluate(
        &mut self,
        request: &CompileRequestV1,
        requirements: &GemmProofRequirementsV1,
    ) -> Result<GemmProofReportV1, GemmProofEvaluationFailureV1>;
}

/// Candidate-producing backend callable only after proof-required admission.
pub trait AdmittedGemmCompilerBackendV1 {
    /// Compiles an admitted request, consuming its unforgeable local gate.
    fn compile_admitted(
        &mut self,
        request: &CompileRequestV1,
        admission: ProofRequiredGemmAdmissionV1,
    ) -> Result<CompileOutputV1, CompilerBackendFailureV1>;
}

/// Transactional backend that verifies before any device candidate work begins.
#[derive(Clone, Debug)]
pub struct ProofRequiredGemmBackendV1<Provider, Backend> {
    requirements: GemmProofRequirementsV1,
    provider: Provider,
    backend: Backend,
}

impl<Provider, Backend> ProofRequiredGemmBackendV1<Provider, Backend> {
    /// Wraps compiler-owned requirements, a report provider, and a candidate backend.
    pub const fn new(
        requirements: GemmProofRequirementsV1,
        provider: Provider,
        backend: Backend,
    ) -> Self {
        Self {
            requirements,
            provider,
            backend,
        }
    }

    /// Returns shared access to requirements, report provider, and admitted backend.
    pub const fn parts(&self) -> (&GemmProofRequirementsV1, &Provider, &Backend) {
        (&self.requirements, &self.provider, &self.backend)
    }

    /// Returns mutable access to the report provider and admitted backend.
    pub fn parts_mut(&mut self) -> (&GemmProofRequirementsV1, &mut Provider, &mut Backend) {
        (&self.requirements, &mut self.provider, &mut self.backend)
    }
}

impl<Provider, Backend> TransactionalCompilerBackendV1
    for ProofRequiredGemmBackendV1<Provider, Backend>
where
    Provider: GemmProofReportProviderV1,
    Backend: AdmittedGemmCompilerBackendV1,
{
    fn compile_transaction(
        &mut self,
        request: &CompileRequestV1,
    ) -> Result<CompileOutputV1, CompilerBackendFailureV1> {
        if let Err(rejection) = verify_requirements_binding(request, &self.requirements) {
            return Ok(rejected_output(request, rejection));
        }
        let report = match self.provider.evaluate(request, &self.requirements) {
            Ok(report) => report,
            Err(failure) => {
                return Ok(rejected_output(
                    request,
                    GemmProofRejectionV1::new(
                        GemmProofRejectionKindV1::EvaluationFailed(failure),
                        None,
                    ),
                ));
            }
        };
        match verify_gemm_report(request, &self.requirements, &report) {
            Ok(admission) => self.backend.compile_admitted(request, admission),
            Err(rejection) => Ok(rejected_output(request, rejection)),
        }
    }
}

fn rejected_output(request: &CompileRequestV1, rejection: GemmProofRejectionV1) -> CompileOutputV1 {
    CompileOutputV1::new(
        request,
        CompileDispositionV1::Rejected,
        Vec::new(),
        Vec::new(),
        vec![rejection.diagnostic(request)],
        None,
    )
    .expect("a checked request always admits one bounded GEMM rejection diagnostic")
}

pub(crate) fn semantic_counterexample_output(
    request: &CompileRequestV1,
    property: GemmSafetyPropertyV1,
    subject: Option<DiagnosticSubjectIdentityV1>,
) -> CompileOutputV1 {
    rejected_output(
        request,
        GemmProofRejectionV1::new(
            GemmProofRejectionKindV1::RequiredPropertyNotDischarged {
                property,
                outcome: GemmObligationOutcomeV1::Counterexample,
            },
            subject,
        ),
    )
}

pub(crate) fn semantic_binding_mismatch_output(request: &CompileRequestV1) -> CompileOutputV1 {
    rejected_output(
        request,
        GemmProofRejectionV1::new(GemmProofRejectionKindV1::RequirementsRequestMismatch, None),
    )
}

pub(crate) fn semantic_malformed_output(request: &CompileRequestV1) -> CompileOutputV1 {
    rejected_output(
        request,
        GemmProofRejectionV1::new(
            GemmProofRejectionKindV1::EvaluationFailed(GemmProofEvaluationFailureV1::InvalidResult),
            None,
        ),
    )
}
