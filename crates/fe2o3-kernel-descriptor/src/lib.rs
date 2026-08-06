#![forbid(unsafe_code)]
#![doc = include_str!("../README.md")]

mod decode;
mod digest;
mod encode;
mod error;
mod model;
mod requirements_v2;
mod wire_v2;

pub use decode::decode_device_descriptor_table_v1;
pub use digest::{
    CANONICAL_CODE_OBJECT_DOMAIN_V1, CanonicalCodeObjectDigest, DEVICE_DESCRIPTOR_TABLE_DOMAIN_V1,
    DEVICE_LAYOUT_DOMAIN_V1, DeviceDescriptorTableDigest, DeviceLayoutIdentity,
    KERNEL_DESCRIPTOR_DOMAIN_V1, KernelDescriptorDigest, RUST_TYPE_DOMAIN_V1, RustTypeIdentity,
};
pub use encode::{
    CANONICAL_CODE_OBJECT_DIGEST_OFFSET, DEVICE_DESCRIPTOR_MAGIC, DEVICE_DESCRIPTOR_VERSION,
    encode_device_descriptor_table_v1,
};
pub use error::{DecodeError, ValidationError};
pub use model::{
    AccessMode, AliasSemantics, BlockSizeV1, BuildEvidenceV1, CapabilityV1, CodeObjectVersion,
    CompilerIdentityV1, DeviceDescriptorTableV1, DeviceLayoutDescriptorV1, DeviceLayoutRecordV1,
    DeviceTargetV1, DimensionsV1, EvidenceDigest, EvidenceIdentity, KernelAbiLayoutV1,
    KernelDescriptorV1, KernelId, LaunchConstraintsV1, LogicalArgumentV1, MAX_ARGUMENTS_PER_KERNEL,
    MAX_DESCRIPTOR_TABLE_BYTES, MAX_KERNARG_SEGMENT_BYTES, MAX_KERNELS, MAX_LAYOUT_RECORDS,
    MAX_NAME_BYTES, MAX_PHYSICAL_COMPONENTS_PER_KERNEL, MAX_TEXT_BYTES, MAX_TYPE_RECORDS,
    OwnershipSemantics, PhysicalAbiComponentKind, ProducerIdentityV1, ScalarTypeV1,
    SourceTypeDescriptorV1, SourceTypeRecordV1, Text, ValidName,
};
pub use requirements_v2::{
    AtomicRequirementsV2, DeviceDescriptorTableV2, KernelTargetRequirementsV2, LdsRequirementsV2,
    RequiredWavefrontWidthV2, SynchronizationRequirementsV2,
};
pub use wire_v2::{
    CANONICAL_CODE_OBJECT_DIGEST_OFFSET_V2, DEVICE_DESCRIPTOR_VERSION_V2,
    decode_device_descriptor_table_v2, encode_device_descriptor_table_v2,
};

#[cfg(test)]
mod tests;
