use fe2o3_device::Invocation3D;

fn assert_sync<T: Sync>() {}

fn main() {
    assert_sync::<Invocation3D>();
}
