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
    /// Expected pinned-Verus transcript digest.
    pub transcript: [u8; 32],
}

impl MoeExpertExpectedEvidenceV1 {
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
