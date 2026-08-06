#[unsafe(no_mangle)]
pub fn fe2o3_kernel_ffi_consumer() {
    // SAFETY: this fixture exercises the explicit device FFI boundary only.
    let _ = unsafe { ffi_export::cross_crate_device_helper(7) };
}

#[used]
static __fe2o3_kernel_registration_ffi_consumer: (
    u64,
    u16,
    u16,
    &'static str,
    &'static str,
    fn(),
) = (
    0x4e52_4b33_4f32_4546,
    1,
    1,
    "ffi_consumer",
    "ffi_consumer",
    fe2o3_kernel_ffi_consumer,
);

fn main() {}
