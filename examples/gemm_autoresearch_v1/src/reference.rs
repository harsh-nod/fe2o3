#![forbid(unsafe_code)]

//! Sequential safe-Rust reference for the dynamic strided kernel contract.

use fe2o3_device::Bf16;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ReferenceProblemV1 {
    pub rows: u32,
    pub columns: u32,
    pub reduction: u32,
    pub lhs_stride: u32,
    pub rhs_stride: u32,
    pub output_stride: u32,
    pub product_scale: f32,
    pub output_scale: f32,
}

pub fn strided_extent_v1(rows: u32, columns: u32, stride: u32) -> Option<usize> {
    if rows == 0 || columns == 0 {
        return Some(0);
    }
    let last_row = usize::try_from(rows - 1).ok()?;
    let stride = usize::try_from(stride).ok()?;
    let columns = usize::try_from(columns).ok()?;
    last_row.checked_mul(stride)?.checked_add(columns)
}

/// Evaluates every logical output in row-major order while preserving stride
/// padding from `initial_output`.
pub fn evaluate_reference_v1(
    lhs: &[u16],
    rhs: &[u16],
    initial_output: &[f32],
    problem: ReferenceProblemV1,
) -> Result<Vec<f32>, &'static str> {
    if problem.rows != 0 && problem.reduction != 0 && problem.lhs_stride < problem.reduction {
        return Err("lhs stride is smaller than the logical reduction extent");
    }
    if problem.reduction != 0 && problem.columns != 0 && problem.rhs_stride < problem.columns {
        return Err("rhs stride is smaller than the logical column extent");
    }
    if problem.rows != 0 && problem.columns != 0 && problem.output_stride < problem.columns {
        return Err("output stride is smaller than the logical column extent");
    }
    let lhs_len = strided_extent_v1(problem.rows, problem.reduction, problem.lhs_stride)
        .ok_or("lhs extent overflow")?;
    let rhs_len = strided_extent_v1(problem.reduction, problem.columns, problem.rhs_stride)
        .ok_or("rhs extent overflow")?;
    let output_len = strided_extent_v1(problem.rows, problem.columns, problem.output_stride)
        .ok_or("output extent overflow")?;
    if lhs.len() < lhs_len || rhs.len() < rhs_len || initial_output.len() < output_len {
        return Err("reference input is shorter than its declared strided extent");
    }

    let mut output = initial_output.to_vec();
    for row in 0..problem.rows as usize {
        for column in 0..problem.columns as usize {
            let mut accumulator = 0.0_f32;
            for depth in 0..problem.reduction as usize {
                let lhs_value =
                    Bf16::from_bits(lhs[row * problem.lhs_stride as usize + depth]).to_f32();
                let rhs_value =
                    Bf16::from_bits(rhs[depth * problem.rhs_stride as usize + column]).to_f32();
                accumulator += lhs_value * rhs_value;
            }
            let index = row * problem.output_stride as usize + column;
            output[index] =
                problem.product_scale * accumulator + problem.output_scale * initial_output[index];
        }
    }
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dynamic_strides_and_zero_reduction_follow_the_same_contract() {
        let bits = |value| Bf16::from_f32(value).to_bits();
        let lhs = [bits(1.0), bits(2.0), bits(-9.0), bits(3.0), bits(4.0)];
        let rhs = [bits(5.0), bits(6.0), bits(-8.0), bits(7.0), bits(8.0)];
        let initial = [1.0, 2.0, -7.0, 3.0, 4.0];
        let output = evaluate_reference_v1(
            &lhs,
            &rhs,
            &initial,
            ReferenceProblemV1 {
                rows: 2,
                columns: 2,
                reduction: 2,
                lhs_stride: 3,
                rhs_stride: 3,
                output_stride: 3,
                product_scale: 1.0,
                output_scale: 1.0,
            },
        )
        .unwrap();
        assert_eq!(output, [20.0, 24.0, -7.0, 46.0, 54.0]);

        let zero = evaluate_reference_v1(
            &[],
            &[],
            &initial,
            ReferenceProblemV1 {
                rows: 2,
                columns: 2,
                reduction: 0,
                lhs_stride: 0,
                rhs_stride: 0,
                output_stride: 3,
                product_scale: 7.0,
                output_scale: 0.5,
            },
        )
        .unwrap();
        assert_eq!(zero, [0.5, 1.0, -7.0, 1.5, 2.0]);
    }
}
