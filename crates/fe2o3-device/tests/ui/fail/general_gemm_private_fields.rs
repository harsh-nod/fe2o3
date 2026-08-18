use core::marker::PhantomData;

use fe2o3_device::{GemmReady, Gfx942TiledGemmWave64V1};

fn forge() -> Gfx942TiledGemmWave64V1<GemmReady> {
    Gfx942TiledGemmWave64V1 {
        lane: 0,
        tile_row: 0,
        tile_column: 0,
        epoch: 0,
        phases: 0,
        accumulator: [0.0; 4],
        _state: PhantomData,
        _not_send_sync: PhantomData,
    }
}

fn main() {
    let _ = forge();
}
