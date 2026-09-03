//! Deterministic, target-neutral optimization over fe2o3 Kernel IR.
//!
//! IR definitions and structural verification remain owned by
//! [`fe2o3_kernel_ir`]. This crate owns the closed Pliron-backed V2
//! transformation policy used by production compilation and replay.

#![forbid(unsafe_code)]

mod optimization_v2;

pub use optimization_v2::*;
