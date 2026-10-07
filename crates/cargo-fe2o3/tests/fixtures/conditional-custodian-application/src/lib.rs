#![cfg_attr(target_arch = "amdgpu", no_std)]

use fe2o3_device::{WriteOnlyDisjointSlice, kernel, thread};

#[kernel(
    typed,
    reference = reference,
    launch(required = [64, 1, 1], max = [64, 1, 1])
)]
pub fn fill_write_only(mut output: WriteOnlyDisjointSlice<u32>) {
    let index = thread::index_1d();
    let value = index.get() as u32;
    let _ = output.write(index, value);
}

fn reference(point: usize, output: &mut u32) {
    *output = point as u32;
}
