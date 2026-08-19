#![forbid(unsafe_code)]

use fe2o3_device::{DisjointSlice, kernel};
use fe2o3_gemm_device_v1::ProofSensitiveGeneralGemmWave64V1;

#[kernel(launch(required = [64, 1, 1], max = [64, 1, 1]))]
#[allow(clippy::too_many_arguments)]
pub fn duplicate_store(
    a: &[u16],
    b: &[u16],
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
    let mut context = ProofSensitiveGeneralGemmWave64V1::from_compiler(k);
    let a_value = load_bf16_or_zero(a, 0, 0, m, k, lda);
    let b_value = load_bf16_or_zero(b, 0, 0, k, n, ldb);
    context.stage([a_value; 4], [b_value; 4]);
    context.publish();
    context.multiply_accumulate();
    context.reuse();
    context.store_c_fragment(&mut c, m, n, ldc, alpha, beta);
    context.store_c_fragment(&mut c, m, n, ldc, alpha, beta);
}

fn load_bf16_or_zero(
    values: &[u16],
    row: u32,
    column: u32,
    rows: u32,
    columns: u32,
    stride: u32,
) -> u16 {
    if row >= rows || column >= columns {
        return 0;
    }
    values
        .get((u64::from(row) * u64::from(stride) + u64::from(column)) as usize)
        .copied()
        .unwrap_or(0)
}

fn main() {}
