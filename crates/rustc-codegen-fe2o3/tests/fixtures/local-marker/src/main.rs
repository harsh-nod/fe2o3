#![feature(rustc_attrs)]
#![allow(internal_features)]

#[cfg(feature = "duplicate-genuine")]
const _: usize = core::mem::size_of::<fe2o3_device_real::ThreadIndex>();

#[repr(C)]
#[rustc_diagnostic_item = "fe2o3_device_disjoint_slice"]
pub struct DisjointSlice<T> {
    ptr: *mut T,
    len: usize,
}

impl<T> DisjointSlice<T> {
    pub fn get_mut(&mut self, index: usize) -> Option<&mut T> {
        if index >= self.len {
            return None;
        }
        Some(unsafe { &mut *self.ptr.add(index) })
    }
}

// This adversarial fixture intentionally has no fe2o3-device dependency. It
// reproduces the immutable legacy collector record so compilation reaches the
// backend, where the local diagnostic-item spoof must still be rejected.
#[unsafe(no_mangle)]
pub fn fe2o3_kernel_local_marker(mut output: DisjointSlice<f32>) {
    if let Some(value) = output.get_mut(0) {
        *value = 1.0;
    }
}

#[used]
// Keep the exact legacy six-field registration shape that this adversarial
// fixture attempts to spoof.
#[allow(
    non_upper_case_globals,
    clippy::redundant_static_lifetimes,
    clippy::type_complexity
)]
static __fe2o3_kernel_registration_local_marker: (
    u64,
    u16,
    u16,
    &'static str,
    &'static str,
    fn(DisjointSlice<f32>),
) = (
    0x4e52_4b33_4f32_4546,
    1,
    1,
    "local_marker",
    "local_marker",
    fe2o3_kernel_local_marker,
);

fn main() {}
