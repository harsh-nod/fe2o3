//! Actual Rust reference binding for the closed gfx942 u32 instruction slice.
//! Wrong-opcode/constant features keep this independent reference unchanged.
//! Reaching a proof request is not proof success or full artifact admission.

use super::{DisjointSlice, kernel, thread};
use fe2o3_device::amdgpu_asm;

#[cfg(any(
    all(
        feature = "assembly-reference-positive",
        feature = "assembly-reference-wrong-opcode"
    ),
    all(
        feature = "assembly-reference-positive",
        feature = "assembly-reference-wrong-constant"
    ),
    all(
        feature = "assembly-reference-wrong-opcode",
        feature = "assembly-reference-wrong-constant"
    ),
))]
compile_error!("select exactly one assembly-reference acceptance feature");

fn cpu_assembly_reference(_point: usize, out: &mut u32, a: u32, b: u32) {
    let difference = a.wrapping_add(b).wrapping_sub(b);
    *out = ((difference ^ b) & 255) | 256;
}

#[kernel(
    typed,
    reference = cpu_assembly_reference,
    launch(required = [64, 1, 1], max = [64, 1, 1])
)]
pub fn assembly_reference_chain(mut out: DisjointSlice<u32>, a: u32, b: u32) {
    let moved = amdgpu_asm!(v_mov_b32(a));
    let moved_again = amdgpu_asm!(v_mov_b32(moved));
    let sum = amdgpu_asm!(v_add_u32(moved_again, b));
    let difference = amdgpu_asm!(v_sub_u32(sum, b));
    #[cfg(not(feature = "assembly-reference-wrong-opcode"))]
    let toggled = amdgpu_asm!(v_xor_b32(difference, b));
    #[cfg(feature = "assembly-reference-wrong-opcode")]
    let toggled = amdgpu_asm!(v_and_b32(difference, b));
    let low = amdgpu_asm!(v_and_b32(toggled, 255));
    #[cfg(not(feature = "assembly-reference-wrong-constant"))]
    let value = amdgpu_asm!(v_or_b32(low, 256));
    #[cfg(feature = "assembly-reference-wrong-constant")]
    let value = amdgpu_asm!(v_or_b32(low, 512));
    let index = thread::index_1d();
    if let Some(output) = out.get_mut(index) {
        *output = value;
    }
}
