use fe2o3_device::device_export;

#[device_export(
    symbol = "generic_root",
    target = "gfx942",
    code_object = 5,
    effects = "none",
    semantic = "1111111111111111111111111111111111111111111111111111111111111111"
)]
pub unsafe extern "C" fn exported<T>(value: T) {
    let _ = value;
}

fn main() {}
