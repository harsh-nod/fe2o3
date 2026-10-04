#![forbid(unsafe_code)]

use fe2o3_gemm_device_v1::{DisjointSlice, Gfx942TiledGemmWave64V1};

fn attempt(mut c: DisjointSlice<f32>) {
    let wave = Gfx942TiledGemmWave64V1::from_compiler(0);
    wave.store_c_at(&mut c, usize::MAX, 1.0);
}

fn main() {}
