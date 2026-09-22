//! Deterministic optimization over verified canonical fe2o3 Kernel IR.
//!
//! IR definitions and structural verification remain owned by
//! [`fe2o3_kernel_ir`]. This crate owns the closed Pliron-backed V2
//! transformation policy and its exact V10 and V11 transport endpoints used
//! by production compilation and replay.
//!
//! The additive checked V12 adapter observes fixed scalar/CFG execution and
//! independently checks the actual input-to-final local rewrite relation. It
//! neither establishes target binding nor changes those historical endpoints.

#![forbid(unsafe_code)]
mod expanded_history_v1;
mod scalar_fixed_point_history_decode_v1;
mod scalar_fixed_point_history_v1;
pub use expanded_history_v1::{
    DecodedExpandedHistoryV1, EXPANDED_HISTORY_DOMAIN_V1, EXPANDED_HISTORY_MAGIC_V1,
    EXPANDED_HISTORY_POLICY_ID_V1, ExpandedHistoryErrorV1, ExpandedHistoryStorageV1,
    InertExpandedHistoryBytesV1, InertExpandedHistoryRefV1, MAX_EXPANDED_HISTORY_BYTES_V1,
    ReplayedExpandedHistoryV1, encode_expanded_history_v1, materialize_expanded_history_v1,
    read_expanded_history_v1,
};
pub use scalar_fixed_point_history_decode_v1::{
    DecodedScalarFixedPointHistoryV1, DecodedScalarFixedPointRoundV1,
    ReplayedScalarFixedPointHistoryV1, ReplayedScalarFixedPointRoundV1,
    materialize_scalar_fixed_point_history_v1,
};
pub use scalar_fixed_point_history_v1::{
    InertScalarFixedPointHistoryBytesV1, InertScalarFixedPointHistoryRefV1,
    InertScalarFixedPointRoundRefV1, MAX_SCALAR_FIXED_POINT_HISTORY_BYTES_V1,
    SCALAR_FIXED_POINT_HISTORY_DOMAIN_V1, SCALAR_FIXED_POINT_HISTORY_MAGIC_V1,
    ScalarFixedPointHistoryErrorV1, ScalarFixedPointHistoryStorageV1,
    encode_scalar_fixed_point_history_v1, read_scalar_fixed_point_history_v1,
    reencode_scalar_fixed_point_history_v1,
};
mod loop_unroll_history_decode_v1;
mod loop_unroll_history_rows_v1;
mod loop_unroll_history_wire_v1;
mod refined_forwarding_history_decode_v1;
mod refined_forwarding_history_rows_v1;
mod refined_forwarding_history_wire_v1;
pub use loop_unroll_history_decode_v1::*;
pub use loop_unroll_history_wire_v1::{
    InertLoopUnrollHistoryBytesV1, InertLoopUnrollHistoryRefV1, LOOP_UNROLL_HISTORY_MAGIC_V1,
    LoopUnrollHistoryInputsV1, LoopUnrollHistoryWireErrorV1, LoopUnrollHistoryWireStorageV1,
    MAX_LOOP_UNROLL_HISTORY_BYTES_V1, MAX_LOOP_UNROLL_HISTORY_GRAPH_BYTES_V1,
    MAX_LOOP_UNROLL_HISTORY_ROW_BYTES_V1, MAX_LOOP_UNROLL_HISTORY_ROWS_V1,
    MAX_LOOP_UNROLL_HISTORY_STORAGE_V1, encode_loop_unroll_history_v1, read_loop_unroll_history_v1,
};
pub use refined_forwarding_history_decode_v1::*;
pub use refined_forwarding_history_wire_v1::*;

mod checked_load_forwarding_v1;
mod checked_optimization_policy3_receipt_v1;
mod checked_optimization_policy3_v1;
mod checked_optimization_policy4_receipt_v1;
mod checked_optimization_policy4_v1;
mod checked_optimization_policy5_semantic_v1;
mod checked_optimization_policy5_v1;
mod checked_optimization_policy6_semantic_v1;
mod checked_optimization_policy6_v1;
mod checked_optimization_policy7_semantic_v1;
mod checked_optimization_policy8_composition_v1;
mod checked_optimization_policy8_graph_pool_v1;
mod checked_optimization_policy8_history_inputs_v1;
mod checked_optimization_policy8_semantic_v1;
mod checked_optimization_policy8_transport_v1;
mod checked_optimization_receipt_v1;
mod checked_optimization_v1;
mod checked_redundant_store_v1;
mod checked_refined_forwarding_history_v1;
mod checked_scalar_fixed_point_v1;
mod checked_store_forwarding_v1;
mod checked_u32_local_order_v1;
mod optimization_v2;
mod optimization_v3;
mod owned_cross_block_forwarding_v1;
mod owned_induction_refinement_v1;
mod owned_licm_v1;
mod owned_loop_preheaders_v1;
mod owned_loop_unroll_v1;
mod owned_private_cell_promotion_v1;
mod private_cell_promotion_resources_v1;
mod structural_replay_admission_v2;
mod structural_replay_admission_v3;

pub use checked_load_forwarding_v1::*;
pub use checked_optimization_policy3_receipt_v1::*;
pub use checked_optimization_policy3_v1::*;
pub use checked_optimization_policy4_receipt_v1::*;
pub use checked_optimization_policy4_v1::*;
pub use checked_optimization_policy5_semantic_v1::*;
pub use checked_optimization_policy5_v1::*;
pub use checked_optimization_policy6_semantic_v1::*;
pub use checked_optimization_policy6_v1::*;
pub use checked_optimization_policy7_semantic_v1::*;
pub use checked_optimization_policy8_composition_v1::*;
pub use checked_optimization_policy8_graph_pool_v1::*;
pub use checked_optimization_policy8_history_inputs_v1::*;
pub use checked_optimization_policy8_semantic_v1::*;
pub use checked_optimization_policy8_transport_v1::*;
pub use checked_optimization_receipt_v1::*;
pub use checked_optimization_v1::*;
pub use checked_redundant_store_v1::*;
pub use checked_refined_forwarding_history_v1::*;
pub use checked_scalar_fixed_point_v1::{
    CheckedScalarFixedPointErrorV1, CheckedScalarFixedPointOwnerV1, CheckedScalarFixedPointRoundV1,
    SCALAR_FIXED_POINT_EXECUTION_BYTES_V1, SCALAR_FIXED_POINT_MAX_ROUNDS_V1,
    SCALAR_FIXED_POINT_POLICY_ID_V1, ScalarFixedPointExecutionV1,
    prepare_checked_scalar_fixed_point_v1,
};
pub use checked_store_forwarding_v1::*;
pub use checked_u32_local_order_v1::*;
pub use optimization_v2::*;
pub use optimization_v3::*;
pub use owned_cross_block_forwarding_v1::*;
pub use owned_induction_refinement_v1::*;
pub use owned_licm_v1::*;
pub use owned_loop_preheaders_v1::*;
pub use owned_loop_unroll_v1::*;
pub use owned_private_cell_promotion_v1::*;
pub use structural_replay_admission_v2::*;
pub use structural_replay_admission_v3::*;
