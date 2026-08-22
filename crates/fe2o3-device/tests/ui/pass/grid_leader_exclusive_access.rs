use fe2o3_device::{DisjointSlice, GridExclusive, StaticIndex, thread};

fn leader_only(mut output: DisjointSlice<u32, GridExclusive>) {
    if let Some(leader) = thread::grid_leader() {
        if let Some(value) = output.get_mut_exclusive(&leader, 7) {
            *value = 1;
        }

        if let Ok(mut tile) = output.checked_static_tile_mut::<2>(&leader, 3) {
            *tile.at_const_mut(StaticIndex::<2, 1>::CHECKED) = 2;
        }
    }
}

fn main() {}
