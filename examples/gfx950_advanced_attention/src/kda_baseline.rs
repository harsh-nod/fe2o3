//! Independent 256-thread matrix-state KDA roots for measured ablations.
//!
//! These baselines preserve launch shape, tensor layout, state ordering, and
//! output ownership. Only the recurrent formulation changes, so comparisons
//! isolate chunkwise WY/UT transformation rather than unrelated behavior.

use fe2o3_device::{
    DisjointWrite, Global, Index1D, KernelContext, ReadOnly, SubgroupWidth64, kernel,
};

use crate::kernel::{GlobalReadView2DF32V1, partition16_reduce_sum_v1};
use crate::{
    KDA_KEY_DIMENSION_V1, KDA_STATE_ELEMENTS_V1, KDA_VALUE_DIMENSION_V1, PREFILL_TOKENS_V1,
    batch_count_for_launch_v1,
};

macro_rules! kda_recurrent_step_baseline_v1 {
    ($token:expr, $query:ident, $key:ident, $value:ident, $alpha:ident, $beta:ident,
     $subgroup:ident, $key_index:ident, $value_column:ident, $state:ident,
     $output:ident) => {{
        // Predict, correct, then query the updated matrix state for one token.
        let token = $token;
        let key_value = $key.load_or(token, $key_index, 0.0);
        let decay = $alpha.load_or(token, $key_index, 0.0) * $state;
        let prediction = partition16_reduce_sum_v1(&$subgroup, key_value * decay);
        let error = $value.load_or(token, $value_column, 0.0) - prediction;
        $state = decay + $beta.load_or(0, token, 0.0) * key_value * error;
        $output = partition16_reduce_sum_v1(
            &$subgroup,
            0.25 * $query.load_or(token, $key_index, 0.0) * $state,
        );
    }};
}

#[cfg(feature = "kernel-kda-decode-baseline-v1")]
#[kernel(
    typed,
    launch(required = [256, 1, 1], max = [256, 1, 1], max_grid = [4, 1, 1])
)]
/// Evaluates one KDA decode token with the scalar recurrent formulation.
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
    // One workgroup owns one complete 16x16 state matrix.
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
    // Checked views make every batch offset and tensor stride explicit.
    let Some(query) =
        GlobalReadView2DF32V1::checked(&query, batch.wrapping_mul(KDA_KEY_DIMENSION_V1), 1, 16, 16)
    else {
        return;
    };
    let Some(key) =
        GlobalReadView2DF32V1::checked(&key, batch.wrapping_mul(KDA_KEY_DIMENSION_V1), 1, 16, 16)
    else {
        return;
    };
    let Some(value) = GlobalReadView2DF32V1::checked(
        &value,
        batch.wrapping_mul(KDA_VALUE_DIMENSION_V1),
        1,
        16,
        16,
    ) else {
        return;
    };
    let Some(alpha) =
        GlobalReadView2DF32V1::checked(&alpha, batch.wrapping_mul(KDA_KEY_DIMENSION_V1), 1, 16, 16)
    else {
        return;
    };
    let Some(beta) = GlobalReadView2DF32V1::checked(&beta, batch, 1, 1, 1) else {
        return;
    };
    let Some(state) = GlobalReadView2DF32V1::checked(
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
    let decay = alpha.load_or(0, key_index, 0.0) * state.load_or(value_column, key_index, 0.0);
    // Narrow compiler-issued Wave64 authority to exact gfx950 Wave16 collectives.
    let (updated, result) = context.with_workgroup(|workgroup| {
        let subgroup = workgroup.subgroup::<SubgroupWidth64>();
        let subgroup = subgroup.gfx950_wave16(workgroup.epoch());
        let prediction =
            partition16_reduce_sum_v1(&subgroup, key.load_or(0, key_index, 0.0) * decay);
        let error = value.load_or(0, value_column, 0.0) - prediction;
        let step = beta.load_or(0, 0, 0.0);
        let updated = decay + step * key.load_or(0, key_index, 0.0) * error;
        let result =
            partition16_reduce_sum_v1(&subgroup, 0.25 * query.load_or(0, key_index, 0.0) * updated);
        (updated, result)
    });
    // Each thread publishes one state element and one replicated output element.
    if !final_state.store(context.invocation().index_1d().into_disjoint(), updated)
        || !output.store(context.invocation().index_1d().into_disjoint(), result)
    {
        fe2o3_device::trap();
    }
}

#[cfg(feature = "kernel-kda-prefill-baseline-v1")]
#[kernel(
    typed,
    launch(required = [256, 1, 1], max = [256, 1, 1], max_grid = [4, 1, 1])
)]
/// Evaluates eight KDA prefill tokens as an ordered recurrent baseline.
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
    // One workgroup owns one eight-token sequence and its complete state.
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
    // Derive checked token-major views for this workgroup's sequence.
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
        let mut c10 = 0.0;
        let mut c11 = 0.0;
        let mut c12 = 0.0;
        let mut c13 = 0.0;
        // Advance tokens in order; capture outputs at the two chunk boundaries.
        kda_recurrent_step_baseline_v1!(
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
            c00
        );
        kda_recurrent_step_baseline_v1!(
            1,
            query,
            key,
            value,
            alpha,
            beta,
            subgroup,
            key_index,
            value_column,
            state,
            c01
        );
        kda_recurrent_step_baseline_v1!(
            2,
            query,
            key,
            value,
            alpha,
            beta,
            subgroup,
            key_index,
            value_column,
            state,
            c02
        );
        kda_recurrent_step_baseline_v1!(
            3,
            query,
            key,
            value,
            alpha,
            beta,
            subgroup,
            key_index,
            value_column,
            state,
            c03
        );
        kda_recurrent_step_baseline_v1!(
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
            c10
        );
        kda_recurrent_step_baseline_v1!(
            5,
            query,
            key,
            value,
            alpha,
            beta,
            subgroup,
            key_index,
            value_column,
            state,
            c11
        );
        kda_recurrent_step_baseline_v1!(
            6,
            query,
            key,
            value,
            alpha,
            beta,
            subgroup,
            key_index,
            value_column,
            state,
            c12
        );
        kda_recurrent_step_baseline_v1!(
            7,
            query,
            key,
            value,
            alpha,
            beta,
            subgroup,
            key_index,
            value_column,
            state,
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
