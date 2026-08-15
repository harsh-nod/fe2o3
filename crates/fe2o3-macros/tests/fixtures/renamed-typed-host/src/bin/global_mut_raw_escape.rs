use gpu_device::{DeviceGlobalMutPtr, kernel};

#[kernel(
    typed,
    namespace = "7c0e8b256bc76d2d17529f43ca8e2ee3480c40dfd019491bd4fb1fc22c4f5f2d",
    launch(required = [64, 1, 1], max = [64, 1, 1])
)]
pub fn atomic(target: DeviceGlobalMutPtr<u32>) {
    let _ = target;
}

fn escape(target: atomic_gpu::GlobalMut<'_, u32>) -> *mut u32 {
    target.as_raw()
}

fn main() {
    let _ = escape;
}
