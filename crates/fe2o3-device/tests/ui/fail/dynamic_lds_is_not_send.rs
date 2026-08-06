use fe2o3_device::DynamicLds;

fn assert_send<T: Send>() {}

fn main() {
    assert_send::<DynamicLds<'static, u32>>();
}
