use fe2o3_core::GpuContext;
use fe2o3_host::{Gfx942Xor4Bf16TileAllocationV1, Gfx942Xor4Bf16TileLeaseV1};
use std::sync::Arc;

fn rejected<'allocation>(
    tile: &'allocation mut Gfx942Xor4Bf16TileAllocationV1,
    context: &Arc<GpuContext>,
) -> Gfx942Xor4Bf16TileLeaseV1<'allocation, 'static> {
    let stream = context.create_stream().unwrap();
    tile.lease(&stream).unwrap()
}

fn main() {}
