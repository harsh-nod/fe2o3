use sha2::{Digest, Sha256};

use crate::{
    DeviceDescriptorTableV1, DeviceLayoutDescriptorV1, KernelDescriptorV1, SourceTypeDescriptorV1,
    ValidationError,
};

pub const RUST_TYPE_DOMAIN_V1: &[u8] = b"FE2O3/RUST-TYPE/V1\0";
pub const DEVICE_LAYOUT_DOMAIN_V1: &[u8] = b"FE2O3/DEVICE-LAYOUT/V1\0";
pub const KERNEL_DESCRIPTOR_DOMAIN_V1: &[u8] = b"FE2O3/KERNEL-DESCRIPTOR/V1\0";
pub const DEVICE_DESCRIPTOR_TABLE_DOMAIN_V1: &[u8] = b"FE2O3/DEVICE-DESCRIPTOR-TABLE/V1\0";
pub const CANONICAL_CODE_OBJECT_DOMAIN_V1: &[u8] = b"FE2O3/AMDHSA-CODE-OBJECT/V1\0";

fn domain_hash(domain: &[u8], payload: &[u8]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(domain);
    hasher.update((payload.len() as u64).to_le_bytes());
    hasher.update(payload);
    hasher.finalize().into()
}

macro_rules! digest_type {
    ($name:ident) => {
        #[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
        #[repr(transparent)]
        pub struct $name([u8; 32]);

        impl $name {
            pub const fn from_bytes(bytes: [u8; 32]) -> Self {
                Self(bytes)
            }

            pub const fn as_bytes(&self) -> &[u8; 32] {
                &self.0
            }
        }
    };
}

digest_type!(RustTypeIdentity);
digest_type!(DeviceLayoutIdentity);
digest_type!(KernelDescriptorDigest);
digest_type!(DeviceDescriptorTableDigest);
digest_type!(CanonicalCodeObjectDigest);

impl RustTypeIdentity {
    pub fn for_descriptor(descriptor: &SourceTypeDescriptorV1) -> Self {
        Self(domain_hash(
            RUST_TYPE_DOMAIN_V1,
            &crate::encode::encode_source_type_descriptor(descriptor),
        ))
    }
}

impl DeviceLayoutIdentity {
    pub fn for_descriptor(descriptor: &DeviceLayoutDescriptorV1) -> Self {
        Self(domain_hash(
            DEVICE_LAYOUT_DOMAIN_V1,
            &crate::encode::encode_device_layout_descriptor(descriptor),
        ))
    }
}

impl KernelDescriptorDigest {
    pub fn calculate(kernel: &KernelDescriptorV1) -> Self {
        Self(domain_hash(
            KERNEL_DESCRIPTOR_DOMAIN_V1,
            &crate::encode::encode_kernel_descriptor(kernel),
        ))
    }
}

impl DeviceDescriptorTableDigest {
    pub fn calculate(table: &DeviceDescriptorTableV1) -> Result<Self, ValidationError> {
        Ok(Self(domain_hash(
            DEVICE_DESCRIPTOR_TABLE_DOMAIN_V1,
            &crate::encode::encode_device_descriptor_table_v1(table)?,
        )))
    }
}

impl CanonicalCodeObjectDigest {
    /// Hashes complete code-object bytes whose one trusted fixed digest field
    /// has already been zeroed by a later ELF-aware producer.
    ///
    /// This operation does not locate or validate that field and does not
    /// authenticate the bytes. It is not an artifact payload digest.
    pub fn calculate_from_canonicalized_hsaco(bytes: &[u8]) -> Self {
        Self(domain_hash(CANONICAL_CODE_OBJECT_DOMAIN_V1, bytes))
    }
}

#[cfg(test)]
pub(crate) fn domain_hash_for_test(domain: &[u8], payload: &[u8]) -> [u8; 32] {
    domain_hash(domain, payload)
}
