#![forbid(unsafe_code)]

use fe2o3_device::{DisjointSlice, kernel};
use fe2o3_gemm_device_v1::ProofSensitiveGeneralGemmWave64V1;

#[kernel(launch(required = [64, 1, 1], max = [64, 1, 1]))]
#[allow(clippy::too_many_arguments)]
pub fn equivalent_manifest(
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
    let _ = (a, b, lda, ldb);
    let mut context = ProofSensitiveGeneralGemmWave64V1::from_compiler(k);
    context.stage([0; 4], [0; 4]);
    context.multiply_accumulate();
    context.reuse();
    context.store_c_fragment(&mut c, m, n, ldc, alpha, beta);
}

fn main() {}
