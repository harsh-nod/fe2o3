use fe2o3_device::{DisjointSlice, GridExclusive, GridLeader, StaticIndex};

fn reuse_parent(parent: &mut DisjointSlice<u32, GridExclusive>, leader: &GridLeader) {
    let tile = parent.checked_static_tile_mut::<4>(leader, 0).unwrap();
    let _ = parent.len();
    let _ = tile.at_const(StaticIndex::<4, 0>::CHECKED);
}

fn main() {}
