use fe2o3_device::atomic::{AtomicI32, AtomicU64, Ordering};

fn standard_atomic_operations(counter: &AtomicU64, signed: &AtomicI32) {
    let _ = counter.load(Ordering::Acquire);
    counter.store(1, Ordering::Release);
    let _ = counter.fetch_add(2, Ordering::AcqRel);
    let _ = counter.compare_exchange(3, 4, Ordering::SeqCst, Ordering::Acquire);
    let _ = signed.fetch_min(-4, Ordering::Relaxed);
}

fn main() {
    let _ = standard_atomic_operations;
}
