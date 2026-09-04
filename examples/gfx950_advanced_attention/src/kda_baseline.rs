//! Independent 256-thread matrix-state KDA roots for measured ablations.

use fe2o3_device::{DisjointSlice, Gfx950Subgroup, Index1D, StridedReadView2D, kernel, thread};

use crate::{
    KDA_KEY_DIMENSION_V1, KDA_STATE_ELEMENTS_V1, KDA_VALUE_DIMENSION_V1, PREFILL_TOKENS_V1,
};

const MULTIGRID_WORKGROUPS_V1: usize = 4;

macro_rules! kda_recurrent_step_baseline_v1 {
    ($token:expr, $query:ident, $key:ident, $value:ident, $alpha:ident, $beta:ident,
     $subgroup:ident, $key_index:ident, $value_column:ident, $state:ident, $output:ident) => {{
        let token = $token;
        let key_value = $key.load_or(token, $key_index, 0.0);
        let decay = $alpha.load_or(token, $key_index, 0.0) * $state;
        let prediction = $subgroup.reduce_sum_f32::<16>(key_value * decay);
        let error = $value.load_or(token, $value_column, 0.0) - prediction;
        $state = decay + $beta.load_or(0, token, 0.0) * key_value * error;
        $output =
            $subgroup.reduce_sum_f32::<16>(0.25 * $query.load_or(token, $key_index, 0.0) * $state);
    }};
}

#[cfg(feature = "kernel-kda-decode-baseline-v1")]
#[kernel(
    typed,
    launch(required = [256, 1, 1], max = [256, 1, 1], max_grid = [4, 1, 1])
)]
pub fn gfx950_kda_decode(
    query: &[f32],
    key: &[f32],
    value: &[f32],
    alpha: &[f32],
    beta: &[f32],
    initial_state: &[f32],
    mut final_state: DisjointSlice<f32, Index1D>,
    mut output: DisjointSlice<f32, Index1D>,
) {
    let batches = MULTIGRID_WORKGROUPS_V1;
    let batch = thread::block_idx_x() as usize;
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
    let Ok(query) = StridedReadView2D::from_shared_slice(
        query,
        batch.wrapping_mul(KDA_KEY_DIMENSION_V1),
        1,
        16,
        16,
    ) else {
        return;
    };
    let Ok(key) = StridedReadView2D::from_shared_slice(
        key,
        batch.wrapping_mul(KDA_KEY_DIMENSION_V1),
        1,
        16,
        16,
    ) else {
        return;
    };
    let Ok(value) = StridedReadView2D::from_shared_slice(
        value,
        batch.wrapping_mul(KDA_VALUE_DIMENSION_V1),
        1,
        16,
        16,
    ) else {
        return;
    };
    let Ok(alpha) = StridedReadView2D::from_shared_slice(
        alpha,
        batch.wrapping_mul(KDA_KEY_DIMENSION_V1),
        1,
        16,
        16,
    ) else {
        return;
    };
    let Ok(beta) = StridedReadView2D::from_shared_slice(beta, batch, 1, 1, 1) else {
        return;
    };
    let Ok(state) = StridedReadView2D::from_shared_slice(
        initial_state,
        batch.wrapping_mul(KDA_STATE_ELEMENTS_V1),
        16,
        16,
        16,
    ) else {
        return;
    };
    let linear = thread::thread_idx_x() as usize;
    let key_index = linear & 15;
    let value_column = linear >> 4;
    let subgroup = Gfx950Subgroup::current();
    let decay = alpha.load_or(0, key_index, 0.0) * state.load_or(value_column, key_index, 0.0);
    let prediction = subgroup.reduce_sum_f32::<16>(key.load_or(0, key_index, 0.0) * decay);
    let error = value.load_or(0, value_column, 0.0) - prediction;
    let step = beta.load_or(0, 0, 0.0);
    let updated = decay + step * key.load_or(0, key_index, 0.0) * error;
    let result = subgroup.reduce_sum_f32::<16>(0.25 * query.load_or(0, key_index, 0.0) * updated);
    if let Some(slot) = final_state.get_mut(thread::index_1d()) {
        *slot = updated;
    }
    if let Some(slot) = output.get_mut(thread::index_1d()) {
        *slot = result;
    }
}

#[cfg(feature = "kernel-kda-prefill-baseline-v1")]
#[kernel(
    typed,
    launch(required = [256, 1, 1], max = [256, 1, 1], max_grid = [4, 1, 1])
)]
pub fn gfx950_kda_chunkwise_prefill(
    query: &[f32],
    key: &[f32],
    value: &[f32],
    alpha: &[f32],
    beta: &[f32],
    initial_state: &[f32],
    mut final_state: DisjointSlice<f32, Index1D>,
    mut output_chunk0: DisjointSlice<f32, Index1D>,
    mut output_chunk1: DisjointSlice<f32, Index1D>,
) {
    let batches = MULTIGRID_WORKGROUPS_V1;
    let batch = thread::block_idx_x() as usize;
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
    let token_base = batch.wrapping_mul(PREFILL_TOKENS_V1);
    let Ok(query) = StridedReadView2D::from_shared_slice(
        query,
        token_base.wrapping_mul(KDA_KEY_DIMENSION_V1),
        8,
        16,
        16,
    ) else {
        return;
    };
    let Ok(key) = StridedReadView2D::from_shared_slice(
        key,
        token_base.wrapping_mul(KDA_KEY_DIMENSION_V1),
        8,
        16,
        16,
    ) else {
        return;
    };
    let Ok(value) = StridedReadView2D::from_shared_slice(
        value,
        token_base.wrapping_mul(KDA_VALUE_DIMENSION_V1),
        8,
        16,
        16,
    ) else {
        return;
    };
    let Ok(alpha) = StridedReadView2D::from_shared_slice(
        alpha,
        token_base.wrapping_mul(KDA_KEY_DIMENSION_V1),
        8,
        16,
        16,
    ) else {
        return;
    };
    let Ok(beta) = StridedReadView2D::from_shared_slice(beta, token_base, 1, 8, 8) else {
        return;
    };
    let Ok(initial_state) = StridedReadView2D::from_shared_slice(
        initial_state,
        batch.wrapping_mul(KDA_STATE_ELEMENTS_V1),
        16,
        16,
        16,
    ) else {
        return;
    };
    let linear = thread::thread_idx_x() as usize;
    let key_index = linear & 15;
    let value_column = linear >> 4;
    let subgroup = Gfx950Subgroup::current();
    let mut state = initial_state.load_or(value_column, key_index, 0.0);
    let mut c00 = 0.0;
    let mut c01 = 0.0;
    let mut c02 = 0.0;
    let mut c03 = 0.0;
    let mut c10 = 0.0;
    let mut c11 = 0.0;
    let mut c12 = 0.0;
    let mut c13 = 0.0;
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
    if let Some(slot) = output_chunk0.get_mut(thread::index_1d()) {
        *slot = selected0;
    }
    if let Some(slot) = output_chunk1.get_mut(thread::index_1d()) {
        *slot = selected1;
    }
    if let Some(slot) = final_state.get_mut(thread::index_1d()) {
        *slot = state;
    }
}
