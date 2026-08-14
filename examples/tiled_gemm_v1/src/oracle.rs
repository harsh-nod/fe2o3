//! Bitwise-specified scalar CPU oracle.

use core::fmt;
use std::hint::black_box;

use fe2o3_device::Bf16;

use crate::contract::ShapeV1;

/// Input validation failed before oracle evaluation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OracleErrorV1 {
    /// `A` did not have exactly `M*K` BF16 elements.
    WrongALength {
        /// Required element count.
        expected: usize,
        /// Supplied element count.
        actual: usize,
    },
    /// `B` did not have exactly `K*N` BF16 elements.
    WrongBLength {
        /// Required element count.
        expected: usize,
        /// Supplied element count.
        actual: usize,
    },
}

impl fmt::Display for OracleErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WrongALength { expected, actual } => {
                write!(formatter, "A requires {expected} elements, got {actual}")
            }
            Self::WrongBLength { expected, actual } => {
                write!(formatter, "B requires {expected} elements, got {actual}")
            }
        }
    }
}

impl std::error::Error for OracleErrorV1 {}

#[inline(never)]
fn fp32_product_v1(left: f32, right: f32) -> f32 {
    black_box(black_box(left) * black_box(right))
}

#[inline(never)]
fn fp32_sum_v1(left: f32, right: f32) -> f32 {
    black_box(black_box(left) + black_box(right))
}

/// Evaluates the V1 host reference for `C=A*B`.
///
/// Each BF16 value widens exactly to FP32. For every output `(row, column)`,
/// accumulation starts at FP32 positive zero and visits `depth=0..K` in
/// increasing order. Multiplication and addition are separate FP32 operations;
/// fused contraction is not part of this oracle. Callers compare `f32::to_bits`
/// when requiring the bitwise reference.
///
/// This defines host evidence only. It does not assert undocumented MFMA
/// evaluation order or prove that a future lowering refines this recurrence.
pub fn tiled_gemm_oracle_v1(
    shape: ShapeV1,
    a: &[Bf16],
    b: &[Bf16],
) -> Result<Vec<f32>, OracleErrorV1> {
    if a.len() != shape.a_elements {
        return Err(OracleErrorV1::WrongALength {
            expected: shape.a_elements,
            actual: a.len(),
        });
    }
    if b.len() != shape.b_elements {
        return Err(OracleErrorV1::WrongBLength {
            expected: shape.b_elements,
            actual: b.len(),
        });
    }

    let mut output = vec![f32::from_bits(0); shape.c_elements];
    for row in 0..shape.m {
        for column in 0..shape.n {
            let mut accumulator = f32::from_bits(0);
            for depth in 0..shape.k {
                let left = a[shape.a_index(row, depth).expect("bounded A coordinate")].to_f32();
                let right = b[shape.b_index(depth, column).expect("bounded B coordinate")].to_f32();
                let product = fp32_product_v1(left, right);
                accumulator = fp32_sum_v1(accumulator, product);
            }
            let index = shape
                .c_index(row, column)
                .expect("bounded output coordinate");
            output[index] = accumulator;
        }
    }
    Ok(output)
}
