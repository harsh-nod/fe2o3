use fe2o3_device::{ActiveLaneGroup, Grid, SubgroupTile, Workgroup, WorkgroupConvergence};

fn assert_send<T: Send>() {}

fn main() {
    assert_send::<Grid<'static>>();
    assert_send::<Workgroup<'static>>();
    assert_send::<SubgroupTile<32>>();
    assert_send::<ActiveLaneGroup>();
    assert_send::<WorkgroupConvergence<'static, 'static>>();
}
