use fe2o3_device::WorkgroupLdsScope;

fn assert_send<T: Send>() {}

fn main() {
    assert_send::<WorkgroupLdsScope<'static>>();
}
