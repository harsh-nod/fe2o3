#[unsafe(no_mangle)]
pub fn fe2o3_kernel_ffi_import_consumer() {
    // SAFETY: this fixture exercises collection at the explicit FFI boundary.
    let _ = unsafe { ffi_import::call_external(7) };
}

#[used]
static __fe2o3_kernel_registration_ffi_import_consumer: (
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
    "ffi_import_consumer",
    "ffi_import_consumer",
    fe2o3_kernel_ffi_import_consumer,
);
