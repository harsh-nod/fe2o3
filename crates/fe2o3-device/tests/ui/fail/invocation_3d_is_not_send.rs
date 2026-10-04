use fe2o3_device::Invocation3D;

fn assert_send<T: Send>() {}

fn main() {
    assert_send::<Invocation3D>();
}
