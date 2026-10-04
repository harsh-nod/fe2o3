use fe2o3_device::WorkgroupLdsScope;

fn assert_sync<T: Sync>() {}

fn main() {
    assert_sync::<WorkgroupLdsScope<'static>>();
}
