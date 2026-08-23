//! Safe Rust dynamic row-softmax qualification kernel.

#![allow(missing_docs)]

use fe2o3_device::{
    DeviceMath, DisjointSlice, Gfx942Collectives, Index1D, Tiled2D, kernel, thread,
};

pub const ROW_SOFTMAX_WORKGROUP_V1: [u32; 3] = [64, 1, 1];
pub const ROW_SOFTMAX_MAX_COLUMNS_V1: usize = 4096;

fn accessed_extent(rows: u32, stride: u32) -> usize {
    if rows == 0 {
        return 0;
    }
    (rows - 1) as usize * stride as usize + ROW_SOFTMAX_MAX_COLUMNS_V1
}

/// Computes independent softmax rows with dynamic dimensions and input stride.
///
/// One wave owns each row. Lane `l` processes columns `l + 64 * iteration`.
/// Each input row has 4,096 readable elements; columns at or above `columns`
/// must be negative infinity. The output uses the same padded row width, which
/// lets `Tiled2D` prove every store disjoint without unsafe source code.
#[kernel(
    typed,
    namespace = "bf0896973a495f4042b006a1fd5f2649c81ac32b5e32228c6a88591c046a35e7",
    launch(required = [64, 1, 1], max = [64, 1, 1]),
    control_flow(loop_bounds(64, 64, 64))
)]
pub fn row_softmax_general_v1(
    input: &[f32],
    mut output: DisjointSlice<f32, Tiled2D<Index1D, 64, 64, 64, 64>>,
    rows: u32,
    columns: u32,
    input_stride: u32,
    output_rows: u32,
) {
    if columns == 0
        || columns as usize > ROW_SOFTMAX_MAX_COLUMNS_V1
        || (input_stride as usize) < ROW_SOFTMAX_MAX_COLUMNS_V1
        || rows > u32::MAX / 64
        || output_rows != rows * 64
        || input.len() < accessed_extent(rows, input_stride)
        || output.len() < output_rows as usize * 64
    {
        return;
    }

    let thread_index = thread::index_1d();
    let raw = thread_index.get();
    let row = raw / 64;
    let lane = raw % 64;
    let Some(output_tile) = thread_index.checked_tiled_2d::<64, 64, 64, 64>() else {
        return;
    };

    let collectives = Gfx942Collectives::current();
    let math = DeviceMath::current();
    let row_base = row * input_stride as usize;

    let mut local_max = f32::NEG_INFINITY;
    let mut component = 0;
    while component < 64 {
        let column = lane + component * 64;
        let value = input[row_base + column];
        if value > local_max {
            local_max = value;
        }
        component += 1;
    }
    let maximum = collectives.subgroup_reduce_max_f32::<64>(local_max);

    let mut local_sum = 0.0_f32;
    component = 0;
    while component < 64 {
        let column = lane + component * 64;
        local_sum += math.exp_f32(input[row_base + column] - maximum);
        component += 1;
    }
    let denominator = collectives.subgroup_reduce_sum_f32::<64>(local_sum);

    component = 0;
    while component < 64 {
        let column = lane + component * 64;
        if column < columns as usize
            && let Some(element) =
                output.get_tiled_2d_mut(&output_tile, component, output_rows as usize, 64, 64)
        {
            *element = math.exp_f32(input[row_base + column] - maximum) / denominator;
        }
        component += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accessed_extent_excludes_trailing_padding() {
        assert_eq!(accessed_extent(3, 4100), 12_296);
        assert_eq!(accessed_extent(0, 4100), 0);
    }
}
