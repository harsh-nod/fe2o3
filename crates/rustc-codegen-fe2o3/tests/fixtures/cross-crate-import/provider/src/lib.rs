use fe2o3_device::{DisjointSlice, device_export, kernel, thread};

#[kernel(typed)]
pub fn external_vecadd(a: &[f32], b: &[f32], mut output: DisjointSlice<f32>) {
    let index = thread::index_1d();
    let offset = index.get();
    if let Some(value) = output.get_mut(index) {
        *value = a[offset] + b[offset];
    }
}

#[device_export(
    symbol = "fe2o3_external_increment_v1",
    target = "gfx942:xnack-",
    code_object = 6,
    effects = "none",
    semantic = "3131313131313131313131313131313131313131313131313131313131313131"
)]
pub unsafe extern "C" fn external_increment(value: u32) -> u32 {
    value ^ 1
}
