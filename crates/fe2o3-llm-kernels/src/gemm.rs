//! Exact Ferric B3-bound Qwen3 linear-operation specialization.
//!
//! This module maps each admitted target/draft generated-plan bucket and
//! linear operator to the existing checked `fe2o3-tiled-gemm-v1` host plan,
//! CPU schedule model, and twelve-property obligation taxonomy. It creates no
//! competing GEMM planner and grants no compiler or executable authority.

use std::error::Error;
use std::fmt;

use fe2o3_tiled_gemm_v1::{
    GEMM_REQUIRED_PROPERTIES_V1, GENERAL_GEMM_PLAN_SCHEMA_V1, GENERAL_GEMM_REFERENCE_SCHEDULE_V1,
    GemmRequiredPropertyV1, GeneralGemmPlanV1, GeneralGemmRequestV1, GeneralLaunchLimitsV1,
    GeneralPlanErrorV1, GeneralReferenceErrorV1, GeneralReferenceResultV1, admit_target_v1,
    exact_target_v1, execute_general_reference_v1, plan_general_gemm_v1,
};
use sha2::{Digest, Sha256};

/// Ferric B3 graph commit defining the exact generated plans specialized here.
pub const FERRIC_B3_GRAPH_COMMIT_V1: &str = "e078ca3f37aeddab43b04e568831b1c7a1471204";
/// Tree of [`FERRIC_B3_GRAPH_COMMIT_V1`].
pub const FERRIC_B3_GRAPH_TREE_V1: &str = "11d048144b76548d5e3c79f15d09934206903fa3";
/// Git blob identity of `crates/ferric-spec/src/graph.rs` at the B3 commit.
pub const FERRIC_B3_GRAPH_BLOB_V1: &str = "94ed32aca3275d6be4b3c01471b83db3432ab722";
/// SHA-256 of the exact B3 `graph.rs` source bytes.
pub const FERRIC_B3_GRAPH_SOURCE_SHA256_V1: &str =
    "576cd0714444ce13152bfa21e82b4b608542dfc4b43d557d790099db335d1b48";

/// Pinned Qwen3 vocabulary size used by both target and draft logits.
pub const QWEN3_VOCABULARY_SIZE_V1: u32 = 151_936;
/// Sentinel layer used by B3 graph-global operators.
pub const QWEN3_NO_LAYER_V1: u16 = u16::MAX;

/// Canonical UTF-8 family-identity preimage.
pub const QWEN3_LINEAR_FAMILY_ID_PREIMAGE_V1: &str = "fe2o3.qwen3.b3.linear.gemm_gemv.gfx942.v1";
/// SHA-256 of [`QWEN3_LINEAR_FAMILY_ID_PREIMAGE_V1`].
pub const QWEN3_LINEAR_FAMILY_ID_V1: [u8; 32] = [
    0x63, 0xd6, 0xa8, 0xfd, 0x5b, 0x04, 0x69, 0x93, 0x5e, 0x9a, 0x02, 0x83, 0x55, 0x9f, 0x11, 0xa8,
    0x23, 0xe0, 0xd5, 0xad, 0xae, 0x81, 0x47, 0xeb, 0x19, 0xa3, 0xea, 0x02, 0xa9, 0xdf, 0xd0, 0x77,
];
/// Canonical UTF-8 candidate-schema preimage.
pub const QWEN3_LINEAR_CANDIDATE_SCHEMA_ID_PREIMAGE_V1: &str =
    "fe2o3.qwen3.b3.linear.candidate.schema.v1";
/// SHA-256 of [`QWEN3_LINEAR_CANDIDATE_SCHEMA_ID_PREIMAGE_V1`].
pub const QWEN3_LINEAR_CANDIDATE_SCHEMA_ID_V1: [u8; 32] = [
    0xf7, 0xb2, 0xc9, 0x9f, 0x91, 0x32, 0x88, 0xe2, 0x73, 0xc0, 0x18, 0xa8, 0x87, 0x5c, 0x6a, 0xd3,
    0x38, 0x20, 0xd3, 0x1b, 0x08, 0xbb, 0xeb, 0x00, 0x0c, 0x79, 0xe5, 0xd8, 0xfd, 0x6e, 0x88, 0x95,
];
/// Canonical UTF-8 general-route adapter preimage.
pub const QWEN3_LINEAR_GENERAL_ROUTE_ID_PREIMAGE_V1: &str =
    "fe2o3.qwen3.b3.linear.general_gemm.reference.adapter.v1";
/// SHA-256 of [`QWEN3_LINEAR_GENERAL_ROUTE_ID_PREIMAGE_V1`].
pub const QWEN3_LINEAR_GENERAL_ROUTE_ID_V1: [u8; 32] = [
    0x83, 0x6c, 0x06, 0xe4, 0x81, 0x8a, 0x1f, 0x98, 0xbc, 0xd9, 0x4f, 0xb3, 0x9c, 0xb7, 0x7f, 0x0e,
    0xa9, 0x0e, 0x13, 0x81, 0x3e, 0x11, 0x1e, 0x7d, 0x1e, 0x0f, 0xff, 0x38, 0xb7, 0xf9, 0x3b, 0xe2,
];

/// Target or draft model selected by the exact Ferric graph.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
#[repr(u8)]
pub enum Qwen3LinearRoleV1 {
    /// Qwen3-8B target.
    Target8B = 1,
    /// Qwen3-0.6B draft.
    Draft06B = 2,
}

/// Exact geometry relevant to B3 linear operators.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Qwen3LinearGeometryV1 {
    /// Transformer layers.
    pub layers: u16,
    /// Hidden dimension.
    pub hidden: u32,
    /// MLP intermediate dimension.
    pub intermediate: u32,
    /// Query heads.
    pub query_heads: u16,
    /// KV heads.
    pub kv_heads: u16,
    /// Dimension per attention head.
    pub head_dimension: u16,
}

impl Qwen3LinearRoleV1 {
    /// Returns the exact B3 model geometry.
    #[must_use]
    pub const fn geometry(self) -> Qwen3LinearGeometryV1 {
        match self {
            Self::Target8B => Qwen3LinearGeometryV1 {
                layers: 36,
                hidden: 4_096,
                intermediate: 12_288,
                query_heads: 32,
                kv_heads: 8,
                head_dimension: 128,
            },
            Self::Draft06B => Qwen3LinearGeometryV1 {
                layers: 28,
                hidden: 1_024,
                intermediate: 3_072,
                query_heads: 16,
                kv_heads: 8,
                head_dimension: 128,
            },
        }
    }
}

/// B3 execution mode.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
#[repr(u8)]
pub enum Qwen3LinearModeV1 {
    /// Prompt prefill.
    Prefill = 1,
    /// One-token autoregressive decode per sequence.
    Decode = 2,
    /// Target verification or draft proposal span.
    Speculative = 3,
}

/// Complete finite B3 plan-bucket vocabulary.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
#[repr(u8)]
pub enum Qwen3LinearBucketV1 {
    /// Prefill `[S=1,T=128]`.
    PrefillS1T128 = 1,
    /// Prefill `[S=8,T=128]`.
    PrefillS8T128 = 2,
    /// Prefill `[S=1,T=512]`.
    PrefillS1T512 = 3,
    /// Prefill `[S=1,T=2048]`.
    PrefillS1T2048 = 4,
    /// Decode `[S=1,C=8192]`.
    DecodeS1C8192 = 5,
    /// Decode `[S=8,C=8192]`.
    DecodeS8C8192 = 6,
    /// Decode `[S=32,C=8192]`.
    DecodeS32C8192 = 7,
    /// Speculation `[S=1,K=4,C=8192]`.
    SpeculativeS1K4C8192 = 8,
    /// Speculation `[S=8,K=4,C=8192]`.
    SpeculativeS8K4C8192 = 9,
    /// Speculation `[S=1,K=8,C=8192]`.
    SpeculativeS1K8C8192 = 10,
    /// Speculation `[S=1,K=16,C=8192]`.
    SpeculativeS1K16C8192 = 11,
}

/// Exact sequence/token/context dimensions selected by a B3 bucket.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Qwen3LinearBucketDimensionsV1 {
    /// Active sequences.
    pub sequences: u32,
    /// Active tokens per sequence.
    pub active_tokens: u32,
    /// Logical context capacity.
    pub context_tokens: u32,
}

impl Qwen3LinearBucketV1 {
    fn dimensions(
        self,
        role: Qwen3LinearRoleV1,
        mode: Qwen3LinearModeV1,
    ) -> Option<Qwen3LinearBucketDimensionsV1> {
        use Qwen3LinearBucketDimensionsV1 as D;
        use Qwen3LinearModeV1 as M;
        match (mode, self) {
            (M::Prefill, Self::PrefillS1T128) => Some(D {
                sequences: 1,
                active_tokens: 128,
                context_tokens: 128,
            }),
            (M::Prefill, Self::PrefillS8T128) => Some(D {
                sequences: 8,
                active_tokens: 128,
                context_tokens: 128,
            }),
            (M::Prefill, Self::PrefillS1T512) => Some(D {
                sequences: 1,
                active_tokens: 512,
                context_tokens: 512,
            }),
            (M::Prefill, Self::PrefillS1T2048) => Some(D {
                sequences: 1,
                active_tokens: 2_048,
                context_tokens: 2_048,
            }),
            (M::Decode, Self::DecodeS1C8192) => Some(D {
                sequences: 1,
                active_tokens: 1,
                context_tokens: 8_192,
            }),
            (M::Decode, Self::DecodeS8C8192) => Some(D {
                sequences: 8,
                active_tokens: 1,
                context_tokens: 8_192,
            }),
            (M::Decode, Self::DecodeS32C8192) => Some(D {
                sequences: 32,
                active_tokens: 1,
                context_tokens: 8_192,
            }),
            (M::Speculative, Self::SpeculativeS1K4C8192) => Some(D {
                sequences: 1,
                active_tokens: match role {
                    Qwen3LinearRoleV1::Target8B => 5,
                    Qwen3LinearRoleV1::Draft06B => 4,
                },
                context_tokens: 8_192,
            }),
            (M::Speculative, Self::SpeculativeS8K4C8192) => Some(D {
                sequences: 8,
                active_tokens: match role {
                    Qwen3LinearRoleV1::Target8B => 5,
                    Qwen3LinearRoleV1::Draft06B => 4,
                },
                context_tokens: 8_192,
            }),
            (M::Speculative, Self::SpeculativeS1K8C8192) => Some(D {
                sequences: 1,
                active_tokens: match role {
                    Qwen3LinearRoleV1::Target8B => 9,
                    Qwen3LinearRoleV1::Draft06B => 8,
                },
                context_tokens: 8_192,
            }),
            (M::Speculative, Self::SpeculativeS1K16C8192) => Some(D {
                sequences: 1,
                active_tokens: match role {
                    Qwen3LinearRoleV1::Target8B => 17,
                    Qwen3LinearRoleV1::Draft06B => 16,
                },
                context_tokens: 8_192,
            }),
            _ => None,
        }
    }
}

/// Complete B3 graph-operator vocabulary, including non-linear neighbors.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
#[repr(u8)]
pub enum Qwen3B3OperatorV1 {
    /// Token embedding lookup.
    TokenEmbedding = 1,
    /// Input RMSNorm.
    InputRmsNorm = 2,
    /// Query projection.
    QueryProjection = 3,
    /// Key projection.
    KeyProjection = 4,
    /// Value projection.
    ValueProjection = 5,
    /// Query RMSNorm.
    QueryRmsNorm = 6,
    /// Key RMSNorm.
    KeyRmsNorm = 7,
    /// Rotary embedding.
    Rope = 8,
    /// Paged-KV write.
    KvWrite = 9,
    /// Attention.
    Attention = 10,
    /// Output projection fused with attention residual.
    AttentionOutputResidual = 11,
    /// Post-attention RMSNorm.
    PostAttentionRmsNorm = 12,
    /// MLP gate projection.
    GateProjection = 13,
    /// MLP up projection.
    UpProjection = 14,
    /// SwiGLU.
    SwiGlu = 15,
    /// MLP down projection fused with residual.
    DownResidual = 16,
    /// Final RMSNorm.
    FinalRmsNorm = 17,
    /// Vocabulary logits projection.
    LogitsProjection = 18,
    /// Argmax and compact completion.
    ArgmaxCompactCompletion = 19,
}

impl Qwen3B3OperatorV1 {
    const fn is_linear(self) -> bool {
        matches!(
            self,
            Self::QueryProjection
                | Self::KeyProjection
                | Self::ValueProjection
                | Self::AttentionOutputResidual
                | Self::GateProjection
                | Self::UpProjection
                | Self::DownResidual
                | Self::LogitsProjection
        )
    }

    const fn has_residual_epilogue(self) -> bool {
        matches!(self, Self::AttentionOutputResidual | Self::DownResidual)
    }
}

/// Exact B3 graph selection specialized into one inert linear plan.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct Qwen3LinearSelectionV1 {
    /// Target or draft model.
    pub role: Qwen3LinearRoleV1,
    /// Prefill, decode, or speculative execution.
    pub mode: Qwen3LinearModeV1,
    /// Exact finite B3 bucket.
    pub bucket: Qwen3LinearBucketV1,
    /// B3 graph operator.
    pub operator: Qwen3B3OperatorV1,
    /// Exact transformer layer, or [`QWEN3_NO_LAYER_V1`] for logits.
    pub layer: u16,
}

/// Exact flattened GEMM dimensions and original B3 bucket dimensions.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Qwen3LinearDimensionsV1 {
    /// Original sequence/token/context dimensions.
    pub bucket: Qwen3LinearBucketDimensionsV1,
    /// Flattened row count `M = sequences * active_tokens`.
    pub m: u32,
    /// Output-feature count `N`.
    pub n: u32,
    /// Input-feature reduction count `K`.
    pub k: u32,
}

/// Exact host specialization selected from the flattened B3 row count.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum Qwen3LinearImplementationV1 {
    /// `M > 1` uses the general matrix-matrix specialization.
    Gemm = 1,
    /// Only `M == 1` uses the matrix-vector specialization.
    Gemv = 2,
}

/// Scalar storage or arithmetic type named by the numerical contract.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Qwen3LinearScalarTypeV1 {
    /// IEEE binary16 brain floating point encoding.
    Bf16,
    /// IEEE binary32 encoding.
    Fp32,
}

/// Source-level evaluation order retained from the general GEMM route.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Qwen3LinearEvaluationOrderV1 {
    /// Increasing K, separate rounded product/add, then separate alpha/beta epilogue.
    IncreasingKSeparateFp32MulAdd,
}

/// Exact Qwen3 numerical policy at this host/model boundary.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Qwen3LinearNumericalPolicyV1 {
    /// Activation type.
    pub activation: Qwen3LinearScalarTypeV1,
    /// Prepared weight type.
    pub weight: Qwen3LinearScalarTypeV1,
    /// Initial output/residual type consumed by the general route.
    pub initial_output: Qwen3LinearScalarTypeV1,
    /// Accumulator type.
    pub accumulator: Qwen3LinearScalarTypeV1,
    /// Host reference output type; narrowing is outside this slice.
    pub output: Qwen3LinearScalarTypeV1,
    /// Exact FP32 alpha bits.
    pub alpha_bits: u32,
    /// Exact FP32 beta bits.
    pub beta_bits: u32,
    /// Whether initial output is semantically live (`beta = 1`).
    pub residual_epilogue: bool,
    /// Exact source evaluation order.
    pub evaluation_order: Qwen3LinearEvaluationOrderV1,
    /// Whether fused contraction is admitted by the CPU source contract.
    pub fused_contraction: bool,
}

/// Matrix storage interpretation at the adapter boundary.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Qwen3LinearLayoutV1 {
    /// Contiguous row-major logical matrix.
    RowMajorContiguous,
    /// Prepared logical `[K,N]` BF16 matrix from Qwen's row-major `[N,K]` tensor.
    PreparedLogicalKxNFromQwenRowMajorNxK,
}

/// Complete layout and stride contract consumed by the general planner.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Qwen3LinearLayoutContractV1 {
    /// Activation layout `[M,K]`.
    pub activation: Qwen3LinearLayoutV1,
    /// Prepared weight layout `[K,N]`.
    pub weight: Qwen3LinearLayoutV1,
    /// Initial-output layout `[M,N]`.
    pub initial_output: Qwen3LinearLayoutV1,
    /// Output layout `[M,N]`.
    pub output: Qwen3LinearLayoutV1,
    /// General-route `[lda, ldb, ldc]` strides in elements.
    pub strides: [u32; 3],
}

/// Logical memory region in one linear operation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Qwen3LinearMemoryRegionV1 {
    /// BF16 activation A.
    Activation,
    /// Prepared BF16 logical K-by-N weight B.
    PreparedWeight,
    /// FP32 initial C; zero-filled when beta is zero.
    InitialOutput,
    /// FP32 logical output.
    Output,
}

/// Logical access mode.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Qwen3LinearAccessV1 {
    /// Initialized read.
    Read,
    /// Exclusive write.
    Write,
}

/// One ordered effect in the reused general-GEMM model.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Qwen3LinearMemoryEffectV1 {
    /// Logical region.
    pub region: Qwen3LinearMemoryRegionV1,
    /// Read or write.
    pub access: Qwen3LinearAccessV1,
    /// Whether complete initialization is required.
    pub requires_initialized: bool,
    /// Whether unique mutable ownership is required.
    pub requires_exclusive_owner: bool,
}

/// Conditional disjointness and writer premises.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Qwen3LinearAliasContractV1 {
    /// Activation and prepared weight allocations are disjoint.
    pub activation_weight_disjoint: bool,
    /// Activation and output allocations are disjoint.
    pub activation_output_disjoint: bool,
    /// Prepared weight and output allocations are disjoint.
    pub weight_output_disjoint: bool,
    /// Initial output and final output are modeled as non-overlapping slices.
    pub initial_output_disjoint: bool,
    /// Each live output coordinate has one logical writer.
    pub output_coordinate_single_writer: bool,
}

/// Masking and epilogue policy inherited from the general route.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Qwen3LinearTailEpilogueV1 {
    /// A loads are guarded by row and K bounds.
    pub guarded_a_tail: bool,
    /// B loads are guarded by K and column bounds.
    pub guarded_b_tail: bool,
    /// Invalid K slots are zero-filled.
    pub zero_filled_k_tail: bool,
    /// C stores are predicated by M and N bounds.
    pub predicated_output_tail: bool,
    /// Accumulators carry through all `ceil(K/16)` phases.
    pub accumulator_carries_all_phases: bool,
    /// Epilogue is exactly `alpha * AB + beta * C`.
    pub alpha_beta_epilogue: bool,
}

/// Exact resource snapshot produced by the existing checked planner.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Qwen3LinearResourcesV1 {
    /// `[A, B, C]` accessed elements.
    pub storage_elements: [usize; 3],
    /// `[A, B, C]` accessed bytes.
    pub storage_bytes: [u64; 3],
    /// `[ceil(N/16), ceil(M/16), 1]`.
    pub block_counts: [u32; 3],
    /// General route AQL-shape arithmetic, not launch authority.
    pub inert_grid_work_items: [u32; 3],
    /// Wave64 workgroup dimensions.
    pub workgroup_dimensions: [u32; 3],
    /// `ceil(K/16)` reduction phases.
    pub reduction_phases: u32,
    /// Complete output tile count.
    pub total_workgroups: u64,
    /// Single-buffered XOR4 LDS bytes.
    pub lds_bytes: u32,
}

/// Explicitly unavailable production authorities and refinements.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Qwen3LinearAuthorityBoundaryV1 {
    /// Attributed source/MIR authority.
    pub attributed_source_authority: bool,
    /// Kernel IR authority.
    pub kernel_ir_authority: bool,
    /// Artifact authority.
    pub artifact_authority: bool,
    /// Load authority.
    pub load_authority: bool,
    /// Dispatch or launch authority.
    pub launch_authority: bool,
    /// Hardware result authority.
    pub hardware_authority: bool,
    /// Performance evidence authority.
    pub performance_authority: bool,
    /// Machine-level numerical refinement.
    pub machine_refinement: bool,
}

/// Exact Ferric graph source identity carried by every candidate.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Qwen3B3SourceIdentityV1 {
    /// Ferric graph commit.
    pub commit: &'static str,
    /// Ferric graph tree.
    pub tree: &'static str,
    /// Exact graph source blob.
    pub graph_blob: &'static str,
    /// SHA-256 of the graph source.
    pub graph_source_sha256: &'static str,
}

/// Complete inert B3 linear candidate.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Qwen3LinearCandidateV1 {
    /// Static operator-family identity.
    pub family_id: [u8; 32],
    /// Static candidate-schema identity.
    pub candidate_schema_id: [u8; 32],
    /// Static adapter identity for the existing general route.
    pub general_route_id: [u8; 32],
    /// Exact Ferric B3 source identity.
    pub b3_source: Qwen3B3SourceIdentityV1,
    /// Exact selection.
    pub selection: Qwen3LinearSelectionV1,
    /// Exact target/draft geometry.
    pub geometry: Qwen3LinearGeometryV1,
    /// Bucket and flattened GEMM dimensions.
    pub dimensions: Qwen3LinearDimensionsV1,
    /// GEMM or GEMV host specialization.
    pub implementation: Qwen3LinearImplementationV1,
    /// Exact BF16/FP32 numerical policy.
    pub numerical: Qwen3LinearNumericalPolicyV1,
    /// Checked logical layouts and strides.
    pub layout: Qwen3LinearLayoutContractV1,
    /// Ordered effects.
    pub effects: [Qwen3LinearMemoryEffectV1; 4],
    /// Conditional alias/race premises.
    pub alias: Qwen3LinearAliasContractV1,
    /// Tail and epilogue behavior.
    pub tail_epilogue: Qwen3LinearTailEpilogueV1,
    /// Checked exact resources.
    pub resources: Qwen3LinearResourcesV1,
    /// Identity produced by the reused general plan.
    pub general_plan_identity: [u8; 32],
    /// Exact shared twelve-property obligation order.
    pub obligations: [GemmRequiredPropertyV1; 12],
    /// Selection-specific deterministic identity.
    pub selection_identity: [u8; 32],
    /// Explicit non-authority boundary.
    pub authority: Qwen3LinearAuthorityBoundaryV1,
}

/// Selection, structural validation, or general-route planning failure.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Qwen3LinearErrorV1 {
    /// Bucket does not belong to the selected execution mode.
    UnsupportedBucketMode,
    /// B3 operator is not one of the eight linear operators.
    UnsupportedOperator(Qwen3B3OperatorV1),
    /// Per-layer operator has an out-of-range or sentinel layer.
    LayerOutOfBounds,
    /// Logits projection did not use [`QWEN3_NO_LAYER_V1`].
    LogitsLayerMustBeAbsent,
    /// Flattened row or feature arithmetic overflowed.
    ArithmeticOverflow,
    /// Existing general-GEMM planning rejected the exact specialization.
    GeneralPlan(GeneralPlanErrorV1),
    /// Candidate differs from the unique reconstruction for its expected selection.
    NonCanonical,
    /// Existing general-GEMM reference rejected supplied storage.
    GeneralReference(GeneralReferenceErrorV1),
    /// Qwen row-major `[N,K]` weight storage has the wrong exact extent.
    WeightSourceExtent {
        /// Required source elements.
        expected: usize,
        /// Supplied source elements.
        actual: usize,
    },
}

impl fmt::Display for Qwen3LinearErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "Qwen3 B3 linear contract failure: {self:?}")
    }
}

impl Error for Qwen3LinearErrorV1 {}

impl From<GeneralPlanErrorV1> for Qwen3LinearErrorV1 {
    fn from(value: GeneralPlanErrorV1) -> Self {
        Self::GeneralPlan(value)
    }
}

impl From<GeneralReferenceErrorV1> for Qwen3LinearErrorV1 {
    fn from(value: GeneralReferenceErrorV1) -> Self {
        Self::GeneralReference(value)
    }
}

fn dimensions_for_selection(
    selection: Qwen3LinearSelectionV1,
) -> Result<Qwen3LinearDimensionsV1, Qwen3LinearErrorV1> {
    if !selection.operator.is_linear() {
        return Err(Qwen3LinearErrorV1::UnsupportedOperator(selection.operator));
    }
    let geometry = selection.role.geometry();
    if selection.operator == Qwen3B3OperatorV1::LogitsProjection {
        if selection.layer != QWEN3_NO_LAYER_V1 {
            return Err(Qwen3LinearErrorV1::LogitsLayerMustBeAbsent);
        }
    } else if selection.layer >= geometry.layers {
        return Err(Qwen3LinearErrorV1::LayerOutOfBounds);
    }
    let bucket = selection
        .bucket
        .dimensions(selection.role, selection.mode)
        .ok_or(Qwen3LinearErrorV1::UnsupportedBucketMode)?;
    let m = bucket
        .sequences
        .checked_mul(bucket.active_tokens)
        .ok_or(Qwen3LinearErrorV1::ArithmeticOverflow)?;
    let query = u32::from(geometry.query_heads)
        .checked_mul(u32::from(geometry.head_dimension))
        .ok_or(Qwen3LinearErrorV1::ArithmeticOverflow)?;
    let kv = u32::from(geometry.kv_heads)
        .checked_mul(u32::from(geometry.head_dimension))
        .ok_or(Qwen3LinearErrorV1::ArithmeticOverflow)?;
    let (n, k) = match selection.operator {
        Qwen3B3OperatorV1::QueryProjection => (query, geometry.hidden),
        Qwen3B3OperatorV1::KeyProjection | Qwen3B3OperatorV1::ValueProjection => {
            (kv, geometry.hidden)
        }
        Qwen3B3OperatorV1::AttentionOutputResidual => (geometry.hidden, geometry.hidden),
        Qwen3B3OperatorV1::GateProjection | Qwen3B3OperatorV1::UpProjection => {
            (geometry.intermediate, geometry.hidden)
        }
        Qwen3B3OperatorV1::DownResidual => (geometry.hidden, geometry.intermediate),
        Qwen3B3OperatorV1::LogitsProjection => (QWEN3_VOCABULARY_SIZE_V1, geometry.hidden),
        _ => {
            return Err(Qwen3LinearErrorV1::UnsupportedOperator(selection.operator));
        }
    };
    Ok(Qwen3LinearDimensionsV1 { bucket, m, n, k })
}

fn general_plan(
    dimensions: Qwen3LinearDimensionsV1,
    beta_bits: u32,
) -> Result<GeneralGemmPlanV1, Qwen3LinearErrorV1> {
    let target = admit_target_v1(exact_target_v1())
        .expect("the existing exact general-GEMM target must admit itself");
    let request = GeneralGemmRequestV1::new(
        dimensions.m,
        dimensions.n,
        dimensions.k,
        dimensions.k,
        dimensions.n,
        dimensions.n,
        1.0,
        f32::from_bits(beta_bits),
    );
    Ok(plan_general_gemm_v1(
        target,
        request,
        GeneralLaunchLimitsV1::representable(),
    )?)
}

const fn effect(
    region: Qwen3LinearMemoryRegionV1,
    access: Qwen3LinearAccessV1,
    requires_initialized: bool,
    requires_exclusive_owner: bool,
) -> Qwen3LinearMemoryEffectV1 {
    Qwen3LinearMemoryEffectV1 {
        region,
        access,
        requires_initialized,
        requires_exclusive_owner,
    }
}

fn selection_identity(
    selection: Qwen3LinearSelectionV1,
    dimensions: Qwen3LinearDimensionsV1,
    general_plan_identity: &[u8; 32],
    beta_bits: u32,
) -> [u8; 32] {
    let mut digest = Sha256::new();
    digest.update(b"fe2o3.qwen3.b3.linear.selection.v1\0");
    for text in [
        FERRIC_B3_GRAPH_COMMIT_V1,
        FERRIC_B3_GRAPH_TREE_V1,
        FERRIC_B3_GRAPH_BLOB_V1,
        FERRIC_B3_GRAPH_SOURCE_SHA256_V1,
        GENERAL_GEMM_PLAN_SCHEMA_V1,
        GENERAL_GEMM_REFERENCE_SCHEDULE_V1,
    ] {
        digest.update((text.len() as u32).to_le_bytes());
        digest.update(text.as_bytes());
    }
    digest.update([
        selection.role as u8,
        selection.mode as u8,
        selection.bucket as u8,
        selection.operator as u8,
    ]);
    digest.update(selection.layer.to_le_bytes());
    for value in [
        dimensions.bucket.sequences,
        dimensions.bucket.active_tokens,
        dimensions.bucket.context_tokens,
        dimensions.m,
        dimensions.n,
        dimensions.k,
        1.0_f32.to_bits(),
        beta_bits,
    ] {
        digest.update(value.to_le_bytes());
    }
    digest.update(general_plan_identity);
    for property in GEMM_REQUIRED_PROPERTIES_V1 {
        let name = property.as_str();
        digest.update((name.len() as u32).to_le_bytes());
        digest.update(name.as_bytes());
    }
    digest.finalize().into()
}

/// Builds the unique inert general-GEMM specialization for one exact B3 selection.
pub fn exact_qwen3_linear_candidate_v1(
    selection: Qwen3LinearSelectionV1,
) -> Result<Qwen3LinearCandidateV1, Qwen3LinearErrorV1> {
    let geometry = selection.role.geometry();
    let dimensions = dimensions_for_selection(selection)?;
    let residual_epilogue = selection.operator.has_residual_epilogue();
    let beta_bits = if residual_epilogue {
        1.0_f32.to_bits()
    } else {
        0.0_f32.to_bits()
    };
    let plan = general_plan(dimensions, beta_bits)?;
    let general_plan_identity = *plan.identity().as_bytes();
    let selection_identity =
        selection_identity(selection, dimensions, &general_plan_identity, beta_bits);
    Ok(Qwen3LinearCandidateV1 {
        family_id: QWEN3_LINEAR_FAMILY_ID_V1,
        candidate_schema_id: QWEN3_LINEAR_CANDIDATE_SCHEMA_ID_V1,
        general_route_id: QWEN3_LINEAR_GENERAL_ROUTE_ID_V1,
        b3_source: Qwen3B3SourceIdentityV1 {
            commit: FERRIC_B3_GRAPH_COMMIT_V1,
            tree: FERRIC_B3_GRAPH_TREE_V1,
            graph_blob: FERRIC_B3_GRAPH_BLOB_V1,
            graph_source_sha256: FERRIC_B3_GRAPH_SOURCE_SHA256_V1,
        },
        selection,
        geometry,
        dimensions,
        implementation: if dimensions.m == 1 {
            Qwen3LinearImplementationV1::Gemv
        } else {
            Qwen3LinearImplementationV1::Gemm
        },
        numerical: Qwen3LinearNumericalPolicyV1 {
            activation: Qwen3LinearScalarTypeV1::Bf16,
            weight: Qwen3LinearScalarTypeV1::Bf16,
            initial_output: Qwen3LinearScalarTypeV1::Fp32,
            accumulator: Qwen3LinearScalarTypeV1::Fp32,
            output: Qwen3LinearScalarTypeV1::Fp32,
            alpha_bits: 1.0_f32.to_bits(),
            beta_bits,
            residual_epilogue,
            evaluation_order: Qwen3LinearEvaluationOrderV1::IncreasingKSeparateFp32MulAdd,
            fused_contraction: false,
        },
        layout: Qwen3LinearLayoutContractV1 {
            activation: Qwen3LinearLayoutV1::RowMajorContiguous,
            weight: Qwen3LinearLayoutV1::PreparedLogicalKxNFromQwenRowMajorNxK,
            initial_output: Qwen3LinearLayoutV1::RowMajorContiguous,
            output: Qwen3LinearLayoutV1::RowMajorContiguous,
            strides: [dimensions.k, dimensions.n, dimensions.n],
        },
        effects: [
            effect(
                Qwen3LinearMemoryRegionV1::Activation,
                Qwen3LinearAccessV1::Read,
                true,
                false,
            ),
            effect(
                Qwen3LinearMemoryRegionV1::PreparedWeight,
                Qwen3LinearAccessV1::Read,
                true,
                false,
            ),
            effect(
                Qwen3LinearMemoryRegionV1::InitialOutput,
                Qwen3LinearAccessV1::Read,
                true,
                false,
            ),
            effect(
                Qwen3LinearMemoryRegionV1::Output,
                Qwen3LinearAccessV1::Write,
                false,
                true,
            ),
        ],
        alias: Qwen3LinearAliasContractV1 {
            activation_weight_disjoint: true,
            activation_output_disjoint: true,
            weight_output_disjoint: true,
            initial_output_disjoint: true,
            output_coordinate_single_writer: true,
        },
        tail_epilogue: Qwen3LinearTailEpilogueV1 {
            guarded_a_tail: true,
            guarded_b_tail: true,
            zero_filled_k_tail: true,
            predicated_output_tail: true,
            accumulator_carries_all_phases: true,
            alpha_beta_epilogue: true,
        },
        resources: Qwen3LinearResourcesV1 {
            storage_elements: plan.storage().elements(),
            storage_bytes: plan.storage().bytes(),
            block_counts: plan.block_counts(),
            inert_grid_work_items: plan.aql_grid_work_items(),
            workgroup_dimensions: plan.workgroup_dimensions(),
            reduction_phases: plan.reduction_phases(),
            total_workgroups: plan.total_workgroups(),
            lds_bytes: plan.lds_bytes(),
        },
        general_plan_identity,
        obligations: GEMM_REQUIRED_PROPERTIES_V1,
        selection_identity,
        authority: Qwen3LinearAuthorityBoundaryV1 {
            attributed_source_authority: false,
            kernel_ir_authority: false,
            artifact_authority: false,
            load_authority: false,
            launch_authority: false,
            hardware_authority: false,
            performance_authority: false,
            machine_refinement: false,
        },
    })
}

/// Validates every candidate field against an independently expected B3 selection.
pub fn validate_qwen3_linear_candidate_v1(
    candidate: &Qwen3LinearCandidateV1,
    expected_selection: Qwen3LinearSelectionV1,
) -> Result<(), Qwen3LinearErrorV1> {
    let expected = exact_qwen3_linear_candidate_v1(expected_selection)?;
    if candidate != &expected {
        return Err(Qwen3LinearErrorV1::NonCanonical);
    }
    Ok(())
}

/// Reconstructs the existing checked inert general-GEMM plan after validation.
pub fn qwen3_linear_general_plan_v1(
    candidate: &Qwen3LinearCandidateV1,
    expected_selection: Qwen3LinearSelectionV1,
) -> Result<GeneralGemmPlanV1, Qwen3LinearErrorV1> {
    validate_qwen3_linear_candidate_v1(candidate, expected_selection)?;
    general_plan(candidate.dimensions, candidate.numerical.beta_bits)
}

/// Converts exact Qwen row-major `[N,K]` BF16 weights to logical `[K,N]` B storage.
///
/// This pure host layout projection validates the candidate and exact source
/// extent before allocation. It is not a compiler prepacking or device-upload
/// implementation.
pub fn prepare_qwen_linear_weight_v1(
    candidate: &Qwen3LinearCandidateV1,
    expected_selection: Qwen3LinearSelectionV1,
    source_nxk_bits: &[u16],
) -> Result<Vec<u16>, Qwen3LinearErrorV1> {
    validate_qwen3_linear_candidate_v1(candidate, expected_selection)?;
    let n = usize::try_from(candidate.dimensions.n)
        .map_err(|_| Qwen3LinearErrorV1::ArithmeticOverflow)?;
    let k = usize::try_from(candidate.dimensions.k)
        .map_err(|_| Qwen3LinearErrorV1::ArithmeticOverflow)?;
    let extent = n
        .checked_mul(k)
        .ok_or(Qwen3LinearErrorV1::ArithmeticOverflow)?;
    if source_nxk_bits.len() != extent {
        return Err(Qwen3LinearErrorV1::WeightSourceExtent {
            expected: extent,
            actual: source_nxk_bits.len(),
        });
    }
    let mut prepared = vec![0_u16; extent];
    for output_feature in 0..n {
        for input_feature in 0..k {
            prepared[input_feature * n + output_feature] =
                source_nxk_bits[output_feature * k + input_feature];
        }
    }
    Ok(prepared)
}

/// Executes the existing CPU tiled schedule model for one validated B3 specialization.
///
/// Inputs are exact BF16 encodings for contiguous logical `A[M,K]` and prepared
/// `B[K,N]`, plus FP32 `C[M,N]`. This host result grants no GPU authority or
/// machine-level floating-point refinement.
pub fn execute_qwen3_linear_reference_v1(
    candidate: &Qwen3LinearCandidateV1,
    expected_selection: Qwen3LinearSelectionV1,
    activation_bits: &[u16],
    prepared_weight_bits: &[u16],
    initial_output: &[f32],
) -> Result<GeneralReferenceResultV1, Qwen3LinearErrorV1> {
    let plan = qwen3_linear_general_plan_v1(candidate, expected_selection)?;
    Ok(execute_general_reference_v1(
        &plan,
        activation_bits,
        prepared_weight_bits,
        initial_output,
    )?)
}
