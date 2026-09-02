//! Deterministic, target-neutral optimization over fe2o3 Kernel IR.
//!
//! IR definitions and structural verification remain owned by
//! [`fe2o3_kernel_ir`]. This crate owns transformation policy. The V1 passes
//! use no whole-function analysis result, so the crate deliberately has no
//! analysis dependency; a later pass should add one only when it consumes an
//! immutable analysis report.

#![forbid(unsafe_code)]

mod optimization_v1;

pub use optimization_v1::*;
