use fe2o3_device::{import_device, import_kernel};

import_kernel!(external_vecadd, provider::__fe2o3_kernel_marker_external_vecadd);
import_device!(
    external_increment,
    provider::__fe2o3_device_export_marker_external_increment
);

pub fn retain_imported_api() {
    let _ = provider::external_increment as unsafe extern "C" fn(u32) -> u32;
}
