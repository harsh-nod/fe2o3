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
mod checked_store_forwarding_v1;
mod checked_u32_local_order_v1;
mod optimization_v2;
mod optimization_v3;
mod owned_cross_block_forwarding_v1;
mod owned_induction_refinement_v1;
mod owned_licm_v1;
mod owned_loop_preheaders_v1;
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
pub use checked_store_forwarding_v1::*;
pub use checked_u32_local_order_v1::*;
pub use optimization_v2::*;
pub use optimization_v3::*;
pub use owned_cross_block_forwarding_v1::*;
pub use owned_induction_refinement_v1::*;
pub use owned_licm_v1::*;
pub use owned_loop_preheaders_v1::*;
pub use owned_private_cell_promotion_v1::*;
pub use structural_replay_admission_v2::*;
pub use structural_replay_admission_v3::*;
