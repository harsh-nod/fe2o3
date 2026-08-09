use fe2o3_device::{ActiveLaneGroup, Grid, SubgroupTile, Workgroup, WorkgroupConvergence};

fn assert_clone<T: Clone>() {}

fn main() {
    assert_clone::<Grid<'static>>();
    assert_clone::<Workgroup<'static>>();
    assert_clone::<SubgroupTile<32>>();
    assert_clone::<ActiveLaneGroup>();
    assert_clone::<WorkgroupConvergence<'static, 'static>>();
}
