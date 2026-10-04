use fe2o3_device::{
    DeviceGlobalConstPtr, DeviceGlobalMutPtr, device_export, device_import,
};

#[device_export(
    symbol = "saxpy_export_v1",
    target = "gfx942:sramecc+:xnack-",
    code_object = 5,
    effects = "read_global,write_global",
    semantic = "1111111111111111111111111111111111111111111111111111111111111111"
)]
pub unsafe extern "C" fn saxpy_export(
    input: DeviceGlobalConstPtr<f32>,
    output: DeviceGlobalMutPtr<f32>,
    count: u64,
) -> u32 {
    let _ = (input, output, count);
    0
}

#[device_import(
    symbol = "saxpy_import_v1",
    target = "gfx942:sramecc+:xnack-",
    code_object = 5,
    effects = "read_global,write_global",
    semantic = "2222222222222222222222222222222222222222222222222222222222222222"
)]
unsafe extern "C" {
    pub fn saxpy_import(
        input: DeviceGlobalConstPtr<f32>,
        output: DeviceGlobalMutPtr<f32>,
        count: u64,
    ) -> u32;
}

#[unsafe(export_name = "saxpy_import_v1")]
unsafe extern "C" fn host_link_fixture(
    _input: DeviceGlobalConstPtr<f32>,
    _output: DeviceGlobalMutPtr<f32>,
    _count: u64,
) -> u32 {
    0
}

fn main() {
    let _: unsafe extern "C" fn(
        DeviceGlobalConstPtr<f32>,
        DeviceGlobalMutPtr<f32>,
        u64,
    ) -> u32 = saxpy_import;
}
