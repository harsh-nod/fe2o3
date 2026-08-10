use fe2o3_device::amdgpu_asm;

fn main() {
    let _ = amdgpu_asm!(global_load_dword(0u32));
}
