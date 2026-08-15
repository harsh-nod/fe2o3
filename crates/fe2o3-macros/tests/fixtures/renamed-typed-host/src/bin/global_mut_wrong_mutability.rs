use gpu_device::{DeviceGlobalMutPtr, kernel};
use gpu_host::__generated::{DeviceBuffer, GeneratedReadDeviceSlice, ObservedContext};

#[kernel(
    typed,
    namespace = "7c0e8b256bc76d2d17529f43ca8e2ee3480c40dfd019491bd4fb1fc22c4f5f2d",
    launch(required = [64, 1, 1], max = [64, 1, 1])
)]
pub fn atomic(target: DeviceGlobalMutPtr<u32>) {
    let _ = target;
}

fn wrong<'a>(observed: &ObservedContext, target: &'a DeviceBuffer<u32>) {
    let target = GeneratedReadDeviceSlice::new(observed, target).unwrap();
    let _ = atomic_gpu::GlobalMut::new(target);
}

fn main() {
    let _ = wrong;
}
