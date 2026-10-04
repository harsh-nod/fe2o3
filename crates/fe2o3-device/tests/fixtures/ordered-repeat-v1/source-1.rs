// Future normal-source fixture, not a claim of completed frontend admission.
#![no_std]
use fe2o3_device::{DisjointSlice, amdgpu_ordered_program, kernel, thread};

#[kernel(typed, launch(required = [64, 1, 1], max = [64, 1, 1]))]
pub fn ordered_repeat_u32(
    mut output: DisjointSlice<u32>,
    a: u32,
    b: u32,
    c: u32,
) {
    let result = amdgpu_ordered_program! {
        gfx942_xnack_off_wave64;
        scratch(32); out(33); in(34) = a; in(35) = b; in(36) = c;
        init { mov(out, input0); }
        repeat(1) { add(out, out, input1); }
    };
    if let Some(element) = output.get_mut(thread::index_1d()) {
        *element = result;
    }
}
