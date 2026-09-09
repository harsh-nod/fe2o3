#![no_std]

use fe2o3_device::{DisjointSlice, kernel, thread};
use issue272_device_helper::generic_device_helper;

#[inline(never)]
fn device_helper(value: u32) -> u32 {
    value + DEVICE_BIAS
}

static DEVICE_BIAS: u32 = 7;

#[kernel(typed)]
pub fn owned_kernel(mut output: DisjointSlice<u32>) {
    let index = thread::index_1d();
    if let Some(element) = output.get_mut(index) {
        let value = device_helper(index.get() as u32);
        *element = generic_device_helper::<7>(value);
    }
}

#[cfg(feature = "multiple-kernels")]
#[kernel(typed)]
pub fn second_owned_kernel(mut output: DisjointSlice<u32>) {
    let index = thread::index_1d();
    if let Some(element) = output.get_mut(index) {
        *element = device_helper(index.get() as u32);
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn retained_host_symbol(value: u32) -> u32 {
    host::owned_kernel(value)
}

mod host {
    #[inline(never)]
    pub(super) fn owned_kernel(value: u32) -> u32 {
        value + 11
    }
}

#[cfg(feature = "device-symbol-collision")]
#[unsafe(export_name = "owned_kernel")]
pub extern "C" fn colliding_host_export(value: u32) -> u32 {
    value
}

#[cfg(feature = "device-global-asm")]
core::arch::global_asm!(
    ".globl issue272_device_global_asm",
    "issue272_device_global_asm:",
    ".quad {kernel}",
    kernel = sym owned_kernel,
);
