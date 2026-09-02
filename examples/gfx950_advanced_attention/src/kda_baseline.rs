//! Independent 256-thread matrix-state KDA roots for measured ablations.

use fe2o3_device::{kernel, thread, DisjointSlice, Gfx950Subgroup, Index1D, StridedReadView2D};

use crate::{
    KDA_KEY_DIMENSION_V1, KDA_STATE_ELEMENTS_V1, KDA_VALUE_DIMENSION_V1, PREFILL_TOKENS_V1,
};

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
    namespace = "160b57240e4d405563c3dd402992eb50ac0b1192c795954e6853d2fe08b4dd09",
    launch(required = [256, 1, 1], max = [256, 1, 1], max_grid = [1, 1, 1])
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
    if query.len() != KDA_KEY_DIMENSION_V1
        || key.len() != KDA_KEY_DIMENSION_V1
        || value.len() != KDA_VALUE_DIMENSION_V1
        || alpha.len() != KDA_KEY_DIMENSION_V1
        || beta.len() != 1
        || initial_state.len() != KDA_STATE_ELEMENTS_V1
        || final_state.len() != KDA_STATE_ELEMENTS_V1
        || output.len() != KDA_STATE_ELEMENTS_V1
    {
        return;
    }
    let Ok(query) = StridedReadView2D::from_shared_slice(query, 0, 1, 16, 16) else {
        return;
    };
    let Ok(key) = StridedReadView2D::from_shared_slice(key, 0, 1, 16, 16) else {
        return;
    };
    let Ok(value) = StridedReadView2D::from_shared_slice(value, 0, 1, 16, 16) else {
        return;
    };
    let Ok(alpha) = StridedReadView2D::from_shared_slice(alpha, 0, 1, 16, 16) else {
        return;
    };
    let Ok(beta) = StridedReadView2D::from_shared_slice(beta, 0, 1, 1, 1) else {
        return;
    };
    let Ok(state) = StridedReadView2D::from_shared_slice(initial_state, 0, 16, 16, 16) else {
        return;
    };
    let linear = thread::index_1d().get();
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
    namespace = "083c8464c05f4af00df5503e2a5905f65e7b865610f441f9eca4a3c7e556efa6",
    launch(required = [256, 1, 1], max = [256, 1, 1], max_grid = [1, 1, 1])
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
    if query.len() != PREFILL_TOKENS_V1 * KDA_KEY_DIMENSION_V1
        || key.len() != PREFILL_TOKENS_V1 * KDA_KEY_DIMENSION_V1
        || value.len() != PREFILL_TOKENS_V1 * KDA_VALUE_DIMENSION_V1
        || alpha.len() != PREFILL_TOKENS_V1 * KDA_KEY_DIMENSION_V1
        || beta.len() != PREFILL_TOKENS_V1
        || initial_state.len() != KDA_STATE_ELEMENTS_V1
        || final_state.len() != KDA_STATE_ELEMENTS_V1
        || output_chunk0.len() != KDA_STATE_ELEMENTS_V1
        || output_chunk1.len() != KDA_STATE_ELEMENTS_V1
    {
        return;
    }
    let Ok(query) = StridedReadView2D::from_shared_slice(query, 0, 8, 16, 16) else {
        return;
    };
    let Ok(key) = StridedReadView2D::from_shared_slice(key, 0, 8, 16, 16) else {
        return;
    };
    let Ok(value) = StridedReadView2D::from_shared_slice(value, 0, 8, 16, 16) else {
        return;
    };
    let Ok(alpha) = StridedReadView2D::from_shared_slice(alpha, 0, 8, 16, 16) else {
        return;
    };
    let Ok(beta) = StridedReadView2D::from_shared_slice(beta, 0, 1, 8, 8) else {
        return;
    };
    let Ok(initial_state) = StridedReadView2D::from_shared_slice(initial_state, 0, 16, 16, 16)
    else {
        return;
    };
    let linear = thread::index_1d().get();
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
