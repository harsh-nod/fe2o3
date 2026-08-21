#![no_std]

use fe2o3_device::{DisjointSlice, kernel, thread};

#[kernel(
    typed,
    namespace = "8d412ec11570c99d56c313012ca2372d2a0e0c1242792aa17f5a9a1b3962385b"
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
    namespace = "9a9c2e4a658f2594fa575abf0647cc3074b13f3252ad9c0fd19c843f237db547"
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
