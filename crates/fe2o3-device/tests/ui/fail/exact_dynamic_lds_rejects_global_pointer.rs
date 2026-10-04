use fe2o3_device::{DeviceGlobalMutPtr, DynamicLds};

fn masquerade(global: DeviceGlobalMutPtr<u8>) {
    let _ = DynamicLds::<i32>::exact_current::<64>(global);
}

fn main() {
    let _ = masquerade;
}
