#![allow(dead_code)] // Ablation-only helpers are selected by mutually exclusive features.

use fe2o3_device::{
    Blocked, CapabilityMemoryElementV1, DisjointWrite, Global, Index1D, KernelCapabilityBrand,
    KernelContext, KernelError, KernelLaunch, KernelResult, KernelTarget, ReadOnly, StrictIeee,
    SubgroupWidth64,
};

use crate::{
    CONTEXT_TOKENS, EXPERT_K_TILE, EXPERT_N_TILE, EXPERTS, HEAD_DIM, MATRIX_ROWS, MXFP4_BLOCKS,
    VALUE_TILE,
};

type RootBrand<'kernel, Kernel, Target, Launch> =
    KernelCapabilityBrand<'kernel, Kernel, Target, Launch>;

pub(crate) type OutputTile = Blocked<Index1D, 16, 4>;

pub(crate) fn store_output_tile<'kernel, Kernel, Target, Launch>(
    context: &KernelContext<'kernel, Kernel, Target, Launch>,
    output: &mut Global<
        'kernel,
        f32,
        DisjointWrite<OutputTile>,
        RootBrand<'kernel, Kernel, Target, Launch>,
    >,
    values: [f32; 4],
) -> KernelResult
where
    Target: KernelTarget,
    Launch: KernelLaunch,
{
    let Some(block) = context.invocation().index_1d().checked_block::<16, 4>() else {
        return Err(KernelError::OutOfBounds);
    };
    if !output.store_block(&block, 0, values[0])
        || !output.store_block(&block, 1, values[1])
        || !output.store_block(&block, 2, values[2])
        || !output.store_block(&block, 3, values[3])
    {
        return Err(KernelError::OutOfBounds);
    }
    Ok(())
}

pub(crate) fn store_packed_route<'kernel, Kernel, Target, Launch>(
    context: &KernelContext<'kernel, Kernel, Target, Launch>,
    output: &mut Global<
        'kernel,
        u32,
        DisjointWrite<Index1D>,
        RootBrand<'kernel, Kernel, Target, Launch>,
    >,
    value: u32,
) -> KernelResult
where
    Target: KernelTarget,
    Launch: KernelLaunch,
{
    if !output.store(context.invocation().index_1d().into_disjoint(), value) {
        return Err(KernelError::OutOfBounds);
    }
    Ok(())
}

pub(crate) struct GlobalReadView2D<'view, 'kernel, T, Brand>
where
    T: CapabilityMemoryElementV1,
{
    source: &'view Global<'kernel, T, ReadOnly, Brand>,
    offset: usize,
    rows: usize,
    columns: usize,
    stride: usize,
}

fn insert_ranked(scores: &mut [f32; 4], ids: &mut [u32; 4], mut score: f32, mut id: u32) {
    let mut position = 0;
    while position < scores.len() {
        let take = ((score > scores[position])
            | ((score == scores[position]) & (id < ids[position]))) as u32;
        let choose = take as f32;
        let keep = 1.0 - choose;
        let old_score = scores[position];
        let old_id = ids[position];
        scores[position] = score * choose + old_score * keep;
        ids[position] = id
            .wrapping_mul(take)
            .wrapping_add(old_id.wrapping_mul(take ^ 1));
        score = old_score * choose + score * keep;
        id = old_id
            .wrapping_mul(take)
            .wrapping_add(id.wrapping_mul(take ^ 1));
        position += 1;
    }
}

pub(crate) fn compute_parallel_router_logits<'kernel, Brand>(
    hidden_f32: &Global<'kernel, f32, ReadOnly, Brand>,
    router_f32: &Global<'kernel, f32, ReadOnly, Brand>,
    item: usize,
    lane: usize,
) -> Result<[f32; 2], KernelError> {
    let hidden = GlobalReadView2D::checked(
        hidden_f32,
        item.wrapping_mul(crate::HIDDEN_SIZE),
        1,
        crate::HIDDEN_SIZE,
        crate::HIDDEN_SIZE,
    )
    .ok_or(KernelError::InvalidArgument)?;
    let router = GlobalReadView2D::checked(
        router_f32,
        0,
        EXPERTS,
        crate::HIDDEN_SIZE,
        crate::HIDDEN_SIZE,
    )
    .ok_or(KernelError::InvalidArgument)?;
    let expert0 = lane.wrapping_mul(2);
    let expert1 = expert0.wrapping_add(1);
    let mut logits = [0.0_f32; 2];
    let mut depth = 0;
    while depth < crate::HIDDEN_SIZE {
        let activation = hidden.load_or(0, depth, 0.0);
        logits[0] += activation * router.load_or(expert0, depth, 0.0);
        logits[1] += activation * router.load_or(expert1, depth, 0.0);
        depth += 1;
    }
    Ok(logits)
}

pub(crate) fn select_serial_top4<'kernel, Brand>(
    hidden_f32: &Global<'kernel, f32, ReadOnly, Brand>,
    router_f32: &Global<'kernel, f32, ReadOnly, Brand>,
    item: usize,
) -> Result<[u32; 4], KernelError> {
    let hidden = GlobalReadView2D::checked(
        hidden_f32,
        item.wrapping_mul(crate::HIDDEN_SIZE),
        1,
        crate::HIDDEN_SIZE,
        crate::HIDDEN_SIZE,
    )
    .ok_or(KernelError::InvalidArgument)?;
    let router = GlobalReadView2D::checked(
        router_f32,
        0,
        EXPERTS,
        crate::HIDDEN_SIZE,
        crate::HIDDEN_SIZE,
    )
    .ok_or(KernelError::InvalidArgument)?;
    let mut scores = [-1.0e30_f32; 4];
    let mut ids = [u32::MAX; 4];
    let mut expert = 0;
    while expert < EXPERTS {
        let mut score = 0.0;
        let mut depth = 0;
        while depth < crate::HIDDEN_SIZE {
            score += hidden.load_or(0, depth, 0.0) * router.load_or(expert, depth, 0.0);
            depth += 1;
        }
        insert_ranked(&mut scores, &mut ids, score, expert as u32);
        expert += 1;
    }
    Ok(ids)
}

pub(crate) fn select_wave_top4<'kernel, Kernel, Target, Launch>(
    context: &mut KernelContext<'kernel, Kernel, Target, Launch>,
    local_logit0: f32,
    local_logit1: f32,
) -> [u32; 4]
where
    Target: KernelTarget,
    Launch: KernelLaunch,
{
    context.with_workgroup(|workgroup| {
        let rank = workgroup.invocation_rank() as usize;
        let logits = workgroup.allocate_lds::<[f32; 2], 256>();
        let logits = logits.initialize_by_invocation(&workgroup, [local_logit0, local_logit1]);
        let (workgroup, logits) = workgroup.publish_lds(logits);
        let wave_base = rank & !63;
        let mut scores = [-1.0e30_f32; 4];
        let mut ids = [u32::MAX; 4];
        let mut source = 0_usize;
        while source < 64 {
            let pair = logits
                .read(&workgroup, wave_base.wrapping_add(source))
                .unwrap_or([-1.0e30; 2]);
            insert_ranked(&mut scores, &mut ids, pair[0], (source * 2) as u32);
            insert_ranked(&mut scores, &mut ids, pair[1], (source * 2 + 1) as u32);
            source += 1;
        }
        ids
    })
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn compute_attention<'kernel, Kernel, Target, Launch>(
    context: &mut KernelContext<'kernel, Kernel, Target, Launch>,
    query_bits: &Global<'kernel, u16, ReadOnly, RootBrand<'kernel, Kernel, Target, Launch>>,
    key_bits: &Global<'kernel, u16, ReadOnly, RootBrand<'kernel, Kernel, Target, Launch>>,
    values_bits: &Global<'kernel, f32, ReadOnly, RootBrand<'kernel, Kernel, Target, Launch>>,
    sinks_bits: &Global<'kernel, f32, ReadOnly, RootBrand<'kernel, Kernel, Target, Launch>>,
    item: usize,
    lane_index: usize,
) -> Result<[f32; 4], KernelError>
where
    Target: KernelTarget,
    Launch: KernelLaunch,
{
    let values = GlobalReadView2D::checked(
        values_bits,
        item.wrapping_mul(CONTEXT_TOKENS * VALUE_TILE),
        CONTEXT_TOKENS,
        VALUE_TILE,
        VALUE_TILE,
    )
    .ok_or(KernelError::InvalidArgument)?;
    let sinks = GlobalReadView2D::checked(
        sinks_bits,
        item.wrapping_mul(MATRIX_ROWS),
        1,
        MATRIX_ROWS,
        MATRIX_ROWS,
    )
    .ok_or(KernelError::InvalidArgument)?;
    let row0 = (lane_index / CONTEXT_TOKENS).wrapping_mul(4);
    let sink0 = sinks.load_or(0, row0, 0.0);
    let sink1 = sinks.load_or(0, row0.wrapping_add(1), 0.0);
    let sink2 = sinks.load_or(0, row0.wrapping_add(2), 0.0);
    let sink3 = sinks.load_or(0, row0.wrapping_add(3), 0.0);
    let column = lane_index % VALUE_TILE;
    let policy = context.numerical_policy::<StrictIeee>();
    let device_math = context.math();
    let math = device_math.with_numerical_policy(&policy);
    context.with_workgroup(|workgroup| {
        let subgroup = workgroup.subgroup::<SubgroupWidth64>();
        let scores = subgroup.with_matrix(workgroup.epoch(), |matrix, lane| {
            let matrix = matrix.with_numerical_policy(&policy);
            let query = matrix
                .bf16_a_global_row_major(
                    query_bits,
                    item.wrapping_mul(MATRIX_ROWS * HEAD_DIM),
                    MATRIX_ROWS,
                    HEAD_DIM,
                    HEAD_DIM,
                )
                .map_err(|_| KernelError::InvalidArgument)?;
            let key = matrix
                .bf16_b_global_row_major(
                    key_bits,
                    item.wrapping_mul(HEAD_DIM * CONTEXT_TOKENS),
                    HEAD_DIM,
                    CONTEXT_TOKENS,
                    CONTEXT_TOKENS,
                )
                .map_err(|_| KernelError::InvalidArgument)?;
            let scores = matrix.bf16_zero_accumulator(lane);
            let scores = matrix.multiply_accumulate(
                query.load_m16k16(lane, 0, 0),
                key.load_k16n16(lane, 0, 0),
                scores,
            );
            let scores = matrix.multiply_accumulate(
                query.load_m16k16(lane, 0, 16),
                key.load_k16n16(lane, 16, 0),
                scores,
            );
            let scores = matrix.multiply_accumulate(
                query.load_m16k16(lane, 0, 32),
                key.load_k16n16(lane, 32, 0),
                scores,
            );
            Ok::<_, KernelError>(
                matrix
                    .multiply_accumulate(
                        query.load_m16k16(lane, 0, 48),
                        key.load_k16n16(lane, 48, 0),
                        scores,
                    )
                    .into_values(),
            )
        })?;
        let wave16 = subgroup.gfx950_wave16(workgroup.epoch());
        let scaled = [
            scores[0] * 0.125,
            scores[1] * 0.125,
            scores[2] * 0.125,
            scores[3] * 0.125,
        ];
        let reduced = [
            wave16.reduce_max_f32(scaled[0]),
            wave16.reduce_max_f32(scaled[1]),
            wave16.reduce_max_f32(scaled[2]),
            wave16.reduce_max_f32(scaled[3]),
        ];
        let sinks = [sink0, sink1, sink2, sink3];
        let mut maximum = [0.0; 4];
        let mut probability = [0.0; 4];
        let mut row = 0;
        while row < 4 {
            let choose = (reduced[row] > sinks[row]) as u32 as f32;
            maximum[row] = reduced[row] * choose + sinks[row] * (1.0 - choose);
            probability[row] = math.exp_f32(scaled[row] - maximum[row]);
            let denominator =
                wave16.reduce_sum_f32(probability[row]) + math.exp_f32(sinks[row] - maximum[row]);
            probability[row] /= denominator;
            row += 1;
        }
        let mut attention = [0.0; 4];
        let mut token = 0;
        while token < CONTEXT_TOKENS {
            let value = values.load_or(token, column, 0.0);
            let mut row = 0;
            while row < 4 {
                attention[row] += wave16.broadcast_f32(probability[row], token as u32) * value;
                row += 1;
            }
            token += 1;
        }
        Ok(attention)
    })
}

fn widen_bf16(bits: u16) -> f32 {
    f32::from_bits((bits as u32) << 16)
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn compute_scalar_attention<'kernel, Kernel, Target, Launch>(
    context: &mut KernelContext<'kernel, Kernel, Target, Launch>,
    query_bits: &Global<'kernel, u16, ReadOnly, RootBrand<'kernel, Kernel, Target, Launch>>,
    key_bits: &Global<'kernel, u16, ReadOnly, RootBrand<'kernel, Kernel, Target, Launch>>,
    values_bits: &Global<'kernel, f32, ReadOnly, RootBrand<'kernel, Kernel, Target, Launch>>,
    sinks_bits: &Global<'kernel, f32, ReadOnly, RootBrand<'kernel, Kernel, Target, Launch>>,
    item: usize,
    lane_index: usize,
) -> Result<[f32; 4], KernelError>
where
    Target: KernelTarget,
    Launch: KernelLaunch,
{
    let query = GlobalReadView2D::checked(
        query_bits,
        item * MATRIX_ROWS * HEAD_DIM,
        MATRIX_ROWS,
        HEAD_DIM,
        HEAD_DIM,
    )
    .ok_or(KernelError::InvalidArgument)?;
    let key = GlobalReadView2D::checked(
        key_bits,
        item * HEAD_DIM * CONTEXT_TOKENS,
        HEAD_DIM,
        CONTEXT_TOKENS,
        CONTEXT_TOKENS,
    )
    .ok_or(KernelError::InvalidArgument)?;
    let values = GlobalReadView2D::checked(
        values_bits,
        item * CONTEXT_TOKENS * VALUE_TILE,
        CONTEXT_TOKENS,
        VALUE_TILE,
        VALUE_TILE,
    )
    .ok_or(KernelError::InvalidArgument)?;
    let sinks =
        GlobalReadView2D::checked(sinks_bits, item * MATRIX_ROWS, 1, MATRIX_ROWS, MATRIX_ROWS)
            .ok_or(KernelError::InvalidArgument)?;
    let token_index = lane_index % CONTEXT_TOKENS;
    let row0 = (lane_index / CONTEXT_TOKENS) * 4;
    let mut scores = [0.0; 4];
    let mut depth = 0;
    while depth < HEAD_DIM {
        let key_value = widen_bf16(key.load_or(depth, token_index, 0));
        let mut row = 0;
        while row < 4 {
            scores[row] += widen_bf16(query.load_or(row0 + row, depth, 0)) * key_value;
            row += 1;
        }
        depth += 1;
    }
    let sink = [
        sinks.load_or(0, row0, 0.0),
        sinks.load_or(0, row0 + 1, 0.0),
        sinks.load_or(0, row0 + 2, 0.0),
        sinks.load_or(0, row0 + 3, 0.0),
    ];
    let column = lane_index % VALUE_TILE;
    let policy = context.numerical_policy::<StrictIeee>();
    let device_math = context.math();
    let math = device_math.with_numerical_policy(&policy);
    let attention = context.with_workgroup(|workgroup| {
        let subgroup = workgroup.subgroup::<SubgroupWidth64>();
        let wave16 = subgroup.gfx950_wave16(workgroup.epoch());
        let mut probability = [0.0; 4];
        let mut row = 0;
        while row < 4 {
            let scaled = scores[row] * 0.125;
            let reduced = wave16.reduce_max_f32(scaled);
            let choose = (reduced > sink[row]) as u32 as f32;
            let maximum = reduced * choose + sink[row] * (1.0 - choose);
            probability[row] = math.exp_f32(scaled - maximum);
            probability[row] /=
                wave16.reduce_sum_f32(probability[row]) + math.exp_f32(sink[row] - maximum);
            row += 1;
        }
        let mut attention = [0.0; 4];
        let mut token = 0;
        while token < CONTEXT_TOKENS {
            let value = values.load_or(token, column, 0.0);
            let mut row = 0;
            while row < 4 {
                attention[row] += wave16.broadcast_f32(probability[row], token as u32) * value;
                row += 1;
            }
            token += 1;
        }
        attention
    });
    Ok(attention)
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn compute_expert_streamed<'kernel, Kernel, Target, Launch>(
    context: &mut KernelContext<'kernel, Kernel, Target, Launch>,
    activations: &Global<'kernel, u8, ReadOnly, RootBrand<'kernel, Kernel, Target, Launch>>,
    weights: &Global<'kernel, u8, ReadOnly, RootBrand<'kernel, Kernel, Target, Launch>>,
    activation_scales: &Global<'kernel, f32, ReadOnly, RootBrand<'kernel, Kernel, Target, Launch>>,
    weight_scales: &Global<'kernel, f32, ReadOnly, RootBrand<'kernel, Kernel, Target, Launch>>,
    item: usize,
    lane_index: usize,
    selected: usize,
) -> Result<[f32; 4], KernelError>
where
    Target: KernelTarget,
    Launch: KernelLaunch,
{
    let activation_scale = GlobalReadView2D::checked(
        activation_scales,
        item.wrapping_mul(MXFP4_BLOCKS),
        1,
        MXFP4_BLOCKS,
        MXFP4_BLOCKS,
    )
    .ok_or(KernelError::InvalidArgument)?;
    let weight_scale = GlobalReadView2D::checked(
        weight_scales,
        0,
        EXPERTS * MXFP4_BLOCKS,
        EXPERT_N_TILE,
        EXPERT_N_TILE,
    )
    .ok_or(KernelError::InvalidArgument)?;
    let column = lane_index % EXPERT_N_TILE;
    let scale = [
        activation_scale.load_or(0, 0, 0.0)
            * weight_scale.load_or(selected * MXFP4_BLOCKS, column, 0.0),
        activation_scale.load_or(0, 1, 0.0)
            * weight_scale.load_or(selected * MXFP4_BLOCKS + 1, column, 0.0),
        activation_scale.load_or(0, 2, 0.0)
            * weight_scale.load_or(selected * MXFP4_BLOCKS + 2, column, 0.0),
        activation_scale.load_or(0, 3, 0.0)
            * weight_scale.load_or(selected * MXFP4_BLOCKS + 3, column, 0.0),
    ];
    let policy = context.numerical_policy::<StrictIeee>();
    context.with_workgroup(|workgroup| {
        let subgroup = workgroup.subgroup::<SubgroupWidth64>();
        subgroup.with_matrix(workgroup.epoch(), |matrix, lane| {
            let policy_matrix = matrix.with_numerical_policy(&policy);
            let matrix = policy_matrix.gfx950();
            let activations = matrix
                .fp4_a_global_row_major(
                    activations,
                    item.wrapping_mul(MXFP4_BLOCKS * MATRIX_ROWS * EXPERT_K_TILE),
                    MXFP4_BLOCKS * MATRIX_ROWS,
                    EXPERT_K_TILE,
                    EXPERT_K_TILE,
                )
                .map_err(|_| KernelError::InvalidArgument)?;
            let weights = matrix
                .fp4_b_global_row_major(
                    weights,
                    0,
                    EXPERTS * MXFP4_BLOCKS * EXPERT_K_TILE,
                    EXPERT_N_TILE,
                    EXPERT_N_TILE,
                )
                .map_err(|_| KernelError::InvalidArgument)?;
            let reduction_base = selected * MXFP4_BLOCKS * EXPERT_K_TILE;
            let mut result = [0.0; 4];
            let mut block = 0;
            while block < MXFP4_BLOCKS {
                let values = matrix
                    .multiply_accumulate_fp4(
                        activations.load_m16k128(lane, block * MATRIX_ROWS, 0),
                        weights.load_k128n16(lane, reduction_base + block * EXPERT_K_TILE, 0),
                        matrix.fp4_zero_accumulator(lane),
                    )
                    .into_values();
                let mut component = 0;
                while component < 4 {
                    result[component] += values[component] * scale[block];
                    component += 1;
                }
                block += 1;
            }
            Ok(result)
        })
    })
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn compute_expert_held<'kernel, Kernel, Target, Launch>(
    context: &mut KernelContext<'kernel, Kernel, Target, Launch>,
    activations: &Global<'kernel, u8, ReadOnly, RootBrand<'kernel, Kernel, Target, Launch>>,
    weights: &Global<'kernel, u8, ReadOnly, RootBrand<'kernel, Kernel, Target, Launch>>,
    activation_scales: &Global<'kernel, f32, ReadOnly, RootBrand<'kernel, Kernel, Target, Launch>>,
    weight_scales: &Global<'kernel, f32, ReadOnly, RootBrand<'kernel, Kernel, Target, Launch>>,
    item: usize,
    lane_index: usize,
    selected: usize,
) -> Result<[f32; 4], KernelError>
where
    Target: KernelTarget,
    Launch: KernelLaunch,
{
    let activation_scale = GlobalReadView2D::checked(
        activation_scales,
        item * MXFP4_BLOCKS,
        1,
        MXFP4_BLOCKS,
        MXFP4_BLOCKS,
    )
    .ok_or(KernelError::InvalidArgument)?;
    let weight_scale = GlobalReadView2D::checked(
        weight_scales,
        0,
        EXPERTS * MXFP4_BLOCKS,
        EXPERT_N_TILE,
        EXPERT_N_TILE,
    )
    .ok_or(KernelError::InvalidArgument)?;
    let column = lane_index % EXPERT_N_TILE;
    let scales = [
        activation_scale.load_or(0, 0, 0.0)
            * weight_scale.load_or(selected * MXFP4_BLOCKS, column, 0.0),
        activation_scale.load_or(0, 1, 0.0)
            * weight_scale.load_or(selected * MXFP4_BLOCKS + 1, column, 0.0),
        activation_scale.load_or(0, 2, 0.0)
            * weight_scale.load_or(selected * MXFP4_BLOCKS + 2, column, 0.0),
        activation_scale.load_or(0, 3, 0.0)
            * weight_scale.load_or(selected * MXFP4_BLOCKS + 3, column, 0.0),
    ];
    let policy = context.numerical_policy::<StrictIeee>();
    context.with_workgroup(|workgroup| {
        let subgroup = workgroup.subgroup::<SubgroupWidth64>();
        subgroup.with_matrix(workgroup.epoch(), |matrix, lane| {
            let policy_matrix = matrix.with_numerical_policy(&policy);
            let matrix = policy_matrix.gfx950();
            let activations = matrix
                .fp4_a_global_row_major(
                    activations,
                    item * MXFP4_BLOCKS * MATRIX_ROWS * EXPERT_K_TILE,
                    MXFP4_BLOCKS * MATRIX_ROWS,
                    EXPERT_K_TILE,
                    EXPERT_K_TILE,
                )
                .map_err(|_| KernelError::InvalidArgument)?;
            let weights = matrix
                .fp4_b_global_row_major(
                    weights,
                    0,
                    EXPERTS * MXFP4_BLOCKS * EXPERT_K_TILE,
                    EXPERT_N_TILE,
                    EXPERT_N_TILE,
                )
                .map_err(|_| KernelError::InvalidArgument)?;
            let reduction = selected * MXFP4_BLOCKS * EXPERT_K_TILE;
            let activation0 = activations.load_m16k128(lane, 0, 0);
            let activation1 = activations.load_m16k128(lane, MATRIX_ROWS, 0);
            let activation2 = activations.load_m16k128(lane, 2 * MATRIX_ROWS, 0);
            let activation3 = activations.load_m16k128(lane, 3 * MATRIX_ROWS, 0);
            let weight0 = weights.load_k128n16(lane, reduction, 0);
            let weight1 = weights.load_k128n16(lane, reduction + EXPERT_K_TILE, 0);
            let weight2 = weights.load_k128n16(lane, reduction + 2 * EXPERT_K_TILE, 0);
            let weight3 = weights.load_k128n16(lane, reduction + 3 * EXPERT_K_TILE, 0);
            let expert0 = matrix
                .multiply_accumulate_fp4(activation0, weight0, matrix.fp4_zero_accumulator(lane))
                .into_values();
            let expert1 = matrix
                .multiply_accumulate_fp4(activation1, weight1, matrix.fp4_zero_accumulator(lane))
                .into_values();
            let expert2 = matrix
                .multiply_accumulate_fp4(activation2, weight2, matrix.fp4_zero_accumulator(lane))
                .into_values();
            let expert3 = matrix
                .multiply_accumulate_fp4(activation3, weight3, matrix.fp4_zero_accumulator(lane))
                .into_values();
            let mut result = [0.0; 4];
            let mut component = 0;
            while component < 4 {
                result[component] = expert0[component] * scales[0]
                    + expert1[component] * scales[1]
                    + expert2[component] * scales[2]
                    + expert3[component] * scales[3];
                component += 1;
            }
            Ok(result)
        })
    })
}

impl<'view, 'kernel, T, Brand> GlobalReadView2D<'view, 'kernel, T, Brand>
where
    T: CapabilityMemoryElementV1,
{
    pub(crate) fn checked(
        source: &'view Global<'kernel, T, ReadOnly, Brand>,
        offset: usize,
        rows: usize,
        columns: usize,
        stride: usize,
    ) -> Option<Self> {
        if rows != 0 && columns > stride {
            return None;
        }
        let required = if rows == 0 || columns == 0 {
            offset
        } else {
            offset
                .checked_add((rows - 1).checked_mul(stride)?)?
                .checked_add(columns)?
        };
        (required <= source.len()).then_some(Self {
            source,
            offset,
            rows,
            columns,
            stride,
        })
    }

    pub(crate) fn load_or(&self, row: usize, column: usize, fallback: T) -> T {
        if row >= self.rows || column >= self.columns {
            return fallback;
        }
        row.checked_mul(self.stride)
            .and_then(|row_offset| self.offset.checked_add(row_offset))
            .and_then(|index| index.checked_add(column))
            .and_then(|index| self.source.load(index))
            .unwrap_or(fallback)
    }
}
