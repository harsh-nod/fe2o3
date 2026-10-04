use fe2o3_device::device_import;

#[device_import(
    symbol = "unsafe_import",
    target = "gfx942",
    code_object = 5,
    effects = "none",
    semantic = "1111111111111111111111111111111111111111111111111111111111111111"
)]
unsafe extern "C" {
    pub fn imported(value: u32) -> u32;
}

fn main() {
    let _ = imported(1);
}
