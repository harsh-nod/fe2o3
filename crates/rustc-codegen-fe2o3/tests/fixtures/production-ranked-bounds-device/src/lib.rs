#![no_std]

use fe2o3_device::{DisjointSlice, kernel, thread};

#[kernel(
    typed,
    namespace = "eb5edfb2b4d2dee01a64d34a34aa530ba7a00c5d098c28ec4d10d51a49373799"
)]
#[cfg(not(feature = "oob"))]
pub fn copy_static(value: f32, mut output: DisjointSlice<f32>) {
    let input = [value; 64];
    let selected = input[63];
    if let Some(element) = output.get_mut(thread::index_1d()) {
        *element = selected;
    }
}

#[kernel(
    typed,
    namespace = "116ae28bde3033fc771d94f491487a788b572f36afe75ba33b786fa463e01ab6"
)]
#[cfg(feature = "oob")]
#[allow(unconditional_panic)]
pub fn copy_static(value: f32, mut output: DisjointSlice<f32>) {
    let input = [value; 64];
    let selected = input[64];
    if let Some(element) = output.get_mut(thread::index_1d()) {
        *element = selected;
    }
}
