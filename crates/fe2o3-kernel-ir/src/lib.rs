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
//! types without changing the frozen V1/V2/V3 encodings. [`encode_module_v5`]
//! adds explicit bounded matrix operations without changing the frozen V1-V4
//! encodings. [`encode_module_v6`] adds checked integer add/subtract/multiply
//! with explicit value and overflow results without changing the frozen V1-V5
//! encodings. [`encode_module_v7`] adds the complete checked tensor-layout
//! contract without changing the frozen V1-V6 encodings. [`encode_module_v8`]
//! adds gfx950 FP8 scaled matrix operations and layouts without changing the
//! frozen V1-V7 encodings.
//! [`encode_module_v9`] adds gfx950 collectives and LDS transpose operations;
//! [`encode_module_v10`] adds exact typed memory intrinsics without changing V1-V9.
//! Decoding establishes wire well-formedness only.
//! Consumers must call [`verify_module`] before relying on semantic invariants. V1-V10
//! reconstruct kernel-entry and import roles from their legacy records; they
//! reject device-FFI exports because the frozen function records cannot
//! distinguish those definitions from internal helpers.
//!
//! [`DebugSourceMapDocumentV2`] and [`VerifiedSimulationBundleV2`] are additive
//! source-variable/debugger formats. They do not reinterpret the frozen V1 map
//! or bundle and confer no compiler-execution, proof, load, launch, or hardware
//! authority.
//! [`VerifiedSimulationBundleV4`] is an additive aggregate-materialization
//! envelope. It retains the complete V3 bytes and binds a separately versioned
//! one-to-many semantic-component storage map without changing any earlier wire.
//!
//! [`SemanticDebugMapDocumentV1`] is a separate, finalized-artifact-bound sidecar for exact
//! bidirectional source/MIR/KIR/schedule/LLVM/ISA correlation. It represents optimization shape
//! and typed absence explicitly and does not broaden Source Map V1/V2 authority.
//!
//! SemanticOperation is the versioned extension boundary for typed
//! target-neutral operation families. Its separate schema and payload-bearing
//! instance codecs do not alter or extend any frozen module wire format.

mod canonical_kir_v10;
mod canonical_kir_v5;
mod canonical_kir_v6;
mod canonical_kir_v7;
mod canonical_kir_v8;
mod canonical_kir_v9;
mod control_flow;
mod debug_source_map_v1;
mod debug_source_map_v2;
mod effect_extraction;
mod formal_memory_obligations;
mod integer_semantic_oracle_v1;
mod interprocedural_effects;
mod ir;
#[allow(dead_code)]
#[path = "launch_kernel_v2.rs"]
mod launch_kernel_contract_v2;
mod matrix;
mod production_semantic_debug_fragment_v1;
mod region_effects;
pub mod scalar_ops_v2;
mod semantic_debug_map_v1;
mod semantic_operations;
mod simulation_bundle_v1;
mod simulation_bundle_v2;
mod simulation_bundle_v3;
mod simulation_bundle_v4;
mod standard_atomics;
mod types;
mod verify;
mod wave_operations;
mod wire;

pub use canonical_kir_v5::*;
pub use canonical_kir_v6::*;
pub use canonical_kir_v7::*;
pub use canonical_kir_v8::*;
pub use canonical_kir_v9::*;
pub use canonical_kir_v10::*;
pub use control_flow::*;
pub use debug_source_map_v1::*;
pub use debug_source_map_v2::*;
pub use effect_extraction::*;
pub use formal_memory_obligations::*;
pub use integer_semantic_oracle_v1::*;
pub use interprocedural_effects::{
    InterproceduralEffectAnalysisV1, InterproceduralEffectDecisionV1,
    InterproceduralEffectIncompleteReasonV1, MAX_INTERPROCEDURAL_EFFECT_CALL_EDGES_V1,
    MAX_INTERPROCEDURAL_EFFECT_FUNCTIONS_V1, analyze_interprocedural_effects_from_verified_v1,
    analyze_interprocedural_effects_v1,
};
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
pub use production_semantic_debug_fragment_v1::*;
pub use region_effects::*;
pub use semantic_debug_map_v1::*;
pub use semantic_operations::*;
pub use simulation_bundle_v1::*;
pub use simulation_bundle_v2::*;
pub use simulation_bundle_v3::*;
pub use simulation_bundle_v4::*;
pub use standard_atomics::*;
pub use types::*;
pub use verify::*;
pub use wave_operations::*;
pub use wire::*;
