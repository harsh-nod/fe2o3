//! Independent CPU reference for the exact routing profile.

use core::cmp::Ordering;

use crate::contract::{
    DROP_ROUTE_V1, MOE_EXPERT_CAPACITY_V1, MOE_EXPERTS_V1, MOE_LOGIT_ELEMENTS_V1,
    MOE_ROUTES_PER_TOKEN_V1, MOE_ROUTES_V1, MOE_TOKENS_V1,
};

/// All observable outputs of the fixed router.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RoutingOutputsV1 {
    /// Selected expert IDs in token-major, rank-minor order.
    pub top2_experts: [u32; MOE_ROUTES_V1],
    /// Number of selected routes requesting each expert before capacity.
    pub requested_counts: [u32; MOE_EXPERTS_V1],
    /// Per-expert request counts clamped to capacity.
    pub admitted_counts: [u32; MOE_EXPERTS_V1],
    /// Exclusive scan of admitted counts, including the terminal total.
    pub expert_offsets: [u32; MOE_EXPERTS_V1 + 1],
    /// Compact slot for each route ID, or [`DROP_ROUTE_V1`].
    pub route_slots: [u32; MOE_ROUTES_V1],
    /// Route ID occupying each compact slot, followed by sentinel tail entries.
    pub permutation: [u32; MOE_ROUTES_V1],
    /// Compact slot indexed by route ID, or [`DROP_ROUTE_V1`].
    pub inverse: [u32; MOE_ROUTES_V1],
}

impl RoutingOutputsV1 {
    /// Constructs an output state with every field initialized to `value`.
    pub const fn filled(value: u32) -> Self {
        Self {
            top2_experts: [value; MOE_ROUTES_V1],
            requested_counts: [value; MOE_EXPERTS_V1],
            admitted_counts: [value; MOE_EXPERTS_V1],
            expert_offsets: [value; MOE_EXPERTS_V1 + 1],
            route_slots: [value; MOE_ROUTES_V1],
            permutation: [value; MOE_ROUTES_V1],
            inverse: [value; MOE_ROUTES_V1],
        }
    }
}

/// Fail-closed CPU-oracle rejection.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MoeOracleErrorV1 {
    /// The logit allocation did not match the exact fixed extent.
    WrongLogitLength {
        /// Required element count.
        expected: usize,
        /// Supplied element count.
        actual: usize,
    },
    /// One input logit was NaN or infinity.
    NonFiniteLogit {
        /// Token coordinate.
        token: usize,
        /// Expert coordinate.
        expert: usize,
    },
}

fn descending_score_then_expert(
    logits: &[f32],
    token: usize,
    left: usize,
    right: usize,
) -> Ordering {
    let left_score = logits[token * MOE_EXPERTS_V1 + left];
    let right_score = logits[token * MOE_EXPERTS_V1 + right];
    match right_score.partial_cmp(&left_score) {
        Some(Ordering::Equal) => left.cmp(&right),
        Some(ordering) => ordering,
        None => Ordering::Equal,
    }
}

fn independent_top2(logits: &[f32], token: usize) -> [u32; 2] {
    let mut experts = [0_usize, 1, 2, 3];
    experts.sort_by(|left, right| descending_score_then_expert(logits, token, *left, *right));
    [experts[0] as u32, experts[1] as u32]
}

/// Computes the independent deterministic top-2 routing reference.
///
/// The reference uses host sorting rather than the kernel's running top-2
/// selection. All inputs and all staged structural outputs are validated
/// before `output` is replaced, so an error leaves caller-visible state
/// unchanged.
pub fn moe_top2_oracle_v1(
    logits: &[f32],
    output: &mut RoutingOutputsV1,
) -> Result<(), MoeOracleErrorV1> {
    if logits.len() != MOE_LOGIT_ELEMENTS_V1 {
        return Err(MoeOracleErrorV1::WrongLogitLength {
            expected: MOE_LOGIT_ELEMENTS_V1,
            actual: logits.len(),
        });
    }
    for token in 0..MOE_TOKENS_V1 {
        for expert in 0..MOE_EXPERTS_V1 {
            if !logits[token * MOE_EXPERTS_V1 + expert].is_finite() {
                return Err(MoeOracleErrorV1::NonFiniteLogit { token, expert });
            }
        }
    }

    let mut staged = RoutingOutputsV1::filled(DROP_ROUTE_V1);
    staged.requested_counts = [0; MOE_EXPERTS_V1];
    staged.admitted_counts = [0; MOE_EXPERTS_V1];
    staged.expert_offsets = [0; MOE_EXPERTS_V1 + 1];

    for token in 0..MOE_TOKENS_V1 {
        let selected = independent_top2(logits, token);
        let route_base = token * MOE_ROUTES_PER_TOKEN_V1;
        staged.top2_experts[route_base] = selected[0];
        staged.top2_experts[route_base + 1] = selected[1];
        staged.requested_counts[selected[0] as usize] += 1;
        staged.requested_counts[selected[1] as usize] += 1;
    }
    for expert in 0..MOE_EXPERTS_V1 {
        staged.admitted_counts[expert] =
            staged.requested_counts[expert].min(MOE_EXPERT_CAPACITY_V1 as u32);
        staged.expert_offsets[expert + 1] =
            staged.expert_offsets[expert] + staged.admitted_counts[expert];
    }

    let mut stable_ranks = [0_u32; MOE_EXPERTS_V1];
    for route in 0..MOE_ROUTES_V1 {
        let expert = staged.top2_experts[route] as usize;
        let rank = stable_ranks[expert];
        stable_ranks[expert] += 1;
        if rank < MOE_EXPERT_CAPACITY_V1 as u32 {
            let slot = staged.expert_offsets[expert] + rank;
            staged.route_slots[route] = slot;
            staged.permutation[slot as usize] = route as u32;
            staged.inverse[route] = slot;
        }
    }

    *output = staged;
    Ok(())
}
