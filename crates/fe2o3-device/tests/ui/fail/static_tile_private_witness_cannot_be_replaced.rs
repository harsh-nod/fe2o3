use fe2o3_device::{DisjointStaticTileMut, Index1D, StaticTileRegionWitness};

fn substitute(
    tile: &mut DisjointStaticTileMut<'_, u32, Index1D, 4>,
    replacement: StaticTileRegionWitness<'_, u32, Index1D, 4>,
) {
    tile.region = replacement;
}

fn main() {}
