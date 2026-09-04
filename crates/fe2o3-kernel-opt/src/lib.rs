//! Deterministic optimization over verified canonical fe2o3 Kernel IR.
//!
//! IR definitions and structural verification remain owned by
//! [`fe2o3_kernel_ir`]. This crate owns the closed Pliron-backed V2
//! transformation policy and its exact V10 and V11 transport endpoints used
//! by production compilation and replay.

#![forbid(unsafe_code)]

mod optimization_v2;
mod optimization_v3;
mod structural_replay_admission_v2;
mod structural_replay_admission_v3;

pub use optimization_v2::*;
pub use optimization_v3::*;
pub use structural_replay_admission_v2::*;
pub use structural_replay_admission_v3::*;
