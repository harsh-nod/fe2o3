unsafe extern "C" {
    #[doc = "__MARKER__"]
    #[link_name = "cross_crate_external_add_v1"]
    pub fn cross_crate_external_add(value: u32) -> u32;
}

#[used]
static __fe2o3_device_ffi_registration_v1_fixture: (
    u64,
    u16,
    u16,
    &'static str,
    &'static str,
    &'static str,
    u16,
    &'static str,
    &'static str,
    &'static str,
    &'static str,
    unsafe extern "C" fn(u32) -> u32,
) = (
    0x4946_4633_4f32_4546,
    1,
    1,
    "__CONTRACT__",
    "cross_crate_external_add_v1",
    "C",
    5,
    "gfx1100",
    "C(u32[size=4,align=4])->u32[size=4,align=4]",
    "none",
    "4444444444444444444444444444444444444444444444444444444444444444",
    cross_crate_external_add,
);

#[inline(always)]
pub unsafe fn call_external(value: u32) -> u32 {
    // SAFETY: the caller owns the reviewed device FFI contract.
    unsafe { cross_crate_external_add(value) }
}
