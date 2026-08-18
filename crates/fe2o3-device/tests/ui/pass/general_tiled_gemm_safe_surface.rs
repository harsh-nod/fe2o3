#![forbid(unsafe_code)]

use fe2o3_device::{DisjointSlice, Gfx942TiledGemmWave64V1};

fn checked_safe_sequence(mut c: DisjointSlice<f32>, k: u32) {
    let mut wave = Gfx942TiledGemmWave64V1::from_compiler(k);
    while wave.has_remaining_phases() {
        let _coordinates = (
            wave.lane(),
            wave.tile_row(),
            wave.tile_column(),
            wave.phase(),
        );
        let staged = wave.stage([0; 4], [0; 4]);
        let published = staged.publish();
        let consumed = published.multiply_accumulate();
        wave = consumed.reuse();
    }
    wave.store_c_fragment(&mut c, 16, 16, 16, 1.0, 0.0);
}

fn main() {
    let _: fn(DisjointSlice<f32>, u32) = checked_safe_sequence;
}
