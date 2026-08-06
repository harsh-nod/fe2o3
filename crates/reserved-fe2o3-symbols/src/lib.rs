//! Reserved names and registration values shared by fe2o3 macros and the backend.
//!
//! Kernel registration is a compiler contract, not an authenticity boundary. The
//! backend validates that records are structurally correct and internally
//! consistent, but Rust source can reproduce the reserved names and field values.
//! Code compiled with the fe2o3 backend is therefore trusted to emit honest
//! registrations.

pub const RESERVED_ROOT: &str = "fe2o3_";
pub const KERNEL_PREFIX: &str = "fe2o3_kernel_";
pub const DEVICE_PREFIX: &str = "fe2o3_device_";
pub const DEVICE_EXTERN_PREFIX: &str = "fe2o3_device_extern_";

/// Final-path-segment prefix for kernel registration statics.
pub const KERNEL_REGISTRATION_PREFIX: &str = "__fe2o3_kernel_registration_";

/// ASCII `FE2O3KRN`, interpreted as a little-endian `u64`.
pub const KERNEL_REGISTRATION_MAGIC: u64 = 0x4e52_4b33_4f32_4546;
pub const KERNEL_REGISTRATION_VERSION_V1: u16 = 1;
/// An ordinary `#[kernel]` registration without a generated typed profile.
pub const KERNEL_REGISTRATION_KIND_KERNEL: u16 = 1;
/// A `#[kernel(typed)]` registration using the exact typed vecadd V1 profile.
pub const KERNEL_REGISTRATION_KIND_TYPED_VECADD_V1: u16 = 2;

/// V1 is an immutable `#[used]` static with this exact tuple shape:
///
/// `(u64 magic, u16 version, u16 kind, &str logical_name, &str export_name, fn pointer)`.
///
/// The function pointer is the direct association to the generated kernel item.
pub const KERNEL_REGISTRATION_V1_FIELD_COUNT: usize = 6;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kernel_registration_v1_values_are_stable() {
        assert_eq!(KERNEL_REGISTRATION_MAGIC.to_le_bytes(), *b"FE2O3KRN");
        assert_eq!(KERNEL_REGISTRATION_VERSION_V1, 1);
        assert_eq!(KERNEL_REGISTRATION_KIND_KERNEL, 1);
        assert_eq!(KERNEL_REGISTRATION_KIND_TYPED_VECADD_V1, 2);
        assert_ne!(
            KERNEL_REGISTRATION_KIND_KERNEL,
            KERNEL_REGISTRATION_KIND_TYPED_VECADD_V1
        );
        assert_eq!(KERNEL_REGISTRATION_V1_FIELD_COUNT, 6);
    }
}
