//! Exact bounded shape, routing, scheduling, and assurance contracts.

pub use fe2o3_moe_top2_v1::{
    DROP_ROUTE_V1, MOE_EXPERT_CAPACITY_V1, MOE_EXPERTS_V1, MOE_ROUTES_PER_TOKEN_V1, MOE_ROUTES_V1,
    MOE_TOKENS_V1,
};

/// Hidden activation width consumed by every expert.
pub const MOE_EXPERT_INPUT_WIDTH_V1: usize = 16;
/// Output width produced by every expert and by combine.
pub const MOE_EXPERT_OUTPUT_WIDTH_V1: usize = 16;
/// Row count in the reused exact tiled GEMM profile.
pub const MOE_EXPERT_TILE_ROWS_V1: usize = 16;
/// Elements in one row-major 16x16 tile.
pub const MOE_EXPERT_TILE_ELEMENTS_V1: usize = 256;
/// Number of input activation elements in token-major order.
pub const MOE_TOKEN_ACTIVATION_ELEMENTS_V1: usize = MOE_TOKENS_V1 * MOE_EXPERT_INPUT_WIDTH_V1;
/// Number of BF16 weight elements across all experts.
pub const MOE_EXPERT_WEIGHT_ELEMENTS_V1: usize = MOE_EXPERTS_V1 * MOE_EXPERT_TILE_ELEMENTS_V1;
/// Number of compact route-output elements.
pub const MOE_COMPACT_OUTPUT_ELEMENTS_V1: usize = MOE_ROUTES_V1 * MOE_EXPERT_OUTPUT_WIDTH_V1;
/// Number of final token-major output elements.
pub const MOE_COMBINED_OUTPUT_ELEMENTS_V1: usize = MOE_TOKENS_V1 * MOE_EXPERT_OUTPUT_WIDTH_V1;
/// Exactly four GEMM launches are host-scheduled, one per expert.
pub const MOE_EXPERT_GEMM_DISPATCHES_V1: usize = MOE_EXPERTS_V1;
/// Physical workgroup for each 16x16x16 expert GEMM.
pub const MOE_EXPERT_GEMM_WORKGROUP_V1: [u32; 3] = [64, 1, 1];
/// Physical workgroup for the 128-element combine launch.
pub const MOE_EXPERT_COMBINE_WORKGROUP_V1: [u32; 3] = [64, 1, 1];
/// Two workgroups cover the exact 128 combine outputs.
pub const MOE_EXPERT_COMBINE_GRID_V1: [u32; 3] = [2, 1, 1];

/// Explicit route-weight contract layered over the routing-only public ABI.
pub const MOE_ROUTE_WEIGHT_POLICY_V1: &str = "caller supplies 16 finite nonnegative f32 weights in token-major rank-minor route-ID order; each token pair sums exactly to 1.0; dropped routes contribute zero without renormalization";
/// Exact host scheduling rule.
pub const MOE_EXPERT_SCHEDULE_POLICY_V1: &str = "compact accepted routes into zero-padded per-expert 16x16 BF16 tiles; launch four independent exact 16x16x16 GEMMs; inverse-permute compact rows; combine in rank order";
/// Exact finite BF16 input policy for activations and expert matrices.
pub const MOE_EXPERT_BF16_POLICY_V1: &str =
    "all token activations and expert weights must be finite BF16 values";
/// Whether compiler-authenticated source-to-IR lowering exists.
pub const MOE_EXPERT_SOURCE_TO_IR_SUPPORTED_V1: bool = false;
/// Whether exact LLVM/finalizer/runtime support exists.
pub const MOE_EXPERT_EXECUTION_SUPPORTED_V1: bool = false;
/// Current fail-closed implementation boundary.
pub const MOE_EXPERT_SOURCE_BLOCKER_V1: &str = "the two attributed kernels have no authenticated MIR-to-Kernel-IR profiles or protected host/runtime joins";

/// Exact fixed profile used by the source, host schedule, oracle, and proof.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MoeExpertProfileV1 {
    /// Token count.
    pub tokens: usize,
    /// Expert count.
    pub experts: usize,
    /// Routes selected per token.
    pub routes_per_token: usize,
    /// Maximum accepted routes per expert.
    pub capacity: usize,
    /// Expert input width.
    pub input_width: usize,
    /// Expert output width.
    pub output_width: usize,
    /// Padded GEMM tile row count.
    pub tile_rows: usize,
    /// Exact target processor.
    pub processor: &'static str,
    /// Exact target features.
    pub target_features: &'static str,
}

/// Sole admitted profile.
pub const EXACT_MOE_EXPERT_PROFILE_V1: MoeExpertProfileV1 = MoeExpertProfileV1 {
    tokens: MOE_TOKENS_V1,
    experts: MOE_EXPERTS_V1,
    routes_per_token: MOE_ROUTES_PER_TOKEN_V1,
    capacity: MOE_EXPERT_CAPACITY_V1,
    input_width: MOE_EXPERT_INPUT_WIDTH_V1,
    output_width: MOE_EXPERT_OUTPUT_WIDTH_V1,
    tile_rows: MOE_EXPERT_TILE_ROWS_V1,
    processor: "gfx942",
    target_features: "+wavefrontsize64,-xnack",
};

/// Returns the route ID for an exact token/rank coordinate.
pub const fn expert_route_id_v1(token: usize, rank: usize) -> Option<usize> {
    if token < MOE_TOKENS_V1 && rank < MOE_ROUTES_PER_TOKEN_V1 {
        Some(token * MOE_ROUTES_PER_TOKEN_V1 + rank)
    } else {
        None
    }
}

/// Returns one token-major activation index.
pub const fn token_activation_index_v1(token: usize, depth: usize) -> Option<usize> {
    if token < MOE_TOKENS_V1 && depth < MOE_EXPERT_INPUT_WIDTH_V1 {
        Some(token * MOE_EXPERT_INPUT_WIDTH_V1 + depth)
    } else {
        None
    }
}

/// Returns one expert-major row-major weight index.
pub const fn expert_weight_index_v1(expert: usize, depth: usize, output: usize) -> Option<usize> {
    if expert < MOE_EXPERTS_V1
        && depth < MOE_EXPERT_INPUT_WIDTH_V1
        && output < MOE_EXPERT_OUTPUT_WIDTH_V1
    {
        Some(expert * MOE_EXPERT_TILE_ELEMENTS_V1 + depth * MOE_EXPERT_OUTPUT_WIDTH_V1 + output)
    } else {
        None
    }
}

/// Returns one expert-major row-major tile index.
pub const fn expert_tile_index_v1(expert: usize, row: usize, column: usize) -> Option<usize> {
    if expert < MOE_EXPERTS_V1
        && row < MOE_EXPERT_TILE_ROWS_V1
        && column < MOE_EXPERT_OUTPUT_WIDTH_V1
    {
        Some(expert * MOE_EXPERT_TILE_ELEMENTS_V1 + row * MOE_EXPERT_OUTPUT_WIDTH_V1 + column)
    } else {
        None
    }
}

/// Returns one slot-major compact output index.
pub const fn compact_output_index_v1(slot: usize, output: usize) -> Option<usize> {
    if slot < MOE_ROUTES_V1 && output < MOE_EXPERT_OUTPUT_WIDTH_V1 {
        Some(slot * MOE_EXPERT_OUTPUT_WIDTH_V1 + output)
    } else {
        None
    }
}

/// Returns one token-major combined output index.
pub const fn combined_output_index_v1(token: usize, output: usize) -> Option<usize> {
    if token < MOE_TOKENS_V1 && output < MOE_EXPERT_OUTPUT_WIDTH_V1 {
        Some(token * MOE_EXPERT_OUTPUT_WIDTH_V1 + output)
    } else {
        None
    }
}

/// Inert expected proof values for fail-closed tests only.
///
/// This copyable descriptor authenticates nothing and cannot mint or join a
/// Verus receipt, compiler receipt, artifact, load, dispatch, or launch token.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MoeExpertExpectedEvidenceV1 {
    /// Expected proof-source digest.
    pub proof_source: [u8; 32],
    /// Expected exact-kernel source digest.
    pub kernel_source: [u8; 32],
    /// Expected pinned Verus executable digest.
    pub verus_executable: [u8; 32],
    /// Expected complete Verus release-closure manifest digest.
    pub verus_closure_manifest: [u8; 32],
    /// Expected negative-source manifest digest.
    pub negative_manifest: [u8; 32],
    /// Expected pinned-Verus transcript digest.
    pub transcript: [u8; 32],
}

impl MoeExpertExpectedEvidenceV1 {
    /// Returns the exact expected values used by fail-closed evidence tests.
    pub const fn exact() -> Self {
        Self {
            proof_source: [
                0x61, 0x7e, 0x67, 0x41, 0xc5, 0xf1, 0x41, 0x5a, 0x8e, 0x79, 0x2e, 0x5e, 0x36, 0xe3,
                0x52, 0x6c, 0x04, 0xba, 0x18, 0x90, 0x34, 0x38, 0xe3, 0xaf, 0x17, 0x8b, 0xb1, 0x07,
                0x76, 0x63, 0x83, 0xd1,
            ],
            kernel_source: [
                0x5a, 0xe3, 0xcf, 0xe5, 0x94, 0x94, 0x34, 0x78, 0x38, 0xfe, 0x41, 0x60, 0xc9, 0x9c,
                0x5b, 0x67, 0x96, 0x86, 0x42, 0xd2, 0x65, 0x50, 0xc0, 0x1e, 0x27, 0xd2, 0xee, 0x12,
                0x47, 0x51, 0x1a, 0xec,
            ],
            verus_executable: [
                0xad, 0x26, 0x69, 0xf5, 0x79, 0xd8, 0x98, 0xed, 0xe5, 0x3f, 0x2b, 0xf8, 0x4e, 0x80,
                0xa1, 0xda, 0xf4, 0xe3, 0x57, 0x87, 0x39, 0xb0, 0xf5, 0x80, 0x7e, 0xf2, 0x09, 0xa0,
                0xc9, 0xf3, 0x82, 0xdd,
            ],
            verus_closure_manifest: [
                0xd2, 0x8d, 0xf3, 0xfb, 0x5e, 0x0d, 0x74, 0x76, 0x37, 0x54, 0x39, 0x33, 0xdf, 0xc3,
                0x8c, 0xff, 0x45, 0x57, 0x6d, 0xa9, 0xb9, 0x20, 0xd7, 0x55, 0xb4, 0xb7, 0xe9, 0x19,
                0xe4, 0x7a, 0x60, 0x19,
            ],
            negative_manifest: [
                0xb4, 0x69, 0x02, 0x71, 0xf2, 0x53, 0xf4, 0x2b, 0xac, 0xd3, 0x87, 0x69, 0x89, 0x30,
                0x06, 0x4a, 0x48, 0xf1, 0x91, 0xdb, 0x5a, 0xf5, 0x74, 0x3d, 0x4c, 0xad, 0x8b, 0xa4,
                0x90, 0x84, 0xef, 0xec,
            ],
            transcript: [
                0x00, 0xe3, 0x84, 0x23, 0x64, 0x23, 0xde, 0x39, 0xf1, 0xaa, 0xdd, 0x51, 0x6a, 0x9f,
                0x40, 0xac, 0x1d, 0x50, 0x64, 0x5c, 0xd1, 0xd4, 0x9d, 0x26, 0x7f, 0xf4, 0xf7, 0xfa,
                0xa4, 0x73, 0x46, 0xcc,
            ],
        }
    }

    /// Reports that this descriptor grants no authority.
    pub const fn authenticates_anything(self) -> bool {
        false
    }

    /// Reports that no source-to-machine refinement is claimed.
    pub const fn proves_source_to_machine_refinement(self) -> bool {
        false
    }

    /// Reports that no generalized race-freedom result is claimed.
    pub const fn proves_generalized_race_freedom(self) -> bool {
        false
    }

    /// Reports that no protected GPU execution is claimed.
    pub const fn proves_protected_gpu_execution(self) -> bool {
        false
    }
}
