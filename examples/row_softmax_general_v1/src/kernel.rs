//! Safe Rust dynamic row-softmax qualification kernel.
//!
//! Review it as dynamic-contract validation, wave/row ownership, one uniform
//! max reduction, one uniform sum reduction, and capability-checked row stores.

#![allow(missing_docs)]

use fe2o3_device::{
    ExclusiveReadWrite, Global, InitialEpoch, KernelContext, KernelError, KernelResult,
    PolicyDeviceMath, ReadOnly, StrictIeee, SubgroupWidth64, WorkgroupCapability, kernel,
};

pub const ROW_SOFTMAX_WORKGROUP_V1: [u32; 3] = [64, 1, 1];
pub const ROW_SOFTMAX_MAX_COLUMNS_V1: usize = 4096;

fn accessed_extent(rows: u32, columns: u32, stride: u32) -> usize {
    if rows == 0 || columns == 0 {
        return 0;
    }
    (rows - 1) as usize * stride as usize + columns as usize
}

fn load_strided_or<Brand>(
    input: &Global<'_, f32, ReadOnly, Brand>,
    row: usize,
    column: usize,
    stride: usize,
    fallback: f32,
) -> f32 {
    row.checked_mul(stride)
        .and_then(|base| base.checked_add(column))
        .and_then(|index| input.load(index))
        .unwrap_or(fallback)
}

#[allow(clippy::too_many_arguments)]
fn normalize_row<'kernel, 'workgroup, Brand>(
    workgroup: WorkgroupCapability<'workgroup, Brand, InitialEpoch>,
    input: &Global<'kernel, f32, ReadOnly, Brand>,
    output: &mut Global<'kernel, f32, ExclusiveReadWrite, Brand>,
    math: &PolicyDeviceMath<'_, Brand, StrictIeee>,
    row: usize,
    lane: usize,
    rows: usize,
    columns: usize,
    input_stride: usize,
    output_stride: usize,
    local_max: f32,
) -> KernelResult {
    let mut maxima = workgroup.allocate_memory::<f32, 64>();
    let maximum_index = workgroup
        .memory_index_1d()
        .ok_or(KernelError::OutOfBounds)?;
    if !maxima.store(&workgroup, maximum_index.into_disjoint(), local_max) {
        return Err(KernelError::OutOfBounds);
    }
    let (workgroup, maxima) = workgroup.publish_memory(maxima);

    let mut maximum = f32::NEG_INFINITY;
    let mut source_lane = 0;
    while source_lane < 64 {
        let value = maxima
            .load(&workgroup, source_lane)
            .ok_or(KernelError::OutOfBounds)?;
        if value > maximum {
            maximum = value;
        }
        source_lane += 1;
    }

    let mut local_sum = 0.0_f32;
    let mut component = 0;
    while component < 64 {
        let column = lane + component * 64;
        let value = load_strided_or(input, row, column, input_stride, f32::NEG_INFINITY);
        local_sum += math.exp_f32(value - maximum);
        component += 1;
    }
    let subgroup = workgroup.subgroup::<SubgroupWidth64>();
    let denominator = subgroup.reduce_sum(workgroup.epoch(), local_sum);

    let mut component = 0;
    while component < 64 {
        let column = lane + component * 64;
        if row < rows && column < columns {
            let output_index = row
                .checked_mul(output_stride)
                .and_then(|base| base.checked_add(column))
                .ok_or(KernelError::OutOfBounds)?;
            let value = load_strided_or(input, row, column, input_stride, f32::NEG_INFINITY);
            if !output.store(output_index, math.exp_f32(value - maximum) / denominator) {
                return Err(KernelError::OutOfBounds);
            }
        }
        component += 1;
    }
    Ok(())
}

/// Computes independent softmax rows with dynamic dimensions and strides.
///
/// One wave owns each row. Lane `l` processes columns `l + 64 * iteration`.
/// Logical edges are zero-work lanes. Exact final-graph ownership analysis must
/// prove that the checked strided output indices are collision-free.
#[kernel(
    typed,
    launch(
        required = [64, 1, 1],
        max = [64, 1, 1],
        max_grid = [4096, 1, 1],
        static_shared_memory_bytes = 256
    ),
    control_flow(loop_bounds(64))
)]
pub fn row_softmax_general_v1(
    mut context: KernelContext<'_>,
    input: Global<'_, f32, ReadOnly>,
    mut output: Global<'_, f32, ExclusiveReadWrite>,
    rows: u32,
    columns: u32,
    input_stride: u32,
    output_stride: u32,
) -> KernelResult {
    // Prove the complete dynamic buffer contract before collective execution.
    if columns == 0
        || columns as usize > ROW_SOFTMAX_MAX_COLUMNS_V1
        || input_stride < columns
        || output_stride < columns
        || rows > u32::MAX / 64
        || input.len() < accessed_extent(rows, columns, input_stride)
        || output.len() < accessed_extent(rows, columns, output_stride)
    {
        return Err(KernelError::InvalidArgument);
    }
    // One Wave64 owns one row; each lane owns a 64-column stripe.
    let thread_index = context.invocation().index_1d();
    let raw = thread_index.get();
    let row = raw / 64;
    let lane = raw % 64;
    let math = context.math();
    let policy = context.numerical_policy::<StrictIeee>();
    let math = math.with_numerical_policy(&policy);

    // Max subtraction keeps the exponentials finite for large logits.
    let mut local_max = f32::NEG_INFINITY;
    let mut component = 0;
    while component < 64 {
        let column = lane + component * 64;
        let value = load_strided_or(
            &input,
            row,
            column,
            input_stride as usize,
            f32::NEG_INFINITY,
        );
        if value > local_max {
            local_max = value;
        }
        component += 1;
    }
    context.with_workgroup(|workgroup| {
        normalize_row(
            workgroup,
            &input,
            &mut output,
            &math,
            row,
            lane,
            rows as usize,
            columns as usize,
            input_stride as usize,
            output_stride as usize,
            local_max,
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accessed_extent_excludes_trailing_padding() {
        assert_eq!(accessed_extent(3, 5, 8), 21);
        assert_eq!(accessed_extent(0, 5, 8), 0);
    }
}
