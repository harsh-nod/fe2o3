//! Exact-semantics gfx950 attention ablations selected one feature at a time.

use fe2o3_device::{
    DeviceMath, DisjointSlice, Index1D, KernelError, KernelResult, StridedReadView2D, kernel,
    thread,
};

use crate::{
    ATTENTION_TOKENS_V1, CHANNELS_V1, HEAD_DIMENSION_V1, MIXING_STREAMS_V1, SELECTED_TOKENS_V1,
};

const ATTENTION_SCALE_V1: f32 = 0.088_388_346;

/// The scalar selected-score attention experiments are retained in the ablation manifest.
/// The V1 control-flow sidecar rejects their bounded loop plus selection macro.

/// Hoists the four logits and weights so each is loaded and evaluated once.
#[cfg(feature = "kernel-attnres-aggregate-explicit-reuse-v1")]
#[kernel(
    typed,
    namespace = "fe6d11c689feb27ace6afe63785978ffefe8668f0b93fa0e33b50e5185b6fb43",
    launch(required = [64, 1, 1], max = [64, 1, 1], max_grid = [1, 1, 1])
)]
pub fn gfx950_attnres_aggregate(
    depth_values: &[f32],
    depth_logits: &[f32],
    mut output: DisjointSlice<f32, Index1D>,
) -> KernelResult {
    if depth_values.len() != MIXING_STREAMS_V1 * CHANNELS_V1
        || depth_logits.len() != MIXING_STREAMS_V1 * CHANNELS_V1
        || output.len() != CHANNELS_V1
    {
        return Err(KernelError::InvalidArgument);
    }
    let index = thread::index_1d();
    let channel = index.get();
    if channel >= CHANNELS_V1 {
        return Ok(());
    }
    let Ok(values) = StridedReadView2D::from_shared_slice(depth_values, 0, 4, 16, 16) else {
        return Err(KernelError::InvalidArgument);
    };
    let Ok(logits) = StridedReadView2D::from_shared_slice(depth_logits, 0, 4, 16, 16) else {
        return Err(KernelError::InvalidArgument);
    };
    let math = DeviceMath::current();
    let logit0 = logits.load_or(0, channel, f32::NEG_INFINITY);
    let logit1 = logits.load_or(1, channel, f32::NEG_INFINITY);
    let logit2 = logits.load_or(2, channel, f32::NEG_INFINITY);
    let logit3 = logits.load_or(3, channel, f32::NEG_INFINITY);
    let mut maximum = logit0;
    if logit1 > maximum {
        maximum = logit1;
    }
    if logit2 > maximum {
        maximum = logit2;
    }
    if logit3 > maximum {
        maximum = logit3;
    }
    let weight0 = math.exp_f32(logit0 - maximum);
    let weight1 = math.exp_f32(logit1 - maximum);
    let weight2 = math.exp_f32(logit2 - maximum);
    let weight3 = math.exp_f32(logit3 - maximum);
    let denominator = ((weight0 + weight1) + weight2) + weight3;
    let value = weight0 * values.load_or(0, channel, 0.0)
        + weight1 * values.load_or(1, channel, 0.0)
        + weight2 * values.load_or(2, channel, 0.0)
        + weight3 * values.load_or(3, channel, 0.0);
    if let Some(slot) = output.get_mut(thread::index_1d()) {
        *slot = value / denominator;
    }
    Ok(())
}

/// Makes the four fixed branches explicit to test loop-unrolling effects.
#[cfg(feature = "kernel-four-branch-residual-explicit-v1")]
#[kernel(
    typed,
    namespace = "5972789e1c05e3508b65dd3ce977460423b63b30b306e0a5d82ff4003d8b4d67",
    launch(required = [64, 1, 1], max = [64, 1, 1], max_grid = [1, 1, 1])
)]
pub fn gfx950_four_branch_residual(
    residual: &[f32],
    branches: &[f32],
    gate_logits: &[f32],
    mut output: DisjointSlice<f32, Index1D>,
) {
    if residual.len() != CHANNELS_V1
        || branches.len() != MIXING_STREAMS_V1 * CHANNELS_V1
        || gate_logits.len() != MIXING_STREAMS_V1 * CHANNELS_V1
        || output.len() != CHANNELS_V1
    {
        return;
    }
    let index = thread::index_1d();
    let channel = index.get();
    if channel >= CHANNELS_V1 {
        return;
    }
    let math = DeviceMath::current();
    let offset1 = CHANNELS_V1.wrapping_add(channel);
    let offset2 = (2 * CHANNELS_V1).wrapping_add(channel);
    let offset3 = (3 * CHANNELS_V1).wrapping_add(channel);
    let gate0 = 1.0 / (1.0 + math.exp_f32(-gate_logits[channel]));
    let gate1 = 1.0 / (1.0 + math.exp_f32(-gate_logits[offset1]));
    let gate2 = 1.0 / (1.0 + math.exp_f32(-gate_logits[offset2]));
    let gate3 = 1.0 / (1.0 + math.exp_f32(-gate_logits[offset3]));
    let value = residual[channel]
        + 0.25 * gate0 * branches[channel]
        + 0.25 * gate1 * branches[offset1]
        + 0.25 * gate2 * branches[offset2]
        + 0.25 * gate3 * branches[offset3];
    if let Some(slot) = output.get_mut(thread::index_1d()) {
        *slot = value;
    }
}

/// Retains the pre-wave16 scalar Sinkhorn implementation as an exact baseline.
#[cfg(feature = "kernel-mhc-sinkhorn-mix-scalar-v1")]
#[kernel(
    typed,
    namespace = "0e2a561e71ced26b05bcaf0287320b4e1969b9909709417dfafb4299ecc6eb92",
    launch(required = [64, 1, 1], max = [64, 1, 1], max_grid = [1, 1, 1]),
    control_flow(loop_bounds(3))
)]
pub fn gfx950_mhc_sinkhorn_mix(
    streams: &[f32],
    mixing_logits: &[f32],
    mut output: DisjointSlice<f32, Index1D>,
) -> KernelResult {
    if streams.len() != MIXING_STREAMS_V1 * CHANNELS_V1
        || mixing_logits.len() != MIXING_STREAMS_V1 * MIXING_STREAMS_V1
        || output.len() != MIXING_STREAMS_V1 * CHANNELS_V1
    {
        return Err(KernelError::InvalidArgument);
    }
    let index = thread::index_1d();
    let linear = index.get();
    if linear >= MIXING_STREAMS_V1 * CHANNELS_V1 {
        return Ok(());
    }
    let math = DeviceMath::current();
    let Ok(logits) = StridedReadView2D::from_shared_slice(mixing_logits, 0, 1, 16, 16) else {
        return Err(KernelError::InvalidArgument);
    };
    let Ok(streams) = StridedReadView2D::from_shared_slice(streams, 0, 4, 16, 16) else {
        return Err(KernelError::InvalidArgument);
    };
    let mut m00 = math.exp_f32(logits.load_or(0, 0, 0.0));
    let mut m01 = math.exp_f32(logits.load_or(0, 1, 0.0));
    let mut m02 = math.exp_f32(logits.load_or(0, 2, 0.0));
    let mut m03 = math.exp_f32(logits.load_or(0, 3, 0.0));
    let mut m10 = math.exp_f32(logits.load_or(0, 4, 0.0));
    let mut m11 = math.exp_f32(logits.load_or(0, 5, 0.0));
    let mut m12 = math.exp_f32(logits.load_or(0, 6, 0.0));
    let mut m13 = math.exp_f32(logits.load_or(0, 7, 0.0));
    let mut m20 = math.exp_f32(logits.load_or(0, 8, 0.0));
    let mut m21 = math.exp_f32(logits.load_or(0, 9, 0.0));
    let mut m22 = math.exp_f32(logits.load_or(0, 10, 0.0));
    let mut m23 = math.exp_f32(logits.load_or(0, 11, 0.0));
    let mut m30 = math.exp_f32(logits.load_or(0, 12, 0.0));
    let mut m31 = math.exp_f32(logits.load_or(0, 13, 0.0));
    let mut m32 = math.exp_f32(logits.load_or(0, 14, 0.0));
    let mut m33 = math.exp_f32(logits.load_or(0, 15, 0.0));
    for _iteration in 0..3 {
        let row0 = m00 + m01 + m02 + m03;
        m00 /= row0;
        m01 /= row0;
        m02 /= row0;
        m03 /= row0;
        let row1 = m10 + m11 + m12 + m13;
        m10 /= row1;
        m11 /= row1;
        m12 /= row1;
        m13 /= row1;
        let row2 = m20 + m21 + m22 + m23;
        m20 /= row2;
        m21 /= row2;
        m22 /= row2;
        m23 /= row2;
        let row3 = m30 + m31 + m32 + m33;
        m30 /= row3;
        m31 /= row3;
        m32 /= row3;
        m33 /= row3;
        let column0 = m00 + m10 + m20 + m30;
        m00 /= column0;
        m10 /= column0;
        m20 /= column0;
        m30 /= column0;
        let column1 = m01 + m11 + m21 + m31;
        m01 /= column1;
        m11 /= column1;
        m21 /= column1;
        m31 /= column1;
        let column2 = m02 + m12 + m22 + m32;
        m02 /= column2;
        m12 /= column2;
        m22 /= column2;
        m32 /= column2;
        let column3 = m03 + m13 + m23 + m33;
        m03 /= column3;
        m13 /= column3;
        m23 /= column3;
        m33 /= column3;
    }
    let row = linear / CHANNELS_V1;
    let channel = linear % CHANNELS_V1;
    let value = if row == 0 {
        m00 * streams.load_or(0, channel, 0.0)
            + m01 * streams.load_or(1, channel, 0.0)
            + m02 * streams.load_or(2, channel, 0.0)
            + m03 * streams.load_or(3, channel, 0.0)
    } else if row == 1 {
        m10 * streams.load_or(0, channel, 0.0)
            + m11 * streams.load_or(1, channel, 0.0)
            + m12 * streams.load_or(2, channel, 0.0)
            + m13 * streams.load_or(3, channel, 0.0)
    } else if row == 2 {
        m20 * streams.load_or(0, channel, 0.0)
            + m21 * streams.load_or(1, channel, 0.0)
            + m22 * streams.load_or(2, channel, 0.0)
            + m23 * streams.load_or(3, channel, 0.0)
    } else {
        m30 * streams.load_or(0, channel, 0.0)
            + m31 * streams.load_or(1, channel, 0.0)
            + m32 * streams.load_or(2, channel, 0.0)
            + m33 * streams.load_or(3, channel, 0.0)
    };
    if let Some(slot) = output.get_mut(thread::index_1d()) {
        *slot = value;
    }
    Ok(())
}
