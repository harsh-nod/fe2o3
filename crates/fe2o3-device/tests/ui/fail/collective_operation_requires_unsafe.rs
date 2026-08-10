use fe2o3_device::{Gfx942Collectives, SubgroupTile};

fn reject(context: &Gfx942Collectives, tile: &SubgroupTile<'_, 64>) {
    let _ = tile.reduce_sum(context, 1_u32);
}

fn main() {}
