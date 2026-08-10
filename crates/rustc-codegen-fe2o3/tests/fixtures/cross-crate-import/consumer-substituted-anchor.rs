use fe2o3_device::import_kernel;

import_kernel!(external_vecadd, provider::__fe2o3_kernel_marker_external_vecadd);

// A same-type anchor cannot substitute a different contract identity.
#[used]
static __fe2o3_cross_crate_device_export_anchor_v1_substitution: (
    u64, u16, &'static str, unsafe extern "C" fn(u32) -> u32,
) = (
    0x5644_5833_4f32_4546, 1,
    "0000000000000000000000000000000000000000000000000000000000000000",
    provider::external_increment,
);

pub fn retain_imported_api() {}
