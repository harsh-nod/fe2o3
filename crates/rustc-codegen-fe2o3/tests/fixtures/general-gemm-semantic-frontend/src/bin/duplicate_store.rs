#![forbid(unsafe_code)]

use fe2o3_device::{DisjointSlice, kernel};
use fe2o3_gemm_device_v1::ProofSensitiveGeneralGemmWave64V1;

#[kernel(launch(required = [64, 1, 1], max = [64, 1, 1]))]
pub fn duplicate_store(mut c: DisjointSlice<f32>, k: u32) {
    let mut context = ProofSensitiveGeneralGemmWave64V1::from_compiler(k);
    context.stage([0; 4], [0; 4]);
    context.publish();
    context.multiply_accumulate();
    context.reuse();
    context.store_c_fragment(&mut c, 16, 16, 16, 1.0, 0.0);
    context.store_c_fragment(&mut c, 16, 16, 16, 1.0, 0.0);
}

fn main() {}
