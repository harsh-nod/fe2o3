#![no_std]

#[inline(never)]
pub fn cross_crate_bias(value: f32) -> f32 {
    value + 11.0
}
