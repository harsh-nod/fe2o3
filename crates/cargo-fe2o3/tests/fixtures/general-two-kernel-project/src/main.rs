use core::ops::{Add, Sub};

use fe2o3_device::{DeviceMath, DisjointSlice, kernel, thread};

#[inline(never)]
fn alternating_adjust<T>(mut value: T, positive_step: T, negative_step: T, iterations: u32) -> T
where
    T: Copy + Add<Output = T> + Sub<Output = T>,
{
    let mut iteration = 0;
    while iteration < iterations {
        if iteration & 1 == 0 {
            value = value + positive_step;
        } else {
            value = value - negative_step;
        }
        iteration += 1;
    }
    value
}

#[kernel]
pub fn adjusted_root(input: &[f32], mut output: DisjointSlice<f32>) {
    let index = thread::index_1d();
    let offset = index.get();
    if let Some(value) = output.get_mut(index) {
        let adjusted = alternating_adjust(input[offset], 0.25, 0.125, 3);
        let math = DeviceMath::current();
        *value = math.sqrt_f32(adjusted * adjusted + 1.0);
    }
}

#[kernel]
pub fn adjusted_sine(input: &[f32], mut output: DisjointSlice<f32>) {
    let index = thread::index_1d();
    let offset = index.get();
    if let Some(value) = output.get_mut(index) {
        let adjusted = alternating_adjust(input[offset], 0.5, 0.25, 4);
        let math = DeviceMath::current();
        *value = math.sin_f32(adjusted);
    }
}

fn main() {}
