#![forbid(unsafe_op_in_unsafe_fn)]
#![deny(missing_docs)]

//! Exact Phase A source and proof-facing host contracts for one bounded
//! FlashAttention forward profile.
//!
//! This crate contains real ordinary attributed Rust kernel source. It does
//! not claim compiler lowering, an admitted descriptor, an HSACO artifact,
//! hardware execution, protected evidence, or a machine-checked refinement
//! proof from the source to a model.

pub mod contract;
pub mod kernel;
pub mod oracle;
pub mod proof_model;
pub mod source_identity;
pub mod vectors;

pub use contract::{
    ACCUMULATION_POLICY_V1, CAUSAL_POLICY_V1, DTYPE_POLICY_V1, EXACT_PROFILE_V1,
    EXCEPTIONAL_VALUE_POLICY_V1, FLASH_ATTENTION_HEAD_DIMENSION_V1,
    FLASH_ATTENTION_INPUT_ELEMENTS_V1, FLASH_ATTENTION_OUTPUT_ELEMENTS_PER_LANE_V1,
    FLASH_ATTENTION_OUTPUT_ELEMENTS_V1, FLASH_ATTENTION_SEQUENCE_LENGTH_V1,
    FLASH_ATTENTION_WAVE_LANES_V1, FlashAttentionProfileV1, LayoutV1, MaskPolicyV1,
    ProfileMismatchV1, TensorV1, exact_launch_v1, key_participates_v1, lane_outputs_v1,
    qkv_index_v1, validate_profile_v1,
};
pub use oracle::{
    ArithmeticStageV1, FlashAttentionOracleErrorV1, FlashAttentionOracleStateV1,
    flash_attention_oracle_v1,
};
pub use proof_model::{
    AccessCoordinateV1, MODEL_ASSURANCE_V1, OnlineInvariantViolationV1, OnlineModelErrorV1,
    OnlineStateV1, OwnershipViolationV1, SOURCE_MODEL_REFINEMENT_PROVED_V1, access_coordinate_v1,
    exact_ownership_map_v1, online_trace_v1, validate_online_state_v1,
    validate_output_ownership_v1,
};
pub use source_identity::{
    FLASH_ATTENTION_KERNEL_NAMESPACE_V1, FLASH_ATTENTION_KERNEL_SOURCE_SHA256_V1,
    FLASH_ATTENTION_PROFILE_IDENTITY_V1, SourceIdentityMismatchV1,
    validate_kernel_source_identity_v1,
};
pub use vectors::{DeterministicVectorV1, deterministic_vectors_v1};
