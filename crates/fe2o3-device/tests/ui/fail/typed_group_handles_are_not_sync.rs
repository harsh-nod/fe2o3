use fe2o3_device::{ActiveLaneGroup, Grid, SubgroupTile, Workgroup, WorkgroupConvergence};

fn assert_sync<T: Sync>() {}

fn main() {
    assert_sync::<Grid<'static>>();
    assert_sync::<Workgroup<'static>>();
    assert_sync::<SubgroupTile<32>>();
    assert_sync::<ActiveLaneGroup>();
    assert_sync::<WorkgroupConvergence<'static, 'static>>();
}
