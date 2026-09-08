//! Complete safe Rust kernel source for the bounded systems kernels.
//!
//! Read every entry point in the same order: validate the whole launch before
//! subgroup operations, map the global thread to a batch/wave/lane owner, build
//! checked typed views, run collectives under uniform control flow, and finish
//! with bounds-checked stores through compiler-bound global-memory roles.
//! Comments emphasize those invariants and the reason for non-obvious code;
//! they intentionally do not narrate ordinary Rust syntax.

#![allow(missing_docs)] // The kernel macro emits helper modules.
#![cfg_attr(target_arch = "amdgpu", allow(unused_imports))]

use fe2o3_device::{
    CapabilityMemoryElementV1, DisjointWrite, ExclusiveReadWrite, Global, Index1D, KernelContext,
    ReadOnly, StrictIeee, SubgroupWidth64, SynchronizationEpoch, WorkgroupCapability, kernel,
};

use crate::{
    ALL_EXPERTS, CANDIDATES, COMBINE_BATCHES, DISPATCH_CAPACITY, DRAFT_STEPS, EXPERTS,
    GRADIENT_SHARDS, HIDDEN, MUON_ELEMENTS, MUON_LEARNING_RATE, NGRAM, OUTPUT, QUERIES,
    STATE_WIDTH, SYSTEM_BATCHES, TABLE_SIZE, TOKENS, TOP_K,
};

#[inline(always)]
fn global_load_2d_or<T: CapabilityMemoryElementV1, Brand>(
    values: &Global<'_, T, ReadOnly, Brand>,
    base: usize,
    row: usize,
    column: usize,
    stride: usize,
    default_value: T,
) -> T {
    let Some(index) = row
        .checked_mul(stride)
        .and_then(|offset| base.checked_add(offset))
        .and_then(|offset| offset.checked_add(column))
    else {
        return default_value;
    };
    values.load(index).unwrap_or(default_value)
}

#[inline(always)]
fn global_store_or_trap<T: CapabilityMemoryElementV1, Brand>(
    values: &mut Global<'_, T, ExclusiveReadWrite, Brand>,
    index: usize,
    value: T,
) {
    if !values.store(index, value) {
        fe2o3_device::trap();
    }
}

#[inline(always)]
fn workgroup_subgroup_slot<const WIDTH: usize>(workgroup_rank: usize, source_lane: usize) -> usize {
    let subgroup_base = workgroup_rank.wrapping_sub(workgroup_rank % WIDTH);
    subgroup_base.wrapping_add(source_lane % WIDTH)
}

#[inline(always)]
fn publish_route_records<'workgroup, KernelBrand, Epoch: SynchronizationEpoch>(
    workgroup: WorkgroupCapability<'workgroup, KernelBrand, Epoch>,
    top_source: usize,
    local_pair: u32,
    first_weight: f32,
    second_weight: f32,
) -> (u32, f32, f32, u64) {
    let workgroup_rank = workgroup.invocation_rank() as usize;
    let routes = workgroup.allocate_lds::<[f32; 3], 256>();
    let routes = routes
        .initialize_by_invocation(&workgroup, [local_pair as f32, first_weight, second_weight]);
    let (workgroup, routes) = workgroup.publish_lds(routes);
    let selected = routes
        .read(
            &workgroup,
            workgroup_subgroup_slot::<64>(workgroup_rank, top_source),
        )
        .unwrap_or([0.0; 3]);
    let wave_base = workgroup_rank & !63;
    let route0 = routes.read(&workgroup, wave_base).unwrap_or([0.0; 3])[0] as u64;
    let route1 = routes.read(&workgroup, wave_base + 1).unwrap_or([0.0; 3])[0] as u64;
    let route2 = routes.read(&workgroup, wave_base + 2).unwrap_or([0.0; 3])[0] as u64;
    let route3 = routes.read(&workgroup, wave_base + 3).unwrap_or([0.0; 3])[0] as u64;
    let route4 = routes.read(&workgroup, wave_base + 4).unwrap_or([0.0; 3])[0] as u64;
    let route5 = routes.read(&workgroup, wave_base + 5).unwrap_or([0.0; 3])[0] as u64;
    let route6 = routes.read(&workgroup, wave_base + 6).unwrap_or([0.0; 3])[0] as u64;
    let route7 = routes.read(&workgroup, wave_base + 7).unwrap_or([0.0; 3])[0] as u64;
    let route8 = routes.read(&workgroup, wave_base + 8).unwrap_or([0.0; 3])[0] as u64;
    let route9 = routes.read(&workgroup, wave_base + 9).unwrap_or([0.0; 3])[0] as u64;
    let route10 = routes.read(&workgroup, wave_base + 10).unwrap_or([0.0; 3])[0] as u64;
    let route11 = routes.read(&workgroup, wave_base + 11).unwrap_or([0.0; 3])[0] as u64;
    let route12 = routes.read(&workgroup, wave_base + 12).unwrap_or([0.0; 3])[0] as u64;
    let route13 = routes.read(&workgroup, wave_base + 13).unwrap_or([0.0; 3])[0] as u64;
    let route14 = routes.read(&workgroup, wave_base + 14).unwrap_or([0.0; 3])[0] as u64;
    let route15 = routes.read(&workgroup, wave_base + 15).unwrap_or([0.0; 3])[0] as u64;
    let packed_routes = route0
        | route1 << 4
        | route2 << 8
        | route3 << 12
        | route4 << 16
        | route5 << 20
        | route6 << 24
        | route7 << 28
        | route8 << 32
        | route9 << 36
        | route10 << 40
        | route11 << 44
        | route12 << 48
        | route13 << 52
        | route14 << 56
        | route15 << 60;
    (selected[0] as u32, selected[1], selected[2], packed_routes)
}

/// Stable top-2 routing, weights, expert counts, and compact dispatch metadata.
#[cfg(any(not(target_arch = "amdgpu"), feature = "kernel-moe-route"))]
#[kernel(
    typed,
    launch(required = [256, 1, 1], max = [256, 1, 1], max_grid = [4, 1, 1]),
    control_flow(loop_bounds(128, 32, 32, 32))
)]
#[allow(clippy::too_many_arguments, unused_assignments)]
pub fn gfx950_moe_route_fp4_t16_e4_k2_v1(
    mut context: KernelContext<'_>,
    activations: Global<'_, u8, ReadOnly>,
    router_weights: Global<'_, f32, ReadOnly>,
    mut top_experts: Global<'_, u32, ExclusiveReadWrite>,
    mut top_weights: Global<'_, f32, ExclusiveReadWrite>,
    mut expert_counts: Global<'_, u32, ExclusiveReadWrite>,
    mut dispatch: Global<'_, i32, ExclusiveReadWrite>,
) {
    // One Wave64 owns one batch. Every lane participates in the broadcasts below.
    let global_index = context.invocation().index_1d().get();
    let batch = global_index / 64;
    let wave_lane = global_index & 63;
    // Reject the complete buffer contract before any lane enters a collective.
    if batch >= SYSTEM_BATCHES
        || activations.len() != SYSTEM_BATCHES * TOKENS * HIDDEN
        || router_weights.len() != SYSTEM_BATCHES * EXPERTS * HIDDEN
        || top_experts.len() != SYSTEM_BATCHES * TOKENS * TOP_K
        || top_weights.len() != SYSTEM_BATCHES * TOKENS * TOP_K
        || expert_counts.len() != SYSTEM_BATCHES * EXPERTS
        || dispatch.len() != SYSTEM_BATCHES * EXPERTS * DISPATCH_CAPACITY
    {
        return;
    }
    let activation_base = batch.wrapping_mul(TOKENS).wrapping_mul(HIDDEN);
    let router_base = batch.wrapping_mul(EXPERTS).wrapping_mul(HIDDEN);
    let token = wave_lane & (TOKENS - 1);
    // Decode packed E2M1 activations once per depth and score all four experts.
    let mut route_logit0 = 0.0_f32;
    let mut route_logit1 = 0.0_f32;
    let mut route_logit2 = 0.0_f32;
    let mut route_logit3 = 0.0_f32;
    let mut depth = 0_usize;
    while depth < HIDDEN {
        let bits = global_load_2d_or(&activations, activation_base, token, depth, HIDDEN, 0);
        let magnitude =
            (0xc864_3210_u32.wrapping_shr(((bits & 7) as u32).wrapping_mul(4)) & 15) as f32 * 0.5;
        let sign = 1.0 - 2.0 * ((bits >> 3) & 1) as f32;
        let activation = sign * magnitude;
        route_logit0 +=
            activation * global_load_2d_or(&router_weights, router_base, 0, depth, HIDDEN, 0.0);
        route_logit1 +=
            activation * global_load_2d_or(&router_weights, router_base, 1, depth, HIDDEN, 0.0);
        route_logit2 +=
            activation * global_load_2d_or(&router_weights, router_base, 2, depth, HIDDEN, 0.0);
        route_logit3 +=
            activation * global_load_2d_or(&router_weights, router_base, 3, depth, HIDDEN, 0.0);
        depth += 1;
    }
    // A branch-light ranking network gives deterministic top-2 tie handling.
    let precedes12 = (route_logit1 >= route_logit2) as u32;
    let precedes13 = (route_logit1 >= route_logit3) as u32;
    let precedes23 = (route_logit2 >= route_logit3) as u32;
    let rank1 = ((route_logit0 >= route_logit1) as u32)
        .wrapping_add(2)
        .wrapping_sub(precedes12)
        .wrapping_sub(precedes13);
    let rank2 = ((route_logit0 >= route_logit2) as u32)
        .wrapping_add(precedes12)
        .wrapping_add(1)
        .wrapping_sub(precedes23);
    let rank3 = ((route_logit0 >= route_logit3) as u32)
        .wrapping_add(precedes13)
        .wrapping_add(precedes23);
    let first_local = ((rank1 == 0) as u32)
        .wrapping_add(2_u32.wrapping_mul((rank2 == 0) as u32))
        .wrapping_add(3_u32.wrapping_mul((rank3 == 0) as u32));
    let second_local = ((rank1 == 1) as u32)
        .wrapping_add(2_u32.wrapping_mul((rank2 == 1) as u32))
        .wrapping_add(3_u32.wrapping_mul((rank3 == 1) as u32));
    let first_logit = if first_local == 0 {
        route_logit0
    } else if first_local == 1 {
        route_logit1
    } else if first_local == 2 {
        route_logit2
    } else {
        route_logit3
    };
    let second_logit = if second_local == 0 {
        route_logit0
    } else if second_local == 1 {
        route_logit1
    } else if second_local == 2 {
        route_logit2
    } else {
        route_logit3
    };
    // Normalize only the selected logits, using the max subtraction for stability.
    let maximum = if first_logit > second_logit {
        first_logit
    } else {
        second_logit
    };
    let policy = context.numerical_policy::<StrictIeee>();
    let math_capability = context.math();
    let math = math_capability.with_numerical_policy(&policy);
    let first_exp = math.exp_f32(first_logit - maximum);
    let second_exp = math.exp_f32(second_logit - maximum);
    let denominator = first_exp + second_exp;
    let first_weight_local = first_exp / denominator;
    let second_weight_local = second_exp / denominator;
    // Publish one route record per lane before other lanes consume it.
    let top_source = (wave_lane / TOP_K) & (TOKENS - 1);
    let local_pair = first_local | (second_local << 2);
    let (top_pair, top_first_weight, top_second_weight, packed_routes) =
        context.with_workgroup(|workgroup| {
            publish_route_records(
                workgroup,
                top_source,
                local_pair,
                first_weight_local,
                second_weight_local,
            )
        });
    let top_first = top_pair & 3;
    let top_second = top_pair >> 2;
    // Each compact index is injective over the batch/lane partition.
    if wave_lane < TOKENS * TOP_K {
        let choice = wave_lane & (TOP_K - 1);
        let selected = if choice == 0 { top_first } else { top_second };
        let weight = if choice == 0 {
            top_first_weight
        } else {
            top_second_weight
        };
        let top_index = batch.wrapping_mul(TOKENS * TOP_K).wrapping_add(wave_lane);
        global_store_or_trap(&mut top_experts, top_index, selected);
        global_store_or_trap(&mut top_weights, top_index, weight);
    }
    let count_expert = wave_lane as u32;
    let mut count = 0_u32;
    let mut record = 0_usize;
    while record < TOKENS * TOP_K {
        let selected = (packed_routes.wrapping_shr(2_usize.wrapping_mul(record) as u32) & 3) as u32;
        count = count.wrapping_add((selected == count_expert) as u32);
        record += 1;
    }
    if wave_lane < EXPERTS {
        let count_index = batch.wrapping_mul(EXPERTS).wrapping_add(wave_lane);
        global_store_or_trap(&mut expert_counts, count_index, count);
    }
    let dispatch_base = batch.wrapping_mul(EXPERTS).wrapping_mul(DISPATCH_CAPACITY);
    let dispatch_expert0 = (wave_lane / DISPATCH_CAPACITY) as u32;
    let wanted0 =
        wave_lane.wrapping_sub((dispatch_expert0 as usize).wrapping_mul(DISPATCH_CAPACITY));
    let mut seen0 = 0_usize;
    let mut dispatched0 = -1_i32;
    let mut route0 = 0_usize;
    while route0 < TOKENS * TOP_K {
        let selected = (packed_routes.wrapping_shr(2_usize.wrapping_mul(route0) as u32) & 3) as u32;
        let dispatch_matches = (selected == dispatch_expert0) as usize;
        let choose = ((dispatch_matches != 0) & (seen0 == wanted0)) as i32;
        dispatched0 = dispatched0.wrapping_add(
            (route0 as i32)
                .wrapping_sub(dispatched0)
                .wrapping_mul(choose),
        );
        seen0 = seen0.wrapping_add(dispatch_matches);
        route0 += 1;
    }
    global_store_or_trap(
        &mut dispatch,
        dispatch_base.wrapping_add(wave_lane),
        dispatched0,
    );
    let dispatch_element1 = wave_lane.wrapping_add(64);
    let dispatch_expert1 = (dispatch_element1 / DISPATCH_CAPACITY) as u32;
    let wanted1 =
        dispatch_element1.wrapping_sub((dispatch_expert1 as usize).wrapping_mul(DISPATCH_CAPACITY));
    let mut seen1 = 0_usize;
    let mut dispatched1 = -1_i32;
    let mut route1 = 0_usize;
    while route1 < TOKENS * TOP_K {
        let selected = (packed_routes.wrapping_shr(2_usize.wrapping_mul(route1) as u32) & 3) as u32;
        let dispatch_matches = (selected == dispatch_expert1) as usize;
        let choose = ((dispatch_matches != 0) & (seen1 == wanted1)) as i32;
        dispatched1 = dispatched1.wrapping_add(
            (route1 as i32)
                .wrapping_sub(dispatched1)
                .wrapping_mul(choose),
        );
        seen1 = seen1.wrapping_add(dispatch_matches);
        route1 += 1;
    }
    global_store_or_trap(
        &mut dispatch,
        dispatch_base.wrapping_add(dispatch_element1),
        dispatched1,
    );
}

/// Computes a routed expert partition and optional shared-expert contribution.
#[cfg(any(not(target_arch = "amdgpu"), feature = "kernel-moe-expert-rank"))]
#[cfg_attr(
    not(feature = "ablation-expert-serial"),
    kernel(
        typed,
        launch(required = [256, 1, 1], max = [256, 1, 1], max_grid = [4, 1, 1])
    )
)]
#[cfg_attr(
    feature = "ablation-expert-serial",
    kernel(
        typed,
        launch(required = [256, 1, 1], max = [256, 1, 1], max_grid = [4, 1, 1])
    )
)]
#[allow(clippy::too_many_arguments)]
pub fn gfx950_moe_expert_rank_fp4_fp8_v1(
    mut context: KernelContext<'_>,
    activations: Global<'_, u8, ReadOnly>,
    expert_weights: Global<'_, u8, ReadOnly>,
    top_experts: Global<'_, u32, ReadOnly>,
    top_weights: Global<'_, f32, ReadOnly>,
    first_expert: u32,
    include_shared_expert: u32,
    mut output: Global<'_, f32, ExclusiveReadWrite>,
) {
    // One Wave64 owns one batch; each lane ultimately writes four output elements.
    let thread_index = context.invocation().index_1d();
    let global_index = thread_index.get();
    let batch = global_index / 64;
    let lane_index = global_index & 63;
    // Validate every buffer and rank range before the first MFMA or broadcast.
    if batch >= SYSTEM_BATCHES
        || activations.len() != SYSTEM_BATCHES * TOKENS * HIDDEN
        || expert_weights.len() != SYSTEM_BATCHES * ALL_EXPERTS * HIDDEN * OUTPUT
        || top_experts.len() != SYSTEM_BATCHES * TOKENS * TOP_K
        || top_weights.len() != SYSTEM_BATCHES * TOKENS * TOP_K
        || output.len() != SYSTEM_BATCHES * TOKENS * OUTPUT
        || first_expert as usize >= EXPERTS - 1
    {
        return;
    }
    // Build policy-bound Global views for two routed experts plus one shared expert.
    let second_expert = first_expert.wrapping_add(1);
    let activation_base = batch.wrapping_mul(TOKENS).wrapping_mul(HIDDEN);
    let expert_batch_base = batch
        .wrapping_mul(ALL_EXPERTS)
        .wrapping_mul(HIDDEN)
        .wrapping_mul(OUTPUT);
    let route_batch_base = batch.wrapping_mul(TOKENS).wrapping_mul(TOP_K);
    let first_offset = expert_batch_base.wrapping_add(
        (first_expert as usize)
            .wrapping_mul(HIDDEN)
            .wrapping_mul(OUTPUT),
    );
    let policy = context.numerical_policy::<StrictIeee>();
    let math_capability = context.math();
    let math = math_capability.with_numerical_policy(&policy);
    let matrix_values = context.with_workgroup(|workgroup| {
        let subgroup = workgroup.subgroup::<SubgroupWidth64>();
        subgroup.with_matrix(workgroup.epoch(), |matrix, lane| {
            let policy_matrix = matrix.with_numerical_policy(&policy);
            let matrix = policy_matrix.gfx950();
            let Ok(activations_view) = matrix.fp4_a_global_row_major(
                &activations,
                activation_base,
                TOKENS,
                HIDDEN,
                HIDDEN,
            ) else {
                return None;
            };

            // Keep all three independent fragment lifetimes overlapping in the production path.
            #[cfg(not(feature = "ablation-expert-serial"))]
            {
                let activations_first = activations_view.load_m16k128(lane, 0, 0);
                let activations_second = activations_view.load_m16k128(lane, 0, 0);
                let activations_shared = activations_view.load_m16k128(lane, 0, 0);
                let Ok(first_weights_view) = matrix.fp8_b_global_row_major(
                    &expert_weights,
                    first_offset,
                    HIDDEN,
                    OUTPUT,
                    OUTPUT,
                ) else {
                    return None;
                };
                let first_weights = first_weights_view.load_k128n16(lane, 0, 0);
                let Ok(second_weights_view) = matrix.fp8_b_global_row_major(
                    &expert_weights,
                    first_offset.wrapping_add(HIDDEN * OUTPUT),
                    HIDDEN,
                    OUTPUT,
                    OUTPUT,
                ) else {
                    return None;
                };
                let second_weights = second_weights_view.load_k128n16(lane, 0, 0);
                let Ok(shared_weights_view) = matrix.fp8_b_global_row_major(
                    &expert_weights,
                    expert_batch_base.wrapping_add((ALL_EXPERTS - 1) * HIDDEN * OUTPUT),
                    HIDDEN,
                    OUTPUT,
                    OUTPUT,
                ) else {
                    return None;
                };
                let shared_weights = shared_weights_view.load_k128n16(lane, 0, 0);
                Some((
                    matrix
                        .multiply_accumulate_fp4_fp8(
                            activations_first,
                            first_weights,
                            matrix.fp4_zero_accumulator(lane),
                        )
                        .into_values(),
                    matrix
                        .multiply_accumulate_fp4_fp8(
                            activations_second,
                            second_weights,
                            matrix.fp4_zero_accumulator(lane),
                        )
                        .into_values(),
                    matrix
                        .multiply_accumulate_fp4_fp8(
                            activations_shared,
                            shared_weights,
                            matrix.fp4_zero_accumulator(lane),
                        )
                        .into_values(),
                ))
            }

            #[cfg(feature = "ablation-expert-serial")]
            {
                let activations_first = activations_view.load_m16k128(lane, 0, 0);
                let Ok(first_weights_view) = matrix.fp8_b_global_row_major(
                    &expert_weights,
                    first_offset,
                    HIDDEN,
                    OUTPUT,
                    OUTPUT,
                ) else {
                    return None;
                };
                let first_weights = first_weights_view.load_k128n16(lane, 0, 0);
                let first_values = matrix
                    .multiply_accumulate_fp4_fp8(
                        activations_first,
                        first_weights,
                        matrix.fp4_zero_accumulator(lane),
                    )
                    .into_values();
                let activations_second = activations_view.load_m16k128(lane, 0, 0);
                let Ok(second_weights_view) = matrix.fp8_b_global_row_major(
                    &expert_weights,
                    first_offset.wrapping_add(HIDDEN * OUTPUT),
                    HIDDEN,
                    OUTPUT,
                    OUTPUT,
                ) else {
                    return None;
                };
                let second_weights = second_weights_view.load_k128n16(lane, 0, 0);
                let second_values = matrix
                    .multiply_accumulate_fp4_fp8(
                        activations_second,
                        second_weights,
                        matrix.fp4_zero_accumulator(lane),
                    )
                    .into_values();
                let activations_shared = activations_view.load_m16k128(lane, 0, 0);
                let Ok(shared_weights_view) = matrix.fp8_b_global_row_major(
                    &expert_weights,
                    expert_batch_base.wrapping_add((ALL_EXPERTS - 1) * HIDDEN * OUTPUT),
                    HIDDEN,
                    OUTPUT,
                    OUTPUT,
                ) else {
                    return None;
                };
                let shared_weights = shared_weights_view.load_k128n16(lane, 0, 0);
                let shared_values = matrix
                    .multiply_accumulate_fp4_fp8(
                        activations_shared,
                        shared_weights,
                        matrix.fp4_zero_accumulator(lane),
                    )
                    .into_values();
                Some((first_values, second_values, shared_values))
            }
        })
    });
    let Some((first_values, second_values, shared_values)) = matrix_values else {
        return;
    };
    // Publish all accumulator components once, then gather each output owner's source lane.
    let redistributed = context.with_workgroup(|workgroup| {
        let workgroup_rank = workgroup.invocation_rank() as usize;
        let fragments = workgroup.allocate_lds::<[f32; 12], 256>();
        let fragments = fragments.initialize_by_invocation(
            &workgroup,
            [
                first_values[0],
                first_values[1],
                first_values[2],
                first_values[3],
                second_values[0],
                second_values[1],
                second_values[2],
                second_values[3],
                shared_values[0],
                shared_values[1],
                shared_values[2],
                shared_values[3],
            ],
        );
        let (workgroup, fragments) = workgroup.publish_lds(fragments);
        let source0 = ((lane_index / OUTPUT / 4) * OUTPUT + lane_index % OUTPUT) & 63;
        let element1 = lane_index.wrapping_add(64);
        let source1 = ((element1 / OUTPUT / 4) * OUTPUT + element1 % OUTPUT) & 63;
        let element2 = lane_index.wrapping_add(128);
        let source2 = ((element2 / OUTPUT / 4) * OUTPUT + element2 % OUTPUT) & 63;
        let element3 = lane_index.wrapping_add(192);
        let source3 = ((element3 / OUTPUT / 4) * OUTPUT + element3 % OUTPUT) & 63;
        [
            fragments
                .read(
                    &workgroup,
                    workgroup_subgroup_slot::<64>(workgroup_rank, source0),
                )
                .unwrap_or([0.0; 12]),
            fragments
                .read(
                    &workgroup,
                    workgroup_subgroup_slot::<64>(workgroup_rank, source1),
                )
                .unwrap_or([0.0; 12]),
            fragments
                .read(
                    &workgroup,
                    workgroup_subgroup_slot::<64>(workgroup_rank, source2),
                )
                .unwrap_or([0.0; 12]),
            fragments
                .read(
                    &workgroup,
                    workgroup_subgroup_slot::<64>(workgroup_rank, source3),
                )
                .unwrap_or([0.0; 12]),
        ]
    });

    // Apply the two route gates and optional shared expert after redistribution.
    macro_rules! compute_component {
        ($output_component:literal) => {{
            let element = lane_index.wrapping_add($output_component * 64);
            let token = element / OUTPUT;
            let accumulator_component = token & 3;
            let values = redistributed[$output_component];
            let first = values[accumulator_component];
            let second = values[4 + accumulator_component];
            let shared = values[8 + accumulator_component];
            let route_base = route_batch_base.wrapping_add(token.wrapping_mul(TOP_K));
            let route_second = route_base.wrapping_add(1);
            let selected0 = top_experts.load(route_base).unwrap_or(0);
            let selected1 = top_experts.load(route_second).unwrap_or(0);
            let gate0 = top_weights.load(route_base).unwrap_or(0.0);
            let gate1 = top_weights.load(route_second).unwrap_or(0.0);
            let mut result = 0.0_f32;
            if selected0 == first_expert {
                result += gate0 * (first / (1.0 + math.exp_f32(-first)));
            } else if selected0 == second_expert {
                result += gate0 * (second / (1.0 + math.exp_f32(-second)));
            }
            if selected1 == first_expert {
                result += gate1 * (first / (1.0 + math.exp_f32(-first)));
            } else if selected1 == second_expert {
                result += gate1 * (second / (1.0 + math.exp_f32(-second)));
            }
            if include_shared_expert != 0 {
                result += 0.25 * (shared / (1.0 + math.exp_f32(-shared)));
            }
            result
        }};
    }
    let result0 = compute_component!(0);
    let result1 = compute_component!(1);
    let result2 = compute_component!(2);
    let result3 = compute_component!(3);
    // The compiler proves this blocked formula injective over lane and component.
    let output_base = batch.wrapping_mul(TOKENS).wrapping_mul(OUTPUT);
    global_store_or_trap(&mut output, output_base.wrapping_add(lane_index), result0);
    global_store_or_trap(
        &mut output,
        output_base.wrapping_add(64).wrapping_add(lane_index),
        result1,
    );
    global_store_or_trap(
        &mut output,
        output_base.wrapping_add(128).wrapping_add(lane_index),
        result2,
    );
    global_store_or_trap(
        &mut output,
        output_base.wrapping_add(192).wrapping_add(lane_index),
        result3,
    );
}

/// Adds two expert-rank partials in fixed rank order.
#[cfg(any(not(target_arch = "amdgpu"), feature = "kernel-combine-expert-ranks"))]
#[cfg_attr(
    not(feature = "ablation-combine-transposed"),
    kernel(
        typed,
        launch(required = [256, 1, 1], max = [256, 1, 1], max_grid = [4, 1, 1])
    )
)]
#[cfg_attr(
    feature = "ablation-combine-transposed",
    kernel(
        typed,
        launch(required = [256, 1, 1], max = [256, 1, 1], max_grid = [4, 1, 1])
    )
)]
#[allow(unused_mut)]
pub fn gfx950_combine_expert_ranks_v1(
    mut context: KernelContext<'_>,
    rank0: Global<'_, f32, ReadOnly>,
    rank1: Global<'_, f32, ReadOnly>,
    mut output: Global<'_, f32, DisjointWrite<Index1D>>,
) {
    // This elementwise boundary has no collectives in the production path.
    let index = context.invocation().index_1d();
    let element = index.get();
    if rank0.len() != COMBINE_BATCHES * TOKENS * OUTPUT
        || rank1.len() != COMBINE_BATCHES * TOKENS * OUTPUT
        || output.len() != COMBINE_BATCHES * TOKENS * OUTPUT
    {
        return;
    }
    if element >= COMBINE_BATCHES * TOKENS * OUTPUT {
        return;
    }
    // Fixed rank order makes the floating-point reference and device result reproducible.
    #[cfg(not(feature = "ablation-combine-transposed"))]
    let result = rank0.load(element).unwrap_or(0.0) + rank1.load(element).unwrap_or(0.0);
    #[cfg(feature = "ablation-combine-transposed")]
    let result = {
        let wave_lane = element & 63;
        let source_lane = 63 - wave_lane;
        let source_element = (element & !63) + source_lane;
        let source_result =
            rank0.load(source_element).unwrap_or(0.0) + rank1.load(source_element).unwrap_or(0.0);
        context.with_workgroup(|workgroup| {
            let workgroup_rank = workgroup.invocation_rank() as usize;
            let values = workgroup.allocate_lds::<f32, 256>();
            let values = values.initialize_by_invocation(&workgroup, source_result);
            let (workgroup, values) = workgroup.publish_lds(values);
            values
                .read(
                    &workgroup,
                    workgroup_subgroup_slot::<64>(workgroup_rank, source_lane),
                )
                .unwrap_or(0.0)
        })
    };
    if !output.store(index.into_disjoint(), result) {
        fe2o3_device::trap();
    }
}

/// Commits state only when every speculative token and score is accepted.
#[cfg(any(
    not(target_arch = "amdgpu"),
    feature = "kernel-speculative-transaction"
))]
#[cfg_attr(
    not(feature = "ablation-speculative-recompute-prefix"),
    kernel(
        typed,
        launch(required = [256, 1, 1], max = [256, 1, 1], max_grid = [4, 1, 1])
    )
)]
#[cfg_attr(
    feature = "ablation-speculative-recompute-prefix",
    kernel(
        typed,
        launch(required = [256, 1, 1], max = [256, 1, 1], max_grid = [4, 1, 1])
    )
)]
#[allow(clippy::too_many_arguments, unused_mut)]
pub fn gfx950_speculative_transaction_v1(
    mut context: KernelContext<'_>,
    draft_tokens: Global<'_, i32, ReadOnly>,
    target_tokens: Global<'_, i32, ReadOnly>,
    draft_scores: Global<'_, f32, ReadOnly>,
    thresholds: Global<'_, f32, ReadOnly>,
    base_state: Global<'_, f32, ReadOnly>,
    proposed_deltas: Global<'_, f32, ReadOnly>,
    mut accepted_steps: Global<'_, u32, ExclusiveReadWrite>,
    mut committed: Global<'_, u32, ExclusiveReadWrite>,
    mut output_state: Global<'_, f32, DisjointWrite<Index1D>>,
) {
    // One Wave64 owns a batch: lanes first evaluate candidates, then own state elements.
    let global_index = context.invocation().index_1d().get();
    let batch = global_index / 64;
    let lane = global_index & 63;
    // Establish the full transactional buffer contract before subgroup broadcasts.
    if batch >= SYSTEM_BATCHES
        || draft_tokens.len() != SYSTEM_BATCHES * CANDIDATES * DRAFT_STEPS
        || target_tokens.len() != SYSTEM_BATCHES * DRAFT_STEPS
        || draft_scores.len() != SYSTEM_BATCHES * CANDIDATES * DRAFT_STEPS
        || thresholds.len() != SYSTEM_BATCHES * DRAFT_STEPS
        || base_state.len() != SYSTEM_BATCHES * STATE_WIDTH
        || proposed_deltas.len() != SYSTEM_BATCHES * CANDIDATES * DRAFT_STEPS * STATE_WIDTH
        || accepted_steps.len() != SYSTEM_BATCHES * CANDIDATES
        || committed.len() != SYSTEM_BATCHES * CANDIDATES
        || output_state.len() != SYSTEM_BATCHES * CANDIDATES * STATE_WIDTH
    {
        return;
    }
    let transaction_base = batch.wrapping_mul(CANDIDATES).wrapping_mul(DRAFT_STEPS);
    let target_base = batch.wrapping_mul(DRAFT_STEPS);
    let state_base = batch.wrapping_mul(STATE_WIDTH);
    let delta_base = batch
        .wrapping_mul(CANDIDATES)
        .wrapping_mul(DRAFT_STEPS)
        .wrapping_mul(STATE_WIDTH);
    macro_rules! target_token {
        ($step:expr) => {
            global_load_2d_or(&target_tokens, target_base, 0, $step, DRAFT_STEPS, 0)
        };
    }
    macro_rules! threshold {
        ($step:expr) => {
            global_load_2d_or(&thresholds, target_base, 0, $step, DRAFT_STEPS, 0.0)
        };
    }
    macro_rules! draft_token {
        ($candidate:expr, $step:expr) => {
            global_load_2d_or(
                &draft_tokens,
                transaction_base,
                $candidate,
                $step,
                DRAFT_STEPS,
                0,
            )
        };
    }
    macro_rules! draft_score {
        ($candidate:expr, $step:expr) => {
            global_load_2d_or(
                &draft_scores,
                transaction_base,
                $candidate,
                $step,
                DRAFT_STEPS,
                0.0,
            )
        };
    }
    #[cfg(feature = "ablation-speculative-recompute-prefix")]
    macro_rules! accepted_prefix {
        ($candidate:expr) => {{
            let accepts0 = (draft_token!($candidate, 0) == target_token!(0))
                & (draft_score!($candidate, 0) >= threshold!(0));
            let accepts1 = accepts0
                & (draft_token!($candidate, 1) == target_token!(1))
                & (draft_score!($candidate, 1) >= threshold!(1));
            let accepts2 = accepts1
                & (draft_token!($candidate, 2) == target_token!(2))
                & (draft_score!($candidate, 2) >= threshold!(2));
            let accepts3 = accepts2
                & (draft_token!($candidate, 3) == target_token!(3))
                & (draft_score!($candidate, 3) >= threshold!(3));
            (accepts0 as usize)
                .wrapping_add(accepts1 as usize)
                .wrapping_add(accepts2 as usize)
                .wrapping_add(accepts3 as usize)
        }};
    }
    // Build a prefix: a later step can be accepted only when every earlier step was.
    let acceptance_candidate = lane & (CANDIDATES - 1);
    let accepts0 = (draft_token!(acceptance_candidate, 0) == target_token!(0))
        & (draft_score!(acceptance_candidate, 0) >= threshold!(0));
    let accepts1 = accepts0
        & (draft_token!(acceptance_candidate, 1) == target_token!(1))
        & (draft_score!(acceptance_candidate, 1) >= threshold!(1));
    let accepts2 = accepts1
        & (draft_token!(acceptance_candidate, 2) == target_token!(2))
        & (draft_score!(acceptance_candidate, 2) >= threshold!(2));
    let accepts3 = accepts2
        & (draft_token!(acceptance_candidate, 3) == target_token!(3))
        & (draft_score!(acceptance_candidate, 3) >= threshold!(3));
    let accepted_local = (accepts0 as usize)
        .wrapping_add(accepts1 as usize)
        .wrapping_add(accepts2 as usize)
        .wrapping_add(accepts3 as usize);
    // Re-map lanes to candidate/state pairs and broadcast one decision per candidate.
    let candidate = lane / STATE_WIDTH;
    let state_element = lane.wrapping_sub(candidate.wrapping_mul(STATE_WIDTH));
    #[cfg(not(feature = "ablation-speculative-recompute-prefix"))]
    let accepted = context.with_workgroup(|workgroup| {
        let workgroup_rank = workgroup.invocation_rank() as usize;
        let accepted = workgroup.allocate_lds::<u32, 256>();
        let accepted = accepted.initialize_by_invocation(&workgroup, accepted_local as u32);
        let (workgroup, accepted) = workgroup.publish_lds(accepted);
        accepted
            .read(
                &workgroup,
                workgroup_subgroup_slot::<64>(workgroup_rank, candidate),
            )
            .unwrap_or(0) as usize
    });
    #[cfg(feature = "ablation-speculative-recompute-prefix")]
    let accepted = accepted_prefix!(candidate);
    // Commit status and all state deltas together; rejected candidates retain base state.
    if lane < CANDIDATES {
        let status_index = batch.wrapping_mul(CANDIDATES).wrapping_add(lane);
        global_store_or_trap(&mut accepted_steps, status_index, accepted_local as u32);
        global_store_or_trap(
            &mut committed,
            status_index,
            if accepted_local == DRAFT_STEPS { 1 } else { 0 },
        );
    }
    let mut value = base_state
        .load(state_base.wrapping_add(state_element))
        .unwrap_or(0.0);
    if accepted == DRAFT_STEPS {
        let candidate_base = delta_base.wrapping_add(
            candidate
                .wrapping_mul(DRAFT_STEPS)
                .wrapping_mul(STATE_WIDTH),
        );
        value += proposed_deltas
            .load(candidate_base.wrapping_add(state_element))
            .unwrap_or(0.0);
        value += proposed_deltas
            .load(
                candidate_base
                    .wrapping_add(STATE_WIDTH)
                    .wrapping_add(state_element),
            )
            .unwrap_or(0.0);
        value += proposed_deltas
            .load(
                candidate_base
                    .wrapping_add(2 * STATE_WIDTH)
                    .wrapping_add(state_element),
            )
            .unwrap_or(0.0);
        value += proposed_deltas
            .load(
                candidate_base
                    .wrapping_add(3 * STATE_WIDTH)
                    .wrapping_add(state_element),
            )
            .unwrap_or(0.0);
    }
    // The one-dimensional capability gives every lane a unique state destination.
    if !output_state.store(context.invocation().index_1d().into_disjoint(), value) {
        fe2o3_device::trap();
    }
}

/// Probes every slot, verifies the full 3-gram, and resolves duplicate keys.
#[cfg(any(not(target_arch = "amdgpu"), feature = "kernel-qwen-ngram-gather"))]
#[cfg_attr(
    not(feature = "ablation-ngram-reverse-probe"),
    kernel(
        typed,
        launch(required = [256, 1, 1], max = [256, 1, 1], max_grid = [4, 1, 1])
    )
)]
#[cfg_attr(
    feature = "ablation-ngram-reverse-probe",
    kernel(
        typed,
        launch(required = [256, 1, 1], max = [256, 1, 1], max_grid = [4, 1, 1])
    )
)]
#[allow(unused_assignments)]
pub fn gfx950_qwen_ngram_gather_v1(
    context: KernelContext<'_>,
    queries: Global<'_, i32, ReadOnly>,
    table_hashes: Global<'_, u64, ReadOnly>,
    table_grams: Global<'_, i32, ReadOnly>,
    table_values: Global<'_, i32, ReadOnly>,
    priorities: Global<'_, i32, ReadOnly>,
    mut output: Global<'_, i32, ExclusiveReadWrite>,
) {
    // Each in-range lane owns one query; no subgroup coordination is required.
    let global_index = context.invocation().index_1d().get();
    let batch = global_index / 64;
    let query = global_index & 63;
    if batch >= SYSTEM_BATCHES
        || queries.len() != SYSTEM_BATCHES * QUERIES * NGRAM
        || table_hashes.len() != SYSTEM_BATCHES * TABLE_SIZE
        || table_grams.len() != SYSTEM_BATCHES * TABLE_SIZE * NGRAM
        || table_values.len() != SYSTEM_BATCHES * TABLE_SIZE
        || priorities.len() != SYSTEM_BATCHES * TABLE_SIZE
        || output.len() != SYSTEM_BATCHES * QUERIES
    {
        return;
    }
    if query >= QUERIES {
        return;
    }
    // Hash all three tokens; the full gram is still checked to reject collisions.
    let query_batch_base = batch.wrapping_mul(QUERIES).wrapping_mul(NGRAM);
    let table_batch_base = batch.wrapping_mul(TABLE_SIZE);
    let gram_batch_base = batch.wrapping_mul(TABLE_SIZE).wrapping_mul(NGRAM);
    let base = query_batch_base.wrapping_add(query.wrapping_mul(NGRAM));
    let mut hash = 1_469_598_103_934_665_603_u64;
    hash ^= queries.load(base).unwrap_or(0) as u32 as u64;
    hash = hash.wrapping_mul(1_099_511_628_211);
    hash ^= queries.load(base.wrapping_add(1)).unwrap_or(0) as u32 as u64;
    hash = hash.wrapping_mul(1_099_511_628_211);
    hash ^= queries.load(base.wrapping_add(2)).unwrap_or(0) as u32 as u64;
    hash = hash.wrapping_mul(1_099_511_628_211);
    let mut best_slot = usize::MAX;
    let mut best_priority = i32::MIN;
    let mut best_value = -1_i32;
    // Probe the bounded table completely and resolve duplicate keys deterministically.
    macro_rules! probe {
        ($probe:literal) => {{
            let slot = hash.wrapping_add($probe) as usize & (TABLE_SIZE - 1);
            let table_slot = table_batch_base.wrapping_add(slot);
            let gram_slot = gram_batch_base.wrapping_add(slot.wrapping_mul(NGRAM));
            let equal = (table_hashes.load(table_slot).unwrap_or(0) == hash)
                & (table_grams.load(gram_slot).unwrap_or(0) == queries.load(base).unwrap_or(0))
                & (table_grams.load(gram_slot.wrapping_add(1)).unwrap_or(0)
                    == queries.load(base.wrapping_add(1)).unwrap_or(0))
                & (table_grams.load(gram_slot.wrapping_add(2)).unwrap_or(0)
                    == queries.load(base.wrapping_add(2)).unwrap_or(0));
            if equal {
                let priority = priorities.load(table_slot).unwrap_or(i32::MIN);
                if priority > best_priority || (priority == best_priority && slot < best_slot) {
                    best_slot = slot;
                    best_priority = priority;
                    best_value = table_values.load(table_slot).unwrap_or(-1);
                }
            }
        }};
    }
    #[cfg(not(feature = "ablation-ngram-reverse-probe"))]
    macro_rules! final_probe {
        ($probe:literal) => {{
            let slot = hash.wrapping_add($probe) as usize & (TABLE_SIZE - 1);
            let table_slot = table_batch_base.wrapping_add(slot);
            let gram_slot = gram_batch_base.wrapping_add(slot.wrapping_mul(NGRAM));
            let equal = (table_hashes.load(table_slot).unwrap_or(0) == hash)
                & (table_grams.load(gram_slot).unwrap_or(0) == queries.load(base).unwrap_or(0))
                & (table_grams.load(gram_slot.wrapping_add(1)).unwrap_or(0)
                    == queries.load(base.wrapping_add(1)).unwrap_or(0))
                & (table_grams.load(gram_slot.wrapping_add(2)).unwrap_or(0)
                    == queries.load(base.wrapping_add(2)).unwrap_or(0));
            if equal {
                let priority = priorities.load(table_slot).unwrap_or(i32::MIN);
                if priority > best_priority || (priority == best_priority && slot < best_slot) {
                    best_value = table_values.load(table_slot).unwrap_or(-1);
                }
            }
        }};
    }
    #[cfg(not(feature = "ablation-ngram-reverse-probe"))]
    {
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
    }
    #[cfg(feature = "ablation-ngram-reverse-probe")]
    {
        probe!(15);
        probe!(14);
        probe!(13);
        probe!(12);
        probe!(11);
        probe!(10);
        probe!(9);
        probe!(8);
        probe!(7);
        probe!(6);
        probe!(5);
        probe!(4);
        probe!(3);
        probe!(2);
        probe!(1);
        probe!(0);
    }
    // The compact batch/query index is unique for every active invocation.
    let output_index = batch.wrapping_mul(QUERIES).wrapping_add(query);
    global_store_or_trap(&mut output, output_index, best_value);
}

/// Copies one gradient shard into deterministic transport staging.
#[cfg(any(not(target_arch = "amdgpu"), feature = "kernel-stage-gradient-shard"))]
#[cfg_attr(
    not(feature = "ablation-stage-tile4"),
    kernel(
        typed,
        launch(required = [256, 1, 1], max = [256, 1, 1], max_grid = [4, 1, 1])
    )
)]
#[cfg_attr(
    feature = "ablation-stage-tile4",
    kernel(
        typed,
        launch(required = [256, 1, 1], max = [256, 1, 1], max_grid = [4, 1, 1])
    )
)]
#[allow(unused_mut)]
pub fn gfx950_stage_gradient_shard_v1(
    mut context: KernelContext<'_>,
    input: Global<'_, f32, ReadOnly>,
    mut output: Global<'_, f32, ExclusiveReadWrite>,
) {
    // One wave stages one batch; only the first 16 lanes correspond to matrix elements.
    let global_index = context.invocation().index_1d().get();
    let batch = global_index / 64;
    let element = global_index & 63;
    if batch >= SYSTEM_BATCHES
        || input.len() != SYSTEM_BATCHES * MUON_ELEMENTS
        || output.len() != SYSTEM_BATCHES * MUON_ELEMENTS
    {
        return;
    }
    #[cfg(not(feature = "ablation-stage-tile4"))]
    if element >= MUON_ELEMENTS {
        return;
    }
    let input_base = batch.wrapping_mul(MUON_ELEMENTS);
    // The production path is a direct coalesced copy; the tile path is an ablation only.
    #[cfg(not(feature = "ablation-stage-tile4"))]
    let value = input.load(input_base.wrapping_add(element)).unwrap_or(0.0);
    #[cfg(feature = "ablation-stage-tile4")]
    let value = context.with_workgroup(|workgroup| {
        let mut tile0 = 0.0_f32;
        let mut tile1 = 0.0_f32;
        let mut tile2 = 0.0_f32;
        let mut tile3 = 0.0_f32;
        if element < 4 {
            let tile_base = element * 4;
            tile0 = input
                .load(input_base.wrapping_add(tile_base))
                .unwrap_or(0.0);
            tile1 = input
                .load(input_base.wrapping_add(tile_base).wrapping_add(1))
                .unwrap_or(0.0);
            tile2 = input
                .load(input_base.wrapping_add(tile_base).wrapping_add(2))
                .unwrap_or(0.0);
            tile3 = input
                .load(input_base.wrapping_add(tile_base).wrapping_add(3))
                .unwrap_or(0.0);
        }
        let workgroup_rank = workgroup.invocation_rank() as usize;
        let tiles = workgroup.allocate_lds::<[f32; 4], 256>();
        let tiles = tiles.initialize_by_invocation(&workgroup, [tile0, tile1, tile2, tile3]);
        let (workgroup, tiles) = workgroup.publish_lds(tiles);
        let source = element / 4;
        let values = tiles
            .read(
                &workgroup,
                workgroup_subgroup_slot::<64>(workgroup_rank, source),
            )
            .unwrap_or([0.0; 4]);
        if element & 3 == 0 {
            values[0]
        } else if element & 3 == 1 {
            values[1]
        } else if element & 3 == 2 {
            values[2]
        } else {
            values[3]
        }
    });
    #[cfg(feature = "ablation-stage-tile4")]
    if element >= MUON_ELEMENTS {
        return;
    }
    // The compact batch/element index is unique for every active invocation.
    let output_index = batch.wrapping_mul(MUON_ELEMENTS).wrapping_add(element);
    global_store_or_trap(&mut output, output_index, value);
}

/// Reduces two shards and computes five Newton-Schulz Muon iterations.
#[cfg(any(not(target_arch = "amdgpu"), feature = "kernel-muon-update"))]
#[cfg_attr(
    not(feature = "ablation-muon-broadcast16"),
    kernel(
        typed,
        launch(required = [256, 1, 1], max = [256, 1, 1], max_grid = [4, 1, 1])
    )
)]
#[cfg_attr(
    feature = "ablation-muon-broadcast16",
    kernel(
        typed,
        launch(required = [256, 1, 1], max = [256, 1, 1], max_grid = [4, 1, 1])
    )
)]
pub fn gfx950_muon_update_4x4_v1(
    mut context: KernelContext<'_>,
    shards: Global<'_, f32, ReadOnly>,
    mut output: Global<'_, f32, ExclusiveReadWrite>,
    mut output_norm: Global<'_, f32, ExclusiveReadWrite>,
) {
    // One Wave64 owns one 4x4 update; the first 16 lanes hold the matrix elements.
    let global_index = context.invocation().index_1d().get();
    let batch = global_index / 64;
    let lane = global_index & 63;
    // Validate every launch-wide shape before the norm reduction.
    if batch >= SYSTEM_BATCHES
        || shards.len() != SYSTEM_BATCHES * GRADIENT_SHARDS * MUON_ELEMENTS
        || output.len() != SYSTEM_BATCHES * MUON_ELEMENTS
        || output_norm.len() != SYSTEM_BATCHES
    {
        return;
    }
    let shard_base = batch
        .wrapping_mul(GRADIENT_SHARDS)
        .wrapping_mul(MUON_ELEMENTS);
    let matrix_element = lane & (MUON_ELEMENTS - 1);
    let active = (lane < MUON_ELEMENTS) as u32 as f32;
    let matrix_value = active
        * (global_load_2d_or(&shards, shard_base, 0, matrix_element, MUON_ELEMENTS, 0.0)
            + global_load_2d_or(&shards, shard_base, 1, matrix_element, MUON_ELEMENTS, 0.0));
    let policy = context.numerical_policy::<StrictIeee>();
    let math_capability = context.math();
    let math = math_capability.with_numerical_policy(&policy);
    let (matrix_value, norm) = context.with_workgroup(|workgroup| {
        let workgroup_rank = workgroup.invocation_rank() as usize;
        #[cfg(feature = "ablation-muon-broadcast16")]
        let wave_base = workgroup_rank & !63;
        let local_square = matrix_value * matrix_value;

        // Reduce in FP32 under the exact workgroup epoch.
        #[cfg(not(feature = "ablation-muon-broadcast16"))]
        let (workgroup, squared_norm) = {
            let subgroup = workgroup.subgroup::<SubgroupWidth64>();
            let squared_norm = subgroup.reduce_sum(workgroup.epoch(), local_square);
            (workgroup, squared_norm)
        };
        #[cfg(feature = "ablation-muon-broadcast16")]
        let (workgroup, squared_norm) = {
            let squares = workgroup.allocate_lds::<f32, 256>();
            let squares = squares.initialize_by_invocation(&workgroup, local_square);
            let (workgroup, squares) = workgroup.publish_lds(squares);
            let mut sum = squares.read(&workgroup, wave_base).unwrap_or(0.0);
            sum += squares.read(&workgroup, wave_base + 1).unwrap_or(0.0);
            sum += squares.read(&workgroup, wave_base + 2).unwrap_or(0.0);
            sum += squares.read(&workgroup, wave_base + 3).unwrap_or(0.0);
            sum += squares.read(&workgroup, wave_base + 4).unwrap_or(0.0);
            sum += squares.read(&workgroup, wave_base + 5).unwrap_or(0.0);
            sum += squares.read(&workgroup, wave_base + 6).unwrap_or(0.0);
            sum += squares.read(&workgroup, wave_base + 7).unwrap_or(0.0);
            sum += squares.read(&workgroup, wave_base + 8).unwrap_or(0.0);
            sum += squares.read(&workgroup, wave_base + 9).unwrap_or(0.0);
            sum += squares.read(&workgroup, wave_base + 10).unwrap_or(0.0);
            sum += squares.read(&workgroup, wave_base + 11).unwrap_or(0.0);
            sum += squares.read(&workgroup, wave_base + 12).unwrap_or(0.0);
            sum += squares.read(&workgroup, wave_base + 13).unwrap_or(0.0);
            sum += squares.read(&workgroup, wave_base + 14).unwrap_or(0.0);
            sum += squares.read(&workgroup, wave_base + 15).unwrap_or(0.0);
            (workgroup, sum)
        };

        let norm = math.sqrt_f32(squared_norm);
        let mut matrix_value = matrix_value / (norm + 1.0e-6);
        let row = matrix_element / 4;
        let column = matrix_element.wrapping_sub(row.wrapping_mul(4));
        let row_base = row.wrapping_mul(4);
        let column_base = column.wrapping_mul(4);
        let mut exchange = workgroup.allocate_lds::<[f32; 2], 256>().into_reusable();
        let mut phases = workgroup.into_reusable();

        // Each iteration publishes X, computes X X^T, publishes both, then forms X X^T X.
        macro_rules! muon_iteration {
            () => {{
                let gram = phases.with_phase(|phase| {
                    let values = phase.bind_reusable_lds(&mut exchange);
                    let values = values.initialize_by_invocation(&phase, [matrix_value, 0.0]);
                    let (phase, values) = phase.publish_lds(values);
                    macro_rules! read_matrix {
                        ($source:expr) => {
                            values
                                .read(
                                    &phase,
                                    workgroup_subgroup_slot::<64>(workgroup_rank, $source),
                                )
                                .unwrap_or([0.0; 2])[0]
                        };
                    }
                    let gram = read_matrix!(row_base) * read_matrix!(column_base)
                        + read_matrix!(row_base + 1) * read_matrix!(column_base + 1)
                        + read_matrix!(row_base + 2) * read_matrix!(column_base + 2)
                        + read_matrix!(row_base + 3) * read_matrix!(column_base + 3);
                    let completion = phase.finish_reusable_phase();
                    (completion, gram)
                });
                matrix_value = phases.with_phase(|phase| {
                    let values = phase.bind_reusable_lds(&mut exchange);
                    let values = values.initialize_by_invocation(&phase, [matrix_value, gram]);
                    let (phase, values) = phase.publish_lds(values);
                    macro_rules! read_values {
                        ($source:expr) => {
                            values
                                .read(
                                    &phase,
                                    workgroup_subgroup_slot::<64>(workgroup_rank, $source),
                                )
                                .unwrap_or([0.0; 2])
                        };
                    }
                    let cubic = read_values!(row_base)[1] * read_values!(column)[0]
                        + read_values!(row_base + 1)[1] * read_values!(column + 4)[0]
                        + read_values!(row_base + 2)[1] * read_values!(column + 8)[0]
                        + read_values!(row_base + 3)[1] * read_values!(column + 12)[0];
                    let completion = phase.finish_reusable_phase();
                    (completion, 1.5 * matrix_value - 0.5 * cubic)
                });
            }};
        }
        muon_iteration!();
        muon_iteration!();
        muon_iteration!();
        muon_iteration!();
        muon_iteration!();
        (matrix_value, norm)
    });
    // Matrix lanes own disjoint compact outputs; lane zero separately owns the norm.
    if lane < MUON_ELEMENTS {
        let output_index = batch.wrapping_mul(MUON_ELEMENTS).wrapping_add(lane);
        global_store_or_trap(
            &mut output,
            output_index,
            -MUON_LEARNING_RATE * matrix_value,
        );
    }
    if lane == 0 {
        global_store_or_trap(&mut output_norm, batch, norm);
    }
}
