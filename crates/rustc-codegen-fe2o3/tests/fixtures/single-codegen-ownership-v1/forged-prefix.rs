#![crate_type = "lib"]

#[unsafe(export_name = "__fe2o3_host_kernel_v1_aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa")]
pub extern "C" fn forged_device_root() -> u32 {
    17
}

#[unsafe(no_mangle)]
pub extern "C" fn retained_host_symbol() -> u32 {
    23
}
