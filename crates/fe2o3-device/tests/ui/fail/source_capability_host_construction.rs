use core::marker::PhantomData;

use fe2o3_device::{InitialEpoch, WorkgroupCapability, WorkgroupEpoch};

fn forge(epoch: WorkgroupEpoch<'static, (), InitialEpoch>) {
    let _ = WorkgroupCapability::<'static, (), InitialEpoch> {
        size: 1,
        rank: 0,
        epoch,
        _not_send_sync: PhantomData,
    };
}

fn main() {}
