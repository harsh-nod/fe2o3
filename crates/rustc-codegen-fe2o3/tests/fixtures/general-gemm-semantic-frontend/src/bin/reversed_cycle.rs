#![forbid(unsafe_code)]

use fe2o3_device::{DisjointSlice, kernel};
use fe2o3_gemm_device_v1::ProofSensitiveGeneralGemmWave64V1;

#[kernel(
    launch(required = [64, 1, 1], max = [64, 1, 1]),
    control_flow(loop_bounds(268435456))
)]
pub fn reversed_cycle(mut c: DisjointSlice<f32>, k: u32) {
    let mut context = ProofSensitiveGeneralGemmWave64V1::from_compiler(k);
    let mut remaining = k;
    while remaining != 0 {
        context.multiply_accumulate();
        context.reuse();
        context.stage([0; 4], [0; 4]);
        context.publish();
        remaining -= 1;
    }
    context.store_c_fragment(&mut c, 16, 16, 16, 1.0, 0.0);
}

fn main() {}
