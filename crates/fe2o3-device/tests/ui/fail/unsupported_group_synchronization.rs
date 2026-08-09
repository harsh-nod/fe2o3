use fe2o3_device::{ActiveLaneGroup, Grid, SubgroupTile};

fn grid(group: &Grid<'_>) {
    let _ = group.assume_uniform();
}

fn tile(group: &SubgroupTile<32>) {
    let _ = group.assume_uniform();
}

fn active(group: &ActiveLaneGroup) {
    let _ = group.assume_uniform();
}

fn main() {}
