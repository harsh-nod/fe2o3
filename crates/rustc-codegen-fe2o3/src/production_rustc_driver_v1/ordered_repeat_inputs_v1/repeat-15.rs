//! Fresh source qualification input; not a captured or admitted compiler owner.
use fe2o3_device::{DisjointSlice, kernel, thread};

#[kernel(typed, launch(required = [64, 1, 1], max = [64, 1, 1]))]
pub fn publish_region(mut output: DisjointSlice<u32>, a: u32, b: u32, c: u32) {
    let value = fe2o3_device::amdgpu_ordered_program! {
        gfx942_xnack_off_wave64;
        scratch(8); out(9); in(10) = a; in(11) = b; in(12) = c;
        init { mov(out, input0); }
        repeat(15) { add(out, out, input1); }
    };
    let index = thread::index_1d();
    if let Some(element) = output.get_mut(index) {
        *element = value;
    }
}
