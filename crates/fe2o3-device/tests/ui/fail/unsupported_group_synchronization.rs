use fe2o3_device::{ActiveLaneGroup, Grid, SubgroupTile};

fn grid(group: &Grid<'_>) {
    group.synchronize();
}

fn tile(group: &SubgroupTile<'_, 32>) {
    group.synchronize();
}

fn active(group: &ActiveLaneGroup<'_>) {
    group.synchronize();
}

fn main() {}
