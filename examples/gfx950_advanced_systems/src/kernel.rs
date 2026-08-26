//! Complete safe Rust kernel source for the bounded systems profiles.

#![allow(missing_docs)] // The kernel macro emits helper modules.

use fe2o3_device::{Blocked, DeviceMath, DisjointSlice, GridExclusive, Index1D, kernel, thread};

use crate::{
    ALL_EXPERTS, CANDIDATES, DISPATCH_CAPACITY, DRAFT_STEPS, EXPERTS, GRADIENT_SHARDS, HIDDEN,
    MUON_DIM, MUON_ELEMENTS, MUON_ITERATIONS, MUON_LEARNING_RATE, NGRAM, OUTPUT, QUERIES,
    STATE_WIDTH, TABLE_SIZE, TOKENS, TOP_K,
};

fn fp4_e2m1_to_f32(bits: u8) -> f32 {
    let table = [0.0_f32, 0.5, 1.0, 1.5, 2.0, 3.0, 4.0, 6.0];
    let magnitude = table[(bits & 7) as usize];
    if bits & 8 == 0 { magnitude } else { -magnitude }
}

fn fp8_e4m3_to_f32(bits: u8) -> f32 {
    let sign = if bits & 0x80 == 0 { 1.0 } else { -1.0 };
    let exponent = (bits >> 3) & 15;
    let mantissa = (bits & 7) as f32;
    if exponent == 15 && mantissa == 7.0 {
        return f32::NAN;
    }
    let magnitude = if exponent == 0 {
        mantissa * (1.0 / 512.0)
    } else {
        (1.0 + mantissa * 0.125) * pow2_i32(exponent as i32 - 7)
    };
    sign * magnitude
}

fn pow2_i32(exponent: i32) -> f32 {
    if exponent >= -126 {
        f32::from_bits(((exponent + 127) as u32) << 23)
    } else {
        0.0
    }
}

fn better(candidate: f32, candidate_id: usize, current: f32, current_id: usize) -> bool {
    candidate > current || (candidate == current && candidate_id < current_id)
}

fn ngram_hash(queries: &[i32], base: usize) -> u64 {
    let mut hash = 1_469_598_103_934_665_603_u64;
    let mut index = 0;
    while index < NGRAM {
        hash ^= queries[base + index] as u32 as u64;
        hash = hash.wrapping_mul(1_099_511_628_211);
        index += 1;
    }
    hash
}

/// Stable top-2 routing, weights, expert counts, and compact dispatch metadata.
#[kernel(
    typed,
    namespace = "ad1fbafcc50548248aeb40e10682d4dcde40cf78d1fa27d3df999223abf728e5",
    launch(required = [64, 1, 1], max = [64, 1, 1]),
    control_flow(loop_bounds(16, 4, 128, 4, 4, 2, 32, 4, 128))
)]
#[allow(clippy::too_many_arguments)]
pub fn gfx950_moe_route_fp4_t16_e4_k2_v1(
    activations: &[u8],
    router_weights: &[f32],
    mut top_experts: DisjointSlice<u32, GridExclusive>,
    mut top_weights: DisjointSlice<f32, GridExclusive>,
    mut expert_counts: DisjointSlice<u32, GridExclusive>,
    mut dispatch: DisjointSlice<i32, GridExclusive>,
) {
    let Some(leader) = thread::grid_leader() else {
        return;
    };
    if activations.len() != TOKENS * HIDDEN
        || router_weights.len() != EXPERTS * HIDDEN
        || top_experts.len() != TOKENS * TOP_K
        || top_weights.len() != TOKENS * TOP_K
        || expert_counts.len() != EXPERTS
        || dispatch.len() != EXPERTS * DISPATCH_CAPACITY
    {
        fe2o3_device::trap();
        return;
    }

    let mut counts = [0_u32; EXPERTS];
    let mut staged_experts = [0_u32; TOKENS * TOP_K];
    let mut staged_weights = [0.0_f32; TOKENS * TOP_K];
    let mut staged_dispatch = [-1_i32; EXPERTS * DISPATCH_CAPACITY];
    let math = DeviceMath::current();
    let mut token = 0;
    while token < TOKENS {
        let mut logits = [0.0_f32; EXPERTS];
        let mut expert = 0;
        while expert < EXPERTS {
            let mut depth = 0;
            while depth < HIDDEN {
                logits[expert] += fp4_e2m1_to_f32(activations[token * HIDDEN + depth])
                    * router_weights[expert * HIDDEN + depth];
                depth += 1;
            }
            expert += 1;
        }
        let mut first = 0;
        expert = 1;
        while expert < EXPERTS {
            if better(logits[expert], expert, logits[first], first) {
                first = expert;
            }
            expert += 1;
        }
        let mut second = if first == 0 { 1 } else { 0 };
        expert = 0;
        while expert < EXPERTS {
            if expert != first && better(logits[expert], expert, logits[second], second) {
                second = expert;
            }
            expert += 1;
        }
        let maximum = if logits[first] > logits[second] {
            logits[first]
        } else {
            logits[second]
        };
        let first_exp = math.exp_f32(logits[first] - maximum);
        let second_exp = math.exp_f32(logits[second] - maximum);
        let denominator = first_exp + second_exp;
        staged_experts[token * TOP_K] = first as u32;
        staged_experts[token * TOP_K + 1] = second as u32;
        staged_weights[token * TOP_K] = first_exp / denominator;
        staged_weights[token * TOP_K + 1] = second_exp / denominator;
        let mut choice = 0;
        while choice < TOP_K {
            let selected = staged_experts[token * TOP_K + choice] as usize;
            let slot = counts[selected] as usize;
            counts[selected] += 1;
            staged_dispatch[selected * DISPATCH_CAPACITY + slot] = (token * TOP_K + choice) as i32;
            choice += 1;
        }
        token += 1;
    }

    let mut index = 0;
    while index < TOKENS * TOP_K {
        *top_experts.get_mut_exclusive(&leader, index).unwrap() = staged_experts[index];
        *top_weights.get_mut_exclusive(&leader, index).unwrap() = staged_weights[index];
        index += 1;
    }
    index = 0;
    while index < EXPERTS {
        *expert_counts.get_mut_exclusive(&leader, index).unwrap() = counts[index];
        index += 1;
    }
    index = 0;
    while index < EXPERTS * DISPATCH_CAPACITY {
        *dispatch.get_mut_exclusive(&leader, index).unwrap() = staged_dispatch[index];
        index += 1;
    }
}

/// Computes a routed expert partition and optional shared-expert contribution.
#[kernel(
    typed,
    namespace = "1db61589cb6c2024b1904572157e1f0434bb70f029f6fd4f91eb949170eb1f93",
    launch(required = [256, 1, 1], max = [256, 1, 1]),
    control_flow(loop_bounds(2, 128, 128))
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
    let index = thread::index_1d();
    let element = index.get();
    if activations.len() != TOKENS * HIDDEN
        || expert_weights.len() != ALL_EXPERTS * HIDDEN * OUTPUT
        || top_experts.len() != TOKENS * TOP_K
        || top_weights.len() != TOKENS * TOP_K
        || output.len() != TOKENS * OUTPUT
    {
        fe2o3_device::trap();
        return;
    }
    if element >= TOKENS * OUTPUT {
        return;
    }
    let token = element / OUTPUT;
    let column = element % OUTPUT;
    let math = DeviceMath::current();
    let mut result = 0.0_f32;
    let mut choice = 0;
    while choice < TOP_K {
        let expert = top_experts[token * TOP_K + choice] as usize;
        if expert >= first_expert as usize && expert < first_expert as usize + 2 {
            let mut value = 0.0_f32;
            let mut depth = 0;
            while depth < HIDDEN {
                value += fp4_e2m1_to_f32(activations[token * HIDDEN + depth])
                    * fp8_e4m3_to_f32(expert_weights[(expert * HIDDEN + depth) * OUTPUT + column]);
                depth += 1;
            }
            result += top_weights[token * TOP_K + choice] * (value / (1.0 + math.exp_f32(-value)));
        }
        choice += 1;
    }
    if include_shared_expert != 0 {
        let mut value = 0.0_f32;
        let mut depth = 0;
        while depth < HIDDEN {
            value += fp4_e2m1_to_f32(activations[token * HIDDEN + depth])
                * fp8_e4m3_to_f32(
                    expert_weights[((ALL_EXPERTS - 1) * HIDDEN + depth) * OUTPUT + column],
                );
            depth += 1;
        }
        result += 0.25 * (value / (1.0 + math.exp_f32(-value)));
    }
    if let Some(slot) = output.get_mut(index) {
        *slot = result;
    }
}

/// Adds two expert-rank partials in fixed rank order.
#[kernel(
    typed,
    namespace = "9dae78d3bd5b2ec5c7c0b67df1ae678dec250360c8fbf783add1bb9f40e72080",
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
        fe2o3_device::trap();
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
#[kernel(
    typed,
    namespace = "ef9c1abf13d31918c6fb03ed1c3bc26827790ca4385cefde3c9432709d7b97e1",
    launch(required = [64, 1, 1], max = [64, 1, 1]),
    control_flow(loop_bounds(4, 8, 4))
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
    mut output_state: DisjointSlice<f32, Blocked<Index1D, 1, 8>>,
) {
    let candidate = thread::index_1d().get();
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
        fe2o3_device::trap();
        return;
    }
    if candidate >= CANDIDATES {
        return;
    }
    let mut accepted = 0;
    while accepted < DRAFT_STEPS
        && draft_tokens[candidate * DRAFT_STEPS + accepted] == target_tokens[accepted]
        && draft_scores[candidate * DRAFT_STEPS + accepted] >= thresholds[accepted]
    {
        accepted += 1;
    }
    let commit = accepted == DRAFT_STEPS;
    if let Some(slot) = accepted_steps.get_mut(thread::index_1d()) {
        *slot = accepted as u32;
    }
    if let Some(slot) = committed.get_mut(thread::index_1d()) {
        *slot = u32::from(commit);
    }
    let Some(block) = thread::index_1d().checked_block::<1, 8>() else {
        fe2o3_device::trap();
        return;
    };
    let mut element = 0;
    while element < STATE_WIDTH {
        let mut value = base_state[element];
        if commit {
            let mut step = 0;
            while step < DRAFT_STEPS {
                value += proposed_deltas[(candidate * DRAFT_STEPS + step) * STATE_WIDTH + element];
                step += 1;
            }
        }
        if let Some(slot) = output_state.get_block_mut(&block, element) {
            *slot = value;
        }
        element += 1;
    }
}

/// Probes every slot, verifies the full 3-gram, and resolves duplicate keys.
#[kernel(
    typed,
    namespace = "56e96094fbf07e6ce2f0008d11126c1ddedb94ccf29918da7d974ebc3e2fcfda",
    launch(required = [64, 1, 1], max = [64, 1, 1]),
    control_flow(loop_bounds(16, 3))
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
        fe2o3_device::trap();
        return;
    }
    if query >= QUERIES {
        return;
    }
    let hash = ngram_hash(queries, query * NGRAM);
    let mut best = usize::MAX;
    let mut probe = 0;
    while probe < TABLE_SIZE {
        let slot = hash.wrapping_add(probe as u64) as usize & (TABLE_SIZE - 1);
        let mut equal = table_hashes[slot] == hash;
        let mut component = 0;
        while component < NGRAM {
            equal = equal
                && table_grams[slot * NGRAM + component] == queries[query * NGRAM + component];
            component += 1;
        }
        if equal
            && (best == usize::MAX
                || priorities[slot] > priorities[best]
                || (priorities[slot] == priorities[best] && slot < best))
        {
            best = slot;
        }
        probe += 1;
    }
    if let Some(slot) = output.get_mut(index) {
        *slot = if best == usize::MAX {
            -1
        } else {
            table_values[best]
        };
    }
}

/// Copies one gradient shard into deterministic transport staging.
#[kernel(
    typed,
    namespace = "9cccc08f502b7275e750276c3951b67daf330bc625340fb2277faa04fd548ed0",
    launch(required = [64, 1, 1], max = [64, 1, 1])
)]
pub fn gfx950_stage_gradient_shard_v1(input: &[f32], mut output: DisjointSlice<f32>) {
    let index = thread::index_1d();
    let element = index.get();
    if input.len() != MUON_ELEMENTS || output.len() != MUON_ELEMENTS {
        fe2o3_device::trap();
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
#[kernel(
    typed,
    namespace = "58f1b2718569a174e0e0220ed0f5dcc3e5d00155021ed64b8500f3ca7bec5d88",
    launch(required = [64, 1, 1], max = [64, 1, 1]),
    control_flow(loop_bounds(16, 2, 16, 16, 5, 4, 4, 4, 4, 4, 4, 16, 16))
)]
pub fn gfx950_muon_update_4x4_v1(
    shards: &[f32],
    mut output: DisjointSlice<f32, GridExclusive>,
    mut output_norm: DisjointSlice<f32, GridExclusive>,
) {
    let Some(leader) = thread::grid_leader() else {
        return;
    };
    if shards.len() != GRADIENT_SHARDS * MUON_ELEMENTS
        || output.len() != MUON_ELEMENTS
        || output_norm.len() != 1
    {
        fe2o3_device::trap();
        return;
    }
    let mut matrix = [0.0_f32; MUON_ELEMENTS];
    let mut element = 0;
    while element < MUON_ELEMENTS {
        let mut shard = 0;
        while shard < GRADIENT_SHARDS {
            matrix[element] += shards[shard * MUON_ELEMENTS + element];
            shard += 1;
        }
        element += 1;
    }
    let mut squared_norm = 0.0_f32;
    element = 0;
    while element < MUON_ELEMENTS {
        squared_norm += matrix[element] * matrix[element];
        element += 1;
    }
    let norm = DeviceMath::current().sqrt_f32(squared_norm);
    *output_norm.get_mut_exclusive(&leader, 0).unwrap() = norm;
    element = 0;
    while element < MUON_ELEMENTS {
        matrix[element] /= norm + 1.0e-6;
        element += 1;
    }
    let mut iteration = 0;
    while iteration < MUON_ITERATIONS {
        let mut gram = [0.0_f32; MUON_ELEMENTS];
        let mut cubic = [0.0_f32; MUON_ELEMENTS];
        let mut row = 0;
        while row < MUON_DIM {
            let mut column = 0;
            while column < MUON_DIM {
                let mut inner = 0;
                while inner < MUON_DIM {
                    gram[row * MUON_DIM + column] +=
                        matrix[row * MUON_DIM + inner] * matrix[column * MUON_DIM + inner];
                    inner += 1;
                }
                column += 1;
            }
            row += 1;
        }
        row = 0;
        while row < MUON_DIM {
            let mut column = 0;
            while column < MUON_DIM {
                let mut inner = 0;
                while inner < MUON_DIM {
                    cubic[row * MUON_DIM + column] +=
                        gram[row * MUON_DIM + inner] * matrix[inner * MUON_DIM + column];
                    inner += 1;
                }
                column += 1;
            }
            row += 1;
        }
        element = 0;
        while element < MUON_ELEMENTS {
            matrix[element] = 1.5 * matrix[element] - 0.5 * cubic[element];
            element += 1;
        }
        iteration += 1;
    }
    element = 0;
    while element < MUON_ELEMENTS {
        *output.get_mut_exclusive(&leader, element).unwrap() =
            -MUON_LEARNING_RATE * matrix[element];
        element += 1;
    }
}
