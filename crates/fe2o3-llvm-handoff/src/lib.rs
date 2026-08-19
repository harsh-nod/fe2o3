#![no_std]
#![forbid(unsafe_code)]
#![doc = include_str!("../README.md")]

extern crate alloc;

mod codec;
mod codec_v2;
mod error;
mod error_v2;
mod model;
mod model_v2;

pub use codec::{
    CanonicalHandoffBytesV1, DecodeHandoffErrorV1, MAX_CANONICAL_HANDOFF_BYTES_V1, WireSectionV1,
};
pub use codec_v2::{CanonicalHandoffBytesV2, DecodeHandoffErrorV2, WireSectionV2};
pub use error::{HandoffDiagnosticV1, HandoffLimitV1};
pub use error_v2::{DefinitionKindV2, HandoffDiagnosticV2, HandoffLimitV2};
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
pub use model_v2::{
    AxisV2, BasicBlockV2, BinaryOperationV2, BlockIdV2, CallTargetV2, CallingConventionV2,
    CastOperationV2, ComparePredicateV2, EvidenceV2, ExecutableModuleV2, FloatBinaryOperationV2,
    FunctionAttributeV2, FunctionIdV2, FunctionKindV2, FunctionParameterV2, FunctionV2,
    GENERAL_GEMM_BINDING_SECTION_V2, GENERAL_GEMM_LDS_ELEMENTS_V2, GENERAL_GEMM_VECTOR_LANES_V2,
    Gfx942HandoffV2, GlobalIdV2, GlobalLinkageV2, GlobalV2, HandoffIdentityV2, InstructionKindV2,
    InstructionV2, IntegerBinaryOperationV2, IntrinsicReferenceV2, IntrinsicV2,
    KERNEL_DESCRIPTOR_SECTION_V2, MAX_CANONICAL_HANDOFF_BYTES_V2, MAX_CONSTANT_GLOBAL_BYTES_V2,
    MAX_EVIDENCE_OBLIGATIONS_V2, MAX_FUNCTION_ATTRIBUTES_V2, MAX_FUNCTION_BLOCKS_V2,
    MAX_FUNCTION_PARAMETERS_V2, MAX_FUNCTIONS_V2, MAX_GEP_INDICES_V2, MAX_GLOBALS_V2,
    MAX_INSTRUCTIONS_PER_FUNCTION_V2, MAX_INTRINSICS_V2, MAX_MODULE_FLAGS_V2,
    MAX_NAMED_METADATA_V2, MAX_PARAMETER_ATTRIBUTES_V2, MAX_SYMBOL_BYTES_V2,
    MAX_VALUES_PER_FUNCTION_V2, ModuleIdentityV2, ReturnTypeV2, ScalarConstantV2, TerminatorV2,
    TypedValueV2, ValueIdV2, ValueTypeV2,
};
