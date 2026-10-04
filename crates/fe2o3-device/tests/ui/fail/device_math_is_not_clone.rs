use fe2o3_device::DeviceMath;

fn require_clone<T: Clone>() {}

fn main() {
    require_clone::<DeviceMath>();
}
