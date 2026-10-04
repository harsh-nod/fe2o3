use fe2o3_device::{Index1D, StaticTileRegionWitness};

fn require_clone<T: Clone>() {}

fn main() {
    require_clone::<StaticTileRegionWitness<'static, u32, Index1D, 4>>();
}
