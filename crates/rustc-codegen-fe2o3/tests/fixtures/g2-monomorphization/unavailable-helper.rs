#![no_std]

#[inline(never)]
pub fn unavailable(value: u32) -> u32 {
    value + 1
}
