use fe2o3_device::{ReusableWorkgroup, ReusableWorkgroupLds};

fn assert_copy<T: Copy>() {}
fn assert_clone<T: Clone>() {}
fn assert_send<T: Send>() {}
fn assert_sync<T: Sync>() {}

fn reject() {
    assert_copy::<ReusableWorkgroup<'static, ()>>();
    assert_clone::<ReusableWorkgroup<'static, ()>>();
    assert_send::<ReusableWorkgroup<'static, ()>>();
    assert_sync::<ReusableWorkgroup<'static, ()>>();
    assert_copy::<ReusableWorkgroupLds<'static, u32, 64, ()>>();
    assert_clone::<ReusableWorkgroupLds<'static, u32, 64, ()>>();
}

fn main() {}
