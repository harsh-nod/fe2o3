use fe2o3_device::{ActiveLaneGroup, Grid, SubgroupTile, Workgroup};

fn assert_send<T: Send>() {}

fn main() {
    assert_send::<Grid<'static>>();
    assert_send::<Workgroup<'static>>();
    assert_send::<SubgroupTile<'static, 32>>();
    assert_send::<ActiveLaneGroup<'static>>();
}
