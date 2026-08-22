use fe2o3_device::{DisjointSlice, GridExclusive, GridLeader, StaticIndex};

fn checked(mut parent: DisjointSlice<u32, GridExclusive>, leader: &GridLeader) {
    let mut tile = parent.checked_static_tile_mut::<4>(leader, 2).unwrap();
    let _: &u32 = tile.at_const(StaticIndex::<4, 0>::CHECKED);
    let _: &mut u32 = tile.at_const_mut(StaticIndex::<4, 3>::CHECKED);
    let _: &[u32; 4] = tile.as_array();
    let _: &mut [u32; 4] = tile.as_mut_array();
    assert_eq!(tile.region_witness().start_element(), 2);
    assert_eq!(tile.region_witness().tile_len(), 4);
}

fn main() {}
