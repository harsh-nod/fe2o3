use fe2o3_device::device_export;

#[device_export(
    symbol = "write_without_mutable_pointer",
    target = "gfx942",
    code_object = 5,
    effects = "write_global",
    semantic = "1111111111111111111111111111111111111111111111111111111111111111"
)]
pub unsafe extern "C" fn effect_mismatch(
    _input: fe2o3_device::DeviceGlobalConstPtr<u32>,
) {
}

fn main() {}
