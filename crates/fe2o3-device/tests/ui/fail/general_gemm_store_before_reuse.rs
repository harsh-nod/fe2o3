use fe2o3_device::{DisjointSlice, Gfx942TiledGemmWave64V1};

fn cannot_store(mut c: DisjointSlice<f32>) {
    let wave = Gfx942TiledGemmWave64V1::from_compiler(16);
    let staged = wave.stage([0; 4], [0; 4]);
    let published = staged.publish();
    let consumed = published.multiply_accumulate();
    consumed.store_c_fragment(&mut c, 16, 16, 16, 1.0, 0.0);
}

fn main() {}
