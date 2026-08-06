use fe2o3_device::device_export;

#[device_export(
    symbol = "reference_abi",
    target = "gfx942",
    code_object = 5,
    effects = "read_global",
    semantic = "1111111111111111111111111111111111111111111111111111111111111111"
)]
pub unsafe extern "C" fn exported(value: &u32) -> u32 {
    *value
}

fn main() {}
