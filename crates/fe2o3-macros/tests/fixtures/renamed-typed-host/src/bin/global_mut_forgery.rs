use gpu_device::{DeviceGlobalMutPtr, kernel};

#[kernel(
    typed,
    namespace = "7c0e8b256bc76d2d17529f43ca8e2ee3480c40dfd019491bd4fb1fc22c4f5f2d",
    launch(required = [64, 1, 1], max = [64, 1, 1])
)]
pub fn atomic(target: DeviceGlobalMutPtr<u32>) {
    let _ = target;
}

fn forge<'a>(region: gpu_host::__generated::GeneratedReadWriteDeviceSlice<'a, u32>) {
    let _ = atomic_gpu::GlobalMut { region };
}

fn main() {
    let _ = forge;
}
