//! Fresh-source fixture body for the EXISTING production-extraction fixture.
//! The runner applies this file to its OWN source-only checkout's
//! src/ordered_program_v32.rs, keeping the existing manifest/device dependency.
//! It is not a second simulator kernel or a source-authority serialization.
use fe2o3_device::{DisjointSlice, amdgpu_ordered_program, kernel, thread};
#[kernel(typed, launch(required = [64, 1, 1], max = [64, 1, 1]))]
pub fn ordered_u32_program(mut output: DisjointSlice<u32>, a: u32, b: u32, c: u32) {
    let value = amdgpu_ordered_program! {
        gfx942_xnack_off_wave64;
        scratch(32); out(33); in(34) = a; in(35) = b; in(36) = c;
        xor(scratch, input0, input1);
        and(scratch, scratch, input2);
        xor(out, input1, scratch);
    };
    let index = thread::index_1d();
    if let Some(element) = output.get_mut(index) {
        *element = value;
    }
}
