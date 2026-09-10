//! Ordinary attributed Rust for the bounded advanced-attention profiles.
//!
//! Device entrypoints follow a common review order: validate whole-launch
//! shapes before collectives, derive batch and lane ownership, create checked
//! multidimensional views, run uniform subgroup or matrix operations, then
//! write only through typed output capabilities. Independent CPU references
//! live in `reference.rs` and are not reachable from an attributed kernel.

#![allow(missing_docs)] // The kernel macro emits an undocumented helper module.

#[cfg(target_arch = "amdgpu")]
use fe2o3_device::{
    DisjointWrite, ExclusiveReadWrite, Gfx950Subgroup, Global, Index1D, KernelContext, KernelError,
    KernelResult, ReadOnly, StrictIeee, Subgroup, SubgroupWidth64, SynchronizationEpoch,
    WorkgroupEpoch, kernel,
};

#[cfg(target_arch = "amdgpu")]
use crate::{
    ATTENTION_TOKENS_V1, CHANNELS_V1, HEAD_DIMENSION_V1, KDA_KEY_DIMENSION_V1,
    KDA_STATE_ELEMENTS_V1, KDA_VALUE_DIMENSION_V1, MIXING_STREAMS_V1, PREFILL_TOKENS_V1,
    SELECTED_BLOCKS_V1, SELECTED_TOKENS_V1, SINKHORN_ITERATIONS_V1, SPARSE_BLOCKS_V1,
    TOKENS_PER_BLOCK_V1, batch_count_for_launch_v1,
};

#[cfg(target_arch = "amdgpu")]
const ATTENTION_SCALE_V1: f32 = 0.088_388_346;

#[cfg(target_arch = "amdgpu")]
#[inline(always)]
fn global_load_2d_f32_v1<Brand>(
    values: &Global<'_, f32, ReadOnly, Brand>,
    base: usize,
    row: usize,
    column: usize,
    stride: usize,
    fallback: f32,
) -> f32 {
    let Some(index) = row
        .checked_mul(stride)
        .and_then(|offset| base.checked_add(offset))
        .and_then(|offset| offset.checked_add(column))
    else {
        return fallback;
    };
    values.load(index).unwrap_or(fallback)
}

#[cfg(target_arch = "amdgpu")]
#[inline(always)]
fn global_load_2d_u8_v1<Brand>(
    values: &Global<'_, u8, ReadOnly, Brand>,
    base: usize,
    row: usize,
    column: usize,
    stride: usize,
) -> u8 {
    row.checked_mul(stride)
        .and_then(|offset| base.checked_add(offset))
        .and_then(|offset| offset.checked_add(column))
        .and_then(|index| values.load(index))
        .unwrap_or(0)
}

#[cfg(target_arch = "amdgpu")]
pub(crate) struct GlobalReadView2DF32V1<'view, 'memory, Brand> {
    values: &'view Global<'memory, f32, ReadOnly, Brand>,
    base: usize,
    rows: usize,
    columns: usize,
    stride: usize,
}

#[cfg(any(target_arch = "amdgpu", test))]
const fn checked_2d_extent_v1(
    base: usize,
    rows: usize,
    columns: usize,
    stride: usize,
    physical_len: usize,
) -> bool {
    if rows == 0 || columns > stride {
        return false;
    }
    let Some(last_row) = rows.checked_sub(1) else {
        return false;
    };
    let Some(last_row_offset) = last_row.checked_mul(stride) else {
        return false;
    };
    let Some(extent) = last_row_offset.checked_add(columns) else {
        return false;
    };
    let Some(end) = base.checked_add(extent) else {
        return false;
    };
    end <= physical_len
}

#[cfg(target_arch = "amdgpu")]
impl<'view, 'memory, Brand> GlobalReadView2DF32V1<'view, 'memory, Brand> {
    #[inline(always)]
    pub(crate) fn checked(
        values: &'view Global<'memory, f32, ReadOnly, Brand>,
        base: usize,
        rows: usize,
        columns: usize,
        stride: usize,
    ) -> Option<Self> {
        if !checked_2d_extent_v1(base, rows, columns, stride, values.len()) {
            return None;
        }
        Some(Self {
            values,
            base,
            rows,
            columns,
            stride,
        })
    }

    #[inline(always)]
    pub(crate) fn load_or(&self, row: usize, column: usize, fallback: f32) -> f32 {
        if row >= self.rows || column >= self.columns {
            return fallback;
        }
        global_load_2d_f32_v1(self.values, self.base, row, column, self.stride, fallback)
    }
}

// Keep decoding as an expression macro because the admitted device subset does
// not lower this helper as an ordinary call on every production path.
#[cfg(any(target_arch = "amdgpu", test))]
macro_rules! decode_fp8_e4m3_v1 {
    ($value:expr) => {{
        let bits = $value;
        let exponent = (bits >> 3_u8) & 0xf;
        let mantissa = bits & 0x7;
        let magnitude = if exponent == 0xf && mantissa == 0x7 {
            let nan_source = f32::from(mantissa ^ 7_u8);
            nan_source / nan_source
        } else if exponent == 0 {
            f32::from(mantissa) / 512.0
        } else {
            let exponent0 = f32::from(exponent & 0x1);
            let exponent1 = f32::from((exponent >> 1_u8) & 0x1);
            let exponent2 = f32::from((exponent >> 2_u8) & 0x1);
            let exponent3 = f32::from((exponent >> 3_u8) & 0x1);
            let scale = (1.0 + exponent0)
                * (1.0 + 3.0 * exponent1)
                * (1.0 + 15.0 * exponent2)
                * (1.0 + 255.0 * exponent3)
                / 128.0;
            (1.0 + f32::from(mantissa) / 8.0) * scale
        };
        if bits & 0x80 == 0 {
            magnitude
        } else {
            -magnitude
        }
    }};
}

#[cfg(target_arch = "amdgpu")]
#[inline(always)]
pub(crate) fn partition16_reduce_sum_v1<'operation, 'workgroup, KernelBrand, Epoch>(
    subgroup: &Gfx950Subgroup<'operation, 'workgroup, KernelBrand, Epoch>,
    value: f32,
) -> f32
where
    Epoch: SynchronizationEpoch,
{
    subgroup.reduce_sum_f32(value)
}

#[cfg(target_arch = "amdgpu")]
#[inline(always)]
fn partition16_broadcast_v1<'operation, 'workgroup, KernelBrand, Epoch>(
    subgroup: &Gfx950Subgroup<'operation, 'workgroup, KernelBrand, Epoch>,
    value: f32,
    source_lane: u32,
) -> f32
where
    Epoch: SynchronizationEpoch,
{
    subgroup.broadcast_f32(value, source_lane)
}

#[cfg(target_arch = "amdgpu")]
#[inline(always)]
fn partition4_reduce_sum_v1<'operation, 'workgroup, KernelBrand, Epoch>(
    subgroup: &Gfx950Subgroup<'operation, 'workgroup, KernelBrand, Epoch>,
    lane: u32,
    value: f32,
) -> f32
where
    Epoch: SynchronizationEpoch,
{
    let group = (lane & 15) / 4;
    let sum0 = subgroup.reduce_sum_f32(if group == 0 { value } else { 0.0 });
    let sum1 = subgroup.reduce_sum_f32(if group == 1 { value } else { 0.0 });
    let sum2 = subgroup.reduce_sum_f32(if group == 2 { value } else { 0.0 });
    let sum3 = subgroup.reduce_sum_f32(if group == 3 { value } else { 0.0 });
    if group == 0 {
        sum0
    } else if group == 1 {
        sum1
    } else if group == 2 {
        sum2
    } else {
        sum3
    }
}

#[cfg(target_arch = "amdgpu")]
#[inline(always)]
fn fp8_attention_score_v1<'workgroup, KernelBrand, Epoch, QueryBrand, KeyBrand>(
    subgroup: &Subgroup<'workgroup, SubgroupWidth64, KernelBrand, Epoch>,
    epoch: &WorkgroupEpoch<'workgroup, KernelBrand, Epoch>,
    query: &Global<'_, u8, ReadOnly, QueryBrand>,
    key: &Global<'_, u8, ReadOnly, KeyBrand>,
    batch: usize,
    token: usize,
) -> f32
where
    Epoch: SynchronizationEpoch,
{
    let lane = subgroup.lane_rank() as usize;
    let query_base = batch * ATTENTION_TOKENS_V1 * HEAD_DIMENSION_V1;
    let key_base = query_base + token * HEAD_DIMENSION_V1;
    let query0 = query.load(query_base + lane).unwrap_or(0);
    let query1 = query.load(query_base + lane + 64).unwrap_or(0);
    let key0 = key.load(key_base + lane).unwrap_or(0);
    let key1 = key.load(key_base + lane + 64).unwrap_or(0);
    subgroup.reduce_sum(
        epoch,
        decode_fp8_e4m3_v1!(query0) * decode_fp8_e4m3_v1!(key0)
            + decode_fp8_e4m3_v1!(query1) * decode_fp8_e4m3_v1!(key1),
    ) * ATTENTION_SCALE_V1
}

// Maintain the three best sparse candidates with deterministic rank order.
#[cfg(target_arch = "amdgpu")]
macro_rules! consider_sparse_candidate_v1 {
    ($id:expr, $rank:expr, $attention:expr, $id0:ident, $rank0:ident, $attention0:ident,
     $id1:ident, $rank1:ident, $attention1:ident,
     $id2:ident, $rank2:ident, $attention2:ident) => {{
        let candidate_rank = $rank;
        let candidate_attention = $attention;
        if candidate_rank > $rank0 {
            $id2 = $id1;
            $rank2 = $rank1;
            $attention2 = $attention1;
            $id1 = $id0;
            $rank1 = $rank0;
            $attention1 = $attention0;
            $id0 = $id;
            $rank0 = candidate_rank;
            $attention0 = candidate_attention;
        } else if candidate_rank > $rank1 {
            $id2 = $id1;
            $rank2 = $rank1;
            $attention2 = $attention1;
            $id1 = $id;
            $rank1 = candidate_rank;
            $attention1 = candidate_attention;
        } else if candidate_rank > $rank2 {
            $id2 = $id;
            $rank2 = candidate_rank;
            $attention2 = candidate_attention;
        }
    }};
}

#[cfg(target_arch = "amdgpu")]
// One WY chunk advances four tokens while carrying one matrix-state element
// per thread. Every Wave16 reduction must be reached uniformly by all lanes.
macro_rules! kda_chunk_wy_v1 {
    ($base:expr, $query:ident, $key:ident, $value:ident, $alpha:ident, $beta:ident,
     $subgroup:ident, $key_index:ident, $value_column:ident, $state:ident,
     $output0:ident, $output1:ident, $output2:ident, $output3:ident) => {{
        let token0 = $base;
        let token1 = $base + 1;
        let token2 = $base + 2;
        let token3 = $base + 3;
        let q0 = 0.25 * $query.load_or(token0, $key_index, 0.0);
        let q1 = 0.25 * $query.load_or(token1, $key_index, 0.0);
        let q2 = 0.25 * $query.load_or(token2, $key_index, 0.0);
        let q3 = 0.25 * $query.load_or(token3, $key_index, 0.0);
        let k0 = $key.load_or(token0, $key_index, 0.0);
        let k1 = $key.load_or(token1, $key_index, 0.0);
        let k2 = $key.load_or(token2, $key_index, 0.0);
        let k3 = $key.load_or(token3, $key_index, 0.0);
        let a0 = $alpha.load_or(token0, $key_index, 0.0);
        let a1 = $alpha.load_or(token1, $key_index, 0.0);
        let a2 = $alpha.load_or(token2, $key_index, 0.0);
        let a3 = $alpha.load_or(token3, $key_index, 0.0);
        let c0 = a0;
        let c1 = a0 * a1;
        let c2 = c1 * a2;
        let c3 = c2 * a3;
        let h0 = partition16_reduce_sum_v1(&$subgroup, c0 * k0 * $state);
        let h1 = partition16_reduce_sum_v1(&$subgroup, c1 * k1 * $state);
        let h2 = partition16_reduce_sum_v1(&$subgroup, c2 * k2 * $state);
        let h3 = partition16_reduce_sum_v1(&$subgroup, c3 * k3 * $state);
        let beta0 = $beta.load_or(0, token0, 0.0);
        let beta1 = $beta.load_or(0, token1, 0.0);
        let beta2 = $beta.load_or(0, token2, 0.0);
        let beta3 = $beta.load_or(0, token3, 0.0);
        let l10 = beta1 * partition16_reduce_sum_v1(&$subgroup, a1 * k1 * k0);
        let l20 = beta2 * partition16_reduce_sum_v1(&$subgroup, a1 * a2 * k2 * k0);
        let l21 = beta2 * partition16_reduce_sum_v1(&$subgroup, a2 * k2 * k1);
        let l30 = beta3 * partition16_reduce_sum_v1(&$subgroup, a1 * a2 * a3 * k3 * k0);
        let l31 = beta3 * partition16_reduce_sum_v1(&$subgroup, a2 * a3 * k3 * k1);
        let l32 = beta3 * partition16_reduce_sum_v1(&$subgroup, a3 * k3 * k2);
        let z0 = beta0 * ($value.load_or(token0, $value_column, 0.0) - h0);
        let z1 = beta1 * ($value.load_or(token1, $value_column, 0.0) - h1) - l10 * z0;
        let z2 = beta2 * ($value.load_or(token2, $value_column, 0.0) - h2) - l20 * z0 - l21 * z1;
        let z3 = beta3 * ($value.load_or(token3, $value_column, 0.0) - h3)
            - l30 * z0
            - l31 * z1
            - l32 * z2;
        let base0 = partition16_reduce_sum_v1(&$subgroup, c0 * q0 * $state);
        let base1 = partition16_reduce_sum_v1(&$subgroup, c1 * q1 * $state);
        let base2 = partition16_reduce_sum_v1(&$subgroup, c2 * q2 * $state);
        let base3 = partition16_reduce_sum_v1(&$subgroup, c3 * q3 * $state);
        let r00 = partition16_reduce_sum_v1(&$subgroup, q0 * k0);
        let r10 = partition16_reduce_sum_v1(&$subgroup, a1 * q1 * k0);
        let r11 = partition16_reduce_sum_v1(&$subgroup, q1 * k1);
        let r20 = partition16_reduce_sum_v1(&$subgroup, a1 * a2 * q2 * k0);
        let r21 = partition16_reduce_sum_v1(&$subgroup, a2 * q2 * k1);
        let r22 = partition16_reduce_sum_v1(&$subgroup, q2 * k2);
        let r30 = partition16_reduce_sum_v1(&$subgroup, a1 * a2 * a3 * q3 * k0);
        let r31 = partition16_reduce_sum_v1(&$subgroup, a2 * a3 * q3 * k1);
        let r32 = partition16_reduce_sum_v1(&$subgroup, a3 * q3 * k2);
        let r33 = partition16_reduce_sum_v1(&$subgroup, q3 * k3);
        $output0 = base0 + r00 * z0;
        $output1 = base1 + r10 * z0 + r11 * z1;
        $output2 = base2 + r20 * z0 + r21 * z1 + r22 * z2;
        $output3 = base3 + r30 * z0 + r31 * z1 + r32 * z2 + r33 * z3;
        $state = c3 * $state + a1 * a2 * a3 * k0 * z0 + a2 * a3 * k1 * z1 + a3 * k2 * z2 + k3 * z3;
    }};
}

/// Evaluates one exact matrix-state Kimi Delta Attention decode step.
#[cfg(all(
    target_arch = "amdgpu",
    feature = "kernel-kda-decode",
    not(feature = "kernel-kda-decode-baseline-v1")
))]
#[kernel(
    typed,
    launch(required = [256, 1, 1], max = [256, 1, 1], max_grid = [4, 1, 1])
)]
pub fn gfx950_kda_decode(
    mut context: KernelContext<'_>,
    query: Global<'_, f32, ReadOnly>,
    key: Global<'_, f32, ReadOnly>,
    value: Global<'_, f32, ReadOnly>,
    alpha: Global<'_, f32, ReadOnly>,
    beta: Global<'_, f32, ReadOnly>,
    initial_state: Global<'_, f32, ReadOnly>,
    mut final_state: Global<'_, f32, DisjointWrite<Index1D>>,
    mut output: Global<'_, f32, DisjointWrite<Index1D>>,
) {
    // One workgroup owns one complete 16x16 matrix-state problem.
    let invocation = context.invocation();
    let grid = invocation.grid_size();
    if invocation.workgroup_size().x() != 256
        || invocation.workgroup_size().y() != 1
        || invocation.workgroup_size().z() != 1
        || grid.y() != 1
        || grid.z() != 1
    {
        fe2o3_device::trap();
    }
    let Some(batches) = batch_count_for_launch_v1(grid.x(), 256) else {
        fe2o3_device::trap();
    };
    let batch = invocation.workgroup_id().x() as usize;
    if query.len() != batches * KDA_KEY_DIMENSION_V1
        || key.len() != batches * KDA_KEY_DIMENSION_V1
        || value.len() != batches * KDA_VALUE_DIMENSION_V1
        || alpha.len() != batches * KDA_KEY_DIMENSION_V1
        || beta.len() != batches
        || initial_state.len() != batches * KDA_STATE_ELEMENTS_V1
        || final_state.len() != batches * KDA_STATE_ELEMENTS_V1
        || output.len() != batches * KDA_STATE_ELEMENTS_V1
    {
        return;
    }
    #[cfg(not(feature = "kernel-kda-decode-baseline-v1"))]
    {
        // The 256 threads map bijectively to (value column, key index).
        let linear = invocation.workitem_id().x() as usize;
        let key_index = linear & 15;
        let value_column = linear >> 4;
        let query_base = batch.wrapping_mul(KDA_KEY_DIMENSION_V1);
        let value_base = batch.wrapping_mul(KDA_VALUE_DIMENSION_V1);
        let state_base = batch.wrapping_mul(KDA_STATE_ELEMENTS_V1);
        let query_value = global_load_2d_f32_v1(&query, query_base, 0, key_index, 16, 0.0);
        let key_value = global_load_2d_f32_v1(&key, query_base, 0, key_index, 16, 0.0);
        let alpha_value = global_load_2d_f32_v1(&alpha, query_base, 0, key_index, 16, 0.0);
        let value_input = global_load_2d_f32_v1(&value, value_base, 0, value_column, 16, 0.0);
        let step = global_load_2d_f32_v1(&beta, batch, 0, 0, 1, 0.0);
        let decay = alpha_value
            * global_load_2d_f32_v1(&initial_state, state_base, value_column, key_index, 16, 0.0);
        // Narrow compiler-issued Wave64 authority to exact gfx950 Wave16 collectives.
        let (updated, result) = context.with_workgroup(|workgroup| {
            let subgroup = workgroup.subgroup::<SubgroupWidth64>();
            let subgroup = subgroup.gfx950_wave16(workgroup.epoch());
            let prediction = partition16_reduce_sum_v1(&subgroup, key_value * decay);
            let error = value_input - prediction;
            let updated = decay + step * key_value * error;
            let result = partition16_reduce_sum_v1(&subgroup, 0.25 * query_value * updated);
            (updated, result)
        });
        // Each thread publishes its state element and replicated output element.
        if !final_state.store(context.invocation().index_1d().into_disjoint(), updated)
            || !output.store(context.invocation().index_1d().into_disjoint(), result)
        {
            fe2o3_device::trap();
        }
    }
}

/// Evaluates two exact four-token WY/UT KDA chunks with one register state carry.
#[cfg(all(
    target_arch = "amdgpu",
    feature = "kernel-kda-prefill",
    not(feature = "kernel-kda-prefill-baseline-v1")
))]
#[kernel(
    typed,
    launch(required = [256, 1, 1], max = [256, 1, 1], max_grid = [4, 1, 1])
)]
pub fn gfx950_kda_chunkwise_prefill(
    mut context: KernelContext<'_>,
    query: Global<'_, f32, ReadOnly>,
    key: Global<'_, f32, ReadOnly>,
    value: Global<'_, f32, ReadOnly>,
    alpha: Global<'_, f32, ReadOnly>,
    beta: Global<'_, f32, ReadOnly>,
    initial_state: Global<'_, f32, ReadOnly>,
    mut final_state: Global<'_, f32, DisjointWrite<Index1D>>,
    mut output_chunk0: Global<'_, f32, DisjointWrite<Index1D>>,
    mut output_chunk1: Global<'_, f32, DisjointWrite<Index1D>>,
) {
    // One workgroup owns one eight-token problem and its carried matrix state.
    let invocation = context.invocation();
    let grid = invocation.grid_size();
    if invocation.workgroup_size().x() != 256
        || invocation.workgroup_size().y() != 1
        || invocation.workgroup_size().z() != 1
        || grid.y() != 1
        || grid.z() != 1
    {
        fe2o3_device::trap();
    }
    let Some(batches) = batch_count_for_launch_v1(grid.x(), 256) else {
        fe2o3_device::trap();
    };
    let batch = invocation.workgroup_id().x() as usize;
    if query.len() != batches * PREFILL_TOKENS_V1 * KDA_KEY_DIMENSION_V1
        || key.len() != batches * PREFILL_TOKENS_V1 * KDA_KEY_DIMENSION_V1
        || value.len() != batches * PREFILL_TOKENS_V1 * KDA_VALUE_DIMENSION_V1
        || alpha.len() != batches * PREFILL_TOKENS_V1 * KDA_KEY_DIMENSION_V1
        || beta.len() != batches * PREFILL_TOKENS_V1
        || initial_state.len() != batches * KDA_STATE_ELEMENTS_V1
        || final_state.len() != batches * KDA_STATE_ELEMENTS_V1
        || output_chunk0.len() != batches * KDA_STATE_ELEMENTS_V1
        || output_chunk1.len() != batches * KDA_STATE_ELEMENTS_V1
    {
        return;
    }
    // Convert the workgroup batch to checked token-major input views.
    let token_base = batch.wrapping_mul(PREFILL_TOKENS_V1);
    let Some(query) = GlobalReadView2DF32V1::checked(
        &query,
        token_base.wrapping_mul(KDA_KEY_DIMENSION_V1),
        8,
        16,
        16,
    ) else {
        return;
    };
    let Some(key) = GlobalReadView2DF32V1::checked(
        &key,
        token_base.wrapping_mul(KDA_KEY_DIMENSION_V1),
        8,
        16,
        16,
    ) else {
        return;
    };
    let Some(value) = GlobalReadView2DF32V1::checked(
        &value,
        token_base.wrapping_mul(KDA_VALUE_DIMENSION_V1),
        8,
        16,
        16,
    ) else {
        return;
    };
    let Some(alpha) = GlobalReadView2DF32V1::checked(
        &alpha,
        token_base.wrapping_mul(KDA_KEY_DIMENSION_V1),
        8,
        16,
        16,
    ) else {
        return;
    };
    let Some(beta) = GlobalReadView2DF32V1::checked(&beta, token_base, 1, 8, 8) else {
        return;
    };
    let Some(initial_state) = GlobalReadView2DF32V1::checked(
        &initial_state,
        batch.wrapping_mul(KDA_STATE_ELEMENTS_V1),
        16,
        16,
        16,
    ) else {
        return;
    };
    // The 256 threads map bijectively to (value column, key index).
    let linear = invocation.workitem_id().x() as usize;
    let key_index = linear & 15;
    let value_column = linear >> 4;
    let (selected0, selected1, state) = context.with_workgroup(|workgroup| {
        let subgroup = workgroup.subgroup::<SubgroupWidth64>();
        let subgroup = subgroup.gfx950_wave16(workgroup.epoch());
        let mut state = initial_state.load_or(value_column, key_index, 0.0);
        let mut c00 = 0.0;
        let mut c01 = 0.0;
        let mut c02 = 0.0;
        let mut c03 = 0.0;
        // Execute two ordered four-token chunks; state is the explicit carry.
        kda_chunk_wy_v1!(
            0,
            query,
            key,
            value,
            alpha,
            beta,
            subgroup,
            key_index,
            value_column,
            state,
            c00,
            c01,
            c02,
            c03
        );
        let mut c10 = 0.0;
        let mut c11 = 0.0;
        let mut c12 = 0.0;
        let mut c13 = 0.0;
        kda_chunk_wy_v1!(
            4,
            query,
            key,
            value,
            alpha,
            beta,
            subgroup,
            key_index,
            value_column,
            state,
            c10,
            c11,
            c12,
            c13
        );
        let selected0 = if key_index < 4 {
            c00
        } else if key_index < 8 {
            c01
        } else if key_index < 12 {
            c02
        } else {
            c03
        };
        let selected1 = if key_index < 4 {
            c10
        } else if key_index < 8 {
            c11
        } else if key_index < 12 {
            c12
        } else {
            c13
        };
        (selected0, selected1, state)
    });
    if !output_chunk0.store(context.invocation().index_1d().into_disjoint(), selected0)
        || !output_chunk1.store(context.invocation().index_1d().into_disjoint(), selected1)
        || !final_state.store(context.invocation().index_1d().into_disjoint(), state)
    {
        fe2o3_device::trap();
    }
}

/// Selects two content blocks, retains three tokens, and computes one 16-value output.
#[cfg(all(target_arch = "amdgpu", feature = "kernel-content-sparse-attention"))]
#[cfg_attr(
    not(feature = "kernel-content-sparse-attention-reciprocal-reuse-v1"),
    kernel(
        typed,
        launch(required = [256, 1, 1], max = [256, 1, 1], max_grid = [4, 1, 1])
    )
)]
#[cfg_attr(
    feature = "kernel-content-sparse-attention-reciprocal-reuse-v1",
    kernel(
        typed,
        launch(required = [256, 1, 1], max = [256, 1, 1], max_grid = [4, 1, 1])
    )
)]
pub fn gfx950_content_sparse_attention(
    mut context: KernelContext<'_>,
    q: Global<'_, u8, ReadOnly>,
    k: Global<'_, u8, ReadOnly>,
    v: Global<'_, u8, ReadOnly>,
    content_scores: Global<'_, f32, ReadOnly>,
    mut output: Global<'_, f32, ExclusiveReadWrite>,
    mut selected_output: Global<'_, u32, ExclusiveReadWrite>,
) {
    let invocation = context.invocation();
    let grid = invocation.grid_size();
    if invocation.workgroup_size().x() != 256
        || invocation.workgroup_size().y() != 1
        || invocation.workgroup_size().z() != 1
        || grid.y() != 1
        || grid.z() != 1
    {
        fe2o3_device::trap();
    }
    let Some(wave_batches) = batch_count_for_launch_v1(grid.x(), 64) else {
        fe2o3_device::trap();
    };
    // Reject malformed launch-wide storage before any subgroup collective.
    if q.len() != wave_batches * ATTENTION_TOKENS_V1 * HEAD_DIMENSION_V1
        || k.len() != wave_batches * ATTENTION_TOKENS_V1 * HEAD_DIMENSION_V1
        || v.len() != wave_batches * ATTENTION_TOKENS_V1 * CHANNELS_V1
        || content_scores.len() != wave_batches * ATTENTION_TOKENS_V1
        || output.len() != wave_batches * CHANNELS_V1
        || selected_output.len() != wave_batches * SELECTED_TOKENS_V1
    {
        fe2o3_device::trap();
    }
    // Each global Wave64 owns one attention item; its lanes cover 16 columns.
    let index = invocation.index_1d();
    let batch = index.get() / 64;
    let wave_lane = index.get() % 64;
    let column = wave_lane % CHANNELS_V1;
    let Some(content) = GlobalReadView2DF32V1::checked(
        &content_scores,
        batch.wrapping_mul(ATTENTION_TOKENS_V1),
        1,
        ATTENTION_TOKENS_V1,
        ATTENTION_TOKENS_V1,
    ) else {
        fe2o3_device::trap();
    };
    let c0 = content.load_or(0, 0, f32::NEG_INFINITY);
    let c1 = content.load_or(0, 1, f32::NEG_INFINITY);
    let c2 = content.load_or(0, 2, f32::NEG_INFINITY);
    let c3 = content.load_or(0, 3, f32::NEG_INFINITY);
    let c4 = content.load_or(0, 4, f32::NEG_INFINITY);
    let c5 = content.load_or(0, 5, f32::NEG_INFINITY);
    let c6 = content.load_or(0, 6, f32::NEG_INFINITY);
    let c7 = content.load_or(0, 7, f32::NEG_INFINITY);
    let c8 = content.load_or(0, 8, f32::NEG_INFINITY);
    let c9 = content.load_or(0, 9, f32::NEG_INFINITY);
    let c10 = content.load_or(0, 10, f32::NEG_INFINITY);
    let c11 = content.load_or(0, 11, f32::NEG_INFINITY);
    let c12 = content.load_or(0, 12, f32::NEG_INFINITY);
    let c13 = content.load_or(0, 13, f32::NEG_INFINITY);
    let c14 = content.load_or(0, 14, f32::NEG_INFINITY);
    let c15 = content.load_or(0, 15, f32::NEG_INFINITY);
    // All lanes execute the same typed reductions and receive every token score.
    let (a0, a1, a2, a3, a4, a5, a6, a7, a8, a9, a10, a11, a12, a13, a14, a15) = context
        .with_workgroup(|workgroup| {
            let subgroup = workgroup.subgroup::<SubgroupWidth64>();
            let epoch = workgroup.epoch();
            (
                fp8_attention_score_v1(&subgroup, epoch, &q, &k, batch, 0) + 0.75 * c0,
                fp8_attention_score_v1(&subgroup, epoch, &q, &k, batch, 1) + 0.75 * c1,
                fp8_attention_score_v1(&subgroup, epoch, &q, &k, batch, 2) + 0.75 * c2,
                fp8_attention_score_v1(&subgroup, epoch, &q, &k, batch, 3) + 0.75 * c3,
                fp8_attention_score_v1(&subgroup, epoch, &q, &k, batch, 4) + 0.75 * c4,
                fp8_attention_score_v1(&subgroup, epoch, &q, &k, batch, 5) + 0.75 * c5,
                fp8_attention_score_v1(&subgroup, epoch, &q, &k, batch, 6) + 0.75 * c6,
                fp8_attention_score_v1(&subgroup, epoch, &q, &k, batch, 7) + 0.75 * c7,
                fp8_attention_score_v1(&subgroup, epoch, &q, &k, batch, 8) + 0.75 * c8,
                fp8_attention_score_v1(&subgroup, epoch, &q, &k, batch, 9) + 0.75 * c9,
                fp8_attention_score_v1(&subgroup, epoch, &q, &k, batch, 10) + 0.75 * c10,
                fp8_attention_score_v1(&subgroup, epoch, &q, &k, batch, 11) + 0.75 * c11,
                fp8_attention_score_v1(&subgroup, epoch, &q, &k, batch, 12) + 0.75 * c12,
                fp8_attention_score_v1(&subgroup, epoch, &q, &k, batch, 13) + 0.75 * c13,
                fp8_attention_score_v1(&subgroup, epoch, &q, &k, batch, 14) + 0.75 * c14,
                fp8_attention_score_v1(&subgroup, epoch, &q, &k, batch, 15) + 0.75 * c15,
            )
        });
    let policy = context.numerical_policy::<StrictIeee>();
    let device_math = context.math();
    let math = device_math.with_numerical_policy(&policy);

    let mut block0 = c0;
    if c1 > block0 {
        block0 = c1;
    }
    if c2 > block0 {
        block0 = c2;
    }
    if c3 > block0 {
        block0 = c3;
    }
    let mut block1 = c4;
    if c5 > block1 {
        block1 = c5;
    }
    if c6 > block1 {
        block1 = c6;
    }
    if c7 > block1 {
        block1 = c7;
    }
    let mut block2 = c8;
    if c9 > block2 {
        block2 = c9;
    }
    if c10 > block2 {
        block2 = c10;
    }
    if c11 > block2 {
        block2 = c11;
    }
    let mut block3 = c12;
    if c13 > block3 {
        block3 = c13;
    }
    if c14 > block3 {
        block3 = c14;
    }
    if c15 > block3 {
        block3 = c15;
    }
    let mut first_block = 0;
    let mut first_block_score = block0;
    if block1 > first_block_score {
        first_block = 1;
        first_block_score = block1;
    }
    if block2 > first_block_score {
        first_block = 2;
        first_block_score = block2;
    }
    if block3 > first_block_score {
        first_block = 3;
    }
    let mut second_block = if first_block == 0 { 1 } else { 0 };
    let mut second_block_score = if second_block == 0 { block0 } else { block1 };
    if first_block != 1 && block1 > second_block_score {
        second_block = 1;
        second_block_score = block1;
    }
    if first_block != 2 && block2 > second_block_score {
        second_block = 2;
        second_block_score = block2;
    }
    if first_block != 3 && block3 > second_block_score {
        second_block = 3;
    }

    let keep0 = first_block == 0 || second_block == 0;
    let keep1 = first_block == 1 || second_block == 1;
    let keep2 = first_block == 2 || second_block == 2;
    let keep3 = first_block == 3 || second_block == 3;
    let e0 = if keep0 { c0 } else { f32::NEG_INFINITY };
    let e1 = if keep0 { c1 } else { f32::NEG_INFINITY };
    let e2 = if keep0 { c2 } else { f32::NEG_INFINITY };
    let e3 = if keep0 { c3 } else { f32::NEG_INFINITY };
    let e4 = if keep1 { c4 } else { f32::NEG_INFINITY };
    let e5 = if keep1 { c5 } else { f32::NEG_INFINITY };
    let e6 = if keep1 { c6 } else { f32::NEG_INFINITY };
    let e7 = if keep1 { c7 } else { f32::NEG_INFINITY };
    let e8 = if keep2 { c8 } else { f32::NEG_INFINITY };
    let e9 = if keep2 { c9 } else { f32::NEG_INFINITY };
    let e10 = if keep2 { c10 } else { f32::NEG_INFINITY };
    let e11 = if keep2 { c11 } else { f32::NEG_INFINITY };
    let e12 = if keep3 { c12 } else { f32::NEG_INFINITY };
    let e13 = if keep3 { c13 } else { f32::NEG_INFINITY };
    let e14 = if keep3 { c14 } else { f32::NEG_INFINITY };
    let e15 = if keep3 { c15 } else { f32::NEG_INFINITY };

    let mut selected0 = usize::MAX;
    let mut selected0_rank = f32::NEG_INFINITY;
    let mut selected0_attention = f32::NEG_INFINITY;
    let mut selected1 = usize::MAX;
    let mut selected1_rank = f32::NEG_INFINITY;
    let mut selected1_attention = f32::NEG_INFINITY;
    let mut selected2 = usize::MAX;
    let mut selected2_rank = f32::NEG_INFINITY;
    let mut selected2_attention = f32::NEG_INFINITY;
    consider_sparse_candidate_v1!(
        0,
        e0,
        a0,
        selected0,
        selected0_rank,
        selected0_attention,
        selected1,
        selected1_rank,
        selected1_attention,
        selected2,
        selected2_rank,
        selected2_attention
    );
    consider_sparse_candidate_v1!(
        1,
        e1,
        a1,
        selected0,
        selected0_rank,
        selected0_attention,
        selected1,
        selected1_rank,
        selected1_attention,
        selected2,
        selected2_rank,
        selected2_attention
    );
    consider_sparse_candidate_v1!(
        2,
        e2,
        a2,
        selected0,
        selected0_rank,
        selected0_attention,
        selected1,
        selected1_rank,
        selected1_attention,
        selected2,
        selected2_rank,
        selected2_attention
    );
    consider_sparse_candidate_v1!(
        3,
        e3,
        a3,
        selected0,
        selected0_rank,
        selected0_attention,
        selected1,
        selected1_rank,
        selected1_attention,
        selected2,
        selected2_rank,
        selected2_attention
    );
    consider_sparse_candidate_v1!(
        4,
        e4,
        a4,
        selected0,
        selected0_rank,
        selected0_attention,
        selected1,
        selected1_rank,
        selected1_attention,
        selected2,
        selected2_rank,
        selected2_attention
    );
    consider_sparse_candidate_v1!(
        5,
        e5,
        a5,
        selected0,
        selected0_rank,
        selected0_attention,
        selected1,
        selected1_rank,
        selected1_attention,
        selected2,
        selected2_rank,
        selected2_attention
    );
    consider_sparse_candidate_v1!(
        6,
        e6,
        a6,
        selected0,
        selected0_rank,
        selected0_attention,
        selected1,
        selected1_rank,
        selected1_attention,
        selected2,
        selected2_rank,
        selected2_attention
    );
    consider_sparse_candidate_v1!(
        7,
        e7,
        a7,
        selected0,
        selected0_rank,
        selected0_attention,
        selected1,
        selected1_rank,
        selected1_attention,
        selected2,
        selected2_rank,
        selected2_attention
    );
    consider_sparse_candidate_v1!(
        8,
        e8,
        a8,
        selected0,
        selected0_rank,
        selected0_attention,
        selected1,
        selected1_rank,
        selected1_attention,
        selected2,
        selected2_rank,
        selected2_attention
    );
    consider_sparse_candidate_v1!(
        9,
        e9,
        a9,
        selected0,
        selected0_rank,
        selected0_attention,
        selected1,
        selected1_rank,
        selected1_attention,
        selected2,
        selected2_rank,
        selected2_attention
    );
    consider_sparse_candidate_v1!(
        10,
        e10,
        a10,
        selected0,
        selected0_rank,
        selected0_attention,
        selected1,
        selected1_rank,
        selected1_attention,
        selected2,
        selected2_rank,
        selected2_attention
    );
    consider_sparse_candidate_v1!(
        11,
        e11,
        a11,
        selected0,
        selected0_rank,
        selected0_attention,
        selected1,
        selected1_rank,
        selected1_attention,
        selected2,
        selected2_rank,
        selected2_attention
    );
    consider_sparse_candidate_v1!(
        12,
        e12,
        a12,
        selected0,
        selected0_rank,
        selected0_attention,
        selected1,
        selected1_rank,
        selected1_attention,
        selected2,
        selected2_rank,
        selected2_attention
    );
    consider_sparse_candidate_v1!(
        13,
        e13,
        a13,
        selected0,
        selected0_rank,
        selected0_attention,
        selected1,
        selected1_rank,
        selected1_attention,
        selected2,
        selected2_rank,
        selected2_attention
    );
    consider_sparse_candidate_v1!(
        14,
        e14,
        a14,
        selected0,
        selected0_rank,
        selected0_attention,
        selected1,
        selected1_rank,
        selected1_attention,
        selected2,
        selected2_rank,
        selected2_attention
    );
    consider_sparse_candidate_v1!(
        15,
        e15,
        a15,
        selected0,
        selected0_rank,
        selected0_attention,
        selected1,
        selected1_rank,
        selected1_attention,
        selected2,
        selected2_rank,
        selected2_attention
    );
    // Only the first three ranks write IDs; row-striped ownership prevents races.
    let selected_rank = wave_lane;
    if selected_rank < SELECTED_TOKENS_V1 {
        let selected = if selected_rank == 0 {
            selected0
        } else if selected_rank == 1 {
            selected1
        } else {
            selected2
        };
        let output_index = batch * SELECTED_TOKENS_V1 + selected_rank;
        if !selected_output.store(output_index, selected as u32) {
            fe2o3_device::trap();
        }
    }

    // Normalize only the retained tokens with max-subtracted FP32 softmax.
    let mut maximum = selected0_attention;
    if selected1_attention > maximum {
        maximum = selected1_attention;
    }
    if selected2_attention > maximum {
        maximum = selected2_attention;
    }
    let weight0 = math.exp_f32(selected0_attention - maximum);
    let weight1 = math.exp_f32(selected1_attention - maximum);
    let weight2 = math.exp_f32(selected2_attention - maximum);
    let denominator = weight0 + weight1 + weight2;
    let value_base = batch.wrapping_mul(ATTENTION_TOKENS_V1 * CHANNELS_V1);
    let value0 = v
        .load(value_base + selected0 * CHANNELS_V1 + column)
        .unwrap_or(0);
    let value1 = v
        .load(value_base + selected1 * CHANNELS_V1 + column)
        .unwrap_or(0);
    let value2 = v
        .load(value_base + selected2 * CHANNELS_V1 + column)
        .unwrap_or(0);
    #[cfg(not(feature = "kernel-content-sparse-attention-reciprocal-reuse-v1"))]
    let result = weight0 / denominator * decode_fp8_e4m3_v1!(value0)
        + weight1 / denominator * decode_fp8_e4m3_v1!(value1)
        + weight2 / denominator * decode_fp8_e4m3_v1!(value2);
    #[cfg(feature = "kernel-content-sparse-attention-reciprocal-reuse-v1")]
    let result = {
        let reciprocal = 1.0 / denominator;
        (weight0 * decode_fp8_e4m3_v1!(value0)
            + weight1 * decode_fp8_e4m3_v1!(value1)
            + weight2 * decode_fp8_e4m3_v1!(value2))
            * reciprocal
    };
    let output_gate = 1.0 / (1.0 + math.exp_f32(-maximum * 0.01));
    // The first 16 lanes own the compact output row; the mapping is injective.
    if wave_lane < CHANNELS_V1 && !output.store(batch * CHANNELS_V1 + column, result * output_gate)
    {
        fe2o3_device::trap();
    }
}

/// Consumes Lightning Indexer top-k token IDs and evaluates only those KV rows.
#[cfg(all(target_arch = "amdgpu", feature = "kernel-deepseek-sparse-attention"))]
#[kernel(
    typed,
    launch(required = [256, 1, 1], max = [256, 1, 1], max_grid = [4, 1, 1])
)]
pub fn gfx950_deepseek_sparse_attention(
    mut context: KernelContext<'_>,
    q: Global<'_, f32, ReadOnly>,
    k: Global<'_, f32, ReadOnly>,
    v: Global<'_, f32, ReadOnly>,
    index0: u32,
    index1: u32,
    index2: u32,
    index3: u32,
    mut output: Global<'_, f32, DisjointWrite<Index1D>>,
    mut softmax_maximum_output: Global<'_, f32, DisjointWrite<Index1D>>,
    mut softmax_normalizer_output: Global<'_, f32, DisjointWrite<Index1D>>,
) {
    let invocation = context.invocation();
    let grid = invocation.grid_size();
    if invocation.workgroup_size().x() != 256
        || invocation.workgroup_size().y() != 1
        || invocation.workgroup_size().z() != 1
        || grid.y() != 1
        || grid.z() != 1
    {
        fe2o3_device::trap();
    }
    // Validate all batch-major tensors before any Wave16 reduction.
    let Some(batches) = batch_count_for_launch_v1(grid.x(), 16) else {
        fe2o3_device::trap();
    };
    if q.len() != batches * HEAD_DIMENSION_V1
        || k.len() != batches * ATTENTION_TOKENS_V1 * HEAD_DIMENSION_V1
        || v.len() != batches * ATTENTION_TOKENS_V1 * CHANNELS_V1
        || output.len() != batches * CHANNELS_V1
        || softmax_maximum_output.len() != batches * CHANNELS_V1
        || softmax_normalizer_output.len() != batches * CHANNELS_V1
    {
        fe2o3_device::trap();
    }

    // Convert sentinels to safe addresses, then carry validity as an explicit mask.
    let raw0 = index0;
    let raw1 = index1;
    let raw2 = index2;
    let raw3 = index3;
    let valid0 = raw0 < ATTENTION_TOKENS_V1 as u32;
    let valid1 = raw1 < ATTENTION_TOKENS_V1 as u32;
    let valid2 = raw2 < ATTENTION_TOKENS_V1 as u32;
    let valid3 = raw3 < ATTENTION_TOKENS_V1 as u32;
    let token0 = raw0 as usize % ATTENTION_TOKENS_V1;
    let token1 = raw1 as usize % ATTENTION_TOKENS_V1;
    let token2 = raw2 as usize % ATTENTION_TOKENS_V1;
    let token3 = raw3 as usize % ATTENTION_TOKENS_V1;
    // Each Wave16 subgroup handles one batch; each lane owns one output channel.
    let linear_index = invocation.index_1d().get();
    let batch = linear_index / CHANNELS_V1;
    let column = linear_index % CHANNELS_V1;
    let Some(query_view) =
        GlobalReadView2DF32V1::checked(&q, 0, batches, HEAD_DIMENSION_V1, HEAD_DIMENSION_V1)
    else {
        fe2o3_device::trap();
    };
    let Some(key_view) = GlobalReadView2DF32V1::checked(
        &k,
        0,
        batches * ATTENTION_TOKENS_V1,
        HEAD_DIMENSION_V1,
        HEAD_DIMENSION_V1,
    ) else {
        fe2o3_device::trap();
    };
    let Some(value_view) = GlobalReadView2DF32V1::checked(
        &v,
        0,
        batches * ATTENTION_TOKENS_V1,
        CHANNELS_V1,
        CHANNELS_V1,
    ) else {
        fe2o3_device::trap();
    };

    // Eight coalesced depth slices per lane cover the full 128-wide dot product.
    let depth0 = column;
    let depth1 = column + CHANNELS_V1;
    let depth2 = column + 2 * CHANNELS_V1;
    let depth3 = column + 3 * CHANNELS_V1;
    let depth4 = column + 4 * CHANNELS_V1;
    let depth5 = column + 5 * CHANNELS_V1;
    let depth6 = column + 6 * CHANNELS_V1;
    let depth7 = column + 7 * CHANNELS_V1;
    let query0 = query_view.load_or(batch, depth0, 0.0);
    let query1 = query_view.load_or(batch, depth1, 0.0);
    let query2 = query_view.load_or(batch, depth2, 0.0);
    let query3 = query_view.load_or(batch, depth3, 0.0);
    let query4 = query_view.load_or(batch, depth4, 0.0);
    let query5 = query_view.load_or(batch, depth5, 0.0);
    let query6 = query_view.load_or(batch, depth6, 0.0);
    let query7 = query_view.load_or(batch, depth7, 0.0);
    let batch_row = batch.wrapping_mul(ATTENTION_TOKENS_V1);
    let row0 = batch_row.wrapping_add(token0);
    let row1 = batch_row.wrapping_add(token1);
    let row2 = batch_row.wrapping_add(token2);
    let row3 = batch_row.wrapping_add(token3);
    // Invalid slots use the safe row zero and are masked after the uniform
    // collective sequence. Every Wave16 subgroup therefore executes the same
    // four reductions even when the top-k list contains sentinels.
    let partial0 = query0 * key_view.load_or(row0, depth0, 0.0)
        + query1 * key_view.load_or(row0, depth1, 0.0)
        + query2 * key_view.load_or(row0, depth2, 0.0)
        + query3 * key_view.load_or(row0, depth3, 0.0)
        + query4 * key_view.load_or(row0, depth4, 0.0)
        + query5 * key_view.load_or(row0, depth5, 0.0)
        + query6 * key_view.load_or(row0, depth6, 0.0)
        + query7 * key_view.load_or(row0, depth7, 0.0);
    let partial1 = query0 * key_view.load_or(row1, depth0, 0.0)
        + query1 * key_view.load_or(row1, depth1, 0.0)
        + query2 * key_view.load_or(row1, depth2, 0.0)
        + query3 * key_view.load_or(row1, depth3, 0.0)
        + query4 * key_view.load_or(row1, depth4, 0.0)
        + query5 * key_view.load_or(row1, depth5, 0.0)
        + query6 * key_view.load_or(row1, depth6, 0.0)
        + query7 * key_view.load_or(row1, depth7, 0.0);
    let partial2 = query0 * key_view.load_or(row2, depth0, 0.0)
        + query1 * key_view.load_or(row2, depth1, 0.0)
        + query2 * key_view.load_or(row2, depth2, 0.0)
        + query3 * key_view.load_or(row2, depth3, 0.0)
        + query4 * key_view.load_or(row2, depth4, 0.0)
        + query5 * key_view.load_or(row2, depth5, 0.0)
        + query6 * key_view.load_or(row2, depth6, 0.0)
        + query7 * key_view.load_or(row2, depth7, 0.0);
    let partial3 = query0 * key_view.load_or(row3, depth0, 0.0)
        + query1 * key_view.load_or(row3, depth1, 0.0)
        + query2 * key_view.load_or(row3, depth2, 0.0)
        + query3 * key_view.load_or(row3, depth3, 0.0)
        + query4 * key_view.load_or(row3, depth4, 0.0)
        + query5 * key_view.load_or(row3, depth5, 0.0)
        + query6 * key_view.load_or(row3, depth6, 0.0)
        + query7 * key_view.load_or(row3, depth7, 0.0);

    let (reduced0, reduced1, reduced2, reduced3) = context.with_workgroup(|workgroup| {
        let subgroup = workgroup.subgroup::<SubgroupWidth64>();
        let subgroup = subgroup.gfx950_wave16(workgroup.epoch());
        (
            partition16_reduce_sum_v1(&subgroup, partial0),
            partition16_reduce_sum_v1(&subgroup, partial1),
            partition16_reduce_sum_v1(&subgroup, partial2),
            partition16_reduce_sum_v1(&subgroup, partial3),
        )
    });
    if !(valid0 || valid1 || valid2 || valid3)
        || (valid0 && valid1 && raw0 == raw1)
        || (valid0 && valid2 && raw0 == raw2)
        || (valid0 && valid3 && raw0 == raw3)
        || (valid1 && valid2 && raw1 == raw2)
        || (valid1 && valid3 && raw1 == raw3)
        || (valid2 && valid3 && raw2 == raw3)
    {
        fe2o3_device::trap();
    }
    let score0 = if valid0 {
        reduced0 * ATTENTION_SCALE_V1
    } else {
        f32::NEG_INFINITY
    };
    let score1 = if valid1 {
        reduced1 * ATTENTION_SCALE_V1
    } else {
        f32::NEG_INFINITY
    };
    let score2 = if valid2 {
        reduced2 * ATTENTION_SCALE_V1
    } else {
        f32::NEG_INFINITY
    };
    let score3 = if valid3 {
        reduced3 * ATTENTION_SCALE_V1
    } else {
        f32::NEG_INFINITY
    };
    let mut maximum = score0;
    if score1 > maximum {
        maximum = score1;
    }
    if score2 > maximum {
        maximum = score2;
    }
    if score3 > maximum {
        maximum = score3;
    }

    // Mask invalid candidates and apply a stable softmax over the retained rows.
    let policy = context.numerical_policy::<StrictIeee>();
    let device_math = context.math();
    let math = device_math.with_numerical_policy(&policy);
    #[cfg(not(feature = "kernel-deepseek-sparse-attention-leader-exp-v1"))]
    let weight0 = if valid0 {
        math.exp_f32(score0 - maximum)
    } else {
        0.0
    };
    #[cfg(not(feature = "kernel-deepseek-sparse-attention-leader-exp-v1"))]
    let weight1 = if valid1 {
        math.exp_f32(score1 - maximum)
    } else {
        0.0
    };
    #[cfg(not(feature = "kernel-deepseek-sparse-attention-leader-exp-v1"))]
    let weight2 = if valid2 {
        math.exp_f32(score2 - maximum)
    } else {
        0.0
    };
    #[cfg(not(feature = "kernel-deepseek-sparse-attention-leader-exp-v1"))]
    let weight3 = if valid3 {
        math.exp_f32(score3 - maximum)
    } else {
        0.0
    };
    #[cfg(feature = "kernel-deepseek-sparse-attention-leader-exp-v1")]
    let (weight0, weight1, weight2, weight3) = {
        // Counterexample: serialize subgroup-invariant exponentials on lane zero
        // and pay four exchanges. This lowers correctly but regresses top-4.
        let leader_weight0 = if column == 0 && valid0 {
            math.exp_f32(score0 - maximum)
        } else {
            0.0
        };
        let leader_weight1 = if column == 0 && valid1 {
            math.exp_f32(score1 - maximum)
        } else {
            0.0
        };
        let leader_weight2 = if column == 0 && valid2 {
            math.exp_f32(score2 - maximum)
        } else {
            0.0
        };
        let leader_weight3 = if column == 0 && valid3 {
            math.exp_f32(score3 - maximum)
        } else {
            0.0
        };
        context.with_workgroup(|workgroup| {
            let subgroup = workgroup.subgroup::<SubgroupWidth64>();
            let subgroup = subgroup.gfx950_wave16(workgroup.epoch());
            (
                subgroup.broadcast_f32(leader_weight0, 0),
                subgroup.broadcast_f32(leader_weight1, 0),
                subgroup.broadcast_f32(leader_weight2, 0),
                subgroup.broadcast_f32(leader_weight3, 0),
            )
        })
    };
    let normalizer = weight0 + weight1 + weight2 + weight3;
    let mut numerator = 0.0_f32;
    if valid0 {
        numerator += weight0 * value_view.load_or(row0, column, 0.0);
    }
    if valid1 {
        numerator += weight1 * value_view.load_or(row1, column, 0.0);
    }
    if valid2 {
        numerator += weight2 * value_view.load_or(row2, column, 0.0);
    }
    if valid3 {
        numerator += weight3 * value_view.load_or(row3, column, 0.0);
    }
    // Output, maximum, and normalizer share the same disjoint linear owner.
    if !output.store(
        context.invocation().index_1d().into_disjoint(),
        numerator / normalizer,
    ) || !softmax_maximum_output.store(context.invocation().index_1d().into_disjoint(), maximum)
        || !softmax_normalizer_output
            .store(context.invocation().index_1d().into_disjoint(), normalizer)
    {
        fe2o3_device::trap();
    }
}

/// Mixes a four-token local window with three four-token compressed global blocks.
#[cfg(all(target_arch = "amdgpu", feature = "kernel-compressed-hybrid-attention"))]
#[cfg_attr(
    not(feature = "kernel-compressed-hybrid-attention-division-baseline-v1"),
    kernel(
        typed,
        launch(required = [256, 1, 1], max = [256, 1, 1], max_grid = [4, 1, 1])
    )
)]
#[cfg_attr(
    feature = "kernel-compressed-hybrid-attention-division-baseline-v1",
    kernel(
        typed,
        launch(required = [256, 1, 1], max = [256, 1, 1], max_grid = [4, 1, 1])
    )
)]
pub fn gfx950_compressed_hybrid_attention(
    mut context: KernelContext<'_>,
    q: Global<'_, u8, ReadOnly>,
    k: Global<'_, u8, ReadOnly>,
    v: Global<'_, u8, ReadOnly>,
    token_bias: Global<'_, f32, ReadOnly>,
    mut output: Global<'_, f32, ExclusiveReadWrite>,
) {
    let invocation = context.invocation();
    let grid = invocation.grid_size();
    if invocation.workgroup_size().x() != 256
        || invocation.workgroup_size().y() != 1
        || invocation.workgroup_size().z() != 1
        || grid.y() != 1
        || grid.z() != 1
    {
        fe2o3_device::trap();
    }
    let Some(wave_batches) = batch_count_for_launch_v1(grid.x(), 64) else {
        fe2o3_device::trap();
    };
    // Validate exact fixed shapes before subgroup collectives.
    if q.len() != wave_batches * ATTENTION_TOKENS_V1 * HEAD_DIMENSION_V1
        || k.len() != wave_batches * ATTENTION_TOKENS_V1 * HEAD_DIMENSION_V1
        || v.len() != wave_batches * ATTENTION_TOKENS_V1 * CHANNELS_V1
        || token_bias.len() != wave_batches * ATTENTION_TOKENS_V1
        || output.len() != wave_batches * CHANNELS_V1
    {
        fe2o3_device::trap();
    }
    // A global wave owns one item; lane modulo 16 selects its output channel.
    let index = invocation.index_1d();
    let batch = index.get() / 64;
    let wave_lane = index.get() % 64;
    let column = wave_lane % CHANNELS_V1;
    let Some(bias) = GlobalReadView2DF32V1::checked(
        &token_bias,
        batch.wrapping_mul(ATTENTION_TOKENS_V1),
        1,
        16,
        16,
    ) else {
        fe2o3_device::trap();
    };
    let (score0, score4, score8, score12, score13, score14, score15) =
        context.with_workgroup(|workgroup| {
            let subgroup = workgroup.subgroup::<SubgroupWidth64>();
            let epoch = workgroup.epoch();
            (
                fp8_attention_score_v1(&subgroup, epoch, &q, &k, batch, 0)
                    + bias.load_or(0, 0, 0.0),
                fp8_attention_score_v1(&subgroup, epoch, &q, &k, batch, 4)
                    + bias.load_or(0, 4, 0.0),
                fp8_attention_score_v1(&subgroup, epoch, &q, &k, batch, 8)
                    + bias.load_or(0, 8, 0.0),
                fp8_attention_score_v1(&subgroup, epoch, &q, &k, batch, 12)
                    + bias.load_or(0, 12, 0.0),
                fp8_attention_score_v1(&subgroup, epoch, &q, &k, batch, 13)
                    + bias.load_or(0, 13, 0.0),
                fp8_attention_score_v1(&subgroup, epoch, &q, &k, batch, 14)
                    + bias.load_or(0, 14, 0.0),
                fp8_attention_score_v1(&subgroup, epoch, &q, &k, batch, 15)
                    + bias.load_or(0, 15, 0.0),
            )
        });
    let policy = context.numerical_policy::<StrictIeee>();
    let device_math = context.math();
    let math = device_math.with_numerical_policy(&policy);

    // Normalize the exact four-token local window independently.
    let mut local_maximum = score12;
    if score13 > local_maximum {
        local_maximum = score13;
    }
    if score14 > local_maximum {
        local_maximum = score14;
    }
    if score15 > local_maximum {
        local_maximum = score15;
    }
    let local_weight0 = math.exp_f32(score12 - local_maximum);
    let local_weight1 = math.exp_f32(score13 - local_maximum);
    let local_weight2 = math.exp_f32(score14 - local_maximum);
    let local_weight3 = math.exp_f32(score15 - local_maximum);
    let local_sum = local_weight0 + local_weight1 + local_weight2 + local_weight3;
    let value_base = batch.wrapping_mul(ATTENTION_TOKENS_V1 * CHANNELS_V1);
    #[cfg(feature = "kernel-compressed-hybrid-attention-division-baseline-v1")]
    let local_value = local_weight0 / local_sum
        * decode_fp8_e4m3_v1!(global_load_2d_u8_v1(
            &v,
            value_base,
            12,
            column,
            CHANNELS_V1
        ))
        + local_weight1 / local_sum
            * decode_fp8_e4m3_v1!(global_load_2d_u8_v1(
                &v,
                value_base,
                13,
                column,
                CHANNELS_V1
            ))
        + local_weight2 / local_sum
            * decode_fp8_e4m3_v1!(global_load_2d_u8_v1(
                &v,
                value_base,
                14,
                column,
                CHANNELS_V1
            ))
        + local_weight3 / local_sum
            * decode_fp8_e4m3_v1!(global_load_2d_u8_v1(
                &v,
                value_base,
                15,
                column,
                CHANNELS_V1
            ));
    #[cfg(not(feature = "kernel-compressed-hybrid-attention-division-baseline-v1"))]
    let local_value = {
        let reciprocal = 1.0 / local_sum;
        (local_weight0
            * decode_fp8_e4m3_v1!(global_load_2d_u8_v1(
                &v,
                value_base,
                12,
                column,
                CHANNELS_V1
            ))
            + local_weight1
                * decode_fp8_e4m3_v1!(global_load_2d_u8_v1(
                    &v,
                    value_base,
                    13,
                    column,
                    CHANNELS_V1
                ))
            + local_weight2
                * decode_fp8_e4m3_v1!(global_load_2d_u8_v1(
                    &v,
                    value_base,
                    14,
                    column,
                    CHANNELS_V1
                ))
            + local_weight3
                * decode_fp8_e4m3_v1!(global_load_2d_u8_v1(
                    &v,
                    value_base,
                    15,
                    column,
                    CHANNELS_V1
                )))
            * reciprocal
    };

    // Normalize three compressed global blocks independently from the local path.
    let mut global_maximum = score0;
    if score4 > global_maximum {
        global_maximum = score4;
    }
    if score8 > global_maximum {
        global_maximum = score8;
    }
    let global_weight0 = math.exp_f32(score0 - global_maximum);
    let global_weight1 = math.exp_f32(score4 - global_maximum);
    let global_weight2 = math.exp_f32(score8 - global_maximum);
    let global_sum = global_weight0 + global_weight1 + global_weight2;
    let compressed0 =
        (decode_fp8_e4m3_v1!(global_load_2d_u8_v1(&v, value_base, 0, column, CHANNELS_V1))
            + decode_fp8_e4m3_v1!(global_load_2d_u8_v1(&v, value_base, 1, column, CHANNELS_V1))
            + decode_fp8_e4m3_v1!(global_load_2d_u8_v1(&v, value_base, 2, column, CHANNELS_V1))
            + decode_fp8_e4m3_v1!(global_load_2d_u8_v1(&v, value_base, 3, column, CHANNELS_V1)))
            * 0.25;
    let compressed1 =
        (decode_fp8_e4m3_v1!(global_load_2d_u8_v1(&v, value_base, 4, column, CHANNELS_V1))
            + decode_fp8_e4m3_v1!(global_load_2d_u8_v1(&v, value_base, 5, column, CHANNELS_V1))
            + decode_fp8_e4m3_v1!(global_load_2d_u8_v1(&v, value_base, 6, column, CHANNELS_V1))
            + decode_fp8_e4m3_v1!(global_load_2d_u8_v1(&v, value_base, 7, column, CHANNELS_V1)))
            * 0.25;
    let compressed2 =
        (decode_fp8_e4m3_v1!(global_load_2d_u8_v1(&v, value_base, 8, column, CHANNELS_V1))
            + decode_fp8_e4m3_v1!(global_load_2d_u8_v1(&v, value_base, 9, column, CHANNELS_V1))
            + decode_fp8_e4m3_v1!(global_load_2d_u8_v1(
                &v,
                value_base,
                10,
                column,
                CHANNELS_V1
            ))
            + decode_fp8_e4m3_v1!(global_load_2d_u8_v1(
                &v,
                value_base,
                11,
                column,
                CHANNELS_V1
            )))
            * 0.25;
    #[cfg(feature = "kernel-compressed-hybrid-attention-division-baseline-v1")]
    let global_value = global_weight0 / global_sum * compressed0
        + global_weight1 / global_sum * compressed1
        + global_weight2 / global_sum * compressed2;
    #[cfg(not(feature = "kernel-compressed-hybrid-attention-division-baseline-v1"))]
    let global_value = (global_weight0 * compressed0
        + global_weight1 * compressed1
        + global_weight2 * compressed2)
        * (1.0 / global_sum);
    // A learned gate blends the paths; the first 16 lanes own the compact row.
    let mix = 1.0 / (1.0 + math.exp_f32(-score0 * 0.01));
    if wave_lane < CHANNELS_V1
        && !output.store(
            batch * CHANNELS_V1 + column,
            mix * global_value + (1.0 - mix) * local_value,
        )
    {
        fe2o3_device::trap();
    }
}

/// Softmax-aggregates four residual depths independently for each channel.
#[cfg(all(
    target_arch = "amdgpu",
    feature = "kernel-attnres-aggregate",
    not(feature = "kernel-attnres-aggregate-explicit-reuse-v1")
))]
#[kernel(
    typed,
    launch(required = [256, 1, 1], max = [256, 1, 1], max_grid = [4, 1, 1]),
    control_flow(loop_bounds(4, 4))
)]
pub fn gfx950_attnres_aggregate(
    context: KernelContext<'_>,
    depth_values: Global<'_, f32, ReadOnly>,
    depth_logits: Global<'_, f32, ReadOnly>,
    mut output: Global<'_, f32, DisjointWrite<Index1D>>,
) -> KernelResult {
    // Wave16 batches map one thread to one output channel.
    let invocation = context.invocation();
    let grid = invocation.grid_size();
    if invocation.workgroup_size().x() != 256
        || invocation.workgroup_size().y() != 1
        || invocation.workgroup_size().z() != 1
        || grid.y() != 1
        || grid.z() != 1
    {
        fe2o3_device::trap();
    }
    let Some(batches) = batch_count_for_launch_v1(grid.x(), 16) else {
        fe2o3_device::trap();
    };
    if depth_values.len() != batches * MIXING_STREAMS_V1 * CHANNELS_V1
        || depth_logits.len() != batches * MIXING_STREAMS_V1 * CHANNELS_V1
        || output.len() != batches * CHANNELS_V1
    {
        return Err(KernelError::InvalidArgument);
    }
    let index = invocation.index_1d();
    let linear = index.get();
    let batch = linear / CHANNELS_V1;
    let channel = linear % CHANNELS_V1;
    let batch_offset = batch.wrapping_mul(MIXING_STREAMS_V1 * CHANNELS_V1);
    let Some(values) = GlobalReadView2DF32V1::checked(&depth_values, batch_offset, 4, 16, 16)
    else {
        return Err(KernelError::InvalidArgument);
    };
    let Some(logits) = GlobalReadView2DF32V1::checked(&depth_logits, batch_offset, 4, 16, 16)
    else {
        return Err(KernelError::InvalidArgument);
    };
    // Use a max-subtracted four-way softmax before the weighted depth sum.
    let policy = context.numerical_policy::<StrictIeee>();
    let device_math = context.math();
    let math = device_math.with_numerical_policy(&policy);
    let mut maximum = logits.load_or(0, channel, f32::NEG_INFINITY);
    for depth in 1..4 {
        let logit = logits.load_or(depth, channel, f32::NEG_INFINITY);
        if logit > maximum {
            maximum = logit;
        }
    }
    let mut denominator = 0.0;
    let mut value = 0.0;
    for depth in 0..4 {
        let weight = math.exp_f32(logits.load_or(depth, channel, f32::NEG_INFINITY) - maximum);
        denominator += weight;
        value += weight * values.load_or(depth, channel, 0.0);
    }
    // The linear index is the exclusive owner of this channel.
    if !output.store(index.into_disjoint(), value / denominator) {
        fe2o3_device::trap();
    }
    Ok(())
}

/// Adds four sigmoid-gated branches to one 16-channel residual.
#[cfg(all(
    target_arch = "amdgpu",
    feature = "kernel-four-branch-residual",
    not(feature = "kernel-four-branch-residual-explicit-v1")
))]
#[kernel(
    typed,
    launch(required = [256, 1, 1], max = [256, 1, 1], max_grid = [4, 1, 1]),
    control_flow(loop_bounds(4))
)]
pub fn gfx950_four_branch_residual(
    context: KernelContext<'_>,
    residual: Global<'_, f32, ReadOnly>,
    branches: Global<'_, f32, ReadOnly>,
    gate_logits: Global<'_, f32, ReadOnly>,
    mut output: Global<'_, f32, DisjointWrite<Index1D>>,
) {
    // Wave16 batches map one thread to one residual channel.
    let invocation = context.invocation();
    let grid = invocation.grid_size();
    if invocation.workgroup_size().x() != 256
        || invocation.workgroup_size().y() != 1
        || invocation.workgroup_size().z() != 1
        || grid.y() != 1
        || grid.z() != 1
    {
        fe2o3_device::trap();
    }
    let Some(batches) = batch_count_for_launch_v1(grid.x(), 16) else {
        fe2o3_device::trap();
    };
    if residual.len() != batches * CHANNELS_V1
        || branches.len() != batches * MIXING_STREAMS_V1 * CHANNELS_V1
        || gate_logits.len() != batches * MIXING_STREAMS_V1 * CHANNELS_V1
        || output.len() != batches * CHANNELS_V1
    {
        return;
    }
    let index = invocation.index_1d();
    let linear = index.get();
    let batch = linear / CHANNELS_V1;
    let channel = linear % CHANNELS_V1;
    let batch_offset = batch.wrapping_mul(MIXING_STREAMS_V1 * CHANNELS_V1);
    let policy = context.numerical_policy::<StrictIeee>();
    let device_math = context.math();
    let math = device_math.with_numerical_policy(&policy);
    // Accumulate each sigmoid gate explicitly in stable branch order.
    let Some(mut value) = residual.load(batch.wrapping_mul(CHANNELS_V1).wrapping_add(channel))
    else {
        fe2o3_device::trap();
    };
    for branch in 0_usize..4 {
        let offset = batch_offset
            .wrapping_add(branch.wrapping_mul(CHANNELS_V1))
            .wrapping_add(channel);
        let (Some(logit), Some(branch_value)) = (gate_logits.load(offset), branches.load(offset))
        else {
            fe2o3_device::trap();
        };
        let gate = 1.0 / (1.0 + math.exp_f32(-logit));
        value += 0.25 * gate * branch_value;
    }
    // The linear index is the exclusive owner of this channel.
    if !output.store(index.into_disjoint(), value) {
        fe2o3_device::trap();
    }
}

/// Runs three Sinkhorn row/column normalizations and mixes four input streams.
#[cfg(all(
    target_arch = "amdgpu",
    feature = "kernel-mhc-sinkhorn-mix",
    not(feature = "kernel-mhc-sinkhorn-mix-scalar-v1")
))]
#[kernel(
    typed,
    launch(required = [256, 1, 1], max = [256, 1, 1], max_grid = [4, 1, 1]),
    control_flow(loop_bounds(3))
)]
pub fn gfx950_mhc_sinkhorn_mix(
    mut context: KernelContext<'_>,
    streams: Global<'_, f32, ReadOnly>,
    mixing_logits: Global<'_, f32, ReadOnly>,
    mut output: Global<'_, f32, DisjointWrite<Index1D>>,
) -> KernelResult {
    // One Wave64 owns one item: four rows by 16 channel lanes.
    let invocation = context.invocation();
    let grid = invocation.grid_size();
    if invocation.workgroup_size().x() != 256
        || invocation.workgroup_size().y() != 1
        || invocation.workgroup_size().z() != 1
        || grid.y() != 1
        || grid.z() != 1
    {
        fe2o3_device::trap();
    }
    let Some(batches) = batch_count_for_launch_v1(grid.x(), 64) else {
        fe2o3_device::trap();
    };
    if streams.len() != batches * MIXING_STREAMS_V1 * CHANNELS_V1
        || mixing_logits.len() != batches * MIXING_STREAMS_V1 * MIXING_STREAMS_V1
        || output.len() != batches * MIXING_STREAMS_V1 * CHANNELS_V1
    {
        return Err(KernelError::InvalidArgument);
    }
    let index = invocation.index_1d();
    let linear = index.get();
    let batch = linear / 64;
    let local = invocation.workitem_id().x() as usize % 64;
    let policy = context.numerical_policy::<StrictIeee>();
    let device_math = context.math();
    let math = device_math.with_numerical_policy(&policy);
    let Some(logits) = GlobalReadView2DF32V1::checked(
        &mixing_logits,
        batch.wrapping_mul(MIXING_STREAMS_V1 * MIXING_STREAMS_V1),
        1,
        16,
        16,
    ) else {
        return Err(KernelError::InvalidArgument);
    };
    let Some(streams) = GlobalReadView2DF32V1::checked(
        &streams,
        batch.wrapping_mul(MIXING_STREAMS_V1 * CHANNELS_V1),
        4,
        16,
        16,
    ) else {
        return Err(KernelError::InvalidArgument);
    };
    let value = context.with_workgroup(|workgroup| {
        let subgroup = workgroup.subgroup::<SubgroupWidth64>();
        let subgroup = subgroup.gfx950_wave16(workgroup.epoch());
        // Map each lane to one 4x4 mixing coefficient and its output channel.
        let row = local / CHANNELS_V1;
        let local_lane = local % CHANNELS_V1;
        let matrix_index = local_lane.wrapping_add(row.wrapping_mul(MIXING_STREAMS_V1))
            % (MIXING_STREAMS_V1 * MIXING_STREAMS_V1);
        let mut matrix = math.exp_f32(logits.load_or(0, matrix_index, 0.0));
        // Alternate row and column normalization for the fixed Sinkhorn depth.
        for _iteration in 0..SINKHORN_ITERATIONS_V1 {
            matrix *= 1.0 / partition4_reduce_sum_v1(&subgroup, local_lane as u32, matrix);

            let column = (local_lane as u32) & 3;
            let column_sum = partition16_broadcast_v1(&subgroup, matrix, column)
                + partition16_broadcast_v1(&subgroup, matrix, column + 4)
                + partition16_broadcast_v1(&subgroup, matrix, column + 8)
                + partition16_broadcast_v1(&subgroup, matrix, column + 12);
            matrix *= 1.0 / column_sum;
        }
        // Broadcast the normalized row and mix the four input streams.
        let weight0 = partition16_broadcast_v1(&subgroup, matrix, 0);
        let weight1 = partition16_broadcast_v1(&subgroup, matrix, 1);
        let weight2 = partition16_broadcast_v1(&subgroup, matrix, 2);
        let weight3 = partition16_broadcast_v1(&subgroup, matrix, 3);
        weight0 * streams.load_or(0, local_lane, 0.0)
            + weight1 * streams.load_or(1, local_lane, 0.0)
            + weight2 * streams.load_or(2, local_lane, 0.0)
            + weight3 * streams.load_or(3, local_lane, 0.0)
    });
    if !output.store(index.into_disjoint(), value) {
        fe2o3_device::trap();
    }
    Ok(())
}

#[cfg(test)]
#[path = "kernel_cfg_tests.rs"]
mod cfg_tests;

#[cfg(test)]
mod tests {
    use super::checked_2d_extent_v1;
    use crate::reference::decode_fp8_e4m3_reference_v1;

    #[test]
    fn target_fp8_e4m3_formula_matches_all_encodings() {
        for bits in u8::MIN..=u8::MAX {
            let actual = decode_fp8_e4m3_v1!(bits);
            let expected = decode_fp8_e4m3_reference_v1(bits);
            if matches!(bits, 0x7f | 0xff) {
                assert!(actual.is_nan(), "0x{bits:02x} must decode as NaN");
                assert!(expected.is_nan(), "reference 0x{bits:02x} must be NaN");
            } else {
                assert_eq!(
                    actual.to_bits(),
                    expected.to_bits(),
                    "finite E4M3FN encoding 0x{bits:02x} changed"
                );
            }
        }

        assert_eq!(decode_fp8_e4m3_v1!(0x00).to_bits(), 0.0_f32.to_bits());
        assert_eq!(decode_fp8_e4m3_v1!(0x80).to_bits(), (-0.0_f32).to_bits());
    }

    #[test]
    fn checked_2d_extents_accept_exact_tails_and_reject_overflow() {
        assert!(checked_2d_extent_v1(0, 1, 16, 16, 16));
        assert!(checked_2d_extent_v1(7, 3, 4, 8, 27));
        assert!(checked_2d_extent_v1(7, 3, 4, 8, 32));
        assert!(!checked_2d_extent_v1(7, 3, 4, 8, 26));
        assert!(!checked_2d_extent_v1(0, 0, 0, 0, 0));
        assert!(!checked_2d_extent_v1(0, 1, 5, 4, 5));
        assert!(!checked_2d_extent_v1(usize::MAX, 1, 1, 1, usize::MAX));
        assert!(!checked_2d_extent_v1(
            0,
            usize::MAX,
            1,
            usize::MAX,
            usize::MAX,
        ));
    }
}
