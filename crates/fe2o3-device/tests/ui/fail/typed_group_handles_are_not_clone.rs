use fe2o3_device::{ActiveLaneGroup, Grid, SubgroupTile, Workgroup};

fn assert_clone<T: Clone>() {}

fn main() {
    assert_clone::<Grid<'static>>();
    assert_clone::<Workgroup<'static>>();
    assert_clone::<SubgroupTile<'static, 32>>();
    assert_clone::<ActiveLaneGroup<'static>>();
}
