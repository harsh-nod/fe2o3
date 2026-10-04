#![forbid(unsafe_code)]
#![deny(missing_docs)]

//! Bounded host-only SwiGLU operator foundation for Qwen3.
//!
//! This crate pins the exact Ferric M1 B3 target/draft shapes, an inert gfx942
//! elementwise schedule, BF16 storage with an explicit FP32 evaluation order,
//! an independent `f64` differential oracle, and fail-closed buffer/effect and
//! resource contracts. It does not compile, publish, load, dispatch, or launch
//! GPU code.
//!
//! The production compiler boundary remains closed pending the owner-consuming
//! same-session Rust MIR authority join tracked by fe2o3 issue #174. Nothing in
//! this crate establishes Rust-to-KIR, KIR-to-machine, IEEE-754, OCML, ISA,
//! hardware, performance, or protected-execution refinement.

mod bf16;
mod contract;
mod identity;
mod reference;

pub use bf16::{Bf16ConversionErrorV1, Bf16V1};
pub use contract::*;
pub use identity::*;
pub use reference::*;

/// Whether a production source-to-KIR correspondence exists for this operator.
pub const SWIGLU_SOURCE_TO_KIR_SUPPORTED_V1: bool = false;
/// Whether this foundation can create or publish a GPU artifact.
pub const SWIGLU_ARTIFACT_PUBLICATION_SUPPORTED_V1: bool = false;
/// Whether this foundation can load an artifact.
pub const SWIGLU_ARTIFACT_LOAD_SUPPORTED_V1: bool = false;
/// Whether this foundation can dispatch or launch GPU work.
pub const SWIGLU_GPU_LAUNCH_SUPPORTED_V1: bool = false;
/// Whether this foundation establishes source-to-machine refinement.
pub const SWIGLU_MACHINE_REFINEMENT_PROVED_V1: bool = false;
/// Current fail-closed production boundary.
pub const SWIGLU_PRODUCTION_BLOCKER_V1: &str = "the structural operator has no owner-consuming same-session Rust MIR authority join, proof discharge, machine refinement, artifact admission, or protected runtime join";
