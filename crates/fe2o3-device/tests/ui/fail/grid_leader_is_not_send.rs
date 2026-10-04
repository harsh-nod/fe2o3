use fe2o3_device::GridLeader;

fn assert_send<T: Send>() {}

fn main() {
    assert_send::<GridLeader>();
}
