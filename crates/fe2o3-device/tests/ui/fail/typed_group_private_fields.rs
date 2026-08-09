use core::marker::PhantomData;
use fe2o3_device::{ActiveLaneGroup, Grid, SubgroupTile, Workgroup};

fn forge_grid() {
    let _ = Grid {
        size: 1,
        thread_rank: 0,
        _invocation: PhantomData,
        _not_send_sync: PhantomData,
    };
}

fn forge_workgroup() {
    let _ = Workgroup {
        size: 1,
        thread_rank: 0,
        _invocation: PhantomData,
        _not_send_sync: PhantomData,
    };
}

fn forge_tile() {
    let _ = SubgroupTile::<32> {
        lane: 0,
        _wave_snapshot: PhantomData,
        _not_send_sync: PhantomData,
    };
}

fn forge_active_group() {
    let _ = ActiveLaneGroup {
        lane: 0,
        asserted_mask: 1,
        _wave_snapshot: PhantomData,
        _not_send_sync: PhantomData,
    };
}

fn main() {}
