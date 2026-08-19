//! Versioned expectations for proof-required general GEMM source rejection.
//!
//! The fixtures are ordinary safe Rust `#[kernel]` sources and are typechecked
//! by their standalone fixture crate. This module does not claim to verify
//! their semantics. It freezes mutation IDs, exact required-property names,
//! stages, diagnostic codes, and sources for a compiler driver to consume and
//! independently satisfy. A source digest mismatch is not an acceptable
//! substitute for one of these semantic outcomes.

/// Schema identity for the general GEMM semantic negative corpus.
pub const GEMM_SEMANTIC_CORPUS_SCHEMA_V1: &str = "fe2o3-general-gemm-negative-corpus-v1";

/// Safe ordinary-Rust source model from which semantic mutations are derived.
///
/// This source typechecks but is not compiler or launch authority. Its fixture
/// support methods stand in for the future sealed safe device capabilities.
pub const GENERAL_GEMM_SAFE_SOURCE_MODEL_V1: &str =
    include_str!("../tests/fixtures/general_tiled_gemm_corpus/src/valid_reference.rs");

/// Exact production property vocabulary shared with the Pliron compiler lane.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum GemmRequiredPropertyV1 {
    /// All memory operations are valid for their regions and capabilities.
    MemorySafe,
    /// Every dynamic access is in bounds under the host preconditions.
    BoundsSafe,
    /// Every read observes completely initialized storage.
    Initialized,
    /// Parallel writes and conflicting accesses are race free.
    RaceFree,
    /// Every required workgroup participant reaches barriers uniformly.
    BarrierConvergent,
    /// Output ownership is injective across groups, lanes, and components.
    OutputRegionInjective,
    /// LDS publication and reuse epochs are ordered correctly.
    LdsEpochCorrect,
    /// Accumulators carry every K-phase contribution.
    AccumulatorPhaseRefinement,
    /// Tail masks and zero fill refine the logical GEMM domain.
    TailRefinement,
    /// The output epilogue implements the recorded alpha/beta expression.
    EpilogueRefinement,
    /// Arithmetic satisfies the shared BF16/F32 numerical contract.
    NumericalContract,
    /// Evidence covers the exact declared machine-refinement boundary.
    MachineRefinementBoundary,
}

/// Complete mirrored property order checked against the compiler driver.
pub const GEMM_REQUIRED_PROPERTIES_V1: [GemmRequiredPropertyV1; 12] = [
    GemmRequiredPropertyV1::MemorySafe,
    GemmRequiredPropertyV1::BoundsSafe,
    GemmRequiredPropertyV1::Initialized,
    GemmRequiredPropertyV1::RaceFree,
    GemmRequiredPropertyV1::BarrierConvergent,
    GemmRequiredPropertyV1::OutputRegionInjective,
    GemmRequiredPropertyV1::LdsEpochCorrect,
    GemmRequiredPropertyV1::AccumulatorPhaseRefinement,
    GemmRequiredPropertyV1::TailRefinement,
    GemmRequiredPropertyV1::EpilogueRefinement,
    GemmRequiredPropertyV1::NumericalContract,
    GemmRequiredPropertyV1::MachineRefinementBoundary,
];

impl GemmRequiredPropertyV1 {
    /// Returns the frozen compiler-facing property spelling.
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

    /// Returns the stable compiler diagnostic code for this property.
    pub const fn diagnostic_code(self) -> u32 {
        match self {
            Self::MemorySafe => 0x4647_0101,
            Self::BoundsSafe => 0x4647_0102,
            Self::Initialized => 0x4647_0103,
            Self::RaceFree => 0x4647_0104,
            Self::BarrierConvergent => 0x4647_0105,
            Self::OutputRegionInjective => 0x4647_0106,
            Self::LdsEpochCorrect => 0x4647_0107,
            Self::AccumulatorPhaseRefinement => 0x4647_0108,
            Self::TailRefinement => 0x4647_0109,
            Self::EpilogueRefinement => 0x4647_010a,
            Self::NumericalContract => 0x4647_010b,
            Self::MachineRefinementBoundary => 0x4647_010c,
        }
    }
}

/// Compiler stage expected to reject one mutation.
///
/// Variants mirror `CompilerStageV1` without making this standalone example
/// depend on compiler-driver implementation code.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(u8)]
pub enum GemmVerificationStageV1 {
    /// Structured algorithm, indexing, and numerical semantics.
    Kernel = 3,
    /// Distributed regions, masks, and physical tile layouts.
    Tile = 5,
    /// Target-neutral executable SIMT representation.
    Gpu = 6,
    /// AMDGPU-selected semantics and target legalization.
    Amdgcn = 7,
}

impl GemmVerificationStageV1 {
    /// Returns the frozen compiler-facing stage spelling.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Kernel => "kernel",
            Self::Tile => "tile",
            Self::Gpu => "gpu",
            Self::Amdgcn => "amdgcn",
        }
    }

    /// Returns the matching `CompilerStageV1` wire tag.
    pub const fn wire_tag(self) -> u8 {
        self as u8
    }
}

/// Whether a semantic rejection has a concrete witness or remains unproved.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum GemmFailureKindV1 {
    /// Analysis produced a concrete violating invocation or access pair.
    Counterexample,
    /// Required proof was unknown, timed out, unsupported, or incomplete.
    Unproved,
}

/// One source-level mutation ID from issue #138.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum SemanticMutationV1 {
    /// A tail load is not guarded by `row < M && depth < K`.
    UnguardedATailLoad,
    /// A tail load is not guarded by `depth < K && column < N`.
    UnguardedBTailLoad,
    /// A tail output store is not guarded by `row < M && column < N`.
    UnguardedCTailStore,
    /// Multiple lanes map to the same logical C element.
    DuplicateLaneCWrite,
    /// Multiple output workgroups map to the same C tile.
    OverlappingWorkgroupCTile,
    /// Multiple lanes map to the same LDS slot in one epoch.
    DuplicateLdsWrite,
    /// An LDS value is read without complete initialization.
    LdsReadBeforeInitialization,
    /// LDS is consumed without a publication barrier.
    MissingPublishBarrier,
    /// A barrier is controlled by a lane-varying condition.
    DivergentBarrier,
    /// LDS is overwritten for the next phase without a reuse barrier.
    MissingReuseBarrier,
    /// An LDS read names an expired phase epoch.
    ExpiredLdsEpoch,
    /// An asynchronously staged value is read before admitted completion.
    StagedReadBeforeWait,
    /// Accumulators are reset between reduction phases.
    AccumulatorReset,
    /// A K-tail slot contains a nonzero value.
    IncorrectKTailZeroFill,
    /// The output expression does not implement `alpha*AB + beta*C`.
    IncorrectAlphaBetaEpilogue,
}

impl SemanticMutationV1 {
    /// Returns the stable snake-case mutation ID.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::UnguardedATailLoad => "unguarded_a_tail_load",
            Self::UnguardedBTailLoad => "unguarded_b_tail_load",
            Self::UnguardedCTailStore => "unguarded_c_tail_store",
            Self::DuplicateLaneCWrite => "duplicate_lane_c_write",
            Self::OverlappingWorkgroupCTile => "overlapping_workgroup_c_tile",
            Self::DuplicateLdsWrite => "duplicate_lds_write",
            Self::LdsReadBeforeInitialization => "lds_read_before_initialization",
            Self::MissingPublishBarrier => "missing_publish_barrier",
            Self::DivergentBarrier => "divergent_barrier",
            Self::MissingReuseBarrier => "missing_reuse_barrier",
            Self::ExpiredLdsEpoch => "expired_lds_epoch",
            Self::StagedReadBeforeWait => "staged_read_before_wait",
            Self::AccumulatorReset => "accumulator_reset",
            Self::IncorrectKTailZeroFill => "incorrect_k_tail_zero_fill",
            Self::IncorrectAlphaBetaEpilogue => "incorrect_alpha_beta_epilogue",
        }
    }
}

/// Stable expected semantic diagnostic for one source mutation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GemmSemanticDiagnosticV1 {
    /// Required property that must fail independently.
    pub property: GemmRequiredPropertyV1,
    /// Compiler stage expected to report the failure.
    pub stage: GemmVerificationStageV1,
    /// Stable fe2o3 diagnostic code.
    pub code: u32,
    /// Whether the expected failure has a concrete witness.
    pub kind: GemmFailureKindV1,
}

/// One safe-Rust source fixture and its exact expected semantic rejection.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GemmSemanticNegativeCaseV1 {
    /// Source mutation identity.
    pub mutation: SemanticMutationV1,
    /// Repository-relative fixture path.
    pub fixture_path: &'static str,
    /// Exact fixture source embedded at crate build time.
    pub source: &'static str,
    /// Expected proof-required failure.
    pub expected: GemmSemanticDiagnosticV1,
}

const fn diagnostic(
    property: GemmRequiredPropertyV1,
    stage: GemmVerificationStageV1,
) -> GemmSemanticDiagnosticV1 {
    GemmSemanticDiagnosticV1 {
        property,
        stage,
        code: property.diagnostic_code(),
        kind: GemmFailureKindV1::Counterexample,
    }
}

macro_rules! case {
    ($mutation:ident, $file:literal, $property:ident, $stage:ident) => {
        GemmSemanticNegativeCaseV1 {
            mutation: SemanticMutationV1::$mutation,
            fixture_path: concat!(
                "tests/fixtures/general_tiled_gemm_corpus/src/invalid/",
                $file,
                ".rs"
            ),
            source: include_str!(concat!(
                "../tests/fixtures/general_tiled_gemm_corpus/src/invalid/",
                $file,
                ".rs"
            )),
            expected: diagnostic(
                GemmRequiredPropertyV1::$property,
                GemmVerificationStageV1::$stage,
            ),
        }
    };
}

/// Complete V1 semantic negative source corpus in issue order.
pub const SEMANTIC_NEGATIVE_CORPUS_V1: &[GemmSemanticNegativeCaseV1] = &[
    case!(
        UnguardedATailLoad,
        "unguarded_a_tail_load",
        BoundsSafe,
        Tile
    ),
    case!(
        UnguardedBTailLoad,
        "unguarded_b_tail_load",
        BoundsSafe,
        Tile
    ),
    case!(
        UnguardedCTailStore,
        "unguarded_c_tail_store",
        BoundsSafe,
        Tile
    ),
    case!(
        DuplicateLaneCWrite,
        "duplicate_lane_c_write",
        OutputRegionInjective,
        Tile
    ),
    case!(
        OverlappingWorkgroupCTile,
        "overlapping_workgroup_c_tile",
        OutputRegionInjective,
        Tile
    ),
    case!(DuplicateLdsWrite, "duplicate_lds_write", RaceFree, Gpu),
    case!(
        LdsReadBeforeInitialization,
        "lds_read_before_initialization",
        Initialized,
        Gpu
    ),
    case!(
        MissingPublishBarrier,
        "missing_publish_barrier",
        Initialized,
        Gpu
    ),
    case!(
        DivergentBarrier,
        "divergent_barrier",
        BarrierConvergent,
        Gpu
    ),
    case!(
        MissingReuseBarrier,
        "missing_reuse_barrier",
        LdsEpochCorrect,
        Gpu
    ),
    case!(ExpiredLdsEpoch, "expired_lds_epoch", LdsEpochCorrect, Gpu),
    case!(
        StagedReadBeforeWait,
        "staged_read_before_wait",
        Initialized,
        Gpu
    ),
    case!(
        AccumulatorReset,
        "accumulator_reset",
        AccumulatorPhaseRefinement,
        Kernel
    ),
    case!(
        IncorrectKTailZeroFill,
        "incorrect_k_tail_zero_fill",
        TailRefinement,
        Kernel
    ),
    case!(
        IncorrectAlphaBetaEpilogue,
        "incorrect_alpha_beta_epilogue",
        EpilogueRefinement,
        Kernel
    ),
];
