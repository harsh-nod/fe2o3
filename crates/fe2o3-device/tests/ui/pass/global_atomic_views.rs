use fe2o3_device::DeviceGlobalMutPtr;
use fe2o3_device::atomic::{AtomicI32, AtomicI64, AtomicU32, AtomicU64, Ordering};

fn supported_global_atomic_views(
    u32_pointer: &DeviceGlobalMutPtr<u32>,
    i32_pointer: &DeviceGlobalMutPtr<i32>,
    u64_pointer: &DeviceGlobalMutPtr<u64>,
    i64_pointer: &DeviceGlobalMutPtr<i64>,
) {
    let u32_atomic: &AtomicU32 = u32_pointer.as_atomic();
    let i32_atomic: &AtomicI32 = i32_pointer.as_atomic();
    let u64_atomic: &AtomicU64 = u64_pointer.as_atomic();
    let i64_atomic: &AtomicI64 = i64_pointer.as_atomic();

    let _ = u32_atomic.fetch_add(1, Ordering::Relaxed);
    let _ = i32_atomic.fetch_add(1, Ordering::Acquire);
    let _ = u64_atomic.fetch_add(1, Ordering::Release);
    let _ = i64_atomic.fetch_add(1, Ordering::SeqCst);
}

fn main() {
    let _ = supported_global_atomic_views;
}
