#![forbid(unsafe_code)]
#![deny(missing_docs)]

//! Bounded host-only RMSNorm plus residual operator foundation for Qwen3.
//!
//! The crate pins the exact M1 B3 workload matrix, a gfx942 Wave64 structural
//! schedule, BF16 input/output encoding, FP32 evaluation order, independent
//! host differential semantics, and explicit memory/effect/resource
//! contracts. Its identities describe inert algorithm and schedule records.
//! They are not compiler evidence, GPU artifacts, load handles, or launch
//! authority.
//!
//! The production compiler boundary remains closed pending the same-session
//! Rust MIR authority join tracked by fe2o3 issue #174. This crate makes no
//! Rust-to-KIR, KIR-to-LLVM/ISA, machine-safety, IEEE-754, performance, or
//! protected-execution refinement claim.

mod bf16;
mod contract;
mod identity;
mod reference;

pub use bf16::{Bf16ConversionErrorV1, Bf16V1};
pub use contract::*;
pub use identity::*;
pub use reference::*;

/// Whether a production source-to-KIR correspondence exists for this operator.
pub const RMSNORM_RESIDUAL_SOURCE_TO_KIR_SUPPORTED_V1: bool = false;
/// Whether this foundation can create or publish a GPU artifact.
pub const RMSNORM_RESIDUAL_ARTIFACT_PUBLICATION_SUPPORTED_V1: bool = false;
/// Whether this foundation can load an artifact.
pub const RMSNORM_RESIDUAL_ARTIFACT_LOAD_SUPPORTED_V1: bool = false;
/// Whether this foundation can dispatch or launch GPU work.
pub const RMSNORM_RESIDUAL_GPU_LAUNCH_SUPPORTED_V1: bool = false;
/// Whether this foundation establishes a source-to-machine refinement.
pub const RMSNORM_RESIDUAL_MACHINE_REFINEMENT_PROVED_V1: bool = false;
/// Current fail-closed production boundary.
pub const RMSNORM_RESIDUAL_PRODUCTION_BLOCKER_V1: &str = "the structural operator has no owner-consuming same-session Rust MIR authority join, proof discharge, machine refinement, artifact admission, or protected runtime join";
