use fe2o3_device::DeviceGlobalMutPtr;
use fe2o3_device::atomic::AtomicU32;

fn escape(pointer: DeviceGlobalMutPtr<u32>) -> &'static AtomicU32 {
    pointer.as_atomic()
}

fn main() {}
