#![forbid(unsafe_code)]
#![cfg_attr(target_arch = "amdgpu", no_std)]

//! Safe ordinary-Rust source for the dynamic strided GEMM qualification kernel.
//!
//! The intended production transaction imports the attributed Rust MIR, proves
//! the generic ranked-memory and workgroup-pipeline obligations, lowers through
//! Kernel IR and LLVM, and emits the HSACO consumed by the qualification runner.

pub mod contract;
pub mod kernel;
#[cfg(not(target_arch = "amdgpu"))]
pub mod reference;
#[cfg(all(not(target_arch = "amdgpu"), feature = "bundle-v8-simulator"))]
pub mod simulator;

/// Whether the attributed kernel contains only safe ordinary Rust.
pub const GENERAL_TILED_GEMM_SAFE_SOURCE_PRESENT_V1: bool = true;
/// Whether production compilation reaches verified Kernel IR.
pub const GENERAL_TILED_GEMM_SOURCE_TO_IR_SUPPORTED_V1: bool = false;
/// Whether production compilation currently reaches deterministic target LLVM.
pub const GENERAL_TILED_GEMM_SOURCE_LOWERING_SUPPORTED_V1: bool = false;
/// Whether the checked-in runner can currently execute a qualification HSACO.
pub const GENERAL_TILED_GEMM_QUALIFICATION_EXECUTION_SUPPORTED_V1: bool = false;
/// Whether the qualification artifact grants protected release authority.
pub const GENERAL_TILED_GEMM_PROTECTED_EXECUTION_SUPPORTED_V1: bool = false;

/// Remaining boundary between qualification execution and protected release.
pub const GENERAL_TILED_GEMM_PROTECTED_EXECUTION_BLOCKER_V1: &str = "typed Global-to-matrix, disjoint global read-modify-write, dynamic workgroup epochs, and source numerical binding are unavailable; Bundle V8 export and the sealed W6/W7 joins remain incomplete";
