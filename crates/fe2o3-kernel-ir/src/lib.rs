//! Target-neutral semantic kernel IR for fe2o3.
//!
//! The crate intentionally has no dependency on rustc, LLVM, or a GPU vendor.
//! Frontends construct this IR, target-independent passes verify and transform
//! it, and target backends lower it to their native representation.
//!
//! [`encode_module_v1`] and [`decode_module_v1`] preserve the original bounded
//! canonical wire representation. [`encode_module_v2`] adds synchronization,
//! LDS, exact wave-width records, and typed canonical integer switches.
//! [`encode_module_v3`] adds source-bound inline assembly without changing the
//! frozen V1/V2 encodings. [`encode_module_v4`] adds 128-bit scalar carrier
//! types without changing the frozen V1/V2/V3 encodings. Decoding establishes
//! wire well-formedness only; consumers must call [`verify_module`] before
//! relying on semantic invariants.
//! V1/V2/V3/V4 reconstruct kernel-entry and import roles from their legacy records;
//! they reject device-FFI exports because the frozen function records cannot
//! distinguish those definitions from internal helpers.
//!
//! SemanticOperation is the versioned extension boundary for typed
//! target-neutral operation families. Its separate schema and payload-bearing
//! instance codecs do not alter or extend any frozen module wire format.

mod control_flow;
mod effect_extraction;
mod formal_memory_obligations;
mod ir;
#[allow(dead_code)]
#[path = "launch_kernel_v2.rs"]
mod launch_kernel_contract_v2;
mod matrix;
mod region_effects;
mod scalar_gemm_v1;
pub mod scalar_ops_v2;
mod semantic_operations;
mod standard_atomics;
mod tiled_gemm_lds_edges_v1;
mod tiled_gemm_lds_grid_v1;
mod tiled_gemm_lds_k32_v2;
mod tiled_gemm_lds_v1;
mod tiled_gemm_v1;
mod types;
mod verify;
mod wire;

pub use control_flow::*;
pub use effect_extraction::*;
pub use formal_memory_obligations::*;
pub use ir::*;
#[doc(hidden)]
pub use launch_kernel_contract_v2::{
    AbiParameterKindV2, AbiParameterV2, AmdArchitectureV2, ArtifactIdentityV2, BlockShapePolicyV2,
    CodeObjectVersionV2, DimensionsV2, EndiannessV2, GFX942_REQUIRED_WAVEFRONT_WIDTH_V2,
    Gfx942LaunchContractV2, Gfx942OccupancyWitnessV2, Gfx942ResourceLimitsV2,
    Gfx942TargetBindingV2, KernelFamilyIdentityV2, KernelIdentityV2, KernelPolicyIdentityV2,
    KernelSignatureIdentityV2, KernelSignatureV2, KernelVariantTupleIdentityV2, KernelVariantV2,
    LaunchCapabilityV2, LaunchKernelFamilyV2, LaunchKernelLimitsV2, LaunchKernelValidationErrorV2,
    LaunchProofKindV2, LaunchProofObligationV2, OccupancyMetadataIdentityV2,
    OccupancySubjectIdentityV2, OccupancyVerifierIdentityV2, SemanticTypeIdentityV2,
    TargetIdentityV2, UnsupportedLaunchFeaturesV2, WavefrontWidthV2,
    canonical_occupancy_subject_identity_v2, canonical_variant_tuple_identity_v2,
};
pub use matrix::*;
pub use region_effects::*;
pub use scalar_gemm_v1::*;
pub use semantic_operations::*;
pub use standard_atomics::*;
pub use tiled_gemm_lds_edges_v1::*;
pub use tiled_gemm_lds_grid_v1::*;
pub use tiled_gemm_lds_k32_v2::*;
pub use tiled_gemm_lds_v1::*;
pub use tiled_gemm_v1::*;
pub use types::*;
pub use verify::*;
pub use wire::*;
