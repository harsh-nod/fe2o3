//! Ordinary Rust fixture inputs, not compiler dispatch names.
use super::{DisjointSlice, kernel, thread};
use fe2o3_device::DeviceMath;

#[cfg(feature = "f32-exp")]
#[kernel(typed, launch(required = [64, 1, 1], max = [64, 1, 1]))]
pub fn f32_exp(mut output: DisjointSlice<f32>, value: f32) {
    if let Some(element) = output.get_mut(thread::index_1d()) {
        *element = DeviceMath::current().exp_f32(value);
    }
}

#[cfg(feature = "f32-helper-exp")]
#[inline(never)]
fn retained_exp(value: f32) -> f32 {
    DeviceMath::current().exp_f32(value)
}

#[cfg(feature = "f32-helper-exp")]
#[kernel(typed, launch(required = [64, 1, 1], max = [64, 1, 1]))]
pub fn f32_helper_exp(mut output: DisjointSlice<f32>, value: f32) {
    if let Some(element) = output.get_mut(thread::index_1d()) {
        *element = retained_exp(value);
    }
}
