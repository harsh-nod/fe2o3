//! Exact-semantics gfx950 attention ablations selected one feature at a time.
//!
//! Each variant preserves the production input, ownership, numerical, and
//! output contracts while changing one named implementation choice. This
//! single-variable rule keeps an ablation interpretable and reviewable.

use fe2o3_device::{
    DisjointWrite, Global, Index1D, KernelContext, KernelError, KernelResult, ReadOnly, StrictIeee,
    kernel,
};

use crate::kernel::GlobalReadView2DF32V1;
use crate::{
    ATTENTION_TOKENS_V1, CHANNELS_V1, HEAD_DIMENSION_V1, MIXING_STREAMS_V1, SELECTED_TOKENS_V1,
    batch_count_for_launch_v1,
};

/// The scalar selected-score attention experiments are retained in the ablation manifest.
/// The V1 control-flow sidecar rejects their bounded loop plus selection macro.

/// Hoists the four logits and weights so each is loaded and evaluated once.
#[cfg(feature = "kernel-attnres-aggregate-explicit-reuse-v1")]
#[kernel(
    typed,
    launch(required = [256, 1, 1], max = [256, 1, 1], max_grid = [4, 1, 1])
)]
pub fn gfx950_attnres_aggregate(
    context: KernelContext<'_>,
    depth_values: Global<'_, f32, ReadOnly>,
    depth_logits: Global<'_, f32, ReadOnly>,
    mut output: Global<'_, f32, DisjointWrite<Index1D>>,
) -> KernelResult {
    // Preserve production shape validation so only reuse strategy changes.
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
    // One thread owns one (batch, channel) result.
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
    // Hoist each logit and exponential exactly once for this reuse experiment.
    let policy = context.numerical_policy::<StrictIeee>();
    let device_math = context.math();
    let math = device_math.with_numerical_policy(&policy);
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
    // Publish through the same disjoint output capability as production.
    if !output.store(index.into_disjoint(), value / denominator) {
        fe2o3_device::trap();
    }
    Ok(())
}

/// Makes the four fixed branches explicit to test loop-unrolling effects.
#[cfg(feature = "kernel-four-branch-residual-explicit-v1")]
#[kernel(
    typed,
    launch(required = [256, 1, 1], max = [256, 1, 1], max_grid = [4, 1, 1])
)]
pub fn gfx950_four_branch_residual(
    context: KernelContext<'_>,
    residual: Global<'_, f32, ReadOnly>,
    branches: Global<'_, f32, ReadOnly>,
    gate_logits: Global<'_, f32, ReadOnly>,
    mut output: Global<'_, f32, DisjointWrite<Index1D>>,
) {
    // Preserve production validation and ownership while making branches explicit.
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
    // Evaluate four fixed gates without changing their accumulation order.
    let policy = context.numerical_policy::<StrictIeee>();
    let device_math = context.math();
    let math = device_math.with_numerical_policy(&policy);
    let branch0 = batch_offset.wrapping_add(channel);
    let offset1 = batch_offset.wrapping_add(CHANNELS_V1).wrapping_add(channel);
    let offset2 = batch_offset
        .wrapping_add(2 * CHANNELS_V1)
        .wrapping_add(channel);
    let offset3 = batch_offset
        .wrapping_add(3 * CHANNELS_V1)
        .wrapping_add(channel);
    let (Some(logit0), Some(logit1), Some(logit2), Some(logit3)) = (
        gate_logits.load(branch0),
        gate_logits.load(offset1),
        gate_logits.load(offset2),
        gate_logits.load(offset3),
    ) else {
        fe2o3_device::trap();
    };
    let (Some(branch_value0), Some(branch_value1), Some(branch_value2), Some(branch_value3)) = (
        branches.load(branch0),
        branches.load(offset1),
        branches.load(offset2),
        branches.load(offset3),
    ) else {
        fe2o3_device::trap();
    };
    let Some(residual_value) = residual.load(batch.wrapping_mul(CHANNELS_V1).wrapping_add(channel))
    else {
        fe2o3_device::trap();
    };
    let gate0 = 1.0 / (1.0 + math.exp_f32(-logit0));
    let gate1 = 1.0 / (1.0 + math.exp_f32(-logit1));
    let gate2 = 1.0 / (1.0 + math.exp_f32(-logit2));
    let gate3 = 1.0 / (1.0 + math.exp_f32(-logit3));
    let value = residual_value
        + 0.25 * gate0 * branch_value0
        + 0.25 * gate1 * branch_value1
        + 0.25 * gate2 * branch_value2
        + 0.25 * gate3 * branch_value3;
    // One linear thread index owns one channel store.
    if !output.store(index.into_disjoint(), value) {
        fe2o3_device::trap();
    }
}

/// Retains the pre-wave16 scalar Sinkhorn implementation as an exact baseline.
#[cfg(feature = "kernel-mhc-sinkhorn-mix-scalar-v1")]
#[kernel(
    typed,
    launch(required = [256, 1, 1], max = [256, 1, 1], max_grid = [4, 1, 1]),
    control_flow(loop_bounds(3))
)]
pub fn gfx950_mhc_sinkhorn_mix(
    context: KernelContext<'_>,
    streams: Global<'_, f32, ReadOnly>,
    mixing_logits: Global<'_, f32, ReadOnly>,
    mut output: Global<'_, f32, DisjointWrite<Index1D>>,
) -> KernelResult {
    // Preserve the WG256/grid4 contract while replacing subgroup Sinkhorn.
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
    // Materialize the 4x4 matrix so row and column normalization is explicit.
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
    // Run the same fixed three Sinkhorn iterations as the production kernel.
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
    let row = local / CHANNELS_V1;
    let channel = local % CHANNELS_V1;
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
    // Each lane publishes its one stream/channel result exactly once.
    if !output.store(index.into_disjoint(), value) {
        fe2o3_device::trap();
    }
    Ok(())
}
