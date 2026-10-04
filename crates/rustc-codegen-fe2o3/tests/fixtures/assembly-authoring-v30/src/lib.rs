#![no_std]

use fe2o3_device::{DisjointSlice, amdgpu_asm, kernel, thread};

/// Seven static occurrences exercise all six typed integer instructions.
/// The edited feature changes only the final instruction's second operand.
#[kernel(typed, launch(required = [64, 1, 1], max = [64, 1, 1]))]
pub fn assembly_chain(mut out: DisjointSlice<u32>, a: u32, b: u32) {
    let moved = amdgpu_asm!(v_mov_b32(a));
    let moved_again = amdgpu_asm!(v_mov_b32(moved));
    let sum = amdgpu_asm!(v_add_u32(moved_again, b));
    let difference = amdgpu_asm!(v_sub_u32(sum, b));
    let toggled = amdgpu_asm!(v_xor_b32(difference, b));
    let low = amdgpu_asm!(v_and_b32(toggled, 255));
    #[cfg(not(feature = "edited"))]
    let result = amdgpu_asm!(v_or_b32(low, 256));
    #[cfg(feature = "edited")]
    let result = amdgpu_asm!(v_or_b32(low, 512));
    let index = thread::index_1d();
    if let Some(output) = out.get_mut(index) {
        *output = result;
    }
}
