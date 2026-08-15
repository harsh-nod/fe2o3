//! Fixed public constants and lane-ownership rules for Phase A.

/// Number of physical and logical lanes in the exact V1 profile.
pub const WAVE64_LANES_V1: usize = 64;

/// Largest admitted absolute integral `f32` input.
///
/// Every admitted prefix is at most `64 * 1024`, so all sums are exact in
/// binary32 regardless of the reduction tree used by the device primitive.
pub const MAX_EXACT_INPUT_MAGNITUDE_V1: f32 = 1024.0;

/// Empty masks are valid and produce positive zero in all three output arrays.
pub const EMPTY_MASK_POLICY_V1: &str =
    "accepted: reduction, inclusive scan, and exclusive scan are +0.0 in every lane";

/// Inactive lanes contribute zero and publish positive zero for every result.
pub const INACTIVE_LANE_OUTPUT_POLICY_V1: &str =
    "inactive lanes contribute +0.0 and publish +0.0 in all outputs";

/// A logical mask never permits physical divergence around a collective.
pub const PHYSICAL_EXECUTION_POLICY_V1: &str =
    "all 64 physical lanes execute every collective convergently";

/// The three output elements exclusively owned by one physical lane.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LaneOutputsV1 {
    /// Physical lane in `0..64`.
    pub lane: usize,
    /// Element exclusively written in the reduction output allocation.
    pub reduction_index: usize,
    /// Element exclusively written in the inclusive-scan output allocation.
    pub inclusive_index: usize,
    /// Element exclusively written in the exclusive-scan output allocation.
    pub exclusive_index: usize,
}

/// Returns whether `lane` is selected by the explicit 64-bit mask.
pub const fn lane_is_active_v1(active_mask: u64, lane: usize) -> bool {
    lane < WAVE64_LANES_V1 && active_mask & (1_u64 << lane) != 0
}

/// Returns the identity output-ownership map for a physical Wave64 lane.
pub const fn lane_outputs_v1(lane: usize) -> Option<LaneOutputsV1> {
    if lane < WAVE64_LANES_V1 {
        Some(LaneOutputsV1 {
            lane,
            reduction_index: lane,
            inclusive_index: lane,
            exclusive_index: lane,
        })
    } else {
        None
    }
}
