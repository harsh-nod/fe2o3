use fe2o3_device::ThreadIndex;

fn assert_sync<T: Sync>() {}

fn main() {
    assert_sync::<ThreadIndex>();
}
