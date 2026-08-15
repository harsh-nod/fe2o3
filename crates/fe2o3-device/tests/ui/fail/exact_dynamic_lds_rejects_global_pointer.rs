use fe2o3_device::{DeviceGlobalMutPtr, DynamicLds};

fn masquerade(global: DeviceGlobalMutPtr<u8>) {
    let _ = unsafe { DynamicLds::<i32>::exact_from_compiler::<64>(global, 0) };
}

fn main() {
    let _ = masquerade;
}
