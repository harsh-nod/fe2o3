use fe2o3_device::{
    amdgpu_asm, clock32, debugtrap, gpu_assert, gpu_printf, profiling_marker, trap,
};

fn bounded_contract(lhs: u32, rhs: u32) -> u32 {
    let moved = amdgpu_asm!(v_mov_b32(lhs));
    let sum = amdgpu_asm!(v_add_u32(moved, rhs));
    let difference = amdgpu_asm!(v_sub_u32(sum, rhs));
    let masked = amdgpu_asm!(v_and_b32(difference, rhs));
    let merged = amdgpu_asm!(v_or_b32(masked, rhs));
    let result = amdgpu_asm!(v_xor_b32(merged, lhs));
    gpu_printf!("result={} clock={}\n", result, clock32());
    gpu_assert!(result == result, "reflexive result");
    profiling_marker!(73);
    if false {
        debugtrap();
        trap();
    }
    result
}

fn main() {
    let _ = bounded_contract as fn(u32, u32) -> u32;
}
