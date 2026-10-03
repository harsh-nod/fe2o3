#![cfg_attr(target_arch = "amdgpu", no_std)]
#![cfg_attr(target_arch = "amdgpu", no_main)]

use fe2o3_device::{DisjointSlice, kernel, thread};

#[kernel(typed, launch(required = [64, 1, 1], max = [64, 1, 1], max_grid = [1, 1, 1]))]
pub fn explicit_bin_export(value: u32, mut output: DisjointSlice<u32>) {
    if let Some(slot) = output.get_mut(thread::index_1d()) {
        *slot = value.wrapping_add(17);
    }
}

#[cfg(target_arch = "amdgpu")]
#[panic_handler]
fn panic(_info: &core::panic::PanicInfo<'_>) -> ! {
    loop {}
}

#[cfg(not(target_arch = "amdgpu"))]
fn main() {}
