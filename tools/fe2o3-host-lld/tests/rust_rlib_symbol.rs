#![no_std]

#[no_mangle]
pub extern "C" fn fe2o3_rust_rlib_symbol(value: usize) -> usize {
    value + 1
}
