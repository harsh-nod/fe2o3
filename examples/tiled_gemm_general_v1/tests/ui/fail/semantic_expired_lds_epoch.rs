#![forbid(unsafe_code)]

use fe2o3_gemm_device_v1::Gfx942TiledGemmWave64V1;

fn main() {
    let wave = Gfx942TiledGemmWave64V1::from_compiler(16);
    let staged = wave.stage([0; 4], [0; 4]);
    let published = staged.publish();
    let consumed = published.multiply_accumulate();
    let ready = consumed.reuse();
    let staged_next = ready.stage([0; 4], [0; 4]);
    let published_next = staged_next.publish();
    let _consumed_next = published_next.multiply_accumulate();
    let _expired = published.multiply_accumulate();
}
