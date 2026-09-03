#![no_std]

use fe2o3_device::{DisjointSlice, kernel, thread};

#[kernel(
    typed,
    launch(required = [64, 1, 1], max = [64, 1, 1]),
)]
pub fn fill(mut out: DisjointSlice<f32>) {
    let idx = thread::index_1d();
    let Some(value) = out.get_mut(idx) else {
        return;
    };
    *value = 42.5;
}
