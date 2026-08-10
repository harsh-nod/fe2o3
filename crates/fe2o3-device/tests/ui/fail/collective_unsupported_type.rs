use fe2o3_device::{Gfx942Collectives, SubgroupTile};

fn reject(context: &Gfx942Collectives, tile: &SubgroupTile<'_, 64>) {
    let _ = unsafe { tile.reduce_sum(context, 1_u64) };
}

fn main() {}
