use fe2o3_device::ThreadIndex;

fn assert_send<T: Send>() {}

fn main() {
    assert_send::<ThreadIndex>();
}
