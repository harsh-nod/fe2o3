use fe2o3_device::{InitialEpoch, SubgroupWidth64, WorkgroupCapability};

fn assert_send<T: Send>() {}
fn assert_sync<T: Sync>() {}
fn assert_clone<T: Clone>() {}

fn reject() {
    assert_send::<WorkgroupCapability<'static, (), InitialEpoch>>();
    assert_sync::<WorkgroupCapability<'static, (), InitialEpoch>>();
    assert_clone::<WorkgroupCapability<'static, (), InitialEpoch>>();
    assert_send::<fe2o3_device::Subgroup<'static, SubgroupWidth64, (), InitialEpoch>>();
}

fn main() {}
