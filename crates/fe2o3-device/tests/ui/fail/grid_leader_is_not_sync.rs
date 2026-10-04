use fe2o3_device::GridLeader;

fn assert_sync<T: Sync>() {}

fn main() {
    assert_sync::<GridLeader>();
}
