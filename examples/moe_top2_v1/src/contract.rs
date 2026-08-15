//! Frozen shape, layout, launch, tie-break, and overflow contracts.

/// Fixed token count.
pub const MOE_TOKENS_V1: usize = 8;
/// Fixed expert count.
pub const MOE_EXPERTS_V1: usize = 4;
/// Number of distinct experts selected per token.
pub const MOE_ROUTES_PER_TOKEN_V1: usize = 2;
/// Number of token/rank route records.
pub const MOE_ROUTES_V1: usize = MOE_TOKENS_V1 * MOE_ROUTES_PER_TOKEN_V1;
/// Number of token-major input logits.
pub const MOE_LOGIT_ELEMENTS_V1: usize = MOE_TOKENS_V1 * MOE_EXPERTS_V1;
/// Maximum accepted routes per expert.
pub const MOE_EXPERT_CAPACITY_V1: usize = 4;
/// Physical lanes in the exact gfx942 Wave64 launch.
pub const MOE_WAVE_LANES_V1: usize = 64;
/// Sentinel for a dropped or absent route/slot.
pub const DROP_ROUTE_V1: u32 = u32::MAX;

/// Exact finite-input rule.
pub const FINITE_LOGIT_POLICY_V1: &str =
    "all 32 f32 logits must be finite; NaN or infinity traps before any output write";
/// Exact deterministic ordering rule.
pub const TIE_BREAK_POLICY_V1: &str = "higher logit first; equal logits prefer the lower expert id";
/// Exact stable capacity rule.
pub const OVERFLOW_POLICY_V1: &str = "token-major then rank-major routes; first four requests per expert are accepted; later requests are dropped";
/// Exact output initialization and tail rule.
pub const TAIL_POLICY_V1: &str =
    "permutation tail and every dropped slot/inverse entry are u32::MAX; lanes 1..63 write nothing";

/// Input layout admitted by the exact profile.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LayoutV1 {
    /// Contiguous token-major `[token][expert]` logits.
    TokenMajorContiguous,
    /// Deliberately unsupported expert-major layout.
    ExpertMajorContiguous,
}

/// Total tie-break policy admitted by the exact profile.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TieBreakPolicyV1 {
    /// Descending logit, then ascending expert ID.
    LowerExpertIdWins,
    /// Deliberately unsupported descending expert-ID tie-break.
    HigherExpertIdWins,
}

/// Capacity-overflow policy admitted by the exact profile.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OverflowPolicyV1 {
    /// Keep the stable prefix for each expert and drop its remaining routes.
    StablePrefixDrop,
    /// Deliberately unsupported replacement policy.
    ReplaceLowestAccepted,
}

/// Complete identity-bearing fixed routing profile.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MoeTop2ProfileV1 {
    /// AMDGPU processor name.
    pub processor: &'static str,
    /// Required target feature identity.
    pub target_features: &'static str,
    /// Physical wave width.
    pub wave_width: usize,
    /// Token count.
    pub tokens: usize,
    /// Expert count.
    pub experts: usize,
    /// Experts selected per token.
    pub top_k: usize,
    /// Accepted routes per expert.
    pub expert_capacity: usize,
    /// Input-logit layout.
    pub layout: LayoutV1,
    /// Total tie-break policy.
    pub tie_break: TieBreakPolicyV1,
    /// Overflow/drop policy.
    pub overflow: OverflowPolicyV1,
    /// Workgroup dimensions.
    pub workgroup: [u32; 3],
    /// Grid dimensions in workgroups.
    pub grid: [u32; 3],
}

/// The only profile admitted by this Phase A crate.
pub const EXACT_PROFILE_V1: MoeTop2ProfileV1 = MoeTop2ProfileV1 {
    processor: "gfx942",
    target_features: "+wavefrontsize64,-xnack",
    wave_width: MOE_WAVE_LANES_V1,
    tokens: MOE_TOKENS_V1,
    experts: MOE_EXPERTS_V1,
    top_k: MOE_ROUTES_PER_TOKEN_V1,
    expert_capacity: MOE_EXPERT_CAPACITY_V1,
    layout: LayoutV1::TokenMajorContiguous,
    tie_break: TieBreakPolicyV1::LowerExpertIdWins,
    overflow: OverflowPolicyV1::StablePrefixDrop,
    workgroup: [64, 1, 1],
    grid: [1, 1, 1],
};

/// Exact profile-admission failure.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProfileMismatchV1 {
    /// Processor or feature identity drifted.
    Target,
    /// Wave width, workgroup, or grid drifted.
    Launch,
    /// Token, expert, top-k, or capacity shape drifted.
    Shape,
    /// Input layout drifted.
    Layout,
    /// Tie-break policy drifted.
    TieBreak,
    /// Overflow policy drifted.
    Overflow,
}

/// Admits only [`EXACT_PROFILE_V1`].
pub fn validate_profile_v1(profile: MoeTop2ProfileV1) -> Result<(), ProfileMismatchV1> {
    if !str_equal(profile.processor, EXACT_PROFILE_V1.processor)
        || !str_equal(profile.target_features, EXACT_PROFILE_V1.target_features)
    {
        return Err(ProfileMismatchV1::Target);
    }
    if profile.wave_width != EXACT_PROFILE_V1.wave_width
        || profile.workgroup != EXACT_PROFILE_V1.workgroup
        || profile.grid != EXACT_PROFILE_V1.grid
    {
        return Err(ProfileMismatchV1::Launch);
    }
    if profile.tokens != EXACT_PROFILE_V1.tokens
        || profile.experts != EXACT_PROFILE_V1.experts
        || profile.top_k != EXACT_PROFILE_V1.top_k
        || profile.expert_capacity != EXACT_PROFILE_V1.expert_capacity
    {
        return Err(ProfileMismatchV1::Shape);
    }
    if !matches!(profile.layout, LayoutV1::TokenMajorContiguous) {
        return Err(ProfileMismatchV1::Layout);
    }
    if !matches!(profile.tie_break, TieBreakPolicyV1::LowerExpertIdWins) {
        return Err(ProfileMismatchV1::TieBreak);
    }
    if !matches!(profile.overflow, OverflowPolicyV1::StablePrefixDrop) {
        return Err(ProfileMismatchV1::Overflow);
    }
    Ok(())
}

fn str_equal(left: &str, right: &str) -> bool {
    let left = left.as_bytes();
    let right = right.as_bytes();
    if left.len() != right.len() {
        return false;
    }
    let mut index = 0;
    while index < left.len() {
        if left[index] != right[index] {
            return false;
        }
        index += 1;
    }
    true
}

/// Returns the exact grid and workgroup dimensions.
pub const fn exact_launch_v1() -> ([u32; 3], [u32; 3]) {
    (EXACT_PROFILE_V1.grid, EXACT_PROFILE_V1.workgroup)
}

/// Returns the token-major logit index for a valid coordinate.
pub const fn logit_index_v1(token: usize, expert: usize) -> Option<usize> {
    if token < MOE_TOKENS_V1 && expert < MOE_EXPERTS_V1 {
        Some(token * MOE_EXPERTS_V1 + expert)
    } else {
        None
    }
}

/// Returns the token-major, rank-minor route ID for a valid coordinate.
pub const fn route_id_v1(token: usize, rank: usize) -> Option<usize> {
    if token < MOE_TOKENS_V1 && rank < MOE_ROUTES_PER_TOKEN_V1 {
        Some(token * MOE_ROUTES_PER_TOKEN_V1 + rank)
    } else {
        None
    }
}
