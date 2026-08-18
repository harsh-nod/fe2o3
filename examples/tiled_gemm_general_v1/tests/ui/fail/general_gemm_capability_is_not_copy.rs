use fe2o3_gemm_device_v1::{GemmReady, Gfx942TiledGemmWave64V1};

fn require_copy<T: Copy>() {}

fn main() {
    require_copy::<Gfx942TiledGemmWave64V1<GemmReady>>();
}
