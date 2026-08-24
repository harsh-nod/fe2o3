//! Ordinary attributed source for the fixed scoped atomic-add profile.

#![allow(missing_docs)] // Generated typed-kernel modules do not carry rustdoc in V1.

use fe2o3_device::atomic::{CORE_ATOMIC_DEFAULT_SCOPE, CoreAtomicDefaultScope, Ordering};
use fe2o3_device::{DeviceGlobalMutPtr, kernel, thread};

const _: CoreAtomicDefaultScope = CoreAtomicDefaultScope::System;
const _: CoreAtomicDefaultScope = CORE_ATOMIC_DEFAULT_SCOPE;

/// Adds each eligible lane's value exactly once to one global atomic object.
///
/// `DeviceGlobalMutPtr` states the global address space. Ordinary Rust atomics
/// map to system scope in fe2o3-device, and `Ordering::Relaxed` is explicit.
/// Host admission guarantees that the exact mathematical sum fits in `u32`.
#[kernel(
    typed,
    launch(required = [64, 1, 1], max = [64, 1, 1])
)]
pub fn scoped_atomic_add_u32_v1(values: &[u32], eligible: &[u32], target: DeviceGlobalMutPtr<u32>) {
    let lane = thread::thread_idx_x() as usize;
    if values.len() != 64 || eligible.len() != 64 || lane >= 64 {
        fe2o3_device::trap();
        return;
    }
    if eligible[lane] != 0 {
        target
            .as_atomic()
            .fetch_add(values[lane], Ordering::Relaxed);
    }
}
