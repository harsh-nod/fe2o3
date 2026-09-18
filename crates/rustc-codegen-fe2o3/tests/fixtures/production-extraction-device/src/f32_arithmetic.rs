//! Ordinary Rust test inputs. These are not compiler dispatch names.
use super::{DisjointSlice, kernel, thread};

#[cfg(feature = "f32-negate")]
#[kernel(typed, launch(required = [64, 1, 1], max = [64, 1, 1]))]
pub fn f32_negate(mut output: DisjointSlice<f32>, left: f32, _right: f32) {
    if let Some(element) = output.get_mut(thread::index_1d()) {
        *element = -left;
    }
}

#[cfg(feature = "f32-divide")]
#[kernel(typed, launch(required = [64, 1, 1], max = [64, 1, 1]))]
pub fn f32_divide(mut output: DisjointSlice<f32>, left: f32, right: f32) {
    if let Some(element) = output.get_mut(thread::index_1d()) {
        *element = left / right;
    }
}

#[cfg(feature = "f32-helper-negate")]
#[inline(never)]
fn retained_negate(left: f32, _right: f32) -> f32 {
    -left
}

#[cfg(feature = "f32-helper-negate")]
#[kernel(typed, launch(required = [64, 1, 1], max = [64, 1, 1]))]
pub fn f32_helper_negate(mut output: DisjointSlice<f32>, left: f32, right: f32) {
    if let Some(element) = output.get_mut(thread::index_1d()) {
        *element = retained_negate(left, right);
    }
}

#[cfg(feature = "f32-helper-divide")]
#[inline(never)]
fn retained_divide(left: f32, right: f32) -> f32 {
    left / right
}

#[cfg(feature = "f32-helper-divide")]
#[kernel(typed, launch(required = [64, 1, 1], max = [64, 1, 1]))]
pub fn f32_helper_divide(mut output: DisjointSlice<f32>, left: f32, right: f32) {
    if let Some(element) = output.get_mut(thread::index_1d()) {
        *element = retained_divide(left, right);
    }
}
