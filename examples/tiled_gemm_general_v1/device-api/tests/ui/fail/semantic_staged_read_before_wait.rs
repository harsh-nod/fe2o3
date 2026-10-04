#![forbid(unsafe_code)]

use fe2o3_gemm_device_v1::Gfx942TiledGemmWave64V1;

fn main() {
    let wave = Gfx942TiledGemmWave64V1::from_compiler(16);
    let staged = wave.stage([0; 4], [0; 4]);
    let _premature = staged.read_staged();
}
