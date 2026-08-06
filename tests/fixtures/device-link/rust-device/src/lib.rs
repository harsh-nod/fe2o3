#![no_std]

use fe2o3_device::{DisjointSlice, device_export, device_import, kernel, thread};

#[device_export(
    symbol = "rust_accumulate_v1",
    target = "gfx942:sramecc+:xnack-",
    code_object = 5,
    effects = "none",
    semantic = "2222222222222222222222222222222222222222222222222222222222222222"
)]
/// Adds the invocation lane to an intermediate value.
///
/// # Safety
///
/// Callers must satisfy the exact target, code-object, physical ABI, and
/// semantic contract recorded by `device_export` above.
pub unsafe extern "C" fn rust_accumulate(value: u32, lane: u32) -> u32 {
    value.wrapping_add(lane)
}

#[device_import(
    symbol = "external_scale_bias_v1",
    target = "gfx942:sramecc+:xnack-",
    code_object = 5,
    effects = "none",
    semantic = "1111111111111111111111111111111111111111111111111111111111111111"
)]
unsafe extern "C" {
    pub fn external_scale_bias(value: u32, lane: u32) -> u32;
}

#[kernel]
pub fn bidirectional_device_ffi(input: &[u32], mut output: DisjointSlice<u32>) {
    let index = thread::index_1d();
    let lane = index.get();
    if let Some(destination) = output.get_mut(index) {
        let value = input[lane];
        // SAFETY: the direct-link contract binds the exact symbol, target,
        // physical ABI, effects, and semantic identity used by this fixture.
        *destination = unsafe { external_scale_bias(value, lane as u32) };
    }
}
