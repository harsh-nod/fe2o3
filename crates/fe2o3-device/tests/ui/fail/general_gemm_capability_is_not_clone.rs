use fe2o3_device::{GemmReady, Gfx942TiledGemmWave64V1};

fn require_clone<T: Clone>() {}

fn main() {
    require_clone::<Gfx942TiledGemmWave64V1<GemmReady>>();
}
