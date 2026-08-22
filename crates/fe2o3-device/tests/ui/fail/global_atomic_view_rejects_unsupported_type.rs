use fe2o3_device::DeviceGlobalMutPtr;

fn unsupported(pointer: &DeviceGlobalMutPtr<u16>) {
    let _ = pointer.as_atomic();
}

fn main() {}
