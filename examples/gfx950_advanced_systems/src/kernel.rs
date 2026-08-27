//! Complete safe Rust kernel source for the bounded systems profiles.

#![allow(missing_docs)] // The kernel macro emits helper modules.
#![cfg_attr(target_arch = "amdgpu", allow(unused_imports))]

use fe2o3_device::{
    DeviceMath, DisjointSlice, Gfx950F32AccumulatorFragment, Gfx950Fp4E2M1, Gfx950Fp4MfmaAMatrix,
    Gfx950Fp8MfmaBMatrix, Gfx950Matrix, Gfx950Subgroup, StridedReadView2D, Wave64, WaveLane,
    kernel, thread,
};

use crate::{
    ALL_EXPERTS, CANDIDATES, DISPATCH_CAPACITY, DRAFT_STEPS, EXPERTS, GRADIENT_SHARDS, HIDDEN,
    MUON_ELEMENTS, MUON_LEARNING_RATE, NGRAM, OUTPUT, QUERIES, STATE_WIDTH, TABLE_SIZE, TOKENS,
    TOP_K,
};

/// Stable top-2 routing, weights, expert counts, and compact dispatch metadata.
#[cfg(any(not(target_arch = "amdgpu"), feature = "kernel-moe-route"))]
#[kernel(
    typed,
    namespace = "5f88dd0eb7d763b42a77dce26f06a50c315730e6a77414e64480fd94f7e9e690",
    launch(required = [256, 1, 1], max = [256, 1, 1], max_grid = [1, 1, 1])
)]
#[allow(clippy::too_many_arguments, unused_assignments)]
pub fn gfx950_moe_route_fp4_t16_e4_k2_v1(
    activations: &[u8],
    router_weights: &[f32],
    mut top_experts: DisjointSlice<u32>,
    mut top_weights: DisjointSlice<f32>,
    mut expert_counts: DisjointSlice<u32>,
    mut dispatch: DisjointSlice<i32>,
) {
    let lane = thread::index_1d().get();
    if activations.len() != TOKENS * HIDDEN
        || router_weights.len() != EXPERTS * HIDDEN
        || top_experts.len() != TOKENS * TOP_K
        || top_weights.len() != TOKENS * TOP_K
        || expert_counts.len() != EXPERTS
        || dispatch.len() != EXPERTS * DISPATCH_CAPACITY
    {
        return;
    }
    if lane >= EXPERTS * DISPATCH_CAPACITY {
        return;
    }
    let Ok(activation_view) =
        StridedReadView2D::from_shared_slice(activations, 0, TOKENS, HIDDEN, HIDDEN)
    else {
        return;
    };
    let Ok(router_view) =
        StridedReadView2D::from_shared_slice(router_weights, 0, EXPERTS, HIDDEN, HIDDEN)
    else {
        return;
    };
    macro_rules! select_route {
        (
            $token:expr,
            $first:ident,
            $second:ident,
            $route_logit0:ident,
            $route_logit1:ident,
            $route_logit2:ident,
            $route_logit3:ident
        ) => {{
            let mut depth = 0_usize;
            while depth < HIDDEN {
                let bits = activation_view.load_or($token, depth, 0);
                let magnitude = ((0xc864_3210_u32 >> (((bits & 7) as u32) * 4)) & 15) as f32 * 0.5;
                let sign = 1.0 - 2.0 * ((bits >> 3) & 1) as f32;
                let activation = sign * magnitude;
                $route_logit0 += activation * router_view.load_or(0, depth, 0.0);
                $route_logit1 += activation * router_view.load_or(1, depth, 0.0);
                $route_logit2 += activation * router_view.load_or(2, depth, 0.0);
                $route_logit3 += activation * router_view.load_or(3, depth, 0.0);
                depth += 1;
            }
            let precedes12 = ($route_logit1 >= $route_logit2) as u32;
            let precedes13 = ($route_logit1 >= $route_logit3) as u32;
            let precedes23 = ($route_logit2 >= $route_logit3) as u32;
            let rank1 = ($route_logit0 >= $route_logit1) as u32 + 2 - precedes12 - precedes13;
            let rank2 = ($route_logit0 >= $route_logit2) as u32 + precedes12 + 1 - precedes23;
            let rank3 = ($route_logit0 >= $route_logit3) as u32 + precedes13 + precedes23;
            $first = ((rank1 == 0) as u32) + 2 * ((rank2 == 0) as u32) + 3 * ((rank3 == 0) as u32);
            $second = ((rank1 == 1) as u32) + 2 * ((rank2 == 1) as u32) + 3 * ((rank3 == 1) as u32);
        }};
    }
    if lane < TOKENS * TOP_K {
        let token = lane / TOP_K;
        let choice = lane - token * TOP_K;
        let mut first = 0_u32;
        let mut second = 0_u32;
        let mut route_logit0 = 0.0_f32;
        let mut route_logit1 = 0.0_f32;
        let mut route_logit2 = 0.0_f32;
        let mut route_logit3 = 0.0_f32;
        select_route!(
            token,
            first,
            second,
            route_logit0,
            route_logit1,
            route_logit2,
            route_logit3
        );
        let first_logit = if first == 0 {
            route_logit0
        } else if first == 1 {
            route_logit1
        } else if first == 2 {
            route_logit2
        } else {
            route_logit3
        };
        let second_logit = if second == 0 {
            route_logit0
        } else if second == 1 {
            route_logit1
        } else if second == 2 {
            route_logit2
        } else {
            route_logit3
        };
        let selected = if choice == 0 { first } else { second };
        let maximum = if first_logit > second_logit {
            first_logit
        } else {
            second_logit
        };
        let math = DeviceMath::current();
        let first_exp = math.exp_f32(first_logit - maximum);
        let second_exp = math.exp_f32(second_logit - maximum);
        let denominator = first_exp + second_exp;
        let weight = if choice == 0 {
            first_exp / denominator
        } else {
            second_exp / denominator
        };
        if let Some(slot) = top_experts.get_mut(thread::index_1d()) {
            *slot = selected;
        }
        if let Some(slot) = top_weights.get_mut(thread::index_1d()) {
            *slot = weight;
        }
    }
    if lane < EXPERTS {
        let expert = lane as u32;
        let mut count = 0_u32;
        macro_rules! count_tokens {
            () => {{
                let mut token = 0_usize;
                while token < TOKENS {
                    let mut first = 0_u32;
                    let mut second = 0_u32;
                    let mut route_logit0 = 0.0_f32;
                    let mut route_logit1 = 0.0_f32;
                    let mut route_logit2 = 0.0_f32;
                    let mut route_logit3 = 0.0_f32;
                    select_route!(
                        token,
                        first,
                        second,
                        route_logit0,
                        route_logit1,
                        route_logit2,
                        route_logit3
                    );
                    count += (first == expert) as u32 + (second == expert) as u32;
                    token += 1;
                }
            }};
        }
        count_tokens!();
        if let Some(slot) = expert_counts.get_mut(thread::index_1d()) {
            *slot = count;
        }
    }
    let expert = (lane / DISPATCH_CAPACITY) as u32;
    let wanted = lane - expert as usize * DISPATCH_CAPACITY;
    let mut seen = 0_usize;
    let mut dispatched = -1_i32;
    macro_rules! dispatch_tokens {
        () => {{
            let mut token = 0_usize;
            while token < TOKENS {
                let mut first = 0_u32;
                let mut second = 0_u32;
                let mut route_logit0 = 0.0_f32;
                let mut route_logit1 = 0.0_f32;
                let mut route_logit2 = 0.0_f32;
                let mut route_logit3 = 0.0_f32;
                select_route!(
                    token,
                    first,
                    second,
                    route_logit0,
                    route_logit1,
                    route_logit2,
                    route_logit3
                );
                let take_first = (first == expert) as usize;
                let choose_first = ((first == expert) & (seen == wanted)) as i32;
                dispatched += (token as i32 * TOP_K as i32 - dispatched) * choose_first;
                seen += take_first;
                let take_second = (second == expert) as usize;
                let choose_second = ((second == expert) & (seen == wanted)) as i32;
                dispatched += (token as i32 * TOP_K as i32 + 1 - dispatched) * choose_second;
                seen += take_second;
                token += 1;
            }
        }};
    }
    dispatch_tokens!();
    if let Some(slot) = dispatch.get_mut(thread::index_1d()) {
        *slot = dispatched;
    }
}

/// Computes a routed expert partition and optional shared-expert contribution.
#[cfg(any(not(target_arch = "amdgpu"), feature = "kernel-moe-expert-rank"))]
#[kernel(
    typed,
    namespace = "95964e6517ecad06b1b825cf64c29fb20fe9ec054dd551dfdc55f2e73c261dfc",
    launch(required = [256, 1, 1], max = [256, 1, 1], max_grid = [1, 1, 1])
)]
#[allow(clippy::too_many_arguments)]
pub fn gfx950_moe_expert_rank_fp4_fp8_v1(
    activations: &[u8],
    expert_weights: &[u8],
    top_experts: &[u32],
    top_weights: &[f32],
    first_expert: u32,
    include_shared_expert: u32,
    mut output: DisjointSlice<f32>,
) {
    let output_index = thread::index_1d().get();
    if activations.len() < TOKENS * HIDDEN
        || expert_weights.len() < ALL_EXPERTS * HIDDEN * OUTPUT
        || top_experts.len() < TOKENS * TOP_K
        || top_weights.len() < TOKENS * TOP_K
        || output.len() < TOKENS * OUTPUT
        || first_expert as usize + 1 >= EXPERTS
    {
        return;
    }
    let lane = WaveLane::<Wave64>::current();
    let Ok(activations_first_view) =
        Gfx950Fp4MfmaAMatrix::row_major(activations, 0, TOKENS, HIDDEN, HIDDEN)
    else {
        return;
    };
    let activations_first = activations_first_view.load_m16k128(&lane, 0, 0);
    let first_offset = first_expert as usize * HIDDEN * OUTPUT;
    let Ok(first_weights_view) =
        Gfx950Fp8MfmaBMatrix::row_major(expert_weights, first_offset, HIDDEN, OUTPUT, OUTPUT)
    else {
        return;
    };
    let first_weights = first_weights_view.load_k128n16(&lane, 0, 0);
    let Ok(second_weights_view) = Gfx950Fp8MfmaBMatrix::row_major(
        expert_weights,
        first_offset + HIDDEN * OUTPUT,
        HIDDEN,
        OUTPUT,
        OUTPUT,
    ) else {
        return;
    };
    let second_weights = second_weights_view.load_k128n16(&lane, 0, 0);
    let Ok(shared_weights_view) = Gfx950Fp8MfmaBMatrix::row_major(
        expert_weights,
        (ALL_EXPERTS - 1) * HIDDEN * OUTPUT,
        HIDDEN,
        OUTPUT,
        OUTPUT,
    ) else {
        return;
    };
    let shared_weights = shared_weights_view.load_k128n16(&lane, 0, 0);
    let matrix = Gfx950Matrix::current();
    let first_values = matrix
        .multiply_accumulate_fp4_fp8(
            activations_first,
            first_weights,
            Gfx950F32AccumulatorFragment::<Gfx950Fp4E2M1>::zero(&lane),
        )
        .into_values();
    let Ok(activations_second_view) =
        Gfx950Fp4MfmaAMatrix::row_major(activations, 0, TOKENS, HIDDEN, HIDDEN)
    else {
        return;
    };
    let activations_second = activations_second_view.load_m16k128(&lane, 0, 0);
    let second_values = matrix
        .multiply_accumulate_fp4_fp8(
            activations_second,
            second_weights,
            Gfx950F32AccumulatorFragment::<Gfx950Fp4E2M1>::zero(&lane),
        )
        .into_values();
    let Ok(activations_shared_view) =
        Gfx950Fp4MfmaAMatrix::row_major(activations, 0, TOKENS, HIDDEN, HIDDEN)
    else {
        return;
    };
    let activations_shared = activations_shared_view.load_m16k128(&lane, 0, 0);
    let shared_values = matrix
        .multiply_accumulate_fp4_fp8(
            activations_shared,
            shared_weights,
            Gfx950F32AccumulatorFragment::<Gfx950Fp4E2M1>::zero(&lane),
        )
        .into_values();
    let token = output_index / OUTPUT;
    let component = token - (token / 4) * 4;
    let column = output_index - token * OUTPUT;
    let source_lane = (((token / 4) * OUTPUT + column) as u32) & 63;
    let subgroup = Gfx950Subgroup::current();
    let first0 = subgroup.broadcast_f32::<64>(first_values[0], source_lane);
    let first1 = subgroup.broadcast_f32::<64>(first_values[1], source_lane);
    let first2 = subgroup.broadcast_f32::<64>(first_values[2], source_lane);
    let first3 = subgroup.broadcast_f32::<64>(first_values[3], source_lane);
    let second0 = subgroup.broadcast_f32::<64>(second_values[0], source_lane);
    let second1 = subgroup.broadcast_f32::<64>(second_values[1], source_lane);
    let second2 = subgroup.broadcast_f32::<64>(second_values[2], source_lane);
    let second3 = subgroup.broadcast_f32::<64>(second_values[3], source_lane);
    let shared0 = subgroup.broadcast_f32::<64>(shared_values[0], source_lane);
    let shared1 = subgroup.broadcast_f32::<64>(shared_values[1], source_lane);
    let shared2 = subgroup.broadcast_f32::<64>(shared_values[2], source_lane);
    let shared3 = subgroup.broadcast_f32::<64>(shared_values[3], source_lane);
    let first = if component == 0 {
        first0
    } else if component == 1 {
        first1
    } else if component == 2 {
        first2
    } else {
        first3
    };
    let second = if component == 0 {
        second0
    } else if component == 1 {
        second1
    } else if component == 2 {
        second2
    } else {
        second3
    };
    let shared = if component == 0 {
        shared0
    } else if component == 1 {
        shared1
    } else if component == 2 {
        shared2
    } else {
        shared3
    };
    let selected0 = top_experts[token * TOP_K];
    let selected1 = top_experts[token * TOP_K + 1];
    let math = DeviceMath::current();
    let mut result = 0.0_f32;
    if selected0 == first_expert {
        result += top_weights[token * TOP_K] * (first / (1.0 + math.exp_f32(-first)));
    } else if selected0 == first_expert + 1 {
        result += top_weights[token * TOP_K] * (second / (1.0 + math.exp_f32(-second)));
    }
    if selected1 == first_expert {
        result += top_weights[token * TOP_K + 1] * (first / (1.0 + math.exp_f32(-first)));
    } else if selected1 == first_expert + 1 {
        result += top_weights[token * TOP_K + 1] * (second / (1.0 + math.exp_f32(-second)));
    }
    if include_shared_expert != 0 {
        result += 0.25 * (shared / (1.0 + math.exp_f32(-shared)));
    }
    if let Some(slot) = output.get_mut(thread::index_1d()) {
        *slot = result;
    }
}

/// Adds two expert-rank partials in fixed rank order.
#[cfg(any(not(target_arch = "amdgpu"), feature = "kernel-combine-expert-ranks"))]
#[kernel(
    typed,
    namespace = "a27beaf1c7c14d2129a2efc9bd9802fba895073515686b9a81495afe4b65047b",
    launch(required = [256, 1, 1], max = [256, 1, 1])
)]
pub fn gfx950_combine_expert_ranks_v1(
    rank0: &[f32],
    rank1: &[f32],
    mut output: DisjointSlice<f32>,
) {
    let index = thread::index_1d();
    let element = index.get();
    if rank0.len() != TOKENS * OUTPUT
        || rank1.len() != TOKENS * OUTPUT
        || output.len() != TOKENS * OUTPUT
    {
        return;
    }
    if element >= TOKENS * OUTPUT {
        return;
    }
    if let Some(slot) = output.get_mut(index) {
        *slot = rank0[element] + rank1[element];
    }
}

/// Commits state only when every speculative token and score is accepted.
#[cfg(any(
    not(target_arch = "amdgpu"),
    feature = "kernel-speculative-transaction"
))]
#[kernel(
    typed,
    namespace = "56cb0ca1edf995cb22811650289af395e5932a3dcb3eaeadd64139781ad8e1fa",
    launch(required = [64, 1, 1], max = [64, 1, 1], max_grid = [1, 1, 1])
)]
#[allow(clippy::too_many_arguments)]
pub fn gfx950_speculative_transaction_v1(
    draft_tokens: &[i32],
    target_tokens: &[i32],
    draft_scores: &[f32],
    thresholds: &[f32],
    base_state: &[f32],
    proposed_deltas: &[f32],
    mut accepted_steps: DisjointSlice<u32>,
    mut committed: DisjointSlice<u32>,
    mut output_state: DisjointSlice<f32>,
) {
    let lane = thread::index_1d().get();
    if draft_tokens.len() != CANDIDATES * DRAFT_STEPS
        || target_tokens.len() != DRAFT_STEPS
        || draft_scores.len() != CANDIDATES * DRAFT_STEPS
        || thresholds.len() != DRAFT_STEPS
        || base_state.len() != STATE_WIDTH
        || proposed_deltas.len() != CANDIDATES * DRAFT_STEPS * STATE_WIDTH
        || accepted_steps.len() != CANDIDATES
        || committed.len() != CANDIDATES
        || output_state.len() != CANDIDATES * STATE_WIDTH
    {
        return;
    }
    let Ok(target_tokens) =
        StridedReadView2D::from_shared_slice(target_tokens, 0, 1, DRAFT_STEPS, DRAFT_STEPS)
    else {
        return;
    };
    let Ok(thresholds) =
        StridedReadView2D::from_shared_slice(thresholds, 0, 1, DRAFT_STEPS, DRAFT_STEPS)
    else {
        return;
    };
    if lane >= CANDIDATES * STATE_WIDTH {
        return;
    }
    if lane < CANDIDATES {
        let candidate = lane;
        let base = candidate * DRAFT_STEPS;
        let accepts0 = (draft_tokens[base] == target_tokens.load_or(0, 0, 0))
            & (draft_scores[base] >= thresholds.load_or(0, 0, 0.0));
        let accepts1 = accepts0
            & (draft_tokens[base + 1] == target_tokens.load_or(0, 1, 0))
            & (draft_scores[base + 1] >= thresholds.load_or(0, 1, 0.0));
        let accepts2 = accepts1
            & (draft_tokens[base + 2] == target_tokens.load_or(0, 2, 0))
            & (draft_scores[base + 2] >= thresholds.load_or(0, 2, 0.0));
        let accepts3 = accepts2
            & (draft_tokens[base + 3] == target_tokens.load_or(0, 3, 0))
            & (draft_scores[base + 3] >= thresholds.load_or(0, 3, 0.0));
        let accepted =
            accepts0 as usize + accepts1 as usize + accepts2 as usize + accepts3 as usize;
        if let Some(slot) = accepted_steps.get_mut(thread::index_1d()) {
            *slot = accepted as u32;
        }
        if let Some(slot) = committed.get_mut(thread::index_1d()) {
            *slot = if accepted == DRAFT_STEPS { 1 } else { 0 };
        }
    }
    let candidate = lane / STATE_WIDTH;
    let state_element = lane - candidate * STATE_WIDTH;
    let base = candidate * DRAFT_STEPS;
    let accepts0 = (draft_tokens[base] == target_tokens.load_or(0, 0, 0))
        & (draft_scores[base] >= thresholds.load_or(0, 0, 0.0));
    let accepts1 = accepts0
        & (draft_tokens[base + 1] == target_tokens.load_or(0, 1, 0))
        & (draft_scores[base + 1] >= thresholds.load_or(0, 1, 0.0));
    let accepts2 = accepts1
        & (draft_tokens[base + 2] == target_tokens.load_or(0, 2, 0))
        & (draft_scores[base + 2] >= thresholds.load_or(0, 2, 0.0));
    let accepts3 = accepts2
        & (draft_tokens[base + 3] == target_tokens.load_or(0, 3, 0))
        & (draft_scores[base + 3] >= thresholds.load_or(0, 3, 0.0));
    let accepted = accepts0 as usize + accepts1 as usize + accepts2 as usize + accepts3 as usize;
    let mut value = base_state[state_element];
    if accepted == DRAFT_STEPS {
        value += proposed_deltas[candidate * DRAFT_STEPS * STATE_WIDTH + state_element];
        value += proposed_deltas[(candidate * DRAFT_STEPS + 1) * STATE_WIDTH + state_element];
        value += proposed_deltas[(candidate * DRAFT_STEPS + 2) * STATE_WIDTH + state_element];
        value += proposed_deltas[(candidate * DRAFT_STEPS + 3) * STATE_WIDTH + state_element];
    }
    if let Some(slot) = output_state.get_mut(thread::index_1d()) {
        *slot = value;
    }
}

/// Probes every slot, verifies the full 3-gram, and resolves duplicate keys.
#[cfg(any(not(target_arch = "amdgpu"), feature = "kernel-qwen-ngram-gather"))]
#[kernel(
    typed,
    namespace = "759fc11ede4636245a173107a33f25014d4f8f0f29f1710bf9b9396aeda69ee9",
    launch(required = [64, 1, 1], max = [64, 1, 1], max_grid = [1, 1, 1])
)]
pub fn gfx950_qwen_ngram_gather_v1(
    queries: &[i32],
    table_hashes: &[u64],
    table_grams: &[i32],
    table_values: &[i32],
    priorities: &[i32],
    mut output: DisjointSlice<i32>,
) {
    let index = thread::index_1d();
    let query = index.get();
    if queries.len() != QUERIES * NGRAM
        || table_hashes.len() != TABLE_SIZE
        || table_grams.len() != TABLE_SIZE * NGRAM
        || table_values.len() != TABLE_SIZE
        || priorities.len() != TABLE_SIZE
        || output.len() != QUERIES
    {
        return;
    }
    if query >= QUERIES {
        return;
    }
    let base = query * NGRAM;
    let mut hash = 1_469_598_103_934_665_603_u64;
    hash ^= queries[base] as u32 as u64;
    hash = hash.wrapping_mul(1_099_511_628_211);
    hash ^= queries[base + 1] as u32 as u64;
    hash = hash.wrapping_mul(1_099_511_628_211);
    hash ^= queries[base + 2] as u32 as u64;
    hash = hash.wrapping_mul(1_099_511_628_211);
    let mut best_slot = usize::MAX;
    let mut best_priority = i32::MIN;
    let mut best_value = -1_i32;
    macro_rules! probe {
        ($probe:literal) => {{
            let slot = hash.wrapping_add($probe) as usize & (TABLE_SIZE - 1);
            let equal = (table_hashes[slot] == hash)
                & (table_grams[slot * NGRAM] == queries[base])
                & (table_grams[slot * NGRAM + 1] == queries[base + 1])
                & (table_grams[slot * NGRAM + 2] == queries[base + 2]);
            if equal {
                let priority = priorities[slot];
                if priority > best_priority || (priority == best_priority && slot < best_slot) {
                    best_slot = slot;
                    best_priority = priority;
                    best_value = table_values[slot];
                }
            }
        }};
    }
    macro_rules! final_probe {
        ($probe:literal) => {{
            let slot = hash.wrapping_add($probe) as usize & (TABLE_SIZE - 1);
            let equal = (table_hashes[slot] == hash)
                & (table_grams[slot * NGRAM] == queries[base])
                & (table_grams[slot * NGRAM + 1] == queries[base + 1])
                & (table_grams[slot * NGRAM + 2] == queries[base + 2]);
            if equal {
                let priority = priorities[slot];
                if priority > best_priority || (priority == best_priority && slot < best_slot) {
                    best_value = table_values[slot];
                }
            }
        }};
    }
    probe!(0);
    probe!(1);
    probe!(2);
    probe!(3);
    probe!(4);
    probe!(5);
    probe!(6);
    probe!(7);
    probe!(8);
    probe!(9);
    probe!(10);
    probe!(11);
    probe!(12);
    probe!(13);
    probe!(14);
    final_probe!(15);
    if let Some(slot) = output.get_mut(index) {
        *slot = best_value;
    }
}

/// Copies one gradient shard into deterministic transport staging.
#[cfg(any(not(target_arch = "amdgpu"), feature = "kernel-stage-gradient-shard"))]
#[kernel(
    typed,
    namespace = "eecf6b35ad78d15ed59c50e42bb156b24bc9977508e57b67c71c449c6486a336",
    launch(required = [64, 1, 1], max = [64, 1, 1], max_grid = [1, 1, 1])
)]
pub fn gfx950_stage_gradient_shard_v1(input: &[f32], mut output: DisjointSlice<f32>) {
    let index = thread::index_1d();
    let element = index.get();
    if input.len() != MUON_ELEMENTS || output.len() != MUON_ELEMENTS {
        return;
    }
    if element >= MUON_ELEMENTS {
        return;
    }
    if let Some(slot) = output.get_mut(index) {
        *slot = input[element];
    }
}

/// Reduces two shards and computes five Newton-Schulz Muon iterations.
#[cfg(any(not(target_arch = "amdgpu"), feature = "kernel-muon-update"))]
#[kernel(
    typed,
    namespace = "62b82262b6a906c5c4bc76bdf41008abdb45f2c4d9830734cd05ae65520150e2",
    launch(required = [64, 1, 1], max = [64, 1, 1], max_grid = [1, 1, 1])
)]
pub fn gfx950_muon_update_4x4_v1(
    shards: &[f32],
    mut output: DisjointSlice<f32>,
    mut output_norm: DisjointSlice<f32>,
) {
    let lane = thread::index_1d().get();
    if shards.len() != GRADIENT_SHARDS * MUON_ELEMENTS
        || output.len() != MUON_ELEMENTS
        || output_norm.len() != 1
    {
        return;
    }
    let Ok(shards) = StridedReadView2D::from_shared_slice(
        shards,
        0,
        GRADIENT_SHARDS,
        MUON_ELEMENTS,
        MUON_ELEMENTS,
    ) else {
        return;
    };
    let mut m00 = shards.load_or(0, 0, 0.0) + shards.load_or(1, 0, 0.0);
    let mut m01 = shards.load_or(0, 1, 0.0) + shards.load_or(1, 1, 0.0);
    let mut m02 = shards.load_or(0, 2, 0.0) + shards.load_or(1, 2, 0.0);
    let mut m03 = shards.load_or(0, 3, 0.0) + shards.load_or(1, 3, 0.0);
    let mut m10 = shards.load_or(0, 4, 0.0) + shards.load_or(1, 4, 0.0);
    let mut m11 = shards.load_or(0, 5, 0.0) + shards.load_or(1, 5, 0.0);
    let mut m12 = shards.load_or(0, 6, 0.0) + shards.load_or(1, 6, 0.0);
    let mut m13 = shards.load_or(0, 7, 0.0) + shards.load_or(1, 7, 0.0);
    let mut m20 = shards.load_or(0, 8, 0.0) + shards.load_or(1, 8, 0.0);
    let mut m21 = shards.load_or(0, 9, 0.0) + shards.load_or(1, 9, 0.0);
    let mut m22 = shards.load_or(0, 10, 0.0) + shards.load_or(1, 10, 0.0);
    let mut m23 = shards.load_or(0, 11, 0.0) + shards.load_or(1, 11, 0.0);
    let mut m30 = shards.load_or(0, 12, 0.0) + shards.load_or(1, 12, 0.0);
    let mut m31 = shards.load_or(0, 13, 0.0) + shards.load_or(1, 13, 0.0);
    let mut m32 = shards.load_or(0, 14, 0.0) + shards.load_or(1, 14, 0.0);
    let mut m33 = shards.load_or(0, 15, 0.0) + shards.load_or(1, 15, 0.0);
    let squared_norm = m00 * m00
        + m01 * m01
        + m02 * m02
        + m03 * m03
        + m10 * m10
        + m11 * m11
        + m12 * m12
        + m13 * m13
        + m20 * m20
        + m21 * m21
        + m22 * m22
        + m23 * m23
        + m30 * m30
        + m31 * m31
        + m32 * m32
        + m33 * m33;
    let norm = DeviceMath::current().sqrt_f32(squared_norm);
    if lane == 0 {
        if let Some(slot) = output_norm.get_mut(thread::index_1d()) {
            *slot = norm;
        }
    }
    let inverse_norm = 1.0 / (norm + 1.0e-6);
    m00 *= inverse_norm;
    m01 *= inverse_norm;
    m02 *= inverse_norm;
    m03 *= inverse_norm;
    m10 *= inverse_norm;
    m11 *= inverse_norm;
    m12 *= inverse_norm;
    m13 *= inverse_norm;
    m20 *= inverse_norm;
    m21 *= inverse_norm;
    m22 *= inverse_norm;
    m23 *= inverse_norm;
    m30 *= inverse_norm;
    m31 *= inverse_norm;
    m32 *= inverse_norm;
    m33 *= inverse_norm;
    macro_rules! muon_iteration {
        () => {{
            let g00 = m00 * m00 + m01 * m01 + m02 * m02 + m03 * m03;
            let g01 = m00 * m10 + m01 * m11 + m02 * m12 + m03 * m13;
            let g02 = m00 * m20 + m01 * m21 + m02 * m22 + m03 * m23;
            let g03 = m00 * m30 + m01 * m31 + m02 * m32 + m03 * m33;
            let g10 = m10 * m00 + m11 * m01 + m12 * m02 + m13 * m03;
            let g11 = m10 * m10 + m11 * m11 + m12 * m12 + m13 * m13;
            let g12 = m10 * m20 + m11 * m21 + m12 * m22 + m13 * m23;
            let g13 = m10 * m30 + m11 * m31 + m12 * m32 + m13 * m33;
            let g20 = m20 * m00 + m21 * m01 + m22 * m02 + m23 * m03;
            let g21 = m20 * m10 + m21 * m11 + m22 * m12 + m23 * m13;
            let g22 = m20 * m20 + m21 * m21 + m22 * m22 + m23 * m23;
            let g23 = m20 * m30 + m21 * m31 + m22 * m32 + m23 * m33;
            let g30 = m30 * m00 + m31 * m01 + m32 * m02 + m33 * m03;
            let g31 = m30 * m10 + m31 * m11 + m32 * m12 + m33 * m13;
            let g32 = m30 * m20 + m31 * m21 + m32 * m22 + m33 * m23;
            let g33 = m30 * m30 + m31 * m31 + m32 * m32 + m33 * m33;
            let c00 = g00 * m00 + g01 * m10 + g02 * m20 + g03 * m30;
            let c01 = g00 * m01 + g01 * m11 + g02 * m21 + g03 * m31;
            let c02 = g00 * m02 + g01 * m12 + g02 * m22 + g03 * m32;
            let c03 = g00 * m03 + g01 * m13 + g02 * m23 + g03 * m33;
            let c10 = g10 * m00 + g11 * m10 + g12 * m20 + g13 * m30;
            let c11 = g10 * m01 + g11 * m11 + g12 * m21 + g13 * m31;
            let c12 = g10 * m02 + g11 * m12 + g12 * m22 + g13 * m32;
            let c13 = g10 * m03 + g11 * m13 + g12 * m23 + g13 * m33;
            let c20 = g20 * m00 + g21 * m10 + g22 * m20 + g23 * m30;
            let c21 = g20 * m01 + g21 * m11 + g22 * m21 + g23 * m31;
            let c22 = g20 * m02 + g21 * m12 + g22 * m22 + g23 * m32;
            let c23 = g20 * m03 + g21 * m13 + g22 * m23 + g23 * m33;
            let c30 = g30 * m00 + g31 * m10 + g32 * m20 + g33 * m30;
            let c31 = g30 * m01 + g31 * m11 + g32 * m21 + g33 * m31;
            let c32 = g30 * m02 + g31 * m12 + g32 * m22 + g33 * m32;
            let c33 = g30 * m03 + g31 * m13 + g32 * m23 + g33 * m33;
            m00 = 1.5 * m00 - 0.5 * c00;
            m01 = 1.5 * m01 - 0.5 * c01;
            m02 = 1.5 * m02 - 0.5 * c02;
            m03 = 1.5 * m03 - 0.5 * c03;
            m10 = 1.5 * m10 - 0.5 * c10;
            m11 = 1.5 * m11 - 0.5 * c11;
            m12 = 1.5 * m12 - 0.5 * c12;
            m13 = 1.5 * m13 - 0.5 * c13;
            m20 = 1.5 * m20 - 0.5 * c20;
            m21 = 1.5 * m21 - 0.5 * c21;
            m22 = 1.5 * m22 - 0.5 * c22;
            m23 = 1.5 * m23 - 0.5 * c23;
            m30 = 1.5 * m30 - 0.5 * c30;
            m31 = 1.5 * m31 - 0.5 * c31;
            m32 = 1.5 * m32 - 0.5 * c32;
            m33 = 1.5 * m33 - 0.5 * c33;
        }};
    }
    muon_iteration!();
    muon_iteration!();
    muon_iteration!();
    muon_iteration!();
    muon_iteration!();
    if lane < MUON_ELEMENTS {
        let value = if lane == 0 {
            m00
        } else if lane == 1 {
            m01
        } else if lane == 2 {
            m02
        } else if lane == 3 {
            m03
        } else if lane == 4 {
            m10
        } else if lane == 5 {
            m11
        } else if lane == 6 {
            m12
        } else if lane == 7 {
            m13
        } else if lane == 8 {
            m20
        } else if lane == 9 {
            m21
        } else if lane == 10 {
            m22
        } else if lane == 11 {
            m23
        } else if lane == 12 {
            m30
        } else if lane == 13 {
            m31
        } else if lane == 14 {
            m32
        } else {
            m33
        };
        if let Some(slot) = output.get_mut(thread::index_1d()) {
            *slot = -MUON_LEARNING_RATE * value;
        }
    }
}
