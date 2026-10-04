#![no_std]

use fe2o3_device::{DeviceMath, DisjointSlice, kernel, thread};

#[kernel(
    typed,
    launch(required = [64, 1, 1], max = [64, 1, 1]),
)]
pub fn device_math_sqrt_f64(value: f64, mut output: DisjointSlice<f32>) {
    let math = DeviceMath::current();
    let root = math.sqrt_f32(value);
    if let Some(element) = output.get_mut(thread::index_1d()) {
        *element = root;
    }
}
