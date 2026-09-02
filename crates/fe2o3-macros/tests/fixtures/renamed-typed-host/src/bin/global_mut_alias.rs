use gpu_device::{DeviceGlobalMutPtr, kernel};
use gpu_host::__generated::GeneratedKfdReadWriteSlice;

#[kernel(
    typed,
    namespace = "7c0e8b256bc76d2d17529f43ca8e2ee3480c40dfd019491bd4fb1fc22c4f5f2d",
    launch(required = [64, 1, 1], max = [64, 1, 1])
)]
pub fn atomic_pair(left: DeviceGlobalMutPtr<u32>, right: DeviceGlobalMutPtr<u32>) {
    let _ = (left, right);
}

fn alias<'a>(target: &'a mut [u32]) {
    let left = GeneratedKfdReadWriteSlice::new(target);
    let right = GeneratedKfdReadWriteSlice::new(target);
    let left = atomic_pair_gpu::GlobalMut::new(left).unwrap();
    let right = atomic_pair_gpu::GlobalMut::new(right).unwrap();
    let _ = atomic_pair_gpu::Arguments::new(left, right);
}

fn main() {
    let _ = alias;
}
