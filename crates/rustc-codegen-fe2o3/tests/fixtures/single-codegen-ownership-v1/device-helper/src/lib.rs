#![no_std]

#[inline(never)]
pub fn generic_device_helper<const BIAS: u32>(value: u32) -> u32 {
    value + BIAS
}
