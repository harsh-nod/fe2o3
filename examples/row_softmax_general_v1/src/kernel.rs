//! Safe Rust dynamic row-softmax qualification kernel.

#![allow(missing_docs)]

use fe2o3_device::{
    GridExclusive, KernelError, KernelResult, Math, WriteOnlyDisjointSlice, kernel, thread,
};

pub const ROW_SOFTMAX_WORKGROUP_V1: [u32; 3] = [64, 1, 1];
pub const ROW_SOFTMAX_MAX_ROWS_V1: u32 = 1024;
pub const ROW_SOFTMAX_MAX_COLUMNS_V1: usize = 192;

fn accessed_extent(rows: u32, columns: u32, stride: u32) -> usize {
    if rows == 0 || columns == 0 {
        return 0;
    }
    (rows - 1) as usize * stride as usize + columns as usize
}

/// Computes independent softmax rows with dynamic row count, column count, and input stride.
///
/// This qualification kernel intentionally uses grid-exclusive write authority:
/// one compiler-authenticated grid leader serializes compact output rows while
/// the production wave-parallel row-striped proof catches up.
#[kernel(
    typed,
    launch(required = [64, 1, 1], max = [64, 1, 1], max_grid = [1, 1, 1]),
    control_flow(loop_bounds(1024, 192, 192, 192))
)]
pub fn row_softmax_general_v1(
    input: &[f32],
    mut output: WriteOnlyDisjointSlice<f32, GridExclusive>,
    rows: u32,
    columns: u32,
    input_stride: u32,
    output_stride: u32,
) -> KernelResult {
    if columns == 0
        || rows == 0
        || rows > ROW_SOFTMAX_MAX_ROWS_V1
        || columns as usize > ROW_SOFTMAX_MAX_COLUMNS_V1
        || input_stride < columns
        || output_stride != columns
    {
        return Err(KernelError::InvalidArgument);
    }
    if input.len() < accessed_extent(rows, columns, input_stride)
        || output.len() < accessed_extent(rows, columns, output_stride)
    {
        return Err(KernelError::InvalidArgument);
    }

    if let Some(leader) = thread::grid_leader() {
        let math = Math::current();
        let input_stride = input_stride as usize;
        let rows = rows as usize;
        let columns = columns as usize;

        let mut input_row_base = 0_usize;
        let mut output_index = 0_usize;
        let mut row = 0_usize;
        while row < rows {
            let mut maximum = f32::NEG_INFINITY;
            let mut column = 0_usize;
            while column < columns {
                let input_index = input_row_base.wrapping_add(column);
                let value = input[input_index];
                if value > maximum {
                    maximum = value;
                }
                column += 1;
            }

            let mut denominator = 0.0_f32;
            column = 0;
            while column < columns {
                let input_index = input_row_base.wrapping_add(column);
                denominator += math.exp_f32(input[input_index] - maximum);
                column += 1;
            }

            column = 0;
            while column < columns {
                let input_index = input_row_base.wrapping_add(column);
                let probability = math.exp_f32(input[input_index] - maximum) / denominator;
                let _ = output.write_exclusive(&leader, output_index, probability);
                output_index = output_index.wrapping_add(1);
                column += 1;
            }
            input_row_base = input_row_base.wrapping_add(input_stride);
            row += 1;
        }
    }
    Ok(())
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
