#[cfg(any(
    feature = "multi-kernel",
    not(any(
        feature = "prefix-spoof",
        feature = "malformed-registration",
        feature = "unknown-registration-version",
        feature = "duplicate-logical-name",
        feature = "duplicate-export-name",
        feature = "multi-kernel",
    )),
))]
use device_api::{DisjointSlice, thread};
#[cfg(any(
    feature = "multi-kernel",
    not(any(
        feature = "prefix-spoof",
        feature = "malformed-registration",
        feature = "unknown-registration-version",
        feature = "duplicate-logical-name",
        feature = "duplicate-export-name",
        feature = "multi-kernel",
    )),
))]
use fe2o3_macros::kernel;

#[cfg(any(
    feature = "malformed-registration",
    feature = "unknown-registration-version",
    feature = "duplicate-logical-name",
    feature = "duplicate-export-name",
))]
const KERNEL_REGISTRATION_MAGIC: u64 = 0x4e52_4b33_4f32_4546;
#[cfg(any(
    feature = "malformed-registration",
    feature = "unknown-registration-version",
    feature = "duplicate-logical-name",
    feature = "duplicate-export-name",
))]
const KERNEL_REGISTRATION_KIND_KERNEL: u16 = 1;

#[cfg(not(any(
    feature = "prefix-spoof",
    feature = "malformed-registration",
    feature = "unknown-registration-version",
    feature = "duplicate-logical-name",
    feature = "duplicate-export-name",
    feature = "multi-kernel",
)))]
#[kernel]
pub fn renamed_genuine(mut output: DisjointSlice<f32>) {
    let index = thread::index_1d();
    if let Some(value) = output.get_mut(index) {
        *value = 42.5;
    }
}

#[cfg(feature = "prefix-spoof")]
#[unsafe(no_mangle)]
pub fn fe2o3_kernel_prefix_spoof() {}

#[cfg(feature = "malformed-registration")]
#[unsafe(no_mangle)]
pub fn fe2o3_kernel_malformed_registration() {}

#[cfg(feature = "malformed-registration")]
#[allow(non_upper_case_globals)]
#[used]
static __fe2o3_kernel_registration_malformed_registration: (
    u64,
    u16,
    u16,
    &'static str,
    &'static str,
    fn(),
) = (
    0x4e52_4b33_4f32_4547,
    1,
    KERNEL_REGISTRATION_KIND_KERNEL,
    "malformed_registration",
    "malformed_registration",
    fe2o3_kernel_malformed_registration,
);

#[cfg(feature = "unknown-registration-version")]
#[unsafe(no_mangle)]
pub fn fe2o3_kernel_unknown_registration_version() {}

#[cfg(feature = "unknown-registration-version")]
#[allow(non_upper_case_globals)]
#[used]
static __fe2o3_kernel_registration_unknown_registration_version: (
    u64,
    u16,
    u16,
    &'static str,
    &'static str,
    fn(),
) = (
    KERNEL_REGISTRATION_MAGIC,
    3,
    KERNEL_REGISTRATION_KIND_KERNEL,
    "unknown_registration_version",
    "unknown_registration_version",
    fe2o3_kernel_unknown_registration_version,
);

#[cfg(feature = "duplicate-logical-name")]
mod duplicate_logical_a {
    use super::{KERNEL_REGISTRATION_KIND_KERNEL, KERNEL_REGISTRATION_MAGIC};

    #[unsafe(no_mangle)]
    pub fn fe2o3_kernel_duplicate_logical_a() {}

    #[allow(non_upper_case_globals)]
    #[used]
    static __fe2o3_kernel_registration_duplicate_logical: (
        u64,
        u16,
        u16,
        &'static str,
        &'static str,
        fn(),
    ) = (
        KERNEL_REGISTRATION_MAGIC,
        1,
        KERNEL_REGISTRATION_KIND_KERNEL,
        "duplicate_logical",
        "duplicate_logical_a",
        fe2o3_kernel_duplicate_logical_a,
    );
}

#[cfg(feature = "duplicate-logical-name")]
mod duplicate_logical_b {
    use super::{KERNEL_REGISTRATION_KIND_KERNEL, KERNEL_REGISTRATION_MAGIC};

    #[unsafe(no_mangle)]
    pub fn fe2o3_kernel_duplicate_logical_b() {}

    #[allow(non_upper_case_globals)]
    #[used]
    static __fe2o3_kernel_registration_duplicate_logical: (
        u64,
        u16,
        u16,
        &'static str,
        &'static str,
        fn(),
    ) = (
        KERNEL_REGISTRATION_MAGIC,
        1,
        KERNEL_REGISTRATION_KIND_KERNEL,
        "duplicate_logical",
        "duplicate_logical_b",
        fe2o3_kernel_duplicate_logical_b,
    );
}

#[cfg(feature = "duplicate-export-name")]
#[unsafe(no_mangle)]
pub fn fe2o3_kernel_duplicate_export() {}

#[cfg(feature = "duplicate-export-name")]
mod duplicate_export_a {
    use super::{
        KERNEL_REGISTRATION_KIND_KERNEL, KERNEL_REGISTRATION_MAGIC, fe2o3_kernel_duplicate_export,
    };

    #[allow(non_upper_case_globals)]
    #[used]
    static __fe2o3_kernel_registration_duplicate_export_a: (
        u64,
        u16,
        u16,
        &'static str,
        &'static str,
        fn(),
    ) = (
        KERNEL_REGISTRATION_MAGIC,
        1,
        KERNEL_REGISTRATION_KIND_KERNEL,
        "duplicate_export_a",
        "duplicate_export",
        fe2o3_kernel_duplicate_export,
    );
}

#[cfg(feature = "duplicate-export-name")]
mod duplicate_export_b {
    use super::{
        KERNEL_REGISTRATION_KIND_KERNEL, KERNEL_REGISTRATION_MAGIC, fe2o3_kernel_duplicate_export,
    };

    #[allow(non_upper_case_globals)]
    #[used]
    static __fe2o3_kernel_registration_duplicate_export_b: (
        u64,
        u16,
        u16,
        &'static str,
        &'static str,
        fn(),
    ) = (
        KERNEL_REGISTRATION_MAGIC,
        1,
        KERNEL_REGISTRATION_KIND_KERNEL,
        "duplicate_export_b",
        "duplicate_export",
        fe2o3_kernel_duplicate_export,
    );
}

#[cfg(feature = "multi-kernel")]
#[kernel]
pub fn zeta(mut output: DisjointSlice<f32>) {
    let index = thread::index_1d();
    if let Some(value) = output.get_mut(index) {
        *value = 2.0;
    }
}

#[cfg(feature = "multi-kernel")]
#[kernel]
pub fn alpha(mut output: DisjointSlice<f32>) {
    let index = thread::index_1d();
    if let Some(value) = output.get_mut(index) {
        *value = 1.0;
    }
}

fn main() {}
