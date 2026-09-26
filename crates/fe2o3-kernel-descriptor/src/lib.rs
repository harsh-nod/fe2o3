#![forbid(unsafe_code)]
#![doc = include_str!("../README.md")]

/// Stable compiler name recorded by the production rustc descriptor emitter.
pub const RUSTC_CODEGEN_FE2O3_COMPILER_NAME_V1: &str = "rustc-codegen-fe2o3";
/// Stable producer name for the workload-neutral production V3 pipeline.
pub const RUSTC_CODEGEN_FE2O3_PRODUCTION_V3_PRODUCER_NAME_V1: &str =
    "rustc-codegen-fe2o3-production-v3";

pub mod conditional_invocation;
mod conditional_invocation_codec;
mod conditional_invocation_codec_v1;
mod conditional_invocation_codec_v2;
mod conditional_invocation_rows_v1;
mod conditional_invocation_schema;
mod conditional_invocation_v1;
mod conditional_invocation_v2;
mod conditional_invocation_validate_v1;
mod conditional_v4;
mod decode;
mod digest;
mod encode;
mod error;
pub mod ffi_contract;
mod launch_policy;
mod model;
mod nominal_v3;
mod requirements_v2;
mod wire_common;
mod wire_v2;
mod wire_v3;
mod wire_v4;
mod wire_v4_join;

pub use conditional_invocation::*;
pub use conditional_v4::*;
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
pub use launch_policy::{
    AdmittedKernelFamilyVariantV1, GFX942_MAX_FLAT_WORKGROUP_SIZE_V1,
    GFX942_MAX_KERNEL_FAMILY_VARIANTS_V1, GFX942_MAX_WAVES_PER_EXECUTION_UNIT_V1,
    GFX942_XNACK_MINUS_TARGET_V1, Gfx942KernelFamilyBundleV1, Gfx942LaunchBoundsV1,
    KernelFamilyIdentityV1, KernelFamilyPolicyErrorV1, KernelFamilyVariantDescriptorV1,
    KernelInterfaceIdentityV1, KernelLaunchPolicyIdentityV1, TypedKernelFamilyVariantExpectationV1,
};
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
pub use nominal_v3::{
    ArgumentCursorV3, CANONICAL_CODE_OBJECT_DIGEST_OFFSET_V3, CapabilityCursorV3,
    DESCRIPTOR_ENCODER_SCRATCH_STORAGE_V3, DESCRIPTOR_QUERY_STORAGE_V3,
    DESCRIPTOR_READER_SCRATCH_STORAGE_V3, DESCRIPTOR_TABLE_VIEW_STORAGE_V3,
    DEVICE_DESCRIPTOR_TABLE_DOMAIN_V3, DEVICE_DESCRIPTOR_VERSION_V3, DescriptorWireErrorV3,
    DeviceDescriptorTableInputV3, DeviceDescriptorTableV3, KernelDescriptorInputV3,
    KernelDescriptorRefV3, LogicalArgumentInputV3, LogicalArgumentRefV3, PhysicalComponentV3,
    RUST_TYPE_DOMAIN_V3, SourceTypeDescriptorV3, SourceTypeRecordV3, device_layout_record_v3,
    encode_device_descriptor_table_v3, encoded_device_descriptor_table_v3_len,
};
pub use requirements_v2::{
    AtomicRequirementsV2, DeviceDescriptorTableV2, KernelTargetRequirementsV2, LdsRequirementsV2,
    RequiredWavefrontWidthV2, SynchronizationRequirementsV2,
};
pub use wire_v2::{
    CANONICAL_CODE_OBJECT_DIGEST_OFFSET_V2, DEVICE_DESCRIPTOR_VERSION_V2,
    decode_device_descriptor_table_v2, encode_device_descriptor_table_v2,
};
pub use wire_v3::decode_device_descriptor_table_v3;
pub use wire_v4::{
    decode_device_descriptor_table_v4, encode_device_descriptor_table_v4,
    encoded_device_descriptor_table_v4_len,
};

#[cfg(test)]
mod tests;
