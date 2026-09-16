#![feature(rustc_attrs)]
#![allow(internal_features)]
#![cfg_attr(
    any(
        feature = "reserved-capability-spoof",
        feature = "reserved-capability-control"
    ),
    no_std
)]

#[cfg(feature = "duplicate-genuine")]
const _: usize = core::mem::size_of::<fe2o3_device_real::ThreadIndex>();

#[repr(C)]
#[rustc_diagnostic_item = "fe2o3_device_disjoint_slice"]
#[cfg(not(any(
    feature = "reserved-capability-spoof",
    feature = "reserved-capability-control"
)))]
pub struct DisjointSlice<T> {
    ptr: *mut T,
    len: usize,
}

#[cfg(not(any(
    feature = "reserved-capability-spoof",
    feature = "reserved-capability-control"
)))]
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
#[allow(unused_mut)]
pub fn fe2o3_kernel_local_marker(mut output: RootArgument) {
    #[cfg(not(any(
        feature = "reserved-capability-spoof",
        feature = "reserved-capability-control"
    )))]
    if let Some(value) = output.get_mut(0) {
        *value = 1.0;
    }
    #[cfg(any(
        feature = "reserved-capability-spoof",
        feature = "reserved-capability-control"
    ))]
    let _ = output;
}

#[cfg(not(any(
    feature = "reserved-capability-spoof",
    feature = "reserved-capability-control"
)))]
type RootArgument = DisjointSlice<f32>;

#[cfg(any(
    feature = "reserved-capability-spoof",
    feature = "reserved-capability-control"
))]
type RootArgument = ReservedMarker;

#[cfg(any(
    feature = "reserved-capability-spoof",
    feature = "reserved-capability-control"
))]
#[cfg_attr(
    feature = "reserved-capability-spoof",
    rustc_diagnostic_item = "fe2o3_device_kernel_context_v1"
)]
pub struct ReservedMarker;

#[cfg(any(
    feature = "reserved-capability-spoof",
    feature = "reserved-capability-control"
))]
const RESERVED_FRONTEND_CONTRACT: &[u8] = &[
    70, 69, 50, 79, 51, 75, 70, 0, 1, 0, 1, 0, 52, 0, 0, 0, 0, 0, 0, 0, 3, 0, 0, 0, 64, 0, 0, 0, 1,
    0, 0, 0, 1, 0, 0, 0, 64, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0,
];

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
    fn(RootArgument),
) = (
    0x4e52_4b33_4f32_4546,
    1,
    1,
    "local_marker",
    "local_marker",
    fe2o3_kernel_local_marker,
);

#[cfg(any(
    feature = "reserved-capability-spoof",
    feature = "reserved-capability-control"
))]
#[used]
#[allow(
    non_upper_case_globals,
    clippy::redundant_static_lifetimes,
    clippy::type_complexity
)]
static __fe2o3_kernel_frontend_contract_v1_local_marker: (
    u64,
    u16,
    u16,
    &'static str,
    &'static [u8],
    fn(RootArgument),
) = (
    0x4146_4b33_4f32_4546,
    1,
    1,
    "local_marker",
    RESERVED_FRONTEND_CONTRACT,
    fe2o3_kernel_local_marker,
);

#[allow(dead_code)]
fn main() {}
