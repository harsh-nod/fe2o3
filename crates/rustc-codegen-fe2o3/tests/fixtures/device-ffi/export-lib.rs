#[inline(always)]
#[doc = "__MARKER__"]
#[unsafe(export_name = "cross_crate_device_helper_v1")]
pub unsafe extern "C" fn cross_crate_device_helper(value: u32) -> u32 {
    value.wrapping_add(1)
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
    2,
    "__CONTRACT__",
    "cross_crate_device_helper_v1",
    "C",
    5,
    "gfx1100",
    "C(u32[size=4,align=4])->u32[size=4,align=4]",
    "none",
    "3333333333333333333333333333333333333333333333333333333333333333",
    cross_crate_device_helper,
);
