use fe2o3_device::{ExclusiveReadWrite, Global};

fn assert_copy<T: Copy>() {}
fn assert_clone<T: Clone>() {}
fn assert_send<T: Send>() {}
fn assert_sync<T: Sync>() {}

fn reject() {
    assert_copy::<Global<'static, f32, ExclusiveReadWrite, ()>>();
    assert_clone::<Global<'static, f32, ExclusiveReadWrite, ()>>();
    assert_send::<Global<'static, f32, ExclusiveReadWrite, ()>>();
    assert_sync::<Global<'static, f32, ExclusiveReadWrite, ()>>();
}

fn main() {}
