use fe2o3_device::device_import;

#[device_import(
    symbol = "has_body",
    target = "gfx942",
    code_object = 5,
    effects = "none",
    semantic = "1111111111111111111111111111111111111111111111111111111111111111"
)]
pub unsafe extern "C" fn imported(value: u32) -> u32 {
    value
}

fn main() {}
