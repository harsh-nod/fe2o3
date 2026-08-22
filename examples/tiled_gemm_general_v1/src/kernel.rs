//! Safe Rust qualification kernel for dynamic strided matrix multiplication.

#![allow(missing_docs)] // Generated typed-kernel modules lack rustdoc in V1.

use fe2o3_device::{DisjointSlice, kernel, thread};

/// Exact workgroup dimensions used by the scalar production baseline.
pub const GENERAL_TILED_GEMM_WORKGROUP_V1: [u32; 3] = [256, 1, 1];
/// Maximum K-loop trip count admitted by the source contract.
pub const GENERAL_TILED_GEMM_MAX_PHASES_V1: u32 = u32::MAX;

const _: [(); 64] = [(); usize::BITS as usize];

fn accessed_extent(rows: u32, columns: u32, stride: u32) -> usize {
    if rows == 0 || columns == 0 {
        return 0;
    }
    ((rows - 1) as u64 * stride as u64 + columns as u64) as usize
}

/// Computes `C = alpha * A * B + beta * C` for dynamic row-major matrices.
///
/// The host launches at least `M * ldc` one-dimensional invocations, rounded
/// up to a whole workgroup. Invocation `i` owns physical output slot `i`.
/// Slots in row padding and invocations beyond the final row return without a
/// memory access. This identity ownership lets the generic race pass prove
/// stores disjoint without knowing that the arithmetic is matrix multiplication.
#[kernel(
    typed,
    namespace = "52eacddf5078f2148b0d451a017208c8651ab8b7b80035f9beba9bf89746391e",
    launch(required = [256, 1, 1], max = [256, 1, 1]),
    control_flow(loop_bounds(4294967295))
)]
#[allow(clippy::too_many_arguments)]
pub fn tiled_gemm_general_v1(
    a: &[f32],
    b: &[f32],
    mut c: DisjointSlice<f32>,
    m: u32,
    n: u32,
    k: u32,
    lda: u32,
    ldb: u32,
    ldc: u32,
    alpha: f32,
    beta: f32,
) {
    let invalid_stride = (m != 0 && k != 0 && lda < k)
        || (k != 0 && n != 0 && ldb < n)
        || (m != 0 && n != 0 && ldc < n);
    let a_extent = accessed_extent(m, k, lda);
    let b_extent = accessed_extent(k, n, ldb);
    let c_extent = accessed_extent(m, n, ldc);
    if invalid_stride || a.len() < a_extent || b.len() < b_extent || c.len() < c_extent {
        return;
    }

    let output_index = thread::index_1d();
    let physical = output_index.get();
    if ldc == 0 {
        return;
    }
    let row = physical / ldc as usize;
    let column = physical % ldc as usize;
    if row >= m as usize || column >= n as usize {
        return;
    }

    let mut depth = 0_u32;
    let mut accumulator = 0.0_f32;
    while depth < k {
        let a_index = row * lda as usize + depth as usize;
        let b_index = depth as usize * ldb as usize + column;
        let a_value = a[a_index];
        let b_value = b[b_index];
        accumulator += a_value * b_value;
        depth += 1;
    }

    if let Some(output) = c.get_mut(output_index) {
        *output = alpha * accumulator + beta * *output;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strided_extents_include_only_accessed_elements() {
        assert_eq!(accessed_extent(3, 2, 5), 12);
        assert_eq!(accessed_extent(0, 2, 5), 0);
        assert_eq!(accessed_extent(3, 0, 5), 0);
    }
}
