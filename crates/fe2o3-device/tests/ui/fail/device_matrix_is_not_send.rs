use fe2o3_device::DeviceMatrix;

fn require_send<T: Send>() {}

fn main() {
    require_send::<DeviceMatrix>();
}
