//! Deterministic optimization over verified canonical fe2o3 Kernel IR.
//!
//! IR definitions and structural verification remain owned by
//! [`fe2o3_kernel_ir`]. This crate owns the closed Pliron-backed V2
//! transformation policy and its exact V10, V11, V12, and V13 transport endpoints
//! used by production compilation and replay. V13 capability facts remain on
//! the one canonical graph and are checked by affected-analysis replay after
//! the closed pass sequence.

#![forbid(unsafe_code)]

mod loop_memory_transforms_v1;
mod optimization_v2;
mod optimization_v3;
mod optimization_v4;
mod optimization_v5;
mod optimization_v6;
mod optimizer_queries_v1;
mod source_coordinate_lineage_v1;
mod structural_replay_admission_v2;
mod structural_replay_admission_v3;
mod structural_replay_admission_v4;
mod structural_replay_admission_v6;
mod target_neutral_cost_v1;
mod target_neutral_module_transforms_v1;
mod transformation_preservation_v1;

pub use loop_memory_transforms_v1::*;
pub use optimization_v2::*;
pub use optimization_v3::*;
pub use optimization_v4::*;
pub use optimization_v5::*;
pub use optimization_v6::*;
pub use optimizer_queries_v1::*;
pub use source_coordinate_lineage_v1::*;
pub use structural_replay_admission_v2::*;
pub use structural_replay_admission_v3::*;
pub use structural_replay_admission_v4::*;
pub use structural_replay_admission_v6::*;
pub use target_neutral_cost_v1::*;
pub use target_neutral_module_transforms_v1::*;
pub use transformation_preservation_v1::*;
