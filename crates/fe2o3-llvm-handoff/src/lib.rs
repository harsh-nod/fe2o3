#![no_std]
#![forbid(unsafe_code)]
#![doc = include_str!("../README.md")]

extern crate alloc;

mod codec;
mod error;
mod model;

pub use codec::{
    CanonicalHandoffBytesV1, DecodeHandoffErrorV1, MAX_CANONICAL_HANDOFF_BYTES_V1, WireSectionV1,
};
pub use error::{HandoffDiagnosticV1, HandoffLimitV1};
pub use model::{
    AddressSpaceV1, CallingConventionV1, CodeModelV1, CodeObjectVersionV1, DeviceLibraryInputV1,
    DeviceLibraryKindV1, FunctionAttributeV1, GFX942_AMDHSA_DATA_LAYOUT_V1,
    GFX942_AMDHSA_TARGET_TRIPLE_V1, Gfx942HandoffInputV1, Gfx942HandoffV1, Gfx942TargetPolicyV1,
    HandoffIdentityV1, IdentityV1, KernelEntryV1, KernelParameterV1, KernelReturnTypeV1,
    KernelValueTypeV1, MAX_DEVICE_LIBRARIES_V1, MAX_DEVICE_LIBRARY_BYTES_V1,
    MAX_FUNCTION_ATTRIBUTES_V1, MAX_KERNEL_PARAMETERS_V1, MAX_KERNELS_V1, MAX_MODULE_FLAGS_V1,
    MAX_NAMED_METADATA_V1, MAX_OBLIGATIONS_V1, MAX_ORIGINS_V1, MAX_PARAMETER_ATTRIBUTES_V1,
    MAX_SOURCE_PATH_BYTES_V1, MAX_SYMBOL_BYTES_V1, ModuleFlagV1, ModuleMetadataV1, NamedMetadataV1,
    ObligationIdentityV1, ObligationKindV1, ObligationV1, OptimizationLevelV1, OriginIdentityV1,
    OriginKindV1, OriginV1, ParameterAttributeV1, RelocationModelV1, ScalarTypeV1, SourceSpanV1,
    StageIdentitiesV1, TargetFeatureStateV1, TargetFeatureV1, WavesPerEuV1, WorkgroupSizeRangeV1,
};
