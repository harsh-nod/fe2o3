//! Ordinary attributed Rust source for one exact deterministic MoE router.
//!
//! This is the complete fixed routing algorithm, not explanatory pseudocode
//! and not a `macro_rules!` facade. Phase A does not claim compiler or hardware
//! support for the source.

#![allow(missing_docs)] // The V1 kernel macro emits an undocumented helper module.

use fe2o3_device::{DisjointSlice, GridExclusive, GridLeader, kernel, thread};

use crate::contract::{
    DROP_ROUTE_V1, MOE_EXPERT_CAPACITY_V1, MOE_EXPERTS_V1, MOE_LOGIT_ELEMENTS_V1,
    MOE_ROUTES_PER_TOKEN_V1, MOE_ROUTES_V1, MOE_TOKENS_V1,
};

/// Exact workgroup dimensions required by the attributed source.
pub const MOE_TOP2_WORKGROUP_V1: [u32; 3] = [64, 1, 1];
/// Exact grid dimensions required by the fixed source profile.
pub const MOE_TOP2_GRID_V1: [u32; 3] = [1, 1, 1];
/// Whether source-authenticated compiler lowering exists for this profile.
pub const MOE_TOP2_SOURCE_LOWERING_SUPPORTED_V1: bool = false;
/// Current authority boundary after Phase A.
pub const MOE_TOP2_SOURCE_BLOCKER_V1: &str = "the exact scan/staging source has no authenticated MIR-to-Kernel-IR compiler profile; finalizer, runtime, proof, and hardware phases are pending";

fn candidate_precedes_v1(
    candidate_score: f32,
    candidate_expert: usize,
    incumbent_score: f32,
    incumbent_expert: usize,
) -> bool {
    candidate_score > incumbent_score
        || (candidate_score == incumbent_score && candidate_expert < incumbent_expert)
}

fn select_top2_v1(logits: &[f32], token: usize) -> [u32; 2] {
    let mut best = usize::MAX;
    let mut second = usize::MAX;
    let mut expert = 0;
    while expert < MOE_EXPERTS_V1 {
        let score = logits[token * MOE_EXPERTS_V1 + expert];
        if best == usize::MAX
            || candidate_precedes_v1(score, expert, logits[token * MOE_EXPERTS_V1 + best], best)
        {
            second = best;
            best = expert;
        } else if second == usize::MAX
            || candidate_precedes_v1(
                score,
                expert,
                logits[token * MOE_EXPERTS_V1 + second],
                second,
            )
        {
            second = expert;
        }
        expert += 1;
    }
    [best as u32, second as u32]
}

fn logits_are_finite_v1(logits: &[f32]) -> bool {
    let mut index = 0;
    while index < MOE_LOGIT_ELEMENTS_V1 {
        if !logits[index].is_finite() {
            return false;
        }
        index += 1;
    }
    true
}

fn write_value_v1(
    output: &mut DisjointSlice<u32, GridExclusive>,
    leader: &GridLeader,
    index: usize,
    value: u32,
) {
    let Some(slot) = output.get_mut_exclusive(leader, index) else {
        fe2o3_device::trap();
        return;
    };
    *slot = value;
}

/// Selects and stably packs top-2 routes for fixed `T8/E4/K2/C4` logits.
///
/// Input is contiguous token-major `[8][4]` finite `f32`. Equal scores prefer
/// the lower expert ID. Route IDs are token-major then rank-major. Per-expert
/// requested counts are scanned after clamping each count to capacity four.
/// For each expert, the first four routes in route-ID order receive compact
/// slots; later routes are dropped. `permutation[slot] = route_id` and
/// `inverse[route_id] = slot` for accepted routes. Dropped and unused entries
/// are `u32::MAX`. Lane zero stages the complete result before committing it;
/// physical lanes 1 through 63 are inactive and perform no writes.
#[kernel(
    typed,
    namespace = "4180ef61545684e646bd5227333e7514d22a2d379d7d657397df4d41f7a192d1",
    launch(required = [64, 1, 1], max = [64, 1, 1]),
    control_flow(loop_bounds(8, 4, 16, 16, 4))
)]
#[allow(clippy::too_many_arguments)]
pub fn moe_top2_route_f32_t8_e4_k2_c4_v1(
    logits: &[f32],
    mut top2_experts: DisjointSlice<u32, GridExclusive>,
    mut requested_counts: DisjointSlice<u32, GridExclusive>,
    mut admitted_counts: DisjointSlice<u32, GridExclusive>,
    mut expert_offsets: DisjointSlice<u32, GridExclusive>,
    mut route_slots: DisjointSlice<u32, GridExclusive>,
    mut permutation: DisjointSlice<u32, GridExclusive>,
    mut inverse: DisjointSlice<u32, GridExclusive>,
) {
    let leader;
    if let Some(current_leader) = thread::grid_leader() {
        leader = current_leader;
    } else {
        return;
    }
    if logits.len() != MOE_LOGIT_ELEMENTS_V1
        || top2_experts.len() != MOE_ROUTES_V1
        || requested_counts.len() != MOE_EXPERTS_V1
        || admitted_counts.len() != MOE_EXPERTS_V1
        || expert_offsets.len() != MOE_EXPERTS_V1 + 1
        || route_slots.len() != MOE_ROUTES_V1
        || permutation.len() != MOE_ROUTES_V1
        || inverse.len() != MOE_ROUTES_V1
        || !logits_are_finite_v1(logits)
    {
        fe2o3_device::trap();
        return;
    }

    let mut staged_top2 = [0_u32; MOE_ROUTES_V1];
    let mut staged_requested = [0_u32; MOE_EXPERTS_V1];
    let mut token = 0;
    while token < MOE_TOKENS_V1 {
        let selected = select_top2_v1(logits, token);
        let route_base = token * MOE_ROUTES_PER_TOKEN_V1;
        staged_top2[route_base] = selected[0];
        staged_top2[route_base + 1] = selected[1];
        staged_requested[selected[0] as usize] += 1;
        staged_requested[selected[1] as usize] += 1;
        token += 1;
    }

    let mut staged_admitted = [0_u32; MOE_EXPERTS_V1];
    let mut staged_offsets = [0_u32; MOE_EXPERTS_V1 + 1];
    let mut expert = 0;
    while expert < MOE_EXPERTS_V1 {
        staged_admitted[expert] = if staged_requested[expert] > MOE_EXPERT_CAPACITY_V1 as u32 {
            MOE_EXPERT_CAPACITY_V1 as u32
        } else {
            staged_requested[expert]
        };
        staged_offsets[expert + 1] = staged_offsets[expert] + staged_admitted[expert];
        expert += 1;
    }

    let mut staged_slots = [DROP_ROUTE_V1; MOE_ROUTES_V1];
    let mut staged_permutation = [DROP_ROUTE_V1; MOE_ROUTES_V1];
    let mut staged_inverse = [DROP_ROUTE_V1; MOE_ROUTES_V1];
    let mut seen = [0_u32; MOE_EXPERTS_V1];
    let mut route = 0;
    while route < MOE_ROUTES_V1 {
        let route_expert = staged_top2[route] as usize;
        let stable_rank = seen[route_expert];
        seen[route_expert] += 1;
        if stable_rank < MOE_EXPERT_CAPACITY_V1 as u32 {
            let slot = staged_offsets[route_expert] + stable_rank;
            staged_slots[route] = slot;
            staged_permutation[slot as usize] = route as u32;
            staged_inverse[route] = slot;
        }
        route += 1;
    }

    let mut index = 0;
    while index < MOE_ROUTES_V1 {
        write_value_v1(&mut top2_experts, &leader, index, staged_top2[index]);
        write_value_v1(&mut route_slots, &leader, index, staged_slots[index]);
        write_value_v1(&mut permutation, &leader, index, staged_permutation[index]);
        write_value_v1(&mut inverse, &leader, index, staged_inverse[index]);
        index += 1;
    }
    index = 0;
    while index < MOE_EXPERTS_V1 {
        write_value_v1(
            &mut requested_counts,
            &leader,
            index,
            staged_requested[index],
        );
        write_value_v1(&mut admitted_counts, &leader, index, staged_admitted[index]);
        write_value_v1(&mut expert_offsets, &leader, index, staged_offsets[index]);
        index += 1;
    }
    write_value_v1(
        &mut expert_offsets,
        &leader,
        MOE_EXPERTS_V1,
        staged_offsets[MOE_EXPERTS_V1],
    );
}
