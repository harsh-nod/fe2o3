use fe2o3_device::{GemmReady, Gfx942TiledGemmWave64V1};

fn require_copy<T: Copy>() {}

fn main() {
    require_copy::<Gfx942TiledGemmWave64V1<GemmReady>>();
}
