use fe2o3_core::Stream;
use fe2o3_host::Gfx942Xor4Bf16TileAllocationV1;

fn rejected(stream: &Stream, mut tile: Gfx942Xor4Bf16TileAllocationV1) {
    let lease = tile.lease(stream).unwrap();
    drop(tile);
    let _ = lease.len();
}

fn main() {}
