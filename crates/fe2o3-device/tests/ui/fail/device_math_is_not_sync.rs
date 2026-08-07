use fe2o3_device::DeviceMath;

fn require_sync<T: Sync>() {}

fn main() {
    require_sync::<DeviceMath>();
}
