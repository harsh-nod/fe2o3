use fe2o3_device::DeviceMath;

fn require_send<T: Send>() {}

fn main() {
    require_send::<DeviceMath>();
}
