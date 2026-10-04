use fe2o3_device::device_export;

#[device_export(
    symbol = "unsafe_boundary",
    target = "gfx942",
    code_object = 5,
    effects = "none",
    semantic = "1111111111111111111111111111111111111111111111111111111111111111"
)]
pub extern "C" fn exported(value: u32) -> u32 {
    value
}

fn main() {}
