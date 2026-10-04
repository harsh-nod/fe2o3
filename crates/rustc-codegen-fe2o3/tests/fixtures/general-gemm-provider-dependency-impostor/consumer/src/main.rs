#![forbid(unsafe_code)]

use fe2o3_device::kernel;
use fe2o3_gemm_device_v1::ProofSensitiveGeneralGemmWave64V1;

#[kernel(launch(required = [64, 1, 1], max = [64, 1, 1]))]
pub fn dependency_impostor(k: u32) {
    let _ = ProofSensitiveGeneralGemmWave64V1::from_compiler(k);
}

fn main() {}
