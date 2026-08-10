use fe2o3_device::{DisjointStaticTileMut, Index1D, StaticIndex};

fn out_of_bounds(tile: &DisjointStaticTileMut<'_, u32, Index1D, 4>) {
    let _ = tile.at_const(StaticIndex::<4, 4>::CHECKED);
}

fn main() {}
