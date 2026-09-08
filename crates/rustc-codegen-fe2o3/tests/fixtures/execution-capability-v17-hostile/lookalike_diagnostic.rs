#![no_std]

use fe2o3_device::kernel;

unsafe fn from_raw_parts(
    _context: u32,
    _physical: *mut u32,
    _elements: usize,
    _obligation: u32,
) {
}

#[kernel(unsafe_provider(raw_memory))]
pub unsafe fn lookalike_raw_memory_provider() {
    unsafe { from_raw_parts(0, core::ptr::null_mut(), 4, 0) }
}
